//! Moebius, again: clouds drawn the way they are drawn on paper, as outlines built from circular
//! arcs, with three of the decisions handed to the keys.
//!
//! The first version fixed all three. Here the line around a cloud has a width you can move, an
//! element laid over a cloud is a union of two to six circles rather than one circle, and the
//! palette is one of twenty rather than one of five.
//!
//! The library holds everything that is not a window. [`cloud`] builds the sky as a list of
//! overlapping circles and the order to draw them in, and owns [`cloud::Style`], the two settings
//! that change the drawing rather than the weather; [`game`] is the weather that moves them;
//! [`look`] is twenty palettes as flat colour; [`sky`] owns the single render pass and the shader
//! beside it. The binary adds the window, the readout and the keys, and the `moebius2_still`
//! example draws a frame with no window.

pub mod cloud;
pub mod game;
pub mod look;
pub mod sky;
