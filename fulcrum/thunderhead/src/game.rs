//! The sky, as a field of small integers.
//!
//! Every physical pixel of the window is one cell of a [`Field`], and every cell holds one
//! *material*: a sky band, the sun, a band of desert, the shadow of a cloud on that desert, a
//! silhouette on the horizon, a band of cloud at one of three distances, or ink. What those
//! materials look like is the binary's business (see `look.rs`); this file only decides what
//! is where.
//!
//! That indirection is the whole reason the piece can be drawn the way Moebius drew: flat
//! areas of colour with a clean line around them. A cell is not a colour, it is a *region
//! label*, so the picture is made of a few dozen regions no matter how many million pixels it
//! covers, and changing the palette is sixty-six numbers rather than a repaint.
//!
//! Three things happen in here.
//!
//! 1. [`build_backdrop`] draws the desert and the sky it stands under, once per window size.
//!    Nothing in it moves, so it is drawn once and copied.
//! 2. [`Forge`] grows one cumulonimbus at a time out of a hundred-odd overlapping puffs,
//!    shades it off its own density gradient, quantises the shading into bands and inks the
//!    edges. This is the expensive part, so it runs on a budget of texels per tick and takes
//!    several ticks over one cloud.
//! 3. [`compose`] copies the backdrop, lays the cloud shadows across the sand and blits the
//!    clouds themselves back to front.
//!
//! A cloud is measured in skies rather than in pixels: its height is a fraction of the height
//! of the sky above the horizon, resolved when the window says how big it is. So the piece
//! composes the same on a laptop panel and on a wall, and "enormous" stays enormous. Nothing is
//! ever scaled to fit, though. A cloud is blitted one texel to one pixel, always, because a
//! resampled ink line is a smeared one; when the window changes shape the clouds are regrown at
//! the new size, one at a time, while the old ones keep sailing.

use fulcrum::prelude::*;
use std::collections::VecDeque;
use std::f32::consts::TAU;

// ---------------------------------------------------------------------------------------
// materials
// ---------------------------------------------------------------------------------------

/// The line. One material for every contour in the picture, so the whole drawing is inked in
/// a single colour, as a pen would ink it.
pub const INK: u8 = 0;

/// First sky band. Band 0 is the zenith and the last one sits on the horizon.
pub const SKY_FIRST: u8 = 1;
/// How many bands the sky is stepped into.
pub const SKY_BANDS: u8 = 16;
/// The sun's disc.
pub const SUN: u8 = SKY_FIRST + SKY_BANDS;
/// First desert band. Band 0 lies against the horizon, the last one at the viewer's feet.
pub const GROUND_FIRST: u8 = SUN + 1;
/// How many bands the desert is stepped into.
pub const GROUND_BANDS: u8 = 6;
/// The same bands again, in the shadow of a cloud. Kept parallel to [`GROUND_FIRST`] so
/// shading a pixel is one addition.
pub const SHADOW_FIRST: u8 = GROUND_FIRST + GROUND_BANDS;
/// Silhouettes standing on the horizon, near one first.
pub const MESA_FIRST: u8 = SHADOW_FIRST + GROUND_BANDS;
/// How many distances the mesas come in.
pub const MESA_DEPTHS: u8 = 2;
/// Stones lying on the sand.
pub const STONE: u8 = MESA_FIRST + MESA_DEPTHS;
/// First cloud material. Clouds are laid out tier by tier: `CLOUD_FIRST + tier *
/// CLOUD_STRIDE + band`, and the band one past the last is that tier's ink.
pub const CLOUD_FIRST: u8 = STONE + 1;
/// How many bands a cloud is shaded in.
pub const CLOUD_BANDS: u8 = 10;
/// Materials per tier: its bands, then its ink.
pub const CLOUD_STRIDE: u8 = CLOUD_BANDS + 1;
/// How many materials there are altogether. Comfortably inside a byte, which is the point.
pub const MATERIALS: usize = CLOUD_FIRST as usize + TIER_COUNT * CLOUD_STRIDE as usize;

/// Ink, inside a cloud's own bitmap, where the values run `0` for open sky and `1..=CLOUD_BANDS`
/// for the bands.
const LOCAL_INK: u8 = CLOUD_BANDS + 1;

// ---------------------------------------------------------------------------------------
// the desert
// ---------------------------------------------------------------------------------------

/// Where the horizon sits, as a fraction of the window's height. Low, because the piece is
/// about what is above it: better than three quarters of the frame is sky.
pub const HORIZON: f32 = 0.76;

/// How the sky's bands are distributed between zenith and horizon. Above one, so the bands
/// crowd together near the horizon, which is both what the atmosphere does and what stops the
/// steps reading as stripes.
const SKY_CURVE: f32 = 1.9;

/// Where the sun stands, across the window and up from the horizon as a fraction of the sky's
/// height.
const SUN_AT: Vec2 = Vec2::new(0.73, 0.34);
/// The sun's radius, as a fraction of the sky's height.
const SUN_RADIUS: f32 = 0.075;
/// How far its glow bends the sky's bands, in sky heights.
const SUN_REACH: f32 = 0.85;
/// How much brighter the sky goes at the middle of that glow, in bands.
const SUN_GLOW: f32 = 4.5;

/// How thick the horizon's line is, in pixels per thousand of window height.
const HORIZON_INK: f32 = 2.6;

/// Where the desert's band boundaries lie, from the horizon down, before the wobble. Squared,
/// so the far bands crowd against the horizon the way ground does under perspective.
const DUNE_AT: [f32; GROUND_BANDS as usize - 1] = [0.06, 0.17, 0.33, 0.54, 0.79];

/// Half-thickness of a dune's contour line at the horizon and at the viewer's feet, in pixels.
/// A line drawn near the eye is a line drawn nearer, so it is drawn thicker.
const DUNE_INK: (f32, f32) = (1.2, 3.4);

/// The desert as it arrives at a given window size. Nothing in it moves, so it is drawn once
/// and copied into the [`Field`] on every tick.
#[derive(Resource, Clone, Default)]
pub struct Backdrop {
    /// Pixels across.
    pub width: u32,
    /// Pixels down.
    pub height: u32,
    /// The row the horizon's line sits on.
    pub horizon: u32,
    /// One material per pixel, `cells[row * width + col]`, row 0 at the top.
    pub cells: Vec<u8>,
}

/// One block of rock standing on the horizon.
struct Butte {
    /// Middle of the block, in pixels across the window.
    at: f32,
    /// Half its width, in pixels.
    half: f32,
    /// How far it rises above the horizon, in pixels.
    rise: f32,
    /// Where its cliff gives way to the scree under it, as a fraction of its half-width.
    shoulder: f32,
    /// How much lower its far end is than its near one, as a fraction of its rise. A mesa's
    /// top is level to within a degree or two, and a degree or two is what keeps it from
    /// looking like a crate.
    tilt: f32,
    /// Which of the [`MESA_DEPTHS`] distances it stands at.
    depth: u8,
}

impl Butte {
    /// How far above the horizon its outline runs at `x`, in pixels. Zero off either end.
    ///
    /// A cliff with a fan of scree at each end of it, which is what erosion leaves and what
    /// Moebius drew: a mesa is a long horizontal line with two short diagonals under it.
    fn rise_at(&self, x: f32) -> f32 {
        let across = (x - self.at) / self.half;
        let t = across.abs();
        if t >= 1.0 {
            return 0.0;
        }
        let top = self.rise * (1.0 + self.tilt * across);
        if t < self.shoulder {
            top
        } else {
            top * (1.0 - (t - self.shoulder) / (1.0 - self.shoulder))
        }
    }
}

/// One stone lying on the sand.
struct Stone {
    /// Where it lies, in pixels.
    at: Vec2,
    /// Half-width and half-height, in pixels.
    size: Vec2,
}

/// Draw the desert, the sky over it and the sun in that sky.
///
/// Every pixel of the window is visited a handful of times here, which would be far too slow
/// to do per frame. It does not have to be: the viewer stands still and only the weather
/// moves, so this runs when the window changes shape and at no other time.
pub fn build_backdrop(width: u32, height: u32, seed: u64) -> Backdrop {
    let mut cells = vec![SKY_FIRST; (width as usize) * (height as usize)];
    let horizon = ((height as f32 * HORIZON) as u32).clamp(1, height.saturating_sub(1));
    let mut back = Backdrop {
        width,
        height,
        horizon,
        cells: Vec::new(),
    };
    if width < 16 || height < 16 {
        back.cells = cells;
        return back;
    }

    // Everything the desert is furnished with is decided before anything is drawn, and from a
    // seed of its own, so that the same place is redrawn at every window size.
    let mut rng = SimRng::seeded(seed);
    let sky_h = horizon as f32;
    let ground_h = (height - horizon) as f32;
    let sun = vec2(width as f32 * SUN_AT.x, sky_h - sky_h * SUN_AT.y);
    let sun_r = sky_h * SUN_RADIUS;

    let mut buttes: Vec<Butte> = Vec::new();
    for _ in 0..rng.range_i32(3..6) {
        let depth = u8::from(rng.chance(0.5));
        // Nearer blocks are bigger, and the far ones only just break the horizon.
        let grade = if depth == 0 { 1.0 } else { 0.6 };
        let at = rng.range_f32(-0.05..1.05) * width as f32;
        // Long and low. A mesa is a horizon that has been left standing where the rest of the
        // ground was carried off, so it is far wider than it is tall; drawn any other way it
        // reads as a tent pitched on the skyline.
        let half = rng.range_f32(0.05..0.17) * width as f32 * grade;
        let rise = rng.range_f32(0.02..0.055) * sky_h * grade;
        // Half of them get a second block standing on the first, off to one side, which is
        // what turns a trapezoid into a butte with a shoulder.
        if rng.chance(0.5) {
            buttes.push(Butte {
                at: at + half * rng.range_f32(-0.55..0.55),
                half: half * rng.range_f32(0.25..0.5),
                rise: rise * rng.range_f32(1.4..2.2),
                shoulder: rng.range_f32(0.45..0.8),
                tilt: rng.range_f32(-0.10..0.10),
                depth,
            });
        }
        buttes.push(Butte {
            at,
            half,
            rise,
            shoulder: rng.range_f32(0.5..0.88),
            tilt: rng.range_f32(-0.08..0.08),
            depth,
        });
    }
    // Far blocks first, so a near one stands in front of what it overlaps.
    buttes.sort_by_key(|butte| std::cmp::Reverse(butte.depth));

    let mut stones: Vec<Stone> = Vec::new();
    for _ in 0..rng.range_i32(9..17) {
        let u = rng.range_f32(0.10..1.0);
        // A stone of one size looks smaller the further off it lies, and the ground runs away
        // fast near the horizon, so its drawn size grows with the square of its distance down
        // the frame.
        let scale = 0.004 + 0.030 * u * u;
        let rx = scale * height as f32 * rng.range_f32(0.7..1.4);
        stones.push(Stone {
            at: vec2(
                rng.range_f32(0.0..1.0) * width as f32,
                horizon as f32 + u * ground_h,
            ),
            size: vec2(rx, rx * rng.range_f32(0.42..0.68)),
        });
    }

    // The sky. One band per pixel, bent into arcs around the sun by its glow: a flat colour
    // with a curved edge is a great deal more sky than a flat colour with a straight one.
    for y in 0..horizon {
        let t = (y as f32 + 0.5) / sky_h;
        let level = t.powf(SKY_CURVE) * (SKY_BANDS - 1) as f32;
        let row = (y as usize) * (width as usize);
        for x in 0..width as usize {
            let away = (vec2(x as f32 + 0.5, y as f32 + 0.5) - sun).length() / (sky_h * SUN_REACH);
            let glow = SUN_GLOW * (1.0 - away).clamp(0.0, 1.0).powf(1.6);
            let band = (level + glow).clamp(0.0, (SKY_BANDS - 1) as f32) as u8;
            cells[row + x] = SKY_FIRST + band;
        }
    }

    // The sun: a flat disc with a line around it, the way it is drawn on a page rather than
    // the way it burns.
    let ink = (HORIZON_INK * height as f32 / 1000.0).max(1.0);
    disc(&mut cells, width, horizon, sun, sun_r, SUN);
    ring(&mut cells, width, horizon, sun, sun_r, ink);

    // The rock on the horizon, each block outlined along its own profile. A nearer block is
    // drawn after the one behind it, so it covers that one's line where it should.
    for butte in &buttes {
        let material = MESA_FIRST + butte.depth;
        let from = ((butte.at - butte.half).floor().max(0.0)) as u32;
        let to = ((butte.at + butte.half).ceil().clamp(0.0, width as f32)) as u32;
        for x in from..to {
            let rise = butte.rise_at(x as f32 + 0.5);
            if rise < 1.0 {
                continue;
            }
            let top = (horizon as f32 - rise).max(0.0) as u32;
            for y in top..horizon {
                cells[(y as usize) * (width as usize) + x as usize] = if (y - top) as f32 <= ink {
                    INK
                } else {
                    material
                };
            }
        }
    }

    // The horizon, and then the ground under it.
    let mut bounds = vec![0.0f32; DUNE_AT.len() * width as usize];
    for (index, &at) in DUNE_AT.iter().enumerate() {
        // Each contour gets its own slow wave, so the desert reads as a landscape of long
        // shallow rises rather than a set of ruled lines.
        let amp = rng.range_f32(0.045..0.11) * (0.35 + at);
        let waves = rng.range_f32(0.35..1.3);
        let phase = rng.range_f32(0.0..TAU);
        let second = rng.range_f32(0.0..TAU);
        for x in 0..width as usize {
            let u = x as f32 / width as f32;
            let wobble = (u * waves * TAU + phase).sin() * 0.7
                + (u * waves * 2.3 * TAU + second).sin() * 0.3;
            bounds[index * width as usize + x] = at + amp * wobble;
        }
    }
    for y in horizon..height {
        let u = (y - horizon) as f32 / ground_h;
        let half = (DUNE_INK.0 + (DUNE_INK.1 - DUNE_INK.0) * u) / ground_h;
        let row = (y as usize) * (width as usize);
        for x in 0..width as usize {
            let mut band = 0u8;
            let mut on_line = false;
            for index in 0..DUNE_AT.len() {
                let edge = bounds[index * width as usize + x];
                if u > edge {
                    band = index as u8 + 1;
                }
                on_line |= (u - edge).abs() < half;
            }
            cells[row + x] = if on_line { INK } else { GROUND_FIRST + band };
        }
    }
    for y in horizon..(horizon + ink.round() as u32 + 1).min(height) {
        let row = (y as usize) * (width as usize);
        cells[row..row + width as usize].fill(INK);
    }

    // The stones, each with the shadow the sun on the right leaves to its left.
    for stone in &stones {
        let shadow = vec2(
            stone.at.x - stone.size.x * 1.5,
            stone.at.y + stone.size.y * 0.55,
        );
        ellipse(
            &mut cells,
            width,
            horizon,
            height,
            shadow,
            vec2(stone.size.x * 1.5, stone.size.y * 0.5),
            shade,
        );
        ellipse(
            &mut cells,
            width,
            horizon,
            height,
            stone.at,
            stone.size,
            |_| STONE,
        );
        outline(
            &mut cells, width, horizon, height, stone.at, stone.size, ink,
        );
    }

    back.cells = cells;
    back
}

/// The shadowed twin of a desert band. Anything else is left alone: ink stays ink, and a
/// stone in shadow is the same stone.
fn shade(material: u8) -> u8 {
    if (GROUND_FIRST..GROUND_FIRST + GROUND_BANDS).contains(&material) {
        material + GROUND_BANDS
    } else {
        material
    }
}

/// Fill a disc, clipped to rows `0..limit`.
fn disc(cells: &mut [u8], width: u32, limit: u32, at: Vec2, radius: f32, material: u8) {
    ellipse(cells, width, 0, limit, at, Vec2::splat(radius), |_| {
        material
    });
}

/// Ink a circle's edge, clipped to rows `0..limit`.
fn ring(cells: &mut [u8], width: u32, limit: u32, at: Vec2, radius: f32, thick: f32) {
    outline(cells, width, 0, limit, at, Vec2::splat(radius), thick);
}

/// Fill an ellipse, clipped to rows `top..limit`, replacing each covered pixel by what `paint`
/// makes of it.
fn ellipse(
    cells: &mut [u8],
    width: u32,
    top: u32,
    limit: u32,
    at: Vec2,
    size: Vec2,
    paint: impl Fn(u8) -> u8,
) {
    if size.x < 0.5 || size.y < 0.5 {
        return;
    }
    let x0 = (at.x - size.x).floor().max(0.0) as u32;
    let x1 = (at.x + size.x).ceil().clamp(0.0, width as f32) as u32;
    let y0 = (at.y - size.y).floor().max(top as f32) as u32;
    let y1 = (at.y + size.y).ceil().clamp(top as f32, limit as f32) as u32;
    for y in y0..y1 {
        let dy = (y as f32 + 0.5 - at.y) / size.y;
        let row = (y as usize) * (width as usize);
        for x in x0..x1 {
            let dx = (x as f32 + 0.5 - at.x) / size.x;
            if dx * dx + dy * dy <= 1.0 {
                cells[row + x as usize] = paint(cells[row + x as usize]);
            }
        }
    }
}

/// Ink an ellipse's edge, clipped to rows `top..limit`.
fn outline(cells: &mut [u8], width: u32, top: u32, limit: u32, at: Vec2, size: Vec2, thick: f32) {
    let inner = vec2(
        (size.x - thick).max(0.1) / size.x,
        (size.y - thick).max(0.1) / size.y,
    );
    let x0 = (at.x - size.x).floor().max(0.0) as u32;
    let x1 = (at.x + size.x).ceil().clamp(0.0, width as f32) as u32;
    let y0 = (at.y - size.y).floor().max(top as f32) as u32;
    let y1 = (at.y + size.y).ceil().clamp(top as f32, limit as f32) as u32;
    for y in y0..y1 {
        let dy = (y as f32 + 0.5 - at.y) / size.y;
        let row = (y as usize) * (width as usize);
        for x in x0..x1 {
            let dx = (x as f32 + 0.5 - at.x) / size.x;
            let outside = dx * dx + dy * dy;
            let hollow = (dx / inner.x) * (dx / inner.x) + (dy / inner.y) * (dy / inner.y);
            if outside <= 1.0 && hollow > 1.0 {
                cells[row + x as usize] = INK;
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// one cloud
// ---------------------------------------------------------------------------------------

/// How far a cloud is off, which settles how big its bitmap is, how fast it crosses, how high
/// its base rides and how heavily it is inked.
#[derive(Clone, Copy)]
pub struct TierSpec {
    /// Its name, for the readout.
    pub name: &'static str,
    /// How tall its bitmap is, as a fraction of the sky's height.
    pub rise: f32,
    /// How wide it is against its own height.
    pub aspect: f32,
    /// How fast it crosses the window at pace one, in sky heights a second.
    pub speed: f32,
    /// Where its base rides, as a fraction of the sky's height above the horizon.
    pub lift: f32,
    /// How far down the desert its shadow falls, as a fraction of the ground's height.
    pub cast: f32,
    /// How thick its silhouette line is, in texels per thousand.
    pub ink: f32,
    /// How many generations of smaller puffs are grown on its lobes. The far tier gets one:
    /// at that size a third generation is under a texel and only costs time.
    pub detail: u32,
}

/// How many distances clouds come in.
pub const TIER_COUNT: usize = 3;

/// The three distances. Everything about a tier moves together, which is what makes the depth
/// read: the near clouds are bigger, faster, higher, more strongly drawn, and their shadows
/// cross the sand right in front of you.
///
/// Every measurement is a fraction of the sky rather than a count of pixels, so the piece
/// composes the same on a laptop panel and on a wall, and so "enormous" stays enormous.
pub const TIERS: [TierSpec; TIER_COUNT] = [
    TierSpec {
        name: "far",
        rise: 0.34,
        aspect: 1.60,
        speed: 0.017,
        lift: 0.045,
        cast: 0.05,
        ink: 4.0,
        detail: 1,
    },
    TierSpec {
        name: "middle",
        rise: 0.58,
        aspect: 1.55,
        speed: 0.034,
        lift: 0.12,
        cast: 0.20,
        ink: 3.4,
        detail: 2,
    },
    TierSpec {
        name: "near",
        rise: 0.84,
        aspect: 1.55,
        speed: 0.058,
        lift: 0.21,
        cast: 0.55,
        ink: 3.0,
        detail: 2,
    },
];

/// What a tier comes to in texels under a sky of `sky` pixels.
pub fn tier_size(tier: usize, sky: f32) -> (u32, u32) {
    let spec = TIERS[tier % TIER_COUNT];
    let height = (sky * spec.rise).max(8.0);
    ((height * spec.aspect) as u32, height as u32)
}

/// One overlapping ball of the density a cloud is grown out of.
#[derive(Clone, Copy)]
struct Puff {
    /// Its middle, in texels.
    at: Vec2,
    /// Its half-width and half-height, in texels.
    size: Vec2,
}

/// How much density a cloud needs before it counts as cloud rather than air.
const SOLID: f32 = 0.25;

/// How far apart the two texels are that the slope is measured across, as a fraction of the
/// cloud's height. Wide on purpose: a slope measured between neighbouring texels is a slope
/// through the smallest puffs there are, and the shading follows every one of them.
const SLOPE_SPAN: f32 = 0.011;

/// How steeply the density is read as a surface when it is lit. Bigger tilts the lobes harder
/// against the light and pulls the bands apart. A pure number, because the slope it multiplies
/// is measured over a span that scales with the cloud: a far cloud is shaded like a near one.
const RELIEF: f32 = 9.0;

/// The lit and unlit ends of the tone the bands are cut from. Nothing in a cloud comes out
/// pure black or pure white, so the bands are spread across the range that is really used
/// rather than across the whole of it, and the palette's darkest colour is spent on something.
const TONE: (f32, f32) = (0.22, 0.96);

/// Where the light comes from, in the bitmap's own axes, with `y` running down the page and
/// `z` out of it. Up and from the right, which is where [`SUN_AT`] puts the sun.
const LIGHT: [f32; 3] = [0.52, -0.66, 0.54];

/// What a cloud is lit by before any light reaches it, how much the light adds, and how much
/// brighter its head is than its base. They sum to a little over one on purpose: a thunderhead
/// has a blinding top and the eye should be made to squint at it.
const AMBIENT: f32 = 0.13;
/// How much of the shading comes from the light striking the lobes.
const LAMBERT: f32 = 0.42;
/// How much of it comes from height alone, which is what darkens the rain-heavy base.
const CROWN: f32 = 0.58;

/// Ink a contour every this many bands. Every band would be lace; every other one reads as a
/// drawn lobe.
const CONTOUR_EVERY: u8 = 3;

/// How deep the base band is, as a fraction of the cloud's height. Only used to work out what
/// shadow the cloud throws, which is cast by its underside.
const HEM_DEPTH: f32 = 0.13;

/// A finished cloud: a bitmap of bands and ink, and the underside that casts its shadow.
#[derive(Clone, Default)]
pub struct Anvil {
    /// Texels across.
    pub width: u32,
    /// Texels down.
    pub height: u32,
    /// `0` for open sky, `1..=CLOUD_BANDS` for a band, and one past that for ink.
    pub cells: Vec<u8>,
    /// The row its flat base sits on. What the cloud is hung from when it is placed.
    pub base: u32,
    /// How much of the base band each column carries, `0..=255`. The shape of the shadow.
    pub hem: Vec<u8>,
}

impl Anvil {
    /// Whether it has been built yet.
    pub fn ready(&self) -> bool {
        !self.cells.is_empty()
    }
}

/// Which part of a cloud is being built.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    /// Adding the puffs up into a density field.
    Stamp,
    /// Turning that density into shaded bands.
    Shade,
    /// Drawing the line around the bands.
    Ink,
    /// Measuring the underside.
    Hem,
    /// Nothing left to do.
    Done,
}

/// One cloud under construction.
///
/// Building a cloud costs tens of millions of texel writes, which is a dropped frame if it is
/// done between two of them. So it is done on an allowance instead: [`Forge::work`] spends a
/// budget of texels and stops wherever it runs out, and a cloud takes however many ticks that
/// comes to. The cloud being replaced stays on screen and intact the whole time, because the
/// new one is grown in a buffer of its own and only swapped in when it is finished.
pub struct Forge {
    /// Which drifter this cloud is for.
    pub target: usize,
    /// Which tier it is being built at.
    pub tier: usize,
    width: u32,
    height: u32,
    base: f32,
    ink: i32,
    span: i32,
    wobble: [f32; 4],
    puffs: Vec<Puff>,
    density: Vec<f32>,
    bands: Vec<u8>,
    cells: Vec<u8>,
    hem: Vec<u8>,
    cursor: u32,
    phase: Phase,
}

impl Forge {
    /// Lay out a new cloud's puffs and clear the field they will be added into.
    pub fn start(target: usize, tier: usize, size: (u32, u32), rng: &mut SimRng) -> Self {
        let spec = TIERS[tier % TIER_COUNT];
        let (width, height) = (size.0.max(8), size.1.max(8));
        let (puffs, base) = grow(&spec, width as f32, height as f32, rng);
        let texels = (width as usize) * (height as usize);
        Self {
            target,
            tier,
            width,
            height,
            base,
            ink: ((height as f32 * spec.ink / 1000.0).round() as i32).max(1),
            span: ((height as f32 * SLOPE_SPAN).round() as i32).max(1),
            // A base that is dead level reads as a cut rather than a cloud, so it is given a
            // slow ripple. Two waves, because one is a sine and looks like one.
            wobble: [
                rng.range_f32(1.0..2.5),
                rng.range_f32(0.0..TAU),
                rng.range_f32(2.5..6.0),
                rng.range_f32(0.0..TAU),
            ],
            puffs,
            density: vec![0.0; texels],
            bands: vec![0; texels],
            cells: vec![0; texels],
            hem: vec![0; width as usize],
            cursor: 0,
            phase: Phase::Stamp,
        }
    }

    /// Whether the cloud is finished.
    pub fn done(&self) -> bool {
        self.phase == Phase::Done
    }

    /// Spend up to `budget` texels of work on it.
    pub fn work(&mut self, budget: u64) {
        let mut spent = 0u64;
        while spent < budget && self.phase != Phase::Done {
            match self.phase {
                Phase::Stamp => {
                    let puff = self.puffs[self.cursor as usize];
                    spent += stamp(&mut self.density, self.width, self.height, &puff);
                    self.cursor += 1;
                    if self.cursor as usize == self.puffs.len() {
                        self.step(Phase::Shade);
                    }
                }
                Phase::Shade => {
                    self.shade_row(self.cursor);
                    spent += u64::from(self.width);
                    self.cursor += 1;
                    if self.cursor == self.height {
                        self.step(Phase::Ink);
                    }
                }
                Phase::Ink => {
                    self.ink_row(self.cursor);
                    spent += u64::from(self.width);
                    self.cursor += 1;
                    if self.cursor == self.height {
                        self.step(Phase::Hem);
                    }
                }
                Phase::Hem => {
                    self.hem_column(self.cursor);
                    spent += u64::from(self.height);
                    self.cursor += 1;
                    if self.cursor == self.width {
                        self.step(Phase::Done);
                    }
                }
                Phase::Done => break,
            }
        }
    }

    /// Move on to the next part.
    fn step(&mut self, next: Phase) {
        self.phase = next;
        self.cursor = 0;
    }

    /// The finished cloud. Only call it once [`done`](Self::done) says so.
    pub fn take(self) -> Anvil {
        Anvil {
            width: self.width,
            height: self.height,
            cells: self.cells,
            base: self.base as u32,
            hem: self.hem,
        }
    }

    /// Where the base lies under column `x`, in texels down the bitmap.
    fn base_at(&self, x: u32) -> f32 {
        let u = x as f32 / self.width as f32;
        let ripple = (u * self.wobble[0] * TAU + self.wobble[1]).sin() * 0.7
            + (u * self.wobble[2] * TAU + self.wobble[3]).sin() * 0.3;
        self.base + ripple * self.height as f32 * 0.012
    }

    /// Shade one row of the density into bands.
    ///
    /// The density is read as a height field and lit off its own gradient, which is a cheat
    /// with a good excuse: where a cloud bulges towards you its density falls off fast at the
    /// edge of the bulge, so the gradient really does point along the lobe. Quantising the
    /// result is what turns the lighting into drawing: every band boundary is a contour of the
    /// lobes, and there is a flat colour between one and the next.
    fn shade_row(&mut self, y: u32) {
        let width = self.width as usize;
        let row = (y as usize) * width;
        let inv = 1.0 / (self.base - 1.0).max(1.0);
        let scale = CLOUD_BANDS as f32 / (TONE.1 - TONE.0);
        for x in 0..width {
            if self.density[row + x] < SOLID || (y as f32) > self.base_at(x as u32) {
                self.bands[row + x] = 0;
                continue;
            }
            let gx =
                self.at(x as i32 + self.span, y as i32) - self.at(x as i32 - self.span, y as i32);
            let gy =
                self.at(x as i32, y as i32 + self.span) - self.at(x as i32, y as i32 - self.span);
            let (gx, gy) = (gx * RELIEF, gy * RELIEF);
            let facing = 1.0 / (gx * gx + gy * gy + 1.0).sqrt();
            let lambert = ((-gx) * LIGHT[0] + (-gy) * LIGHT[1] + LIGHT[2]) * facing;
            // How high up the cloud this texel sits, from nothing at the base to one at the
            // crown. A thunderhead's own bulk is what shades its underside, and no lighting
            // model in here knows that, so it is stated outright.
            let lift = ((self.base - y as f32) * inv).clamp(0.0, 1.0);
            let tone = AMBIENT + LAMBERT * lambert.max(0.0) + CROWN * lift;
            let band = ((tone - TONE.0) * scale).clamp(0.0, (CLOUD_BANDS - 1) as f32) as u8;
            self.bands[row + x] = band + 1;
        }
    }

    /// The density at `(x, y)`, clamped to the bitmap. What the lighting is read off.
    fn at(&self, x: i32, y: i32) -> f32 {
        let x = x.clamp(0, self.width as i32 - 1) as usize;
        let y = y.clamp(0, self.height as i32 - 1) as usize;
        self.density[y * (self.width as usize) + x]
    }

    /// Draw the line: around the cloud, and along every other contour inside it.
    fn ink_row(&mut self, y: u32) {
        let width = self.width as usize;
        let row = (y as usize) * width;
        let thick = self.ink;
        for x in 0..width {
            let here = self.bands[row + x];
            if here == 0 {
                self.cells[row + x] = 0;
                continue;
            }
            let mut inked = false;
            // The silhouette, thickened by looking further out along the four directions
            // rather than by a second pass over the bitmap.
            for reach in 1..=thick {
                let sky = self.sample(x as i32 - reach, y as i32) == 0
                    || self.sample(x as i32 + reach, y as i32) == 0
                    || self.sample(x as i32, y as i32 - reach) == 0
                    || self.sample(x as i32, y as i32 + reach) == 0;
                if sky {
                    inked = true;
                    break;
                }
            }
            if !inked {
                let east = self.sample(x as i32 + 1, y as i32);
                let south = self.sample(x as i32, y as i32 + 1);
                for other in [east, south] {
                    if other != 0 && other != here && here.max(other) % CONTOUR_EVERY == 0 {
                        inked = true;
                    }
                }
            }
            self.cells[row + x] = if inked { LOCAL_INK } else { here };
        }
    }

    /// The band at `(x, y)`, counting everything off the bitmap as open sky.
    fn sample(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return 0;
        }
        self.bands[(y as usize) * (self.width as usize) + x as usize]
    }

    /// Measure how much cloud column `x` carries in its base band, which is what its shadow
    /// on the sand is drawn from.
    fn hem_column(&mut self, x: u32) {
        let depth = (self.height as f32 * HEM_DEPTH).max(1.0);
        let from = (self.base - depth).max(0.0) as u32;
        let to = (self.base as u32 + 1).min(self.height);
        let mut covered = 0u32;
        for y in from..to {
            if self.bands[(y as usize) * (self.width as usize) + x as usize] != 0 {
                covered += 1;
            }
        }
        let share = covered as f32 / (to - from).max(1) as f32;
        self.hem[x as usize] = (share * 255.0) as u8;
    }
}

/// Build a whole cloud in one go. The startup path: at the opening of the piece there is
/// nothing on screen to stutter yet, so the sky is filled before the first frame.
pub fn forge_now(target: usize, tier: usize, size: (u32, u32), rng: &mut SimRng) -> Anvil {
    let mut forge = Forge::start(target, tier, size, rng);
    while !forge.done() {
        forge.work(u64::MAX);
    }
    forge.take()
}

/// Lay one puff into the density field, and report what it cost.
///
/// The puffs are combined by taking the *larger* of the two rather than by adding them, which
/// is the single decision that makes these read as drawn clouds instead of as rocks. Added
/// together, two overlapping puffs bulge where they meet and the outline of the pile comes out
/// as one smooth blob. Taken larger-of, the outline is the union of the puffs and stays a chain
/// of arcs, which is how a cloud is drawn with a pen; a puff buried inside a bigger one leaves
/// no mark at all, so the inside stays calm however much detail is heaped on the edge; and the
/// crease where two lobes meet is a real crease, which the ink pass then finds and draws.
///
/// Each puff's own falloff is `(1 - q)^2`, smooth where it meets zero, so the only sharp edges
/// in the field are the ones the union puts there.
fn stamp(density: &mut [f32], width: u32, height: u32, puff: &Puff) -> u64 {
    let x0 = (puff.at.x - puff.size.x).floor().max(0.0) as u32;
    let x1 = (puff.at.x + puff.size.x).ceil().clamp(0.0, width as f32) as u32;
    let y0 = (puff.at.y - puff.size.y).floor().max(0.0) as u32;
    let y1 = (puff.at.y + puff.size.y).ceil().clamp(0.0, height as f32) as u32;
    for y in y0..y1 {
        let dy = (y as f32 + 0.5 - puff.at.y) / puff.size.y;
        let dy2 = dy * dy;
        if dy2 >= 1.0 {
            continue;
        }
        let row = (y as usize) * (width as usize);
        for x in x0..x1 {
            let dx = (x as f32 + 0.5 - puff.at.x) / puff.size.x;
            let q = dx * dx + dy2;
            if q < 1.0 {
                let f = 1.0 - q;
                let cell = &mut density[row + x as usize];
                *cell = cell.max(f * f);
            }
        }
    }
    u64::from(x1.saturating_sub(x0)) * u64::from(y1.saturating_sub(y0))
}

/// Lay out the puffs of one cumulonimbus, and say which row its base sits on.
///
/// The anatomy is the real one, because the real one is what the eye recognises. A flat base
/// where the air stops being able to hold its water; a tower of lobes boiling up from it,
/// leaning downwind as it rises; a bulge of shoulders where the tower is fattest; and, at the
/// top, the anvil, where the rising air hits a ceiling it cannot pass and spreads out sideways
/// under it.
fn grow(spec: &TierSpec, w: f32, h: f32, rng: &mut SimRng) -> (Vec<Puff>, f32) {
    let base = h * 0.93;
    let top = h * 0.14;
    let middle = w * 0.5;
    // How far the head is carried downwind of the foot.
    let lean = rng.range_f32(-0.18..0.18) * w;
    let mut puffs: Vec<Puff> = Vec::new();

    // The rising mass. Widest a third of the way up and narrowing from there, which is the
    // shape convection leaves: the cloud is fed from below and runs out of push as it climbs.
    let stack = rng.range_i32(5..8);
    for index in 0..stack {
        let t = index as f32 / (stack - 1) as f32;
        let envelope = 1.0 - 0.5 * ((t - 0.3) / 0.7).max(0.0);
        let size = w * 0.19 * envelope * rng.range_f32(0.9..1.15);
        puffs.push(Puff {
            at: vec2(
                middle + lean * t * t + rng.range_f32(-0.05..0.05) * w,
                base - (base - top) * t,
            ),
            size: vec2(size, size * rng.range_f32(0.78..1.05)),
        });
    }

    // The bank along the base, which is what makes the cloud sit on a line rather than balance
    // on a point.
    for _ in 0..rng.range_i32(3..6) {
        let size = w * 0.13 * rng.range_f32(0.85..1.25);
        puffs.push(Puff {
            at: vec2(
                middle + rng.range_f32(-0.32..0.32) * w,
                base - rng.range_f32(0.06..0.20) * h,
            ),
            size: vec2(size, size * rng.range_f32(0.7..0.95)),
        });
    }

    // Shoulders, off to one side or the other of the middle.
    for _ in 0..rng.range_i32(2..5) {
        let t = rng.range_f32(0.15..0.6);
        let side = if rng.chance(0.5) { 1.0 } else { -1.0 };
        let size = w * 0.12 * rng.range_f32(0.85..1.3);
        puffs.push(Puff {
            at: vec2(
                middle + lean * t * t + side * w * rng.range_f32(0.18..0.30),
                base - (base - top) * t,
            ),
            size: vec2(size, size * 0.85),
        });
    }

    // The anvil. Flat because it has hit a ceiling and can only go sideways under it, and wide
    // because that is what the top of a storm does with the air it cannot lift any further.
    for _ in 0..rng.range_i32(4..7) {
        puffs.push(Puff {
            at: vec2(
                middle + lean + rng.range_f32(-0.32..0.32) * w,
                top + h * 0.06 + rng.range_f32(-0.02..0.05) * h,
            ),
            size: vec2(
                w * rng.range_f32(0.13..0.22),
                h * rng.range_f32(0.055..0.095),
            ),
        });
    }

    // The cauliflower. Each lobe sprouts smaller lobes around its rim, mostly upwards, and some
    // of those sprout smaller ones again: the silhouette gets its roughness from the same rule
    // at two scales, which is why it reads as boiling rather than as a drawn outline.
    //
    // Few and large, not many and small. Under the larger-of rule a sprout only shows where it
    // pokes out of its parent, so a hundred of them is a hundred scallops on the edge and the
    // cloud turns into coral.
    let mut generation = 0..puffs.len();
    for round in 0..spec.detail {
        let mut grown = Vec::new();
        for index in generation.clone() {
            let parent = puffs[index];
            if round > 0 && !rng.chance(0.5) {
                continue;
            }
            for _ in 0..rng.range_i32(2..5) {
                let mut angle = rng.range_f32(0.0..TAU);
                // In bitmap axes `y` runs down, so a positive sine points at the ground. Most
                // of those are turned over: a cloud grows up.
                if angle.sin() > 0.0 && rng.chance(0.72) {
                    angle = -angle;
                }
                let shrink = rng.range_f32(0.36..0.72);
                let out = rng.range_f32(0.45..0.85);
                grown.push(Puff {
                    at: parent.at
                        + vec2(angle.cos() * parent.size.x, angle.sin() * parent.size.y) * out,
                    size: parent.size * shrink,
                });
            }
        }
        generation = puffs.len()..puffs.len() + grown.len();
        puffs.extend(grown);
    }

    // Nothing above decided how far the puffs would actually reach, and a cloud that overruns
    // its bitmap comes back with its anvil sawn off square at both ends. So the whole cloud is
    // measured and then moved and scaled, once, to stand inside the bitmap with its base on
    // the floor. One scale for both axes: a cloud squeezed to fit is a cloud drawn wrong.
    let margin = h * 0.02 + 4.0;
    let mut low = Vec2::splat(f32::MAX);
    let mut high = Vec2::splat(f32::MIN);
    for puff in &puffs {
        low = low.min(puff.at - puff.size);
        high = high.max(puff.at + puff.size);
    }
    // Downwards, only as far as the base: everything under it is cut off anyway.
    let scale = ((w - margin * 2.0) / (high.x - low.x))
        .min((h - margin * 2.0) / (base - low.y))
        .min(1.6);
    let shift = vec2(
        (w - (high.x - low.x) * scale) * 0.5 - low.x * scale,
        (h - margin) - base * scale,
    );
    for puff in &mut puffs {
        puff.at = puff.at * scale + shift;
        puff.size *= scale;
    }

    (puffs, h - margin)
}

// ---------------------------------------------------------------------------------------
// the weather
// ---------------------------------------------------------------------------------------

/// How many clouds are in the air at once.
pub const DRIFTERS: usize = 7;

/// Which tier each of them belongs to, far ones first. The list doubles as the depth order
/// the compositor draws in, so it must stay sorted.
pub const DEPTHS: [usize; DRIFTERS] = [0, 0, 0, 1, 1, 2, 2];

/// How much of a cloud's own width is left clear when it re-enters from the right.
const GAP: f32 = 0.15;

/// How wide a shadow is against the cloud that throws it, and how flat.
const SHADOW_WIDE: f32 = 0.92;
/// How deep a shadow is against its own width.
const SHADOW_FLAT: f32 = 0.085;
/// How far the shadow is dragged away from the sun, as a fraction of the cloud's width.
const SHADOW_LEAN: f32 = 0.22;

/// One cloud in the air.
#[derive(Clone, Copy)]
pub struct Drifter {
    /// Which tier it is, an index into [`TIERS`].
    pub tier: usize,
    /// Where its bitmap's left edge is, in pixels across the window.
    pub x: f32,
    /// Where its base rides, as a fraction of the sky's height above the horizon.
    pub lift: f32,
    /// How fast it crosses, in pixels a second.
    pub speed: f32,
}

/// How fast the weather goes, at the viewer's asking.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct Motion {
    /// A multiplier on every cloud's own speed.
    pub pace: f32,
    /// Whether the sky is held still.
    pub held: bool,
}

impl Default for Motion {
    fn default() -> Self {
        Self {
            pace: 1.0,
            held: false,
        }
    }
}

/// Slowest and fastest the weather is allowed to run.
pub const PACE_MIN: f32 = 0.05;
/// Fastest the weather is allowed to run. Past this the clouds cross faster than they can be
/// looked at, which is a different piece.
pub const PACE_MAX: f32 = 24.0;
/// How much the pace is multiplied per second of holding the key. Multiplied rather than
/// added, because the useful range covers three decades and a step that suited one end would
/// be absurd at the other.
const PACE_RAMP: f32 = 3.2;

/// How many texels of cloud-building are paid for per tick. About a millisecond of the
/// sixteen a tick has, so a cloud takes a fraction of a second and no frame notices.
pub const FORGE_BUDGET: u64 = 2_000_000;

/// The weather: the clouds in the air, the shapes they are wearing, and the queue of shapes
/// waiting to be grown.
#[derive(Resource, Default)]
pub struct Sky {
    /// What the desert is drawn from. Kept so a resize redraws the same place.
    pub seed: u64,
    /// What each tier's bitmap comes to under the window it was last sized for.
    pub sizes: Vec<(u32, u32)>,
    /// One cloud per drifter, in the same order.
    pub shapes: Vec<Anvil>,
    /// The clouds in the air, far ones first.
    pub drifters: Vec<Drifter>,
    /// Drifters whose shape is to be regrown, in the order they went off the edge.
    pub waiting: VecDeque<usize>,
    /// The one being grown now.
    pub forge: Option<Forge>,
    /// Whether the drifters have been spread across a window yet.
    pub placed: bool,
    /// How many clouds have been grown since the piece opened, for the readout.
    pub grown: u64,
}

impl Sky {
    /// How many texels of cloud are in the air. The piece's own boast, in the readout.
    pub fn texels(&self) -> u64 {
        self.shapes
            .iter()
            .map(|anvil| u64::from(anvil.width) * u64::from(anvil.height))
            .sum()
    }
}

/// The window, one cell per physical pixel, with a material in each.
#[derive(Resource, Clone, Default)]
pub struct Field {
    /// Pixels across.
    pub width: u32,
    /// Pixels down.
    pub height: u32,
    /// One material per pixel, `cells[row * width + col]`, row 0 at the top.
    pub cells: Vec<u8>,
}

/// Replayable command: the window reporting its size in physical pixels ("width height").
pub const RESIZE_COMMAND: &str = "window";

/// What the window opens at before the real screen size arrives on the command channel.
pub const DEFAULT_WINDOW: (u32, u32) = (1600, 1000);

/// Encode a window size for [`RESIZE_COMMAND`]: whole pixels, so it round-trips exactly.
pub fn window_payload(width: u32, height: u32) -> String {
    format!("{width} {height}")
}

/// Decode a [`window_payload`]. `None` for anything malformed or degenerate.
pub fn parse_window(payload: &str) -> Option<(u32, u32)> {
    let mut parts = payload.split_whitespace();
    let width: u32 = parts.next()?.parse().ok()?;
    let height: u32 = parts.next()?.parse().ok()?;
    (parts.next().is_none() && width > 0 && height > 0).then_some((width, height))
}

/// Paint the whole window: the desert, then the shadows crossing it, then the clouds.
///
/// Cheap, and deliberately so. The backdrop is a copy, a shadow touches a strip of sand, and a
/// cloud touches only its own bitmap, so the cost is a few million byte writes however
/// complicated the weather looks. Everything expensive was paid for in [`Forge`], once.
pub fn compose(field: &mut Field, backdrop: &Backdrop, sky: &Sky) {
    if field.cells.len() != backdrop.cells.len() || field.cells.is_empty() {
        return;
    }
    field.cells.copy_from_slice(&backdrop.cells);
    let horizon = backdrop.horizon as f32;
    let ground = field.height as f32 - horizon;

    // Shadows first, all of them, then the clouds: a near cloud may well be drawn over the
    // patch of sand that a far one is darkening.
    for (index, drifter) in sky.drifters.iter().enumerate() {
        let Some(anvil) = sky.shapes.get(index).filter(|anvil| anvil.ready()) else {
            continue;
        };
        cast_shadow(field, anvil, drifter, horizon, ground);
    }
    for (index, drifter) in sky.drifters.iter().enumerate() {
        let Some(anvil) = sky.shapes.get(index).filter(|anvil| anvil.ready()) else {
            continue;
        };
        let top = horizon - drifter.lift * horizon - anvil.base as f32;
        blit(field, anvil, drifter.x, top, drifter.tier);
    }
}

/// Blit one cloud onto the field, clipped to it, skipping open sky.
pub fn blit(field: &mut Field, anvil: &Anvil, x: f32, y: f32, tier: usize) {
    let base = CLOUD_FIRST + (tier % TIER_COUNT) as u8 * CLOUD_STRIDE;
    let left = x.round() as i64;
    let top = y.round() as i64;
    let width = field.width as i64;
    let x0 = left.max(0);
    let x1 = (left + anvil.width as i64).min(width);
    let y0 = top.max(0);
    let y1 = (top + anvil.height as i64).min(field.height as i64);
    for row in y0..y1 {
        let src = ((row - top) as usize) * (anvil.width as usize);
        let dst = (row as usize) * (field.width as usize);
        for col in x0..x1 {
            let value = anvil.cells[src + (col - left) as usize];
            if value != 0 {
                field.cells[dst + col as usize] = base + value - 1;
            }
        }
    }
}

/// Lay a cloud's shadow across the sand.
///
/// Not a projection: the piece has no third dimension to project from. It is the cloud's own
/// underside, squashed flat and dragged away from the sun, which is enough to make the desert
/// darken and clear again as a thunderhead goes over.
fn cast_shadow(field: &mut Field, anvil: &Anvil, drifter: &Drifter, horizon: f32, ground: f32) {
    let spec = TIERS[drifter.tier % TIER_COUNT];
    let width = anvil.width as f32 * SHADOW_WIDE;
    let depth = width * SHADOW_FLAT;
    let middle = drifter.x + anvil.width as f32 * 0.5 - anvil.width as f32 * SHADOW_LEAN;
    let left = middle - width * 0.5;
    let row = horizon + ground * spec.cast;
    let x0 = left.floor().max(0.0) as i64;
    let x1 = (left + width).ceil().clamp(0.0, field.width as f32) as i64;
    for col in x0..x1 {
        let u = (col as f32 + 0.5 - left) / width;
        let source = ((u * anvil.width as f32) as usize).min(anvil.hem.len().saturating_sub(1));
        let carry = anvil.hem[source] as f32 / 255.0;
        if carry <= 0.02 {
            continue;
        }
        // Thinned towards both ends, because a shadow with square ends is a rug. The taper is
        // an ellipse's own profile, so the patch of shade is a lens with the cloud's underside
        // written along it.
        let across = 2.0 * u - 1.0;
        let taper = (1.0 - across * across).max(0.0).sqrt();
        let half = depth * carry * taper * 0.5;
        let y0 = (row - half).floor().clamp(horizon, field.height as f32) as u32;
        let y1 = (row + half).ceil().clamp(horizon, field.height as f32) as u32;
        for y in y0..y1 {
            let at = (y as usize) * (field.width as usize) + col as usize;
            field.cells[at] = shade(field.cells[at]);
        }
    }
}

// ---------------------------------------------------------------------------------------
// the plugin
// ---------------------------------------------------------------------------------------

/// Installs the sky, the desert under it and the weather that crosses it.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut().insert_resource(Field::default());
        app.world_mut().insert_resource(Backdrop::default());
        app.world_mut().insert_resource(Sky::default());
        app.world_mut().insert_resource(Motion::default());
        app.add_systems(Startup, open);
        app.add_systems(
            FixedUpdate,
            (apply_resize, steer, drift, tend_forge, repaint).chain(),
        );
    }
}

/// Note the seed the desert is drawn from. The weather cannot be grown yet: how big a cloud is
/// depends on the window, and no window has reported its size at startup.
pub fn open(mut sky: ResMut<Sky>, mut rng: ResMut<SimRng>) {
    sky.seed = rng.u64();
}

/// Fill an empty sky with its clouds. Every one is built in full here rather than on the
/// allowance, since this happens on the first tick, when there is no frame to drop and a piece
/// that opens on an empty sky opens on nothing.
pub fn populate(sky: &mut Sky, sky_height: f32, rng: &mut SimRng) {
    sky.sizes = (0..TIER_COUNT).map(|t| tier_size(t, sky_height)).collect();
    for (slot, &tier) in DEPTHS.iter().enumerate() {
        let spec = TIERS[tier];
        sky.shapes.push(forge_now(slot, tier, sky.sizes[tier], rng));
        sky.drifters.push(Drifter {
            tier,
            x: 0.0,
            lift: spec.lift * rng.range_f32(0.8..1.25),
            speed: spec.speed * sky_height * rng.range_f32(0.85..1.2),
        });
        sky.grown += 1;
    }
}

/// Rebuild the desert when the window changes shape, grow the weather the first time a real
/// window size arrives, and put every cloud back in the queue when the sky it has to fill
/// changes size under it.
fn apply_resize(
    mut field: ResMut<Field>,
    mut backdrop: ResMut<Backdrop>,
    mut sky: ResMut<Sky>,
    mut rng: ResMut<SimRng>,
    mut orders: EventReader<CommandEvent>,
) {
    for order in orders.read() {
        if order.name != RESIZE_COMMAND {
            continue;
        }
        let Some((width, height)) = parse_window(&order.payload) else {
            continue;
        };
        if (width, height) == (field.width, field.height) {
            continue;
        }
        field.width = width;
        field.height = height;
        field.cells = vec![0; (width as usize) * (height as usize)];
        *backdrop = build_backdrop(width, height, sky.seed);
        let sky_height = backdrop.horizon as f32;
        if !sky.placed {
            sky.placed = true;
            populate(&mut sky, sky_height, &mut rng);
            place(&mut sky, width as f32);
            continue;
        }
        // The clouds are already in the air, so the new ones are grown on the allowance and
        // each replaces its own as it is finished. Rebuilding them all where they stand would
        // cost a second of frames, which is the one thing a resize must not do.
        let wanted: Vec<(u32, u32)> = (0..TIER_COUNT).map(|t| tier_size(t, sky_height)).collect();
        if wanted == sky.sizes {
            continue;
        }
        sky.sizes = wanted;
        for index in 0..sky.drifters.len() {
            let tier = sky.drifters[index].tier;
            sky.drifters[index].speed = TIERS[tier].speed * sky_height * rng.range_f32(0.85..1.2);
            if !sky.waiting.contains(&index) {
                sky.waiting.push_back(index);
            }
        }
    }
}

/// Deal the clouds out across the window so the piece opens on weather rather than on one
/// cloud. Spaced by the golden ratio, which is the cheapest way to scatter a handful of things
/// without two of them landing on top of each other.
pub fn place(sky: &mut Sky, width: f32) {
    for index in 0..sky.drifters.len() {
        let span = width + sky.shapes[index].width as f32;
        let share = (index as f32 * 0.618_034).fract();
        sky.drifters[index].x = -(sky.shapes[index].width as f32) + span * share;
    }
}

/// Up and down set the pace, `Space` holds the sky still.
fn steer(input: Res<Input>, time: Res<Time>, mut motion: ResMut<Motion>) {
    let ramp = PACE_RAMP.powf(time.fixed_delta);
    if input.pressed(Key::Up) {
        motion.pace = (motion.pace * ramp).min(PACE_MAX);
    }
    if input.pressed(Key::Down) {
        motion.pace = (motion.pace / ramp).max(PACE_MIN);
    }
    if input.just_pressed(Key::Space) {
        motion.held = !motion.held;
    }
}

/// Carry the clouds downwind, and send one back to the right when it has gone off the left.
fn drift(
    time: Res<Time>,
    motion: Res<Motion>,
    field: Res<Field>,
    mut sky: ResMut<Sky>,
    mut rng: ResMut<SimRng>,
) {
    if motion.held || field.width == 0 || !sky.placed {
        return;
    }
    let step = time.fixed_delta * motion.pace;
    for index in 0..sky.drifters.len() {
        let width = sky.shapes[index].width as f32;
        let drifter = &mut sky.drifters[index];
        drifter.x -= drifter.speed * step;
        if drifter.x + width >= 0.0 {
            continue;
        }
        // Back in from the right, and a new shape to wear when it gets there. It has its own
        // width to cross before any of it shows, which is a great deal longer than growing a
        // cloud takes, so the new one is always ready in time.
        let spec = TIERS[drifter.tier];
        drifter.x = field.width as f32 + width * GAP;
        drifter.lift = spec.lift * rng.range_f32(0.8..1.25);
        drifter.speed = spec.speed * backdrop_height(&field) * rng.range_f32(0.85..1.2);
        if !sky.waiting.contains(&index) {
            sky.waiting.push_back(index);
        }
    }
}

/// How tall the sky is under the current window. Taken from the window rather than from the
/// [`Backdrop`], so that a cloud's speed is settled by the same rule wherever it is asked.
fn backdrop_height(field: &Field) -> f32 {
    (field.height as f32 * HORIZON).max(1.0)
}

/// Spend the tick's allowance on whatever cloud is being grown, and start the next one when
/// that is finished.
fn tend_forge(mut sky: ResMut<Sky>, mut rng: ResMut<SimRng>) {
    if sky.forge.is_none()
        && let Some(next) = sky.waiting.pop_front()
    {
        let tier = sky.drifters[next].tier;
        let size = sky.sizes[tier];
        sky.forge = Some(Forge::start(next, tier, size, &mut rng));
    }
    let Some(forge) = &mut sky.forge else { return };
    forge.work(FORGE_BUDGET);
    if forge.done() {
        let forge = sky.forge.take().expect("just checked");
        let target = forge.target;
        sky.shapes[target] = forge.take();
        sky.grown += 1;
    }
}

/// Draw the whole window again.
fn repaint(mut field: ResMut<Field>, backdrop: Res<Backdrop>, sky: Res<Sky>) {
    compose(&mut field, &backdrop, &sky);
}
