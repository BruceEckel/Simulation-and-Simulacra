//! Behavior tests, headless: the flock stays in the arena, keeps to its speed band, and
//! actually flocks — neighbors end up flying together, which is the whole point and the thing
//! a broken steering term would quietly lose.

use boids::game::{
    Boid, DEFAULT_ARENA, FLOCK_SIZE, GamePlugin, MAX_SPEED, MIN_SPEED, NEIGHBOR_RADIUS, Rules,
    Velocity,
};
use fulcrum::prelude::*;

/// A headless app with the flock installed and startup run.
fn app(rules: Rules) -> Fulcrum {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed: 9,
        window_size: (1024, 768),
        ..Default::default()
    })
    .with_plugin(SpatialPlugin {
        cell_size: NEIGHBOR_RADIUS,
    })
    .with_plugin(GamePlugin);
    app.world_mut().insert_resource(rules);
    app.run_startup();
    app
}

/// Position and velocity of every boid, right now.
fn flock(app: &mut Fulcrum) -> Vec<(Vec2, Vec2)> {
    let world = app.world_mut();
    world
        .query_filtered::<(&Transform2D, &Velocity), With<Boid>>()
        .iter(world)
        .map(|(transform, velocity)| (transform.translation, velocity.0))
        .collect()
}

/// Mean heading agreement with neighbors: 1.0 = everyone in a neighborhood flies the same way,
/// 0.0 = no relationship. Brute force on purpose — the grid is what's under test.
fn alignment(flock: &[(Vec2, Vec2)]) -> f32 {
    let mut total = 0.0;
    let mut pairs = 0.0;
    for (position, velocity) in flock {
        for (other_position, other_velocity) in flock {
            if position.distance(*other_position) > NEIGHBOR_RADIUS || position == other_position {
                continue;
            }
            total += velocity.normalize().dot(other_velocity.normalize());
            pairs += 1.0;
        }
    }
    if pairs == 0.0 { 0.0 } else { total / pairs }
}

#[test]
fn the_flock_forms() {
    // No predator: it exists to break the flock up, which is the opposite of what's measured.
    let mut app = app(Rules {
        predator: false,
        ..Rules::default()
    });
    let before = alignment(&flock(&mut app));
    for _ in 0..900 {
        app.tick();
    }
    let after = alignment(&flock(&mut app));
    assert!(
        after > before + 0.3,
        "neighbors should end up flying together: {before:.2} -> {after:.2}"
    );
}

#[test]
fn rules_off_means_no_flocking() {
    // Same seed, same everything, but the three rules are switched off: boids should just
    // cruise, so neighbor headings stay uncorrelated. This is what proves the previous test
    // measures the rules and not, say, the walls herding everyone into the same lane.
    let mut app = app(Rules {
        separation: false,
        alignment: false,
        cohesion: false,
        predator: false,
    });
    for _ in 0..900 {
        app.tick();
    }
    assert!(
        alignment(&flock(&mut app)) < 0.3,
        "with every rule off there is nothing to align the flock"
    );
}

#[test]
fn boids_stay_in_bounds_and_in_their_speed_band() {
    let mut app = app(Rules::default());
    for _ in 0..900 {
        app.tick();
    }
    let flock = flock(&mut app);
    assert_eq!(flock.len(), FLOCK_SIZE);
    let limit = DEFAULT_ARENA / 2.0;
    for (position, velocity) in flock {
        assert!(
            position.x.abs() <= limit.x && position.y.abs() <= limit.y,
            "boid escaped the arena at {position}"
        );
        let speed = velocity.length();
        assert!(
            (MIN_SPEED - 0.01..=MAX_SPEED + 0.01).contains(&speed),
            "boid speed {speed} outside [{MIN_SPEED}, {MAX_SPEED}]"
        );
    }
}
