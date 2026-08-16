//! Determinism gate: run the court headless, same seed + scripted input twice, and require
//! bit-identical outcomes. The script covers the population controls and a resize, since both
//! feed the simulation from outside and are the likeliest places for a divergence to enter.

use fulcrum::prelude::*;
use rally::game::{
    Ball, COURT_COMMAND, Census, GamePlugin, Paddle, Stats, court_payload, parse_court,
};

/// Scripted input: add a paddle, add balls, pause, unpause.
fn script(input: &mut Input, tick: u32) {
    match tick {
        60 => input.push_key(Key::P, true),
        61 => input.push_key(Key::P, false),
        120 => input.push_key(Key::B, true),
        121 => input.push_key(Key::B, false),
        300 => input.push_key(Key::Space, true), // pause
        301 => input.push_key(Key::Space, false),
        360 => input.push_key(Key::Space, true), // and resume
        361 => input.push_key(Key::Space, false),
        _ => {}
    }
}

/// Run the sim for `ticks` and return the stats plus every transform as exact bits.
fn run(seed: u64, ticks: u32) -> (Stats, Census, Vec<(u32, u32)>) {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed,
        window_size: (1024, 768),
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
        // A resize partway through, the same way the window would send one.
        if tick == 500 {
            app.world_mut()
                .resource_mut::<CommandOutbox>()
                .send(COURT_COMMAND, court_payload(vec2(1400.0, 560.0)));
        }
        app.tick();
    }

    let stats = *app.world_mut().resource::<Stats>();
    let census = *app.world_mut().resource::<Census>();
    let world = app.world_mut();
    let transforms = world
        .query_filtered::<&Transform2D, Or<(With<Ball>, With<Paddle>)>>()
        .iter(world)
        .map(|t| (t.translation.x.to_bits(), t.translation.y.to_bits()))
        .collect();
    (stats, census, transforms)
}

#[test]
fn determinism_same_seed_same_outcome() {
    let a = run(42, 900);
    let b = run(42, 900);
    assert!(
        a.0.saves > 0,
        "nothing was returned; the run is not exercising much"
    );
    assert_eq!(a, b, "same seed + same input must be bit-identical");
}

#[test]
fn determinism_different_seeds_diverge() {
    // Proves the runs above match because the simulation is deterministic, not because the
    // seed is being ignored.
    assert_ne!(run(1, 400), run(2, 400), "the seed should matter");
}

#[test]
fn a_malformed_resize_is_ignored() {
    assert_eq!(parse_court("1400 560"), Some(vec2(1400.0, 560.0)));
    for payload in ["", "wide", "1400", "1400 x", "0 0", "-8 -8"] {
        assert_eq!(
            parse_court(payload),
            None,
            "payload {payload:?} should be rejected"
        );
    }
}
