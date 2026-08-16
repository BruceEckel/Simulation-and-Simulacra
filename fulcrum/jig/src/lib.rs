//! A skeleton whose every bone is a pendulum hanging off the one before it. The library holds
//! the body, the one equation ([`game::swing`]) and the dance ([`game::GamePlugin`]) so that
//! headless tests can drive it; the binary adds the bones' looks and the noise they make.

pub mod game;
