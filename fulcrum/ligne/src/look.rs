//! Five palettes, as flat colour.
//!
//! Unlike the volumetric piece next door, nothing in here is a radiance and nothing is tone
//! mapped. These are the colours that land on the screen, because every area in this drawing is
//! filled with one of them and there is no shading anywhere: the light decides *which* band a
//! patch of cloud is in, and the band decides the colour.
//!
//! That is what makes a palette here a real decision rather than a tint. Four cloud stops, two
//! sky ends, two sand ends, a rock and a line: eleven colours, and the picture is those eleven
//! and nothing else.

/// One sky, whole.
pub struct Look {
    /// Its name, for the readout.
    pub name: &'static str,
    /// The line. One colour for every contour in the picture, the way a pen has one colour.
    pub ink: [f32; 3],
    /// The sky straight overhead.
    pub sky_zenith: [f32; 3],
    /// The sky where it meets the sand.
    pub sky_horizon: [f32; 3],
    /// The sun's disc.
    pub sun: [f32; 3],
    /// Sand at the viewer's feet.
    pub ground_near: [f32; 3],
    /// Sand at the horizon.
    pub ground_far: [f32; 3],
    /// Rock standing on the horizon.
    pub mesa: [f32; 3],
    /// What distance fades into.
    pub haze: [f32; 3],
    /// The four stops a cloud is filled from, rain-dark base to lit crown.
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
    // The evening this whole set of pieces was drawn for: a turquoise sky going gold at the
    // horizon, ochre sand, and clouds carrying the sunset up one side.
    Look {
        name: "arzach",
        ink: [0.10, 0.07, 0.13],
        sky_zenith: [0.10, 0.22, 0.45],
        sky_horizon: [0.98, 0.84, 0.55],
        sun: [1.00, 0.97, 0.84],
        ground_near: [0.55, 0.25, 0.22],
        ground_far: [0.93, 0.72, 0.44],
        mesa: [0.42, 0.34, 0.52],
        haze: [0.93, 0.83, 0.62],
        cloud: [
            [0.30, 0.19, 0.44],
            [0.74, 0.33, 0.52],
            [0.99, 0.68, 0.55],
            [1.00, 0.97, 0.92],
        ],
        elevation: 0.15,
        azimuth: 0.34,
    },
    // Cool and green: the light of a world with a different star, and the one palette where the
    // clouds are colder than the ground they cross.
    Look {
        name: "verdigris",
        ink: [0.07, 0.12, 0.14],
        sky_zenith: [0.05, 0.30, 0.42],
        sky_horizon: [0.93, 0.96, 0.83],
        sun: [0.99, 1.00, 0.92],
        ground_near: [0.48, 0.43, 0.32],
        ground_far: [0.86, 0.83, 0.60],
        mesa: [0.30, 0.42, 0.46],
        haze: [0.88, 0.93, 0.82],
        cloud: [
            [0.20, 0.34, 0.46],
            [0.46, 0.66, 0.69],
            [0.80, 0.91, 0.86],
            [1.00, 1.00, 0.99],
        ],
        elevation: 0.31,
        azimuth: -0.28,
    },
    // Noon on a hot world: the sky bleached out at the horizon, the sand red, the clouds going
    // up through orange into white.
    Look {
        name: "ember",
        ink: [0.16, 0.06, 0.06],
        sky_zenith: [0.30, 0.20, 0.45],
        sky_horizon: [1.00, 0.93, 0.70],
        sun: [1.00, 0.99, 0.90],
        ground_near: [0.52, 0.18, 0.18],
        ground_far: [0.94, 0.66, 0.38],
        mesa: [0.50, 0.26, 0.34],
        haze: [1.00, 0.90, 0.70],
        cloud: [
            [0.38, 0.14, 0.31],
            [0.82, 0.33, 0.31],
            [1.00, 0.74, 0.46],
            [1.00, 0.99, 0.94],
        ],
        elevation: 0.62,
        azimuth: 0.40,
    },
    // The half hour after the sun has gone: two colours in the whole frame, and the clouds are
    // the only things still lit.
    Look {
        name: "nocturne",
        ink: [0.02, 0.02, 0.06],
        sky_zenith: [0.05, 0.06, 0.20],
        sky_horizon: [0.62, 0.55, 0.70],
        sun: [0.95, 0.95, 1.00],
        ground_near: [0.14, 0.11, 0.21],
        ground_far: [0.34, 0.30, 0.46],
        mesa: [0.14, 0.13, 0.26],
        haze: [0.55, 0.50, 0.66],
        cloud: [
            [0.11, 0.11, 0.26],
            [0.32, 0.28, 0.48],
            [0.63, 0.56, 0.72],
            [0.95, 0.94, 1.00],
        ],
        elevation: 0.20,
        azimuth: 0.50,
    },
    // The loudest: flat cyan against flat pink, with a violet in the clouds that belongs to
    // neither of them.
    Look {
        name: "mineral",
        ink: [0.09, 0.09, 0.14],
        sky_zenith: [0.06, 0.44, 0.62],
        sky_horizon: [0.96, 0.92, 0.78],
        sun: [1.00, 0.95, 0.72],
        ground_near: [0.58, 0.30, 0.40],
        ground_far: [0.94, 0.76, 0.66],
        mesa: [0.34, 0.32, 0.54],
        haze: [0.92, 0.88, 0.80],
        cloud: [
            [0.24, 0.26, 0.52],
            [0.63, 0.40, 0.66],
            [0.98, 0.70, 0.62],
            [1.00, 0.98, 0.94],
        ],
        elevation: 0.24,
        azimuth: 0.30,
    },
];
