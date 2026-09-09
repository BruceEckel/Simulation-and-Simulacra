//! Determinism gate: same seed and same input twice, bit-identical both times.

use fulcrum::prelude::*;
use pond::game::{FIELD_COMMAND, GamePlugin, Lantern, Water, field_payload, parse_field};

/// Scripted input: the pointer wanders in, a tap, a long press dragged along, the pace
/// changed, the lantern sent off, and the water stilled.
fn script(input: &mut Input, tick: u32) {
    match tick {
        30 => input.push_cursor(vec2(300.0, 300.0)),
        31..=90 => input.push_cursor(vec2(300.0 + (tick - 30) as f32 * 6.0, 300.0)),
        120 => input.push_mouse_button(MouseButton::Left, true),
        123 => input.push_mouse_button(MouseButton::Left, false),
        200 => input.push_mouse_button(MouseButton::Left, true),
        201..=320 => input.push_cursor(vec2(660.0, 300.0 + (tick - 200) as f32 * 2.0)),
        321 => input.push_mouse_button(MouseButton::Left, false),
        400 => input.push_key(Key::Up, true),
        460 => input.push_key(Key::Up, false),
        500 => input.push_key(Key::Space, true),
        501 => input.push_key(Key::Space, false),
        900 => input.push_key(Key::R, true),
        901 => input.push_key(Key::R, false),
        _ => {}
    }
}

/// The whole state as raw bits: every cell's height and speed, and where the lantern is. Bits
/// rather than floats so that a difference of one unit in the last place is a failure, which is
/// the only standard worth holding a replayable simulation to.
fn run(seed: u64, ticks: u32) -> (Vec<u32>, Vec<u32>, [u32; 4]) {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed,
        window_size: (1280, 800),
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
        if tick == 700 {
            app.world_mut()
                .resource_mut::<CommandOutbox>()
                .send(FIELD_COMMAND, field_payload(vec2(1000.0, 900.0)));
        }
        app.tick();
    }

    let water = app.world_mut().resource::<Water>();
    let heights = water.height.iter().map(|h| h.to_bits()).collect();
    let speeds = water.speed.iter().map(|s| s.to_bits()).collect();
    let lantern = app.world_mut().resource::<Lantern>();
    let where_it_is = [
        lantern.at.x.to_bits(),
        lantern.at.y.to_bits(),
        lantern.velocity.x.to_bits(),
        lantern.velocity.y.to_bits(),
    ];
    (heights, speeds, where_it_is)
}

#[test]
fn determinism_same_seed_same_outcome() {
    let a = run(42, 1000);
    let b = run(42, 1000);
    assert!(!a.0.is_empty(), "the pond should have water in it");
    assert_eq!(a, b, "same seed + same input must be bit-identical");
}

#[test]
fn determinism_different_seeds_diverge() {
    // The rain is all the seed touches, and the first drop falls at three seconds.
    assert_ne!(run(1, 400), run(2, 400), "the seed should matter");
}

#[test]
fn a_malformed_resize_is_ignored() {
    assert_eq!(parse_field("1000 900"), Some(vec2(1000.0, 900.0)));
    for payload in ["", "wide", "1000", "1000 x", "0 0", "-8 -8"] {
        assert_eq!(
            parse_field(payload),
            None,
            "payload {payload:?} should be rejected"
        );
    }
}
