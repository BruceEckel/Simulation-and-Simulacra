//! The colour schemes.
//!
//! Six colours to a scheme, and none of them is a light value: these are the numbers that land
//! on the screen. Nothing here is lit or shaded, because there is nothing here with a surface.
//! A cell is in a state, and a state is a colour.
//!
//! What the six are for is worth saying, because two of them exist for reasons the rule itself
//! does not have.
//!
//! - `back` is an empty cell, and most of the field most of the time.
//! - `live` is a live cell, and the colour a scheme is recognised by.
//! - `fresh` is a live cell that was born this generation, fading to `live` as it holds. Turn
//!   that off and every live cell is `live`. It is not part of any rule: it is a reading of the
//!   field that shows you where the work is being done, which in Life is always at the edges.
//! - `dying` is a cell in one of the Generations states, on its way from live to empty. In a
//!   two-state rule nothing is ever this colour. In Brian's Brain almost everything is.
//! - `trail` is where a cell was recently, decaying towards `back`. Also not part of any rule —
//!   it is a long exposure, and at one cell to the pixel it is what turns a field of noise into
//!   something with a direction.
//! - `ink` draws the line between cells when the cells are big enough to want one.

/// One colour scheme. Every colour is linear-ish sRGB in `0..=1`, written the way it should
/// look, because the pass writes into an sRGB target and the hardware does the conversion.
pub struct Look {
    /// Its name, for the readout.
    pub name: &'static str,
    /// An empty cell.
    pub back: [f32; 3],
    /// A live cell that has been alive a while, and the colour with the age reading off.
    pub live: [f32; 3],
    /// A live cell born this generation.
    pub fresh: [f32; 3],
    /// A cell in a Generations dying state, just after it stopped being alive.
    pub dying: [f32; 3],
    /// Where a cell has recently been.
    pub trail: [f32; 3],
    /// The line between cells.
    pub ink: [f32; 3],
}

/// Twelve schemes. Two of them are light, which matters more than it sounds: a field of Life on
/// paper reads as a diagram, and the same field on black reads as something alive.
pub const LOOKS: &[Look] = &[
    Look {
        name: "Phosphor",
        back: [0.020, 0.045, 0.030],
        live: [0.290, 0.950, 0.420],
        fresh: [0.800, 1.000, 0.750],
        dying: [0.060, 0.420, 0.200],
        trail: [0.050, 0.220, 0.110],
        ink: [0.070, 0.160, 0.100],
    },
    Look {
        name: "Mono",
        back: [0.030, 0.030, 0.035],
        live: [0.780, 0.800, 0.840],
        fresh: [1.000, 1.000, 1.000],
        dying: [0.340, 0.350, 0.380],
        trail: [0.150, 0.150, 0.170],
        ink: [0.110, 0.110, 0.130],
    },
    Look {
        name: "Amber",
        back: [0.045, 0.028, 0.008],
        live: [1.000, 0.680, 0.160],
        fresh: [1.000, 0.940, 0.720],
        dying: [0.480, 0.240, 0.040],
        trail: [0.220, 0.110, 0.020],
        ink: [0.140, 0.090, 0.030],
    },
    Look {
        name: "Ember",
        back: [0.035, 0.012, 0.012],
        live: [0.980, 0.350, 0.100],
        fresh: [1.000, 0.930, 0.620],
        dying: [0.550, 0.090, 0.050],
        trail: [0.240, 0.050, 0.030],
        ink: [0.160, 0.050, 0.040],
    },
    Look {
        name: "Ice",
        back: [0.015, 0.035, 0.075],
        live: [0.420, 0.800, 0.980],
        fresh: [0.930, 0.990, 1.000],
        dying: [0.100, 0.320, 0.600],
        trail: [0.050, 0.140, 0.300],
        ink: [0.050, 0.100, 0.190],
    },
    Look {
        name: "Nebula",
        back: [0.045, 0.015, 0.075],
        live: [0.850, 0.350, 0.920],
        fresh: [1.000, 0.880, 0.720],
        dying: [0.400, 0.120, 0.620],
        trail: [0.180, 0.050, 0.300],
        ink: [0.130, 0.050, 0.200],
    },
    Look {
        name: "Peacock",
        back: [0.015, 0.045, 0.055],
        live: [0.100, 0.780, 0.680],
        fresh: [0.960, 0.800, 0.240],
        dying: [0.050, 0.340, 0.360],
        trail: [0.030, 0.160, 0.180],
        ink: [0.040, 0.120, 0.130],
    },
    Look {
        name: "Neon",
        back: [0.020, 0.015, 0.035],
        live: [1.000, 0.160, 0.550],
        fresh: [0.200, 1.000, 0.950],
        dying: [0.450, 0.060, 0.350],
        trail: [0.180, 0.030, 0.180],
        ink: [0.120, 0.040, 0.160],
    },
    Look {
        name: "Copper",
        back: [0.045, 0.030, 0.022],
        live: [0.860, 0.520, 0.260],
        fresh: [1.000, 0.900, 0.660],
        dying: [0.400, 0.200, 0.110],
        trail: [0.190, 0.110, 0.070],
        ink: [0.140, 0.090, 0.060],
    },
    Look {
        name: "Blueprint",
        back: [0.050, 0.140, 0.340],
        live: [0.900, 0.940, 1.000],
        fresh: [1.000, 0.850, 0.350],
        dying: [0.300, 0.450, 0.720],
        trail: [0.120, 0.240, 0.480],
        ink: [0.110, 0.220, 0.440],
    },
    Look {
        name: "Ink on paper",
        back: [0.940, 0.930, 0.890],
        live: [0.080, 0.090, 0.120],
        fresh: [0.720, 0.180, 0.140],
        dying: [0.520, 0.520, 0.500],
        trail: [0.800, 0.790, 0.750],
        ink: [0.780, 0.770, 0.720],
    },
    Look {
        name: "Sepia",
        back: [0.900, 0.850, 0.760],
        live: [0.300, 0.200, 0.130],
        fresh: [0.620, 0.360, 0.160],
        dying: [0.660, 0.580, 0.470],
        trail: [0.800, 0.740, 0.640],
        ink: [0.760, 0.700, 0.600],
    },
];
