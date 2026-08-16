//! Behavior tests, headless: the population grows on schedule and stops at its ceiling, the
//! paddles actually defend their wall, and nothing leaves the court. These are the properties
//! that make it a simulation worth watching rather than a screensaver.

use fulcrum::prelude::*;
use rally::game::{
    BALL_EVERY, BALL_SIZE_MAX, Ball, Census, DEFAULT_COURT, GamePlugin, HOLD_DELAY, HOLD_PERIOD,
    MIN_EXTENT, PADDLE_EVERY, Paddle, Paused, SPEED_MAX, SPEED_MIN, START_BALLS, START_PADDLES,
    Speed, Stats, ball_size, paddle_shape, paddle_x,
};

/// A headless court with startup run.
fn court() -> Fulcrum {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed: 5,
        window_size: (1024, 768),
        ..Default::default()
    })
    .with_plugin(GamePlugin);
    app.run_startup();
    app
}

/// Run `ticks` ticks with no input.
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

/// Tap a key for one tick, then release it.
fn press(app: &mut Fulcrum, key: Key) {
    {
        let mut input = app.world_mut().resource_mut::<Input>();
        input.push_key(key, true);
        input.sample(|screen| screen);
    }
    app.tick();
    {
        let mut input = app.world_mut().resource_mut::<Input>();
        input.push_key(key, false);
        input.sample(|screen| screen);
    }
    app.tick();
}

/// Every ball's position.
fn balls(app: &mut Fulcrum) -> Vec<Vec2> {
    let world = app.world_mut();
    world
        .query_filtered::<&Transform2D, With<Ball>>()
        .iter(world)
        .map(|t| t.translation)
        .collect()
}

#[test]
fn it_opens_as_pong() {
    let mut app = court();
    let census = *app.world_mut().resource::<Census>();
    assert_eq!(census.balls, START_BALLS, "one ball to start");
    assert_eq!(census.paddles(), START_PADDLES, "two paddles to start");
    assert_eq!(
        (census.left, census.right),
        (1, 1),
        "one paddle on each wall"
    );
}

#[test]
fn the_population_grows_on_schedule() {
    let mut app = court();
    run(&mut app, BALL_EVERY as u32);
    assert_eq!(
        app.world_mut().resource::<Census>().balls,
        START_BALLS + 1,
        "a ball joins every {BALL_EVERY} ticks"
    );

    let mut app = court();
    run(&mut app, PADDLE_EVERY as u32);
    let census = *app.world_mut().resource::<Census>();
    assert_eq!(
        census.paddles(),
        START_PADDLES + 1,
        "a paddle joins every {PADDLE_EVERY} ticks"
    );
    assert_eq!(
        census.left.abs_diff(census.right),
        1,
        "paddles alternate walls, so the sides never drift apart"
    );
}

#[test]
fn the_population_never_stops_growing() {
    // Ten simulated minutes, well past any count the old ceilings allowed.
    let mut app = court();
    run(&mut app, 36_000);
    let census = *app.world_mut().resource::<Census>();
    assert_eq!(
        census.balls,
        START_BALLS + (36_000 / BALL_EVERY) as u32,
        "balls should keep arriving on schedule with no ceiling"
    );
    assert_eq!(
        census.paddles(),
        START_PADDLES + (36_000 / PADDLE_EVERY) as u32,
        "paddles should keep arriving on schedule with no ceiling"
    );

    // And the court is still playing: a wall of very thin paddles must still return balls,
    // which is the thing a sampled (non-swept) collision test would quietly lose here.
    let before = *app.world_mut().resource::<Stats>();
    run(&mut app, 1_200);
    let after = *app.world_mut().resource::<Stats>();
    let saves = after.saves - before.saves;
    let misses = after.misses - before.misses;
    assert!(
        saves > misses,
        "a crowded court should still be returning balls: {saves} saves vs {misses} misses"
    );
}

#[test]
fn balls_shrink_steadily_and_forever() {
    assert_eq!(ball_size(START_BALLS), BALL_SIZE_MAX);
    // No ceiling on the population, so the taper is checked far past any count a run is
    // likely to reach, and has to still be shrinking when it gets there.
    let sizes: Vec<f32> = (START_BALLS..300).map(ball_size).collect();
    let floored = sizes.iter().position(|size| *size <= MIN_EXTENT).unwrap();
    assert!(
        floored > 200,
        "balls should still be shrinking at 200 of them, not floored by {floored}"
    );
    assert!(
        sizes[..floored].windows(2).all(|pair| pair[1] < pair[0]),
        "every extra ball should shrink the set"
    );

    // Steady means every added ball costs the same *proportion* of the size, so the taper is
    // as visible between the first two balls as between the hundredth and hundred-first.
    let ratios: Vec<f32> = sizes[..floored]
        .windows(2)
        .map(|pair| pair[1] / pair[0])
        .collect();
    let (smallest, largest) = ratios.iter().fold((f32::MAX, 0.0f32), |acc, ratio| {
        (acc.0.min(*ratio), acc.1.max(*ratio))
    });
    assert!(
        largest - smallest < 0.001,
        "each ball should shrink the set by the same fraction: {smallest:.4}..{largest:.4}"
    );
    assert!(
        smallest < 0.99,
        "a step of {smallest:.4} is too small to notice"
    );
}

#[test]
fn paddles_shrink_steadily_and_forever() {
    // Sampled through the simulation, since a paddle's size depends on the whole population
    // and on how crowded its own wall is.
    let mut app = court();
    let opening = widest_paddle(&mut app);
    let mut previous = opening;
    for _ in 0..12 {
        run(&mut app, 3_000); // roughly five more paddles each pass
        let now = widest_paddle(&mut app);
        assert!(
            now.0 < previous.0 && now.1 < previous.1,
            "paddles should keep shrinking in both dimensions: {previous:?} -> {now:?}"
        );
        previous = now;
    }
    assert!(
        previous.0 < opening.0 / 3.0 && previous.1 < opening.1 / 3.0,
        "after 36000 ticks paddles should be a fraction of the opening pair: \
         {previous:?} vs {opening:?}"
    );
}

/// The largest `(thickness, length)` on the court right now.
fn widest_paddle(app: &mut Fulcrum) -> (f32, f32) {
    let census = *app.world_mut().resource::<Census>();
    let world = app.world_mut();
    world
        .query::<&Paddle>()
        .iter(world)
        .map(|paddle| {
            let shape = paddle_shape(DEFAULT_COURT, census, paddle.side, paddle.slot);
            (shape.half_thickness * 2.0, shape.half_length * 2.0)
        })
        .fold((0.0f32, 0.0f32), |acc, size| {
            (acc.0.max(size.0), acc.1.max(size.1))
        })
}

#[test]
fn holding_a_spawn_key_keeps_them_coming() {
    // A tap is one arrival; holding starts repeating after the delay.
    let mut app = court();
    let start = *app.world_mut().resource::<Census>();
    press(&mut app, Key::B);
    assert_eq!(
        app.world_mut().resource::<Census>().balls,
        start.balls + 1,
        "a tap should add exactly one"
    );

    let mut app = court();
    let start = *app.world_mut().resource::<Census>();
    let held = 120; // two seconds
    hold(&mut app, Key::B, held);
    let expected = 1 + (held - HOLD_DELAY) / HOLD_PERIOD;
    let added = app.world_mut().resource::<Census>().balls - start.balls;
    assert!(
        added.abs_diff(expected) <= 1,
        "holding for {held} ticks should add about {expected} balls, added {added}"
    );

    let mut app = court();
    let start = *app.world_mut().resource::<Census>();
    hold(&mut app, Key::P, held);
    let added = app.world_mut().resource::<Census>().paddles() - start.paddles();
    assert!(
        added.abs_diff(expected) <= 1,
        "holding P should pour in paddles too, added {added}"
    );
}

#[test]
fn the_court_can_be_sped_up_and_slowed_down() {
    // Speed scales simulated time: the clock, the schedule, and the motion together.
    let mut app = court();
    hold(&mut app, Key::Up, 90);
    let fast = app.world_mut().resource::<Speed>().0;
    assert!(
        fast > 2.0,
        "holding up should ramp the speed well past 1: {fast}"
    );

    let before = app.world_mut().resource::<Stats>().ticks;
    run(&mut app, 100);
    let elapsed = app.world_mut().resource::<Stats>().ticks - before;
    assert!(
        elapsed > 200,
        "at {fast:.2}x, 100 ticks should advance the clock by far more: {elapsed}"
    );

    hold(&mut app, Key::Down, 240);
    let slow = app.world_mut().resource::<Speed>().0;
    assert!(slow < 1.0, "holding down should drop below normal: {slow}");
    let before = app.world_mut().resource::<Stats>().ticks;
    run(&mut app, 100);
    let elapsed = app.world_mut().resource::<Stats>().ticks - before;
    assert!(
        elapsed < 100,
        "at {slow:.2}x, 100 ticks should advance the clock by fewer: {elapsed}"
    );

    press(&mut app, Key::Digit0);
    assert_eq!(app.world_mut().resource::<Speed>().0, 1.0, "0 resets speed");

    // And the ends are clamped rather than running away.
    hold(&mut app, Key::Up, 600);
    assert_eq!(app.world_mut().resource::<Speed>().0, SPEED_MAX);
    hold(&mut app, Key::Down, 1_200);
    assert_eq!(app.world_mut().resource::<Speed>().0, SPEED_MIN);
}

#[test]
fn every_ball_has_its_own_place_in_the_spectrum() {
    // The view spreads hues over `index / balls`, which only reads as a spectrum if the
    // indices are exactly 0..balls with no gaps and no repeats.
    let mut app = court();
    run(&mut app, 4_000);
    let balls = app.world_mut().resource::<Census>().balls;
    let world = app.world_mut();
    let mut indices: Vec<u32> = world
        .query::<&Ball>()
        .iter(world)
        .map(|ball| ball.index)
        .collect();
    indices.sort_unstable();
    assert_eq!(
        indices,
        (0..balls).collect::<Vec<_>>(),
        "ball indices should be a gapless 0..{balls}"
    );
}

#[test]
fn the_paddles_are_actually_playing() {
    let mut app = court();
    run(&mut app, 3_600); // one minute
    let stats = *app.world_mut().resource::<Stats>();
    assert!(
        stats.saves > 20,
        "paddles should be returning balls: {stats:?}"
    );
    assert!(
        stats.saves > stats.misses,
        "a rally should outlast the misses: {stats:?}"
    );
}

#[test]
fn nothing_leaves_the_court() {
    let mut app = court();
    run(&mut app, 6_000);
    let court_size = DEFAULT_COURT;
    for position in balls(&mut app) {
        assert!(
            position.x.abs() <= court_size.x / 2.0 + BALL_SIZE_MAX,
            "ball at {position} was not served again"
        );
        assert!(
            position.y.abs() <= court_size.y / 2.0,
            "ball at {position} passed through a wall"
        );
    }

    let census = *app.world_mut().resource::<Census>();
    let world = app.world_mut();
    let strays: Vec<_> = world
        .query::<(&Paddle, &Transform2D)>()
        .iter(world)
        .filter(|(paddle, transform)| {
            let shape = paddle_shape(court_size, census, paddle.side, paddle.slot);
            let slack = 0.01;
            (transform.translation.x - paddle_x(court_size, paddle.side)).abs() > slack
                || transform.translation.y < shape.travel.0 - slack
                || transform.translation.y > shape.travel.1 + slack
        })
        .map(|(_, transform)| transform.translation)
        .collect();
    assert!(
        strays.is_empty(),
        "paddles left their own stretch of wall: {strays:?}"
    );
}

#[test]
fn pause_freezes_the_court() {
    let mut app = court();
    run(&mut app, 600);
    press(&mut app, Key::Space);
    assert!(app.world_mut().resource::<Paused>().0, "space should pause");

    let before = balls(&mut app);
    let stats_before = *app.world_mut().resource::<Stats>();
    run(&mut app, 300);
    assert_eq!(balls(&mut app), before, "paused balls should not move");
    assert_eq!(
        *app.world_mut().resource::<Stats>(),
        stats_before,
        "a paused court should not age, score, or grow"
    );

    press(&mut app, Key::Space);
    run(&mut app, 60);
    assert_ne!(balls(&mut app), before, "space again should resume");
}

#[test]
fn the_controls_add_to_the_court() {
    let mut app = court();
    let before = *app.world_mut().resource::<Census>();
    press(&mut app, Key::B);
    press(&mut app, Key::P);
    let after = *app.world_mut().resource::<Census>();
    assert_eq!(after.balls, before.balls + 1, "b adds a ball");
    assert_eq!(after.paddles(), before.paddles() + 1, "p adds a paddle");

    run(&mut app, 1_200);
    press(&mut app, Key::R);
    let census = *app.world_mut().resource::<Census>();
    let stats = *app.world_mut().resource::<Stats>();
    assert_eq!(census.balls, START_BALLS, "r starts over");
    assert_eq!(census.paddles(), START_PADDLES, "r starts over");
    // The clock restarts too, but `press` ticks once more to release the key, so it is back
    // near zero rather than exactly zero.
    assert_eq!((stats.saves, stats.misses), (0, 0), "r resets the counters");
    assert!(stats.ticks <= 2, "r restarts the clock: {stats:?}");
}
