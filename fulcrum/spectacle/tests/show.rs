//! The rules the show is built on: the arc of a shell, the shape of the programme, the delay
//! on the sound, and the promise that the sky stays inside its limits.

use fulcrum::prelude::*;
use spectacle::game::{
    self, Act, BURST_CLIMB, Census, GRAVITY, GamePlugin, MAX_SPARKS, Paused, Pending, Shell, Show,
    Spark, launch_speed,
};

/// A show driven for `ticks`, with an optional bit of input each tick.
fn show(seed: u64, ticks: u32, mut script: impl FnMut(&mut Input, u32)) -> Fulcrum {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed,
        window_size: (1280, 720),
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
    app
}

/// Every star in the sky, as positions.
fn sky(app: &mut Fulcrum) -> Vec<Vec2> {
    let world = app.world_mut();
    world
        .query_filtered::<&Transform2D, With<Spark>>()
        .iter(world)
        .map(|transform| transform.translation)
        .collect()
}

#[test]
fn a_shell_is_thrown_exactly_as_high_as_it_is_aimed() {
    // The launch speed is the whole fuse: the break happens when the climb runs out, so if
    // this arithmetic is wrong the shells break in the wrong half of the sky.
    for height in [120.0_f32, 300.0, 520.0, 800.0] {
        let speed = launch_speed(height);
        assert!(
            (speed * speed - 2.0 * GRAVITY * height).abs() < 0.5,
            "v^2 should be 2gh at {height}"
        );

        // Fly it the way the simulation does and see where it gives up.
        let dt = 1.0 / 60.0;
        let (mut climb, mut y) = (speed, 0.0_f32);
        while climb > BURST_CLIMB {
            climb -= GRAVITY * dt;
            y += climb * dt;
        }
        let error = (y - height).abs() / height;
        assert!(
            error < 0.06,
            "a shell aimed at {height} broke at {y}, which is {:.1}% out",
            error * 100.0
        );
    }
}

#[test]
fn a_shell_aimed_at_the_ground_does_not_fly() {
    assert_eq!(launch_speed(0.0), 0.0);
    assert_eq!(launch_speed(-100.0), 0.0);
}

#[test]
fn the_programme_is_a_round() {
    let mut act = Act::Overture;
    let mut seen = Vec::new();
    for _ in 0..5 {
        seen.push(act);
        act = act.next();
    }
    assert_eq!(act, Act::Overture, "the round should close");
    assert_eq!(seen.len(), 5, "five passages, each visited once");
    for one in seen {
        assert!(!one.name().is_empty());
        let (short, long) = one.gap();
        assert!(short > 0.0 && short < long, "{} has a bad gap", one.name());
        let (fewest, most) = one.salvo();
        assert!(
            fewest >= 1 && fewest <= most,
            "{} has a bad salvo",
            one.name()
        );
        let (weakest, strongest) = one.power();
        assert!(
            weakest > 0.0 && weakest < strongest,
            "{} has a bad power range",
            one.name()
        );
        assert!(
            !one.repertoire().is_empty(),
            "{} has nothing to fire",
            one.name()
        );
        assert!(one.duration() > 0.0);
    }
}

#[test]
fn the_hush_is_quieter_than_the_finale() {
    // The shape of the show is the point of it, so the ordering of the passages is worth a
    // test of its own: a finale that fires no faster than the hush is not a finale.
    assert!(Act::Finale.gap().1 < Act::Hush.gap().0);
    assert!(Act::Cascade.gap().1 < Act::Overture.gap().0);
    assert!(Act::Finale.salvo().1 > Act::Overture.salvo().1);
}

#[test]
fn the_sound_arrives_after_the_light() {
    let field = game::DEFAULT_FIELD;
    let overhead = vec2(0.0, -field.y * 0.5 + 60.0);
    let high = vec2(0.0, field.y * 0.35);
    let far = vec2(field.x * 0.45, field.y * 0.4);

    assert!(game::travel_delay(overhead, field) < 0.1);
    assert!(game::travel_delay(high, field) > 0.4);
    assert!(game::travel_delay(far, field) > game::travel_delay(high, field));

    // Distance takes the top off the volume as well as delaying it.
    assert!(game::travel_volume(far, field) < game::travel_volume(overhead, field));
    for at in [overhead, high, far] {
        let volume = game::travel_volume(at, field);
        assert!(
            (0.0..=1.0).contains(&volume),
            "volume {volume} out of range"
        );
    }

    assert!(game::travel_pan(vec2(-600.0, 0.0), field) < -0.5);
    assert!(game::travel_pan(vec2(600.0, 0.0), field) > 0.5);
    assert!(game::travel_pan(vec2(0.0, 0.0), field).abs() < 1e-6);
    assert!(game::travel_pan(vec2(9000.0, 0.0), field) <= 0.85);
}

#[test]
fn stars_go_out_before_they_reach_the_water() {
    let field = game::DEFAULT_FIELD;
    let water = game::horizon(field);
    assert_eq!(game::water_fade(water, field), 0.0);
    assert_eq!(game::water_fade(water - 50.0, field), 0.0);
    assert_eq!(game::water_fade(water + 400.0, field), 1.0);
    let low = game::water_fade(water + 20.0, field);
    let high = game::water_fade(water + 50.0, field);
    assert!(low > 0.0 && low < high && high < 1.0, "{low} then {high}");
}

#[test]
fn the_water_line_sits_inside_the_field() {
    for size in [
        game::DEFAULT_FIELD,
        vec2(1600.0, 576.0),
        vec2(700.0, 1300.0),
    ] {
        let water = game::horizon(size);
        assert!(water > -size.y * 0.5 && water < 0.0, "water at {water}");
    }
}

#[test]
fn a_resize_keeps_the_area_and_clamps_the_shape() {
    let area = game::DEFAULT_FIELD.x * game::DEFAULT_FIELD.y;
    for window in [
        vec2(1280.0, 720.0),
        vec2(3840.0, 600.0),
        vec2(400.0, 1400.0),
        vec2(1000.0, 1000.0),
    ] {
        let field = game::field_for_window(window);
        assert!(
            ((field.x * field.y) / area - 1.0).abs() < 0.02,
            "{field:?} does not hold the area"
        );
        let aspect = field.x / field.y;
        assert!(
            aspect >= game::ASPECT_LIMITS.0 - 0.01 && aspect <= game::ASPECT_LIMITS.1 + 0.01,
            "aspect {aspect} outside the limits"
        );
    }
}

#[test]
fn the_show_puts_something_in_the_sky_and_keeps_count() {
    let mut app = show(7, 600, |_, _| {});
    let census = *app.world_mut().resource::<Census>();
    let stars = sky(&mut app).len() as u32;
    assert!(stars > 0, "nothing is burning");
    assert_eq!(census.sparks, stars, "the count should match the sky");
    assert!(
        app.world_mut().resource::<Show>().fired > 0,
        "nothing fired"
    );
}

#[test]
fn nothing_burns_below_the_water() {
    let mut app = show(11, 900, |_, _| {});
    let field = app.world_mut().resource::<game::Field>().0;
    let water = game::horizon(field);
    for at in sky(&mut app) {
        assert!(at.y > water, "a star at {at:?} is under the water");
    }
}

#[test]
fn the_finale_stays_inside_its_budget() {
    // Held at the busiest the show ever gets, for long enough to fill the sky several times
    // over. The cap is a promise about the tick, so it is worth holding it to.
    let mut app = show(3, 2400, |input, tick| {
        if tick % 240 == 0 {
            input.push_key(Key::F, true);
        }
        if tick % 240 == 1 {
            input.push_key(Key::F, false);
        }
    });
    let census = *app.world_mut().resource::<Census>();
    assert!(
        census.sparks <= MAX_SPARKS,
        "{} stars is over the cap",
        census.sparks
    );
    assert_eq!(census.sparks, sky(&mut app).len() as u32);
}

#[test]
fn f_brings_the_finale_forward() {
    let mut app = show(5, 120, |input, tick| {
        if tick == 60 {
            input.push_key(Key::F, true);
        }
        if tick == 61 {
            input.push_key(Key::F, false);
        }
    });
    assert_eq!(app.world_mut().resource::<Show>().act, Act::Finale);
}

#[test]
fn clicking_the_sky_fires_a_shell() {
    let mut app = show(9, 40, |input, tick| {
        if tick == 20 {
            input.push_cursor(vec2(220.0, 260.0));
            input.push_mouse_button(MouseButton::Left, true);
        }
        if tick == 24 {
            input.push_mouse_button(MouseButton::Left, false);
        }
    });
    let world = app.world_mut();
    let shells = world.query::<&Shell>().iter(world).count();
    assert!(shells > 0, "the click should have put a shell in the air");
}

#[test]
fn clicking_the_water_fires_nothing() {
    let mut app = show(9, 40, |input, tick| {
        if tick == 20 {
            // Below the water line, where a shell has nowhere to go.
            input.push_cursor(vec2(220.0, -320.0));
            input.push_mouse_button(MouseButton::Left, true);
        }
        if tick == 24 {
            input.push_mouse_button(MouseButton::Left, false);
        }
    });
    assert_eq!(app.world_mut().resource::<Show>().fired, 0);
    let world = app.world_mut();
    assert_eq!(world.query::<&Shell>().iter(world).count(), 0);
}

#[test]
fn stillness_stops_everything() {
    let mut app = show(13, 700, |input, tick| {
        if tick == 690 {
            input.push_key(Key::Space, true);
        }
        if tick == 691 {
            input.push_key(Key::Space, false);
        }
    });
    assert!(
        app.world_mut().resource::<Paused>().0,
        "space should still it"
    );
    let before = sky(&mut app);
    assert!(!before.is_empty(), "nothing to hold still");
    let queued = app.world_mut().resource::<Pending>().0.len();
    for _ in 0..60 {
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            input.sample(|screen| screen);
        }
        app.tick();
    }
    assert_eq!(before, sky(&mut app), "a still sky should not move");
    assert_eq!(
        queued,
        app.world_mut().resource::<Pending>().0.len(),
        "the sound should wait with everything else"
    );
}

#[test]
fn a_report_waits_on_the_queue_and_then_lands() {
    // The queue is the only place a sound can hide. A shell fired by hand puts one on it, and
    // a second later, which is longer than any crossing takes, it is gone again.
    let mut app = show(17, 12, |input, tick| {
        if tick == 10 {
            input.push_cursor(vec2(0.0, 300.0));
            input.push_mouse_button(MouseButton::Left, true);
        }
        if tick == 11 {
            input.push_mouse_button(MouseButton::Left, false);
        }
    });
    let queue = app.world_mut().resource::<Pending>().0.clone();
    assert!(!queue.is_empty(), "the launch should be on its way");
    for (left, _) in &queue {
        assert!(
            *left > 0.0 && *left < 3.0,
            "a report {left} seconds out is not on its way anywhere"
        );
    }

    // Left running, the queue never piles up: everything on it has somewhere to be.
    let mut longest = 0;
    for _ in 0..1200 {
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            input.sample(|screen| screen);
        }
        app.tick();
        longest = longest.max(app.world_mut().resource::<Pending>().0.len());
    }
    assert!(longest > 0, "a running show should make some noise");
    assert!(longest < 200, "{longest} reports queued at once is a leak");
}
