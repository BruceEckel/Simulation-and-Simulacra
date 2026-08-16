//! Moebius: clouds drawn the way they are drawn on paper, as outlines built from circular arcs.
//!
//! The library holds everything that is not a window. [`cloud`] builds the sky as a list of
//! overlapping circles and the order to draw them in; [`game`] is the weather that moves them;
//! [`look`] is five palettes as flat colour; [`sky`] owns the single render pass and the shader
//! beside it. The binary adds the window, the readout and the keys, and the `moebius_still`
//! example draws a frame with no window.

pub mod cloud;
pub mod game;
pub mod look;
pub mod sky;
