//! Determinism gate: same seed and same input twice, bit-identical both times. A gas of
//! several hundred atoms resolving collisions through a shared spatial grid is about the most
//! order-sensitive thing in this repo, so this is the test that matters here.

use fulcrum::prelude::*;
use heatflow::game::{
    ATOM_RADIUS, Atom, COURT_COMMAND, Census, GamePlugin, Meter, Walls, court_payload, parse_court,
};

/// Scripted input: change both surfaces, pour atoms in, take some out.
fn script(input: &mut Input, tick: u32) {
    match tick {
        200..=260 => input.push_key(Key::Q, true),
        261 => input.push_key(Key::Q, false),
        400..=460 => input.push_key(Key::D, true),
        461 => input.push_key(Key::D, false),
        600..=640 => input.push_key(Key::N, true),
        641 => input.push_key(Key::N, false),
        800..=830 => input.push_key(Key::M, true),
        831 => input.push_key(Key::M, false),
        _ => {}
    }
}

/// Run the gas for `ticks` and return the readings plus every atom's state as exact bits.
fn run(seed: u64, ticks: u32) -> (u32, Vec<(u32, u32, u32, u32)>) {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed,
        window_size: (1024, 768),
        ..Default::default()
    })
    .with_plugin(SpatialPlugin {
        cell_size: ATOM_RADIUS * 8.0,
    })
    .with_plugin(GamePlugin);

    app.run_startup();
    for tick in 0..ticks {
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            script(&mut input, tick);
            input.sample(|screen| screen);
        }
        if tick == 500 {
            app.world_mut()
                .resource_mut::<CommandOutbox>()
                .send(COURT_COMMAND, court_payload(vec2(1400.0, 560.0)));
        }
        app.tick();
    }

    let atoms = app.world_mut().resource::<Census>().atoms;
    let world = app.world_mut();
    let state = world
        .query_filtered::<(&Transform2D, &heatflow::game::Velocity), With<Atom>>()
        .iter(world)
        .map(|(transform, velocity)| {
            (
                transform.translation.x.to_bits(),
                transform.translation.y.to_bits(),
                velocity.0.x.to_bits(),
                velocity.0.y.to_bits(),
            )
        })
        .collect();
    (atoms, state)
}

#[test]
fn determinism_same_seed_same_outcome() {
    let a = run(42, 1_200);
    let b = run(42, 1_200);
    assert!(!a.1.is_empty(), "the box should have gas in it");
    assert_eq!(a, b, "same seed + same input must be bit-identical");
}

#[test]
fn determinism_different_seeds_diverge() {
    assert_ne!(run(1, 400), run(2, 400), "the seed should matter");
}

#[test]
fn the_measurements_replay_identically() {
    let reading = |seed| {
        let mut app = Fulcrum::with_config(FulcrumConfig {
            seed,
            window_size: (1024, 768),
            ..Default::default()
        })
        .with_plugin(SpatialPlugin {
            cell_size: ATOM_RADIUS * 8.0,
        })
        .with_plugin(GamePlugin);
        app.world_mut().insert_resource(Walls {
            left: 300.0,
            right: 900.0,
        });
        app.run_startup();
        for _ in 0..1_500 {
            app.tick();
        }
        let meter = app.world_mut().resource::<Meter>().clone();
        (
            meter.left_flux.to_bits(),
            meter.right_flux.to_bits(),
            meter.profile.map(f32::to_bits),
            meter.collisions,
            meter.wall_hits,
        )
    };
    assert_eq!(
        reading(7),
        reading(7),
        "the instruments must read the same on an identical run"
    );
}

#[test]
fn a_malformed_resize_is_ignored() {
    assert_eq!(parse_court("1400 560"), Some(vec2(1400.0, 560.0)));
    for payload in ["", "wide", "1400", "1400 x", "0 0", "-8 -8"] {
        assert_eq!(
            parse_court(payload),
            None,
            "payload {payload:?} should be rejected"
        );
    }
}
