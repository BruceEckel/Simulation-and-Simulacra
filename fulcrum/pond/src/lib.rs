//! Pond. The library exposes the water and the lantern ([`game::GamePlugin`]) so headless
//! tests can run minutes of it in a second, and the render pass ([`screen`]) so a still can be
//! taken without a window. The binary puts the two together and adds the keys.

pub mod game;
pub mod look;
pub mod screen;
