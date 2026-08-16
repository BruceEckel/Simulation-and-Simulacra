//! Absolute zero, headless. A 0 K surface is a real setting rather than a guarded one, and
//! these are the properties that make it safe to offer: it absorbs without emitting, a box
//! between two of them freezes out, and warming the surfaces afterwards brings the gas back.

use fulcrum::prelude::*;
use heatflow::game::{
    ATOM_RADIUS, Atom, DEFAULT_COURT, GamePlugin, Meter, PROFILE_BINS, TEMPERATURE_MIN, Velocity,
    Walls, ZERO_SNAP, cooler, emitted_velocity, warmer,
};

/// A headless box with the gas installed and startup run.
fn box_of_gas(left: f32, right: f32) -> Fulcrum {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed: 3,
        window_size: (1024, 768),
        ..Default::default()
    })
    .with_plugin(SpatialPlugin {
        cell_size: ATOM_RADIUS * 8.0,
    })
    .with_plugin(GamePlugin);
    app.world_mut().insert_resource(Walls { left, right });
    app.run_startup();
    app
}

fn run(app: &mut Fulcrum, ticks: u32) {
    for _ in 0..ticks {
        app.tick();
    }
}

/// Mean atom speed, and how many atoms are pinned motionless against a surface.
fn survey(app: &mut Fulcrum) -> (f32, usize) {
    let edge = DEFAULT_COURT.x / 2.0 - ATOM_RADIUS - 0.5;
    let world = app.world_mut();
    let atoms: Vec<(Vec2, Vec2)> = world
        .query_filtered::<(&Transform2D, &Velocity), With<Atom>>()
        .iter(world)
        .map(|(transform, velocity)| (transform.translation, velocity.0))
        .collect();
    let mean = atoms.iter().map(|(_, v)| v.length()).sum::<f32>() / atoms.len().max(1) as f32;
    let pinned = atoms
        .iter()
        .filter(|(p, v)| v.length() < 1.0 && p.x.abs() > edge)
        .count();
    (mean, pinned)
}

#[test]
fn a_zero_kelvin_surface_emits_nothing() {
    let mut rng = SimRng::seeded(1);
    for _ in 0..100 {
        assert_eq!(
            emitted_velocity(&mut rng, 0.0, 1.0),
            Vec2::ZERO,
            "an atom leaving absolute zero carries no energy at all"
        );
    }
}

#[test]
fn absolute_zero_is_reachable_and_escapable() {
    // The ramp is proportional, which cannot reach zero by dividing or leave it by
    // multiplying. Both ends need the snap to work.
    let mut temperature = 400.0;
    for _ in 0..600 {
        temperature = cooler(temperature);
    }
    assert_eq!(temperature, 0.0, "holding the cool key should reach 0 K");
    assert_eq!(
        warmer(0.0),
        ZERO_SNAP,
        "and heating should climb back off it"
    );
    assert!(
        warmer(ZERO_SNAP) > ZERO_SNAP,
        "and keep climbing from there"
    );
    assert_eq!(TEMPERATURE_MIN, 0.0, "0 K is the documented bottom");
}

#[test]
fn a_zero_kelvin_surface_makes_the_sharpest_gradient() {
    // The most extreme setting the box allows, and the one worth having the floor removed for.
    let mut app = box_of_gas(0.0, 1200.0);
    run(&mut app, 6_000);
    let meter = app.world_mut().resource::<Meter>().clone();
    assert!(
        meter.profile[0] < 60.0,
        "the gas against a 0 K surface should be nearly still: {:.0} K",
        meter.profile[0]
    );
    assert!(
        meter.profile[PROFILE_BINS - 1] > 700.0,
        "while the far side stays hot: {:.0} K",
        meter.profile[PROFILE_BINS - 1]
    );
    assert!(
        meter.left_flux < 0.0 && meter.right_flux > 0.0,
        "with heat running from the hot surface into the cold one"
    );
}

#[test]
fn a_box_at_zero_freezes_out_and_comes_back() {
    // Both surfaces at absolute zero: the gas loses its energy to the walls and settles. This
    // is correct physics rather than a stall, so it is allowed — but it must be reversible.
    let mut app = box_of_gas(0.0, 0.0);
    run(&mut app, 600);
    let (early, _) = survey(&mut app);
    run(&mut app, 2_400);
    let (frozen, pinned) = survey(&mut app);
    assert!(
        frozen < early * 0.6,
        "the gas should be freezing out: {early:.1} to {frozen:.1} units/s"
    );
    assert!(
        pinned > 50,
        "with atoms coming to rest on the surfaces: {pinned}"
    );

    // Now warm both surfaces and the gas has to recover — nothing may stay stuck to a wall
    // that is trying to give it energy.
    app.world_mut().insert_resource(Walls {
        left: 600.0,
        right: 600.0,
    });
    run(&mut app, 1_500);
    let (revived, still_pinned) = survey(&mut app);
    assert!(
        revived > frozen * 5.0,
        "warming the surfaces should bring the gas back: {frozen:.1} to {revived:.1} units/s"
    );
    assert_eq!(
        still_pinned, 0,
        "and no atom should be left frozen to a warm surface"
    );
}
