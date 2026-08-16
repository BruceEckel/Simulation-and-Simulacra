//! The properties the night is built on, headless.
//!
//! Whether it actually helps anybody sleep is not testable here, and neither is whether it is
//! nice to look at. What is testable is the set of promises the piece makes, and every one of
//! them is a promise about something *not* happening: nothing changes quickly, nothing is still
//! moving once the light has gone, nothing keeps making noise forever, nothing escapes.

use fulcrum::prelude::*;
use lullaby::game::{
    self, Breath, DEFAULT_FIELD, DEPTH_RATE, Depth, FIELD_COMMAND, GamePlugin, LIGHT_FULL,
    LIGHT_OUT, NIGHT_STEP, Night, REPRIEVE, SETTLED, STARS, Star, VOICE_HOLD, Velocity,
    breath_period, drag, field_payload, home_pull, inhale_fraction, jitter, luminance,
    star_presence, voice_level,
};

/// A headless night of `minutes`, with startup run.
fn night(seed: u64, minutes: f32) -> Fulcrum {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed,
        window_size: (1024, 768),
        ..Default::default()
    })
    .with_plugin(GamePlugin);
    app.run_startup();
    app.world_mut().insert_resource(Night(minutes * 60.0));
    app
}

fn run(app: &mut Fulcrum, ticks: u32) {
    for _ in 0..ticks {
        app.tick();
    }
}

/// Tap a key for one tick and release it.
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

/// Thin the sky down to `keep` stars.
///
/// The tests below are about the schedule and about how one star behaves, and a few hundred
/// answer both exactly as well as three thousand, while a full sky over several simulated
/// minutes is minutes of real time in CI for no extra confidence.
fn thin_to(app: &mut Fulcrum, keep: usize) {
    let world = app.world_mut();
    let extra: Vec<Entity> = world
        .query_filtered::<Entity, With<Star>>()
        .iter(world)
        .skip(keep)
        .collect();
    for entity in extra {
        world.despawn(entity);
    }
}

fn depth_of(app: &mut Fulcrum) -> f32 {
    app.world_mut().resource::<Depth>().now
}

#[test]
fn the_night_arrives_at_stillness() {
    // The claim: it does not merely get too dim to see, it stops. By the end every star is
    // exactly on its resting place with exactly no velocity, and stays there, so there is
    // nothing left in the window that could catch a half-open eye.
    let mut app = night(3, 5.0);
    thin_to(&mut app, 250);
    run(&mut app, 18_400); // the whole five minutes, and a little over

    assert_eq!(
        depth_of(&mut app),
        1.0,
        "the night should have run its course"
    );
    let world = app.world_mut();
    let mut counted = 0;
    for (star, transform, velocity) in world
        .query::<(&Star, &Transform2D, &Velocity)>()
        .iter(world)
    {
        assert_eq!(velocity.0, Vec2::ZERO, "a star should end at rest");
        assert_eq!(
            transform.translation, star.home,
            "and end on its resting place"
        );
        counted += 1;
    }
    assert_eq!(counted, 250, "and all of them should have got there");
}

#[test]
fn the_sky_stops_while_there_is_still_light_to_see_it_by() {
    // The order matters more than either event. Motion ends at SETTLED and the light lasts until
    // LIGHT_OUT, so there is a stretch of the night where the sky is finished and still lit: the
    // settled picture is something you get to see arrive. The other order (going dark while
    // things are still moving) leaves you straining at a window that still has something in it.
    const {
        assert!(
            SETTLED < LIGHT_OUT,
            "the motion should stop before the light does"
        );
    }
    assert_eq!(jitter(SETTLED), 0.0, "and stop completely");
    assert_eq!(jitter(1.0), 0.0);
    assert!(
        luminance(SETTLED) > 0.25,
        "with a good part of the light left to see it by: {:.2}",
        luminance(SETTLED)
    );

    // And the cooling is gradual all the way down rather than being switched off at the end.
    let mut last = f32::INFINITY;
    let mut depth = 0.0;
    while depth <= SETTLED {
        let now = jitter(depth);
        assert!(
            now <= last + 1e-6,
            "the sky should only ever cool: {depth:.2}"
        );
        last = now;
        depth += 0.01;
    }
    // The medium thickens as the pull grows, which is what keeps the arrival overdamped: a star
    // drifts onto its place rather than swinging past it and coming back.
    for depth in [0.0, 0.25, 0.5, SETTLED, 1.0] {
        let critical = 2.0 * (home_pull(depth) * 1.4).sqrt();
        assert!(
            drag(depth) > critical * 0.75,
            "the settling should stay near or past critical damping at depth {depth}: \
             drag {:.2} against {:.2}",
            drag(depth),
            critical
        );
    }
}

#[test]
fn nothing_changes_suddenly() {
    // The load-bearing promise. Every control writes the schedule, never the state, and the
    // state walks toward the schedule at a fixed rate. So no key, no resize, and no restart can
    // produce a step change in what the room looks like.
    //
    // This drives the piece through the worst of it (a night cut to its shortest, a reprieve, a
    // restart, and a night stretched to its longest), and watches the size of every step.
    let mut app = night(5, 5.0);
    thin_to(&mut app, 60);

    let mut previous = (depth_of(&mut app), luminance(depth_of(&mut app)));
    let mut worst_depth: f32 = 0.0;
    let mut worst_light: f32 = 0.0;
    let ceiling = DEPTH_RATE / 60.0 + 1e-6;

    for tick in 0..12_000u32 {
        // Exactly one tick per turn of this loop, so that every step measured below really is
        // one tick's worth of change rather than two of them lumped together. Each key is held
        // for the first tick of its pair and let go on the second.
        let key = match tick {
            2_000 | 2_001 => Some(Key::Digit1),
            5_000 | 5_001 => Some(Key::Space),
            7_000 | 7_001 => Some(Key::R),
            9_000 | 9_001 => Some(Key::Digit9),
            _ => None,
        };
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            if let Some(key) = key {
                input.push_key(key, tick.is_multiple_of(2));
            }
            input.sample(|screen| screen);
        }
        app.tick();
        let depth = depth_of(&mut app);
        worst_depth = worst_depth.max((depth - previous.0).abs());
        worst_light = worst_light.max((luminance(depth) - previous.1).abs());
        previous = (depth, luminance(depth));
    }

    assert!(
        worst_depth <= ceiling,
        "the night should never move faster than its rate: {worst_depth:.3e} against {ceiling:.3e}"
    );
    // Luminance has a bounded slope in depth, so a bounded step in depth is a bounded step in
    // brightness: about a twentieth of one percent per tick, at the very worst.
    let brightest_step = 1.5 / (LIGHT_OUT - LIGHT_FULL) * ceiling;
    assert!(
        worst_light <= brightest_step,
        "and the light with it: {worst_light:.3e} against {brightest_step:.3e}"
    );
    assert!(
        worst_depth > 0.0,
        "and it should have actually been moving, or this proves nothing"
    );
}

#[test]
fn the_breath_lengthens_and_the_release_takes_all_of_it() {
    // Six breaths a minute down to under four, and, the part that matters, the draw stays
    // where it is while the release swallows every second of the difference.
    assert!(
        (60.0 / breath_period(0.0) - 6.0).abs() < 0.01,
        "six a minute to begin with"
    );
    assert!(60.0 / breath_period(1.0) < 4.0, "under four by the end");

    let draw_at = |depth: f32| breath_period(depth) * inhale_fraction(depth);
    let release_at = |depth: f32| breath_period(depth) * (1.0 - inhale_fraction(depth));
    assert!(
        (draw_at(1.0) - draw_at(0.0)).abs() < 0.6,
        "the draw should stay about where it is: {:.2}s to {:.2}s",
        draw_at(0.0),
        draw_at(1.0)
    );
    assert!(
        release_at(1.0) > release_at(0.0) * 1.9,
        "and the release should roughly double: {:.2}s to {:.2}s",
        release_at(0.0),
        release_at(1.0)
    );
    for depth in [0.0, 0.5, 1.0] {
        assert!(
            release_at(depth) > draw_at(depth),
            "the release is always the longer half"
        );
    }

    // Both turns are eased, so there is no instant at which the breath changes direction sharply.
    let inhale = inhale_fraction(0.0);
    let slope = |cycle: f32| {
        (game::breath_phase(cycle + 0.001, inhale) - game::breath_phase(cycle - 0.001, inhale))
            / 0.002
    };
    assert_eq!(
        game::breath_phase(0.0, inhale),
        0.0,
        "a breath starts empty"
    );
    assert!(
        (game::breath_phase(inhale, inhale) - 1.0).abs() < 1e-5,
        "and fills at the turn"
    );
    assert!(slope(0.002).abs() < 0.5, "it should ease out of the bottom");
    assert!(slope(inhale - 0.002).abs() < 0.5, "and into the top");
}

#[test]
fn the_breath_never_stumbles_when_its_length_changes() {
    // The breath is held as a position in its cycle rather than as a number of seconds, so
    // changing the length of a breath changes only how fast that position advances. Held as
    // seconds, every change of pace would jolt the breath, and the piece changes the pace
    // continuously, all night.
    let mut app = night(11, 5.0);
    thin_to(&mut app, 40);

    let mut last = app.world_mut().resource::<Breath>().phase;
    let mut worst: f32 = 0.0;
    for tick in 0..9_000u32 {
        // Hit it with the biggest change of pace available, twice, mid-breath.
        match tick {
            1_500 => press(&mut app, Key::Digit9),
            4_000 => press(&mut app, Key::Digit1),
            _ => app.tick(),
        }
        let breath = *app.world_mut().resource::<Breath>();
        assert!(
            (0.0..=1.0).contains(&breath.phase),
            "the breath should stay in range: {}",
            breath.phase
        );
        worst = worst.max((breath.phase - last).abs());
        last = breath.phase;
    }
    // The fastest the breath can travel is a shade over a third of its range per second.
    assert!(
        worst < 0.01,
        "the breath should never jump: worst step {worst:.4}"
    );

    // The voice answers these counters, so each has to advance exactly once per breath: one
    // draw and one release, never two of either and never none.
    let breath = *app.world_mut().resource::<Breath>();
    assert!(
        (10..=16).contains(&breath.draws),
        "a sensible number of breaths in two and a half minutes: {}",
        breath.draws
    );
    assert!(
        breath.draws.abs_diff(breath.releases) <= 1,
        "every draw should be answered by a release: {} draws, {} releases",
        breath.draws,
        breath.releases
    );
}

#[test]
fn the_voice_outlasts_the_light() {
    // The point of the whole design. You cannot watch a screen with your eyes closed, so the
    // light is spent first and the sound carries the rest of the night on its own.
    assert_eq!(
        luminance(LIGHT_OUT),
        0.0,
        "the light should be properly gone"
    );
    assert_eq!(luminance(1.0), 0.0, "and stay gone");
    assert!(
        voice_level(LIGHT_OUT, 600.0) > 0.9,
        "while the voice is still at full strength: {:.2}",
        voice_level(LIGHT_OUT, 600.0)
    );
    const {
        assert!(VOICE_HOLD > LIGHT_OUT, "and only starts to go afterwards");
    }
    assert_eq!(voice_level(1.0, 6000.0), 0.0, "and it does go, in the end");

    // It comes up gently at the start, too: nothing at full strength in a dark room.
    assert_eq!(voice_level(0.0, 0.0), 0.0);
    assert!(
        voice_level(0.0, 5.0) < 0.05,
        "barely there after five seconds"
    );
    assert!(voice_level(0.0, 60.0) > 0.99, "and arrived after a minute");

    // Both ends of the light are flat, so it neither snaps on nor snaps off.
    assert_eq!(luminance(0.0), 1.0);
    assert_eq!(luminance(LIGHT_FULL), 1.0);
    let mut last = f32::INFINITY;
    let mut depth = 0.0;
    while depth <= 1.0 {
        let now = luminance(depth);
        assert!(
            now <= last + 1e-6,
            "the light should only ever fall: {depth:.2}"
        );
        last = now;
        depth += 0.005;
    }
}

#[test]
fn stars_go_out_one_at_a_time_and_none_of_them_comes_back() {
    // Every star has its own depth at which it starts to go, spread across a wide band, so the
    // sky thins unevenly instead of every star dimming in lockstep like a lamp on a dimmer.
    let app = &mut night(13, 25.0);
    let world = app.world_mut();
    let mut dims: Vec<f32> = world
        .query::<&Star>()
        .iter(world)
        .map(|star| star.dim_at)
        .collect();
    assert_eq!(dims.len(), STARS as usize);
    dims.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let spread = dims[dims.len() - 1] - dims[0];
    assert!(
        spread > 0.3,
        "the going-out should be spread wide: {spread:.2}"
    );

    // And each one is a smooth, one-way trip to nothing.
    for dim_at in [game::STAR_DIM.0, 0.5, game::STAR_DIM.1] {
        assert_eq!(star_presence(dim_at, 0.0), 1.0);
        assert_eq!(star_presence(dim_at, dim_at + game::STAR_FADE), 0.0);
        let mut last = f32::INFINITY;
        let mut depth = 0.0;
        while depth <= 1.0 {
            let now = star_presence(dim_at, depth);
            assert!((0.0..=1.0).contains(&now));
            assert!(now <= last + 1e-6, "a star never brightens again");
            last = now;
            depth += 0.005;
        }
    }
    // Everything is out before the ceiling reaches zero, so the last of the light leaves by the
    // stars going rather than by the ceiling cutting them off.
    assert!(game::STAR_DIM.1 + game::STAR_FADE <= LIGHT_OUT);
}

#[test]
fn a_star_wanders_in_a_cloud_it_cannot_leave() {
    // The physics, checked against what the physics says it should be. A spring shaken by
    // gaussian noise settles at a mean square displacement of temperature over stiffness, and
    // this piece leans on that: it is the reason a star needs no boundary, no wrapping, and no
    // turning around at an edge, and the reason the sky is a bounded haze at the top of the
    // night rather than something that slowly disperses.
    //
    // Getting the noise scaling wrong (a kick that does not carry its `sqrt(2·drag·T·dt)`)
    // still looks like motion, so nothing else here would catch it. This would: it would be
    // wrong by a factor of the tick rate.
    let mut app = night(17, 45.0);
    thin_to(&mut app, 800);

    let field = DEFAULT_FIELD;
    let mut furthest: f32 = 0.0;
    for _ in 0..3_600 {
        app.tick();
        let world = app.world_mut();
        for transform in world
            .query_filtered::<&Transform2D, With<Star>>()
            .iter(world)
        {
            furthest = furthest.max(
                (transform.translation.x.abs() / field.x)
                    .max(transform.translation.y.abs() / field.y),
            );
        }
    }

    let depth = depth_of(&mut app);
    let temperature = jitter(depth) * jitter(depth);
    let pull = home_pull(depth);
    let world = app.world_mut();
    let mut measured = 0.0;
    let mut predicted = 0.0;
    let mut count = 0.0;
    for (star, transform) in world.query::<(&Star, &Transform2D)>().iter(world) {
        measured += (transform.translation - star.home).length_squared();
        // Two axes, each with mean square displacement T / k.
        predicted += 2.0 * temperature / (pull * star.lag);
        count += 1.0;
    }
    let measured = (measured / count).sqrt();
    let predicted = (predicted / count).sqrt();
    assert!(
        measured > predicted * 0.7 && measured < predicted * 1.4,
        "a star's cloud should be the size the physics says: {measured:.1} against {predicted:.1}"
    );
    assert!(
        furthest < 0.85,
        "and no star should ever get near leaving the field: {:.2} of the way out",
        furthest
    );
}

#[test]
fn still_awake_hands_time_back_without_a_jolt() {
    // Press space when you are still awake and the night gives you a few minutes back. The
    // schedule jumps; the room does not, it walks back up over the following couple of minutes,
    // which is slow enough that you are unlikely to notice being given anything.
    let mut app = night(19, 10.0);
    thin_to(&mut app, 40);
    run(&mut app, 60 * 60 * 5); // five minutes in

    let before = *app.world_mut().resource::<Depth>();
    press(&mut app, Key::Space);
    let after = *app.world_mut().resource::<Depth>();

    assert!(
        (before.elapsed - after.elapsed - REPRIEVE).abs() < 0.2,
        "the schedule should hand back four minutes"
    );
    assert!(
        after.wanted < before.wanted,
        "so the night has less of it behind it"
    );
    assert!(
        (after.now - before.now).abs() < 0.01,
        "but what you see should barely have moved yet: {:.4}",
        (after.now - before.now).abs()
    );
    run(&mut app, 60 * 90);
    assert!(
        depth_of(&mut app) < before.now,
        "and should have walked back over the following minute and a half"
    );

    // Handing back more than there is does not run the clock backwards past the start.
    for _ in 0..6 {
        press(&mut app, Key::Space);
    }
    assert_eq!(
        app.world_mut().resource::<Depth>().elapsed.max(0.0),
        app.world_mut().resource::<Depth>().elapsed,
        "the night should never run backwards past its start"
    );
}

#[test]
fn the_sky_keeps_its_shape_when_the_window_changes() {
    let square = game::field_for_window(vec2(900.0, 900.0));
    assert!((square.x / square.y - 1.0).abs() < 0.01);
    let wide = game::field_for_window(vec2(2560.0, 1080.0));
    assert!((wide.x / wide.y - 2560.0 / 1080.0).abs() < 0.02);
    let area = DEFAULT_FIELD.x * DEFAULT_FIELD.y;
    assert!(
        ((wide.x * wide.y) / area - 1.0).abs() < 0.01,
        "a resize should hold the same area, so the sky keeps its density"
    );

    // A resize stretches the resting places with it rather than clipping them. Clipping would
    // pile every star that fell outside onto the new edge, which is the one arrangement the sky
    // must never have.
    let mut app = night(23, 25.0);
    thin_to(&mut app, 200);
    run(&mut app, 120);
    let before: Vec<Vec2> = {
        let world = app.world_mut();
        world.query::<&Star>().iter(world).map(|s| s.home).collect()
    };
    app.world_mut()
        .resource_mut::<CommandOutbox>()
        .send(FIELD_COMMAND, field_payload(vec2(1400.0, 560.0)));
    app.tick();

    let scale = vec2(1400.0 / DEFAULT_FIELD.x, 560.0 / DEFAULT_FIELD.y);
    let world = app.world_mut();
    let after: Vec<Vec2> = world.query::<&Star>().iter(world).map(|s| s.home).collect();
    assert_eq!(before.len(), after.len());
    for (was, now) in before.iter().zip(&after) {
        assert!(
            (*was * scale - *now).length() < 0.01,
            "resting places should stretch with the window, not be clipped to it"
        );
        assert!(
            now.x.abs() <= 700.0 && now.y.abs() <= 280.0,
            "and all of them should still be inside it"
        );
    }
}

#[test]
fn the_sky_has_a_band_across_it() {
    // The resting places are drawn from a density with a band in it, so the settled sky is a
    // place rather than a texture. An even scattering is the obvious thing to do and it is worth
    // exactly nothing: it looks like noise, because it is.
    let mut app = night(29, 25.0);
    let world = app.world_mut();
    let homes: Vec<Vec2> = world.query::<&Star>().iter(world).map(|s| s.home).collect();

    let dense = homes
        .iter()
        .filter(|home| game::sky_density(**home, DEFAULT_FIELD) > 0.8)
        .count() as f32
        / homes.len() as f32;
    let area = {
        // What share of the field that same test covers, so the comparison is against chance.
        let mut inside = 0.0;
        let mut total = 0.0;
        for row in 0..64 {
            for column in 0..64 {
                let point = vec2(
                    -DEFAULT_FIELD.x / 2.0 + DEFAULT_FIELD.x * (column as f32 + 0.5) / 64.0,
                    -DEFAULT_FIELD.y / 2.0 + DEFAULT_FIELD.y * (row as f32 + 0.5) / 64.0,
                );
                if game::sky_density(point, DEFAULT_FIELD) > 0.8 {
                    inside += 1.0;
                }
                total += 1.0;
            }
        }
        inside / total
    };
    assert!(
        dense > area * 1.4,
        "the band should actually be denser than the rest: {:.0}% of the stars in {:.0}% of \
         the sky",
        dense * 100.0,
        area * 100.0
    );
    // But the rest of the sky is not empty. A band with nothing around it is a stripe.
    assert!(
        dense < 0.8,
        "and the rest of the sky should still have stars in it: {:.0}%",
        dense * 100.0
    );
}

#[test]
fn the_night_is_a_sensible_length_however_it_is_asked_for() {
    // The rate limit is a floor on how long a descent can take. It has to sit under the shortest
    // night on offer, or the shortest night would be governed by the limit instead of by what
    // was asked for, and the number keys would quietly stop meaning anything.
    let floor = 1.0 / DEPTH_RATE;
    assert!(
        floor < NIGHT_STEP,
        "the slowest possible descent ({floor:.0}s) should still fit inside the shortest night"
    );
    assert!(
        floor > 60.0,
        "but it should be long enough to be unnoticeable"
    );
    assert_eq!(game::wanted_depth(0.0, 1500.0), 0.0);
    assert_eq!(game::wanted_depth(1500.0, 1500.0), 1.0);
    assert_eq!(
        game::wanted_depth(9999.0, 1500.0),
        1.0,
        "and it stops at the end"
    );
}
