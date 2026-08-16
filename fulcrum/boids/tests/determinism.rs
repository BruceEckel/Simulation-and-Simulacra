//! Determinism gate: run the flock headless, same seed + scripted input twice, and require
//! bit-identical outcomes. 240 boids reading a shared spatial grid is exactly the shape of
//! simulation where iteration order tends to leak in, so this is the test that matters here.

use boids::game::{Boid, GamePlugin, Predator};
use fulcrum::prelude::*;

/// Scripted input: flip rules mid-run so the toggles are covered too.
fn script(input: &mut Input, tick: u32) {
    match tick {
        150 => input.push_key(Key::Digit3, true), // cohesion off
        151 => input.push_key(Key::Digit3, false),
        300 => input.push_key(Key::P, true), // predator parked
        301 => input.push_key(Key::P, false),
        450 => input.push_key(Key::Digit3, true), // cohesion back on
        451 => input.push_key(Key::Digit3, false),
        _ => {}
    }
}

/// Run the sim for `ticks` and return every boid and predator transform as exact bits.
fn run(seed: u64, ticks: u32) -> Vec<(u32, u32, u32)> {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed,
        window_size: (1024, 768),
        ..Default::default()
    })
    .with_plugin(SpatialPlugin {
        cell_size: boids::game::NEIGHBOR_RADIUS,
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

    let world = app.world_mut();
    world
        .query_filtered::<&Transform2D, Or<(With<Boid>, With<Predator>)>>()
        .iter(world)
        .map(|t| {
            (
                t.translation.x.to_bits(),
                t.translation.y.to_bits(),
                t.rotation.to_bits(),
            )
        })
        .collect()
}

#[test]
fn determinism_same_seed_same_outcome() {
    let a = run(42, 600);
    let b = run(42, 600);
    assert_eq!(a.len(), boids::game::FLOCK_SIZE + 1, "flock did not spawn");
    assert_eq!(a, b, "same seed + same input must be bit-identical");
}

#[test]
fn determinism_different_seeds_diverge() {
    // Not a determinism requirement, but it proves the runs above are identical because the
    // simulation is deterministic, not because the seed is being ignored.
    assert_ne!(run(1, 200), run(2, 200), "the seed should matter");
}
