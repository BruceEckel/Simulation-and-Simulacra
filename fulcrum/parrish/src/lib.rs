//! Parrish: a cloud field painted the way Maxfield Parrish painted, in transparent coats over a
//! white ground.
//!
//! The library holds everything that is not a window. [`game`] is the weather; [`field`] builds
//! the one sheet of tiling noise the whole sky is read out of; [`look`] is five palettes, each
//! of them a set of transmittances rather than a set of colours; [`sky`] owns the single render
//! pass and the shader beside it. The binary adds the window, the readout and the keys, and the
//! `parrish_still` example draws a frame with no window.

pub mod field;
pub mod game;
pub mod look;
pub mod sky;
