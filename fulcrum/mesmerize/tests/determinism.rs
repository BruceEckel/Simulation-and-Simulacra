//! Determinism gate: same seed and same input twice, bit-identical both times.

use fulcrum::prelude::*;
use mesmerize::game::{
    Census, FIELD_COMMAND, GamePlugin, Mote, Velocity, field_payload, parse_field,
};

/// Scripted input: pace changes, a reseed, and light added and taken away.
fn script(input: &mut Input, tick: u32) {
    match tick {
        100..=160 => input.push_key(Key::Up, true),
        161 => input.push_key(Key::Up, false),
        300..=340 => input.push_key(Key::N, true),
        341 => input.push_key(Key::N, false),
        500 => input.push_key(Key::R, true),
        501 => input.push_key(Key::R, false),
        700..=730 => input.push_key(Key::M, true),
        731 => input.push_key(Key::M, false),
        _ => {}
    }
}

/// Run the piece for `ticks` and return the census plus every mote's state as exact bits.
fn run(seed: u64, ticks: u32) -> (u32, Vec<(u32, u32, u32, u32)>) {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed,
        window_size: (1024, 768),
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
        if tick == 400 {
            app.world_mut()
                .resource_mut::<CommandOutbox>()
                .send(FIELD_COMMAND, field_payload(vec2(1400.0, 560.0)));
        }
        app.tick();
    }

    let motes = app.world_mut().resource::<Census>().0;
    let world = app.world_mut();
    let state = world
        .query_filtered::<(&Transform2D, &Velocity), With<Mote>>()
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
    (motes, state)
}

#[test]
fn determinism_same_seed_same_outcome() {
    let a = run(42, 900);
    let b = run(42, 900);
    assert!(!a.1.is_empty(), "the field should have light in it");
    assert_eq!(a, b, "same seed + same input must be bit-identical");
}

#[test]
fn determinism_different_seeds_diverge() {
    assert_ne!(run(1, 400), run(2, 400), "the seed should matter");
}

#[test]
fn a_malformed_resize_is_ignored() {
    assert_eq!(parse_field("1400 560"), Some(vec2(1400.0, 560.0)));
    for payload in ["", "wide", "1400", "1400 x", "0 0", "-8 -8"] {
        assert_eq!(
            parse_field(payload),
            None,
            "payload {payload:?} should be rejected"
        );
    }
}
