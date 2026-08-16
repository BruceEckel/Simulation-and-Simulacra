//! Determinism gate: same seed and same input twice, bit-identical both times.

use flutter::game::{Clock, Flock, GamePlugin, Moth, Speed};
use fulcrum::prelude::*;

/// Scripted input: the swarm grown, cut back and grown again, the pace pushed both ways and
/// reset, the lamp put out and lit, a pause, and a restock — the population changing under a
/// running simulation is exactly the thing this test is here to pin down.
fn script(input: &mut Input, tick: u32) {
    match tick {
        0..=60 => input.push_cursor(vec2(-300.0 + 6.0 * tick as f32, 120.0)),
        80..=140 => input.push_key(Key::Up, true),
        141 => input.push_key(Key::Up, false),
        160..=200 => input.push_key(Key::Right, true),
        201 => input.push_key(Key::Right, false),
        240..=280 => input.push_key(Key::Down, true),
        281 => input.push_key(Key::Down, false),
        300 => input.push_key(Key::L, true),
        301 => input.push_key(Key::L, false),
        340 => input.push_key(Key::Space, true),
        341 => input.push_key(Key::Space, false),
        380 => input.push_key(Key::Space, true),
        381 => input.push_key(Key::Space, false),
        420..=460 => input.push_key(Key::Left, true),
        461 => input.push_key(Key::Left, false),
        500 => input.push_key(Key::Digit0, true),
        501 => input.push_key(Key::Digit0, false),
        540 => input.push_key(Key::R, true),
        541 => input.push_key(Key::R, false),
        560..=600 => input.push_key(Key::Up, true),
        601 => input.push_key(Key::Up, false),
        _ => {}
    }
}

/// One moth's pose and wingbeat, as exact bits.
type Bits = (u32, u32, u32, u32);

/// Everything a run is judged on.
type Outcome = (Flock, u32, u32, Vec<Bits>);

/// Run the room for `ticks` and return what is in it, as exact bits.
fn run(seed: u64, ticks: u32) -> Outcome {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed,
        ..Default::default()
    })
    .with_plugin(GamePlugin);

    app.run_startup();
    for tick in 0..ticks {
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            script(&mut input, tick);
            input.sample(|screen| screen);
        }
        app.tick();
    }

    let flock = *app.world_mut().resource::<Flock>();
    let speed = app.world_mut().resource::<Speed>().0.to_bits();
    let clock = app.world_mut().resource::<Clock>().0.to_bits();
    let world = app.world_mut();
    let swarm = world
        .query::<(&Moth, &Transform2D)>()
        .iter(world)
        .map(|(moth, transform)| {
            (
                transform.translation.x.to_bits(),
                transform.translation.y.to_bits(),
                moth.heading.to_bits(),
                moth.wing.to_bits(),
            )
        })
        .collect();
    (flock, speed, clock, swarm)
}

#[test]
fn determinism_same_seed_same_outcome() {
    let a = run(42, 700);
    let b = run(42, 700);
    assert!(!a.3.is_empty(), "there should be moths in the room");
    assert_eq!(a, b, "same seed + same input must be bit-identical");
}

#[test]
fn determinism_different_seeds_diverge() {
    assert_ne!(run(1, 300), run(2, 300), "the seed should matter");
}
