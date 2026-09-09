//! The palettes: what the water is coloured with.
//!
//! Each is five stops from its deepest to its brightest, written as display values, the
//! numbers you would type into a paint program. The shader blends between them and takes the
//! display curve off at the very end, so every blend is between the colours as written
//! rather than between the physics of adding light, and a ramp from an indigo to an apricot
//! passes through the violets a person would expect.
//!
//! All of them are analogous, neighbours on the wheel, rather than complementary. Contrast
//! draws the eye and holds it, which is the wrong thing here; a narrow band of hues lets the
//! eye rest anywhere. Every palette has a dark bottom, because the water is mostly bottom: a
//! ring is a bright edge on a dark field, and a field that is bright everywhere has no room
//! for a ring to be seen against.

/// One palette.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub name: &'static str,
    /// Deepest to brightest.
    pub stops: [[f32; 3]; 5],
}

/// The palettes, in the order `c` walks them.
pub const LOOKS: [Look; 6] = [
    Look {
        name: "dusk",
        stops: [
            [0.05, 0.04, 0.16],
            [0.22, 0.11, 0.38],
            [0.56, 0.22, 0.44],
            [0.93, 0.48, 0.40],
            [1.00, 0.84, 0.64],
        ],
    },
    Look {
        name: "lagoon",
        stops: [
            [0.01, 0.10, 0.17],
            [0.03, 0.28, 0.36],
            [0.09, 0.54, 0.56],
            [0.44, 0.81, 0.73],
            [0.91, 0.97, 0.88],
        ],
    },
    Look {
        name: "ember",
        stops: [
            [0.11, 0.02, 0.06],
            [0.36, 0.05, 0.10],
            [0.72, 0.19, 0.10],
            [0.96, 0.55, 0.18],
            [1.00, 0.89, 0.58],
        ],
    },
    Look {
        name: "moon",
        stops: [
            [0.03, 0.04, 0.08],
            [0.09, 0.12, 0.22],
            [0.25, 0.31, 0.44],
            [0.57, 0.63, 0.72],
            [0.95, 0.96, 0.99],
        ],
    },
    Look {
        name: "meadow",
        stops: [
            [0.02, 0.08, 0.06],
            [0.07, 0.23, 0.14],
            [0.21, 0.47, 0.24],
            [0.60, 0.78, 0.36],
            [0.97, 0.98, 0.74],
        ],
    },
    Look {
        name: "aurora",
        stops: [
            [0.02, 0.03, 0.11],
            [0.04, 0.19, 0.30],
            [0.05, 0.54, 0.46],
            [0.42, 0.86, 0.62],
            [0.88, 0.78, 0.96],
        ],
    },
];

/// A blend of two palettes, `t` of the way from the first to the second.
pub fn blend(from: &Look, to: &Look, t: f32) -> [[f32; 3]; 5] {
    let t = t.clamp(0.0, 1.0);
    let mut stops = from.stops;
    for (stop, target) in stops.iter_mut().zip(to.stops.iter()) {
        for (channel, want) in stop.iter_mut().zip(target.iter()) {
            *channel += (want - *channel) * t;
        }
    }
    stops
}

/// A colour from a set of stops at `position` in `0..1`, as the shader has it: eased
/// between neighbouring stops.
pub fn sample(stops: &[[f32; 3]; 5], position: f32) -> [f32; 3] {
    let scaled = position.clamp(0.0, 1.0) * (stops.len() - 1) as f32;
    let low = scaled.floor() as usize;
    let high = (low + 1).min(stops.len() - 1);
    let t = scaled - low as f32;
    let blend = t * t * (3.0 - 2.0 * t);
    [
        stops[low][0] + (stops[high][0] - stops[low][0]) * blend,
        stops[low][1] + (stops[high][1] - stops[low][1]) * blend,
        stops[low][2] + (stops[high][2] - stops[low][2]) * blend,
    ]
}
