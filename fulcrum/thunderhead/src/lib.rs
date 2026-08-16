//! Thunderhead: the library holds the sky ([`game::GamePlugin`]) and the palettes
//! ([`look::LOOKS`]) so that headless tests and the `thunderhead_still` example can draw the picture
//! without a window; the binary adds the window, the readout and the keys.

pub mod game;
pub mod look;
