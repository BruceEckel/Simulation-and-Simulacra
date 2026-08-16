//! Holding the piece to its claims, which are all about pendulums.
//!
//! The dance is only worth anything if the bones really are pendulums, so the tests that matter
//! are the ones that check the physics against physics rather than against last week's output.
//! Three of them do the real work:
//!
//! - a bone left alone swings at the period the textbook gives for a uniform rod;
//! - a bone whose joint is shaken hard enough stands on its head, at the speed Kapitza says;
//! - and with the tone turned off, so that there is no spring and no friction anywhere, the
//!   whole body's energy never rises. That last one is the sharp instrument. A mass matrix with
//!   a sign wrong in it, or a missing centrifugal term, does not look wrong on screen — it
//!   looks *lively*. It shows up here as a skeleton quietly inventing energy.

use fulcrum::prelude::*;
use jig::game::{
    BONE_COUNT, BONES, Beat, Frame, GamePlugin, Part, Routine, SUBSTEPS, Skeleton, Tone, direction,
    kapitza_number, rest_bend, swing,
};
use std::f32::consts::{PI, TAU};

/// How long one substep of the real simulation is.
const SLICE: f32 = 1.0 / 60.0 / SUBSTEPS as f32;

/// Swing one bone on its own, with nothing but gravity and whatever its pivot is doing, and
/// keep the angle at every step. The same integrator the game uses, on the same equation.
fn lone_bone(length: f32, start: f32, steps: u32, mut shove: impl FnMut(f32) -> Vec2) -> Vec<f32> {
    let mut angle = start;
    let mut rate = 0.0f32;
    let mut track = Vec::with_capacity(steps as usize);
    for step in 0..steps {
        rate += swing(length, angle, shove(step as f32 * SLICE)) * SLICE;
        angle += rate * SLICE;
        track.push(angle);
    }
    track
}

#[test]
fn a_bone_left_alone_swings_at_a_pendulums_rate() {
    // A uniform rod pivoted at one end is a pendulum of length 2L/3, so it swings at
    // 2π·sqrt(2L/3g). Nothing here is tuned to make that come out; it comes out because the
    // equation is the equation.
    for length in [20.0f32, 54.0, 64.0, 86.0] {
        let expected = TAU * (2.0 * length / (3.0 * jig::game::GRAVITY)).sqrt();
        let track = lone_bone(length, 0.12, 12_000, |_| Vec2::ZERO);

        let crossings: Vec<f32> = track
            .windows(2)
            .enumerate()
            .filter(|(_, pair)| pair[0] < 0.0 && pair[1] >= 0.0)
            .map(|(step, _)| step as f32 * SLICE)
            .collect();
        assert!(
            crossings.len() >= 2,
            "a {length}-unit bone should have swung back and forth several times"
        );
        let measured = crossings[1] - crossings[0];
        assert!(
            (measured / expected - 1.0).abs() < 0.02,
            "a {length}-unit bone swung with a period of {measured:.4}s; \
             the pendulum period is {expected:.4}s"
        );
    }
}

#[test]
fn a_shaken_joint_stands_a_bone_on_its_head() {
    // Kapitza's pendulum. Bob a pivot up and down fast enough and upside down stops being the
    // way a pendulum falls and becomes somewhere it will sit. The threshold is (Aω)² = 2gℓ
    // with ℓ = 2L/3, and it is sharp enough to test from both sides of.
    //
    // The bob has to be small compared with the bone as well as fast — the whole effect is an
    // averaging argument, and it needs the wobble it averages over to be small. That is why the
    // SHIVER step in the game is a hand's width six times a beat rather than a big slow heave.
    let length = 64.0f32;
    let reach = 8.0f32;
    let upright = PI;

    for (rate, expect_upright) in [(120.0f32, true), (40.0f32, false)] {
        let number = kapitza_number(length, reach, rate);
        assert_eq!(
            number > 1.0,
            expect_upright,
            "the test is set up wrong: {rate} rad/s gives a Kapitza number of {number:.2}"
        );
        // A pivot at A·sin(ωt) accelerates at −Aω²·sin(ωt).
        let track = lone_bone(length, upright + 0.10, 24_000, |time| {
            vec2(0.0, -reach * rate * rate * (rate * time).sin())
        });
        let strayed = track
            .iter()
            .map(|angle| (angle - upright).abs())
            .fold(0.0f32, f32::max);
        if expect_upright {
            assert!(
                strayed < 0.6,
                "at a Kapitza number of {number:.2} the bone should have stayed up, \
                 and it wandered {strayed:.2} rad"
            );
        } else {
            assert!(
                strayed > 2.0,
                "at a Kapitza number of {number:.2} the bone should have fallen over, \
                 and it only wandered {strayed:.2} rad"
            );
        }
    }
}

#[test]
fn a_body_with_no_tone_in_it_never_gains_energy() {
    // The test that earns its keep. With the tone at nothing there is no spring, no holding and
    // no friction anywhere in the skeleton: it is eighteen rods on frictionless pins, and the
    // only thing that can take energy out of it is a joint arriving at one of its stops, which
    // is a damper. So the total of the kinetic and gravitational energy may fall, and it may
    // sit still, and it may not rise. Ever.
    //
    // Get a sign wrong in the mass matrix, or drop the term that says a swinging bone pulls
    // inwards on whatever it hangs from, and the body quietly invents energy. On screen that
    // does not look like a bug. It looks *livelier*, which is worse.
    let mut app = Fulcrum::with_config(FulcrumConfig::default()).with_plugin(GamePlugin);
    app.run_startup();
    app.world_mut().resource_mut::<Beat>().tempo = 0.0; // a still pelvis does no work
    app.world_mut().resource_mut::<Tone>().0 = 0.0;
    // Bent well away from the rest pose, but not into a stop: a joint that *starts* inside one
    // is being held by a spring whose energy this sum does not count, and the push it gets on
    // the way out looks exactly like energy from nowhere.
    for (index, offset) in [
        (0usize, 0.25f32),
        (5, -0.5),
        (9, 0.5),
        (12, -0.3),
        (15, 0.3),
    ] {
        app.world_mut().resource_mut::<Skeleton>().joints[index].angle += offset;
    }
    {
        let skeleton = app.world_mut().resource::<Skeleton>();
        for (index, spec) in BONES.iter().enumerate() {
            let (low, high) = spec.limits();
            let strain = skeleton.strain(index);
            assert!(
                strain > low && strain < high,
                "the test starts {:?} {:?} inside its own stop",
                spec.side,
                spec.part
            );
        }
    }

    let frame = app.world_mut().resource::<Frame>().clone();
    let start = app.world_mut().resource::<Skeleton>().energy(&frame);
    // Measured against the swing the body was given, not against its total energy, which is
    // mostly the height of a skeleton above the origin and says nothing about anything.
    let scale = BONES.iter().map(|spec| spec.mass()).sum::<f32>() * jig::game::GRAVITY * 40.0;
    let mut high = start;
    for _ in 0..1_200 {
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            input.sample(|screen| screen);
        }
        app.tick();
        high = high.max(app.world_mut().resource::<Skeleton>().energy(&frame));
    }
    assert!(
        (high - start) / scale < 0.01,
        "the body gained {:.1}% of its swing out of nowhere",
        100.0 * (high - start) / scale
    );
    // And it should have lost some, on the stops, or the test above is passing for the boring
    // reason that nothing moved.
    let end = app.world_mut().resource::<Skeleton>().energy(&frame);
    assert!(
        (start - end) / scale > 0.02,
        "nothing seems to have happened: the energy moved by {:.2}% in twenty seconds",
        100.0 * (start - end) / scale
    );
}

#[test]
fn the_rest_pose_is_inside_every_limit() {
    // The table is written in world angles, which is easy to read and easy to get wrong by a
    // whole turn: write the left arm at −12 instead of 348 and its shoulder is suddenly asked
    // to bend most of the way round. Every joint's rest bend should be a small angle, and its
    // stops should straddle it.
    for (index, spec) in BONES.iter().enumerate() {
        let bend = rest_bend(index);
        assert!(
            bend.abs() <= PI,
            "{:?} {:?} rests {:.0}° from the bone it hangs off, which is the long way round",
            spec.side,
            spec.part,
            bend.to_degrees()
        );
        let (low, high) = spec.limits();
        assert!(
            low < 0.0 && high > 0.0,
            "{:?} {:?} has stops that do not straddle its own rest angle",
            spec.side,
            spec.part
        );
    }
}

#[test]
fn the_body_is_a_tree_with_the_pelvis_at_its_root() {
    // One pass down the list has to be one pass down the tree, or a bone would be placed using
    // where its parent was last tick rather than where it is now.
    for (index, spec) in BONES.iter().enumerate() {
        if let Some(parent) = spec.parent {
            assert!(
                parent < index,
                "bone {index} ({:?}) hangs off {parent}, which comes after it",
                spec.part
            );
        }
    }
    assert_eq!(
        BONES.iter().filter(|spec| spec.parent.is_none()).count(),
        3,
        "the spine and the two thighs are the only bones on the pelvis itself"
    );
}

#[test]
fn two_limbs_only_feel_each_other_through_the_body_they_share() {
    // The mass matrix has to be symmetric — it is a kinetic energy — and it has to be empty
    // wherever two bones are not on the same path back to the pelvis. A left hand cannot pull
    // directly on a right foot; it has to go through the chest and the pelvis like everything
    // else.
    let frame = Frame::default();
    let above = |mut walk: usize, wanted: usize| loop {
        if walk == wanted {
            return true;
        }
        match BONES[walk].parent {
            Some(parent) => walk = parent,
            None => return false,
        }
    };
    #[expect(
        clippy::needless_range_loop,
        reason = "both indexes name bones, not slots"
    )]
    for j in 0..BONE_COUNT {
        for k in 0..BONE_COUNT {
            assert!(
                (frame.coupling[j][k] - frame.coupling[k][j]).abs() < 1.0,
                "the mass matrix is not symmetric at {j},{k}"
            );
            if !above(j, k) && !above(k, j) {
                assert_eq!(
                    frame.coupling[j][k], 0.0,
                    "{:?} {:?} and {:?} {:?} are on different limbs and should not be coupled",
                    BONES[j].side, BONES[j].part, BONES[k].side, BONES[k].part
                );
            }
        }
    }
}

/// Run the whole skeleton for `ticks` at this tempo and tone, and hand back the app.
fn dance(ticks: u32, tempo: f32, tone: f32, routine: usize) -> Fulcrum {
    let mut app = Fulcrum::with_config(FulcrumConfig::default()).with_plugin(GamePlugin);
    app.run_startup();
    app.world_mut().resource_mut::<Beat>().tempo = tempo;
    app.world_mut().resource_mut::<Tone>().0 = tone;
    app.world_mut().resource_mut::<Routine>().0 = routine;
    for _ in 0..ticks {
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            input.sample(|screen| screen);
        }
        app.tick();
    }
    app
}

/// How high the top of the skull is above the pelvis.
fn head_height(app: &mut Fulcrum) -> f32 {
    let skeleton = app.world_mut().resource::<Skeleton>();
    let skull = BONES
        .iter()
        .position(|spec| spec.part == Part::Skull)
        .unwrap();
    skeleton.places[skull].tip.y - skeleton.hips.y
}

#[test]
fn tone_over_one_holds_the_pose_and_tone_under_one_cannot() {
    // Tone is scaled by exactly the weight each joint carries, so one is not a tuning constant:
    // it is the least tone at which a joint can hold its own limb up, and — the same balance
    // written the other way — the least at which a standing spine is stable rather than an
    // inverted pendulum waiting to fall. Nudged off the pose with the music off, a body over
    // one comes back, and a body under one goes over.
    let settle = |tone: f32| {
        let mut app = Fulcrum::with_config(FulcrumConfig::default()).with_plugin(GamePlugin);
        app.run_startup();
        app.world_mut().resource_mut::<Beat>().tempo = 0.0;
        app.world_mut().resource_mut::<Tone>().0 = tone;
        app.world_mut().resource_mut::<Skeleton>().joints[0].angle += 0.05;
        for _ in 0..900 {
            {
                let mut input = app.world_mut().resource_mut::<Input>();
                input.sample(|screen| screen);
            }
            app.tick();
        }
        app.world_mut().resource::<Skeleton>().strain(0).abs()
    };

    let held = settle(1.6);
    let folded = settle(0.6);
    assert!(
        held < 0.05,
        "over one, a nudged spine should come back to its pose; it is {:.0}° off",
        held.to_degrees()
    );
    assert!(
        folded > 0.5,
        "under one it should have gone over; it is only {:.0}° off",
        folded.to_degrees()
    );
}

#[test]
fn a_limp_skeleton_hangs_lower_than_a_taut_one() {
    let taut = head_height(&mut dance(600, 90.0, 2.5, 2));
    let limp = head_height(&mut dance(600, 90.0, 0.5, 2));
    assert!(
        taut > 150.0,
        "with tone to spare the skull should be well above the hips, and it is at {taut:.0}"
    );
    assert!(
        limp < taut * 0.8,
        "and a limp body should be lower: {limp:.0} against {taut:.0}"
    );
}

#[test]
fn a_loose_skeleton_never_dances_the_same_bar_twice() {
    // Two skeletons, one of them a ten-thousandth of a radian out at one shoulder. Coupled
    // pendulums do not forgive that. It is why a loose body never settles into a loop even
    // though the thing driving it is two sine waves and nothing in here is random.
    let nudge = |amount: f32| {
        let mut app = Fulcrum::with_config(FulcrumConfig::default()).with_plugin(GamePlugin);
        app.run_startup();
        app.world_mut().resource_mut::<Beat>().tempo = 200.0;
        app.world_mut().resource_mut::<Tone>().0 = 0.6;
        app.world_mut().resource_mut::<Skeleton>().joints[5].angle += amount;
        for _ in 0..900 {
            {
                let mut input = app.world_mut().resource_mut::<Input>();
                input.sample(|screen| screen);
            }
            app.tick();
        }
        app.world_mut().resource::<Skeleton>().clone()
    };

    let plain = nudge(0.0);
    let nudged = nudge(1.0e-4);
    let gap = (0..BONE_COUNT)
        .map(|index| (plain.joints[index].angle - nudged.joints[index].angle).abs())
        .fold(0.0f32, f32::max);
    assert!(
        gap > 0.3,
        "fifteen seconds after a ten-thousandth of a radian, the two should be visibly          different dances; the widest joint differs by {gap:.4} rad"
    );
}

#[test]
fn a_taut_one_finds_a_groove_and_stays_in_it() {
    // The other end of the same knob, and the reason the tone is worth having as a control
    // rather than a constant. Damp the joints enough and the whole body locks on to the beat:
    // a driven system with somewhere for its energy to go settles on to a cycle, and after a
    // few bars the skeleton is in the same place at the same point of every bar.
    let mut app = dance(900, 90.0, 2.5, 2);
    let before: Vec<f32> = {
        let skeleton = app.world_mut().resource::<Skeleton>();
        (0..BONE_COUNT)
            .map(|index| skeleton.joints[index].angle)
            .collect()
    };
    for _ in 0..40 {
        // one beat at ninety to the minute
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            input.sample(|screen| screen);
        }
        app.tick();
    }
    let skeleton = app.world_mut().resource::<Skeleton>().clone();
    let drift = (0..BONE_COUNT)
        .map(|index| (skeleton.joints[index].angle - before[index]).abs())
        .fold(0.0f32, f32::max);
    assert!(
        drift < 0.06,
        "a bar later a taut skeleton should be back where it was; the widest joint has moved          {:.1}°",
        drift.to_degrees()
    );
}

#[test]
fn no_joint_is_ever_pushed_far_past_its_stop() {
    // The stops are ligaments rather than walls, so a joint arriving hard does squash a little
    // way past one. A little is fine. A lot would mean an elbow bending the wrong way in front
    // of everybody.
    for (tempo, tone) in [(240.0f32, 0.0f32), (240.0, 8.0), (92.0, 2.6), (0.0, 0.0)] {
        for routine in 0..5 {
            let mut app = dance(600, tempo, tone, routine);
            let skeleton = app.world_mut().resource::<Skeleton>().clone();
            for (index, spec) in BONES.iter().enumerate() {
                let (low, high) = spec.limits();
                let strain = skeleton.strain(index);
                let past = (low - strain).max(strain - high).max(0.0);
                assert!(
                    past < 0.12,
                    "at {tempo} bpm, tone {tone}, step {routine}: {:?} {:?} is {:.0}° past its \
                     stop",
                    spec.side,
                    spec.part,
                    past.to_degrees()
                );
            }
        }
    }
}

#[test]
fn the_hardest_shaking_it_can_be_given_does_not_tear_it_apart() {
    // The failure mode an explicit integrator really has: the drive's acceleration goes as the
    // square of the tempo, and too coarse a step feeds the skeleton energy out of nowhere.
    for routine in 0..5 {
        let mut app = dance(1_200, 240.0, 0.0, routine);
        let skeleton = app.world_mut().resource::<Skeleton>().clone();
        for (index, spec) in BONES.iter().enumerate() {
            let joint = skeleton.joints[index];
            assert!(
                joint.angle.is_finite() && joint.rate.is_finite(),
                "on step {routine}, {:?} {:?} came apart",
                spec.side,
                spec.part
            );
            assert!(
                joint.rate.abs() < 300.0,
                "on step {routine}, {:?} {:?} is turning at {:.0} rad/s, which is not dancing",
                spec.side,
                spec.part,
                joint.rate
            );
            // And it should still be a skeleton: every bone the length it started.
            let place = skeleton.places[index];
            let along = (place.tip - place.pivot) - direction(place.angle) * spec.length;
            assert!(
                along.length() < 0.01,
                "{:?} {:?} is not where it says it is",
                spec.side,
                spec.part
            );
        }
    }
}

#[test]
fn with_the_music_stopped_it_comes_to_rest() {
    // Nothing in here is self-propelled. Stop the hips and, so long as there is any tone at all
    // to take the energy, everything else winds down.
    let mut app = dance(1_200, 0.0, 2.6, 2);
    let skeleton = app.world_mut().resource::<Skeleton>().clone();
    let fastest = (0..BONE_COUNT)
        .map(|index| skeleton.joints[index].rate.abs())
        .fold(0.0f32, f32::max);
    assert!(
        fastest < 0.02,
        "twenty seconds after the music stopped, something is still moving at {fastest:.3} rad/s"
    );
    assert_eq!(
        skeleton.hips,
        Vec2::ZERO,
        "and the hips should be back where they started"
    );
}
