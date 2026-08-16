//! Five palettes, as transmittances.
//!
//! This is the one file where this piece differs from the three next door before a single pixel
//! is drawn. Nothing in here is a colour. Every entry is a **transmittance**: what one coat of
//! transparent paint lets through, per channel. The picture is a white ground seen through a
//! stack of those coats, so a colour is never chosen, only arrived at by deciding how many coats
//! of what stand between the eye and the ground.
//!
//! That is not a conceit, it is how these paintings were made. Parrish laid down a white ground,
//! then glazed it with thin transparent films of a single pigment, varnishing between coats and
//! building up as many as thirty of them. No two pigments were ever mixed on the palette. The
//! light that reaches you has gone *through* the colour, off the white, and back out through it
//! again, which is why the blue is so deep and still looks lit from within, and why the shadows
//! in a Parrish are saturated blue instead of the grey that mixing gives you.
//!
//! Two consequences fall out of that and both of them shape the shader:
//!
//! - **A glaze can only darken.** Nothing laid on white gets brighter than white. Everything
//!   luminous in one of these pictures is paint *not* laid, or paint taken back off.
//! - **Depth is a count, not a blend.** Two coats are the tint squared, not the tint twice as
//!   strong, so the colour deepens and saturates along a curve rather than sliding towards
//!   whatever it is being mixed with.
//!
//! One practical note about the numbers, which look far too dark to be the colours they end up
//! as. They are transmittances of *light*, and multiplying light is only meaningful in a linear
//! space, so this is one: the frame is written to an sRGB target and the hardware encodes it on
//! the way out. A tint of `0.088` in the red is a sky whose red comes out around a tenth of the
//! way up the screen's range, not a black one.

/// One painting's worth of decisions.
pub struct Look {
    /// Its name, for the readout.
    pub name: &'static str,
    /// The ground everything is painted on. Near white, and never quite white: a real ground is
    /// warm, and the warmth survives every coat laid over it.
    pub ground: [f32; 3],
    /// The blue the sky is glazed with. Parrish blue is this tint under enough coats.
    pub sky_high: [f32; 3],
    /// The warm wash along the horizon, where the ground is left nearly bare.
    pub sky_low: [f32; 3],
    /// The warmth laid in where the blue has been scrubbed back around the sun, and along the
    /// edge of a cloud with the light behind it.
    pub glow: [f32; 3],
    /// The thin warm coat on a cloud's lit side.
    pub cloud_light: [f32; 3],
    /// The cool coat on the side turned away.
    pub cloud_shadow: [f32; 3],
    /// The deepest coat, under the cloud where it sits on its own base.
    pub cloud_deep: [f32; 3],
    /// Distance, as a glaze. Aerial perspective in a Parrish does not go grey and milky, it goes
    /// *bluer*: the far range is a deeper, more saturated blue than the near one.
    pub distance: [f32; 3],
    /// What the reflection is seen through. Water is a coat like any other.
    pub water: [f32; 3],
    /// The far range of hills.
    pub ridge_far: [f32; 3],
    /// The near one.
    pub ridge_near: [f32; 3],
    /// The rock across the bottom of the frame, which is the darkest thing in the picture and
    /// the reason the sky reads as bright.
    pub ledge: [f32; 3],
    /// Coats of [`Look::sky_high`] at the zenith.
    pub sky_depth: f32,
    /// Coats of [`Look::sky_low`] down at the horizon.
    pub horizon_depth: f32,
    /// How many coats are lifted back off around the sun.
    pub glow_depth: f32,
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
///
/// Each one was arrived at backwards: pick the colour a thing should come out as, divide by the
/// ground, and take the root of however many coats stand in front of it. That is the only
/// workable way to choose a transmittance, because a tint on its own tells you nothing about
/// what it will look like on the wall.
pub const LOOKS: &[Look] = &[
    // The one everybody has seen, whether or not they know whose it is: a cobalt zenith, a
    // horizon so pale it reads as the light source, and clouds carrying the low sun along their
    // flanks.
    Look {
        name: "daybreak",
        ground: [1.00, 0.985, 0.95],
        sky_high: [0.088, 0.245, 0.650],
        sky_low: [1.00, 0.78, 0.48],
        glow: [1.00, 0.80, 0.52],
        cloud_light: [1.00, 0.89, 0.76],
        cloud_shadow: [0.40, 0.43, 0.80],
        cloud_deep: [0.62, 0.66, 0.78],
        distance: [0.53, 0.65, 0.85],
        water: [0.45, 0.60, 0.72],
        ridge_far: [0.32, 0.41, 0.67],
        ridge_near: [0.15, 0.17, 0.33],
        ledge: [0.06, 0.07, 0.12],
        sky_depth: 0.86,
        horizon_depth: 0.60,
        glow_depth: 0.62,
        elevation: 0.13,
        azimuth: 0.36,
    },
    // Midday, and the blue at its full weight: the most coats of the five, and a horizon left
    // cool instead of gold. The whole picture is the contrast between a zenith under a dozen
    // glazes and a cloud that is very nearly bare ground.
    Look {
        name: "cobalt",
        ground: [1.00, 0.99, 0.97],
        sky_high: [0.065, 0.200, 0.600],
        sky_low: [0.80, 0.90, 1.00],
        glow: [1.00, 0.93, 0.72],
        cloud_light: [1.00, 0.95, 0.86],
        cloud_shadow: [0.36, 0.47, 0.80],
        cloud_deep: [0.58, 0.63, 0.78],
        distance: [0.48, 0.62, 0.86],
        water: [0.40, 0.56, 0.72],
        ridge_far: [0.28, 0.38, 0.66],
        ridge_near: [0.13, 0.16, 0.32],
        ledge: [0.05, 0.06, 0.11],
        sky_depth: 1.10,
        horizon_depth: 0.40,
        glow_depth: 0.42,
        elevation: 0.52,
        azimuth: 0.30,
    },
    // Late afternoon: the light coming almost level across the frame, everything it touches
    // going to gold and everything it misses going violet.
    Look {
        name: "hilltop",
        ground: [1.00, 0.97, 0.92],
        sky_high: [0.140, 0.280, 0.620],
        sky_low: [1.00, 0.70, 0.36],
        glow: [1.00, 0.72, 0.38],
        cloud_light: [1.00, 0.82, 0.56],
        cloud_shadow: [0.45, 0.46, 0.76],
        cloud_deep: [0.64, 0.60, 0.74],
        distance: [0.58, 0.62, 0.86],
        water: [0.50, 0.58, 0.70],
        ridge_far: [0.36, 0.40, 0.64],
        ridge_near: [0.17, 0.16, 0.30],
        ledge: [0.07, 0.06, 0.10],
        sky_depth: 0.72,
        horizon_depth: 0.80,
        glow_depth: 0.70,
        elevation: 0.09,
        azimuth: 0.44,
    },
    // The half hour after the sun has gone. Coats of blue over the whole sky, a band of
    // green-gold left along the horizon, and the clouds the only things still catching it.
    Look {
        name: "twilight",
        ground: [1.00, 0.98, 0.94],
        sky_high: [0.100, 0.160, 0.460],
        sky_low: [0.86, 0.82, 0.48],
        glow: [1.00, 0.80, 0.50],
        cloud_light: [0.94, 0.78, 0.64],
        cloud_shadow: [0.32, 0.36, 0.66],
        cloud_deep: [0.54, 0.54, 0.72],
        distance: [0.40, 0.48, 0.78],
        water: [0.36, 0.46, 0.64],
        ridge_far: [0.24, 0.30, 0.56],
        ridge_near: [0.11, 0.13, 0.26],
        ledge: [0.04, 0.05, 0.09],
        sky_depth: 1.25,
        horizon_depth: 0.72,
        glow_depth: 0.64,
        elevation: 0.045,
        azimuth: 0.52,
    },
    // The loudest of the five: a turquoise sky rather than a blue one, and clouds that go
    // through rose on their way to white.
    Look {
        name: "ecstasy",
        ground: [1.00, 0.99, 0.96],
        sky_high: [0.100, 0.360, 0.620],
        sky_low: [1.00, 0.76, 0.62],
        glow: [1.00, 0.76, 0.66],
        cloud_light: [1.00, 0.86, 0.78],
        cloud_shadow: [0.52, 0.46, 0.80],
        cloud_deep: [0.68, 0.58, 0.76],
        distance: [0.54, 0.68, 0.86],
        water: [0.46, 0.62, 0.74],
        ridge_far: [0.34, 0.44, 0.68],
        ridge_near: [0.16, 0.17, 0.32],
        ledge: [0.06, 0.06, 0.11],
        sky_depth: 0.90,
        horizon_depth: 0.68,
        glow_depth: 0.66,
        elevation: 0.17,
        azimuth: 0.28,
    },
];
