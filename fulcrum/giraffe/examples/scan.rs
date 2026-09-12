//! A scan of the board's terms, headless: how the temptation and the conflict dial change what
//! the board does. This is how the starting values in `game.rs` were chosen.
//!
//! `cargo run -p giraffe --example scan --release`

use fulcrum::prelude::*;
use giraffe::game::{ACROSS, DOWN, Field, Strategy, Terms};

/// A seeded board.
fn board(seed: u64) -> (Field, SimRng) {
    let mut rng = SimRng::seeded(seed);
    let mut field = Field::new();
    field.seed(&mut rng);
    (field, rng)
}

/// How many cells changed strategy in the last generation.
fn churn(field: &Field) -> usize {
    field.age.iter().filter(|&&age| age == 0).count()
}

fn main() {
    println!("temptation, at the starting conflict: sharing fraction over 300 generations");
    for temptation in [1.4, 1.5, 1.55, 1.58, 1.6, 1.62, 1.65, 1.68, 1.7, 1.8] {
        let terms = Terms {
            temptation,
            ..Terms::default()
        };
        let (mut field, _) = board(42);
        let mut line = format!("  b {temptation:.2}:");
        for generation in 0..=300u32 {
            if [0, 10, 50, 100, 200, 300].contains(&generation) {
                line += &format!(" {:.2}", field.sharing());
            }
            if generation < 300 {
                field.generation(&terms);
            }
        }
        line += &format!("   churn {}", churn(&field));
        println!("{line}");
    }

    println!();
    println!("conflict dial, at the starting temptation: a planted patch of giraffes, radius 6.5,");
    println!("speaking fraction over 150 generations");
    for tenths in 0..=10 {
        let conflict = tenths as f32 / 10.0;
        let terms = Terms {
            conflict,
            ..Terms::default()
        };
        let (mut field, _) = board(42);
        field.plant(
            vec2(ACROSS as f32 / 2.0, DOWN as f32 / 2.0),
            6.5,
            Strategy::GIRAFFE,
        );
        let mut line = format!("  k {conflict:.1}:");
        for generation in 0..=150u32 {
            if [0, 5, 10, 20, 40, 80, 150].contains(&generation) {
                line += &format!(" {:.3}", field.speaking());
            }
            if generation < 150 {
                field.generation(&terms);
            }
        }
        let tally = field.tally();
        line += &format!("   tally {tally:?}");
        println!("{line}");
    }

    println!();
    println!("all four kinds sprinkled evenly, tally after 300 generations, by conflict");
    println!("  (silent sharer, silent taker, speaking sharer, speaking taker)");
    for tenths in 0..=10 {
        let conflict = tenths as f32 / 10.0;
        let terms = Terms {
            conflict,
            ..Terms::default()
        };
        let mut rng = SimRng::seeded(42);
        let mut field = Field::new();
        for cell in &mut field.cells {
            *cell = Strategy::ALL[rng.range_i32(0..4) as usize];
        }
        for _ in 0..300 {
            field.generation(&terms);
        }
        println!(
            "  k {conflict:.1}: {:?}   churn {}",
            field.tally(),
            churn(&field)
        );
    }

    println!();
    println!("the same, with a patch of jackals planted into a board of giraffes");
    for tenths in 0..=10 {
        let conflict = tenths as f32 / 10.0;
        let terms = Terms {
            conflict,
            ..Terms::default()
        };
        let mut field = Field::new();
        field.plant(vec2(0.0, 0.0), 1000.0, Strategy::GIRAFFE);
        field.plant(
            vec2(ACROSS as f32 / 2.0, DOWN as f32 / 2.0),
            6.5,
            Strategy::JACKAL,
        );
        let mut line = format!("  k {conflict:.1}:");
        for generation in 0..=150u32 {
            if [0, 5, 10, 20, 40, 80, 150].contains(&generation) {
                line += &format!(" {:.3}", field.speaking());
            }
            if generation < 150 {
                field.generation(&terms);
            }
        }
        println!("{line}");
    }
}
