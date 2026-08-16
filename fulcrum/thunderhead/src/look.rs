//! What the materials look like: five palettes, and the table that turns one into colour.
//!
//! Kept beside the simulation rather than inside the binary because two programs need it. The
//! binary paints the window with it, and the `thunderhead_still` example writes a frame to a PNG with it,
//! which is how the look was tuned in the first place. Nothing in `game.rs` reads this file:
//! the picture is decided there, the colour here.
//!
//! Every value is sRGB, and it goes to the GPU as sRGB bytes in an sRGB texture, so nothing in
//! this piece converts anything. That is worth saying because the temptation is to be clever
//! about linear light, and the moment these numbers are treated as linear the whole desert
//! washes out.

use crate::game::{
    CLOUD_BANDS, CLOUD_FIRST, CLOUD_STRIDE, GROUND_BANDS, GROUND_FIRST, INK, MESA_DEPTHS,
    MESA_FIRST, SHADOW_FIRST, SKY_BANDS, SKY_FIRST, STONE, SUN, TIER_COUNT,
};

/// One palette. Moebius worked in flat areas of colour with a clean line around them, and the
/// only thing that decides whether a picture reads as his is which colours those areas are, so
/// the piece keeps them as data and nothing else.
pub struct Look {
    /// Its name, for the readout.
    pub name: &'static str,
    /// Rock standing on the horizon, before the distance is allowed at it.
    pub rock: [f32; 3],
    /// The line. One colour for every contour in the picture.
    pub ink: [f32; 3],
    /// The sky, from the zenith down to the horizon.
    pub sky: &'static [[f32; 3]],
    /// The sun's disc.
    pub sun: [f32; 3],
    /// The desert, from the horizon down to the viewer's feet.
    pub ground: &'static [[f32; 3]],
    /// What sand mixes towards where a cloud's shadow crosses it.
    pub shadow: [f32; 3],
    /// How far it mixes.
    pub dusk: f32,
    /// A cloud, from its rain-dark base up to its lit crown.
    pub cloud: &'static [[f32; 3]],
    /// What distance fades things towards. Near the colour of the sky at the horizon, since
    /// that is the air being looked through.
    pub haze: [f32; 3],
}

/// How far each cloud tier is faded into the haze, far tier first.
///
/// Aerial perspective, and the only depth cue the piece has beyond size and speed. It is doing
/// more work than it looks: three tiers of white cloud on one flat sky read as three clouds at
/// one distance, and the same three with this applied read as weather going back for miles.
const TIER_HAZE: [f32; TIER_COUNT] = [0.56, 0.27, 0.0];

/// The same, for the two distances of rock on the horizon.
const MESA_HAZE: [f32; MESA_DEPTHS as usize] = [0.34, 0.60];

/// Five palettes. Each one is a *whole* sky: the clouds have to sit against their own sky and
/// throw their own shadow onto their own sand, so a palette is nine ramps that were chosen
/// together rather than nine colours that happen to be nearby.
pub const LOOKS: &[Look] = &[
    // The evening the piece was drawn for: a turquoise sky going gold at the horizon, ochre
    // sand, and thunderheads carrying the sunset up their western faces.
    Look {
        name: "arzach",
        rock: [0.30, 0.24, 0.44],
        ink: [0.10, 0.07, 0.13],
        sky: &[
            [0.10, 0.22, 0.45],
            [0.13, 0.40, 0.62],
            [0.20, 0.60, 0.70],
            [0.42, 0.78, 0.75],
            [0.78, 0.86, 0.70],
            [0.98, 0.84, 0.55],
        ],
        sun: [1.00, 0.96, 0.80],
        ground: &[
            [0.93, 0.72, 0.44],
            [0.88, 0.58, 0.33],
            [0.80, 0.44, 0.27],
            [0.68, 0.33, 0.24],
            [0.55, 0.25, 0.22],
        ],
        shadow: [0.32, 0.20, 0.34],
        dusk: 0.42,
        cloud: &[
            [0.28, 0.18, 0.42],
            [0.48, 0.24, 0.50],
            [0.72, 0.32, 0.52],
            [0.92, 0.48, 0.50],
            [0.99, 0.68, 0.55],
            [1.00, 0.86, 0.72],
            [1.00, 0.97, 0.92],
        ],
        haze: [0.80, 0.85, 0.72],
    },
    // Cool and green: the light of a world with a different star, and the one palette here
    // where the clouds are colder than the ground they cross.
    Look {
        name: "verdigris",
        rock: [0.20, 0.32, 0.40],
        ink: [0.08, 0.12, 0.14],
        sky: &[
            [0.05, 0.30, 0.42],
            [0.10, 0.48, 0.55],
            [0.24, 0.68, 0.66],
            [0.50, 0.83, 0.74],
            [0.78, 0.92, 0.80],
            [0.93, 0.96, 0.83],
        ],
        sun: [0.97, 1.00, 0.90],
        ground: &[
            [0.86, 0.83, 0.60],
            [0.80, 0.74, 0.48],
            [0.72, 0.63, 0.40],
            [0.60, 0.52, 0.35],
            [0.48, 0.43, 0.32],
        ],
        shadow: [0.24, 0.38, 0.42],
        dusk: 0.40,
        cloud: &[
            [0.20, 0.34, 0.46],
            [0.32, 0.50, 0.58],
            [0.48, 0.68, 0.70],
            [0.66, 0.84, 0.80],
            [0.84, 0.93, 0.88],
            [0.96, 0.98, 0.95],
            [1.00, 1.00, 1.00],
        ],
        haze: [0.86, 0.93, 0.80],
    },
    // Noon on a hot world: the sky bleached out at the horizon, the sand red, and the clouds
    // going up through orange into white.
    Look {
        name: "ember",
        rock: [0.40, 0.20, 0.34],
        ink: [0.16, 0.06, 0.06],
        sky: &[
            [0.30, 0.20, 0.45],
            [0.55, 0.32, 0.48],
            [0.80, 0.48, 0.42],
            [0.94, 0.66, 0.42],
            [1.00, 0.82, 0.52],
            [1.00, 0.93, 0.70],
        ],
        sun: [1.00, 0.98, 0.86],
        ground: &[
            [0.94, 0.66, 0.38],
            [0.90, 0.50, 0.28],
            [0.82, 0.36, 0.22],
            [0.68, 0.26, 0.20],
            [0.52, 0.18, 0.18],
        ],
        shadow: [0.40, 0.14, 0.24],
        dusk: 0.45,
        cloud: &[
            [0.36, 0.14, 0.30],
            [0.58, 0.20, 0.32],
            [0.80, 0.32, 0.30],
            [0.95, 0.52, 0.32],
            [1.00, 0.74, 0.46],
            [1.00, 0.90, 0.72],
            [1.00, 0.99, 0.94],
        ],
        haze: [0.98, 0.86, 0.60],
    },
    // The half hour after the sun has gone: everything is one of two colours, and the clouds
    // are the only things still lit.
    Look {
        name: "nocturne",
        rock: [0.10, 0.10, 0.22],
        ink: [0.06, 0.05, 0.12],
        sky: &[
            [0.03, 0.04, 0.16],
            [0.06, 0.09, 0.28],
            [0.12, 0.17, 0.42],
            [0.22, 0.30, 0.55],
            [0.40, 0.44, 0.66],
            [0.62, 0.58, 0.72],
        ],
        sun: [0.90, 0.92, 1.00],
        ground: &[
            [0.44, 0.40, 0.58],
            [0.36, 0.31, 0.48],
            [0.29, 0.24, 0.39],
            [0.22, 0.18, 0.30],
            [0.16, 0.13, 0.23],
        ],
        shadow: [0.08, 0.07, 0.16],
        dusk: 0.50,
        cloud: &[
            [0.12, 0.12, 0.28],
            [0.22, 0.20, 0.40],
            [0.36, 0.30, 0.52],
            [0.52, 0.44, 0.64],
            [0.70, 0.62, 0.78],
            [0.86, 0.82, 0.92],
            [0.97, 0.96, 1.00],
        ],
        haze: [0.45, 0.46, 0.64],
    },
    // Flat cyan, flat pink, and a violet in the clouds that belongs to neither: the loudest of
    // the five, and the one that looks most like ink on paper.
    Look {
        name: "mineral",
        rock: [0.24, 0.26, 0.48],
        ink: [0.10, 0.10, 0.14],
        sky: &[
            [0.06, 0.44, 0.62],
            [0.10, 0.60, 0.72],
            [0.24, 0.76, 0.80],
            [0.52, 0.88, 0.86],
            [0.80, 0.94, 0.88],
            [0.96, 0.92, 0.78],
        ],
        sun: [1.00, 0.94, 0.68],
        ground: &[
            [0.94, 0.76, 0.66],
            [0.92, 0.62, 0.56],
            [0.86, 0.48, 0.48],
            [0.74, 0.38, 0.44],
            [0.58, 0.30, 0.40],
        ],
        shadow: [0.30, 0.28, 0.52],
        dusk: 0.42,
        cloud: &[
            [0.24, 0.26, 0.52],
            [0.40, 0.32, 0.62],
            [0.62, 0.40, 0.66],
            [0.85, 0.52, 0.62],
            [0.98, 0.70, 0.62],
            [1.00, 0.88, 0.76],
            [1.00, 0.98, 0.94],
        ],
        haze: [0.86, 0.92, 0.84],
    },
];

/// A colour from a ramp, `at` running `0..=1` from its first stop to its last.
fn along(stops: &[[f32; 3]], at: f32) -> [f32; 3] {
    let last = stops.len() - 1;
    let place = at.clamp(0.0, 1.0) * last as f32;
    let low = (place as usize).min(last);
    let high = (low + 1).min(last);
    let blend = place - low as f32;
    let mut out = [0.0; 3];
    for channel in 0..3 {
        out[channel] = stops[low][channel] + (stops[high][channel] - stops[low][channel]) * blend;
    }
    out
}

/// Two colours mixed, `by` of the way from the first to the second.
fn toward(from: [f32; 3], to: [f32; 3], by: f32) -> [f32; 3] {
    let by = by.clamp(0.0, 1.0);
    let mut out = [0.0; 3];
    for channel in 0..3 {
        out[channel] = from[channel] + (to[channel] - from[channel]) * by;
    }
    out
}

/// One band, evenly spaced through a set of `count` of them.
fn band(stops: &[[f32; 3]], index: u8, count: u8) -> [f32; 3] {
    along(stops, index as f32 / (count - 1).max(1) as f32)
}

/// Every material's colour, as RGBA bytes ready for an sRGB texture.
///
/// The whole palette is sixty-six colours, so this is rebuilt whenever the viewer presses the key and
/// the entire picture recolours on the next frame, however many million pixels it covers.
pub fn lut(look: &Look) -> [[u8; 4]; 256] {
    let mut table = [[0u8, 0, 0, 255]; 256];
    let mut put = |index: u8, rgb: [f32; 3]| {
        table[index as usize] = [
            (rgb[0].clamp(0.0, 1.0) * 255.0).round() as u8,
            (rgb[1].clamp(0.0, 1.0) * 255.0).round() as u8,
            (rgb[2].clamp(0.0, 1.0) * 255.0).round() as u8,
            255,
        ];
    };

    put(INK, look.ink);
    put(SUN, look.sun);
    for index in 0..SKY_BANDS {
        put(SKY_FIRST + index, band(look.sky, index, SKY_BANDS));
    }
    for index in 0..GROUND_BANDS {
        let sand = band(look.ground, index, GROUND_BANDS);
        put(GROUND_FIRST + index, sand);
        put(SHADOW_FIRST + index, toward(sand, look.shadow, look.dusk));
    }
    // Rock on the horizon is a shape in the air rather than a thing with a colour, so the
    // palette names it outright: dark, and already halfway to the sky's own hue, before the
    // distance takes it the rest of the way.
    for depth in 0..MESA_DEPTHS {
        put(
            MESA_FIRST + depth,
            toward(look.rock, look.haze, MESA_HAZE[depth as usize]),
        );
    }
    put(
        STONE,
        toward(look.ink, look.ground[look.ground.len() - 1], 0.4),
    );

    for (tier, &haze) in TIER_HAZE.iter().enumerate() {
        let first = CLOUD_FIRST + tier as u8 * CLOUD_STRIDE;
        for index in 0..CLOUD_BANDS {
            let lit = band(look.cloud, index, CLOUD_BANDS);
            put(first + index, toward(lit, look.haze, haze));
        }
        // A line drawn on something a long way off is a line seen through the same air, so it
        // fades harder than the shape it draws. Without this the far clouds keep a crisp black
        // outline and stop being far.
        put(
            first + CLOUD_BANDS,
            toward(look.ink, look.haze, (haze * 1.35).min(0.85)),
        );
    }
    table
}
