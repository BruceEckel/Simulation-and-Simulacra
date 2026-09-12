//! Giraffe. The library exposes the board and its terms ([`game::GamePlugin`]) so headless
//! tests can run a thousand generations in a moment, and the palettes ([`look`]) so they can
//! be checked for legibility. The binary puts the two together and adds the keys.

pub mod game;
pub mod look;
