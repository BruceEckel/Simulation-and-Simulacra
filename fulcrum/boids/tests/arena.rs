//! The resizing arena, headless. The window is what drives a resize in the real game, but the
//! simulation only ever sees a command — which is exactly why these tests can run with no
//! window at all, and why a replay can reproduce a resize.

use boids::game::{
    ARENA_COMMAND, Arena, Boid, DEFAULT_ARENA, GamePlugin, NEIGHBOR_RADIUS, Predator,
    arena_for_window, arena_payload, parse_arena,
};
use fulcrum::prelude::*;

/// A headless app with the flock installed and startup run.
fn app() -> Fulcrum {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed: 11,
        window_size: (1024, 768),
        ..Default::default()
    })
    .with_plugin(SpatialPlugin {
        cell_size: NEIGHBOR_RADIUS,
    })
    .with_plugin(GamePlugin);
    app.run_startup();
    app
}

/// Queue a resize the way the binary's `fit_window` does, then let it land.
fn resize(app: &mut Fulcrum, size: Vec2) {
    app.world_mut()
        .resource_mut::<CommandOutbox>()
        .send(ARENA_COMMAND, arena_payload(size));
    app.tick();
}

/// Every mover's position.
fn positions(app: &mut Fulcrum) -> Vec<Vec2> {
    let world = app.world_mut();
    world
        .query_filtered::<&Transform2D, Or<(With<Boid>, With<Predator>)>>()
        .iter(world)
        .map(|t| t.translation)
        .collect()
}

#[test]
fn arena_matches_the_window_shape_at_a_constant_area() {
    let area = DEFAULT_ARENA.x * DEFAULT_ARENA.y;
    for window in [
        vec2(1024.0, 768.0),
        vec2(2560.0, 1080.0),
        vec2(600.0, 1200.0),
    ] {
        let arena = arena_for_window(window);
        let want = window.x / window.y;
        let got = arena.x / arena.y;
        assert!(
            (got - want).abs() < 0.01,
            "arena {arena} should match window {window}'s aspect: {got:.3} vs {want:.3}"
        );
        assert!(
            ((arena.x * arena.y) / area - 1.0).abs() < 0.01,
            "arena {arena} should preserve the flock's area"
        );
    }
    // A pathological window shape gets clamped rather than producing a sliver of a world.
    let sliver = arena_for_window(vec2(4000.0, 100.0));
    assert!(sliver.x / sliver.y <= 3.51, "aspect should be clamped");
}

#[test]
fn a_resize_command_moves_the_walls_and_gathers_the_strays() {
    let mut app = app();
    for _ in 0..120 {
        app.tick();
    }
    let tall = vec2(600.0, 1200.0);
    resize(&mut app, tall);

    assert_eq!(app.world_mut().resource::<Arena>().0, tall);
    let limit = tall / 2.0;
    for position in positions(&mut app) {
        assert!(
            position.x.abs() <= limit.x && position.y.abs() <= limit.y,
            "a shrink left {position} outside the new arena"
        );
    }

    // And the flock keeps living inside the new shape rather than drifting back out.
    for _ in 0..600 {
        app.tick();
    }
    for position in positions(&mut app) {
        assert!(
            position.x.abs() <= limit.x && position.y.abs() <= limit.y,
            "{position} escaped the resized arena"
        );
    }
}

#[test]
fn resizes_replay_identically() {
    // The whole reason a resize is a command and not a direct write: the same sequence of
    // resizes has to produce bit-identical results, or replays of a resized session diverge.
    let run = || {
        let mut app = app();
        for tick in 0..600 {
            match tick {
                100 => resize(&mut app, vec2(1400.0, 560.0)),
                250 => resize(&mut app, vec2(700.0, 1120.0)),
                420 => resize(&mut app, DEFAULT_ARENA),
                _ => app.tick(),
            }
        }
        positions(&mut app)
            .iter()
            .map(|p| (p.x.to_bits(), p.y.to_bits()))
            .collect::<Vec<_>>()
    };
    assert_eq!(run(), run(), "a resized run must be reproducible");
}

#[test]
fn a_malformed_command_is_ignored() {
    let mut app = app();
    for payload in ["", "wide", "800", "800 x", "0 0", "-40 -40"] {
        app.world_mut()
            .resource_mut::<CommandOutbox>()
            .send(ARENA_COMMAND, payload);
        app.tick();
        assert_eq!(
            app.world_mut().resource::<Arena>().0,
            DEFAULT_ARENA,
            "payload {payload:?} should have been rejected"
        );
    }
    assert_eq!(parse_arena("1024 768"), Some(DEFAULT_ARENA));
}
