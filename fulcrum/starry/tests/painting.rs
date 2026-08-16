//! The rules the painting is built on: a current that cannot pile paint up, a picture that has
//! all of its parts, and paint that finds its way home after it has been pushed around.

use fulcrum::prelude::*;
use starry::game::{
    self, CANVAS, Census, GamePlugin, Healing, Layer, MAX_STARS, MAX_STROKES, Paused, Sky, Stroke,
};

/// A canvas driven for `ticks`, with an optional bit of input each tick.
fn painting(seed: u64, ticks: u32, mut script: impl FnMut(&mut Input, u32)) -> Fulcrum {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed,
        window_size: (CANVAS.x as u32, CANVAS.y as u32),
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

/// Drive `ticks` more with no input at all.
fn settle(app: &mut Fulcrum, ticks: u32) {
    for _ in 0..ticks {
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            input.sample(|screen| screen);
        }
        app.tick();
    }
}

/// How far the paint sits from the picture, on average: zero when every stroke has found the
/// colour of the point it is standing on.
fn wrongness(app: &mut Fulcrum) -> f32 {
    let sky = app.world_mut().resource::<Sky>().clone();
    let time = app.world_mut().resource::<game::Elapsed>().0;
    let world = app.world_mut();
    let mut total = 0.0;
    let mut count = 0;
    let mut query = world.query::<(&Stroke, &Transform2D)>();
    for (stroke, transform) in query.iter(world) {
        let (layer, tone) = game::paint_at(transform.translation, &sky, time);
        let wanted = (tone + stroke.weave).clamp(0.0, 1.0);
        total += (stroke.tone - wanted).abs() + if layer == stroke.layer { 0.0 } else { 0.35 };
        count += 1;
    }
    total / count.max(1) as f32
}

#[test]
fn the_current_never_piles_paint_up() {
    // The whole reason velocity is the curl of a field: divergence-free means no sources and no
    // sinks, so the canvas cannot thin out in one place and thicken in another. Worth checking
    // numerically, since the canvas is wider than it is tall and the scaling has to know it.
    let sky = Sky::default();
    let step = 1.0;
    let mut worst: f32 = 0.0;
    for row in -4..=4 {
        for column in -5..=5 {
            let at = vec2(
                column as f32 * CANVAS.x * 0.09,
                row as f32 * CANVAS.y * 0.09,
            );
            let right = game::flow(at + vec2(step, 0.0), &sky, 3.0).x;
            let left = game::flow(at - vec2(step, 0.0), &sky, 3.0).x;
            let up = game::flow(at + vec2(0.0, step), &sky, 3.0).y;
            let down = game::flow(at - vec2(0.0, step), &sky, 3.0).y;
            let divergence = (right - left) / (2.0 * step) + (up - down) / (2.0 * step);
            worst = worst.max(divergence.abs());
        }
    }
    assert!(worst < 0.01, "divergence reached {worst} per second");
}

#[test]
fn the_sky_moves_and_the_ground_does_not() {
    let sky = Sky::default();
    let up_there = game::flow(vec2(-140.0, 160.0), &sky, 0.0).length();
    let down_there = game::flow(vec2(-140.0, -CANVAS.y * 0.42), &sky, 0.0).length();
    assert!(up_there > 8.0, "the sky should be turning, not {up_there}");
    assert!(
        down_there < 0.5,
        "the ground should be still, not {down_there}"
    );
}

#[test]
fn the_picture_has_all_of_its_parts() {
    let sky = Sky::default();
    let at = |across, up| game::canvas_point(across, up);
    let layer_at = |across, up| game::paint_at(at(across, up), &sky, 0.0).0;

    assert_eq!(layer_at(-0.395, -0.20), Layer::Cypress, "the cypress");
    assert_eq!(layer_at(0.0, 0.42), Layer::Sky, "open sky");
    assert_eq!(layer_at(-0.30, -0.30), Layer::Hill, "the hills");
    assert_eq!(layer_at(0.30, -0.46), Layer::Ground, "the fields");

    // The first star, at its middle and then out in its rings.
    let star = sky.stars[0];
    assert_eq!(
        game::paint_at(star.at, &sky, 0.0).0,
        Layer::Star,
        "the middle of a star"
    );
    let ring = (1..40)
        .map(|step| star.at + vec2(star.radius * step as f32 / 40.0, 0.0))
        .any(|point| game::paint_at(point, &sky, 0.0).0 == Layer::Halo);
    assert!(ring, "a star should have rings around it");

    // The village, somewhere along its width, with a roof and a lit window in it.
    let mut village = false;
    let mut window = false;
    for step in 0..400 {
        let across = -0.315 + 0.77 * step as f32 / 400.0;
        for rung in 0..40 {
            let up = -0.34 + 0.10 * rung as f32 / 40.0;
            match layer_at(across, up) {
                Layer::Village => village = true,
                Layer::Window => window = true,
                _ => {}
            }
        }
    }
    assert!(village, "there should be a village");
    assert!(window, "somebody should be up");

    // The moon, and the night bitten out of it.
    let moon = at(game::MOON.0, game::MOON.1);
    let radius = game::MOON.2 * CANVAS.y;
    let crescent = (0..48).any(|step| {
        let angle = std::f32::consts::TAU * step as f32 / 48.0;
        let point = moon + vec2(angle.cos(), angle.sin()) * radius * 0.8;
        game::paint_at(point, &sky, 0.0).0 == Layer::Moon
    });
    assert!(crescent, "the moon should show a crescent");
    assert_ne!(
        game::paint_at(moon + vec2(radius * 0.3, radius * 0.3), &sky, 0.0).0,
        Layer::Moon,
        "the bitten-out part of the moon should be night"
    );
}

#[test]
fn the_sky_is_mostly_dark() {
    // The painting reads as light because very little of it is light. If the average sky tone
    // creeps up, the whole thing turns to milk, which is exactly what it did before the bands
    // were sharpened.
    let sky = Sky::default();
    let mut total = 0.0;
    let mut count = 0;
    for row in 0..40 {
        for column in 0..50 {
            let point =
                game::canvas_point(-0.5 + column as f32 / 50.0, 0.0 + 0.5 * row as f32 / 40.0);
            let (layer, tone) = game::paint_at(point, &sky, 0.0);
            if layer == Layer::Sky {
                total += tone;
                count += 1;
            }
        }
    }
    let average = total / count.max(1) as f32;
    assert!(count > 500, "that is not much sky to judge by");
    assert!(
        (0.08..0.42).contains(&average),
        "the average sky tone is {average}"
    );
}

#[test]
fn a_new_star_changes_the_picture_under_it() {
    let mut sky = Sky::default();
    let empty = game::canvas_point(-0.14, -0.06);
    assert_eq!(game::paint_at(empty, &sky, 0.0).0, Layer::Sky);
    sky.stars.push(game::Starlight {
        at: empty,
        radius: 0.05 * CANVAS.y,
        spin: 1.0,
    });
    assert_eq!(
        game::paint_at(empty, &sky, 0.0).0,
        Layer::Star,
        "the picture should have a star in it now"
    );
}

#[test]
fn clicking_hangs_a_star_and_x_takes_it_down() {
    let start = Sky::default().stars.len();
    let mut app = painting(5, 30, |input, tick| {
        if tick == 10 {
            input.push_cursor(vec2(-100.0, 240.0));
            input.push_mouse_button(MouseButton::Left, true);
        }
        if tick == 12 {
            input.push_mouse_button(MouseButton::Left, false);
        }
        if tick == 20 {
            input.push_key(Key::X, true);
        }
        if tick == 21 {
            input.push_key(Key::X, false);
        }
    });
    assert_eq!(
        app.world_mut().resource::<Sky>().stars.len(),
        start,
        "one hung and one taken down should leave the sky as it was"
    );
}

#[test]
fn a_star_cannot_be_hung_on_the_village() {
    let start = Sky::default().stars.len();
    let mut app = painting(5, 30, |input, tick| {
        if tick == 10 {
            input.push_cursor(game::canvas_point(0.1, -0.30));
            input.push_mouse_button(MouseButton::Left, true);
        }
        if tick == 12 {
            input.push_mouse_button(MouseButton::Left, false);
        }
    });
    assert_eq!(app.world_mut().resource::<Sky>().stars.len(), start);
}

#[test]
fn the_sky_only_holds_so_many_stars() {
    let mut app = painting(5, 400, |input, tick| {
        // A click every fifth tick, all over the sky.
        if tick % 5 == 0 {
            let across = -0.4 + 0.8 * ((tick % 50) as f32 / 50.0);
            input.push_cursor(game::canvas_point(across, 0.2));
            input.push_mouse_button(MouseButton::Left, true);
        }
        if tick % 5 == 2 {
            input.push_mouse_button(MouseButton::Left, false);
        }
    });
    assert_eq!(app.world_mut().resource::<Sky>().stars.len(), MAX_STARS);
}

/// Paint over the whole canvas in one flat tone, as wrong as it can be made.
fn ruin(app: &mut Fulcrum) {
    let world = app.world_mut();
    let mut query = world.query::<&mut Stroke>();
    for mut stroke in query.iter_mut(world) {
        stroke.tone = 0.0;
    }
}

#[test]
fn the_paint_finds_its_way_home() {
    // The rule the whole piece rests on. Let it settle, ruin it, and leave it alone: the paint
    // has to find the picture again on its own.
    let mut app = painting(21, 400, |_, _| {});
    let settled = wrongness(&mut app);
    ruin(&mut app);
    let ruined = wrongness(&mut app);
    assert!(
        ruined > settled * 2.0,
        "the ruin should show: {settled} then {ruined}"
    );

    settle(&mut app, 420);
    let healed = wrongness(&mut app);
    assert!(
        healed < settled * 1.3,
        "the paint should have found its way home: {settled}, {ruined}, then {healed}"
    );
}

#[test]
fn healing_can_be_turned_off() {
    let mut app = painting(23, 400, |input, tick| {
        if tick == 300 {
            input.push_key(Key::H, true);
        }
        if tick == 301 {
            input.push_key(Key::H, false);
        }
    });
    assert!(!app.world_mut().resource::<Healing>().0, "H turns it off");
    let settled = wrongness(&mut app);
    ruin(&mut app);
    let ruined = wrongness(&mut app);
    settle(&mut app, 240);
    let after = wrongness(&mut app);
    // Strokes still reach the end of their lives and are laid down again as fresh paint, so
    // the canvas recovers a little whatever happens. What must not happen is healing.
    assert!(
        after > ruined * 0.6,
        "with healing off the ruin should stay: {settled}, {ruined}, then {after}"
    );
}

#[test]
fn a_drag_pushes_the_paint_along() {
    // The pointer is a palette knife: paint near it goes with it, and paint away from it
    // carries on as if nothing had happened.
    let mut app = painting(29, 400, |_, _| {});
    let path = |tick: u32| vec2(-380.0 + 22.0 * tick as f32, 120.0);
    let watched: Vec<(Entity, Vec2, bool)> = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<(Entity, &Transform2D), With<Stroke>>();
        query
            .iter(world)
            .map(|(entity, transform)| {
                let near = (transform.translation.y - 120.0).abs() < 60.0
                    && transform.translation.x.abs() < 300.0;
                (entity, transform.translation, near)
            })
            .collect()
    };

    for tick in 0..40 {
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            input.push_cursor(path(tick));
            input.sample(|screen| screen);
        }
        app.tick();
    }

    let mut pushed = (0.0, 0);
    let mut untouched = (0.0, 0);
    let world = app.world_mut();
    for (entity, was, near) in watched {
        let Ok(transform) = world.query::<&Transform2D>().get(world, entity) else {
            continue;
        };
        let moved = (transform.translation - was).length();
        if near {
            pushed.0 += moved;
            pushed.1 += 1;
        } else if was.y < -200.0 {
            untouched.0 += moved;
            untouched.1 += 1;
        }
    }
    let pushed = pushed.0 / pushed.1.max(1) as f32;
    let untouched = untouched.0 / untouched.1.max(1) as f32;
    assert!(
        pushed > untouched * 4.0 && pushed > 30.0,
        "paint under the pointer moved {pushed}, paint away from it {untouched}"
    );
}

#[test]
fn the_paint_stays_on_the_canvas() {
    let mut app = painting(31, 1200, |_, _| {});
    let world = app.world_mut();
    let mut query = world.query_filtered::<&Transform2D, With<Stroke>>();
    for transform in query.iter(world) {
        let at = transform.translation;
        assert!(
            at.x.abs() < CANVAS.x / 2.0 + 60.0 && at.y.abs() < CANVAS.y / 2.0 + 60.0,
            "a stroke wandered off to {at:?}"
        );
    }
}

#[test]
fn the_village_holds_still_while_the_sky_turns() {
    let mut app = painting(37, 900, |_, _| {});
    let world = app.world_mut();
    let mut query = world.query::<(&Stroke, &Transform2D)>();
    let mut held = 0;
    let mut travelled = 0;
    for (stroke, transform) in query.iter(world) {
        let drift = (transform.translation - stroke.anchor).length();
        if stroke.layer.airborne() {
            if drift > 40.0 {
                travelled += 1;
            }
        } else {
            assert!(
                drift < 60.0,
                "a {:?} stroke has wandered {drift} from where it was laid",
                stroke.layer
            );
            held += 1;
        }
    }
    assert!(held > 100, "there should be paint on the ground");
    assert!(travelled > 100, "the sky should have moved");
}

#[test]
fn stillness_stops_everything() {
    let mut app = painting(41, 600, |input, tick| {
        if tick == 590 {
            input.push_key(Key::Space, true);
        }
        if tick == 591 {
            input.push_key(Key::Space, false);
        }
    });
    assert!(app.world_mut().resource::<Paused>().0);
    let before = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<&Transform2D, With<Stroke>>();
        query
            .iter(world)
            .map(|transform| transform.translation)
            .collect::<Vec<_>>()
    };
    settle(&mut app, 60);
    let after = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<&Transform2D, With<Stroke>>();
        query
            .iter(world)
            .map(|transform| transform.translation)
            .collect::<Vec<_>>()
    };
    assert_eq!(before, after, "a still painting should not move");
}

#[test]
fn more_paint_and_less() {
    let mut app = painting(43, 60, |input, tick| {
        if (10..40).contains(&tick) {
            input.push_key(Key::N, true);
        }
        if tick == 40 {
            input.push_key(Key::N, false);
        }
    });
    let fuller = app.world_mut().resource::<Census>().0;
    assert!(
        fuller > game::START_STROKES,
        "N should add paint, not {fuller}"
    );
    assert!(fuller <= MAX_STROKES);

    let mut app = painting(43, 60, |input, tick| {
        if (10..40).contains(&tick) {
            input.push_key(Key::M, true);
        }
        if tick == 40 {
            input.push_key(Key::M, false);
        }
    });
    let thinner = app.world_mut().resource::<Census>().0;
    assert!(
        thinner < game::START_STROKES,
        "M should take paint away, not {thinner}"
    );

    // And the count is the truth about what is on the canvas.
    let world = app.world_mut();
    let mut query = world.query_filtered::<Entity, With<Stroke>>();
    assert_eq!(thinner, query.iter(world).count() as u32);
}

#[test]
fn repainting_lays_the_whole_canvas_down_again() {
    let mut app = painting(47, 800, |input, tick| {
        if tick == 790 {
            input.push_key(Key::R, true);
        }
        if tick == 791 {
            input.push_key(Key::R, false);
        }
    });
    let world = app.world_mut();
    let mut query = world.query::<&Stroke>();
    let oldest = query
        .iter(world)
        .map(|stroke| stroke.age)
        .fold(0.0_f32, f32::max);
    assert!(oldest < 1.0, "the oldest stroke is {oldest} seconds old");
}
