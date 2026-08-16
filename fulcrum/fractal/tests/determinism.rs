//! Determinism gate: same seed and same input twice, bit-identical both times.
//!
//! Two things here could drift and neither is allowed to. The chaos game draws from the
//! simulation RNG, so the fern is only the same fern twice if the draws are. And both
//! renderers work to a per-tick budget, so a budget that measured time rather than counting
//! work would put a different amount of the picture on screen on a busier machine — which is
//! exactly the sort of thing a replay cannot survive.

use fractal::game::{
    Cloud, Depth, Field, GamePlugin, Motion, RESIZE_COMMAND, Selection, View, parse_window,
    window_payload,
};
use fulcrum::prelude::*;

/// Scripted input: along the row of fractals, into one, and back out again. Deliberately ends
/// on a chaos game, having passed through two escape-time sets on the way.
fn script(input: &mut Input, tick: u32) {
    match tick {
        40 => input.push_key(Key::Digit2, true), // Julia, which moves on its own
        41 => input.push_key(Key::Digit2, false),
        90..=140 => input.push_key(Key::Z, true), // zoom in
        141 => input.push_key(Key::Z, false),
        160..=190 => input.push_key(Key::Right, true), // and pan
        191 => input.push_key(Key::Right, false),
        220 => input.push_key(Key::Digit4, true), // Newton
        221 => input.push_key(Key::Digit4, false),
        250..=290 => input.push_key(Key::E, true), // more iterations
        291 => input.push_key(Key::E, false),
        320 => input.push_key(Key::Digit6, true), // the fern, which draws from the RNG
        321 => input.push_key(Key::Digit6, false),
        360..=400 => input.push_key(Key::Up, true),
        401 => input.push_key(Key::Up, false),
        430 => input.push_key(Key::Tab, true), // on to the Sierpinski triangle
        431 => input.push_key(Key::Tab, false),
        _ => {}
    }
}

/// Fold every scrap of simulation state into one number.
fn digest(app: &mut Fulcrum) -> u64 {
    let field = app.world_mut().resource::<Field>().clone();
    let cloud = app.world_mut().resource::<Cloud>().clone();
    let view = *app.world_mut().resource::<View>();
    let motion = *app.world_mut().resource::<Motion>();
    let depth = *app.world_mut().resource::<Depth>();
    let selection = *app.world_mut().resource::<Selection>();

    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut eat = |value: u64| {
        hash ^= value;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    };
    eat(selection.0.index() as u64);
    eat(view.center_x.to_bits());
    eat(view.center_y.to_bits());
    eat(view.span.to_bits());
    eat(motion.phase.to_bits());
    eat(depth.0 as u64);
    eat(field.level as u64);
    eat(field.cursor as u64);
    eat(field.finished);
    for sample in &field.samples {
        eat(sample.value.to_bits() as u64);
        eat(sample.band as u64 | u64::from(sample.inside) << 8);
    }
    eat(cloud.steps);
    eat(cloud.x.to_bits());
    eat(cloud.y.to_bits());
    for speck in &cloud.points {
        eat(speck.x.to_bits() as u64);
        eat(speck.y.to_bits() as u64);
        eat(speck.tone.to_bits() as u64);
        eat(speck.band as u64);
    }
    hash
}

/// Run the viewer for `ticks`, taking a digest every so often so that a drift is caught where
/// it happens rather than only if it survives to the end.
fn run(seed: u64, ticks: u32) -> Vec<u64> {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed,
        window_size: (1280, 800),
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
        if tick == 300 {
            app.world_mut()
                .resource_mut::<CommandOutbox>()
                .send(RESIZE_COMMAND, window_payload(vec2(1000.0, 900.0)));
        }
        app.tick();
        if tick % 25 == 0 {
            marks.push(digest(&mut app));
        }
    }
    marks.push(digest(&mut app));
    marks
}

#[test]
fn determinism_same_seed_same_outcome() {
    let first = run(42, 480);
    let second = run(42, 480);
    assert_eq!(
        first, second,
        "same seed + same input must be bit-identical"
    );
}

#[test]
fn determinism_different_seeds_diverge() {
    // Only the chaos games draw from the RNG, so the run has to reach one before the seed can
    // possibly matter. The script arrives at the fern on tick 320.
    assert_ne!(
        run(1, 400),
        run(2, 400),
        "the seed decides where the chaos game walks"
    );
}

#[test]
fn a_malformed_resize_is_ignored() {
    assert_eq!(parse_window("1000 900"), Some(vec2(1000.0, 900.0)));
    for payload in ["", "wide", "1000", "1000 x", "0 0", "-8 -8"] {
        assert_eq!(
            parse_window(payload),
            None,
            "payload {payload:?} should be rejected"
        );
    }
}

#[test]
fn a_resize_reshapes_the_picture() {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed: 3,
        window_size: (1280, 800),
        ..Default::default()
    })
    .with_plugin(GamePlugin);
    app.run_startup();
    app.tick();

    app.world_mut()
        .resource_mut::<CommandOutbox>()
        .send(RESIZE_COMMAND, window_payload(vec2(700.0, 1300.0)));
    app.tick();

    let grid = *app.world_mut().resource::<fractal::game::Grid>();
    let court = *app.world_mut().resource::<fractal::game::Court>();
    assert!(
        grid.height > grid.width,
        "a tall window should get a tall grid, got {grid:?}"
    );
    assert_eq!(
        app.world_mut().resource::<Field>().samples.len(),
        grid.cells(),
        "the field should have been resized with the grid"
    );

    // Cells have to stay square, or every fractal in here comes out stretched.
    let cell = vec2(
        court.0.x / grid.width as f32,
        court.0.y / grid.height as f32,
    );
    let squareness = cell.x / cell.y;
    assert!(
        (0.96..1.04).contains(&squareness),
        "cells should be square, got {cell:?} ({squareness:.3})"
    );
}
