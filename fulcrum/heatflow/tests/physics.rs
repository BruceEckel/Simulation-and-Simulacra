//! The physics, headless. These are the tests that would catch the simulation quietly
//! becoming decorative: a gas that doesn't reach its walls' temperature, a gradient that never
//! forms, or collisions that leak energy.

use fulcrum::prelude::*;
use heatflow::game::{
    ATOM_RADIUS, Atom, BOLTZMANN, Census, DEFAULT_COURT, GamePlugin, Meter, PROFILE_BINS,
    START_ATOMS, Speed, TEMPERATURE_MAX, TEMPERATURE_MIN, Velocity, Walls, collide, kinetic_energy,
    temperature_of,
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

/// The heat current: the average of what the two surfaces are exchanging.
fn current_of(meter: &Meter) -> f32 {
    (meter.left_flux.abs() + meter.right_flux.abs()) / 2.0
}

/// Mean measured temperature over a slice of the profile columns.
fn band(meter: &Meter, columns: std::ops::Range<usize>) -> f32 {
    let width = columns.len().max(1) as f32;
    meter.profile[columns].iter().sum::<f32>() / width
}

#[test]
fn a_collision_conserves_momentum_and_energy() {
    // Head-on, glancing, and a pair that is touching but already separating.
    let head_on = collide(
        (vec2(0.0, 0.0), vec2(ATOM_RADIUS * 1.5, 0.0)),
        (vec2(120.0, 0.0), vec2(-80.0, 0.0)),
    );
    let (a, b) = head_on.expect("approaching atoms in contact should collide");
    assert_eq!(a + b, vec2(40.0, 0.0), "momentum is conserved");
    assert!(
        (kinetic_energy(a) + kinetic_energy(b)
            - (kinetic_energy(vec2(120.0, 0.0)) + kinetic_energy(vec2(-80.0, 0.0))))
        .abs()
            < 1e-3,
        "energy is conserved"
    );
    assert_eq!(
        a,
        vec2(-80.0, 0.0),
        "equal masses swap the normal components"
    );

    let glancing = collide(
        (vec2(0.0, 0.0), vec2(ATOM_RADIUS, ATOM_RADIUS)),
        (vec2(100.0, 40.0), vec2(-20.0, 15.0)),
    );
    let (a, b) = glancing.expect("a glancing contact still collides");
    let before = kinetic_energy(vec2(100.0, 40.0)) + kinetic_energy(vec2(-20.0, 15.0));
    assert!(
        (kinetic_energy(a) + kinetic_energy(b) - before).abs() < 1e-2,
        "a glancing collision conserves energy too"
    );
    assert!((a + b - vec2(80.0, 55.0)).length() < 1e-3, "and momentum");

    assert!(
        collide(
            (vec2(0.0, 0.0), vec2(ATOM_RADIUS, 0.0)),
            (vec2(-50.0, 0.0), vec2(50.0, 0.0)),
        )
        .is_none(),
        "atoms already moving apart must not be collided again"
    );
    assert!(
        collide(
            (vec2(0.0, 0.0), vec2(ATOM_RADIUS * 4.0, 0.0)),
            (vec2(50.0, 0.0), vec2(-50.0, 0.0)),
        )
        .is_none(),
        "atoms out of contact do not collide"
    );
}

#[test]
fn the_gas_reaches_the_temperature_of_its_walls() {
    // Both surfaces at the same temperature: whatever the gas started at, it should end up
    // there. This is the test that catches an incorrect emission distribution — using the
    // bulk Maxwellian at the wall instead of the flux-weighted one leaves the gas cold.
    let target = 600.0;
    let mut app = box_of_gas(target, target);
    run(&mut app, 5_000);
    let meter = app.world_mut().resource::<Meter>().clone();
    let settled = meter.mean_temperature();
    assert!(
        (settled - target).abs() / target < 0.12,
        "gas settled at {settled:.0} K between two {target:.0} K walls"
    );
    assert!(
        meter.collisions > 0,
        "the atoms should be colliding with each other"
    );
}

#[test]
fn atoms_at_equilibrium_still_have_wildly_different_energies() {
    // Temperature is a property of the ensemble, not of an atom. A gas in equilibrium has a
    // Maxwell-Boltzmann distribution of speeds, which in two dimensions means the individual
    // kinetic energies are exponentially distributed about the mean: most atoms below it, a
    // long tail well above it. So the atoms never converge on one energy, no matter how long
    // it runs or how equal the walls are.
    //
    // This is pinned as a test because it looks like a bug from the outside: colour the atoms
    // by their own energy and an equilibrium gas stays a permanent rainbow.
    let mut app = box_of_gas(600.0, 600.0);
    run(&mut app, 5_000);

    let world = app.world_mut();
    let mut energies: Vec<f32> = world
        .query_filtered::<&Velocity, With<Atom>>()
        .iter(world)
        .map(|velocity| kinetic_energy(velocity.0))
        .collect();
    energies.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = energies.iter().sum::<f32>() / energies.len() as f32;
    let at = |fraction: f32| energies[(energies.len() as f32 * fraction) as usize];

    // The exponential distribution puts its tenth percentile at 0.105 of the mean and its
    // ninetieth at 2.303. Wide margins here: this is a statement about the shape, not a fit.
    assert!(
        at(0.1) < 0.25 * mean,
        "the coldest tenth should be far below the mean: {:.0} against {mean:.0}",
        at(0.1)
    );
    assert!(
        at(0.9) > 1.9 * mean,
        "the hottest tenth should be far above it: {:.0} against {mean:.0}",
        at(0.9)
    );
    assert!(
        at(0.5) < 0.85 * mean,
        "and the median should sit below the mean, as an exponential's does: {:.0}",
        at(0.5)
    );
}

#[test]
fn unequal_walls_drive_a_gradient_and_a_current() {
    let (cold, hot) = (200.0, 1200.0);
    let mut app = box_of_gas(cold, hot);
    run(&mut app, 6_000);
    let meter = app.world_mut().resource::<Meter>().clone();

    let near_cold = band(&meter, 0..PROFILE_BINS / 6);
    let near_hot = band(&meter, PROFILE_BINS - PROFILE_BINS / 6..PROFILE_BINS);
    assert!(
        near_hot > near_cold + 200.0,
        "a gradient should stand between the surfaces: {near_cold:.0} K to {near_hot:.0} K"
    );
    assert!(
        near_cold > cold * 0.8 && near_hot < hot * 1.2,
        "the gas should sit between the two wall temperatures: {near_cold:.0}..{near_hot:.0}"
    );

    // And it should rise across the box, not just at the ends.
    let quarters: Vec<f32> = (0..4)
        .map(|q| band(&meter, q * PROFILE_BINS / 4..(q + 1) * PROFILE_BINS / 4))
        .collect();
    assert!(
        quarters.windows(2).all(|pair| pair[1] > pair[0]),
        "temperature should climb from the cold wall to the hot one: {quarters:?}"
    );

    // Heat in at the hot face, out at the cold one, in comparable amounts.
    assert!(
        meter.right_flux > 0.0,
        "the hot surface should be feeding energy in: {}",
        meter.right_flux
    );
    assert!(
        meter.left_flux < 0.0,
        "the cold surface should be taking it out: {}",
        meter.left_flux
    );
    let imbalance = (meter.right_flux + meter.left_flux).abs()
        / meter.right_flux.abs().max(meter.left_flux.abs());
    assert!(
        imbalance < 0.35,
        "at steady state the two surfaces should roughly balance: \
         in {:.0}, out {:.0}",
        meter.right_flux,
        meter.left_flux
    );
}

#[test]
fn heat_flows_the_other_way_when_the_walls_swap() {
    // The same run mirrored, so nothing about the gradient can be an artifact of which side
    // the profile is measured from.
    let mut app = box_of_gas(1200.0, 200.0);
    run(&mut app, 6_000);
    let meter = app.world_mut().resource::<Meter>().clone();
    assert!(
        band(&meter, 0..PROFILE_BINS / 6)
            > band(&meter, PROFILE_BINS - PROFILE_BINS / 6..PROFILE_BINS) + 200.0,
        "with the hot wall on the left the gradient should run the other way"
    );
    assert!(meter.left_flux > 0.0 && meter.right_flux < 0.0);
}

#[test]
fn atoms_stay_in_the_box() {
    let mut app = box_of_gas(TEMPERATURE_MIN, TEMPERATURE_MAX);
    run(&mut app, 3_000);
    let limit = DEFAULT_COURT / 2.0;
    let world = app.world_mut();
    for position in world
        .query_filtered::<&Transform2D, With<Atom>>()
        .iter(world)
        .map(|transform| transform.translation)
    {
        assert!(
            position.x.abs() <= limit.x && position.y.abs() <= limit.y,
            "atom escaped at {position}"
        );
    }
}

#[test]
fn an_atom_leaving_a_wall_carries_the_walls_temperature() {
    // Emission is flux-weighted, so atoms *leaving* a surface average 1.5 kT rather than the
    // bulk's kT. Measured here directly, because getting this wrong is invisible until the
    // whole gas sits at the wrong temperature.
    let mut rng = SimRng::seeded(11);
    let target = 500.0;
    let samples = 40_000;
    let mean: f32 = (0..samples)
        .map(|_| kinetic_energy(heatflow::game::emitted_velocity(&mut rng, target, 1.0)))
        .sum::<f32>()
        / samples as f32;
    let expected = 1.5 * BOLTZMANN * target;
    assert!(
        (mean - expected).abs() / expected < 0.05,
        "emitted atoms averaged {mean:.0} against the expected {expected:.0}"
    );

    // The bulk distribution, by contrast, averages kT — that is what the gas settles at.
    let bulk: f32 = (0..samples)
        .map(|_| kinetic_energy(heatflow::game::thermal_velocity(&mut rng, target)))
        .sum::<f32>()
        / samples as f32;
    assert!(
        (temperature_of(bulk) - target).abs() / target < 0.05,
        "an equilibrium sample should read back as its own temperature: {:.0}",
        temperature_of(bulk)
    );
}

#[test]
fn the_population_and_the_temperatures_are_adjustable() {
    let mut app = box_of_gas(400.0, 400.0);
    let start = app.world_mut().resource::<Census>().atoms;
    assert_eq!(start, START_ATOMS);

    hold(&mut app, Key::N, 60);
    let grown = app.world_mut().resource::<Census>().atoms;
    assert!(grown > start, "holding N should pour atoms in: {grown}");

    hold(&mut app, Key::M, 60);
    let shrunk = app.world_mut().resource::<Census>().atoms;
    assert!(shrunk < grown, "holding M should take them out: {shrunk}");
    let world = app.world_mut();
    let live = world.query_filtered::<(), With<Atom>>().iter(world).count();
    assert_eq!(live as u32, shrunk, "the census should match the gas");

    // Surfaces ramp while held and clamp at the ends.
    hold(&mut app, Key::Q, 60);
    assert!(
        app.world_mut().resource::<Walls>().left > 400.0,
        "q heats the left"
    );
    hold(&mut app, Key::A, 120);
    assert!(
        app.world_mut().resource::<Walls>().left < 400.0,
        "a cools it"
    );
    hold(&mut app, Key::E, 60);
    assert!(
        app.world_mut().resource::<Walls>().right > 400.0,
        "e heats the right"
    );
    hold(&mut app, Key::D, 120);
    assert!(
        app.world_mut().resource::<Walls>().right < 400.0,
        "d cools it"
    );

    hold(&mut app, Key::Q, 600);
    assert_eq!(app.world_mut().resource::<Walls>().left, TEMPERATURE_MAX);
    hold(&mut app, Key::A, 1_200);
    assert_eq!(app.world_mut().resource::<Walls>().left, TEMPERATURE_MIN);
}

#[test]
fn the_surfaces_can_be_made_exactly_equal() {
    // A proportional ramp can get the two surfaces close but never equal: scaling both by the
    // same factor preserves their ratio. S is the way to ask for exactly equal, and exactly
    // is what it has to be — a hair's difference is still a heat current.
    let mut app = box_of_gas(200.0, 1200.0);
    hold(&mut app, Key::S, 1);
    let walls = *app.world_mut().resource::<Walls>();
    assert_eq!(walls.left, walls.right, "s should equalize the surfaces");
    assert_eq!(walls.left, 700.0, "at the average of the two");

    // And with them equal, the gradient goes away: no current, no slope.
    run(&mut app, 8_000);
    let meter = app.world_mut().resource::<Meter>().clone();
    let near_left = band(&meter, 0..PROFILE_BINS / 6);
    let near_right = band(&meter, PROFILE_BINS - PROFILE_BINS / 6..PROFILE_BINS);
    assert!(
        (near_left - near_right).abs() < 90.0,
        "an equalized box should flatten out: {near_left:.0} K against {near_right:.0} K"
    );
    assert!(
        (meter.mean_temperature() - 700.0).abs() < 60.0,
        "and settle at the temperature both surfaces now hold: {:.0} K",
        meter.mean_temperature()
    );

    // The heat current should collapse — but not to zero, and not to any number worth
    // hard-coding: a wall's net flux is a small difference between two large gross flows, so
    // what is left at equilibrium is shot noise. Compare against the same box actually
    // driving heat instead.
    let driven = {
        let mut app = box_of_gas(200.0, 1200.0);
        run(&mut app, 5_000);
        current_of(app.world_mut().resource::<Meter>())
    };
    let equalized = current_of(&meter);
    assert!(
        equalized < driven * 0.4,
        "equalized current {equalized:.0} should be a fraction of the driven {driven:.0}"
    );
}

#[test]
fn the_simulation_can_be_paused_and_paced() {
    let mut app = box_of_gas(400.0, 400.0);
    run(&mut app, 300);
    let positions = |app: &mut Fulcrum| {
        let world = app.world_mut();
        world
            .query_filtered::<&Transform2D, With<Atom>>()
            .iter(world)
            .map(|transform| transform.translation)
            .collect::<Vec<_>>()
    };

    hold(&mut app, Key::Space, 1);
    let frozen = positions(&mut app);
    let elapsed = app.world_mut().resource::<Meter>().elapsed;
    run(&mut app, 120);
    assert_eq!(positions(&mut app), frozen, "a paused gas should not move");
    assert_eq!(
        app.world_mut().resource::<Meter>().elapsed,
        elapsed,
        "and its clock should not run"
    );

    hold(&mut app, Key::Space, 1);
    hold(&mut app, Key::Up, 90);
    assert!(
        app.world_mut().resource::<Speed>().0 > 2.0,
        "up speeds it up"
    );
    let before = app.world_mut().resource::<Meter>().elapsed;
    run(&mut app, 60);
    let advanced = app.world_mut().resource::<Meter>().elapsed - before;
    assert!(
        advanced > 2.0,
        "at speed, a second of ticks should advance the clock further: {advanced:.2}s"
    );
}

/// Total kinetic energy of the gas.
fn total_energy(app: &mut Fulcrum) -> f32 {
    let world = app.world_mut();
    world
        .query_filtered::<&Velocity, With<Atom>>()
        .iter(world)
        .map(|velocity| kinetic_energy(velocity.0))
        .sum()
}

#[test]
fn an_isothermal_box_holds_its_energy() {
    // Both surfaces at the same temperature and the gas already settled there: total energy
    // should hold steady. A collision pass that leaked or manufactured energy would show up
    // here as a drift, and nowhere else — the walls would happily absorb it.
    //
    // Measured on the gas rather than on the wall fluxes, because at equilibrium each wall's
    // net flux is itself near zero and comparing two noise terms proves nothing.
    let mut app = box_of_gas(600.0, 600.0);
    run(&mut app, 3_000);
    let settled = total_energy(&mut app);
    run(&mut app, 3_000);
    let later = total_energy(&mut app);
    assert!(
        (later - settled).abs() / settled < 0.15,
        "energy drifted from {settled:.0} to {later:.0} in an isothermal box"
    );

    // And it is the right amount of energy: N atoms at kT each.
    let atoms = app.world_mut().resource::<Census>().atoms as f32;
    assert!(
        (temperature_of(later / atoms) - 600.0).abs() / 600.0 < 0.12,
        "the gas should hold {atoms} atoms' worth of 600 K, not {:.0} K",
        temperature_of(later / atoms)
    );
}
