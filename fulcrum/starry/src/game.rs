//! Starry Night as a fluid: the painting, and the current that keeps painting it.
//!
//! Four decisions carry the piece, and each one is doing work rather than decoration:
//!
//! **One scalar field is both the picture and the motion.** Everything in the sky comes from a
//! stream function [`field`]: the brushstrokes are carried along its contours, and the sky's
//! light and dark bands are read off its value. Van Gogh's sky is drawn as flow lines, so
//! making the flow lines the drawing is not a trick, it is the same statement twice. Move a
//! swirl and the bands move with it, because they were never separate things.
//!
//! **The current is the curl of that field, so paint is never destroyed.** Velocity is
//! `(dpsi/dy, -dpsi/dx)`, which is divergence-free: it has no sources and no sinks, so the
//! canvas cannot thin out in one place and pile up in another however long it runs. See
//! [`flow`].
//!
//! **The picture is a function of position, and the paint remembers it.** [`paint_at`] answers,
//! for any point, what belongs there: cypress, village, hill, halo, sky. Every stroke carries
//! its own colour and drifts slowly back toward what the point it is standing on ought to be
//! ([`HEAL_RATE`]). That single rule gives the piece its whole character: smear the sky with
//! the pointer and it flows, holds the smear for a moment, then finds its way back into the
//! painting. Add a star and the paint grows a halo into it, one stroke at a time.
//!
//! **Only the sky is free.** Strokes below the skyline are on springs to where they were laid
//! down ([`Layer::body`]), so the village stays a village and the cypress sways like a flame
//! instead of blowing away. The stream function is faded out at ground level as well, by
//! multiplying it by a mask, which keeps the current divergence-free while it dies: the curl
//! of anything is still divergence-free.
//!
//! Pure logic, no sprites, so it runs headless for the determinism test. The binary decides
//! what "cobalt" means.

use fulcrum::prelude::*;
use std::f32::consts::TAU;

/// The canvas, in world units. Five to four, the proportions of the painting.
pub const CANVAS: Vec2 = Vec2::new(1200.0, 960.0);

/// Strokes at startup. Enough to cover the canvas twice over: the picture is made of overlaps,
/// and a sky you can see between is a sky of confetti.
pub const START_STROKES: u32 = 7200;
/// The most the canvas will hold.
pub const MAX_STROKES: u32 = 26_000;
/// Strokes added or taken away per repeat while a density key is held.
pub const HOLD_BATCH: u32 = 120;
/// Ticks a held key must be down before it starts repeating.
pub const HOLD_DELAY: u32 = 10;
/// Ticks between arrivals once a held key is repeating.
pub const HOLD_PERIOD: u32 = 2;

/// How fast the current carries a stroke where the field is steepest, in units per second.
pub const FLOW_SPEED: f32 = 52.0;
/// How quickly a stroke's velocity converges on the current, per second. Low values give the
/// drift its lag and its weight; high values make strokes snap to the field and look rigid.
pub const FLOW_GRIP: f32 = 3.0;
/// How fast a stroke turns to line up with where it is going, per second.
pub const TURN_RATE: f32 = 3.5;

/// How quickly paint finds its way back to the picture, per second. The whole feel of the
/// piece is in this number: too fast and a smear snaps back like elastic, too slow and the
/// painting never recovers from being touched.
pub const HEAL_RATE: f32 = 0.55;

/// How far a stroke's tone may sit from what the picture asks for.
///
/// Without this every stroke in a patch is the same colour and the patch is a wash. Van Gogh
/// laid strokes of noticeably different blues side by side, and the weave that makes is most of
/// what the eye reads as paint. Deliberately large.
pub const TONE_SPREAD: f32 = 0.26;

/// Shortest and longest a stroke lasts before it is laid down again somewhere else, in
/// seconds. Long, and staggered, so the canvas renews itself without ever flickering.
pub const STROKE_LIFE: (f32, f32) = (14.0, 34.0);
/// Fraction of its life a stroke spends fading in, and again fading out.
pub const STROKE_FADE: f32 = 0.12;

/// How far the pointer's smear reaches, in units.
pub const SMEAR_RADIUS: f32 = 190.0;
/// How hard a moving pointer pushes the paint along.
pub const SMEAR_PUSH: f32 = 420.0;
/// How much of the push comes back as a turn around the pointer, which is what makes a drag
/// feel like a palette knife rather than a fan.
pub const SMEAR_CURL: f32 = 0.45;
/// How quickly the smear fades once the pointer stops, per second.
pub const SMEAR_DECAY: f32 = 2.2;
/// Pointer travel per second that counts as a full-strength smear.
pub const SMEAR_FULL_SPEED: f32 = 700.0;

/// Slowest the piece can run, as a multiple of real time.
pub const SPEED_MIN: f32 = 0.1;
/// Fastest it can run.
pub const SPEED_MAX: f32 = 4.0;
/// How much a held speed key multiplies the rate each tick.
pub const SPEED_RAMP: f32 = 1.02;

/// The most stars the sky will hold, the painting's own included.
pub const MAX_STARS: usize = 26;

/// Height above which the sky flows freely, and below which it is held still, as a fraction of
/// the canvas from its middle.
pub const CALM_BELOW: f32 = -0.20;
/// See [`CALM_BELOW`].
pub const CALM_ABOVE: f32 = -0.02;

/// The great double swirl: centre, spread and strength, in canvas fractions. The pair is the
/// one thing everybody remembers about the painting, and it is two vortices turning against
/// each other.
pub const SWIRLS: [(f32, f32, f32, f32); 2] =
    [(-0.115, 0.165, 0.135, 0.255), (0.075, 0.105, 0.092, -0.175)];

/// The rest of the sky's motion: `(cycles across, cycles up, weight, drift)`. Whole cycle
/// counts make the field meet itself at the edges of the canvas instead of ending.
pub const WAVES: [(f32, f32, f32, f32); 4] = [
    (1.0, 0.0, 0.085, 0.055),
    (1.0, 1.0, 0.055, -0.037),
    (2.0, 1.0, 0.030, 0.071),
    (2.0, 2.0, 0.017, -0.089),
];

/// How many light-and-dark bands the sky's stream function is read as.
pub const BANDS: f32 = 7.0;

/// The eleven stars, in canvas fractions: `(across, up, radius, which way it turns)`.
pub const STARS: [(f32, f32, f32, f32); 11] = [
    (-0.325, 0.315, 0.086, 1.0),
    (-0.440, 0.130, 0.054, -1.0),
    (-0.200, 0.415, 0.050, 1.0),
    (-0.055, 0.335, 0.044, -1.0),
    (0.100, 0.425, 0.055, 1.0),
    (0.245, 0.300, 0.049, -1.0),
    (0.155, 0.145, 0.042, 1.0),
    (-0.250, 0.030, 0.038, -1.0),
    (0.325, 0.115, 0.045, 1.0),
    (-0.415, 0.440, 0.040, -1.0),
    (0.440, -0.020, 0.034, 1.0),
];

/// The moon: centre and radius, in canvas fractions.
pub const MOON: (f32, f32, f32) = (0.355, 0.330, 0.072);
/// How far the moon's glow reaches, as a multiple of its radius.
pub const MOON_HALO: f32 = 2.15;
/// How much of the moon is bitten out, and by how far the bite is offset.
pub const MOON_BITE: (f32, f32) = (0.86, 0.44);

/// The cypress: where it stands and how wide it is at the base, in canvas fractions. It runs
/// off the top of the canvas, as it does off the top of the painting.
pub const CYPRESS: (f32, f32) = (-0.395, 0.100);
/// The little cypress beside it.
pub const CYPRESS_SMALL: (f32, f32) = (-0.472, 0.052);

/// Where the fields in front of the village give way, as a fraction of the canvas.
pub const FIELD_LINE: f32 = -0.345;
/// The village: leftmost and rightmost edge, in canvas fractions.
pub const VILLAGE: (f32, f32) = (-0.315, 0.455);
/// Where the church spire stands, and how high it reaches.
pub const SPIRE: (f32, f32) = (-0.022, 0.045);

/// What a stroke is painting.
///
/// Not a colour: the simulation never picks one. A layer and a tone say what belongs at a
/// point, and the binary decides what the village and the sky actually look like, so a change
/// of palette repaints everything without the painting knowing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layer {
    /// The turning sky.
    Sky,
    /// The rings of light around a star or the moon.
    Halo,
    /// The white middle of a star.
    Star,
    /// The moon itself.
    Moon,
    /// The hills behind the village.
    Hill,
    /// The fields in front of it.
    Ground,
    /// The village and its church.
    Village,
    /// A lit window.
    Window,
    /// The cypress.
    Cypress,
}

impl Layer {
    /// How this layer behaves: how strongly a stroke is held to where it was laid down, and how
    /// much of the current it feels.
    ///
    /// This one pair of numbers is the difference between a sky and a village. Sky strokes are
    /// free and travel; everything below the skyline is on a spring, so the shapes stay put
    /// while the same current combs through them.
    pub fn body(self) -> (f32, f32) {
        match self {
            Layer::Sky | Layer::Halo | Layer::Star | Layer::Moon => (0.0, 1.0),
            // Loose enough to sway like the flame it is drawn as, tight enough to stay a tree.
            Layer::Cypress => (5.0, 0.42),
            Layer::Hill => (7.0, 0.16),
            Layer::Ground => (8.0, 0.12),
            Layer::Village | Layer::Window => (16.0, 0.05),
        }
    }

    /// How long and how wide a stroke of this layer is, in world units.
    pub fn brush(self) -> (f32, f32) {
        match self {
            Layer::Sky => (42.0, 8.5),
            Layer::Halo => (17.0, 7.5),
            Layer::Star | Layer::Moon => (18.0, 11.0),
            Layer::Hill => (30.0, 9.0),
            Layer::Ground => (34.0, 9.0),
            Layer::Village => (17.0, 8.0),
            Layer::Window => (7.0, 5.0),
            Layer::Cypress => (30.0, 7.0),
        }
    }

    /// Whether this layer is up in the sky, where the current is free to take it.
    pub fn airborne(self) -> bool {
        matches!(self, Layer::Sky | Layer::Halo | Layer::Star | Layer::Moon)
    }
}

/// One brushstroke of paint.
#[derive(Component)]
pub struct Stroke {
    /// What it is painting.
    pub layer: Layer,
    /// Where it sits in that layer's range of colour, `0..1`.
    pub tone: f32,
    /// How far this stroke sits from the tone the picture asks for, and keeps sitting: its own
    /// mind about the colour, held through every healing.
    pub weave: f32,
    /// Where it was laid down, for the layers that are held to it.
    pub anchor: Vec2,
    /// Which way it is pointing, in radians. Its own state, so it turns rather than snapping.
    pub angle: f32,
    /// Seconds it has been on the canvas.
    pub age: f32,
    /// Seconds before it is laid down again somewhere else.
    pub life: f32,
    /// Length and width in world units.
    pub size: Vec2,
    /// A fixed number in `0..1`, for the small differences between one stroke and the next.
    pub seed: f32,
}

impl Stroke {
    /// How solid a stroke is, `0..1`. Paint arrives and leaves gradually, so the canvas renews
    /// itself with nothing ever seen to appear.
    pub fn presence(&self) -> f32 {
        let fraction = (self.age / self.life.max(1e-3)).clamp(0.0, 1.0);
        let rising = (fraction / STROKE_FADE).clamp(0.0, 1.0);
        let falling = ((1.0 - fraction) / STROKE_FADE).clamp(0.0, 1.0);
        let ease = |t: f32| t * t * (3.0 - 2.0 * t);
        ease(rising) * ease(falling)
    }
}

/// Simulation velocity, units per second.
#[derive(Component)]
pub struct Velocity(pub Vec2);

/// One light in the sky, and the small vortex that turns its halo.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Starlight {
    /// Where it hangs, in world units.
    pub at: Vec2,
    /// How far its rings reach, in world units.
    pub radius: f32,
    /// Which way it turns, and how strongly.
    pub spin: f32,
}

/// Everything hanging in the sky. Changed by clicking, which is why it is a resource rather
/// than a table.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct Sky {
    /// The stars, the painting's own first and any you have added after them.
    pub stars: Vec<Starlight>,
}

impl Default for Sky {
    fn default() -> Self {
        Self {
            stars: STARS
                .iter()
                .map(|&(u, v, radius, spin)| Starlight {
                    at: canvas_point(u, v),
                    radius: radius * CANVAS.y,
                    spin,
                })
                .collect(),
        }
    }
}

/// Total seconds painted, which is also the clock the sky turns on.
#[derive(Resource, Default, Clone, Copy, PartialEq, Debug)]
pub struct Elapsed(pub f32);

/// Whether the paint is finding its way back to the picture.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Healing(pub bool);

impl Default for Healing {
    fn default() -> Self {
        Self(true)
    }
}

/// The pointer's smear: where it is, which way it is pushing, and how much of it is left.
#[derive(Resource, Default, Clone, Copy, PartialEq, Debug)]
pub struct Smear {
    /// Where the push is centred.
    pub at: Vec2,
    /// Which way it pushes, as a unit vector.
    pub push: Vec2,
    /// How strong it is, `0..1`, decaying once the pointer stops.
    pub strength: f32,
    /// Where the pointer was last tick.
    pub was: Vec2,
    /// Whether a pointer position has been seen yet, so the first frame is not a jump from the
    /// origin.
    pub started: bool,
}

/// How much paint is on the canvas.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Census(pub u32);

/// Nothing moves while this is set, and the piece is a painting again.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Paused(pub bool);

/// How fast the piece runs, as a multiple of real time.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct Speed(pub f32);

impl Default for Speed {
    fn default() -> Self {
        Self(1.0)
    }
}

/// How far this tick advances. Written once per tick by [`set_step`].
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct Step {
    /// Seconds this tick, already scaled by speed. Zero while still.
    pub seconds: f32,
    /// The speed multiplier, or zero while still.
    pub scale: f32,
}

/// How long each held key has been down, for key repeat.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct Holds {
    /// Ticks the add key has been down.
    pub more: u32,
    /// Ticks the remove key has been down.
    pub fewer: u32,
}

/// A point given in canvas fractions, in world units.
pub fn canvas_point(across: f32, up: f32) -> Vec2 {
    vec2(across * CANVAS.x, up * CANVAS.y)
}

/// Smoothstep between two edges.
fn ramp(low: f32, high: f32, at: f32) -> f32 {
    let t = ((at - low) / (high - low)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How free the current is at this height, `0` on the ground and `1` in the sky.
///
/// Applied to the stream function rather than to the velocity, so the current can die away
/// without the flow gaining sources and sinks where it fades. Returns the mask and its slope.
fn calm(up: f32) -> (f32, f32) {
    let mask = ramp(CALM_BELOW, CALM_ABOVE, up);
    let span = CALM_ABOVE - CALM_BELOW;
    let t = ((up - CALM_BELOW) / span).clamp(0.0, 1.0);
    let slope = if t <= 0.0 || t >= 1.0 {
        0.0
    } else {
        6.0 * t * (1.0 - t) / span
    };
    (mask, slope)
}

/// The stream function at a point, and its slope, in canvas fractions.
///
/// The sky is the sum of the great double swirl, a few long waves, and one small vortex per
/// star. Reading it two ways, as a height for colour and as a slope for motion, is what keeps
/// the bands and the current from ever disagreeing.
pub fn field(point: Vec2, sky: &Sky, time: f32) -> (f32, Vec2) {
    let here = vec2(point.x / CANVAS.x, point.y / CANVAS.y);
    let mut psi = 0.0;
    let mut slope = Vec2::ZERO;

    for (across, up, spread, strength) in SWIRLS {
        // The swirls breathe: a vortex of fixed strength turns the same picture forever.
        let breathing = strength * (1.0 + 0.16 * (time * 0.07 + across * 9.0).sin());
        let offset = here - vec2(across, up);
        let falloff = (-offset.length_squared() / (2.0 * spread * spread)).exp();
        let value = breathing * falloff;
        psi += value;
        slope -= offset * (value / (spread * spread));
    }

    for (across, up, weight, drift) in WAVES {
        let wave = vec2(TAU * across, TAU * up);
        let phase = wave.dot(here) + drift * time;
        psi += weight * phase.sin();
        slope += wave * (weight * phase.cos());
    }

    for star in &sky.stars {
        let offset = here - vec2(star.at.x / CANVAS.x, star.at.y / CANVAS.y);
        let spread = (star.radius / CANVAS.y) * 0.8;
        // Cheap rejection first: most strokes are nowhere near most stars, and the exponential
        // is the only expensive thing in here. Far enough out that what is dropped is four
        // parts in a hundred thousand: a nearer cutoff puts a step in the field, and a step in
        // the field is a source of paint sitting on a circle around every star.
        let reach = spread * 4.5;
        if offset.length_squared() > reach * reach {
            continue;
        }
        let strength = star.spin * 0.055;
        let falloff = (-offset.length_squared() / (2.0 * spread * spread)).exp();
        let value = strength * falloff;
        psi += value;
        slope -= offset * (value / (spread * spread));
    }

    let (mask, mask_slope) = calm(here.y);
    (
        psi * mask,
        vec2(slope.x * mask, slope.y * mask + psi * mask_slope),
    )
}

/// The current at a point, in units per second.
///
/// The curl of [`field`], scaled so that it stays divergence-free on a canvas that is wider
/// than it is tall: paint is carried around forever and never gathers anywhere.
pub fn flow(point: Vec2, sky: &Sky, time: f32) -> Vec2 {
    let (_, slope) = field(point, sky, time);
    vec2(slope.y, -slope.x * (CANVAS.y / CANVAS.x)) * FLOW_SPEED
}

/// The line of the hills, in canvas fractions.
pub fn ridge(across: f32) -> f32 {
    -0.185
        + 0.050 * across
        + 0.062 * (3.4 * across + 0.5).sin()
        + 0.034 * (5.2 * across + 2.2).sin()
        + 0.014 * (11.3 * across).sin()
}

/// The roofline of the village, in canvas fractions, or `None` where there is no village.
pub fn roofline(across: f32) -> Option<f32> {
    if across < VILLAGE.0 || across > VILLAGE.1 {
        return None;
    }
    let along = (across - VILLAGE.0) / (VILLAGE.1 - VILLAGE.0);
    // Fourteen houses of uneven height, from a repeating sum that never quite repeats.
    let house = (along * 21.0).floor();
    let lot = (house * 12.9898).sin() * 43758.547;
    let draw = lot - lot.floor();
    // One lot in four is a gap, so the village is a cluster of roofs rather than a wall.
    if draw < 0.26 {
        return None;
    }
    Some(-0.300 + 0.062 * draw)
}

/// Half the width of the cypress at this height, in canvas fractions. Zero above its tip.
fn cypress_width(base: f32, across: f32, up: f32) -> f32 {
    let bottom = -0.52;
    let top = 0.56;
    if up < bottom || up > top {
        return 0.0;
    }
    let along = (up - bottom) / (top - bottom);
    // Bulbous low down and drawn out to a point, with a flame's wobble down both edges.
    let taper = (1.0 - along).powf(0.92) * (0.30 + 0.70 * (along * 3.0).min(1.0));
    let wobble = 1.0 + 0.30 * (up * 26.0 + across * 7.0).sin() + 0.16 * (up * 47.0).sin();
    base * taper * wobble
}

/// Whether a point is inside the cypress, and how far into it.
fn in_cypress(here: Vec2) -> Option<f32> {
    for (across, base) in [CYPRESS, CYPRESS_SMALL] {
        let width = cypress_width(base, here.x, here.y);
        if width > 0.0 {
            let lean = across + 0.035 * (here.y * 1.9).sin();
            let into = (here.x - lean).abs() / width;
            if into < 1.0 {
                return Some(into);
            }
        }
    }
    None
}

/// Whether a point is inside the church spire.
fn in_spire(here: Vec2) -> bool {
    let base = -0.30;
    if here.y < base || here.y > SPIRE.1 {
        return false;
    }
    let along = (here.y - base) / (SPIRE.1 - base);
    // A thin needle over a squat body: the church is the one thing in the village that reaches
    // into the sky, and it is what stops the town reading as a row of boxes.
    let width = if along < 0.34 {
        0.026
    } else {
        0.013 * (1.0 - along).powf(0.55)
    };
    (here.x - SPIRE.0).abs() < width
}

/// The moon at a point: its crescent, or one of its rings, or nothing.
fn moonlight(point: Vec2) -> Option<(Layer, f32)> {
    let moon = canvas_point(MOON.0, MOON.1);
    let radius = MOON.2 * CANVAS.y;
    let from_moon = (point - moon).length();
    if from_moon < radius {
        let bite = (point - moon - vec2(1.0, 0.75).normalize() * radius * MOON_BITE.1).length();
        return Some(if bite > radius * MOON_BITE.0 {
            (Layer::Moon, 0.7 + 0.3 * (1.0 - from_moon / radius))
        } else {
            // The bitten-out part is night, not glow. Filling it with the halo's yellow is
            // what turns a crescent into a lamp, and a lamp is not what he painted.
            (Layer::Sky, 0.05)
        });
    }
    if from_moon >= radius * MOON_HALO {
        return None;
    }
    let out = (from_moon - radius) / (radius * (MOON_HALO - 1.0));
    let rings = 0.5 + 0.5 * (out * TAU * 2.0).cos();
    Some(if rings > 0.62 {
        (
            Layer::Halo,
            ((1.0 - out) * 0.5 + 0.5 * rings).clamp(0.0, 1.0),
        )
    } else {
        // Deep sky between the rings. Letting the ordinary sky through here is not enough:
        // near the horizon the ordinary sky is pale, and pale beside yellow is one mass.
        (Layer::Sky, 0.06 + 0.10 * rings)
    })
}

/// A star at a point: its white middle, or one of its rings, or nothing.
fn starlight(point: Vec2, star: &Starlight) -> Option<(Layer, f32)> {
    let from_star = (point - star.at).length();
    if from_star > star.radius {
        return None;
    }
    if from_star < star.radius * 0.30 {
        return Some((Layer::Star, 1.0 - 0.3 * (from_star / star.radius)));
    }
    let out = (from_star - star.radius * 0.30) / (star.radius * 0.70);
    let rings = 0.5 + 0.5 * (out * TAU * 1.75).cos();
    Some(if rings > 0.55 {
        (
            Layer::Halo,
            ((1.0 - out) * 0.45 + 0.55 * rings).clamp(0.0, 1.0),
        )
    } else {
        (Layer::Sky, 0.06 + 0.10 * rings)
    })
}

/// What belongs at a point: which layer, and where in that layer's range it sits.
///
/// The whole picture is this one function. Nothing else knows what Starry Night looks like,
/// which is what lets the paint find its way home from anywhere.
pub fn paint_at(point: Vec2, sky: &Sky, time: f32) -> (Layer, f32) {
    let here = vec2(point.x / CANVAS.x, point.y / CANVAS.y);

    // The cypress stands in front of everything, including the frame.
    if let Some(into) = in_cypress(here) {
        let flame = 0.5 + 0.5 * (here.y * 34.0 + here.x * 12.0 + into * 3.0).sin();
        // Nearly black, with the flame licks that catch the light along its edges.
        return (
            Layer::Cypress,
            (0.05 + 0.34 * flame * (0.35 + 0.65 * into)).min(1.0),
        );
    }

    // The village, and the spire that stands up into the sky.
    let roof = roofline(here.x);
    let in_village =
        in_spire(here) || roof.is_some_and(|roof| here.y <= roof && here.y > FIELD_LINE - 0.005);
    if in_village {
        // The spire is one dark shape from footing to point, not a wall with a roof on it.
        if in_spire(here) {
            return (Layer::Village, 0.03);
        }
        let block = ((here.x * 62.0).sin() * 43758.547).fract().abs();
        let eaves = roof.map_or(-0.30, |roof| roof - 0.034);
        if here.y > eaves {
            // Roofs, dark against the hills behind them.
            return (Layer::Village, 0.02 + 0.12 * block);
        }
        // Windows: a coarse grid, thinned right out, and never in a roof.
        let across = (here.x * 150.0).rem_euclid(1.0);
        let up = (here.y * 120.0).rem_euclid(1.0);
        if across < 0.17 && up < 0.20 && block > 0.45 {
            return (Layer::Window, 0.55 + 0.45 * block);
        }
        return (Layer::Village, 0.34 + 0.5 * block);
    }

    if here.y <= ridge(here.x) {
        if here.y <= FIELD_LINE {
            // Long furrows drawn across the fields, the way the foreground is combed.
            let furrow = 0.5 + 0.5 * (here.y * 42.0 + here.x * 5.0 + (here.x * 9.0).sin()).sin();
            return (
                Layer::Ground,
                (0.08 + 0.16 * furrow.powf(2.0) + 0.30 * furrow).min(1.0),
            );
        }
        // Contours parallel to the skyline rather than a flat wash: the hills are drawn as
        // bands following their own silhouette, which is the same idea as the sky's ribbons and
        // the only thing that makes a dark mass read as painted rather than as a hole.
        let below = ridge(here.x) - here.y;
        let contour = 0.5 + 0.5 * (below * 46.0 + here.x * 2.5).sin();
        let up = ramp(-0.36, ridge(here.x), here.y);
        return (
            Layer::Hill,
            (0.06 + 0.30 * up + 0.44 * contour.powf(1.7)).min(1.0),
        );
    }

    // The moon and the stars. Each gives back nothing between its rings, so the sky underneath
    // shows through: a halo painted as a solid disc is a lamp, and a halo painted as rings with
    // night between them is what Van Gogh actually did.
    if let Some(found) = moonlight(point) {
        return found;
    }
    for star in &sky.stars {
        if let Some(found) = starlight(point, star) {
            return found;
        }
    }

    // Open sky: the light and dark bands are the stream function, read as height.
    let (psi, _) = field(point, sky, time);
    // Raised to a power so the light bands are ribbons through a deep sky rather than half of
    // it. The painting is mostly dark and reads as light, because the little of it that is
    // light is very light.
    let bands = (0.5 + 0.5 * (psi * BANDS * TAU).sin()).powf(3.6);
    // Lighter toward the horizon, the way the paint is.
    let depth = ramp(0.5, -0.12, here.y);
    (Layer::Sky, (0.04 + 0.60 * bands + 0.14 * depth).min(1.0))
}

/// Which way a stroke of this layer wants to lie, when the current is not telling it.
pub fn lie_of_the_land(layer: Layer, point: Vec2) -> f32 {
    let here = vec2(point.x / CANVAS.x, point.y / CANVAS.y);
    match layer {
        // Up the tree, curling: the cypress is painted as a flame, not as a trunk.
        Layer::Cypress => std::f32::consts::FRAC_PI_2 + 0.55 * (here.y * 9.0 + here.x * 4.0).sin(),
        // Along the hills: the slope of the skyline they are following.
        Layer::Hill => {
            let step = 0.01;
            ((ridge(here.x + step) - ridge(here.x - step)) / (2.0 * step) * (CANVAS.y / CANVAS.x))
                .atan()
        }
        // Along the furrows.
        Layer::Ground => 0.12 * (here.x * 9.0).cos(),
        // Along the walls.
        Layer::Village | Layer::Window => 0.0,
        _ => 0.0,
    }
}

/// Installs the painting.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut().insert_resource(Sky::default());
        app.world_mut().insert_resource(Elapsed::default());
        app.world_mut().insert_resource(Healing::default());
        app.world_mut().insert_resource(Smear::default());
        app.world_mut().insert_resource(Census::default());
        app.world_mut().insert_resource(Paused::default());
        app.world_mut().insert_resource(Speed::default());
        app.world_mut().insert_resource(Step::default());
        app.world_mut().insert_resource(Holds::default());
        app.add_systems(Startup, prime_canvas);
        app.add_systems(
            FixedUpdate,
            (
                pace,
                set_step,
                advance_clock,
                smear_from_pointer,
                hang_stars,
                density_controls,
                repaint,
                move_strokes,
            )
                .chain(),
        );
    }
}

/// Lay the first coat. Public so the binary can order its sprite-hanging after it.
pub fn prime_canvas(
    mut commands: Commands,
    mut rng: ResMut<SimRng>,
    mut census: ResMut<Census>,
    sky: Res<Sky>,
) {
    add_strokes(
        &mut commands,
        &mut rng,
        &mut census,
        &sky,
        START_STROKES,
        0.0,
    );
}

/// Where a stroke of this layer should point, given where it is and how fast it is going.
fn aim(layer: Layer, point: Vec2, velocity: Vec2) -> f32 {
    if layer.airborne() || velocity.length_squared() > 90.0 {
        velocity.to_angle()
    } else {
        lie_of_the_land(layer, point)
    }
}

/// Lay `count` strokes at random points, at random moments in their lives so the canvas does
/// not renew itself all at once.
fn add_strokes(
    commands: &mut Commands,
    rng: &mut SimRng,
    census: &mut Census,
    sky: &Sky,
    count: u32,
    time: f32,
) {
    let half = CANVAS / 2.0;
    for _ in 0..count.min(MAX_STROKES.saturating_sub(census.0)) {
        let at = vec2(
            rng.range_f32(-half.x..half.x),
            rng.range_f32(-half.y..half.y),
        );
        let (layer, tone) = paint_at(at, sky, time);
        let (length, width) = layer.brush();
        let life = rng.range_f32(STROKE_LIFE.0..STROKE_LIFE.1);
        let weave = rng.range_f32(-0.5..0.5) * TONE_SPREAD;
        let velocity = if layer.airborne() {
            flow(at, sky, time)
        } else {
            Vec2::ZERO
        };
        commands.spawn((
            Stroke {
                layer,
                tone: (tone + weave).clamp(0.0, 1.0),
                weave,
                anchor: at,
                angle: aim(layer, at, velocity),
                age: rng.range_f32(0.0..life),
                life,
                size: vec2(
                    length * rng.range_f32(0.78..1.22),
                    width * rng.range_f32(0.8..1.2),
                ),
                seed: rng.unit_f32(),
            },
            Transform2D::from_translation(at),
            Velocity(velocity),
        ));
        census.0 += 1;
    }
}

/// Lay a stroke down again somewhere else, as fresh paint.
fn relay(
    stroke: &mut Stroke,
    transform: &mut Transform2D,
    velocity: &mut Velocity,
    rng: &mut SimRng,
    sky: &Sky,
    time: f32,
) {
    let half = CANVAS / 2.0;
    let at = vec2(
        rng.range_f32(-half.x..half.x),
        rng.range_f32(-half.y..half.y),
    );
    let (layer, tone) = paint_at(at, sky, time);
    let (length, width) = layer.brush();
    stroke.layer = layer;
    stroke.weave = rng.range_f32(-0.5..0.5) * TONE_SPREAD;
    stroke.tone = (tone + stroke.weave).clamp(0.0, 1.0);
    stroke.anchor = at;
    stroke.age = 0.0;
    stroke.life = rng.range_f32(STROKE_LIFE.0..STROKE_LIFE.1);
    stroke.size = vec2(
        length * rng.range_f32(0.78..1.22),
        width * rng.range_f32(0.8..1.2),
    );
    stroke.seed = rng.unit_f32();
    velocity.0 = if layer.airborne() {
        flow(at, sky, time)
    } else {
        Vec2::ZERO
    };
    stroke.angle = aim(layer, at, velocity.0);
    transform.translation = at;
    transform.rotation = stroke.angle;
}

/// Stillness, pace, and whether the paint is finding its way home.
fn pace(
    mut paused: ResMut<Paused>,
    mut speed: ResMut<Speed>,
    mut healing: ResMut<Healing>,
    input: Res<Input>,
) {
    if input.just_pressed(Key::Space) {
        paused.0 = !paused.0;
    }
    if input.just_pressed(Key::H) {
        healing.0 = !healing.0;
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

/// Fix this tick's step. Stillness is a step of zero.
fn set_step(mut step: ResMut<Step>, time: Res<Time>, speed: Res<Speed>, paused: Res<Paused>) {
    step.scale = if paused.0 { 0.0 } else { speed.0 };
    step.seconds = time.fixed_delta * step.scale;
}

/// Advance the clock the sky turns on.
fn advance_clock(mut elapsed: ResMut<Elapsed>, step: Res<Step>) {
    elapsed.0 += step.seconds;
}

/// Turn pointer movement into a push through the wet paint.
///
/// Strength comes from how fast the pointer is travelling rather than merely from where it is,
/// so a resting hand leaves the canvas alone and a moving one drags paint with it. The push
/// then fades on its own instead of stopping when the hand does.
fn smear_from_pointer(mut smear: ResMut<Smear>, input: Res<Input>, step: Res<Step>) {
    if step.seconds <= 0.0 {
        return;
    }
    let pointer = input.mouse_world();
    if !smear.started {
        smear.was = pointer;
        smear.started = true;
    }
    let travel = pointer - smear.was;
    smear.was = pointer;
    smear.at = pointer;
    let speed = travel.length() / step.seconds;
    let fresh = (speed / SMEAR_FULL_SPEED).clamp(0.0, 1.0);
    let faded = smear.strength * (-SMEAR_DECAY * step.seconds).exp();
    if fresh >= faded && travel.length_squared() > 1e-6 {
        smear.push = travel.normalize();
        smear.strength = fresh;
    } else {
        smear.strength = faded;
    }
}

/// Click the sky to hang a new star in it, X to take the last one down again.
///
/// The star is only added to the sky; no paint is moved. The halo appears because the picture
/// has changed underneath the strokes and they find their way to it, which takes a second or
/// two and looks like the painting growing one.
fn hang_stars(mut sky: ResMut<Sky>, mut rng: ResMut<SimRng>, input: Res<Input>, step: Res<Step>) {
    if step.seconds <= 0.0 {
        return;
    }
    if input.just_pressed(Key::X) {
        sky.stars.pop();
    }
    if !input.mouse_just_pressed(MouseButton::Left) {
        return;
    }
    let at = input.mouse_world();
    let half = CANVAS / 2.0;
    if at.x.abs() > half.x || at.y.abs() > half.y || sky.stars.len() >= MAX_STARS {
        return;
    }
    // Only in open sky: a star behind the village would be a hole in the village.
    let here = vec2(at.x / CANVAS.x, at.y / CANVAS.y);
    if here.y <= ridge(here.x) + 0.02 || in_cypress(here).is_some() {
        return;
    }
    let radius = rng.range_f32(0.036..0.075) * CANVAS.y;
    let spin = if rng.chance(0.5) { 1.0 } else { -1.0 };
    sky.stars.push(Starlight { at, radius, spin });
}

/// Key repeat: fires the tick a key goes down, then again every [`HOLD_PERIOD`] ticks once it
/// has been held for [`HOLD_DELAY`].
fn repeating(held: &mut u32, down: bool) -> bool {
    if !down {
        *held = 0;
        return false;
    }
    let fire =
        *held == 0 || (*held >= HOLD_DELAY && (*held - HOLD_DELAY).is_multiple_of(HOLD_PERIOD));
    *held += 1;
    fire
}

/// Hold N for more paint, M for less.
#[expect(
    clippy::too_many_arguments,
    reason = "adding paint needs the canvas, the picture and the clock"
)]
fn density_controls(
    mut commands: Commands,
    mut rng: ResMut<SimRng>,
    mut census: ResMut<Census>,
    mut holds: ResMut<Holds>,
    sky: Res<Sky>,
    elapsed: Res<Elapsed>,
    input: Res<Input>,
    strokes: Query<Entity, With<Stroke>>,
) {
    if repeating(&mut holds.more, input.pressed(Key::N)) {
        add_strokes(
            &mut commands,
            &mut rng,
            &mut census,
            &sky,
            HOLD_BATCH,
            elapsed.0,
        );
    }
    if repeating(&mut holds.fewer, input.pressed(Key::M)) {
        for entity in strokes.iter().take(HOLD_BATCH as usize) {
            commands.entity(entity).despawn();
            census.0 = census.0.saturating_sub(1);
        }
    }
}

/// R lays the whole canvas down again, without disturbing the sky it is painting.
fn repaint(
    mut strokes: Query<(&mut Stroke, &mut Transform2D, &mut Velocity)>,
    mut rng: ResMut<SimRng>,
    sky: Res<Sky>,
    elapsed: Res<Elapsed>,
    input: Res<Input>,
) {
    if !input.just_pressed(Key::R) {
        return;
    }
    for (mut stroke, mut transform, mut velocity) in &mut strokes {
        relay(
            &mut stroke,
            &mut transform,
            &mut velocity,
            &mut rng,
            &sky,
            elapsed.0,
        );
    }
}

/// Everything a stroke of paint does: drift, turn, find its colour, and be laid down again.
fn move_strokes(
    mut strokes: Query<(&mut Stroke, &mut Transform2D, &mut Velocity)>,
    mut rng: ResMut<SimRng>,
    sky: Res<Sky>,
    elapsed: Res<Elapsed>,
    healing: Res<Healing>,
    smear: Res<Smear>,
    step: Res<Step>,
) {
    if step.seconds <= 0.0 {
        return;
    }
    let dt = step.seconds;
    let time = elapsed.0;
    let half = CANVAS / 2.0;
    // Exponential approach, so the lag is the same at any tick rate or speed setting.
    let grip = 1.0 - (-FLOW_GRIP * dt).exp();
    let turn = 1.0 - (-TURN_RATE * dt).exp();
    let heal = 1.0 - (-HEAL_RATE * dt).exp();

    for (mut stroke, mut transform, mut velocity) in &mut strokes {
        stroke.age += dt;
        if stroke.age >= stroke.life {
            relay(
                &mut stroke,
                &mut transform,
                &mut velocity,
                &mut rng,
                &sky,
                time,
            );
            continue;
        }

        let at = transform.translation;
        let (spring, drift) = stroke.layer.body();
        let mut target = flow(at, &sky, time) * drift;

        // The pointer's push, mostly along the drag with a turn around it: paint dragged by a
        // knife goes with the blade and rolls at its edges.
        if smear.strength > 0.001 {
            let offset = at - smear.at;
            let distance = offset.length();
            if distance < SMEAR_RADIUS {
                let falloff = 1.0 - distance / SMEAR_RADIUS;
                let curl = vec2(-smear.push.y, smear.push.x);
                target += (smear.push + curl * SMEAR_CURL)
                    * SMEAR_PUSH
                    * smear.strength
                    * falloff
                    * falloff;
            }
        }

        // Everything below the skyline is on a spring to where it was laid down.
        if spring > 0.0 {
            target += (stroke.anchor - at) * spring;
        }

        let carried = velocity.0;
        velocity.0 += (target - carried) * grip;
        let next = at + velocity.0 * dt;

        if next.x.abs() > half.x + 40.0 || next.y.abs() > half.y + 40.0 {
            relay(
                &mut stroke,
                &mut transform,
                &mut velocity,
                &mut rng,
                &sky,
                time,
            );
            continue;
        }
        transform.translation = next;

        // Turn toward where it is going, the long way round never being the short way.
        let wanted = aim(stroke.layer, next, velocity.0);
        let mut delta = (wanted - stroke.angle).rem_euclid(TAU);
        if delta > std::f32::consts::PI {
            delta -= TAU;
        }
        stroke.angle += delta * turn;
        transform.rotation = stroke.angle;

        if !healing.0 {
            continue;
        }
        // Find the way home: the tone eases toward what belongs here, and the layer changes
        // over on a coin toss weighted by the same rate, so a region converts one stroke at a
        // time rather than all at once.
        let (layer, tone) = paint_at(next, &sky, time);
        let wanted = (tone + stroke.weave).clamp(0.0, 1.0);
        stroke.tone += (wanted - stroke.tone) * heal;
        if layer != stroke.layer && rng.chance(heal) {
            stroke.layer = layer;
            // Where a stroke belongs is where it is standing when it joins the land. Keeping
            // the point it was first laid down at would have the spring drag it back across
            // the canvas, which is a stroke sliding sideways through the village.
            stroke.anchor = next;
            let (length, width) = layer.brush();
            stroke.size = vec2(
                length * (0.78 + 0.44 * stroke.seed),
                width * (0.8 + 0.4 * stroke.seed),
            );
        }
    }
}
