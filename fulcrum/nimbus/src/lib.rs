//! Nimbus: real-time volumetric clouds over a desert.
//!
//! The library holds everything that is not a window. [`game`] is the weather, a handful of
//! numbers advanced on the fixed tick; [`noise`] builds the two tiling volumes the cloud shape
//! is carved out of; [`look`] is five skies as light; [`sky`] owns the two render passes and
//! the shader beside it. The binary adds the window, the readout and the keys, and the `still`
//! example draws a frame with no window at all.

pub mod game;
pub mod look;
pub mod noise;
pub mod sky;
