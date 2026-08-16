//! Twenty palettes, as flat colour.
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
//!
//! The first five are the ones the original piece was drawn with, and the fifteen after them are
//! new. Every one of them is held to the same four rules by `tests/sky.rs`: the line is the
//! darkest thing in the picture, the sky darkens from the horizon upwards, the sand lightens
//! into the distance, and the four cloud colours are four rather than two. A palette that breaks
//! one of those is not a palette with an unusual mood, it is a picture with a hole in it, so the
//! tests are where a new one is checked rather than the screen.

/// One sky, whole.
pub struct Look {
    /// Its name, for the readout.
    pub name: &'static str,
    /// The line. Every line in the picture is this colour.
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

/// All of them.
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
    // Cool and green: the light of a world with a different star, and one of the two palettes
    // where the clouds are colder than the ground they cross.
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
        // One of the two palettes where the line has to be watched. Everything else here is
        // within a narrow range, and a zenith that goes much darker than this takes the line at
        // the top of the sky down with it.
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
    // Yellow-green along the horizon going to a deep teal, with the sand kept dull so the sky
    // has the whole of the colour.
    Look {
        name: "citron",
        ink: [0.06, 0.10, 0.10],
        sky: [
            [0.96, 0.95, 0.64],
            [0.88, 0.92, 0.62],
            [0.60, 0.80, 0.62],
            [0.30, 0.58, 0.52],
            [0.10, 0.34, 0.40],
        ],
        sand: [[0.42, 0.36, 0.24], [0.70, 0.62, 0.36], [0.86, 0.80, 0.52]],
        sun: [1.00, 1.00, 0.86],
        mesa: [0.28, 0.36, 0.38],
        cloud: [
            [0.52, 0.66, 0.60],
            [0.70, 0.80, 0.66],
            [0.88, 0.90, 0.72],
            [1.00, 0.98, 0.88],
        ],
        elevation: 0.28,
        azimuth: -0.60,
    },
    // Shallow water seen from above: aqua overhead, and a warm pale sand under it.
    Look {
        name: "lagoon",
        ink: [0.05, 0.11, 0.13],
        sky: [
            [0.95, 0.93, 0.72],
            [0.72, 0.90, 0.86],
            [0.40, 0.76, 0.80],
            [0.16, 0.52, 0.68],
            [0.05, 0.28, 0.50],
        ],
        sand: [[0.40, 0.34, 0.30], [0.72, 0.62, 0.48], [0.90, 0.82, 0.64]],
        sun: [1.00, 0.99, 0.90],
        mesa: [0.30, 0.38, 0.44],
        cloud: [
            [0.46, 0.68, 0.72],
            [0.66, 0.82, 0.82],
            [0.86, 0.93, 0.90],
            [1.00, 0.99, 0.94],
        ],
        elevation: 0.18,
        azimuth: 1.10,
    },
    // Dust in the air all the way up: the one palette with no cool colour anywhere in it.
    Look {
        name: "sirocco",
        ink: [0.14, 0.07, 0.05],
        sky: [
            [0.99, 0.88, 0.58],
            [0.94, 0.80, 0.52],
            [0.80, 0.62, 0.40],
            [0.58, 0.42, 0.32],
            [0.34, 0.24, 0.24],
        ],
        sand: [[0.34, 0.22, 0.16], [0.62, 0.44, 0.28], [0.82, 0.64, 0.42]],
        sun: [1.00, 0.96, 0.80],
        mesa: [0.44, 0.28, 0.22],
        cloud: [
            [0.70, 0.54, 0.42],
            [0.86, 0.68, 0.50],
            [0.96, 0.82, 0.62],
            [1.00, 0.94, 0.82],
        ],
        elevation: 0.45,
        azimuth: 0.90,
    },
    // Pink into a deep violet, with the rock carrying the violet down to the ground.
    Look {
        name: "orchid",
        ink: [0.11, 0.05, 0.13],
        sky: [
            [1.00, 0.86, 0.72],
            [0.98, 0.76, 0.78],
            [0.82, 0.56, 0.72],
            [0.54, 0.34, 0.62],
            [0.26, 0.18, 0.44],
        ],
        sand: [[0.36, 0.24, 0.34], [0.66, 0.46, 0.50], [0.88, 0.70, 0.66]],
        sun: [1.00, 0.97, 0.90],
        mesa: [0.38, 0.26, 0.46],
        cloud: [
            [0.66, 0.48, 0.68],
            [0.82, 0.62, 0.76],
            [0.94, 0.78, 0.82],
            [1.00, 0.94, 0.96],
        ],
        elevation: 0.22,
        azimuth: -0.50,
    },
    // Ice light: a low sun, grey ground, and clouds that are the coldest thing in the frame.
    Look {
        name: "tundra",
        ink: [0.06, 0.08, 0.14],
        sky: [
            [0.94, 0.92, 0.78],
            [0.76, 0.86, 0.92],
            [0.52, 0.68, 0.84],
            [0.28, 0.46, 0.68],
            [0.10, 0.24, 0.46],
        ],
        sand: [[0.30, 0.32, 0.34], [0.56, 0.58, 0.58], [0.78, 0.80, 0.78]],
        sun: [0.98, 0.99, 1.00],
        mesa: [0.32, 0.36, 0.46],
        cloud: [
            [0.50, 0.62, 0.76],
            [0.68, 0.78, 0.86],
            [0.84, 0.90, 0.94],
            [0.99, 1.00, 1.00],
        ],
        elevation: 0.16,
        azimuth: 2.20,
    },
    // Vermilion, and the sun high enough to be in the frame: the hottest hour of the set.
    Look {
        name: "cinnabar",
        ink: [0.16, 0.05, 0.08],
        sky: [
            [1.00, 0.88, 0.60],
            [0.99, 0.70, 0.44],
            [0.90, 0.46, 0.34],
            [0.62, 0.26, 0.34],
            [0.30, 0.14, 0.30],
        ],
        sand: [[0.40, 0.14, 0.14], [0.70, 0.30, 0.20], [0.90, 0.54, 0.32]],
        sun: [1.00, 0.98, 0.86],
        mesa: [0.46, 0.20, 0.26],
        cloud: [
            [0.82, 0.40, 0.36],
            [0.94, 0.58, 0.44],
            [1.00, 0.76, 0.56],
            [1.00, 0.93, 0.84],
        ],
        elevation: 0.70,
        azimuth: 0.10,
    },
    // Ink on old paper: everything in one warm range, and the drawing carried by the line.
    Look {
        name: "papyrus",
        ink: [0.13, 0.10, 0.07],
        sky: [
            [0.99, 0.90, 0.66],
            [0.92, 0.86, 0.72],
            [0.80, 0.72, 0.56],
            [0.62, 0.54, 0.40],
            [0.40, 0.34, 0.26],
        ],
        sand: [[0.44, 0.36, 0.26], [0.70, 0.60, 0.44], [0.88, 0.80, 0.62]],
        sun: [1.00, 0.99, 0.92],
        mesa: [0.46, 0.40, 0.32],
        cloud: [
            [0.72, 0.62, 0.50],
            [0.86, 0.76, 0.62],
            [0.95, 0.88, 0.76],
            [1.00, 0.97, 0.92],
        ],
        elevation: 0.38,
        azimuth: -1.20,
    },
    // Weather coming: a grey-green sky, low and heavy, with the clouds nearly the brightest
    // thing left.
    Look {
        name: "monsoon",
        ink: [0.05, 0.08, 0.09],
        sky: [
            [0.88, 0.90, 0.72],
            [0.70, 0.76, 0.68],
            [0.50, 0.58, 0.54],
            [0.32, 0.40, 0.42],
            [0.16, 0.24, 0.30],
        ],
        sand: [[0.24, 0.24, 0.20], [0.46, 0.46, 0.36], [0.66, 0.68, 0.56]],
        sun: [0.96, 0.98, 0.94],
        mesa: [0.26, 0.30, 0.34],
        cloud: [
            [0.40, 0.48, 0.50],
            [0.58, 0.66, 0.64],
            [0.76, 0.82, 0.78],
            [0.94, 0.97, 0.92],
        ],
        elevation: 0.34,
        azimuth: 2.60,
    },
    // A hard blue overhead with the ground kept warm under it, which is the widest the sky and
    // the sand get from each other anywhere in the set.
    Look {
        name: "cobalt",
        ink: [0.04, 0.06, 0.12],
        sky: [
            [0.98, 0.92, 0.74],
            [0.80, 0.86, 0.84],
            [0.44, 0.68, 0.84],
            [0.16, 0.44, 0.76],
            [0.06, 0.22, 0.56],
        ],
        sand: [[0.42, 0.28, 0.20], [0.72, 0.52, 0.34], [0.92, 0.74, 0.50]],
        sun: [1.00, 0.97, 0.84],
        mesa: [0.28, 0.30, 0.48],
        cloud: [
            [0.48, 0.62, 0.82],
            [0.68, 0.78, 0.90],
            [0.86, 0.90, 0.94],
            [1.00, 0.98, 0.96],
        ],
        elevation: 0.52,
        azimuth: -0.90,
    },
    // Deep yellow at the horizon and indigo overhead, with nothing in the middle that belongs to
    // either of them.
    Look {
        name: "saffron",
        ink: [0.10, 0.07, 0.12],
        sky: [
            [1.00, 0.94, 0.50],
            [0.98, 0.82, 0.38],
            [0.74, 0.58, 0.44],
            [0.42, 0.36, 0.52],
            [0.16, 0.16, 0.42],
        ],
        sand: [[0.38, 0.30, 0.22], [0.68, 0.56, 0.32], [0.90, 0.78, 0.48]],
        sun: [1.00, 0.99, 0.78],
        mesa: [0.34, 0.28, 0.44],
        cloud: [
            [0.62, 0.52, 0.60],
            [0.82, 0.68, 0.58],
            [0.96, 0.84, 0.60],
            [1.00, 0.97, 0.86],
        ],
        elevation: 0.58,
        azimuth: 0.70,
    },
    // The palest sky in the set, and the darkest zenith over it.
    Look {
        name: "glacier",
        ink: [0.04, 0.07, 0.11],
        sky: [
            [0.86, 0.94, 0.82],
            [0.68, 0.86, 0.86],
            [0.40, 0.66, 0.80],
            [0.18, 0.40, 0.62],
            [0.06, 0.18, 0.40],
        ],
        sand: [[0.26, 0.30, 0.32], [0.50, 0.56, 0.56], [0.74, 0.80, 0.76]],
        sun: [1.00, 1.00, 1.00],
        mesa: [0.24, 0.32, 0.42],
        cloud: [
            [0.44, 0.60, 0.72],
            [0.64, 0.76, 0.84],
            [0.82, 0.90, 0.92],
            [0.98, 1.00, 0.98],
        ],
        elevation: 0.12,
        azimuth: 1.80,
    },
    // Green all the way down, with the rock the only thing in the picture that is not.
    Look {
        name: "moss",
        ink: [0.07, 0.09, 0.05],
        sky: [
            [0.96, 0.94, 0.68],
            [0.82, 0.88, 0.60],
            [0.56, 0.72, 0.50],
            [0.30, 0.50, 0.38],
            [0.12, 0.28, 0.24],
        ],
        sand: [[0.30, 0.28, 0.16], [0.56, 0.50, 0.28], [0.78, 0.72, 0.44]],
        sun: [1.00, 1.00, 0.88],
        mesa: [0.30, 0.34, 0.24],
        cloud: [
            [0.54, 0.64, 0.48],
            [0.72, 0.80, 0.60],
            [0.88, 0.92, 0.74],
            [1.00, 1.00, 0.92],
        ],
        elevation: 0.42,
        azimuth: -2.00,
    },
    // Salmon and warm pink, with the clouds pulled the other way into a cold white so that they
    // stand off a sky that is nearly their own value.
    Look {
        name: "coral",
        ink: [0.12, 0.06, 0.11],
        sky: [
            [1.00, 0.92, 0.80],
            [1.00, 0.76, 0.68],
            [0.86, 0.54, 0.58],
            [0.54, 0.36, 0.56],
            [0.22, 0.20, 0.42],
        ],
        sand: [[0.42, 0.26, 0.28], [0.72, 0.48, 0.42], [0.92, 0.72, 0.60]],
        sun: [1.00, 0.96, 0.92],
        mesa: [0.40, 0.28, 0.44],
        cloud: [
            [0.72, 0.52, 0.62],
            [0.88, 0.66, 0.68],
            [0.96, 0.82, 0.80],
            [0.98, 0.99, 1.00],
        ],
        elevation: 0.26,
        azimuth: 1.40,
    },
    // The other night, warm where nocturne is cold: brown air, and a sun that has only just gone.
    Look {
        name: "sable",
        // The second palette where the line has to be watched, and the harder of the two: the
        // whole picture sits inside the bottom half of the range, so the line has almost nowhere
        // left to be darker than.
        ink: [0.03, 0.02, 0.05],
        sky: [
            [0.60, 0.48, 0.42],
            [0.46, 0.36, 0.36],
            [0.34, 0.26, 0.32],
            [0.22, 0.18, 0.28],
            [0.12, 0.10, 0.20],
        ],
        sand: [[0.14, 0.10, 0.12], [0.26, 0.20, 0.20], [0.38, 0.30, 0.28]],
        sun: [0.98, 0.92, 0.82],
        mesa: [0.18, 0.14, 0.20],
        cloud: [
            [0.40, 0.32, 0.38],
            [0.56, 0.46, 0.48],
            [0.72, 0.62, 0.60],
            [0.92, 0.86, 0.80],
        ],
        elevation: 0.10,
        azimuth: -0.20,
    },
    // Lilac going to teal, every colour of it half a step from grey: the quietest of the twenty.
    Look {
        name: "opal",
        ink: [0.08, 0.08, 0.13],
        sky: [
            [0.98, 0.88, 0.72],
            [0.86, 0.80, 0.86],
            [0.62, 0.72, 0.82],
            [0.36, 0.54, 0.68],
            [0.16, 0.32, 0.50],
        ],
        sand: [[0.34, 0.30, 0.34], [0.60, 0.54, 0.52], [0.84, 0.78, 0.70]],
        sun: [1.00, 0.98, 0.94],
        mesa: [0.34, 0.34, 0.48],
        cloud: [
            [0.58, 0.60, 0.78],
            [0.74, 0.76, 0.86],
            [0.88, 0.90, 0.92],
            [0.96, 1.00, 0.98],
        ],
        elevation: 0.30,
        azimuth: 0.20,
    },
];
