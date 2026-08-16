//! Determinism gate: same seed and same input twice, bit-identical both times.
//!
//! Stronger here than in the other games in this repository, and worth saying why. There is no
//! floating point anywhere in this simulation: grains are counts, toppling is subtraction, and the
//! histogram bins come from `ilog2`. Two runs do not merely agree to within rounding, they agree
//! exactly, and they would agree on a machine with a different floating point unit or none at all.

use avalanche::game::{ARENA, GamePlugin, Ledger, Sizes, Table, cell_at};
use fulcrum::prelude::*;

/// Scripted input: pouring in two places, a handful, a sweep, a load, and a change of pace.
fn script(input: &mut Input, tick: u32) {
    match tick {
        200..=320 => {
            input.push_cursor(cell_at(40 + (tick as i32 - 200) / 4, 70));
            input.push_mouse_button(MouseButton::Left, true);
        }
        321 => input.push_mouse_button(MouseButton::Left, false),
        400 => {
            input.push_cursor(cell_at(120, 40));
            input.push_key(Key::B, true);
        }
        401 => input.push_key(Key::B, false),
        520 => input.push_key(Key::R, true),
        521 => input.push_key(Key::R, false),
        700 => input.push_key(Key::F, true),
        701 => input.push_key(Key::F, false),
        800..=830 => input.push_key(Key::Up, true),
        831 => input.push_key(Key::Up, false),
        _ => {}
    }
}

/// Everything a run is judged on. All of it integers.
type Outcome = (Vec<u16>, Vec<u8>, [u32; avalanche::game::BINS], Ledger);

/// Run the table for `ticks` and return exactly what is on it.
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

    let table = app.world_mut().resource::<Table>().clone();
    let sizes = app.world_mut().resource::<Sizes>().clone();
    let ledger = *app.world_mut().resource::<Ledger>();
    (table.grains, table.glow, sizes.bins, ledger)
}

#[test]
fn determinism_same_seed_same_outcome() {
    let a = run(42, 1200);
    let b = run(42, 1200);
    assert!(a.3.measured > 0, "the table should have measured something");
    assert!(a.3.topples > 0, "and something should have moved");
    assert_eq!(a, b, "same seed + same input must be bit-identical");
}

#[test]
fn determinism_different_seeds_diverge() {
    assert_ne!(run(1, 900), run(2, 900), "the seed should matter");
}
