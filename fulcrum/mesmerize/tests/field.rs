//! The properties the piece is built on, headless. Beauty isn't testable; the things that
//! would quietly destroy it are.

use fulcrum::prelude::*;
use mesmerize::game::{
    BREATH_INHALE, BREATH_PERIOD, Census, DEFAULT_FIELD, Field, GamePlugin, MAX_MOTES, Mote,
    START_MOTES, Speed, breath_phase, edge_presence, flow,
};

/// A headless field with the piece installed and startup run.
fn piece() -> Fulcrum {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed: 4,
        window_size: (1024, 768),
        ..Default::default()
    })
    .with_plugin(GamePlugin);
    app.run_startup();
    app
}

fn run(app: &mut Fulcrum, ticks: u32) {
    for _ in 0..ticks {
        app.tick();
    }
}

/// Hold a key down for `ticks` ticks, then release it.
fn hold(app: &mut Fulcrum, key: Key, ticks: u32) {
    for _ in 0..ticks {
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            input.push_key(key, true);
            input.sample(|screen| screen);
        }
        app.tick();
    }
    {
        let mut input = app.world_mut().resource_mut::<Input>();
        input.push_key(key, false);
        input.sample(|screen| screen);
    }
    app.tick();
}

#[test]
fn the_current_has_no_sources_and_no_sinks() {
    // The property the whole piece rests on. A field with divergence has places where motes
    // accumulate; within a few minutes everything is piled into them and there is nothing
    // left to watch. Curl fields cannot do that, and this checks that ours is really one:
    // divergence estimated numerically, everywhere, at several times.
    let field = DEFAULT_FIELD;
    let step = 0.5;
    let mut worst: f32 = 0.0;
    let mut typical_speed = 0.0;
    let mut samples = 0.0;
    for time in [0.0, 37.0, 180.0] {
        for row in 0..24 {
            for column in 0..24 {
                let point = vec2(
                    -field.x / 2.0 + field.x * (column as f32 + 0.5) / 24.0,
                    -field.y / 2.0 + field.y * (row as f32 + 0.5) / 24.0,
                );
                let dx = flow(point + vec2(step, 0.0), field, time).x
                    - flow(point - vec2(step, 0.0), field, time).x;
                let dy = flow(point + vec2(0.0, step), field, time).y
                    - flow(point - vec2(0.0, step), field, time).y;
                let divergence = (dx + dy) / (2.0 * step);
                worst = worst.max(divergence.abs());
                typical_speed += flow(point, field, time).length();
                samples += 1.0;
            }
        }
    }
    let typical_speed = typical_speed / samples;
    // Divergence is measured in units per second per unit; compare it against the field's own
    // scale, which is a speed over the distance it takes to turn over.
    let scale = typical_speed / field.x.min(field.y);
    assert!(
        worst < scale * 0.02,
        "the current should be divergence-free: worst {worst:.3e} against a field scale of {scale:.3e}"
    );
    assert!(
        (20.0..90.0).contains(&typical_speed),
        "and it should carry a mote at a watchable pace: {typical_speed:.1} units/s"
    );
}

/// Thin the field down to `keep` motes.
///
/// The long-run tests are about how the light *distributes*, which a twelfth of the motes
/// answers just as well as all of them — and unoptimized, a full field over several simulated
/// minutes is minutes of real time in CI for no extra confidence.
fn thin_to(app: &mut Fulcrum, keep: usize) {
    let world = app.world_mut();
    let extra: Vec<Entity> = world
        .query_filtered::<Entity, With<Mote>>()
        .iter(world)
        .skip(keep)
        .collect();
    for entity in extra {
        world.despawn(entity);
    }
    world.resource_mut::<Census>().0 = keep as u32;
}

/// How the light is distributed over a coarse grid: `(share held by the fullest cell,
/// fraction of cells holding any at all)`.
fn distribution(app: &mut Fulcrum) -> (f32, f32) {
    let world = app.world_mut();
    let mut cells = [0.0f32; 64];
    let mut total = 0.0f32;
    for position in world
        .query_filtered::<&Transform2D, With<Mote>>()
        .iter(world)
        .map(|transform| transform.translation)
    {
        let column = (((position.x / DEFAULT_FIELD.x + 0.5) * 8.0) as usize).min(7);
        let row = (((position.y / DEFAULT_FIELD.y + 0.5) * 8.0) as usize).min(7);
        cells[row * 8 + column] += 1.0;
        total += 1.0;
    }
    let fullest = cells.iter().fold(0.0f32, |a, b| a.max(*b)) / total.max(1.0);
    let occupied = cells.iter().filter(|n| **n > 0.0).count() as f32 / 64.0;
    (fullest, occupied)
}

#[test]
fn the_light_gathers_without_collapsing() {
    // The motes *should* gather — they carry inertia, so they are thrown out of the vortex
    // cores and collect along the strands between, and that gathering is the entire visible
    // structure of the piece. What must never happen is collapse: everything ending up in a
    // few knots with the rest of the field bare.
    //
    // So this doesn't ask for an even spread. It asks that no corner of the field ever holds
    // a large share of the light, and that the light keeps reaching nearly everywhere.
    let mut app = piece();
    thin_to(&mut app, 1_200);
    run(&mut app, 6_000); // a hundred seconds
    let (fullest, occupied) = distribution(&mut app);
    assert!(
        fullest < 0.10,
        "no part of the field should hoard the light: the fullest cell holds {:.0}%",
        fullest * 100.0
    );
    assert!(
        occupied > 0.9,
        "and it should still reach nearly everywhere: {:.0}% of the field occupied",
        occupied * 100.0
    );

    // And it is still like that much later, rather than slowly winding down into knots.
    run(&mut app, 12_000); // three and a half minutes more
    let (fullest, occupied) = distribution(&mut app);
    assert!(
        fullest < 0.10 && occupied > 0.9,
        "after five minutes: fullest cell {:.0}%, {:.0}% occupied",
        fullest * 100.0,
        occupied * 100.0
    );
}

#[test]
fn nothing_is_ever_seen_to_arrive_or_leave() {
    // Every mote's visibility is a product of its age and its distance from the boundary, and
    // both ends of both are smooth. Nothing in the picture may pop.
    let mut app = piece();
    thin_to(&mut app, 1_200);
    run(&mut app, 2_400);
    let world = app.world_mut();
    for (mote, transform) in world
        .query::<(&Mote, &Transform2D)>()
        .iter(world)
        .map(|(mote, transform)| (mote.presence(), transform.translation))
    {
        let presence = mote * edge_presence(transform, DEFAULT_FIELD);
        assert!(
            (0.0..=1.0).contains(&presence),
            "presence should stay in range: {presence}"
        );
    }
    // A mote at the boundary is invisible; one well inside is not.
    assert_eq!(
        edge_presence(vec2(DEFAULT_FIELD.x / 2.0, 0.0), DEFAULT_FIELD),
        0.0
    );
    assert_eq!(edge_presence(Vec2::ZERO, DEFAULT_FIELD), 1.0);
}

#[test]
fn it_breathes_at_five_and_a_half_a_minute() {
    assert_eq!(breath_phase(0.0), 0.0, "a breath starts at rest");
    assert!(
        (breath_phase(BREATH_PERIOD * BREATH_INHALE) - 1.0).abs() < 1e-5,
        "and fills at the top of the draw"
    );
    assert!(
        (breath_phase(BREATH_PERIOD) - 0.0).abs() < 1e-5,
        "and comes back to rest a full period later"
    );
    assert!(
        (60.0 / BREATH_PERIOD - 5.45).abs() < 0.1,
        "which is five and a half breaths a minute"
    );

    // The release is the longer half, and both turns are smooth rather than cornered.
    let draw = BREATH_PERIOD * BREATH_INHALE;
    assert!(draw < BREATH_PERIOD - draw, "the release should be longer");
    let slope = |t: f32| (breath_phase(t + 0.01) - breath_phase(t - 0.01)) / 0.02;
    assert!(slope(0.05).abs() < 0.5, "it should ease out of the turn");
    assert!(slope(draw - 0.05).abs() < 0.5, "and ease into the next one");

    // And it keeps its own time: the breath is real-time, so pace changes never touch it.
    let mut app = piece();
    hold(&mut app, Key::Up, 90);
    assert!(app.world_mut().resource::<Speed>().0 > 2.0);
    let start = app
        .world_mut()
        .resource::<mesmerize::game::Breath>()
        .seconds;
    run(&mut app, 60);
    let advanced = app
        .world_mut()
        .resource::<mesmerize::game::Breath>()
        .seconds
        - start;
    assert!(
        (advanced - 1.0).abs() < 0.1,
        "a second of ticks should be a second of breath whatever the pace: {advanced:.2}s"
    );
}

#[test]
fn the_amount_of_light_is_adjustable() {
    let mut app = piece();
    assert_eq!(app.world_mut().resource::<Census>().0, START_MOTES);

    hold(&mut app, Key::N, 40);
    let more = app.world_mut().resource::<Census>().0;
    assert!(more > START_MOTES, "holding N should add light: {more}");

    hold(&mut app, Key::M, 40);
    let fewer = app.world_mut().resource::<Census>().0;
    assert!(fewer < more, "holding M should take it away: {fewer}");
    let world = app.world_mut();
    let live = world.query_filtered::<(), With<Mote>>().iter(world).count();
    assert_eq!(live as u32, fewer, "the census should match the field");
    assert!(fewer <= MAX_MOTES);
}

#[test]
fn the_field_follows_the_window_shape() {
    let square = mesmerize::game::field_for_window(vec2(900.0, 900.0));
    assert!((square.x / square.y - 1.0).abs() < 0.01);
    let wide = mesmerize::game::field_for_window(vec2(2560.0, 1080.0));
    assert!((wide.x / wide.y - 2560.0 / 1080.0).abs() < 0.02);
    let area = DEFAULT_FIELD.x * DEFAULT_FIELD.y;
    assert!(
        ((wide.x * wide.y) / area - 1.0).abs() < 0.01,
        "and hold the same area, so the density of light does not change with the window"
    );
}

#[test]
fn a_still_field_stays_still() {
    let mut app = piece();
    run(&mut app, 300);
    let snapshot = |app: &mut Fulcrum| {
        let world = app.world_mut();
        world
            .query_filtered::<&Transform2D, With<Mote>>()
            .iter(world)
            .map(|transform| transform.translation)
            .collect::<Vec<_>>()
    };
    hold(&mut app, Key::Space, 1);
    let stilled = snapshot(&mut app);
    run(&mut app, 120);
    assert_eq!(snapshot(&mut app), stilled, "space should still the field");
    assert!(
        app.world_mut().resource::<Field>().0.x > 0.0,
        "and leave everything else alone"
    );
}
