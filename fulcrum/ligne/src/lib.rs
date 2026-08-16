//! Ligne: a live two-dimensional cloud field, drawn in flat colour with a clean line.
//!
//! The library holds everything that is not a window. [`game`] is the weather; [`field`] builds
//! the one sheet of tiling noise the whole sky is read out of; [`look`] is five palettes as flat
//! colour; [`sky`] owns the single render pass and the shader beside it. The binary adds the
//! window, the readout and the keys, and the `ligne_still` example draws a frame with no window.

pub mod field;
pub mod game;
pub mod look;
pub mod sky;
