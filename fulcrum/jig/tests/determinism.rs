//! Determinism gate: same input twice, bit-identical both times.
//!
//! Nothing in this piece is random, so the risk is not the RNG. It is that the body is a
//! chaotic system being integrated in `f32`: two runs that differ in the last bit of one angle
//! are visibly different dances ten seconds later, and that is a *property of the piece*, not a
//! fault. It also means there is nowhere for a sloppy dependence on wall-clock time to hide.
//! Accumulate the tempo off the frame time rather than the fixed tick, or let the substep count
//! follow the machine, and this test fails immediately and dramatically.
//!
//! Both halves are needed and they are not in tension. Identical input has to give an identical
//! dance, or there is no replay; input a hair different has to give a different one, or the
//! piece is a loop. `pendulums.rs` tests the second half.

use fulcrum::prelude::*;
use jig::game::{BONE_COUNT, Beat, GamePlugin, Routine, Skeleton, Tone};

/// Scripted input: wind the tempo up, take the tone out from under it, put both back, and walk
/// through the steps on the way.
fn script(input: &mut Input, tick: u32) {
    match tick {
        0..=90 => input.push_key(Key::Up, true), // faster
        91 => input.push_key(Key::Up, false),
        120 => input.push_key(Key::Digit3, true), // the figure of eight
        121 => input.push_key(Key::Digit3, false),
        160..=260 => input.push_key(Key::Left, true), // go limp
        261 => input.push_key(Key::Left, false),
        300 => input.push_key(Key::Digit5, true), // the shiver
        301 => input.push_key(Key::Digit5, false),
        340..=430 => input.push_key(Key::Right, true), // and stiffen up again
        431 => input.push_key(Key::Right, false),
        470 => input.push_key(Key::R, true), // stand up straight
        471 => input.push_key(Key::R, false),
        500 => input.push_key(Key::Digit1, true), // the sway
        501 => input.push_key(Key::Digit1, false),
        540..=620 => input.push_key(Key::Down, true), // and stop the band
        621 => input.push_key(Key::Down, false),
        _ => {}
    }
}

/// Fold every scrap of simulation state into one number.
fn digest(app: &mut Fulcrum) -> u64 {
    let beat = *app.world_mut().resource::<Beat>();
    let tone = *app.world_mut().resource::<Tone>();
    let routine = *app.world_mut().resource::<Routine>();
    let skeleton = app.world_mut().resource::<Skeleton>().clone();

    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut eat = |value: u32| {
        hash ^= value as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    };
    eat(beat.tempo.to_bits());
    eat(beat.phase.to_bits());
    eat(beat.count as u32);
    eat(tone.0.to_bits());
    eat(routine.0 as u32);
    eat(skeleton.hips.x.to_bits());
    eat(skeleton.hips.y.to_bits());
    eat(skeleton.knocks.len() as u32);
    for index in 0..BONE_COUNT {
        let joint = skeleton.joints[index];
        eat(joint.angle.to_bits());
        eat(joint.rate.to_bits());
        eat(u32::from(joint.stopped));
        let place = skeleton.places[index];
        eat(place.pivot.x.to_bits());
        eat(place.pivot.y.to_bits());
        eat(place.tip.x.to_bits());
        eat(place.tip.y.to_bits());
    }
    hash
}

/// Run the skeleton for `ticks`, taking a digest every so often so that a drift is caught where
/// it happens rather than only if it survives to the end.
fn run(seed: u64, ticks: u32) -> Vec<u64> {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed,
        ..Default::default()
    })
    .with_plugin(GamePlugin);

    app.run_startup();
    let mut marks = Vec::new();
    for tick in 0..ticks {
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            script(&mut input, tick);
            input.sample(|screen| screen);
        }
        app.tick();
        if tick % 40 == 0 {
            marks.push(digest(&mut app));
        }
    }
    marks.push(digest(&mut app));
    marks
}

#[test]
fn determinism_same_input_same_outcome() {
    let first = run(42, 700);
    let second = run(42, 700);
    assert_eq!(first, second, "same input must be bit-identical");
}

#[test]
fn the_seed_has_nothing_to_do_with_it() {
    // Not one thing in this piece is random. The dance looks improvised because a system of
    // coupled pendulums has no short way of saying what it will do next, not because anything
    // rolled a die. If this ever fails, some state has crept in that should not be there.
    assert_eq!(run(1, 400), run(2, 400), "no part of this piece is random");
}
