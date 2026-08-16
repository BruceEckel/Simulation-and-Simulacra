//! Determinism gate: same seed and same input twice, bit-identical both times.

use fulcrum::prelude::*;
use spectacle::game::{
    Census, FIELD_COMMAND, GamePlugin, Show, Spark, Velocity, field_payload, parse_field,
};

/// Scripted input: a finale called early, a pause, pace changes, and two shells fired by hand.
fn script(input: &mut Input, tick: u32) {
    match tick {
        120..=180 => input.push_key(Key::Up, true),
        181 => input.push_key(Key::Up, false),
        400 => input.push_key(Key::F, true),
        401 => input.push_key(Key::F, false),
        600 => input.push_key(Key::Space, true),
        601 => input.push_key(Key::Space, false),
        640 => input.push_key(Key::Space, true),
        641 => input.push_key(Key::Space, false),
        700 => {
            input.push_cursor(vec2(-180.0, 220.0));
            input.push_mouse_button(MouseButton::Left, true);
        }
        740 => input.push_mouse_button(MouseButton::Left, false),
        _ => {}
    }
}

/// One star's position and velocity, as exact bits.
type Bits = (u32, u32, u32, u32);

/// Everything a run is judged on: what is in the sky, how many shells went up, and the state
/// of every star down to the bit.
type Outcome = (Census, u32, Vec<Bits>);

/// Run the show for `ticks` and return what is in the sky, as exact bits.
fn run(seed: u64, ticks: u32) -> Outcome {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed,
        window_size: (1280, 720),
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
        if tick == 500 {
            app.world_mut()
                .resource_mut::<CommandOutbox>()
                .send(FIELD_COMMAND, field_payload(vec2(1600.0, 576.0)));
        }
        app.tick();
    }

    let census = *app.world_mut().resource::<Census>();
    let fired = app.world_mut().resource::<Show>().fired;
    let world = app.world_mut();
    let sky = world
        .query_filtered::<(&Transform2D, &Velocity), With<Spark>>()
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
    (census, fired, sky)
}

#[test]
fn determinism_same_seed_same_outcome() {
    let a = run(42, 900);
    let b = run(42, 900);
    assert!(!a.2.is_empty(), "the sky should have something in it");
    assert_eq!(a, b, "same seed + same input must be bit-identical");
}

#[test]
fn determinism_different_seeds_diverge() {
    assert_ne!(run(1, 600), run(2, 600), "the seed should matter");
}

#[test]
fn a_malformed_resize_is_ignored() {
    assert_eq!(parse_field("1600 576"), Some(vec2(1600.0, 576.0)));
    for payload in ["", "wide", "1600", "1600 x", "0 0", "-8 -8"] {
        assert_eq!(
            parse_field(payload),
            None,
            "payload {payload:?} should be rejected"
        );
    }
}
