//! The rules the joke rests on: balloons that go up and are counted once, a beat before anybody
//! falls, everybody surviving the landing, and nothing left behind in the sky.

use fulcrum::prelude::*;
use popped::game::{
    self, ARENA, Animal, Balloon, Basket, Census, GROUND, GamePlugin, Mood, Scrap, Tally,
};

/// A sky driven for `ticks`, with an optional bit of input each tick.
fn sky(seed: u64, ticks: u32, mut script: impl FnMut(&mut Input, u32)) -> Fulcrum {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed,
        window_size: (ARENA.x as u32, ARENA.y as u32),
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

/// Drive `ticks` more with no input.
fn drift(app: &mut Fulcrum, ticks: u32) {
    for _ in 0..ticks {
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            input.sample(|screen| screen);
        }
        app.tick();
    }
}

/// How many of each thing there is.
fn population(app: &mut Fulcrum) -> (usize, usize, usize) {
    let world = app.world_mut();
    let balloons = world.query::<&Balloon>().iter(world).count();
    let baskets = world.query::<&Basket>().iter(world).count();
    let animals = world.query::<&Animal>().iter(world).count();
    (balloons, baskets, animals)
}

/// Where the first balloon in the sky is, and how big.
fn first_balloon(app: &mut Fulcrum) -> Option<(Vec2, f32)> {
    let world = app.world_mut();
    world
        .query::<(&Balloon, &Transform2D)>()
        .iter(world)
        .map(|(balloon, transform)| (transform.translation, balloon.radius))
        .next()
}

#[test]
fn balloons_go_up_and_are_counted_once() {
    // A rig leaves exactly one mark on the tally. Getting this wrong is invisible in the sky
    // and obvious on the scoreboard, which is the only part of the piece that keeps score.
    let mut app = sky(7, 60 * 90, |_, _| {});
    let tally = *app.world_mut().resource::<Tally>();
    assert!(tally.escaped > 0, "nothing has got away in ninety seconds");
    // Ninety seconds, one balloon every second and a half at the very fastest.
    assert!(
        tally.escaped <= 60,
        "{} got away, which is more than could have gone up",
        tally.escaped
    );
    assert_eq!(tally.popped, 0, "nobody touched anything");
}

#[test]
fn nothing_is_left_behind_in_the_sky() {
    let mut app = sky(11, 60 * 90, |_, _| {});
    let (balloons, baskets, animals) = population(&mut app);
    // Every basket belongs to a balloon, since nothing has been popped: no orphans.
    assert_eq!(
        balloons, baskets,
        "{baskets} baskets for {balloons} balloons"
    );
    assert!(animals >= balloons, "every balloon carries somebody");
    assert!(
        balloons as u32 <= game::MAX_BALLOONS,
        "{balloons} balloons is over the cap"
    );
    let census = *app.world_mut().resource::<Census>();
    assert_eq!(census.balloons as usize, balloons, "the count should match");
    assert_eq!(census.animals as usize, animals, "the count should match");
}

#[test]
fn a_click_pops_the_balloon_under_it() {
    let mut app = sky(13, 60 * 6, |_, _| {});
    let (at, _) = first_balloon(&mut app).expect("a balloon to aim at");
    for tick in 0..4 {
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            input.push_cursor(at);
            if tick == 1 {
                input.push_mouse_button(MouseButton::Left, true);
            }
            if tick == 3 {
                input.push_mouse_button(MouseButton::Left, false);
            }
            input.sample(|screen| screen);
        }
        app.tick();
    }
    let tally = *app.world_mut().resource::<Tally>();
    assert_eq!(tally.popped, 1, "the click should have popped exactly one");
    let world = app.world_mut();
    assert_eq!(
        world.query::<&Scrap>().iter(world).count(),
        1,
        "and left the skin of it flying around"
    );
}

#[test]
fn a_click_on_empty_sky_pops_nothing() {
    let mut app = sky(13, 60 * 6, |_, _| {});
    for tick in 0..4 {
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            // The very bottom corner, where balloons have not got to yet.
            input.push_cursor(vec2(-ARENA.x * 0.49, GROUND + 4.0));
            if tick == 1 {
                input.push_mouse_button(MouseButton::Left, true);
            }
            if tick == 3 {
                input.push_mouse_button(MouseButton::Left, false);
            }
            input.sample(|screen| screen);
        }
        app.tick();
    }
    assert_eq!(app.world_mut().resource::<Tally>().popped, 0);
}

/// Pop the first balloon in the sky, and hand back where it was.
fn pop_one(app: &mut Fulcrum) -> Vec2 {
    let (at, _) = first_balloon(app).expect("a balloon to aim at");
    for tick in 0..3 {
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            input.push_cursor(at);
            if tick == 0 {
                input.push_mouse_button(MouseButton::Left, true);
            }
            if tick == 2 {
                input.push_mouse_button(MouseButton::Left, false);
            }
            input.sample(|screen| screen);
        }
        app.tick();
    }
    at
}

#[test]
fn nobody_falls_until_they_have_had_a_moment() {
    // The beat is the joke. If gravity gets to them first there is no joke, so this is the one
    // test in here that is really about comedy rather than about book-keeping.
    let mut app = sky(17, 60 * 8, |_, _| {});
    pop_one(&mut app);

    let hanging: Vec<(Entity, Vec2)> = {
        let world = app.world_mut();
        world
            .query::<(Entity, &Animal, &Transform2D)>()
            .iter(world)
            .filter(|(_, animal, _)| animal.mood == Mood::Beat)
            .map(|(entity, _, transform)| (entity, transform.translation))
            .collect()
    };
    assert!(!hanging.is_empty(), "somebody should have noticed");

    // A fifth of a second later, still up there, and still looking at you.
    drift(&mut app, 12);
    let world = app.world_mut();
    for (entity, was) in &hanging {
        let (animal, transform) = world
            .query::<(&Animal, &Transform2D)>()
            .get(world, *entity)
            .expect("still in the air");
        assert_eq!(animal.mood, Mood::Beat, "the beat should still be running");
        assert!(
            (transform.translation - *was).length() < 1.0,
            "nobody moves during the beat"
        );
    }
}

#[test]
fn everybody_lands_and_walks_it_off() {
    let mut app = sky(19, 60 * 8, |_, _| {});
    pop_one(&mut app);
    let fallers = {
        let world = app.world_mut();
        world
            .query::<&Animal>()
            .iter(world)
            .filter(|animal| animal.mood == Mood::Beat)
            .count()
    };
    assert!(fallers > 0);

    // Long enough for the fall, the bouncing, the sitting down and the walk off the screen.
    drift(&mut app, 60 * 24);
    let tally = *app.world_mut().resource::<Tally>();
    assert!(
        tally.landed >= fallers as u32,
        "{} fell and {} landed",
        fallers,
        tally.landed
    );

    // Nobody has fallen through the ground. Riders are excused: a balloon on its way up is
    // below the field for its first few seconds, which is the point of the field being drawn
    // in front of it.
    let world = app.world_mut();
    for (animal, transform) in world.query::<(&Animal, &Transform2D)>().iter(world) {
        if animal.mood == Mood::Riding {
            continue;
        }
        assert!(
            transform.translation.y >= GROUND - 1.0,
            "somebody {:?} is under the ground at {:?}",
            animal.mood,
            transform.translation
        );
        assert_ne!(animal.mood, Mood::Beat, "nobody is still hanging about");
    }
}

#[test]
fn a_falling_animal_takes_out_what_it_hits() {
    // The chain reaction, which is where the piece stops being a toy and starts being a joke
    // that writes itself. Popping the highest balloon in a busy sky is the likeliest way to
    // catch somebody underneath.
    let mut app = sky(23, 60 * 50, |_, _| {});
    let highest = {
        let world = app.world_mut();
        world
            .query::<(&Balloon, &Transform2D)>()
            .iter(world)
            .map(|(_, transform)| transform.translation)
            .fold(None::<Vec2>, |best, at| match best {
                Some(best) if best.y >= at.y => Some(best),
                _ => Some(at),
            })
            .expect("a sky with something in it")
    };
    for tick in 0..3 {
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            input.push_cursor(highest);
            if tick == 0 {
                input.push_mouse_button(MouseButton::Left, true);
            }
            if tick == 2 {
                input.push_mouse_button(MouseButton::Left, false);
            }
            input.sample(|screen| screen);
        }
        app.tick();
    }
    drift(&mut app, 60 * 6);
    let tally = *app.world_mut().resource::<Tally>();
    assert_eq!(tally.popped, 1, "you only clicked once");
    // Not guaranteed on any one run, but over fifty seconds of a filling sky it is close to it.
    assert!(
        tally.chained > 0,
        "somebody falling through a full sky should have taken a balloon with them"
    );
}

#[test]
fn everybody_who_goes_up_is_accounted_for() {
    // Popping constantly for a minute: whatever happens, the counts have to add up and the
    // world must not fill with things nobody is looking at any more.
    let mut app = sky(29, 60 * 60, |input, tick| {
        if tick % 30 == 0 {
            input.push_cursor(vec2(
                (tick as f32 * 0.37).sin() * ARENA.x * 0.4,
                (tick as f32 * 0.21).cos() * ARENA.y * 0.3,
            ));
            input.push_mouse_button(MouseButton::Left, true);
        }
        if tick % 30 == 2 {
            input.push_mouse_button(MouseButton::Left, false);
        }
    });
    let tally = *app.world_mut().resource::<Tally>();
    let census = *app.world_mut().resource::<Census>();
    let (balloons, baskets, animals) = population(&mut app);

    assert!(tally.popped > 0, "an entire minute of clicking hit nothing");
    assert_eq!(census.balloons as usize, balloons);
    assert_eq!(census.animals as usize, animals);
    assert!(baskets < 60, "{baskets} baskets is a leak");
    let world = app.world_mut();
    assert!(
        world.query::<&Scrap>().iter(world).count() < 30,
        "scraps are piling up"
    );
}

#[test]
fn stillness_stops_everything() {
    let mut app = sky(31, 60 * 20, |input, tick| {
        if tick == 60 * 20 - 10 {
            input.push_key(Key::Space, true);
        }
        if tick == 60 * 20 - 9 {
            input.push_key(Key::Space, false);
        }
    });
    let before = {
        let world = app.world_mut();
        world
            .query_filtered::<&Transform2D, With<Balloon>>()
            .iter(world)
            .map(|transform| transform.translation)
            .collect::<Vec<_>>()
    };
    assert!(!before.is_empty());
    drift(&mut app, 60);
    let after = {
        let world = app.world_mut();
        world
            .query_filtered::<&Transform2D, With<Balloon>>()
            .iter(world)
            .map(|transform| transform.translation)
            .collect::<Vec<_>>()
    };
    assert_eq!(before, after, "a still sky should not move");
}

#[test]
fn a_parachute_is_a_gentle_way_down() {
    // Nobody who opens one hits the ground hard, and everybody who opens one walks away.
    let mut app = sky(37, 60 * 40, |_, _| {});
    for _ in 0..12 {
        if first_balloon(&mut app).is_some() {
            pop_one(&mut app);
        }
        drift(&mut app, 40);
    }
    drift(&mut app, 60 * 20);
    let tally = *app.world_mut().resource::<Tally>();
    assert!(tally.popped >= 8, "not enough popping to see one");
    assert!(
        tally.chuted > 0,
        "in {} fallers nobody remembered a parachute",
        tally.landed
    );
}
