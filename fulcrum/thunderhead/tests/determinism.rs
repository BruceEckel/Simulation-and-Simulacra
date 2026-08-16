//! Determinism gate: same seed and same input twice, bit-identical both times.
//!
//! Three things in here could drift and none of them is allowed to. The desert, the shape of
//! every cloud and where each one is hung all come out of the simulation RNG, so the same
//! weather has to be drawn twice. The clouds are grown on an allowance of texels per tick, so
//! a cloud that finished a tick earlier on a faster machine would put different pixels on the
//! screen at the same tick. And the window changes shape mid-run here, which regrows every
//! cloud in the sky: the queue has to come back in the same order both times.

use fulcrum::prelude::*;
use thunderhead::game::{Field, GamePlugin, Motion, RESIZE_COMMAND, Sky, window_payload};

/// Scripted input: the pace up and down, and the sky held and let go.
fn script(input: &mut Input, tick: u32) {
    match tick {
        40..=70 => input.push_key(Key::Up, true),
        71 => input.push_key(Key::Up, false),
        120..=150 => input.push_key(Key::Down, true),
        151 => input.push_key(Key::Down, false),
        200 => input.push_key(Key::Space, true),
        201 => input.push_key(Key::Space, false),
        240 => input.push_key(Key::Space, true),
        241 => input.push_key(Key::Space, false),
        _ => {}
    }
}

/// Fold every scrap of simulation state into one number.
fn digest(app: &mut Fulcrum) -> u64 {
    let field = app.world_mut().resource::<Field>().clone();
    let motion = *app.world_mut().resource::<Motion>();

    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut eat = |value: u64| {
        hash ^= value;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    };
    eat(u64::from(field.width) << 32 | u64::from(field.height));
    for chunk in field.cells.chunks(8) {
        let mut word = 0u64;
        for (index, &cell) in chunk.iter().enumerate() {
            word |= u64::from(cell) << (index * 8);
        }
        eat(word);
    }
    eat(u64::from(motion.pace.to_bits()) | u64::from(motion.held) << 32);

    let sky = app.world_mut().resource::<Sky>();
    eat(sky.grown);
    eat(sky.waiting.len() as u64);
    for drifter in &sky.drifters {
        eat(u64::from(drifter.x.to_bits()));
        eat(u64::from(drifter.lift.to_bits()));
        eat(u64::from(drifter.speed.to_bits()));
    }
    for shape in &sky.shapes {
        eat(u64::from(shape.width) << 32 | u64::from(shape.height));
        eat(u64::from(shape.base));
        for chunk in shape.cells.chunks(8) {
            let mut word = 0u64;
            for (index, &cell) in chunk.iter().enumerate() {
                word |= u64::from(cell) << (index * 8);
            }
            eat(word);
        }
    }
    hash
}

/// Run the piece headless for `ticks`, taking a digest every so often so a drift is caught
/// where it happens rather than only if it survives to the end.
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
        // The window arrives sized as a window would size it, and changes shape mid-run, which
        // puts every cloud in the sky back in the queue to be regrown.
        if tick == 0 {
            app.world_mut()
                .resource_mut::<CommandOutbox>()
                .send(RESIZE_COMMAND, window_payload(240, 160));
        }
        if tick == 170 {
            app.world_mut()
                .resource_mut::<CommandOutbox>()
                .send(RESIZE_COMMAND, window_payload(200, 260));
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
fn determinism_same_seed_same_outcome() {
    let first = run(42, 300);
    let second = run(42, 300);
    assert_eq!(
        first, second,
        "same seed + same input must be bit-identical"
    );
}

#[test]
fn different_seeds_diverge() {
    let first = run(42, 60);
    let second = run(43, 60);
    assert_ne!(
        first, second,
        "different seeds should give different weather"
    );
}
