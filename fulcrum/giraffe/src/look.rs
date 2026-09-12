//! The palettes: one colour for each of the four strategies, a night for the ground, and a
//! lamp.
//!
//! Every palette is laid out the same way, so that once you have read one you have read them
//! all. Voice is hue: the cells that say what they need are warm, the silent ones are cool.
//! Stance is lightness: sharers are the lighter of each pair, takers the darker. So the
//! lightest, warmest colour on the board is always the sharer who says what it needs, and the
//! darkest is the silent taker, and you can tell the four apart with the colour names
//! forgotten.
//!
//! Colours are written in sRGB, which is how they were picked, and converted to linear light
//! once by [`palette`], so that every blend on the way to the screen is between the colours as
//! written.

use crate::game::Strategy;
use fulcrum::prelude::*;

/// A palette, in sRGB.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub name: &'static str,
    /// The ground behind the tiles.
    pub night: [f32; 3],
    /// One tone per strategy, in [`Strategy::index`] order: silent sharer, silent taker,
    /// speaking sharer, speaking taker.
    pub tones: [[f32; 3]; 4],
    /// The light that follows the pointer.
    pub lamp: [f32; 3],
}

/// The same, in linear light and ready to blend.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub night: [f32; 3],
    pub tones: [[f32; 3]; 4],
    pub lamp: [f32; 3],
}

/// The palettes, in the order `c` walks them.
pub const LOOKS: [Look; 6] = [
    Look {
        name: "dusk",
        night: [0.043, 0.051, 0.102],
        tones: [
            [0.235, 0.541, 0.541],
            [0.165, 0.184, 0.369],
            [0.949, 0.757, 0.306],
            [0.710, 0.325, 0.235],
        ],
        lamp: [1.0, 0.851, 0.627],
    },
    Look {
        name: "lagoon",
        night: [0.024, 0.078, 0.102],
        tones: [
            [0.310, 0.702, 0.627],
            [0.122, 0.227, 0.373],
            [0.980, 0.862, 0.520],
            [0.878, 0.478, 0.373],
        ],
        lamp: [0.851, 1.0, 0.949],
    },
    Look {
        name: "ember",
        night: [0.078, 0.039, 0.031],
        tones: [
            [0.353, 0.420, 0.478],
            [0.169, 0.169, 0.200],
            [1.0, 0.702, 0.278],
            [0.659, 0.196, 0.243],
        ],
        lamp: [1.0, 0.769, 0.549],
    },
    Look {
        name: "moss",
        night: [0.039, 0.071, 0.043],
        tones: [
            [0.498, 0.639, 0.478],
            [0.122, 0.239, 0.169],
            [0.965, 0.886, 0.478],
            [0.722, 0.463, 0.227],
        ],
        lamp: [0.918, 1.0, 0.816],
    },
    Look {
        name: "moon",
        night: [0.047, 0.047, 0.063],
        tones: [
            [0.663, 0.706, 0.761],
            [0.239, 0.267, 0.322],
            [0.975, 0.905, 0.690],
            [0.788, 0.471, 0.561],
        ],
        lamp: [1.0, 1.0, 1.0],
    },
    Look {
        name: "aurora",
        night: [0.020, 0.039, 0.078],
        tones: [
            [0.431, 0.906, 0.718],
            [0.298, 0.227, 0.541],
            [1.0, 0.839, 0.647],
            [0.753, 0.294, 0.612],
        ],
        lamp: [0.816, 0.941, 1.0],
    },
];

/// One sRGB channel in linear light.
pub fn linear(channel: f32) -> f32 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

/// A colour in sRGB, as the renderer takes it.
pub fn to_linear(srgb: [f32; 3]) -> [f32; 3] {
    [linear(srgb[0]), linear(srgb[1]), linear(srgb[2])]
}

/// Relative luminance of a linear colour: how light it looks.
pub fn luminance(rgb: [f32; 3]) -> f32 {
    0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]
}

/// How warm a colour is: red over blue. Positive is warm.
pub fn warmth(rgb: [f32; 3]) -> f32 {
    rgb[0] - rgb[2]
}

/// A palette in linear light.
pub fn palette(look: &Look) -> Palette {
    Palette {
        night: to_linear(look.night),
        tones: look.tones.map(to_linear),
        lamp: to_linear(look.lamp),
    }
}

/// `t` of the way from one palette to another, channel by channel.
pub fn blend(from: &Palette, to: &Palette, t: f32) -> Palette {
    let t = t.clamp(0.0, 1.0);
    let mix = |a: [f32; 3], b: [f32; 3]| [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t);
    Palette {
        night: mix(from.night, to.night),
        tones: [0, 1, 2, 3].map(|i| mix(from.tones[i], to.tones[i])),
        lamp: mix(from.lamp, to.lamp),
    }
}

/// The tone a strategy is drawn in.
pub fn tone(palette: &Palette, strategy: Strategy) -> [f32; 3] {
    palette.tones[strategy.index()]
}

/// A linear colour, scaled and given an alpha, as a renderer [`Color`].
pub fn colour(rgb: [f32; 3], scale: f32, alpha: f32) -> Color {
    Color::rgba(
        (rgb[0] * scale).min(1.0),
        (rgb[1] * scale).min(1.0),
        (rgb[2] * scale).min(1.0),
        alpha,
    )
}
