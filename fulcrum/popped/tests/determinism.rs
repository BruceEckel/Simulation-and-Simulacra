//! Determinism gate: same seed and same input twice, bit-identical both times.

use fulcrum::prelude::*;
use popped::game::{ARENA, Animal, Balloon, Census, GamePlugin, Tally, Velocity};

/// Scripted input: clicks all over the sky, a pause, and a change of pace.
fn script(input: &mut Input, tick: u32) {
    if tick.is_multiple_of(47) {
        input.push_cursor(vec2(
            (tick as f32 * 0.31).sin() * ARENA.x * 0.42,
            (tick as f32 * 0.17).cos() * ARENA.y * 0.35,
        ));
        input.push_mouse_button(MouseButton::Left, true);
    }
    if tick % 47 == 3 {
        input.push_mouse_button(MouseButton::Left, false);
    }
    match tick {
        300..=340 => input.push_key(Key::Up, true),
        341 => input.push_key(Key::Up, false),
        600 => input.push_key(Key::Space, true),
        601 => input.push_key(Key::Space, false),
        660 => input.push_key(Key::Space, true),
        661 => input.push_key(Key::Space, false),
        _ => {}
    }
}

/// One thing in the sky, as exact bits.
type Bits = (u32, u32, u32, u32);

/// Everything a run is judged on.
type Outcome = (Tally, Census, Vec<Bits>, Vec<Bits>);

/// Run the sky for `ticks` and return what is in it, as exact bits.
fn run(seed: u64, ticks: u32) -> Outcome {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed,
        window_size: (ARENA.x as u32, ARENA.y as u32),
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

    let tally = *app.world_mut().resource::<Tally>();
    let census = *app.world_mut().resource::<Census>();
    let world = app.world_mut();
    let balloons = world
        .query::<(&Balloon, &Transform2D)>()
        .iter(world)
        .map(|(balloon, transform)| {
            (
                transform.translation.x.to_bits(),
                transform.translation.y.to_bits(),
                balloon.sway.to_bits(),
                balloon.radius.to_bits(),
            )
        })
        .collect();
    let world = app.world_mut();
    let animals = world
        .query::<(&Animal, &Transform2D, &Velocity)>()
        .iter(world)
        .map(|(animal, transform, velocity)| {
            (
                transform.translation.x.to_bits(),
                transform.translation.y.to_bits(),
                animal.timer.to_bits(),
                velocity.0.length_squared().to_bits(),
            )
        })
        .collect();
    (tally, census, balloons, animals)
}

#[test]
fn determinism_same_seed_same_outcome() {
    let a = run(42, 900);
    let b = run(42, 900);
    assert!(!a.3.is_empty(), "there should be somebody up there");
    assert!(a.0.popped > 0, "the script should have popped something");
    assert_eq!(a, b, "same seed + same input must be bit-identical");
}

#[test]
fn determinism_different_seeds_diverge() {
    assert_ne!(run(1, 600), run(2, 600), "the seed should matter");
}
