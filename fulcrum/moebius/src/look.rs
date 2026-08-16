//! Five palettes, as flat colour.
//!
//! Nothing in here is a light value and nothing is tone mapped. These are the colours that land
//! on the screen, because every enclosed region in this drawing is filled with one of them. No
//! colour is mixed with another to suggest that a surface is turning away from the light: a
//! cloud is one colour, the cloud on top of it is one colour, and the line between them is what
//! says there are two.
//!
//! Nor is a band of sky a step off a gradient. Five colours are chosen for the sky and three for
//! the sand, the way an artist hands a printer a list of flat inks, so the colour halfway up is
//! a decision rather than the average of the two ends. That is the difference between a stepped
//! ramp and a separation, and it is most of what makes these skies look drawn.
//!
//! Fifteen colours to a palette: five sky, three sand, four cloud, plus the rock, the sun and
//! the line.

/// One sky, whole.
pub struct Look {
    /// Its name, for the readout.
    pub name: &'static str,
    /// The line. Every line in the picture is this colour and this weight.
    pub ink: [f32; 3],
    /// The five flat colours of the sky, horizon first and zenith last.
    pub sky: [[f32; 3]; 5],
    /// The three flat colours of the desert, underfoot first and horizon last.
    pub sand: [[f32; 3]; 3],
    /// The sun's disc.
    pub sun: [f32; 3],
    /// Rock standing on the horizon.
    pub mesa: [f32; 3],
    /// The four flat colours the clouds are filled with, far band first.
    pub cloud: [[f32; 3]; 4],
    /// How far above the horizon the sun stands, in radians.
    pub elevation: f32,
    /// Where it stands around the compass, in radians, measured the way the heading is.
    pub azimuth: f32,
}

impl Look {
    /// The direction towards the sun.
    pub fn sun_direction(&self) -> [f32; 3] {
        let (rise, run) = self.elevation.sin_cos();
        let (across, along) = self.azimuth.sin_cos();
        [across * run, rise, along * run]
    }
}

/// The five.
pub const LOOKS: &[Look] = &[
    // The evening the whole set of pieces was drawn for: gold along the horizon, turquoise
    // overhead, and a mauve between them that belongs to neither end.
    Look {
        name: "arzach",
        ink: [0.10, 0.07, 0.13],
        sky: [
            [0.99, 0.86, 0.56],
            [0.97, 0.74, 0.52],
            [0.63, 0.57, 0.67],
            [0.25, 0.37, 0.63],
            [0.10, 0.22, 0.48],
        ],
        sand: [[0.55, 0.25, 0.22], [0.88, 0.62, 0.38], [0.95, 0.80, 0.55]],
        sun: [1.00, 0.97, 0.84],
        mesa: [0.42, 0.34, 0.52],
        cloud: [
            [0.66, 0.49, 0.63],
            [0.86, 0.60, 0.59],
            [0.99, 0.79, 0.67],
            [1.00, 0.96, 0.90],
        ],
        elevation: 0.15,
        azimuth: 0.34,
    },
    // Cool and green: the light of a world with a different star, and the one palette where the
    // clouds are colder than the ground they cross.
    Look {
        name: "verdigris",
        ink: [0.07, 0.12, 0.14],
        sky: [
            [0.93, 0.96, 0.83],
            [0.74, 0.90, 0.78],
            [0.43, 0.73, 0.67],
            [0.15, 0.51, 0.55],
            [0.05, 0.32, 0.44],
        ],
        sand: [[0.48, 0.43, 0.32], [0.76, 0.74, 0.52], [0.88, 0.87, 0.66]],
        sun: [0.99, 1.00, 0.92],
        mesa: [0.30, 0.42, 0.46],
        cloud: [
            [0.55, 0.74, 0.73],
            [0.71, 0.86, 0.81],
            [0.87, 0.94, 0.88],
            [1.00, 1.00, 0.98],
        ],
        elevation: 0.31,
        azimuth: -0.28,
    },
    // Noon on a hot world: the sky bleached out along the horizon, the sand red, and the clouds
    // separated into orange and white.
    Look {
        name: "ember",
        ink: [0.16, 0.06, 0.06],
        sky: [
            [1.00, 0.93, 0.70],
            [1.00, 0.80, 0.52],
            [0.88, 0.55, 0.44],
            [0.58, 0.34, 0.48],
            [0.32, 0.21, 0.47],
        ],
        sand: [[0.52, 0.18, 0.18], [0.86, 0.48, 0.26], [0.96, 0.70, 0.42]],
        sun: [1.00, 0.99, 0.90],
        mesa: [0.50, 0.26, 0.34],
        cloud: [
            [0.87, 0.52, 0.43],
            [0.98, 0.67, 0.45],
            [1.00, 0.84, 0.62],
            [1.00, 0.98, 0.92],
        ],
        elevation: 0.62,
        azimuth: 0.40,
    },
    // The half hour after the sun has gone: the whole picture within one narrow range, and the
    // clouds the only things with any light left in them.
    Look {
        name: "nocturne",
        // The one palette where the line has to be watched. Everything else here is within a
        // narrow range, and a zenith that goes much darker than this takes the line at the top
        // of the sky down with it.
        ink: [0.015, 0.015, 0.05],
        sky: [
            [0.62, 0.55, 0.70],
            [0.45, 0.40, 0.60],
            [0.28, 0.26, 0.46],
            [0.16, 0.16, 0.34],
            [0.07, 0.08, 0.24],
        ],
        sand: [[0.14, 0.11, 0.21], [0.27, 0.24, 0.38], [0.36, 0.32, 0.48]],
        sun: [0.95, 0.95, 1.00],
        mesa: [0.14, 0.13, 0.26],
        cloud: [
            [0.44, 0.40, 0.60],
            [0.59, 0.54, 0.71],
            [0.75, 0.70, 0.83],
            [0.94, 0.93, 1.00],
        ],
        elevation: 0.20,
        azimuth: 0.50,
    },
    // The loudest: flat cyan against flat pink, with a violet in the clouds that belongs to
    // neither of them.
    Look {
        name: "mineral",
        ink: [0.09, 0.09, 0.14],
        sky: [
            [0.96, 0.92, 0.78],
            [0.87, 0.87, 0.80],
            [0.56, 0.77, 0.81],
            [0.24, 0.61, 0.73],
            [0.06, 0.46, 0.64],
        ],
        sand: [[0.58, 0.30, 0.40], [0.85, 0.60, 0.56], [0.94, 0.78, 0.68]],
        sun: [1.00, 0.95, 0.72],
        mesa: [0.34, 0.32, 0.54],
        cloud: [
            [0.62, 0.57, 0.79],
            [0.81, 0.65, 0.81],
            [0.97, 0.80, 0.77],
            [1.00, 0.97, 0.93],
        ],
        elevation: 0.24,
        azimuth: 0.30,
    },
];
