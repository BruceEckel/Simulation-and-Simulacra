//! Ten fractals and the machinery for looking at them.
//!
//! Two families share one viewer, because "fractal" names two quite different things.
//!
//! **Escape-time fields.** Mandelbrot, Julia, Burning Ship, Newton and Phoenix ask the same
//! question of every point of the plane: feed it to a rule over and over, and does it run away
//! or stay put? The answer is a number per point, and the picture is that number colored in.
//! These are computed into [`Field`], one sample per grid cell.
//!
//! **Chaos-game clouds.** The fern, the Sierpinski triangle, the dragon, the tree and the Koch
//! snowflake are attractors of a handful of affine maps. Drop a point anywhere, apply a
//! randomly chosen map a few million times, and the points land on the shape and nowhere else.
//! These accumulate into [`Cloud`], one speck per hit.
//!
//! Both are drawn progressively under a fixed per-tick budget, so a deep zoom or a heavy
//! iteration count costs sharpness rather than frame rate. The budget is a constant and not a
//! time measurement, which is what keeps a replay identical: the same tick always does exactly
//! the same amount of work.
//!
//! Pure logic — no sprites, no color — so it runs headless for the determinism test. Color
//! lives in the binary, which is the only part that has an opinion about how pretty this is.
//!
//! The grid resizes with the window through [`RESIZE_COMMAND`] on the replayable command
//! channel, never by reading renderer state.

use fulcrum::prelude::*;
use std::f64::consts::{LN_2, TAU};

// ---------------------------------------------------------------------------------------
// the shape of the picture
// ---------------------------------------------------------------------------------------

/// Cells the grid aims for, whatever shape the window is.
///
/// This is a sprite budget as much as a resolution. The renderer draws sprites, not textures,
/// so an escape-time picture is a grid of tinted squares and every one of them is a sprite.
pub const CELL_TARGET: f32 = 40_000.0;

/// Aspect-ratio limits. The court and the grid are cut from the same clamped ratio, which is
/// what keeps cells square however the window is dragged.
pub const ASPECT_LIMITS: (f32, f32) = (0.4, 3.2);

/// Window size at startup, and the court area every resize preserves.
pub const DEFAULT_WINDOW: Vec2 = Vec2::new(1280.0, 800.0);

/// Fewest cells across the grid may go, so a sliver of a window still shows something.
pub const GRID_MIN: u32 = 24;
/// Most cells across the grid may go, so an enormous window can't blow the sprite budget.
pub const GRID_MAX: u32 = 420;

/// Name of the resize command on the replayable command channel.
pub const RESIZE_COMMAND: &str = "window";

/// The court a window of this size gets: the window's shape, at a fixed area.
pub fn court_for_window(window: Vec2) -> Vec2 {
    let area = DEFAULT_WINDOW.x * DEFAULT_WINDOW.y;
    let aspect = (window.x / window.y).clamp(ASPECT_LIMITS.0, ASPECT_LIMITS.1);
    vec2(
        (area * aspect).sqrt().round(),
        (area / aspect).sqrt().round(),
    )
}

/// The grid a window of this size gets: the window's shape, at a fixed cell count.
pub fn grid_for_window(window: Vec2) -> Grid {
    let aspect = (window.x / window.y).clamp(ASPECT_LIMITS.0, ASPECT_LIMITS.1);
    let low = GRID_MIN as f32;
    let high = GRID_MAX as f32;
    Grid {
        width: (CELL_TARGET * aspect).sqrt().round().clamp(low, high) as u32,
        height: (CELL_TARGET / aspect).sqrt().round().clamp(low, high) as u32,
    }
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

// ---------------------------------------------------------------------------------------
// budgets
// ---------------------------------------------------------------------------------------

/// Iterations the escape-time renderer may spend in one tick.
///
/// A constant rather than a time measurement, on purpose: a budget that watched the clock
/// would make the picture depend on how fast the machine is, and a replay would diverge. The
/// cost of a hard view is paid in sharpness — the picture arrives coarse and refines — never
/// in a dropped frame.
pub const ITERATION_BUDGET: u64 = 1_400_000;

/// Block size of the first pass, as a power of two. The picture is drawn at 1:8, then 1:4,
/// 1:2, and finally 1:1, so a fresh view is legible immediately and sharpens from there.
pub const COARSEST_LEVEL: u32 = 3;

/// Chaos-game steps the cloud renderer may take in one tick.
pub const CHAOS_BUDGET: u32 = 7_000;

/// Specks a cloud holds. The second sprite budget in this file.
pub const CLOUD_POINTS: usize = 18_000;

/// Steps taken before any point is kept. The chaos game starts wherever it likes and is only
/// on the attractor in the limit; a few dozen steps is far more than enough to land on it.
pub const CHAOS_WARMUP: u32 = 30;

/// Steps after which a cloud gives up trying to fill. Zoom into a corner where the attractor
/// never goes and the hit rate is zero; this is what stops that costing anything forever.
pub const CHAOS_LIMIT: u64 = 3_000_000;

// ---------------------------------------------------------------------------------------
// controls
// ---------------------------------------------------------------------------------------

/// Fewest iterations before a point is declared to be inside the set.
pub const DEPTH_MIN: u32 = 24;
/// Most iterations before a point is declared to be inside the set.
pub const DEPTH_MAX: u32 = 6_000;
/// Iteration limit at startup. Enough for the whole of every home view.
pub const DEPTH_START: u32 = 220;
/// How much a held detail key multiplies the iteration limit each tick.
pub const DEPTH_RAMP: f32 = 1.04;

/// How far a held arrow key pans per tick, as a fraction of the view.
pub const PAN_STEP: f64 = 0.012;
/// How much a held zoom key multiplies the view width each tick.
pub const ZOOM_STEP: f64 = 1.022;
/// How much one notch of the wheel multiplies the view width.
pub const SCROLL_STEP: f64 = 1.18;
/// How much a left click multiplies the view width. A right click uses its reciprocal.
pub const CLICK_STEP: f64 = 0.45;

/// Narrowest view allowed. Past roughly here `f64` runs out of mantissa and the picture goes
/// blocky for real rather than pending — so this is where the zoom honestly stops.
pub const SPAN_MIN: f64 = 2.0e-13;
/// Widest view allowed, so you cannot lose the fractal entirely.
pub const SPAN_MAX: f64 = 24.0;

/// The circle the Julia constant travels around: `c = 0.7885 e^{i0}`. Every point of it is an
/// interesting Julia set, which is why this particular circle is the one everybody animates.
pub const JULIA_RADIUS: f64 = 0.7885;
/// How far around that circle each finished picture moves. Small: this is a drift, not a spin.
pub const JULIA_STEP: f64 = 0.018;
/// Where on the circle it starts: the one part of it that is inside the Mandelbrot set.
///
/// Almost all of this circle lies outside, and a `c` outside the Mandelbrot set has a Julia
/// set that is dust — infinitely many points, no interior, nothing joined to anything. Those
/// are worth seeing and the drift spends most of its time among them, but they are a strange
/// thing to be shown first and told is a Julia set. Starting here opens on the one connected
/// set the circle passes through, and the drift then takes it apart in front of you, which is
/// as good a demonstration of what the Mandelbrot set *means* as this program has.
pub const JULIA_START: f64 = std::f64::consts::PI;

// ---------------------------------------------------------------------------------------
// the ten
// ---------------------------------------------------------------------------------------

/// Which fractal is on screen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fractal {
    /// `z -> z^2 + c` over every starting `c`. The one everyone means.
    Mandelbrot,
    /// `z -> z^2 + c` for one fixed `c`, over every starting `z`.
    Julia,
    /// Mandelbrot's rule with the signs stripped off each part before squaring.
    BurningShip,
    /// Newton's method hunting the cube roots of one, colored by which root it finds.
    Newton,
    /// A quadratic with a memory: each step also reaches back to the step before it.
    Phoenix,
    /// Barnsley's fern: four affine maps, one of them almost never chosen.
    Fern,
    /// The Sierpinski triangle, by the midpoint game.
    Sierpinski,
    /// The Heighway dragon: fold a strip of paper in half forever.
    Dragon,
    /// A branching tree, also four maps, also Barnsley's.
    Tree,
    /// The Koch snowflake: one Koch curve on each side of a triangle.
    Snowflake,
}

/// Every fractal, in the order the number keys pick them.
pub const FRACTALS: [Fractal; 10] = [
    Fractal::Mandelbrot,
    Fractal::Julia,
    Fractal::BurningShip,
    Fractal::Newton,
    Fractal::Phoenix,
    Fractal::Fern,
    Fractal::Sierpinski,
    Fractal::Dragon,
    Fractal::Tree,
    Fractal::Snowflake,
];

impl Fractal {
    /// Its place in [`FRACTALS`], which is also the number key that picks it.
    pub fn index(self) -> usize {
        FRACTALS.iter().position(|&kind| kind == self).unwrap_or(0)
    }

    /// Its name, for the readout.
    pub fn name(self) -> &'static str {
        match self {
            Fractal::Mandelbrot => "MANDELBROT",
            Fractal::Julia => "JULIA",
            Fractal::BurningShip => "BURNING SHIP",
            Fractal::Newton => "NEWTON",
            Fractal::Phoenix => "PHOENIX",
            Fractal::Fern => "BARNSLEY FERN",
            Fractal::Sierpinski => "SIERPINSKI TRIANGLE",
            Fractal::Dragon => "DRAGON CURVE",
            Fractal::Tree => "FRACTAL TREE",
            Fractal::Snowflake => "KOCH SNOWFLAKE",
        }
    }

    /// One line on what makes it itself.
    pub fn blurb(self) -> &'static str {
        match self {
            Fractal::Mandelbrot => {
                "square, add, repeat: the black part is every c whose orbit never runs away"
            }
            Fractal::Julia => {
                "same rule, c held still and drifting: watch it come apart as c leaves the set"
            }
            Fractal::BurningShip => {
                "Mandelbrot's rule with both signs thrown away before each squaring"
            }
            Fractal::Newton => {
                "root-hunting on z^3 = 1: the color is which of the three roots it fell into"
            }
            Fractal::Phoenix => "a squaring with one step of memory, which folds it into wings",
            Fractal::Fern => "four affine maps; the stem is the one picked once in a hundred",
            Fractal::Sierpinski => "jump halfway to a corner chosen at random, forever",
            Fractal::Dragon => "two maps, each a half turn and a shrink; a paper strip folded",
            Fractal::Tree => "four maps: a trunk, two boughs at 45 degrees, and a bud",
            Fractal::Snowflake => "the Koch curve, on each of three sides, bulging outward",
        }
    }

    /// True when this one is a cloud of specks rather than a field of cells.
    pub fn is_cloud(self) -> bool {
        self.cloud().is_some()
    }

    /// True when this one moves on its own, so the pause key has something to hold.
    pub fn is_moving(self) -> bool {
        matches!(self, Fractal::Julia)
    }

    /// Where it lives and how much room it needs: centre, then the least width and height of
    /// complex plane that show the whole thing.
    ///
    /// Width and height are both given because the shapes disagree about which way is long —
    /// the fern is far taller than it is wide, the dragon the other way — and the window can
    /// be any shape at all. [`home`](Self::home) picks whichever is binding.
    pub fn home_frame(self) -> (f64, f64, f64, f64) {
        // Measured, not guessed: each of these is the box the fractal actually occupies, plus
        // a margin. Several of them are nowhere near the origin and two are far taller than
        // they are wide, so guessing would have cut them off.
        match self {
            Fractal::Mandelbrot => (-0.70, 0.00, 3.10, 2.35),
            Fractal::Julia => (0.00, 0.00, 3.00, 2.20),
            Fractal::BurningShip => (-0.40, 0.50, 3.10, 2.30),
            Fractal::Newton => (0.00, 0.00, 3.00, 2.40),
            Fractal::Phoenix => (0.00, 0.04, 2.80, 1.70),
            Fractal::Fern => (0.24, 5.03, 5.30, 10.60),
            Fractal::Sierpinski => (0.00, 0.25, 2.00, 1.75),
            Fractal::Dragon => (0.42, 0.17, 1.75, 1.20),
            Fractal::Tree => (0.00, 0.23, 0.58, 0.50),
            Fractal::Snowflake => (0.00, 0.00, 1.20, 1.35),
        }
    }

    /// The view it opens at, on this grid: its frame, widened to whichever of the two
    /// requirements the window's shape makes binding.
    pub fn home(self, grid: Grid) -> View {
        let (center_x, center_y, want_wide, want_tall) = self.home_frame();
        View {
            center_x,
            center_y,
            span: want_wide.max(want_tall * grid.aspect()),
        }
    }

    /// The maps whose attractor this is, or `None` for the escape-time fractals.
    pub fn cloud(self) -> Option<CloudSpec> {
        match self {
            Fractal::Fern => Some(CloudSpec::new(FERN)),
            Fractal::Sierpinski => Some(CloudSpec::new(SIERPINSKI)),
            Fractal::Dragon => Some(CloudSpec::new(DRAGON)),
            Fractal::Tree => Some(CloudSpec::new(TREE)),
            // One Koch curve, laid along the bottom side of a triangle centred on the origin
            // and flipped so it bulges away from the middle, then turned onto the other two
            // sides. Three turns of a one-sided attractor is the whole snowflake.
            Fractal::Snowflake => Some(CloudSpec {
                maps: KOCH,
                place: Some(Affine::new(
                    1.0,
                    0.0,
                    0.0,
                    -1.0,
                    -0.5,
                    -TRIANGLE_INRADIUS,
                    1.0,
                )),
                symmetry: 3,
            }),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------------------
// escape-time fractals
// ---------------------------------------------------------------------------------------

/// How far a point has to get before it counts as gone.
///
/// Much larger than the 2 that decides the question, on purpose. The smooth-iteration count
/// below is only exact in the limit of a large escape radius, and 512 is where the leftover
/// error stops being visible as a seam between color bands.
pub const BAILOUT: f64 = 512.0;

/// The three cube roots of one, which every Newton orbit ends at.
pub const CUBE_ROOTS: [(f64, f64); 3] = [
    (1.0, 0.0),
    (-0.5, 0.866_025_403_784_438_6),
    (-0.5, -0.866_025_403_784_438_6),
];

/// How close to a root counts as arrived, as a squared distance.
pub const NEWTON_TOLERANCE: f64 = 1.0e-18;

/// The Phoenix constant, and the weight on its one step of memory.
pub const PHOENIX_C: f64 = 0.5667;
/// How much of the previous step each new one keeps.
pub const PHOENIX_P: f64 = -0.5;

/// What one point of the plane turned out to be.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sample {
    /// For a point that escaped, how many steps it took, with a fractional part so that the
    /// count is continuous across a band edge instead of stepping. For one that never
    /// escaped, how close its orbit ever came to the origin, from 0 to 1 — which is what
    /// gives the interior its structure instead of a flat black.
    pub value: f32,
    /// Which basin it fell into. Always 0 except for Newton, which has three.
    pub band: u8,
    /// True when the point never escaped inside the iteration limit.
    pub inside: bool,
}

/// What one point of the plane is, under this fractal's rule.
///
/// `phase` is only read by [`Fractal::Julia`], as the angle of its travelling constant.
pub fn sample_at(kind: Fractal, x: f64, y: f64, depth: u32, phase: f64) -> Sample {
    match kind {
        // The plane is c and every orbit starts at zero.
        Fractal::Mandelbrot => quadratic(0.0, 0.0, x, y, depth, false),
        // The plane is the starting z and c is the same everywhere. Which c is the whole
        // question, so it travels: every point of this circle is a different Julia set.
        Fractal::Julia => quadratic(
            x,
            y,
            JULIA_RADIUS * phase.cos(),
            JULIA_RADIUS * phase.sin(),
            depth,
            false,
        ),
        // Same as Mandelbrot but for the absolute values, which breaks the symmetry that
        // makes Mandelbrot smooth and leaves the masts and rigging instead. The imaginary
        // axis is flipped so the ship sits the way its pictures always show it.
        Fractal::BurningShip => quadratic(0.0, 0.0, x, -y, depth, true),
        Fractal::Newton => newton(x, y, depth),
        Fractal::Phoenix => phoenix(x, y, depth),
        // A cloud has no per-point answer to give.
        _ => Sample::default(),
    }
}

/// How much work [`sample_at`] did, which is what the per-tick budget is spent in. Derived
/// rather than returned: a point that escaped says so in its own step count, and one that
/// did not ran to the limit by definition.
pub fn cost_of(sample: Sample, depth: u32) -> u64 {
    if sample.inside {
        depth as u64
    } else {
        sample.value as u64 + 2
    }
}

/// `z -> z^2 + c`, optionally taking the absolute value of each part first (which is the
/// Burning Ship). Mandelbrot and Julia are the same loop differing only in what starts where.
fn quadratic(mut zx: f64, mut zy: f64, cx: f64, cy: f64, depth: u32, ship: bool) -> Sample {
    let limit = BAILOUT * BAILOUT;
    // Nearest the orbit ever comes to the origin. Only read for points that never escape,
    // where the escape count has nothing left to say.
    let mut trap = f64::MAX;
    for step in 0..depth {
        if ship {
            zx = zx.abs();
            zy = zy.abs();
        }
        let next_x = zx * zx - zy * zy + cx;
        let next_y = 2.0 * zx * zy + cy;
        zx = next_x;
        zy = next_y;
        let magnitude = zx * zx + zy * zy;
        trap = trap.min(magnitude);
        if magnitude > limit {
            return escaped(step, magnitude);
        }
    }
    trapped(trap)
}

/// `z -> z^2 + c + p z_prev`. The plane is the starting point, as in a Julia set, and the
/// term reaching one step back is what folds the picture into its two wings.
///
/// The axes are swapped on the way in, which is how every published picture of this set is
/// oriented: it stands up rather than lying on its side.
fn phoenix(x: f64, y: f64, depth: u32) -> Sample {
    let limit = BAILOUT * BAILOUT;
    let (mut zx, mut zy) = (y, x);
    let (mut prev_x, mut prev_y) = (0.0, 0.0);
    let mut trap = f64::MAX;
    for step in 0..depth {
        let next_x = zx * zx - zy * zy + PHOENIX_C + PHOENIX_P * prev_x;
        let next_y = 2.0 * zx * zy + PHOENIX_P * prev_y;
        prev_x = zx;
        prev_y = zy;
        zx = next_x;
        zy = next_y;
        let magnitude = zx * zx + zy * zy;
        trap = trap.min(magnitude);
        if magnitude > limit {
            return escaped(step, magnitude);
        }
    }
    trapped(trap)
}

/// Newton's method on `f(z) = z^3 - 1`, which every starting point but one solves. Which of
/// the three roots it arrives at is the picture: the three basins are tangled at every scale,
/// and their boundary is the fractal.
fn newton(x: f64, y: f64, depth: u32) -> Sample {
    let (mut zx, mut zy) = (x, y);
    for step in 0..depth {
        // z - (z^3 - 1) / (3 z^2), written out in real arithmetic.
        let (square_x, square_y) = (zx * zx - zy * zy, 2.0 * zx * zy);
        let (cube_x, cube_y) = (square_x * zx - square_y * zy, square_x * zy + square_y * zx);
        let (top_x, top_y) = (cube_x - 1.0, cube_y);
        let (bottom_x, bottom_y) = (3.0 * square_x, 3.0 * square_y);
        let divisor = bottom_x * bottom_x + bottom_y * bottom_y;
        if divisor == 0.0 {
            break; // the origin, where the method has nothing to go on
        }
        zx -= (top_x * bottom_x + top_y * bottom_y) / divisor;
        zy -= (top_y * bottom_x - top_x * bottom_y) / divisor;
        for (root, &(root_x, root_y)) in CUBE_ROOTS.iter().enumerate() {
            let gap = (zx - root_x) * (zx - root_x) + (zy - root_y) * (zy - root_y);
            if gap < NEWTON_TOLERANCE {
                return Sample {
                    value: newton_smooth(step, gap) as f32,
                    band: root as u8,
                    inside: false,
                };
            }
        }
    }
    // Only the origin and points that ran out of steps land here.
    Sample {
        value: 0.0,
        band: 0,
        inside: true,
    }
}

/// A fractional step count for a Newton orbit that has arrived.
///
/// The method converges quadratically, so the log of the remaining error halves at every
/// step; comparing where the error landed against where it had to get turns the integer count
/// into a smooth one, and the basins lose their contour lines.
fn newton_smooth(step: u32, gap: f64) -> f64 {
    let reached = gap.sqrt().ln();
    let needed = NEWTON_TOLERANCE.sqrt().ln();
    let smooth = step as f64 - (reached / needed).ln() / LN_2;
    if smooth.is_finite() {
        smooth.max(0.0)
    } else {
        step as f64
    }
}

/// A point that got away, at `step`, having landed at squared magnitude `magnitude`.
///
/// The fractional part is how far past the escape radius it overshot. Without it the picture
/// is a staircase of flat bands; with it, a continuous palette comes out continuous.
fn escaped(step: u32, magnitude: f64) -> Sample {
    let smooth = step as f64 + 1.0 - (0.5 * magnitude.ln()).ln() / LN_2;
    Sample {
        value: smooth.max(0.0) as f32,
        band: 0,
        inside: false,
    }
}

/// A point that never got away, recorded by its orbit's nearest approach to the origin.
fn trapped(trap: f64) -> Sample {
    Sample {
        value: (trap.sqrt().min(2.0) / 2.0) as f32,
        band: 0,
        inside: true,
    }
}

// ---------------------------------------------------------------------------------------
// chaos-game fractals
// ---------------------------------------------------------------------------------------

/// One affine map: `(x, y) -> (a x + b y + e, c x + d y + f)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    /// x from x.
    pub a: f64,
    /// x from y.
    pub b: f64,
    /// y from x.
    pub c: f64,
    /// y from y.
    pub d: f64,
    /// Shift along x.
    pub e: f64,
    /// Shift along y.
    pub f: f64,
    /// Its share of the steps, relative to the others in its set. A map's share should be
    /// roughly the share of the attractor it is responsible for, or the thin parts come out
    /// grainy and the fat parts come out saturated.
    pub weight: f32,
}

impl Affine {
    /// A map, spelled out.
    pub const fn new(a: f64, b: f64, c: f64, d: f64, e: f64, f: f64, weight: f32) -> Self {
        Self {
            a,
            b,
            c,
            d,
            e,
            f,
            weight,
        }
    }

    /// Where this map sends a point.
    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.b * y + self.e,
            self.c * x + self.d * y + self.f,
        )
    }
}

/// A set of maps, and what to do with the attractor once it has been found.
#[derive(Clone, Copy, Debug)]
pub struct CloudSpec {
    /// The maps whose attractor this is.
    pub maps: &'static [Affine],
    /// Applied to every point after the game and before [`symmetry`](Self::symmetry). Used to
    /// put an attractor somewhere other than where its maps happen to leave it.
    pub place: Option<Affine>,
    /// How many turns of the attractor about the origin make the finished picture. One for
    /// everything but the snowflake, which is three copies of a one-sided curve.
    pub symmetry: u32,
}

impl CloudSpec {
    /// A set of maps taken exactly as they are.
    pub const fn new(maps: &'static [Affine]) -> Self {
        Self {
            maps,
            place: None,
            symmetry: 1,
        }
    }
}

/// Barnsley's fern. The first map is chosen once in a hundred steps and draws the whole stem;
/// the second, chosen five times in six, is the one that makes it a fern rather than a bush.
pub const FERN: &[Affine] = &[
    Affine::new(0.00, 0.00, 0.00, 0.16, 0.00, 0.00, 0.01),
    Affine::new(0.85, 0.04, -0.04, 0.85, 0.00, 1.60, 0.85),
    Affine::new(0.20, -0.26, 0.23, 0.22, 0.00, 1.60, 0.07),
    Affine::new(-0.15, 0.28, 0.26, 0.24, 0.00, 0.44, 0.07),
];

/// Half the way to one of three corners, chosen at random. That this makes a Sierpinski
/// triangle rather than a smear is the surprise the chaos game is famous for.
pub const SIERPINSKI: &[Affine] = &[
    Affine::new(0.5, 0.0, 0.0, 0.5, 0.0, 0.5, 1.0),
    Affine::new(0.5, 0.0, 0.0, 0.5, -0.433_012_701_892_219_3, -0.25, 1.0),
    Affine::new(0.5, 0.0, 0.0, 0.5, 0.433_012_701_892_219_3, -0.25, 1.0),
];

/// The Heighway dragon: two maps, each a shrink by root two and a quarter turn, one of them
/// reversed. The shape a strip of paper takes when folded in half over and over.
pub const DRAGON: &[Affine] = &[
    Affine::new(0.5, -0.5, 0.5, 0.5, 0.0, 0.0, 1.0),
    Affine::new(-0.5, -0.5, 0.5, -0.5, 1.0, 0.0, 1.0),
];

/// Barnsley's tree: a trunk that is almost never chosen, two boughs leaning 45 degrees apart,
/// and a bud that shrinks everything to a tenth and starts again.
pub const TREE: &[Affine] = &[
    Affine::new(0.00, 0.00, 0.00, 0.50, 0.00, 0.00, 0.05),
    Affine::new(0.42, -0.42, 0.42, 0.42, 0.00, 0.20, 0.40),
    Affine::new(0.42, 0.42, -0.42, 0.42, 0.00, 0.20, 0.40),
    Affine::new(0.10, 0.00, 0.00, 0.10, 0.00, 0.20, 0.15),
];

/// A third, exactly, as far as `f64` will say it.
const THIRD: f64 = 1.0 / 3.0;
/// A sixth: the scale of a Koch map's rotated parts.
const SIXTH: f64 = 1.0 / 6.0;
/// Height of the Koch bump, `sqrt(3) / 6`, which is also the inradius of a unit triangle.
const KOCH_RISE: f64 = 0.288_675_134_594_812_87;
/// Distance from the centre of a unit equilateral triangle to the middle of a side.
const TRIANGLE_INRADIUS: f64 = KOCH_RISE;

/// The Koch curve, from `(0, 0)` to `(1, 0)`: four copies of itself at a third the size, the
/// middle two turned 60 degrees apart to make the bump.
pub const KOCH: &[Affine] = &[
    Affine::new(THIRD, 0.0, 0.0, THIRD, 0.0, 0.0, 1.0),
    Affine::new(SIXTH, -KOCH_RISE, KOCH_RISE, SIXTH, THIRD, 0.0, 1.0),
    Affine::new(SIXTH, KOCH_RISE, -KOCH_RISE, SIXTH, 0.5, KOCH_RISE, 1.0),
    Affine::new(THIRD, 0.0, 0.0, THIRD, 2.0 * THIRD, 0.0, 1.0),
];

/// One point of a cloud.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Speck {
    /// Where it landed, in the complex plane.
    pub x: f32,
    /// Where it landed, in the complex plane.
    pub y: f32,
    /// Which of the set's maps put it there.
    pub band: u8,
    /// Where it sits across the shape as a whole, from roughly -1 to 1. Presentation reads
    /// this to lay a gradient over the attractor rather than coloring it in blocks.
    pub tone: f32,
}

// ---------------------------------------------------------------------------------------
// state
// ---------------------------------------------------------------------------------------

/// How many cells the picture is divided into. Simulation state, changed only by
/// [`RESIZE_COMMAND`].
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Grid {
    /// Cells across.
    pub width: u32,
    /// Cells down.
    pub height: u32,
}

impl Default for Grid {
    fn default() -> Self {
        grid_for_window(DEFAULT_WINDOW)
    }
}

impl Grid {
    /// Cells in the whole grid.
    pub fn cells(&self) -> usize {
        self.width as usize * self.height as usize
    }

    /// Where cell `(x, y)` sits in a [`Field`]'s samples.
    pub fn index(&self, x: u32, y: u32) -> usize {
        y as usize * self.width as usize + x as usize
    }

    /// Cells across per cell down.
    pub fn aspect(&self) -> f64 {
        self.width as f64 / self.height as f64
    }
}

/// The world-unit rectangle the picture is drawn into. Simulation state so that the mapping
/// from a mouse position to a point of the plane does not have to ask the renderer anything.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct Court(pub Vec2);

impl Default for Court {
    fn default() -> Self {
        Self(court_for_window(DEFAULT_WINDOW))
    }
}

/// Which piece of the complex plane is on screen.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct View {
    /// Middle of the picture, real part.
    pub center_x: f64,
    /// Middle of the picture, imaginary part.
    pub center_y: f64,
    /// How wide the picture is, in the plane. Height follows from the grid's shape, which is
    /// what keeps a fractal from stretching when the window does.
    pub span: f64,
}

impl View {
    /// How wide and how tall the picture is, in the plane.
    pub fn extents(&self, grid: Grid) -> (f64, f64) {
        (self.span, self.span / grid.aspect())
    }

    /// The point of the plane that cell `(x, y)` stands for.
    pub fn complex_at(&self, grid: Grid, x: u32, y: u32) -> (f64, f64) {
        let step = self.span / grid.width as f64;
        (
            self.center_x + (x as f64 + 0.5 - grid.width as f64 / 2.0) * step,
            self.center_y + (y as f64 + 0.5 - grid.height as f64 / 2.0) * step,
        )
    }

    /// The point of the plane a world position lands on.
    pub fn complex_of_world(&self, grid: Grid, court: Vec2, world: Vec2) -> (f64, f64) {
        let (wide, tall) = self.extents(grid);
        (
            self.center_x + (world.x / court.x) as f64 * wide,
            self.center_y + (world.y / court.y) as f64 * tall,
        )
    }

    /// Where in the court a point of the plane is drawn.
    pub fn world_of_complex(&self, grid: Grid, court: Vec2, x: f64, y: f64) -> Vec2 {
        let (wide, tall) = self.extents(grid);
        vec2(
            ((x - self.center_x) / wide) as f32 * court.x,
            ((y - self.center_y) / tall) as f32 * court.y,
        )
    }

    /// Whether a point of the plane is on screen at all.
    pub fn holds(&self, grid: Grid, x: f64, y: f64) -> bool {
        let (wide, tall) = self.extents(grid);
        (x - self.center_x).abs() <= wide / 2.0 && (y - self.center_y).abs() <= tall / 2.0
    }

    /// Zoom by `factor` about a fixed point, so whatever is under the pointer stays under it.
    pub fn zoom_about(&mut self, anchor: (f64, f64), factor: f64) {
        let span = (self.span * factor).clamp(SPAN_MIN, SPAN_MAX);
        // Not `factor`: at the ends of the range the zoom is clipped, and the anchor has to
        // be held against what actually happened rather than what was asked for.
        let applied = span / self.span;
        self.center_x = anchor.0 + (self.center_x - anchor.0) * applied;
        self.center_y = anchor.1 + (self.center_y - anchor.1) * applied;
        self.span = span;
    }
}

impl Default for View {
    fn default() -> Self {
        Fractal::Mandelbrot.home(Grid::default())
    }
}

/// Which fractal is on screen.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Selection(pub Fractal);

impl Default for Selection {
    fn default() -> Self {
        Self(Fractal::Mandelbrot)
    }
}

/// Where each fractal was left, so that coming back to one returns to it rather than to its
/// front door. `None` means it has not been looked at yet, or has been sent home since, and
/// its opening view should be worked out fresh for whatever shape the window is now.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct Views(pub [Option<View>; FRACTALS.len()]);

/// How many steps a point gets before it is called a member of the set.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Depth(pub u32);

impl Default for Depth {
    fn default() -> Self {
        Self(DEPTH_START)
    }
}

/// Whether the things that move are moving.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct Motion {
    /// The angle of Julia's travelling constant.
    pub phase: f64,
    /// False while the pause key is holding everything still.
    pub running: bool,
}

impl Default for Motion {
    fn default() -> Self {
        Self {
            phase: JULIA_START,
            running: true,
        }
    }
}

/// The escape-time picture, and how much of it has been worked out so far.
#[derive(Resource, Clone, Debug, Default)]
pub struct Field {
    /// One sample per cell, in row order.
    pub samples: Vec<Sample>,
    /// Block size of the pass under way, as a power of two: 3 is the first and coarsest pass,
    /// 0 is the last and finishes the picture.
    pub level: u32,
    /// How far through the current pass.
    pub cursor: usize,
    /// True when level 0 has finished and there is nothing left to sharpen.
    pub done: bool,
    /// Fingerprint of the view this was computed for. When it stops matching, the picture is
    /// stale and starts again.
    pub stamp: u64,
    /// How many pictures have been finished. The Julia constant moves on each one, which is
    /// what paces its drift to whatever the machine can keep up with.
    pub finished: u64,
}

impl Field {
    /// Start again from the coarsest pass, sized for this grid.
    pub fn restart(&mut self, stamp: u64, cells: usize, idle: bool) {
        self.samples.clear();
        self.samples.resize(cells, Sample::default());
        self.level = COARSEST_LEVEL;
        self.cursor = 0;
        self.done = idle;
        self.stamp = stamp;
    }

    /// How coarse the picture is right now: 1 once it is finished, 8 on the first pass.
    pub fn detail(&self) -> u32 {
        if self.done { 1 } else { 1 << self.level }
    }
}

/// The chaos-game picture, and how much of it has landed so far.
#[derive(Resource, Clone, Debug, Default)]
pub struct Cloud {
    /// Every point found so far that is on screen.
    pub points: Vec<Speck>,
    /// Where the game has got to. Not a point of the picture — the walk continues across
    /// ticks, and stopping and restarting it would bias where the points fall.
    pub x: f64,
    /// Where the game has got to.
    pub y: f64,
    /// Steps taken so far, counted against [`CHAOS_LIMIT`].
    pub steps: u64,
    /// Steps taken before points start counting.
    pub warm: u32,
    /// True when the cloud is full, or has given up trying to fill.
    pub done: bool,
    /// Fingerprint of the view this was gathered for.
    pub stamp: u64,
}

impl Cloud {
    /// Throw the cloud away and start the walk over.
    pub fn restart(&mut self, stamp: u64, idle: bool) {
        self.points.clear();
        self.x = 0.0;
        self.y = 0.0;
        self.steps = 0;
        self.warm = 0;
        self.done = idle;
        self.stamp = stamp;
    }

    /// How full it is, from 0 to 1.
    pub fn fullness(&self) -> f32 {
        self.points.len() as f32 / CLOUD_POINTS as f32
    }
}

// ---------------------------------------------------------------------------------------
// the plugin
// ---------------------------------------------------------------------------------------

/// Installs the viewer: the fractals, the view you look at them through, and the two
/// progressive renderers that fill it in.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut().insert_resource(Grid::default());
        app.world_mut().insert_resource(Court::default());
        app.world_mut().insert_resource(View::default());
        app.world_mut().insert_resource(Views::default());
        app.world_mut().insert_resource(Selection::default());
        app.world_mut().insert_resource(Depth::default());
        app.world_mut().insert_resource(Motion::default());
        app.world_mut().insert_resource(Field::default());
        app.world_mut().insert_resource(Cloud::default());
        app.add_systems(Startup, open);
        app.add_systems(
            FixedUpdate,
            (
                apply_resize,
                choose_fractal,
                navigate,
                tune_depth,
                hold_still,
                draw_field,
                draw_cloud,
                advance_motion,
            )
                .chain(),
        );
    }
}

/// Size the field to the grid before anything tries to read it.
pub fn open(grid: Res<Grid>, mut field: ResMut<Field>) {
    field.samples.resize(grid.cells(), Sample::default());
}

/// Take the window's new shape: a new court to draw into, and a new grid to draw with.
fn apply_resize(
    mut grid: ResMut<Grid>,
    mut court: ResMut<Court>,
    mut orders: EventReader<CommandEvent>,
) {
    for order in orders.read() {
        if order.name != RESIZE_COMMAND {
            continue;
        }
        let Some(window) = parse_window(&order.payload) else {
            continue;
        };
        let wanted = grid_for_window(window);
        if *grid != wanted {
            *grid = wanted;
        }
        court.0 = court_for_window(window);
    }
}

/// The number keys pick a fractal outright; tab walks along the row.
///
/// Leaving one puts its view away and coming back gets it out again, so that wandering off to
/// look at the fern does not cost you the corner of the Mandelbrot set you had found.
fn choose_fractal(
    input: Res<Input>,
    grid: Res<Grid>,
    mut selection: ResMut<Selection>,
    mut view: ResMut<View>,
    mut views: ResMut<Views>,
) {
    const KEYS: [Key; 10] = [
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
    ];
    let mut wanted = None;
    for (slot, key) in KEYS.iter().enumerate() {
        if input.just_pressed(*key) {
            wanted = Some(FRACTALS[slot]);
        }
    }
    if input.just_pressed(Key::Tab) {
        wanted = Some(FRACTALS[(selection.0.index() + 1) % FRACTALS.len()]);
    }
    let Some(wanted) = wanted.filter(|&kind| kind != selection.0) else {
        return;
    };
    views.0[selection.0.index()] = Some(*view);
    selection.0 = wanted;
    *view = views.0[wanted.index()].unwrap_or_else(|| wanted.home(*grid));
}

/// Pan, zoom, and go home. The wheel and the mouse buttons zoom about the pointer, so the
/// thing you are aiming at is the thing that stays put.
fn navigate(
    input: Res<Input>,
    grid: Res<Grid>,
    court: Res<Court>,
    selection: Res<Selection>,
    mut view: ResMut<View>,
    mut views: ResMut<Views>,
) {
    if input.just_pressed(Key::R) {
        *view = selection.0.home(*grid);
        // Forgotten rather than remembered, so that a window reshaped later re-frames it.
        views.0[selection.0.index()] = None;
        return;
    }

    let (wide, tall) = view.extents(*grid);
    if input.pressed(Key::Left) {
        view.center_x -= wide * PAN_STEP;
    }
    if input.pressed(Key::Right) {
        view.center_x += wide * PAN_STEP;
    }
    if input.pressed(Key::Down) {
        view.center_y -= tall * PAN_STEP;
    }
    if input.pressed(Key::Up) {
        view.center_y += tall * PAN_STEP;
    }

    // Keyed zoom holds the middle, which is the only anchor a keyboard can mean.
    let middle = (view.center_x, view.center_y);
    if input.pressed(Key::Z) {
        view.zoom_about(middle, 1.0 / ZOOM_STEP);
    }
    if input.pressed(Key::X) {
        view.zoom_about(middle, ZOOM_STEP);
    }

    let pointer = view.complex_of_world(*grid, court.0, input.mouse_world());
    let scroll = input.scroll_delta();
    if scroll != 0.0 {
        view.zoom_about(pointer, SCROLL_STEP.powf(-scroll as f64));
    }
    if input.mouse_just_pressed(MouseButton::Left) {
        view.zoom_about(pointer, CLICK_STEP);
    }
    if input.mouse_just_pressed(MouseButton::Right) {
        view.zoom_about(pointer, 1.0 / CLICK_STEP);
    }
}

/// More or fewer iterations. Held, it ramps proportionally, because the useful range spans two
/// orders of magnitude and a fixed step would be wrong at one end or the other.
fn tune_depth(input: Res<Input>, mut depth: ResMut<Depth>) {
    if input.pressed(Key::E) {
        let raised = (depth.0 as f32 * DEPTH_RAMP).ceil() as u32;
        depth.0 = raised.max(depth.0 + 1).min(DEPTH_MAX);
    }
    if input.pressed(Key::Q) {
        let lowered = (depth.0 as f32 / DEPTH_RAMP).floor() as u32;
        depth.0 = lowered.min(depth.0.saturating_sub(1)).max(DEPTH_MIN);
    }
}

/// Space holds everything that moves.
fn hold_still(input: Res<Input>, mut motion: ResMut<Motion>) {
    if input.just_pressed(Key::Space) {
        motion.running = !motion.running;
    }
}

/// Work out the escape-time picture, coarse pass first.
///
/// Each pass samples the middle cell of every block and fills the block with it, so the first
/// pass costs a sixty-fourth of the picture and shows the whole of it. Halving the block each
/// time costs about a third more than going straight to full resolution, which is a fair price
/// for never showing a half-drawn screen.
fn draw_field(
    selection: Res<Selection>,
    view: Res<View>,
    grid: Res<Grid>,
    depth: Res<Depth>,
    motion: Res<Motion>,
    mut field: ResMut<Field>,
) {
    let kind = selection.0;
    let wanted = stamp(&[
        kind.index() as u64,
        view.center_x.to_bits(),
        view.center_y.to_bits(),
        view.span.to_bits(),
        depth.0 as u64,
        grid.width as u64,
        grid.height as u64,
        motion.phase.to_bits(),
    ]);
    if field.stamp != wanted {
        field.restart(wanted, grid.cells(), kind.is_cloud());
    }
    if field.done {
        return;
    }

    let block = 1usize << field.level;
    let across = (grid.width as usize).div_ceil(block);
    let down = (grid.height as usize).div_ceil(block);
    let blocks = across * down;
    let mut spent = 0u64;

    while field.cursor < blocks {
        let (column, row) = (field.cursor % across, field.cursor / across);
        let (left, bottom) = (column * block, row * block);
        // The middle of the block, so a coarse pass is a fair shrink of the picture rather
        // than a corner-sampled one leaning up and to the left.
        let x = (left + block / 2).min(grid.width as usize - 1) as u32;
        let y = (bottom + block / 2).min(grid.height as usize - 1) as u32;
        let (real, imaginary) = view.complex_at(*grid, x, y);
        let sample = sample_at(kind, real, imaginary, depth.0, motion.phase);

        let right = (left + block).min(grid.width as usize);
        let top = (bottom + block).min(grid.height as usize);
        for fill_y in bottom..top {
            let start = fill_y * grid.width as usize;
            field.samples[start + left..start + right].fill(sample);
        }

        field.cursor += 1;
        spent += cost_of(sample, depth.0);
        if spent >= ITERATION_BUDGET {
            return;
        }
    }

    if field.level == 0 {
        field.done = true;
        field.finished += 1;
    } else {
        field.level -= 1;
        field.cursor = 0;
    }
}

/// Play the chaos game and keep the points that land on screen.
///
/// Keeping only what is on screen is what makes these fractals zoomable: the walk visits the
/// whole attractor regardless, and a narrow view simply takes longer to fill. [`CHAOS_LIMIT`]
/// is what stops a view containing none of it from trying forever.
fn draw_cloud(
    selection: Res<Selection>,
    view: Res<View>,
    grid: Res<Grid>,
    mut cloud: ResMut<Cloud>,
    mut rng: ResMut<SimRng>,
) {
    let kind = selection.0;
    let wanted = stamp(&[
        kind.index() as u64,
        view.center_x.to_bits(),
        view.center_y.to_bits(),
        view.span.to_bits(),
        grid.width as u64,
        grid.height as u64,
    ]);
    if cloud.stamp != wanted {
        cloud.restart(wanted, !kind.is_cloud());
    }
    let Some(spec) = kind.cloud() else {
        return;
    };
    if cloud.done {
        return;
    }

    let (home_x, home_y, home_wide, home_tall) = kind.home_frame();
    let total: f32 = spec.maps.iter().map(|map| map.weight).sum();

    for _ in 0..CHAOS_BUDGET {
        if cloud.points.len() >= CLOUD_POINTS || cloud.steps >= CHAOS_LIMIT {
            cloud.done = true;
            return;
        }
        cloud.steps += 1;

        let mut ticket = rng.range_f32(0.0..total);
        let mut chosen = spec.maps.len() - 1;
        for (slot, map) in spec.maps.iter().enumerate() {
            if ticket < map.weight {
                chosen = slot;
                break;
            }
            ticket -= map.weight;
        }
        let (x, y) = spec.maps[chosen].apply(cloud.x, cloud.y);
        cloud.x = x;
        cloud.y = y;
        if cloud.warm < CHAOS_WARMUP {
            cloud.warm += 1;
            continue;
        }

        let (placed_x, placed_y) = match spec.place {
            Some(place) => place.apply(x, y),
            None => (x, y),
        };
        for turn in 0..spec.symmetry {
            let (spun_x, spun_y) = turn_about_origin(placed_x, placed_y, turn, spec.symmetry);
            if !view.holds(*grid, spun_x, spun_y) {
                continue;
            }
            // Where this speck sits across the shape as a whole, not across the view: zooming
            // in should magnify the colors along with everything else, not repaint them.
            let tone = ((spun_x - home_x) / home_wide + (spun_y - home_y) / home_tall) as f32;
            cloud.points.push(Speck {
                x: spun_x as f32,
                y: spun_y as f32,
                band: chosen as u8,
                tone,
            });
        }
    }
}

/// Move the Julia constant along, once the picture it belongs to has finished.
///
/// Tying the step to a finished picture rather than to the clock is what keeps the drift
/// smooth on any machine: a slow one takes bigger steps less often, and neither ever shows a
/// picture that is half one constant and half the next.
fn advance_motion(selection: Res<Selection>, field: Res<Field>, mut motion: ResMut<Motion>) {
    if selection.0.is_moving() && motion.running && field.done {
        motion.phase = (motion.phase + JULIA_STEP).rem_euclid(TAU);
    }
}

/// Rotate a point by `turn` of `turns` full turns about the origin.
fn turn_about_origin(x: f64, y: f64, turn: u32, turns: u32) -> (f64, f64) {
    if turn == 0 {
        return (x, y);
    }
    let angle = TAU * turn as f64 / turns as f64;
    let (sin, cos) = angle.sin_cos();
    (x * cos - y * sin, x * sin + y * cos)
}

/// A fingerprint of everything a picture depends on, so that a renderer can notice in one
/// comparison that what it has is no longer what was asked for.
fn stamp(parts: &[u64]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for &part in parts {
        hash ^= part;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}
