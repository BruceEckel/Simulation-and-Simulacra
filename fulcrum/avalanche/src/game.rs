//! Avalanche: a table of sand with one rule, which turns out to be enough.
//!
//! **The rule.** Every cell holds a whole number of grains. Any cell holding four or more topples:
//! it gives one grain to each of its four neighbours and keeps the rest. Grains that topple off
//! the edge of the table are gone. That is the entire simulation; everything else in this file is
//! book-keeping.
//!
//! **What the rule does.** Drop grains on a flat table and the pile organises itself. Not to a
//! neat shape: to a state where a single grain can cause an avalanche of any size at all, from
//! nothing to a third of the table, and where there is no way to tell in advance which you will
//! get. The average height climbs (or falls) to about 2.1 grains a cell and stays there, from
//! either direction, without anybody choosing that number. This is self-organised criticality,
//! and the sandpile is the example Bak, Tang and Wiesenfeld used to name it in 1987.
//!
//! **Why the avalanche sizes are the point.** [`Sizes`] keeps a histogram of them. On log axes it
//! comes out a straight line, which is what it looks like when a distribution has no typical
//! size. Earthquakes do this. Forest fires do this. Nothing in the rule mentions it.
//!
//! **Every number here is an integer.** No floating point anywhere in the simulation: grains are
//! counts, toppling is subtraction, and the histogram bins are computed with [`u32::ilog2`]. This
//! piece therefore replays bit-identically on any machine that can add, which is a stronger
//! promise than the rest of the games in this repository can make. The curve fitting lives in the
//! binary, where being approximate is somebody else's problem.
//!
//! A wave at a time: all the cells that are unstable topple together ([`Table::wave`]), then the
//! ones that just became unstable topple together, and so on. That is what makes an avalanche
//! something you can watch spread rather than something that has already happened.

use fulcrum::prelude::*;

/// Cells across the table.
pub const WIDE: usize = 160;
/// Cells up the table.
pub const TALL: usize = 120;
/// How many cells there are.
pub const CELLS: usize = WIDE * TALL;
/// How wide a cell is, in world units.
pub const CELL: f32 = 7.0;

/// The grains a cell can hold before it goes. Four, because a cell has four neighbours: the pile
/// is exactly as steep as it can be while still being able to share what it has.
pub const THRESHOLD: u16 = 4;

/// The table, and the strip above it the readouts live in, in world units.
pub const ARENA: Vec2 = Vec2::new(WIDE as f32 * CELL, TALL as f32 * CELL + 180.0);

/// How many cell-topples a tick may do before it gives up and finishes next tick.
///
/// A budget rather than "run it to the end": the biggest avalanches are hundreds of thousands of
/// topples, and doing one in a single tick would drop a frame and, worse, would hide the thing
/// worth seeing. Spread over ticks it spreads across the table in front of you.
pub const TOPPLE_BUDGET: u32 = 24_000;

/// Grains a poured stream lays down per tick.
pub const POUR_RATE: u16 = 2;
/// Radius of a handful, in cells.
pub const HANDFUL_REACH: i32 = 4;
/// Grains a handful drops on each cell inside that.
pub const HANDFUL_DEPTH: u16 = 3;

/// What the table is filled to by the fill key. One short of toppling, so the next grain anywhere
/// starts something, and everything after it for the next minute or two.
pub const FILL_DEPTH: u16 = 3;
/// What it starts at. Below the level it settles to, so the opening is a quiet table filling up
/// and finding its own state, and the loaded table is one key away rather than unavoidable.
pub const START_DEPTH: u16 = 2;

/// How fast the mark left by a toppling cell fades, per tick.
///
/// Fast. A slow fade paints the whole area an avalanche covered, which after the first big one is
/// most of the table and hides the pile underneath it. A fast one leaves only the front, so what
/// you see is the edge of the avalanche travelling, which is both prettier and more informative.
pub const GLOW_FADE: u8 = 34;

/// Histogram bins, two to the octave. Wide enough for an avalanche of two million topples, which
/// is more than this table can produce.
pub const BINS: usize = 42;

/// Slowest the table can run, as a multiple of real time.
pub const SPEED_MIN: f32 = 0.1;
/// Fastest it can run.
pub const SPEED_MAX: f32 = 8.0;
/// How much a held speed key multiplies the rate each tick.
pub const SPEED_RAMP: f32 = 1.03;

/// The table of sand.
///
/// One resource rather than an entity per cell: the grid is the whole state of the piece, it is
/// nineteen thousand small integers, and keeping it in one place is what makes a snapshot of the
/// simulation a thing you can compare with `==`.
#[derive(Resource, Clone, PartialEq, Eq, Debug)]
pub struct Table {
    /// Grains in each cell, row by row from the bottom.
    pub grains: Vec<u16>,
    /// How recently each cell toppled, `255` for just now, fading to nothing.
    pub glow: Vec<u8>,
    /// Cells that are unstable and will topple in the next wave.
    hot: Vec<u32>,
    /// The wave being processed, swapped out of `hot` so new arrivals can queue behind it.
    going: Vec<u32>,
    /// Whether a cell is already queued, so nothing is queued twice.
    queued: Vec<bool>,
}

impl Default for Table {
    fn default() -> Self {
        Self {
            grains: vec![0; CELLS],
            glow: vec![0; CELLS],
            hot: Vec::new(),
            going: Vec::new(),
            queued: vec![false; CELLS],
        }
    }
}

impl Table {
    /// The cell at `(column, row)`, or `None` off the table.
    pub fn index(column: i32, row: i32) -> Option<usize> {
        (column >= 0 && row >= 0 && (column as usize) < WIDE && (row as usize) < TALL)
            .then(|| row as usize * WIDE + column as usize)
    }

    /// What is in a cell.
    pub fn at(&self, column: i32, row: i32) -> u16 {
        Self::index(column, row).map_or(0, |index| self.grains[index])
    }

    /// Whether anything is still moving.
    pub fn busy(&self) -> bool {
        !self.hot.is_empty()
    }

    /// Cells waiting to topple.
    pub fn unstable(&self) -> usize {
        self.hot.len()
    }

    /// Grains on the table.
    pub fn total(&self) -> u64 {
        self.grains.iter().map(|&grains| grains as u64).sum()
    }

    /// Grains per cell, which is the number that finds its own level.
    pub fn mean(&self) -> f32 {
        self.total() as f32 / CELLS as f32
    }

    /// Put `count` grains in one cell, and queue it if that is too many for it.
    pub fn add(&mut self, index: usize, count: u16) {
        self.grains[index] = self.grains[index].saturating_add(count);
        if self.grains[index] >= THRESHOLD && !self.queued[index] {
            self.queued[index] = true;
            self.hot.push(index as u32);
        }
    }

    /// Empty the table.
    pub fn sweep(&mut self) {
        self.grains.fill(0);
        self.glow.fill(0);
        self.hot.clear();
        self.going.clear();
        self.queued.fill(false);
    }

    /// Fill every cell to `depth`. Queues nothing while `depth` is under the threshold, which is
    /// the point of filling to three: the table is loaded and perfectly still.
    pub fn fill(&mut self, depth: u16) {
        self.sweep();
        if depth == 0 {
            return;
        }
        for index in 0..CELLS {
            self.add(index, depth);
        }
    }

    /// Topple every unstable cell once, all at the same moment, and return how many went.
    ///
    /// Simultaneous is not a detail. Toppling cells one at a time in some order gives the same
    /// final pile (the pile is abelian, which is the surprising theorem about it) but a different
    /// picture on the way, and the picture is what there is to look at.
    ///
    /// Returns the number of cells that toppled and the number of grains that went off the edge.
    pub fn wave(&mut self) -> (u32, u32) {
        if self.hot.is_empty() {
            return (0, 0);
        }
        std::mem::swap(&mut self.hot, &mut self.going);
        self.hot.clear();
        let toppled = self.going.len() as u32;
        let mut lost = 0;

        for &cell in &self.going {
            let index = cell as usize;
            self.grains[index] -= THRESHOLD;
            self.glow[index] = 255;
        }
        // Sharing out is a second pass over the same cells, so that what a cell hands out is what
        // it held when the wave began.
        for step in 0..self.going.len() {
            let index = self.going[step] as usize;
            let column = (index % WIDE) as i32;
            let row = (index / WIDE) as i32;
            for (across, up) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                match Self::index(column + across, row + up) {
                    Some(neighbour) => {
                        self.grains[neighbour] += 1;
                        if self.grains[neighbour] >= THRESHOLD && !self.queued[neighbour] {
                            self.queued[neighbour] = true;
                            self.hot.push(neighbour as u32);
                        }
                    }
                    None => lost += 1,
                }
            }
        }
        // A cell that toppled may have been handed enough to go again.
        for step in 0..self.going.len() {
            let index = self.going[step] as usize;
            self.queued[index] = false;
            if self.grains[index] >= THRESHOLD {
                self.queued[index] = true;
                self.hot.push(index as u32);
            }
        }
        (toppled, lost)
    }
}

/// Which histogram bin an avalanche of this size belongs in, two bins to the octave.
///
/// Integer only, so the histogram is part of what the determinism gate can check. `ilog2` is the
/// octave; the half-octave is a comparison against one and a half times it, done in integers.
pub fn bin_of(size: u32) -> usize {
    if size == 0 {
        return 0;
    }
    let octave = size.ilog2();
    // Doubled rather than halved: `(3 << octave) / 2` throws away the half at the bottom octave,
    // where it is the only thing that distinguishes the two bins, and puts size one in the wrong
    // one. Comparing twice the size against three times the octave is the same test with nothing
    // rounded away.
    let half = u32::from(2 * size >= 3u32 << octave);
    ((octave * 2 + half) as usize).min(BINS - 1)
}

/// The smallest and largest avalanche that lands in a bin.
pub fn bin_span(bin: usize) -> (u32, u32) {
    let octave = bin as u32 / 2;
    // Where the half-octave falls, rounded up, so it agrees with the doubled test in `bin_of`.
    let middle = (3u32 << octave).div_ceil(2);
    let (low, high) = if bin.is_multiple_of(2) {
        (1u32 << octave, middle)
    } else {
        (middle, 1u32 << (octave + 1))
    };
    // The bottom octave holds one integer and cannot really be halved, so its upper bin comes out
    // empty. Nothing is ever binned there; the guard is only so a width is never zero.
    (low, high.max(low + 1))
}

/// How many avalanches of each size there have been.
#[derive(Resource, Clone, PartialEq, Eq, Debug)]
pub struct Sizes {
    /// Counts, two bins to the octave. See [`bin_of`].
    pub bins: [u32; BINS],
}

impl Default for Sizes {
    fn default() -> Self {
        Self { bins: [0; BINS] }
    }
}

impl Sizes {
    /// Forget everything.
    pub fn clear(&mut self) {
        self.bins = [0; BINS];
    }

    /// How many avalanches have been counted.
    pub fn counted(&self) -> u32 {
        self.bins.iter().sum()
    }

    /// The highest bin with anything in it.
    pub fn widest(&self) -> usize {
        self.bins
            .iter()
            .rposition(|&count| count > 0)
            .unwrap_or_default()
    }
}

/// The avalanche going on right now.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Slide {
    /// Whether a grain has landed and the table has not finished with it.
    pub running: bool,
    /// Cells toppled so far.
    pub topples: u32,
    /// Waves so far, which is roughly how far it has spread.
    pub waves: u32,
    /// Whether it was left alone. An avalanche that had more grains dropped on it while it was
    /// still going is a fine thing to watch and useless as a measurement, so it is not counted.
    pub clean: bool,
}

/// Everything the table has done.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Ledger {
    /// Grains dropped on the table.
    pub poured: u64,
    /// Grains that went off the edge.
    pub lost: u64,
    /// Cells toppled, ever.
    pub topples: u64,
    /// Avalanches measured.
    pub measured: u32,
    /// Avalanches thrown away for being disturbed.
    pub disturbed: u32,
    /// Grains that landed and did nothing at all.
    pub duds: u32,
    /// The biggest avalanche so far, in topples.
    pub biggest: u32,
    /// How many waves the biggest one took.
    pub longest: u32,
}

/// Whether the table feeds itself a grain whenever it is still.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rain(pub bool);

impl Default for Rain {
    fn default() -> Self {
        Self(true)
    }
}

/// Nothing moves while this is set.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Paused(pub bool);

/// How fast the table runs, as a multiple of real time.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct Speed(pub f32);

impl Default for Speed {
    fn default() -> Self {
        Self(1.0)
    }
}

/// How much work this tick may do. Written once per tick by [`set_step`].
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct Step {
    /// Cell-topples allowed this tick. Zero while still.
    pub budget: u32,
    /// Whether the table is running at all this tick.
    pub going: bool,
}

/// Where the pointer is on the table, in cells, if it is on the table at all.
pub fn cell_under(point: Vec2) -> Option<(i32, i32)> {
    let local = point - table_origin();
    let column = ((local.x / CELL) + WIDE as f32 / 2.0).floor() as i32;
    let row = ((local.y / CELL) + TALL as f32 / 2.0).floor() as i32;
    Table::index(column, row).map(|_| (column, row))
}

/// The middle of the table, in world units. The table sits low, leaving a strip above it for the
/// readouts.
pub fn table_origin() -> Vec2 {
    vec2(0.0, -70.0)
}

/// Where a cell's middle is, in world units.
pub fn cell_at(column: i32, row: i32) -> Vec2 {
    table_origin()
        + vec2(
            (column as f32 + 0.5 - WIDE as f32 / 2.0) * CELL,
            (row as f32 + 0.5 - TALL as f32 / 2.0) * CELL,
        )
}

/// Installs the table.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut().insert_resource(Table::default());
        app.world_mut().insert_resource(Sizes::default());
        app.world_mut().insert_resource(Slide::default());
        app.world_mut().insert_resource(Ledger::default());
        app.world_mut().insert_resource(Rain::default());
        app.world_mut().insert_resource(Paused::default());
        app.world_mut().insert_resource(Speed::default());
        app.world_mut().insert_resource(Step::default());
        app.add_systems(Startup, load_the_table);
        app.add_systems(
            FixedUpdate,
            (
                pace,
                set_step,
                reset_table,
                pour,
                sprinkle,
                relax,
                fade_glow,
            )
                .chain(),
        );
    }
}

/// Start with two grains in every cell.
///
/// Not the classic empty table, on purpose: from flat it takes forty thousand grains before
/// anything interesting happens, which is ten minutes of watching sand land. Two is just under
/// the level the pile settles at, so it fills up and arrives there within a minute, and the
/// avalanches grow from nothing to enormous while you watch.
///
/// The other two openings are a key away. `f` loads every cell to three, which is one short of
/// toppling everywhere: the next grain takes half the table with it and the average height comes
/// *down* to the same place. `r` sweeps it clean and it climbs up to the same place from zero.
/// That the same number turns up from above, from below, and from the middle is the whole idea.
pub fn load_the_table(mut table: ResMut<Table>, mut ledger: ResMut<Ledger>) {
    table.fill(START_DEPTH);
    ledger.poured = table.total();
}

/// Stillness, pace, rain, and the two ways to start over.
fn pace(
    mut paused: ResMut<Paused>,
    mut speed: ResMut<Speed>,
    mut rain: ResMut<Rain>,
    input: Res<Input>,
) {
    if input.just_pressed(Key::Space) {
        paused.0 = !paused.0;
    }
    if input.just_pressed(Key::T) {
        rain.0 = !rain.0;
    }
    if input.pressed(Key::Up) {
        speed.0 *= SPEED_RAMP;
    }
    if input.pressed(Key::Down) {
        speed.0 /= SPEED_RAMP;
    }
    if input.just_pressed(Key::Digit0) {
        speed.0 = 1.0;
    }
    speed.0 = speed.0.clamp(SPEED_MIN, SPEED_MAX);
}

/// Fix this tick's budget. Pausing is a budget of nothing.
fn set_step(mut step: ResMut<Step>, speed: Res<Speed>, paused: Res<Paused>) {
    step.going = !paused.0;
    step.budget = if paused.0 {
        0
    } else {
        (TOPPLE_BUDGET as f32 * speed.0) as u32
    };
}

/// F loads the table again, R sweeps it clean. Both keep the histogram, since the sizes measured
/// on one table are the same sizes measured on the next one.
fn reset_table(
    mut table: ResMut<Table>,
    mut ledger: ResMut<Ledger>,
    mut slide: ResMut<Slide>,
    mut sizes: ResMut<Sizes>,
    input: Res<Input>,
) {
    if input.just_pressed(Key::F) {
        let was = table.total();
        table.fill(FILL_DEPTH);
        ledger.poured += table.total().saturating_sub(was);
        *slide = Slide::default();
    }
    if input.just_pressed(Key::R) {
        table.sweep();
        *slide = Slide::default();
    }
    if input.just_pressed(Key::X) {
        sizes.clear();
        ledger.measured = 0;
        ledger.disturbed = 0;
        ledger.duds = 0;
        ledger.biggest = 0;
        ledger.longest = 0;
    }
}

/// Hold the pointer down to pour, or press B for a handful.
fn pour(
    mut table: ResMut<Table>,
    mut ledger: ResMut<Ledger>,
    mut slide: ResMut<Slide>,
    input: Res<Input>,
    step: Res<Step>,
) {
    if !step.going {
        return;
    }
    let Some((column, row)) = cell_under(input.mouse_world()) else {
        return;
    };
    let handful = input.just_pressed(Key::B) || input.mouse_just_pressed(MouseButton::Right);
    let stream = input.mouse_pressed(MouseButton::Left);
    if !handful && !stream {
        return;
    }

    let mut dropped = 0u64;
    if handful {
        for down in -HANDFUL_REACH..=HANDFUL_REACH {
            for across in -HANDFUL_REACH..=HANDFUL_REACH {
                if across * across + down * down > HANDFUL_REACH * HANDFUL_REACH {
                    continue;
                }
                if let Some(index) = Table::index(column + across, row + down) {
                    table.add(index, HANDFUL_DEPTH);
                    dropped += HANDFUL_DEPTH as u64;
                }
            }
        }
    } else if let Some(index) = Table::index(column, row) {
        table.add(index, POUR_RATE);
        dropped += POUR_RATE as u64;
    }
    if dropped == 0 {
        return;
    }
    ledger.poured += dropped;
    // Anything landing on a moving pile spoils the measurement, and is meant to be allowed.
    if slide.running {
        slide.clean = false;
    } else {
        *slide = Slide {
            running: true,
            clean: false,
            ..Slide::default()
        };
    }
}

/// One grain, dropped somewhere at random, whenever the table is still.
///
/// This is the whole experiment. A grain lands, the table does whatever it does, and when it has
/// finished the size of what happened goes in the histogram. Nothing else drives the piece, and
/// nothing about that grain knows or cares where the pile is steep.
fn sprinkle(
    mut table: ResMut<Table>,
    mut ledger: ResMut<Ledger>,
    mut slide: ResMut<Slide>,
    mut rng: ResMut<SimRng>,
    rain: Res<Rain>,
    step: Res<Step>,
) {
    if !step.going || !rain.0 || table.busy() || slide.running {
        return;
    }
    let column = rng.range_i32(0..WIDE as i32);
    let row = rng.range_i32(0..TALL as i32);
    let Some(index) = Table::index(column, row) else {
        return;
    };
    table.add(index, 1);
    ledger.poured += 1;
    *slide = Slide {
        running: true,
        clean: true,
        ..Slide::default()
    };
}

/// Run waves until the table is still or the tick has done enough.
fn relax(
    mut table: ResMut<Table>,
    mut slide: ResMut<Slide>,
    mut sizes: ResMut<Sizes>,
    mut ledger: ResMut<Ledger>,
    step: Res<Step>,
) {
    if !step.going {
        return;
    }
    let mut spent = 0;
    while spent < step.budget {
        let (toppled, lost) = table.wave();
        if toppled == 0 {
            if slide.running {
                settle(&mut slide, &mut sizes, &mut ledger);
            }
            break;
        }
        slide.topples += toppled;
        slide.waves += 1;
        ledger.topples += toppled as u64;
        ledger.lost += lost as u64;
        spent += toppled;
    }
}

/// Write down what just happened, and get ready for the next one.
fn settle(slide: &mut Slide, sizes: &mut Sizes, ledger: &mut Ledger) {
    if slide.topples == 0 {
        ledger.duds += 1;
    } else if slide.clean {
        sizes.bins[bin_of(slide.topples)] += 1;
        ledger.measured += 1;
        if slide.topples > ledger.biggest {
            ledger.biggest = slide.topples;
            ledger.longest = slide.waves;
        }
    } else {
        ledger.disturbed += 1;
        // Still worth remembering, even if it does not go in the histogram.
        if slide.topples > ledger.biggest {
            ledger.biggest = slide.topples;
            ledger.longest = slide.waves;
        }
    }
    *slide = Slide::default();
}

/// The mark a toppling cell leaves fades, which is what draws the shape of an avalanche after it
/// has gone through.
fn fade_glow(mut table: ResMut<Table>, step: Res<Step>) {
    if !step.going {
        return;
    }
    for glow in &mut table.glow {
        *glow = glow.saturating_sub(GLOW_FADE);
    }
}
