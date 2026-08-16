//! Determinism gate: same seed and same input twice, bit-identical both times.

use fulcrum::prelude::*;
use starry::game::{CANVAS, Census, GamePlugin, Sky, Stroke, Velocity};

/// Scripted input: a pointer dragged through the paint, a star hung and taken down, healing
/// turned off and on, paint added and taken away, and a repaint.
fn script(input: &mut Input, tick: u32) {
    match tick {
        100..=160 => {
            input.push_cursor(vec2(-400.0 + 12.0 * (tick - 100) as f32, 140.0));
        }
        200 => {
            input.push_cursor(vec2(180.0, 300.0));
            input.push_mouse_button(MouseButton::Left, true);
        }
        202 => input.push_mouse_button(MouseButton::Left, false),
        260 => input.push_key(Key::H, true),
        261 => input.push_key(Key::H, false),
        320..=360 => input.push_key(Key::N, true),
        361 => input.push_key(Key::N, false),
        420 => input.push_key(Key::H, true),
        421 => input.push_key(Key::H, false),
        500..=530 => input.push_key(Key::M, true),
        531 => input.push_key(Key::M, false),
        600 => input.push_key(Key::X, true),
        601 => input.push_key(Key::X, false),
        700 => input.push_key(Key::R, true),
        701 => input.push_key(Key::R, false),
        780..=820 => input.push_key(Key::Up, true),
        821 => input.push_key(Key::Up, false),
        _ => {}
    }
}

/// One stroke's position, heading and colour, as exact bits.
type Bits = (u32, u32, u32, u32, u32);

/// Everything a run is judged on.
type Outcome = (Census, usize, Vec<Bits>);

/// Run the painting for `ticks` and return what is on the canvas, as exact bits.
fn run(seed: u64, ticks: u32) -> Outcome {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed,
        window_size: (CANVAS.x as u32, CANVAS.y as u32),
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

    let census = *app.world_mut().resource::<Census>();
    let stars = app.world_mut().resource::<Sky>().stars.len();
    let world = app.world_mut();
    let canvas = world
        .query::<(&Stroke, &Transform2D, &Velocity)>()
        .iter(world)
        .map(|(stroke, transform, velocity)| {
            (
                transform.translation.x.to_bits(),
                transform.translation.y.to_bits(),
                stroke.angle.to_bits(),
                stroke.tone.to_bits(),
                velocity.0.length_squared().to_bits(),
            )
        })
        .collect();
    (census, stars, canvas)
}

#[test]
fn determinism_same_seed_same_outcome() {
    let a = run(42, 900);
    let b = run(42, 900);
    assert!(!a.2.is_empty(), "there should be paint on the canvas");
    assert_eq!(a, b, "same seed + same input must be bit-identical");
}

#[test]
fn determinism_different_seeds_diverge() {
    assert_ne!(run(1, 500), run(2, 500), "the seed should matter");
}
