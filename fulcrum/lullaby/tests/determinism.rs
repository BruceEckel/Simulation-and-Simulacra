//! Determinism gate: same seed and same input twice, bit-identical both times.

use fulcrum::prelude::*;
use lullaby::game::{
    Depth, FIELD_COMMAND, GamePlugin, Night, Star, Velocity, field_payload, parse_field,
};

/// Scripted input: the night shortened, then lengthened, a reprieve, and a fresh sky.
fn script(input: &mut Input, tick: u32) {
    match tick {
        120 => input.push_key(Key::Digit2, true),
        121 => input.push_key(Key::Digit2, false),
        400 => input.push_key(Key::Space, true),
        401 => input.push_key(Key::Space, false),
        800 => input.push_key(Key::R, true),
        801 => input.push_key(Key::R, false),
        1200 => input.push_key(Key::Digit9, true),
        1201 => input.push_key(Key::Digit9, false),
        _ => {}
    }
}

/// One star's whole state as raw bits: where it is, how fast, and where it is heading to rest.
/// Bits rather than floats so that a difference of one unit in the last place is a failure,
/// which is the only standard worth holding a replayable simulation to.
type StarBits = (u32, u32, u32, u32, u32, u32);

/// Run a night for `ticks` and return every star's state as exact bits, plus where the night got
/// to. Homes are included: a fresh sky is drawn from the same RNG as everything else, so a
/// divergence there has to show up here.
fn run(seed: u64, ticks: u32) -> (u32, Vec<StarBits>) {
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
        if tick == 600 {
            app.world_mut()
                .resource_mut::<CommandOutbox>()
                .send(FIELD_COMMAND, field_payload(vec2(1400.0, 560.0)));
        }
        app.tick();
    }

    let depth = app.world_mut().resource::<Depth>().now.to_bits();
    let world = app.world_mut();
    let state = world
        .query::<(&Star, &Transform2D, &Velocity)>()
        .iter(world)
        .map(|(star, transform, velocity)| {
            (
                transform.translation.x.to_bits(),
                transform.translation.y.to_bits(),
                velocity.0.x.to_bits(),
                velocity.0.y.to_bits(),
                star.home.x.to_bits(),
                star.home.y.to_bits(),
            )
        })
        .collect();
    (depth, state)
}

#[test]
fn determinism_same_seed_same_outcome() {
    let a = run(42, 1400);
    let b = run(42, 1400);
    assert!(!a.1.is_empty(), "the sky should have stars in it");
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

#[test]
fn the_length_of_the_night_is_what_was_asked_for() {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed: 7,
        window_size: (1024, 768),
        ..Default::default()
    })
    .with_plugin(GamePlugin);
    app.run_startup();

    for (key, minutes) in [(Key::Digit1, 5.0), (Key::Digit9, 45.0), (Key::Digit5, 25.0)] {
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            input.push_key(key, true);
            input.sample(|screen| screen);
        }
        app.tick();
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            input.push_key(key, false);
            input.sample(|screen| screen);
        }
        app.tick();
        assert_eq!(
            app.world_mut().resource::<Night>().0,
            minutes * 60.0,
            "the number keys should set the night in five minute steps"
        );
    }
}
