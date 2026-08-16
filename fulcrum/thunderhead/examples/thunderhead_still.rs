//! One frame of the piece, written to a PNG, with no window and no GPU anywhere near it.
//!
//! This is the tool the look was tuned with, and it works because the picture is decided in
//! plain code: `game.rs` composes a field of materials, `look.rs` says what a material is, and
//! the window is only ever the thing that shows the answer.
//!
//! ```sh
//! cargo run -p thunderhead --release --example thunderhead_still -- still.png 2560 1600 0 7
//! ```
//!
//! The arguments are the file, the size in pixels, which palette, and the seed.

use fulcrum::prelude::SimRng;
use thunderhead::game::{Field, HORIZON, Sky, build_backdrop, compose, place, populate};
use thunderhead::look::{LOOKS, lut};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap_or_else(|| "still.png".to_string());
    let width = number(args.next(), 2560);
    let height = number(args.next(), 1600);
    let palette = number(args.next(), 0) as usize % LOOKS.len();
    let seed = u64::from(number(args.next(), 7));

    let mut rng = SimRng::seeded(seed);
    let mut sky = Sky {
        seed: rng.u64(),
        ..Default::default()
    };
    populate(&mut sky, height as f32 * HORIZON, &mut rng);
    place(&mut sky, width as f32);

    let backdrop = build_backdrop(width, height, sky.seed);
    let mut field = Field {
        width,
        height,
        cells: vec![0; (width as usize) * (height as usize)],
    };
    compose(&mut field, &backdrop, &sky);

    let table = lut(&LOOKS[palette]);
    let mut pixels = Vec::with_capacity(field.cells.len() * 4);
    for &cell in &field.cells {
        pixels.extend_from_slice(&table[cell as usize]);
    }
    image::save_buffer(
        &path,
        &pixels,
        width,
        height,
        image::ExtendedColorType::Rgba8,
    )
    .expect("write the still");
    println!(
        "{path}: {width}x{height}, palette {}, seed {seed}",
        LOOKS[palette].name
    );
}

/// One numeric argument, or a default.
fn number(arg: Option<String>, fallback: u32) -> u32 {
    arg.and_then(|value| value.parse().ok()).unwrap_or(fallback)
}
