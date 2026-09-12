//! Determinism gate: same seed and same input twice, bit-identical both times.

use fulcrum::prelude::*;
use giraffe::game::{Field, GamePlugin, Pace, Terms};

/// Scripted input: the pointer wanders in, a patch of giraffes painted along a drag, a patch of
/// jackals from the right button, the dial turned both ways, the pace hurried, the temptation
/// raised, the board stopped and started twice, and then sown again.
fn script(input: &mut Input, tick: u32) {
    match tick {
        30 => input.push_cursor(vec2(300.0, 300.0)),
        31..=90 => input.push_cursor(vec2(300.0 + (tick - 30) as f32 * 6.0, 300.0)),
        120 => input.push_mouse_button(MouseButton::Left, true),
        121..=250 => input.push_cursor(vec2(660.0, 300.0 + (tick - 120) as f32 * 2.0)),
        251 => input.push_mouse_button(MouseButton::Left, false),
        300 => input.push_mouse_button(MouseButton::Right, true),
        303 => input.push_mouse_button(MouseButton::Right, false),
        350 | 352 => input.push_scroll(1.0),
        420 => input.push_scroll(-1.0),
        450 => input.push_key(Key::Up, true),
        500 => input.push_key(Key::Up, false),
        520 => input.push_key(Key::Right, true),
        560 => input.push_key(Key::Right, false),
        600 => input.push_key(Key::Space, true),
        601 => input.push_key(Key::Space, false),
        700 => input.push_key(Key::Space, true),
        701 => input.push_key(Key::Space, false),
        900 => input.push_key(Key::R, true),
        901 => input.push_key(Key::R, false),
        _ => {}
    }
}

/// The whole state as raw bits: every cell's strategy, what it scored, how long it has held
/// that strategy, how many generations have been played, and the terms the board is under.
/// Bits rather than floats so that a difference of one unit in the last place is a failure,
/// which is the only standard worth holding a replayable simulation to.
fn run(seed: u64, ticks: u32) -> (Vec<u8>, Vec<u32>, Vec<u32>, u64, [u32; 4]) {
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
        app.tick();
    }

    let field = app.world_mut().resource::<Field>();
    let cells = field.cells.iter().map(|c| c.index() as u8).collect();
    let scores = field.score.iter().map(|s| s.to_bits()).collect();
    let ages = field.age.clone();
    let generation = field.generation;
    let terms = app.world_mut().resource::<Terms>();
    let (conflict, temptation) = (terms.conflict.to_bits(), terms.temptation.to_bits());
    let pace = app.world_mut().resource::<Pace>();
    let under = [
        conflict,
        temptation,
        pace.period.to_bits(),
        pace.due.to_bits(),
    ];
    (cells, scores, ages, generation, under)
}

#[test]
fn determinism_same_seed_same_outcome() {
    let a = run(42, 1000);
    let b = run(42, 1000);
    assert!(!a.0.is_empty(), "the board should have cells in it");
    assert_eq!(a, b, "same seed + same input must be bit-identical");
}

#[test]
fn determinism_different_seeds_diverge() {
    // The seed sprinkles the takers at startup, so two seeds differ from tick zero.
    assert_ne!(run(1, 10), run(2, 10), "the seed should matter");
}
