//! The field, the rule applied to it, and the keys that drive both.
//!
//! Pure logic: no wgpu, no colour, no window. It runs headless under the determinism gate, and
//! `main.rs` is the half that decides what any of it looks like.
//!
//! # What one generation costs
//!
//! The resolution control goes down to one cell per physical pixel, which on a large display is
//! several million cells, sixty times over if you ask for it. So the step is written for that
//! case rather than for the pretty one.
//!
//! Neighbour counting is split from applying the rule. Counting is the part that differs
//! between the three families and it is done three ways:
//!
//! - **Eight neighbours** — the ordinary case, and the one that has to be fast. Counting each
//!   cell's eight neighbours directly reads every cell nine times. Instead each row is summed
//!   into threes once, and then a cell's count is three of those sums added together: five
//!   additions a cell rather than eight, and every read is sequential.
//! - **A wide neighbourhood** — Larger than Life reaches out as far as radius ten, which is
//!   four hundred and forty-one cells. Counting those one at a time is not affordable at any
//!   resolution, so a summed-area table is built over the field and every count, whatever the
//!   radius, is four lookups. The table is built over a copy padded by the radius, which is
//!   what lets the wrapped edge and the walled edge share one code path.
//! - **Four neighbours** — the von Neumann case, which is small enough to read directly.
//!
//! Applying the rule is then one pass that all three share, and it is the pass that also keeps
//! the age, the trails, the population, and the checksum this generation is recognised by.
//!
//! # What is simulation and what is not
//!
//! The age of a cell and the trail it leaves are computed here, but nothing reads them back:
//! they cannot change what the rule does. They are in this file because they are per-cell
//! histories that have to be updated in step with the field, and they are here rather than in
//! the renderer because the renderer sees one frame at a time and these are about time.
//!
//! The window's size arrives on the replayable command channel as [`RESIZE_COMMAND`], never by
//! reading renderer state, which is what lets a headless run reshape the same way a windowed
//! one does.

use crate::rules::{RULES, Rule, Seeding, Shape};
use fulcrum::prelude::*;

// ---------------------------------------------------------------------------------------
// the shape of the field
// ---------------------------------------------------------------------------------------

/// Window size at startup.
pub const DEFAULT_WINDOW: Vec2 = Vec2::new(1600.0, 1000.0);

/// Name of the resize command on the replayable command channel.
pub const RESIZE_COMMAND: &str = "window";

/// How many physical pixels a cell may be drawn at, coarsest last.
///
/// The first entry is the point of the whole ladder: one cell to one pixel, no upscale
/// anywhere, so a full display is a field of several million cells and the rule is being
/// evaluated at the finest grain the screen can show. The last is big enough to count the
/// neighbours of a cell with your finger.
pub const CELL_SIZES: &[u32] = &[1, 2, 3, 4, 5, 6, 8, 10, 12, 16, 20, 24, 32, 40, 48, 64];

/// Which of [`CELL_SIZES`] the piece opens at: six pixels, which is small enough to hold a
/// field worth watching and big enough that a glider is plainly a glider.
pub const OPENING_SIZE: usize = 5;

/// Fewest cells across the field, so a sliver of a window still has something in it.
pub const GRID_MIN: u32 = 8;

/// Most cells the field is allowed to hold, whatever the window and the resolution ask for.
///
/// A backstop rather than a working limit: a 4K display at one cell to the pixel is eight and a
/// third million, well inside this. It is here so that an absurd window on an absurd display
/// raises the cell size rather than trying to allocate its way to a halt.
pub const CELL_CAP: usize = 40_000_000;

/// Cells the upload buffer is padded to a multiple of.
///
/// Two bytes go up per cell, so a row padded to a multiple of 128 cells is a multiple of 256
/// bytes, which is the alignment a texture write wants. Paying for it here means the field can
/// be any width at all.
pub const STRIDE_ALIGN: u32 = 128;

// ---------------------------------------------------------------------------------------
// pace
// ---------------------------------------------------------------------------------------

/// Slowest the field is allowed to run, in generations a second.
pub const PACE_MIN: f32 = 0.25;
/// Fastest. Past this a generation is gone before the display has shown it.
pub const PACE_MAX: f32 = 480.0;
/// Generations a second at startup.
pub const PACE_START: f32 = 15.0;
/// How much the pace is multiplied per second of holding the key.
const PACE_RAMP: f32 = 4.0;

/// Most generations one simulation tick will run, however fast the pace is set and however
/// small the field is.
///
/// The pace is what you asked for; this is what the tick will actually do before handing the
/// frame back. Past it the field runs slower than the dial says rather than the window stopping
/// answering, and the readout says so by showing the pace actually achieved.
pub const MAX_STEPS_PER_TICK: u32 = 12;

/// Cells a tick will bring forward before it hands the frame back, whatever that costs in
/// generations.
///
/// A cell budget and not a time budget, and the difference matters twice over. It is what keeps
/// this deterministic: a tick that watched the clock would run a different number of
/// generations on a fast machine than on a slow one, and a replay would diverge on the first
/// one it was played back on. And it is the right thing to bound anyway, because what a
/// generation costs is not a constant — a field at one cell to the pixel is a hundred times the
/// work of the same window at ten, and twelve generations of it in one tick is a window that has
/// stopped answering. So a small field gets [`MAX_STEPS_PER_TICK`] generations a tick and a
/// display-sized one gets one, and the readout is honest about the difference.
pub const CELLS_PER_TICK: usize = 5_000_000;

/// How much of a trail one generation takes away. Sixteen is about sixteen generations from
/// the moment a cell dies to the moment there is no sign it was there.
const TRAIL_DECAY: u8 = 16;

/// How many generations a cell has to hold before it counts as fully settled, for the colour
/// that reads age. Small, because in Life most things that last at all last forever.
const AGE_SOFTNESS: u32 = 8;

/// How many past generations are remembered for spotting a field that has stopped going
/// anywhere. Long enough to catch the common oscillators and the pulsar's fifteen.
const MEMORY: usize = 32;

/// How many settled generations pass before the field is sown again, when it has been asked to
/// do that by itself.
const PATIENCE: u32 = 90;

// ---------------------------------------------------------------------------------------
// how a field is started
// ---------------------------------------------------------------------------------------

/// What the number keys put on an empty field.
///
/// The four patterns at the end are Life's, and famously so, but they are stamped whatever rule
/// is loaded: an acorn under Day & Night is not an acorn for very long, and watching what
/// becomes of it is a fair way to learn what the rule does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Start {
    /// Whatever the rule itself asks for. Every rule carries one.
    Native,
    /// Random cells over the whole field.
    Soup,
    /// Random cells in a square in the middle.
    Patch,
    /// A soup mirrored into all four quadrants, which stays symmetric forever because the rule
    /// is. Worth doing once on any rule at all.
    Symmetry,
    /// A single live cell.
    Spark,
    /// The R-pentomino: five cells, and eleven hundred generations of consequences.
    Pentomino,
    /// The acorn: seven cells that take five thousand generations to settle.
    Acorn,
    /// Gosper's glider gun, the first pattern shown to grow without limit.
    Gun,
    /// The diehard: seven cells that vanish completely after a hundred and thirty generations.
    Diehard,
    /// Nothing. Draw your own with the mouse.
    Empty,
}

/// The starts, in the order the number keys walk them.
pub const STARTS: &[Start] = &[
    Start::Native,
    Start::Soup,
    Start::Patch,
    Start::Symmetry,
    Start::Spark,
    Start::Pentomino,
    Start::Acorn,
    Start::Gun,
    Start::Diehard,
    Start::Empty,
];

impl Start {
    /// Its name, for the readout.
    pub fn name(self) -> &'static str {
        match self {
            Start::Native => "the rule's own",
            Start::Soup => "soup",
            Start::Patch => "patch",
            Start::Symmetry => "symmetry",
            Start::Spark => "one cell",
            Start::Pentomino => "R-pentomino",
            Start::Acorn => "acorn",
            Start::Gun => "glider gun",
            Start::Diehard => "diehard",
            Start::Empty => "empty",
        }
    }
}

/// The R-pentomino, as offsets from its top-left corner.
const PENTOMINO: &[(i32, i32)] = &[(1, 0), (2, 0), (0, 1), (1, 1), (1, 2)];

/// The acorn.
const ACORN: &[(i32, i32)] = &[(1, 0), (3, 1), (0, 2), (1, 2), (4, 2), (5, 2), (6, 2)];

/// The diehard.
const DIEHARD: &[(i32, i32)] = &[(6, 0), (0, 1), (1, 1), (1, 2), (5, 2), (6, 2), (7, 2)];

/// Gosper's glider gun: thirty-six cells that emit a glider every thirty generations.
#[rustfmt::skip]
const GUN: &[(i32, i32)] = &[
                                                                        (24, 0),
                                                      (22, 1),          (24, 1),
              (12, 2), (13, 2),          (20, 2), (21, 2),                                (34, 2), (35, 2),
     (11, 3),                   (15, 3), (20, 3), (21, 3),                                (34, 3), (35, 3),
    (0, 4), (1, 4), (10, 4),    (16, 4), (20, 4), (21, 4),
    (0, 5), (1, 5), (10, 5),    (14, 5), (16, 5), (17, 5), (22, 5),      (24, 5),
                    (10, 6),    (16, 6),                                (24, 6),
             (11, 7),           (15, 7),
              (12, 8), (13, 8),
];

// ---------------------------------------------------------------------------------------
// the field
// ---------------------------------------------------------------------------------------

/// The field and everything that has happened to it.
///
/// `cells` is the simulation. `age` and `trail` are histories kept alongside it that nothing in
/// the rule ever reads: they exist so that the picture can show *when* as well as *what*.
/// `pixels` is those three folded into the two bytes a cell is drawn from, ready to go to the
/// GPU without the renderer having to know any of this.
#[derive(Resource)]
pub struct Board {
    /// Cells across.
    pub width: u32,
    /// Cells down.
    pub height: u32,
    /// One byte a cell: 0 empty, 1 alive, and 2 upwards for the stages of dying that a
    /// Generations rule adds. A two-state rule never leaves 0 and 1.
    pub cells: Vec<u8>,
    /// How many generations each live cell has been alive, saturating.
    pub age: Vec<u8>,
    /// How lately each cell was alive, decaying by [`TRAIL_DECAY`] a generation.
    pub trail: Vec<u8>,
    /// Two bytes a cell, ready to upload: what the cell is, and how long it has been that.
    /// Rows are [`Board::stride`] cells apart, not `width`.
    pub pixels: Vec<u8>,
    /// Cells between the starts of two rows of `pixels`. `width` rounded up.
    pub stride: u32,
    /// Generations since the field was last sown.
    pub generation: u64,
    /// Live cells now.
    pub population: u32,
    /// Cells that came alive on the last generation.
    pub births: u32,
    /// Cells that stopped being alive on it.
    pub deaths: u32,
    /// Bumped whenever `pixels` changed, so the renderer can upload only when there is
    /// something new to upload.
    pub revision: u64,
    /// The period the field has fallen into, if it has fallen into one. A still field is one.
    pub period: Option<u32>,
    /// How many generations in a row it has had a period.
    pub settled: u32,
    /// Checksums of the last [`MEMORY`] generations, newest first. What `period` is found in.
    marks: [u64; MEMORY],
    /// Next generation, swapped with `cells` at the end of a step.
    next: Vec<u8>,
    /// Neighbour counts, filled by whichever counter the rule's neighbourhood asks for.
    counts: Vec<u16>,
    /// Each row summed into threes, for the eight-neighbour counter.
    rows: Vec<u8>,
    /// A row of nothing, which is what a walled edge looks up into.
    zeros: Vec<u8>,
    /// The field padded by the radius, for the wide counter.
    pad: Vec<u8>,
    /// A summed-area table over `pad`.
    sums: Vec<u32>,
    /// Fractional generations owed at the current pace.
    pace_owed: f32,
    /// Set when the picture needs painting again.
    dirty: bool,
}

impl Default for Board {
    fn default() -> Self {
        let cell = CELL_SIZES[OPENING_SIZE];
        let (width, height) = grid_for(DEFAULT_WINDOW, cell);
        Self::new(width, height)
    }
}

/// The field a window of this size gets at this cell size.
pub fn grid_for(window: Vec2, cell: u32) -> (u32, u32) {
    let cell = cell.max(1) as f32;
    let mut width = (window.x.max(1.0) / cell).ceil() as u32;
    let mut height = (window.y.max(1.0) / cell).ceil() as u32;
    width = width.max(GRID_MIN);
    height = height.max(GRID_MIN);
    // The backstop. Shrinking both sides by the same factor keeps the cells square, which is
    // the one thing that must not give.
    let cells = width as usize * height as usize;
    if cells > CELL_CAP {
        let shrink = (CELL_CAP as f64 / cells as f64).sqrt();
        width = ((width as f64 * shrink) as u32).max(GRID_MIN);
        height = ((height as f64 * shrink) as u32).max(GRID_MIN);
    }
    (width, height)
}

/// Encode a window size for [`RESIZE_COMMAND`]: whole pixels, so it round-trips exactly.
pub fn window_payload(size: Vec2) -> String {
    format!("{} {}", size.x as i32, size.y as i32)
}

/// Decode a [`window_payload`]. `None` for anything malformed or degenerate.
pub fn parse_window(payload: &str) -> Option<Vec2> {
    let (width, height) = payload.split_once(' ')?;
    let size = vec2(
        width.trim().parse::<i32>().ok()? as f32,
        height.trim().parse::<i32>().ok()? as f32,
    );
    (size.x >= 1.0 && size.y >= 1.0).then_some(size)
}

impl Board {
    /// An empty field of this size.
    pub fn new(width: u32, height: u32) -> Self {
        let width = width.max(GRID_MIN);
        let height = height.max(GRID_MIN);
        let cells = width as usize * height as usize;
        let stride = width.div_ceil(STRIDE_ALIGN) * STRIDE_ALIGN;
        Self {
            width,
            height,
            cells: vec![0; cells],
            age: vec![0; cells],
            trail: vec![0; cells],
            pixels: vec![0; stride as usize * height as usize * 2],
            stride,
            generation: 0,
            population: 0,
            births: 0,
            deaths: 0,
            revision: 0,
            period: None,
            settled: 0,
            // Distinct rather than zero, so an empty history cannot look like a match.
            marks: std::array::from_fn(|index| u64::MAX - index as u64),
            next: vec![0; cells],
            counts: vec![0; cells],
            rows: vec![0; cells],
            zeros: vec![0; width as usize],
            pad: Vec::new(),
            sums: Vec::new(),
            pace_owed: 0.0,
            dirty: true,
        }
    }

    /// Cells in the whole field.
    pub fn area(&self) -> usize {
        self.width as usize * self.height as usize
    }

    /// The state of one cell, or zero for anywhere off the field.
    pub fn at(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return 0;
        }
        self.cells[y as usize * self.width as usize + x as usize]
    }

    /// Put a cell in a state, ignoring anywhere off the field.
    pub fn set(&mut self, x: i32, y: i32, state: u8) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        let index = y as usize * self.width as usize + x as usize;
        self.cells[index] = state;
        self.age[index] = 0;
        if state == 1 {
            self.trail[index] = 255;
        }
        self.dirty = true;
    }

    /// Take the field to a new size, keeping what is on it.
    ///
    /// The old field is copied in with its middle on the new field's middle and **not** scaled.
    /// That is the only honest answer: a glider blown up by a factor of six is a six-by-six
    /// blob and no longer a glider, so a resample would quietly destroy the thing you were
    /// watching in order to keep it the same size on the glass. Going finer therefore leaves
    /// the pattern where it was, at its own size, with more field around it. Sow it again to
    /// fill the new room.
    pub fn reshape(&mut self, width: u32, height: u32) {
        let width = width.max(GRID_MIN);
        let height = height.max(GRID_MIN);
        if width == self.width && height == self.height {
            return;
        }
        let mut grown = Board::new(width, height);
        let shift_x = (width as i64 - self.width as i64) / 2;
        let shift_y = (height as i64 - self.height as i64) / 2;
        for y in 0..self.height as i64 {
            let landed_y = y + shift_y;
            if landed_y < 0 || landed_y >= height as i64 {
                continue;
            }
            for x in 0..self.width as i64 {
                let landed_x = x + shift_x;
                if landed_x < 0 || landed_x >= width as i64 {
                    continue;
                }
                let from = y as usize * self.width as usize + x as usize;
                let to = landed_y as usize * width as usize + landed_x as usize;
                grown.cells[to] = self.cells[from];
                grown.age[to] = self.age[from];
                grown.trail[to] = self.trail[from];
            }
        }
        grown.generation = self.generation;
        grown.pace_owed = self.pace_owed;
        grown.revision = self.revision + 1;
        // What the field remembers of its own past does not carry over: the checksums were
        // taken over a differently shaped field and cannot be compared with the ones to come.
        // The population does carry over, counted again, because it is a fact about now and a
        // resize is not supposed to look like everything dying.
        grown.recount();
        *self = grown;
    }

    /// Bring every cell into range for a rule with this many states.
    ///
    /// Called when the rule changes under a field that is already running. A cell part-way
    /// through Fireworks's twenty-one states means nothing to Life, so it is simply emptied;
    /// everything alive stays alive, and the field carries on under the new law.
    pub fn clamp_states(&mut self, states: u32) {
        for cell in &mut self.cells {
            if u32::from(*cell) >= states {
                *cell = 0;
            }
        }
        self.forget();
        self.dirty = true;
    }

    /// Throw away what the field remembers of its own recent past, so that a change of rule or
    /// of size cannot be mistaken for a field that has settled.
    fn forget(&mut self) {
        self.marks = std::array::from_fn(|index| u64::MAX - index as u64);
        self.period = None;
        self.settled = 0;
    }

    // -----------------------------------------------------------------------------------
    // sowing
    // -----------------------------------------------------------------------------------

    /// Empty the field.
    pub fn clear(&mut self) {
        self.cells.fill(0);
        self.age.fill(0);
        self.trail.fill(0);
        self.generation = 0;
        self.population = 0;
        self.births = 0;
        self.deaths = 0;
        self.pace_owed = 0.0;
        self.forget();
        self.dirty = true;
    }

    /// Sow the field the way `start` asks, under `rule`.
    pub fn sow(&mut self, rule: &Rule, start: Start, rng: &mut SimRng) {
        self.clear();
        let native = rule.seed;
        let density = match native {
            Seeding::Soup(density) | Seeding::Patch(density, _) => density,
            Seeding::Spark | Seeding::Block(_) => 0.32,
        };
        match start {
            Start::Native => self.scatter(native, rng),
            Start::Soup => self.scatter(Seeding::Soup(density), rng),
            Start::Patch => self.scatter(Seeding::Patch(density, 0.25), rng),
            Start::Symmetry => self.mirror_soup(density, rng),
            Start::Spark => self.scatter(Seeding::Spark, rng),
            Start::Pentomino => self.stamp(PENTOMINO, Placing::Middle),
            Start::Acorn => self.stamp(ACORN, Placing::Middle),
            Start::Diehard => self.stamp(DIEHARD, Placing::Middle),
            // The gun fires down and to the right, so it is put in the top-left corner with the
            // whole field ahead of it rather than in the middle with half of one.
            Start::Gun => self.stamp(GUN, Placing::Corner),
            Start::Empty => {}
        }
        self.recount();
        self.dirty = true;
    }

    /// One of the rule's own seedings, laid down.
    fn scatter(&mut self, seeding: Seeding, rng: &mut SimRng) {
        let (width, height) = (self.width as i32, self.height as i32);
        match seeding {
            Seeding::Soup(density) => {
                for cell in &mut self.cells {
                    *cell = u8::from(rng.chance(density));
                }
            }
            Seeding::Patch(density, fraction) => {
                let side = ((width.min(height) as f32 * fraction).round() as i32).max(1);
                let left = (width - side) / 2;
                let top = (height - side) / 2;
                for y in top..top + side {
                    for x in left..left + side {
                        if rng.chance(density) {
                            self.set(x, y, 1);
                        }
                    }
                }
            }
            Seeding::Spark => self.set(width / 2, height / 2, 1),
            Seeding::Block(side) => {
                let side = side as i32;
                let left = (width - side) / 2;
                let top = (height - side) / 2;
                for y in top..top + side {
                    for x in left..left + side {
                        self.set(x, y, 1);
                    }
                }
            }
        }
        // `scatter` writes `cells` directly in the soup case, which skips the trail `set`
        // keeps. Nothing has happened yet, so there is nothing to trail.
        self.trail.fill(0);
    }

    /// A soup in one quadrant, mirrored into the other three.
    ///
    /// Every rule here treats the four reflections alike, so a field that starts symmetric
    /// stays symmetric for as long as it runs. It is the cheapest way to make any of these
    /// rules beautiful, and it costs one line of the seeding.
    fn mirror_soup(&mut self, density: f32, rng: &mut SimRng) {
        let (width, height) = (self.width as usize, self.height as usize);
        for y in 0..height.div_ceil(2) {
            for x in 0..width.div_ceil(2) {
                let live = u8::from(rng.chance(density));
                for (px, py) in [
                    (x, y),
                    (width - 1 - x, y),
                    (x, height - 1 - y),
                    (width - 1 - x, height - 1 - y),
                ] {
                    self.cells[py * width + px] = live;
                }
            }
        }
    }

    /// Stamp a named pattern onto the field.
    fn stamp(&mut self, pattern: &[(i32, i32)], placing: Placing) {
        let span_x = pattern.iter().map(|(x, _)| *x).max().unwrap_or(0) + 1;
        let span_y = pattern.iter().map(|(_, y)| *y).max().unwrap_or(0) + 1;
        let (left, top) = match placing {
            Placing::Middle => (
                (self.width as i32 - span_x) / 2,
                (self.height as i32 - span_y) / 2,
            ),
            Placing::Corner => (self.width as i32 / 8, self.height as i32 / 8),
        };
        for (x, y) in pattern {
            self.set(left + x, top + y, 1);
        }
    }

    /// Count the live cells, for after something has been put on the field by hand.
    fn recount(&mut self) {
        self.population = self.cells.iter().filter(|cell| **cell == 1).count() as u32;
    }

    // -----------------------------------------------------------------------------------
    // drawing on it
    // -----------------------------------------------------------------------------------

    /// Fill or empty a disc of cells, for the mouse.
    pub fn brush(&mut self, at: (i32, i32), radius: i32, alive: bool) {
        self.dab(at, radius, alive);
        self.recount();
    }

    /// The same, along the line between two points, so that a fast stroke is a stroke rather
    /// than a row of dots.
    pub fn stroke(&mut self, from: (i32, i32), to: (i32, i32), radius: i32, alive: bool) {
        let steps = (to.0 - from.0).abs().max((to.1 - from.1).abs()).max(1);
        for step in 0..=steps {
            let x = from.0 + (to.0 - from.0) * step / steps;
            let y = from.1 + (to.1 - from.1) * step / steps;
            self.dab((x, y), radius, alive);
        }
        // Once, at the end. Counting the whole field after every dab of a stroke that crossed a
        // large window would be a hundred passes over several million cells in one tick.
        self.recount();
    }

    /// One dab of the brush, without counting the field afterwards.
    fn dab(&mut self, at: (i32, i32), radius: i32, alive: bool) {
        let state = u8::from(alive);
        for offset_y in -radius..=radius {
            for offset_x in -radius..=radius {
                if offset_x * offset_x + offset_y * offset_y > radius * radius {
                    continue;
                }
                self.set(at.0 + offset_x, at.1 + offset_y, state);
            }
        }
    }

    // -----------------------------------------------------------------------------------
    // one generation
    // -----------------------------------------------------------------------------------

    /// Take the field forward one generation under `rule`.
    ///
    /// `wrap` is the boundary: true makes the field a torus, where the left edge is the right
    /// edge's neighbour, and false puts a wall of permanent emptiness around it. It is not a
    /// detail — a glider that leaves a walled field is gone, and on a torus it comes back.
    pub fn step(&mut self, rule: &Rule, wrap: bool) {
        match rule.shape {
            Shape::Moore(1) => self.count_eight(wrap),
            Shape::Moore(radius) => self.count_wide(radius, wrap),
            Shape::VonNeumann => self.count_four(wrap),
        }
        self.apply(rule);
        self.generation += 1;
        self.dirty = true;
    }

    /// The eight-neighbour count, in two sequential passes.
    ///
    /// First every row is summed into threes: `rows[x]` is how many of the cells at `x - 1`,
    /// `x` and `x + 1` on that row are alive. Then a cell's neighbourhood is the row sum above
    /// it, its own, and the one below, added together. Nine reads a cell becomes five
    /// additions, and both passes walk memory forwards.
    ///
    /// The three-by-three that comes out includes the middle cell, which is not its own
    /// neighbour. Every counter here leaves it in and [`Board::apply`] takes it back out, so
    /// that Larger than Life's rules — which do want it counted — need no separate path.
    fn count_eight(&mut self, wrap: bool) {
        let (width, height) = (self.width as usize, self.height as usize);
        {
            let cells = &self.cells[..];
            let rows = &mut self.rows[..];
            for y in 0..height {
                let row = &cells[y * width..y * width + width];
                let out = &mut rows[y * width..y * width + width];
                // The inside of the row first, with no edge in it. This is the loop the whole
                // step is paid for, and it is written with nothing in it to branch on so that
                // it vectorises: asking "am I at the edge?" of every cell of a field of
                // millions costs more than the addition it is guarding.
                for x in 1..width - 1 {
                    out[x] = u8::from(row[x - 1] == 1)
                        + u8::from(row[x] == 1)
                        + u8::from(row[x + 1] == 1);
                }
                // Then the two ends, where the boundary decides what is off the side.
                let first = u8::from(row[0] == 1);
                let last = u8::from(row[width - 1] == 1);
                out[0] = if wrap { last } else { 0 } + first + u8::from(row[1] == 1);
                out[width - 1] =
                    u8::from(row[width - 2] == 1) + last + if wrap { first } else { 0 };
            }
        }

        let rows = &self.rows[..];
        let zeros = &self.zeros[..];
        let counts = &mut self.counts[..];
        for y in 0..height {
            let above = match (y > 0, wrap) {
                (true, _) => &rows[(y - 1) * width..(y - 1) * width + width],
                (false, true) => &rows[(height - 1) * width..(height - 1) * width + width],
                (false, false) => zeros,
            };
            let below = match (y + 1 < height, wrap) {
                (true, _) => &rows[(y + 1) * width..(y + 1) * width + width],
                (false, true) => &rows[0..width],
                (false, false) => zeros,
            };
            let here = &rows[y * width..y * width + width];
            let out = &mut counts[y * width..y * width + width];
            for x in 0..width {
                out[x] = u16::from(above[x]) + u16::from(here[x]) + u16::from(below[x]);
            }
        }
    }

    /// The four-neighbour count, read directly. Small enough not to be worth a trick.
    fn count_four(&mut self, wrap: bool) {
        let (width, height) = (self.width as i32, self.height as i32);
        let cells = &self.cells[..];
        let counts = &mut self.counts[..];
        let live = |x: i32, y: i32| -> u16 {
            let (x, y) = if wrap {
                (x.rem_euclid(width), y.rem_euclid(height))
            } else if x < 0 || y < 0 || x >= width || y >= height {
                return 0;
            } else {
                (x, y)
            };
            u16::from(cells[y as usize * width as usize + x as usize] == 1)
        };
        for y in 0..height {
            for x in 0..width {
                // The middle cell is in the count, as it is in the other two counters, and
                // `apply` is the one place that takes it out again.
                counts[y as usize * width as usize + x as usize] =
                    live(x, y) + live(x - 1, y) + live(x + 1, y) + live(x, y - 1) + live(x, y + 1);
            }
        }
    }

    /// The wide count, through a summed-area table.
    ///
    /// The field is first copied into one padded by the radius on every side, filled by
    /// wrapping or with emptiness according to the boundary. Every neighbourhood a cell asks
    /// for then lies wholly inside that copy, which is what lets one table serve both
    /// boundaries with no special cases at the edges. A running two-dimensional sum over it
    /// turns any rectangle, however large, into four lookups and three additions — so radius
    /// ten costs the same per cell as radius two.
    ///
    /// The middle cell is inside the rectangle. Rules that do not count it have it taken off
    /// again in [`Board::apply`], which is where a cell's own state is to hand.
    fn count_wide(&mut self, radius: u32, wrap: bool) {
        let (width, height) = (self.width as usize, self.height as usize);
        let radius = radius as usize;
        let (pad_w, pad_h) = (width + 2 * radius, height + 2 * radius);

        self.pad.resize(pad_w * pad_h, 0);
        {
            let cells = &self.cells[..];
            let pad = &mut self.pad[..];
            for pad_y in 0..pad_h {
                let source = pad_y as isize - radius as isize;
                let out = &mut pad[pad_y * pad_w..pad_y * pad_w + pad_w];
                let source = if wrap {
                    source.rem_euclid(height as isize) as usize
                } else if source < 0 || source as usize >= height {
                    out.fill(0);
                    continue;
                } else {
                    source as usize
                };
                let row = &cells[source * width..source * width + width];
                for (pad_x, slot) in out.iter_mut().enumerate() {
                    let from = pad_x as isize - radius as isize;
                    *slot = if wrap {
                        u8::from(row[from.rem_euclid(width as isize) as usize] == 1)
                    } else if from < 0 || from as usize >= width {
                        0
                    } else {
                        u8::from(row[from as usize] == 1)
                    };
                }
            }
        }

        // The table is one wider and one taller than the padded field, with a row and a column
        // of zeros along the top and the left, so that every rectangle is one subtraction.
        let table_w = pad_w + 1;
        self.sums.resize(table_w * (pad_h + 1), 0);
        self.sums[0..table_w].fill(0);
        for pad_y in 0..pad_h {
            let mut run = 0u32;
            self.sums[(pad_y + 1) * table_w] = 0;
            for pad_x in 0..pad_w {
                run += u32::from(self.pad[pad_y * pad_w + pad_x]);
                self.sums[(pad_y + 1) * table_w + pad_x + 1] =
                    self.sums[pad_y * table_w + pad_x + 1] + run;
            }
        }

        let sums = &self.sums[..];
        let counts = &mut self.counts[..];
        let span = 2 * radius + 1;
        for y in 0..height {
            let top = y * table_w;
            let bottom = (y + span) * table_w;
            let out = &mut counts[y * width..y * width + width];
            for (x, slot) in out.iter_mut().enumerate() {
                // The two corners that are added, then the two that are taken off, and in that
                // order. Written as `a - b - c + d` it comes to the same number, but `a - b - c`
                // on its own is very often negative — the whole width of the field to the left
                // of this box is in `c` — and these are unsigned. Release arithmetic wraps
                // round and lands on the right answer anyway; debug arithmetic stops.
                let corners = sums[bottom + x + span] + sums[top + x];
                let sides = sums[top + x + span] + sums[bottom + x];
                *slot = (corners - sides) as u16;
            }
        }
    }

    /// Apply the rule to every cell, and keep everything that is kept per cell while doing it.
    ///
    /// One pass, whichever counter filled `counts`, and it is the only place the rule is
    /// actually consulted. It also folds the new generation into a checksum, which is how the
    /// field notices it has stopped going anywhere.
    fn apply(&mut self, rule: &Rule) {
        let states = rule.states;
        let generations = states > 2;
        let uncounted = !rule.centre;
        let mut population = 0u32;
        let mut births = 0u32;
        let mut deaths = 0u32;
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;

        // The rule, answered once for every count it could ever be asked about, before the loop
        // that asks it millions of times. What is being got rid of is not the arithmetic — a
        // bit test is nothing — but the branch on which *kind* of rule this is, which is the
        // same answer every time and is in the way of everything the compiler would otherwise
        // do with this loop. `VERDICTS` is comfortably above the largest neighbourhood here,
        // which is radius ten's four hundred and forty-one.
        const VERDICTS: usize = 512;
        debug_assert!((rule.ceiling() as usize) < VERDICTS);
        let mut born = [false; VERDICTS];
        let mut lives = [false; VERDICTS];
        for count in 0..VERDICTS {
            born[count] = rule.birth.holds(count as u32);
            lives[count] = rule.survive.holds(count as u32);
        }

        let cells = &self.cells[..];
        let counts = &self.counts[..];
        let age = &mut self.age[..];
        let trail = &mut self.trail[..];
        let next = &mut self.next[..];

        for index in 0..cells.len() {
            let state = cells[index];
            let alive = state == 1;
            // A neighbourhood that includes the middle cell was counted with it in; a rule
            // that does not want it has it taken off here, where the state is already loaded.
            let count = (usize::from(counts[index]) - usize::from(uncounted && alive)) % VERDICTS;
            let outcome = match state {
                0 => u8::from(born[count]),
                1 => {
                    if lives[count] {
                        1
                    } else if generations {
                        2
                    } else {
                        0
                    }
                }
                dying => {
                    let older = u32::from(dying) + 1;
                    if older >= states { 0 } else { older as u8 }
                }
            };

            if outcome == 1 {
                population += 1;
                if alive {
                    age[index] = age[index].saturating_add(1);
                } else {
                    births += 1;
                    age[index] = 0;
                }
                trail[index] = 255;
            } else {
                if alive {
                    deaths += 1;
                }
                age[index] = 0;
                trail[index] = trail[index].saturating_sub(TRAIL_DECAY);
            }
            next[index] = outcome;
            hash = (hash ^ u64::from(outcome)).wrapping_mul(0x0000_0100_0000_01b3);
        }

        std::mem::swap(&mut self.cells, &mut self.next);
        self.population = population;
        self.births = births;
        self.deaths = deaths;
        self.remember(hash);
    }

    /// File this generation's checksum, and see whether it has been here before.
    ///
    /// A field that repeats itself has stopped producing anything new, and the distance back to
    /// the last generation identical to this one is its period: one for a still field, two for
    /// a garden of blinkers, fifteen for a pulsar. A glider crossing a torus is not caught by
    /// this until it has come all the way round, which is correct — it really has not settled.
    fn remember(&mut self, hash: u64) {
        self.marks.rotate_right(1);
        self.marks[0] = hash;
        // Only once there is a full history to look back through, so that the fill values
        // cannot be part of an answer.
        self.period = if self.generation >= MEMORY as u64 {
            (1..MEMORY)
                .find(|distance| self.marks[*distance] == hash)
                .map(|distance| distance as u32)
        } else {
            None
        };
        self.settled = if self.period.is_some() {
            self.settled.saturating_add(1)
        } else {
            0
        };
    }

    /// Has the field been repeating itself long enough to be worth starting again?
    ///
    /// Nothing counts until at least one generation has run, so that an empty field waiting to
    /// be drawn on is not mistaken for one that has died.
    pub fn exhausted(&self) -> bool {
        self.generation > 0 && (self.population == 0 || self.settled >= PATIENCE)
    }

    // -----------------------------------------------------------------------------------
    // what it looks like
    // -----------------------------------------------------------------------------------

    /// Fold the field into the two bytes a cell is drawn from, if anything has changed.
    ///
    /// The first byte says what the cell is: 255 alive, 0 empty, and everything between is a
    /// Generations cell part-way through dying, counting down as it goes. The second says how
    /// long — the age of a live cell, and the trail left by a dead one. Two bytes rather than
    /// four because at one cell to the pixel this buffer goes over the bus every generation,
    /// and half of sixteen megabytes is eight.
    pub fn repaint(&mut self, rule: &Rule) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        let (width, height) = (self.width as usize, self.height as usize);
        let stride = self.stride as usize;
        // How many stages of dying a rule has: states 2 upwards, so `states - 2` of them, and
        // at least one so that the division below is always safe. Brian's Brain has exactly
        // one, and that one should read as fully dying rather than as nearly gone.
        let stages = rule.states.saturating_sub(2).max(1);

        let cells = &self.cells[..];
        let age = &self.age[..];
        let trail = &self.trail[..];
        let pixels = &mut self.pixels[..];
        for y in 0..height {
            let row = &cells[y * width..y * width + width];
            let out = &mut pixels[y * stride * 2..y * stride * 2 + width * 2];
            for x in 0..width {
                let index = y * width + x;
                let (life, mark) = match row[x] {
                    0 => (0u8, trail[index]),
                    1 => (255u8, age_mark(age[index])),
                    dying => {
                        // How much life is left, counted from the far end: the whole of it for
                        // the stage just after alive, one stage's worth for the last one
                        // before empty. Never zero, which is what tells it from an empty cell.
                        let left = rule.states.saturating_sub(u32::from(dying)).min(stages);
                        ((left * 254 / stages) as u8, trail[index])
                    }
                };
                out[x * 2] = life;
                out[x * 2 + 1] = mark;
            }
        }
        self.revision += 1;
    }
}

/// Where a stamped pattern goes.
enum Placing {
    /// In the middle of the field.
    Middle,
    /// Up in the top-left, for anything that travels.
    Corner,
}

/// How old a live cell reads as, on a curve rather than a straight line.
///
/// Age matters most in its first few generations and hardly at all after that: the difference
/// between a cell that has held for two generations and one that has held for ten is worth
/// seeing, and the difference between two hundred and a thousand is not. A straight ramp over
/// a byte would put the entire visible range in the last of a still life's life and paint every
/// busy part of the field the same colour.
fn age_mark(age: u8) -> u8 {
    let age = u32::from(age);
    (255 * age / (age + AGE_SOFTNESS)) as u8
}

// ---------------------------------------------------------------------------------------
// the knobs
// ---------------------------------------------------------------------------------------

/// A key that repeats while it is held, the way a text cursor does.
///
/// There are forty-four rules and sixteen resolutions, and walking either of them one press at
/// a time is a chore. Held for a third of a second, the key starts moving on its own.
#[derive(Default, Clone, Copy)]
pub struct Repeat {
    held: f32,
    fired: u32,
}

impl Repeat {
    /// Before it starts repeating.
    const DELAY: f32 = 0.35;
    /// And how fast it goes once it has.
    const PERIOD: f32 = 1.0 / 14.0;

    /// Does the key act on this tick?
    pub fn fires(&mut self, down: bool, delta: f32) -> bool {
        if !down {
            self.held = 0.0;
            self.fired = 0;
            return false;
        }
        self.held += delta;
        let due = if self.held < Self::DELAY {
            1
        } else {
            2 + ((self.held - Self::DELAY) / Self::PERIOD) as u32
        };
        if self.fired < due {
            self.fired += 1;
            true
        } else {
            false
        }
    }
}

/// The repeating keys, one state each.
#[derive(Resource, Default)]
pub struct Held {
    /// Previous rule, next rule.
    rule: [Repeat; 2],
    /// Coarser cells, finer cells.
    size: [Repeat; 2],
    /// One generation at a time, while the field is held.
    single: Repeat,
}

/// Everything the simulation is set to.
#[derive(Resource)]
pub struct Dials {
    /// Which of [`RULES`] is running.
    pub rule: usize,
    /// Which of [`CELL_SIZES`] a cell is drawn at.
    pub size: usize,
    /// Generations a second asked for.
    pub pace: f32,
    /// Whether it is running at all.
    pub running: bool,
    /// Whether the field is a torus. False puts a wall round it.
    pub wrap: bool,
    /// Which of [`STARTS`] sowing uses.
    pub start: Start,
    /// Whether a field that has stopped going anywhere is sown again by itself.
    pub restart: bool,
    /// The window, in physical pixels, as it last arrived on the command channel.
    pub window: Vec2,
    /// Set when something has asked for a fresh field.
    pub reseed: bool,
    /// Set when the rule has changed and the field has not been brought into line yet.
    pub retuned: bool,
    /// Where the pointer was last, in cells, while a button was down.
    stroke_from: Option<(i32, i32)>,
}

impl Default for Dials {
    fn default() -> Self {
        Self {
            rule: crate::rules::OPENING,
            size: OPENING_SIZE,
            pace: PACE_START,
            running: true,
            wrap: true,
            start: Start::Native,
            restart: true,
            window: DEFAULT_WINDOW,
            reseed: true,
            retuned: false,
            stroke_from: None,
        }
    }
}

impl Dials {
    /// The rule in force.
    pub fn rule(&self) -> &'static Rule {
        &RULES[self.rule % RULES.len()]
    }

    /// How many physical pixels a cell is drawn at.
    pub fn cell(&self) -> u32 {
        CELL_SIZES[self.size.min(CELL_SIZES.len() - 1)]
    }
}

/// How wide the mouse draws, in physical pixels. Turned into cells against the resolution, so
/// the brush is the same size on the glass whether a cell is one pixel or sixty-four.
const BRUSH_PIXELS: f32 = 10.0;

// ---------------------------------------------------------------------------------------
// the plugin
// ---------------------------------------------------------------------------------------

/// Installs the field and everything that drives it.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut().insert_resource(Board::default());
        app.world_mut().insert_resource(Dials::default());
        app.world_mut().insert_resource(Held::default());
        app.add_systems(
            FixedUpdate,
            (
                apply_resize,
                steer,
                retune,
                fit_field,
                sow_field,
                draw_on_it,
                advance,
                start_again,
                repaint,
            )
                .chain(),
        );
    }
}

/// Take the window size off the command channel.
fn apply_resize(mut orders: EventReader<CommandEvent>, mut dials: ResMut<Dials>) {
    for order in orders.read() {
        if order.name != RESIZE_COMMAND {
            continue;
        }
        if let Some(window) = parse_window(&order.payload) {
            dials.window = window;
        }
    }
}

/// Every key that is not purely a matter of taste. The ones that are live in `main.rs`.
fn steer(
    input: Res<Input>,
    time: Res<Time>,
    mut dials: ResMut<Dials>,
    mut held: ResMut<Held>,
    mut board: ResMut<Board>,
) {
    let delta = time.fixed_delta;

    // The rules, which repeat when the key is held down.
    let rules = RULES.len();
    if held.rule[0].fires(input.pressed(Key::N), delta) {
        dials.rule = (dials.rule + rules - 1) % rules;
        dials.retuned = true;
    }
    if held.rule[1].fires(input.pressed(Key::M), delta) {
        dials.rule = (dials.rule + 1) % rules;
        dials.retuned = true;
    }
    if input.just_pressed(Key::Tab) {
        dials.rule = crate::rules::next_family(dials.rule);
        dials.retuned = true;
    }

    // The resolution, likewise. Z goes coarser, X goes finer, and X bottoms out at one cell to
    // the physical pixel.
    if held.size[0].fires(input.pressed(Key::Z), delta) {
        dials.size = (dials.size + 1).min(CELL_SIZES.len() - 1);
    }
    if held.size[1].fires(input.pressed(Key::X), delta) {
        dials.size = dials.size.saturating_sub(1);
    }
    let wheel = input.scroll_delta();
    if wheel > 0.5 {
        dials.size = dials.size.saturating_sub(1);
    } else if wheel < -0.5 {
        dials.size = (dials.size + 1).min(CELL_SIZES.len() - 1);
    }

    // The pace, which ramps rather than stepping: the useful range is three orders of
    // magnitude wide and a linear key would spend all of its travel at one end.
    let ramp = PACE_RAMP.powf(delta);
    if input.pressed(Key::Up) {
        dials.pace = (dials.pace * ramp).min(PACE_MAX);
    }
    if input.pressed(Key::Down) {
        dials.pace = (dials.pace / ramp).max(PACE_MIN);
    }

    if input.just_pressed(Key::Space) {
        dials.running = !dials.running;
    }
    // One generation at a time, which is the only way to actually read what a rule does.
    if held.single.fires(input.pressed(Key::S), delta) && !dials.running {
        board.step(dials.rule(), dials.wrap);
    }

    if input.just_pressed(Key::T) {
        dials.wrap = !dials.wrap;
    }
    if input.just_pressed(Key::K) {
        dials.restart = !dials.restart;
    }

    // How the field is sown, and sowing it.
    for (slot, key) in [
        Key::Digit1,
        Key::Digit2,
        Key::Digit3,
        Key::Digit4,
        Key::Digit5,
        Key::Digit6,
        Key::Digit7,
        Key::Digit8,
        Key::Digit9,
        Key::Digit0,
    ]
    .into_iter()
    .enumerate()
    {
        if input.just_pressed(key) && slot < STARTS.len() {
            dials.start = STARTS[slot];
            dials.reseed = true;
        }
    }
    if input.just_pressed(Key::R) {
        dials.reseed = true;
    }
    if input.just_pressed(Key::C) {
        board.clear();
    }
}

/// Bring the field into line with a rule that has just changed under it.
fn retune(mut dials: ResMut<Dials>, mut board: ResMut<Board>) {
    if !dials.retuned {
        return;
    }
    dials.retuned = false;
    let states = dials.rule().states;
    board.clamp_states(states);
}

/// Keep the field the size the window and the resolution say it should be.
fn fit_field(dials: Res<Dials>, mut board: ResMut<Board>) {
    let (width, height) = grid_for(dials.window, dials.cell());
    board.reshape(width, height);
}

/// Sow the field when something has asked for it.
fn sow_field(mut dials: ResMut<Dials>, mut board: ResMut<Board>, mut rng: ResMut<SimRng>) {
    if !dials.reseed {
        return;
    }
    dials.reseed = false;
    let rule = dials.rule();
    board.sow(rule, dials.start, &mut rng);
}

/// The mouse: left draws live cells, right takes them away.
fn draw_on_it(input: Res<Input>, mut dials: ResMut<Dials>, mut board: ResMut<Board>) {
    let left = input.mouse_pressed(MouseButton::Left);
    let right = input.mouse_pressed(MouseButton::Right);
    if !left && !right {
        dials.stroke_from = None;
        return;
    }
    let cell = dials.cell() as f32;
    let at = input.mouse_screen();
    let to = ((at.x / cell) as i32, (at.y / cell) as i32);
    let radius = (BRUSH_PIXELS / cell).round().max(1.0) as i32;
    match dials.stroke_from {
        Some(from) => board.stroke(from, to, radius, left),
        None => board.brush(to, radius, left),
    }
    dials.stroke_from = Some(to);
}

/// Run as many generations as the pace has earned, up to what one tick will do.
fn advance(time: Res<Time>, dials: Res<Dials>, mut board: ResMut<Board>) {
    if !dials.running {
        return;
    }
    let rule = dials.rule();
    let allowed =
        (CELLS_PER_TICK / board.area().max(1)).clamp(1, MAX_STEPS_PER_TICK as usize) as u32;
    board.pace_owed += dials.pace * time.fixed_delta;
    let mut taken = 0;
    while board.pace_owed >= 1.0 && taken < allowed {
        board.step(rule, dials.wrap);
        board.pace_owed -= 1.0;
        taken += 1;
    }
    // Whatever the tick could not afford is dropped rather than banked, so a pace the machine
    // cannot keep up with makes the field run slow instead of running away with it later.
    board.pace_owed = board.pace_owed.min(1.0);
}

/// Sow the field again once it has stopped going anywhere, if it has been asked to.
///
/// Never on an empty field: choosing the empty start is how you ask for a blank sheet to draw
/// on, and having it filled in under you would be the opposite of what was wanted.
fn start_again(mut dials: ResMut<Dials>, board: Res<Board>) {
    if dials.restart && dials.running && dials.start != Start::Empty && board.exhausted() {
        dials.reseed = true;
    }
}

/// Fold the field into the bytes the renderer uploads.
fn repaint(dials: Res<Dials>, mut board: ResMut<Board>) {
    let rule = dials.rule();
    board.repaint(rule);
}
