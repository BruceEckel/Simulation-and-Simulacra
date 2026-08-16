//! Determinism gate: same seed and same input twice, bit-identical both times.
//!
//! There is not much simulation here to drift, which is the point of running the gate anyway.
//! The weather is five numbers advanced on the fixed tick, and every one of them is a float being
//! integrated: the moment one of those integrations picks up the frame's wall-clock delta instead
//! of the fixed one, the sky stops being reproducible and the replay is worth nothing. This
//! catches that.

use fulcrum::prelude::*;
use parrish::game::{GamePlugin, Weather};

/// Scripted input: the pace up and down, the head turned both ways, the sky held and let go.
fn script(input: &mut Input, tick: u32) {
    match tick {
        20..=60 => input.push_key(Key::Up, true),
        61 => input.push_key(Key::Up, false),
        80..=130 => input.push_key(Key::Right, true),
        131 => input.push_key(Key::Right, false),
        150..=190 => input.push_key(Key::Down, true),
        191 => input.push_key(Key::Down, false),
        210 => input.push_key(Key::Space, true),
        211 => input.push_key(Key::Space, false),
        230..=250 => input.push_key(Key::Left, true),
        251 => input.push_key(Key::Left, false),
        260 => input.push_key(Key::Space, true),
        261 => input.push_key(Key::Space, false),
        _ => {}
    }
}

/// Fold the whole of the weather into one number.
fn digest(app: &mut Fulcrum) -> u64 {
    let weather = *app.world_mut().resource::<Weather>();
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut eat = |value: u32| {
        hash ^= u64::from(value);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    };
    for axis in weather.drift {
        eat(axis.to_bits());
    }
    eat(weather.boil.to_bits());
    eat(weather.yaw.to_bits());
    eat(weather.pace.to_bits());
    eat(u32::from(weather.held));
    // The eye is derived rather than stored, so it is folded in as well: a change to how the
    // heading becomes a camera is a change to the picture even when the heading is the same.
    let eye = weather.eye();
    for axis in eye.forward.into_iter().chain(eye.right).chain(eye.up) {
        eat(axis.to_bits());
    }
    hash
}

/// Run the piece headless for `ticks`, taking a digest every so often.
fn run(seed: u64, ticks: u32) -> Vec<u64> {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed,
        ..Default::default()
    })
    .with_plugin(GamePlugin);

    app.run_startup();
    let mut marks = Vec::new();
    for tick in 0..ticks {
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            script(&mut input, tick);
            input.sample(|screen| screen);
        }
        app.tick();
        if tick % 20 == 0 {
            marks.push(digest(&mut app));
        }
    }
    marks.push(digest(&mut app));
    marks
}

#[test]
fn determinism_same_input_same_weather() {
    let first = run(42, 300);
    let second = run(42, 300);
    assert_eq!(first, second, "same input must give the same sky");
}

#[test]
fn the_weather_actually_moves() {
    // A gate that passes because nothing ever changes is not a gate.
    let marks = run(42, 300);
    assert!(
        marks.windows(2).any(|pair| pair[0] != pair[1]),
        "the weather never moved"
    );
}
