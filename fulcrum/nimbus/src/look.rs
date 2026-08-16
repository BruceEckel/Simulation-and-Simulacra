//! Five skies, as light rather than as colour.
//!
//! Every number in here is a radiance, not a pixel value: the shader adds them up along a ray
//! and the tone map at the end decides what any of it looks like on a screen. So a sun is
//! allowed to be twenty and the sand is allowed to be a half, and the difference between the
//! two is the whole reason a cloud edge can blow out to white while the cloud beside it stays
//! violet.
//!
//! A palette also carries where the sun is, because it has to. The colour of the light and the
//! angle it arrives at are one decision: a low sun that is not warm looks broken, and a warm
//! sun overhead looks like a mistake.

/// One sky, whole.
pub struct Look {
    /// Its name, for the readout.
    pub name: &'static str,
    /// The sky straight overhead.
    pub sky_zenith: [f32; 3],
    /// The sky where it meets the sand.
    pub sky_horizon: [f32; 3],
    /// The colour of sunlight.
    pub sun: [f32; 3],
    /// How bright the sun's own disc is against that.
    pub sun_power: f32,
    /// The light the sky throws back down into the clouds, which is what keeps their undersides
    /// from being black.
    pub ambient: [f32; 3],
    /// How much of it there is.
    pub ambient_power: f32,
    /// Sand, in its darker tone.
    pub ground_near: [f32; 3],
    /// Sand, in its lighter one.
    pub ground_far: [f32; 3],
    /// What distance fades into. Near the horizon sky, since that is the air being looked
    /// through, and any daylight between the two shows up as a seam along the horizon.
    pub haze: [f32; 3],
    /// How far above the horizon the sun stands, in radians.
    pub elevation: f32,
    /// Where it stands around the compass, in radians, measured the way the view's heading is.
    pub azimuth: f32,
    /// How much light makes white.
    pub exposure: f32,
}

impl Look {
    /// The direction towards the sun.
    pub fn sun_direction(&self) -> [f32; 3] {
        let (rise, run) = self.elevation.sin_cos();
        let (across, along) = self.azimuth.sin_cos();
        [across * run, rise, along * run]
    }
}

/// The five. Each was tuned against the same frame, because a palette is only right relative to
/// the one it is being compared with.
pub const LOOKS: &[Look] = &[
    // The hour before the sun goes: a cold zenith, a furnace at the horizon, and every cloud
    // lit along one side and violet on the other.
    Look {
        name: "arzach",
        sky_zenith: [0.05, 0.15, 0.46],
        sky_horizon: [0.62, 0.36, 0.26],
        sun: [1.45, 0.88, 0.46],
        sun_power: 9.0,
        ambient: [0.26, 0.40, 0.70],
        ambient_power: 0.30,
        ground_near: [0.40, 0.19, 0.11],
        ground_far: [0.66, 0.38, 0.19],
        haze: [0.70, 0.44, 0.32],
        elevation: 0.150,
        azimuth: 0.32,
        exposure: 1.15,
    },
    // Noon on a hot world: a hard blue overhead, the horizon bleached out of it, and the shadows
    // straight down under the clouds where you cannot see them.
    Look {
        name: "noon",
        sky_zenith: [0.08, 0.26, 0.90],
        sky_horizon: [0.55, 0.66, 0.86],
        sun: [1.60, 1.52, 1.35],
        sun_power: 14.0,
        ambient: [0.40, 0.58, 0.95],
        ambient_power: 0.42,
        ground_near: [0.52, 0.25, 0.13],
        ground_far: [0.88, 0.55, 0.29],
        haze: [0.62, 0.70, 0.88],
        elevation: 0.95,
        azimuth: 0.45,
        exposure: 0.85,
    },
    // The light under a front that has not broken yet: everything grey-green, the sun somewhere
    // behind it, and the sand the only warm thing left.
    Look {
        name: "monsoon",
        sky_zenith: [0.08, 0.11, 0.16],
        sky_horizon: [0.34, 0.37, 0.36],
        sun: [0.72, 0.68, 0.58],
        sun_power: 5.0,
        ambient: [0.24, 0.29, 0.33],
        ambient_power: 0.55,
        ground_near: [0.22, 0.20, 0.15],
        ground_far: [0.44, 0.41, 0.30],
        haze: [0.36, 0.39, 0.38],
        elevation: 0.34,
        azimuth: -0.40,
        exposure: 1.05,
    },
    // An hour after sunset with a full moon up: the clouds are the only lit things in the frame
    // and the desert is a rumour underneath them.
    Look {
        name: "nocturne",
        sky_zenith: [0.010, 0.018, 0.065],
        sky_horizon: [0.070, 0.080, 0.180],
        sun: [0.30, 0.35, 0.52],
        sun_power: 30.0,
        ambient: [0.07, 0.09, 0.20],
        ambient_power: 0.75,
        ground_near: [0.085, 0.080, 0.150],
        ground_far: [0.170, 0.155, 0.260],
        haze: [0.085, 0.095, 0.195],
        elevation: 0.26,
        azimuth: 0.55,
        exposure: 2.20,
    },
    // The loud one, and the one the flat-colour mode was tuned on: cyan against pink with
    // nothing in between them.
    Look {
        name: "mineral",
        sky_zenith: [0.04, 0.40, 0.80],
        sky_horizon: [0.72, 0.48, 0.42],
        sun: [1.55, 0.98, 0.62],
        sun_power: 10.0,
        ambient: [0.30, 0.50, 0.80],
        ambient_power: 0.34,
        ground_near: [0.52, 0.24, 0.24],
        ground_far: [0.88, 0.52, 0.44],
        haze: [0.78, 0.56, 0.48],
        elevation: 0.19,
        azimuth: 0.28,
        exposure: 1.10,
    },
];
