//! The two dials: how many moths there are, and how fast the room runs.

use flutter::game::{
    ARENA, Flock, GamePlugin, Lamp, MAX_MOTHS, Moth, Paused, START_MOTHS, Speed, WING_FRAMES,
    wing_frame,
};
use fulcrum::prelude::*;

/// A room, ready to tick.
fn room(seed: u64) -> Fulcrum {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed,
        ..Default::default()
    })
    .with_plugin(GamePlugin);
    app.run_startup();
    app
}

/// Tick `ticks` times with `key` held (or nothing held, for `None`).
fn hold(app: &mut Fulcrum, key: Option<Key>, ticks: u32) {
    for _ in 0..ticks {
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            if let Some(key) = key {
                input.push_key(key, true);
            }
            input.sample(|screen| screen);
        }
        app.tick();
    }
    if let Some(key) = key {
        let mut input = app.world_mut().resource_mut::<Input>();
        input.push_key(key, false);
        input.sample(|screen| screen);
    }
}

/// Every moth's ordinal and position.
fn swarm(app: &mut Fulcrum) -> Vec<(u32, Vec2)> {
    let world = app.world_mut();
    let mut poses: Vec<_> = world
        .query::<(&Moth, &Transform2D)>()
        .iter(world)
        .map(|(moth, transform)| (moth.ordinal, transform.translation))
        .collect();
    poses.sort_by_key(|(ordinal, _)| *ordinal);
    poses
}

/// How far the average moth is from `at`.
fn mean_distance(app: &mut Fulcrum, at: Vec2) -> f32 {
    let poses = swarm(app);
    poses.iter().map(|(_, p)| p.distance(at)).sum::<f32>() / poses.len() as f32
}

#[test]
fn the_room_starts_with_a_swarm_in_it() {
    let mut app = room(7);
    hold(&mut app, None, 1);
    assert_eq!(app.world_mut().resource::<Flock>().count, START_MOTHS);
    assert_eq!(swarm(&mut app).len(), START_MOTHS as usize);
}

#[test]
fn holding_up_adds_moths_and_holding_down_takes_them_away() {
    let mut app = room(7);
    hold(&mut app, Some(Key::Up), 60);
    let grown = app.world_mut().resource::<Flock>().count;
    assert!(
        grown > START_MOTHS * 2,
        "sixty ticks of holding up should more than double the swarm, got {grown}"
    );
    assert_eq!(
        swarm(&mut app).len(),
        grown as usize,
        "counted, not claimed"
    );

    hold(&mut app, Some(Key::Down), 60);
    let cut = app.world_mut().resource::<Flock>().count;
    assert!(
        cut < grown / 2,
        "and holding down should undo it, got {cut}"
    );
    assert_eq!(swarm(&mut app).len(), cut as usize);
}

#[test]
fn a_tap_is_worth_one_moth_however_few_are_left() {
    let mut app = room(7);
    app.world_mut().insert_resource(Flock {
        target: 4,
        count: 0,
        restock: false,
    });
    hold(&mut app, None, 1);
    hold(&mut app, Some(Key::Up), 1);
    assert_eq!(app.world_mut().resource::<Flock>().count, 5);
    hold(&mut app, Some(Key::Down), 1);
    assert_eq!(app.world_mut().resource::<Flock>().count, 4);
}

#[test]
fn the_swarm_stops_at_the_cap_and_at_nothing() {
    let mut app = room(7);
    hold(&mut app, Some(Key::Up), 400);
    assert_eq!(app.world_mut().resource::<Flock>().count, MAX_MOTHS);
    hold(&mut app, Some(Key::Down), 600);
    assert_eq!(app.world_mut().resource::<Flock>().count, 0);
    assert!(swarm(&mut app).is_empty(), "and the room should be empty");
}

#[test]
fn taking_moths_away_leaves_the_others_flying_exactly_as_they_were() {
    // The claim the whole design rests on: a moth's path depends on nothing but that moth, so
    // the swarm can be cut in half mid-flight without nudging a single survivor.
    let mut cut = room(11);
    hold(&mut cut, None, 60);
    hold(&mut cut, Some(Key::Down), 40);
    hold(&mut cut, None, 120);

    let mut whole = room(11);
    hold(&mut whole, None, 220);

    let survivors = swarm(&mut cut);
    let untouched = swarm(&mut whole);
    assert!(
        survivors.len() < untouched.len() && !survivors.is_empty(),
        "the cut run should have lost moths and kept some"
    );
    for (ordinal, at) in survivors {
        assert_eq!(
            at, untouched[ordinal as usize].1,
            "moth {ordinal} should not have noticed the others leaving"
        );
    }
}

#[test]
fn a_new_swarm_is_a_different_swarm() {
    let mut app = room(3);
    hold(&mut app, None, 30);
    let before = swarm(&mut app);
    hold(&mut app, Some(Key::R), 1);
    hold(&mut app, None, 1);
    let after = swarm(&mut app);
    assert_eq!(after.len(), before.len(), "same count");
    assert!(after != before, "but freshly drawn");
}

#[test]
fn speed_only_rescales_time() {
    // Two rooms drawn from the same seed, one run at double speed for half as long. Speed is a
    // multiplier on the step, not a different simulation, so the two should end up in very
    // nearly the same place — all that differs is the size of the integration step.
    let mut slow = room(5);
    let mut fast = room(5);
    fast.world_mut().insert_resource(Speed(2.0));
    hold(&mut slow, None, 1);
    hold(&mut fast, None, 1);

    let start = swarm(&mut slow);
    hold(&mut slow, None, 120);
    hold(&mut fast, None, 60);

    let flown = |poses: Vec<(u32, Vec2)>| {
        poses
            .iter()
            .zip(&start)
            .map(|((_, now), (_, then))| now.distance(*then))
            .sum::<f32>()
            / poses.len() as f32
    };
    let (slow_flown, fast_flown) = (flown(swarm(&mut slow)), flown(swarm(&mut fast)));
    assert!(slow_flown > 20.0, "the swarm should have gone somewhere");
    let ratio = fast_flown / slow_flown;
    assert!(
        (0.85..1.15).contains(&ratio),
        "double speed for half the ticks should land in the same place, got {ratio:.2}x"
    );
}

#[test]
fn the_speed_keys_stay_inside_their_limits() {
    let mut app = room(5);
    hold(&mut app, Some(Key::Right), 600);
    assert_eq!(app.world_mut().resource::<Speed>().0, 8.0);
    hold(&mut app, Some(Key::Left), 600);
    assert_eq!(app.world_mut().resource::<Speed>().0, 0.05);
    hold(&mut app, Some(Key::Digit0), 1);
    assert_eq!(app.world_mut().resource::<Speed>().0, 1.0);
}

#[test]
fn pausing_holds_every_moth_where_it_is() {
    let mut app = room(9);
    hold(&mut app, None, 40);
    let before = swarm(&mut app);
    hold(&mut app, Some(Key::Space), 1);
    assert!(app.world_mut().resource::<Paused>().0);
    hold(&mut app, None, 60);
    assert_eq!(swarm(&mut app), before, "nothing moves while it is still");
}

#[test]
fn the_lamp_gathers_the_swarm_and_putting_it_out_lets_them_go() {
    let mut lit = room(13);
    let mut dark = room(13);
    dark.world_mut().insert_resource(Lamp {
        at: Vec2::ZERO,
        on: false,
    });
    hold(&mut lit, None, 1);
    let spread = mean_distance(&mut lit, Vec2::ZERO);
    hold(&mut lit, None, 360);
    hold(&mut dark, None, 361);

    let gathered = mean_distance(&mut lit, Vec2::ZERO);
    let scattered = mean_distance(&mut dark, Vec2::ZERO);
    assert!(
        gathered < spread,
        "the lamp should draw the swarm in: {spread:.0} -> {gathered:.0}"
    );
    assert!(
        gathered < scattered,
        "and an unlit room should leave them further out: {gathered:.0} vs {scattered:.0}"
    );
}

#[test]
fn moths_stay_in_the_room() {
    let mut app = room(17);
    app.world_mut().insert_resource(Speed(4.0));
    hold(&mut app, None, 900);
    let limit = ARENA / 2.0;
    for (ordinal, at) in swarm(&mut app) {
        assert!(
            at.x.abs() <= limit.x && at.y.abs() <= limit.y,
            "moth {ordinal} got out at {at:?}"
        );
    }
}

#[test]
fn the_wingbeat_covers_every_frame_and_survives_a_ragged_phase() {
    let seen: Vec<u32> = (0..WING_FRAMES)
        .map(|frame| wing_frame(frame as f32 / WING_FRAMES as f32 + 0.01))
        .collect();
    assert_eq!(seen, (0..WING_FRAMES).collect::<Vec<_>>());
    assert_eq!(wing_frame(1.0), 0, "a full beat is back at the start");
    assert!(
        wing_frame(-0.2) < WING_FRAMES,
        "and a negative phase is safe"
    );
}
