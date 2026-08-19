//! Conway's Game of Life, and forty-three of its relatives.
//!
//! The library is the whole simulation and the whole rule table, so a headless test can drive
//! either without opening a window. [`game`] is the field and what happens to it, [`rules`] is
//! the published rules it can be run under, [`look`] is the colour schemes and [`screen`] is
//! the one render pass. The binary adds the window, the keys of taste, and the readout.

pub mod game;
pub mod look;
pub mod rules;
pub mod screen;
