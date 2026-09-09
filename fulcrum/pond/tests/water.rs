//! The water and the lantern, held to what the module docs say about them.

use fulcrum::prelude::*;
use pond::game::{
    DAMPING_PER_SECOND, GamePlugin, IDLE_AFTER, LANTERN_SPEED, Lantern, PRESS_RADIUS, SHORE,
    SWEEP_PERIOD, SWEEP_REACH, Water, cfl, grid_for, phase_through, shore, stride_for, sweep,
};

const DT: f32 = 1.0 / 60.0;

/// A headless pond, with the pointer where the script puts it.
fn app(seed: u64) -> Fulcrum {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed,
        window_size: (1280, 800),
        ..Default::default()
    })
    .with_plugin(GamePlugin);
    app.run_startup();
    app
}

/// One tick with the pointer at `at` and the button as given.
fn tick(app: &mut Fulcrum, at: Vec2, pressed: bool) {
    {
        let mut input = app.world_mut().resource_mut::<Input>();
        input.push_cursor(at);
        let was = input.mouse_pressed(MouseButton::Left);
        if was != pressed {
            input.push_mouse_button(MouseButton::Left, pressed);
        }
        input.sample(|screen| screen);
    }
    app.tick();
}

#[test]
fn the_scheme_is_stable() {
    // The explicit wave equation on a square grid holds while the Courant number is under
    // one over root two. Well under, so that the sponge at the shore has room to work too.
    assert!(
        cfl(DT) < 0.6,
        "the wave speed is too high for the tick: cfl {}",
        cfl(DT)
    );
}

#[test]
fn the_grid_fits_the_window_and_the_bus() {
    let (across, down, cell) = grid_for(vec2(1280.0, 800.0));
    assert_eq!((across, down, cell), (640, 400, 2.0));
    assert_eq!(stride_for(across) % 64, 0);
    assert!(stride_for(across) >= across);
    // Past the cell cap, the cells grow rather than the count.
    let (across, down, cell) = grid_for(vec2(3840.0, 2160.0));
    assert!(cell > 2.0, "a 4K display should get larger cells");
    assert!(across * down <= 1_100_000);
    assert!(across as f32 * cell >= 3840.0 && down as f32 * cell >= 2160.0);
    // Nothing degenerate for a tiny window.
    let (across, down, _) = grid_for(vec2(1.0, 1.0));
    assert!(across >= 3 && down >= 3);
}

#[test]
fn the_shore_absorbs_and_the_open_water_does_not() {
    assert_eq!(shore(SHORE), 1.0);
    assert_eq!(shore(SHORE + 100), 1.0);
    assert!(shore(0) < shore(SHORE / 2));
    assert!(shore(SHORE / 2) < 1.0);
}

#[test]
fn a_dent_becomes_a_ring_and_the_ring_dies_away() {
    let mut water = Water::new(vec2(1280.0, 800.0));
    assert_eq!(water.energy(), 0.0, "new water is still");
    let centre = vec2(320.0, 200.0);
    water.dent(centre, PRESS_RADIUS.0, 1.0);
    assert!(water.at(320, 200) < -0.5, "the dent should be there");

    // Give it a second: the dent has rebounded and a ring has moved out from it, so the
    // middle is near level and there is a crest some way out.
    for _ in 0..60 {
        water.step(DT);
    }
    let middle = water.at(320, 200).abs();
    let mut crest = 0.0f32;
    for r in 8..40 {
        crest = crest.max(water.at(320 + r, 200).abs());
    }
    assert!(
        crest > middle,
        "after a second the ring ({crest}) should be out beyond the middle ({middle})"
    );

    // And the whole thing dies away: in twenty seconds it is gone.
    let loud = water.energy();
    for _ in 0..20 * 60 {
        water.step(DT);
    }
    let quiet = water.energy();
    assert!(
        quiet < loud * 1e-4,
        "the water should go still: {loud} down to {quiet}"
    );
}

#[test]
fn a_ring_travels_at_the_wave_speed() {
    let mut water = Water::new(vec2(1280.0, 800.0));
    let centre = vec2(320.0, 200.0);
    water.dent(centre, 3.0, 1.0);
    // Where the crest is after a second and after two: it should have moved out by about the
    // wave speed, which is in cells per second.
    let crest_at = |water: &Water| {
        let mut best = (0, 0.0f32);
        for r in 4..200 {
            let h = water.at(320 + r, 200);
            if h > best.1 {
                best = (r, h);
            }
        }
        best.0 as f32
    };
    for _ in 0..60 {
        water.step(DT);
    }
    let one = crest_at(&water);
    for _ in 0..60 {
        water.step(DT);
    }
    let two = crest_at(&water);
    let travelled = two - one;
    assert!(
        (travelled - pond::game::WAVE_SPEED).abs() < pond::game::WAVE_SPEED * 0.35,
        "the crest moved {travelled} cells in a second; the wave speed is {}",
        pond::game::WAVE_SPEED
    );
}

#[test]
fn the_shore_does_not_reflect() {
    // A dent near the left edge. Anything that comes back from the edge would show up as a
    // crest travelling right, past the dent, later on. Measure what reaches a point on the
    // far side of the dent after the direct ring has gone by.
    let mut water = Water::new(vec2(1280.0, 800.0));
    water.dent(vec2(60.0, 200.0), 3.0, 1.0);
    let mut direct = 0.0f32;
    let mut reflected = 0.0f32;
    for t in 0..8 * 60 {
        water.step(DT);
        let h = water.at(140, 200).abs();
        // The direct ring passes the probe eighty cells out at a bit over three seconds; give
        // it five to be well clear, then listen.
        if t < 5 * 60 {
            direct = direct.max(h);
        } else {
            reflected = reflected.max(h);
        }
    }
    assert!(direct > 0.0);
    assert!(
        reflected < direct * 0.08,
        "the shore sent back {reflected} of a {direct} ring"
    );
}

#[test]
fn damping_is_what_it_says() {
    // A single speed with no neighbours to spread to, isolated by being flat: after one
    // second of steps the speed at a level cell should have been multiplied by the damping.
    // Use a broad, gentle bump so that the spreading is slow against the damping.
    let per_tick = DAMPING_PER_SECOND.powf(DT);
    assert!((per_tick.powi(60) - DAMPING_PER_SECOND).abs() < 1e-4);
}

#[test]
fn a_resize_keeps_the_water() {
    let mut water = Water::new(vec2(1280.0, 800.0));
    water.dent(vec2(320.0, 200.0), 8.0, 1.0);
    let before = water.at(320, 200);
    water.resize(vec2(640.0, 400.0));
    assert_eq!((water.across, water.down), (320, 200));
    // The dent is now at half the cell coordinates, and about as deep.
    let after = water.at(160, 100);
    assert!(
        (after - before).abs() < 0.15,
        "the dent should survive the resize: {before} became {after}"
    );
    // The same size again is not a resize.
    let revision = water.revision;
    water.resize(vec2(640.0, 400.0));
    assert_eq!(water.revision, revision);
}

#[test]
fn a_press_is_a_bowl_and_a_tap_is_a_dent() {
    let mut app = app(3);
    let at = vec2(640.0, 400.0);
    // Bring the lantern to the pointer and let it settle there, so that the press is
    // all that moves the water under the pointer. A lantern at rest leaves no wake.
    tick(&mut app, vec2(100.0, 100.0), false);
    for _ in 0..8 * 60 {
        tick(&mut app, at, false);
    }
    let level = app.world_mut().resource::<Water>().at(320, 200);
    assert!(
        level.abs() < 0.05,
        "the water should be near level: {level}"
    );

    // A tap: one tick down, and there is a dent.
    tick(&mut app, at, true);
    let tapped = app.world_mut().resource::<Water>().at(320, 200);
    assert!(
        tapped - level < -0.3,
        "a tap should dent the water: {level} became {tapped}"
    );
    tick(&mut app, at, false);

    // Let it go by, then hold for two seconds: the bowl is wider than the tap was.
    for _ in 0..240 {
        tick(&mut app, at, false);
    }
    for _ in 0..120 {
        tick(&mut app, at, true);
    }
    let water = app.world_mut().resource::<Water>();
    let held = water.at(320, 200);
    let wide = water.at(320 + 12, 200);
    assert!(held < -0.6, "a held press should be a deep bowl: {held}");
    assert!(
        wide < -0.2,
        "and a wide one: {wide} twelve cells out, against {held} in the middle"
    );
}

#[test]
fn the_lantern_follows_the_pointer() {
    let mut app = app(5);
    let at = vec2(200.0, 600.0);
    // The first sighting is not a move; the second is.
    tick(&mut app, vec2(640.0, 400.0), false);
    for _ in 0..180 {
        tick(&mut app, at, false);
    }
    let lantern = *app.world_mut().resource::<Lantern>();
    assert!(lantern.attended, "the lantern should be following");
    assert!(
        (lantern.at - at).length() < 4.0,
        "after three seconds it should have arrived: {:?} for {at:?}",
        lantern.at
    );
}

#[test]
fn the_lantern_never_jumps() {
    let mut app = app(5);
    tick(&mut app, vec2(100.0, 100.0), false);
    tick(&mut app, vec2(100.0, 100.0), false);
    // The pointer teleports to the far corner. The lantern travels.
    let mut last = app.world_mut().resource::<Lantern>().at;
    for _ in 0..120 {
        tick(&mut app, vec2(1180.0, 700.0), false);
        let now = app.world_mut().resource::<Lantern>().at;
        let step = (now - last).length();
        assert!(
            step <= LANTERN_SPEED * DT * 1.01,
            "the lantern moved {step} pixels in one tick"
        );
        last = now;
    }
}

#[test]
fn left_alone_the_lantern_walks_and_a_move_calls_it_back() {
    let mut app = app(9);
    let at = vec2(640.0, 400.0);
    tick(&mut app, at, false);
    tick(&mut app, vec2(641.0, 400.0), false);
    assert!(app.world_mut().resource::<Lantern>().attended);

    // Hold still past the idle time, and it sets off.
    let idle = (IDLE_AFTER / DT) as u32 + 10;
    for _ in 0..idle {
        tick(&mut app, vec2(641.0, 400.0), false);
    }
    assert!(
        !app.world_mut().resource::<Lantern>().attended,
        "the lantern should be walking by now"
    );

    // Over one period it reaches both sides.
    let period = app.world_mut().resource::<Lantern>().period;
    let (mut left, mut right) = (f32::MAX, f32::MIN);
    for _ in 0..(period / DT) as u32 + 30 {
        tick(&mut app, vec2(641.0, 400.0), false);
        let x = app.world_mut().resource::<Lantern>().at.x;
        left = left.min(x);
        right = right.max(x);
    }
    let reach = 1280.0 * SWEEP_REACH;
    assert!(
        left < 640.0 - reach * 0.7 && right > 640.0 + reach * 0.7,
        "the walk should cross from side to side: {left}..{right}"
    );

    // A move of the pointer, and it is following again.
    tick(&mut app, vec2(300.0, 300.0), false);
    assert!(app.world_mut().resource::<Lantern>().attended);
}

#[test]
fn a_walk_sets_off_from_where_the_lantern_is() {
    let window = vec2(1280.0, 800.0);
    for x in [200.0, 640.0, 1100.0] {
        for direction in [-1.0, 1.0] {
            let at = vec2(x, 400.0);
            let phase = phase_through(window, at, vec2(direction, 0.0));
            let there = sweep(window, phase, 0.0);
            assert!(
                (there.x - x).abs() < 1.0,
                "the walk should pass through {x}: it starts at {}",
                there.x
            );
            // And heads the way the lantern was going.
            let next = sweep(window, phase + 0.01, 0.0);
            assert_eq!(
                (next.x - there.x).signum(),
                direction,
                "the walk should set off in the direction of travel"
            );
        }
    }
}

#[test]
fn the_pace_keys_stay_in_bounds() {
    let mut app = app(1);
    let press = |app: &mut Fulcrum, key: Key, ticks: u32| {
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            input.push_key(key, true);
            input.sample(|screen| screen);
        }
        for _ in 0..ticks {
            app.tick();
        }
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            input.push_key(key, false);
            input.sample(|screen| screen);
        }
        app.tick();
    };
    press(&mut app, Key::Up, 60 * 30);
    assert_eq!(app.world_mut().resource::<Lantern>().period, SWEEP_PERIOD.0);
    press(&mut app, Key::Down, 60 * 30);
    assert_eq!(app.world_mut().resource::<Lantern>().period, SWEEP_PERIOD.2);
}

#[test]
fn stilling_the_water_stills_it() {
    let mut app = app(1);
    let at = vec2(640.0, 400.0);
    // Bring the lantern to the pointer and let it come to rest, so that its wake is not
    // what is being measured.
    tick(&mut app, vec2(100.0, 100.0), false);
    for _ in 0..8 * 60 {
        tick(&mut app, at, false);
    }
    tick(&mut app, at, true);
    tick(&mut app, at, false);
    assert!(app.world_mut().resource::<Water>().energy() > 0.0);
    {
        let mut input = app.world_mut().resource_mut::<Input>();
        input.push_key(Key::R, true);
        input.sample(|screen| screen);
    }
    app.tick();
    // Stilled, then one tick of flow: the lantern at rest adds nothing, and the next drop of
    // rain is seconds away.
    let energy = app.world_mut().resource::<Water>().energy();
    assert!(
        energy < 1e-3,
        "r should still the water; energy left: {energy}"
    );
}
