//! What has to be true of a sky before anything is drawn with it.
//!
//! The picture cannot be checked by a test, so these check the things underneath it that a
//! picture would only show once they had gone wrong: that the drawing order is back to front,
//! that the caps the shader throws work by really do hold what they claim, that no cloud is ever
//! swapped for another while it is on the screen, and that a whole compass of weather still fits
//! down the bus.
//!
//! Two of them are here for what this version changes. No element anywhere in the sky is allowed
//! to be one circle, at any setting and at any moment in its life, since a closed shape drawn as
//! a circle is the bubble the whole construction is meant to avoid. And turning the arc setting
//! is allowed to add arcs to elements and to do nothing else: the same clouds, in the same
//! places, with the same first circle in each.

use moebius3::cloud::{
    ARCS_MAX, ARCS_MIN, BANDS, MAX_DISCS, MAX_GROUPS, SMALLEST, Sky, Style, length,
};
use moebius3::game::Weather;
use moebius3::look::LOOKS;
use moebius3::sky::{Slab, compose, cull};

/// A spread of moments, far enough apart to be different weather.
fn moments() -> impl Iterator<Item = f32> {
    (0..40).map(|step| step as f32 * 37.3)
}

/// Every arc setting the keys can reach.
fn settings() -> impl Iterator<Item = Style> {
    (ARCS_MIN..=ARCS_MAX).map(|arcs| Style {
        arcs,
        ..Default::default()
    })
}

#[test]
fn groups_come_back_to_front() {
    // Height stands in for distance here, so the far band has to go down first and the near band
    // last. Get this the wrong way round and a cloud on the horizon is drawn in front of one
    // overhead, which is the one mistake in this piece that no amount of palette will hide.
    for clock in moments() {
        let sky = Sky::at(clock, Style::default());
        let mut band = u32::MAX;
        for group in &sky.groups {
            assert!(
                group.band <= band,
                "band {} drawn after band {band} at {clock}",
                group.band
            );
            band = group.band;
        }
    }
}

#[test]
fn no_element_is_a_plain_circle() {
    // The rule this version is built around. Every group is one closed shape with one outline,
    // and a group of a single circle draws that outline as a circle, which reads as a hole in the
    // paper rather than as a cloud sitting on a cloud. Crests are unions of two to six by
    // construction; the case worth watching is a body worn down to one circle by the envelope,
    // in the first and last seconds of a cloud's life.
    for style in settings() {
        for clock in moments() {
            let sky = Sky::at(clock, style);
            for group in &sky.groups {
                assert!(
                    group.count >= 2,
                    "a group of {} circle at {clock} with {} arcs, which is a bubble",
                    group.count,
                    style.arcs
                );
            }
        }
    }
}

#[test]
fn an_element_is_never_two_shapes() {
    // The other half of the same rule. Two circles far enough apart draw as two closed outlines
    // inside one group, which is two bubbles rather than none, so every circle in a group has to
    // overlap another one in it.
    for style in settings() {
        for clock in [0.0, 211.0, 640.0, 1503.0] {
            let sky = Sky::at(clock, style);
            for group in &sky.groups {
                let discs = &sky.discs[group.first as usize..(group.first + group.count) as usize];
                for (index, disc) in discs.iter().enumerate() {
                    let touches = discs.iter().enumerate().any(|(other, near)| {
                        other != index
                            && length([disc[0] - near[0], disc[1] - near[1], disc[2] - near[2]])
                                < disc[3] + near[3]
                    });
                    assert!(
                        touches,
                        "a circle stands on its own inside a group at {clock} with {} arcs",
                        style.arcs
                    );
                }
            }
        }
    }
}

#[test]
fn turning_the_arcs_up_only_adds_arcs() {
    // The setting moves under a key while the sky is running, so it has to add and take away
    // arcs without reshuffling anything: the same clouds, the same number of elements, and the
    // same circle at the head of each. Every arc a crest could have is drawn from the hash
    // whether it is used or not, which is what buys this.
    for clock in [0.0, 311.0, 907.0] {
        let fewest = Sky::at(
            clock,
            Style {
                arcs: ARCS_MIN,
                ..Default::default()
            },
        );
        for arcs in ARCS_MIN + 1..=ARCS_MAX {
            let more = Sky::at(
                clock,
                Style {
                    arcs,
                    ..Default::default()
                },
            );
            assert_eq!(
                fewest.groups.len(),
                more.groups.len(),
                "{arcs} arcs changed how many elements there are at {clock}"
            );
            assert!(
                more.discs.len() > fewest.discs.len(),
                "{arcs} arcs drew no more circles than {ARCS_MIN} at {clock}"
            );
            for (thin, thick) in fewest.groups.iter().zip(&more.groups) {
                assert_eq!(
                    sky_disc(&fewest, thin.first),
                    sky_disc(&more, thick.first),
                    "{arcs} arcs moved an element at {clock}"
                );
                assert!(
                    thick.count >= thin.count,
                    "{arcs} arcs took an arc off an element at {clock}"
                );
            }
        }
    }
}

/// One circle out of a sky, by index.
fn sky_disc(sky: &Sky, index: u32) -> [f32; 4] {
    sky.discs[index as usize]
}

#[test]
fn the_ceiling_on_the_arcs_is_worth_having() {
    // The first of this version's changes, and the thing that would quietly undo it. Raising the
    // setting is not enough on its own: an arc only arrives if it comes out bigger than the
    // smallest circle worth drawing, so a ceiling raised without the arcs to fill it would leave
    // the top of the range doing nothing and no other test would say a word.
    const { assert!(ARCS_MAX >= 16, "that is not a ceiling worth raising") };
    let style = Style {
        arcs: ARCS_MAX,
        ..Default::default()
    };
    let sky = Sky::at(640.0, style);
    let full = sky
        .groups
        .iter()
        .filter(|group| group.count == ARCS_MAX)
        .count();
    assert!(
        full > 20,
        "only {full} elements in the whole sky reached all {ARCS_MAX} arcs"
    );
}

#[test]
fn every_circle_is_inside_its_cap() {
    // The shader skips a whole group on this cap. A circle poking out of one is a piece of cloud
    // that is drawn from some angles and not from others.
    for style in settings() {
        for clock in moments() {
            let sky = Sky::at(clock, style);
            for group in &sky.groups {
                let cap = group.cap;
                for disc in &sky.discs[group.first as usize..(group.first + group.count) as usize] {
                    let offset = [disc[0] - cap[0], disc[1] - cap[1], disc[2] - cap[2]];
                    assert!(
                        length(offset) + disc[3] <= cap[3] + 1e-5,
                        "a circle reaches past its cap at {clock}"
                    );
                }
            }
        }
    }
}

#[test]
fn a_circle_is_never_smaller_than_the_line_around_it() {
    for style in settings() {
        for clock in moments() {
            let sky = Sky::at(clock, style);
            for disc in &sky.discs {
                assert!(
                    disc[3] > SMALLEST,
                    "a circle of {} rad is a full stop, not a circle",
                    disc[3]
                );
            }
        }
    }
}

#[test]
fn the_circles_are_unit_directions() {
    for clock in moments() {
        for disc in &Sky::at(clock, Style::default()).discs {
            let len = length([disc[0], disc[1], disc[2]]);
            assert!((len - 1.0).abs() < 1e-4, "a circle points {len} of the way");
            assert!(disc[3].is_finite(), "a circle of no size at all");
        }
    }
}

#[test]
fn a_moment_is_a_moment() {
    // Nothing accumulates, so the same clock has to give the same sky however it was arrived at.
    // It is what lets the still example render any moment in the weather without playing up to
    // it, and what makes the pictures the numbers were set by reproducible.
    for clock in moments() {
        let first = Sky::at(clock, Style::default());
        let second = Sky::at(clock, Style::default());
        assert_eq!(first.groups, second.groups, "the groups moved at {clock}");
        assert_eq!(first.discs, second.discs, "the circles moved at {clock}");
    }
}

#[test]
fn the_weather_is_weather() {
    // A sky that never changes would pass every other test in this file.
    let early = Sky::at(100.0, Style::default());
    let late = Sky::at(400.0, Style::default());
    assert_ne!(early.discs, late.discs, "the sky stood still for 5 minutes");
}

/// How much circle there is in the sky, as the sum of the squares of the radii. Not an area, but
/// proportional to one, and the cheapest number that moves when any circle does.
fn weight(sky: &Sky) -> f64 {
    sky.discs.iter().map(|d| f64::from(d[3] * d[3])).sum()
}

#[test]
fn a_cloud_is_never_swapped_while_it_is_showing() {
    // The one thing that would be unwatchable. A slot holds one cloud after another, and when it
    // hands over, the new cloud is somewhere else in the sky and a different shape. So every
    // circle has to be gone before the handover and grow back after it, and the way to check
    // that without naming circles is to watch how much circle there is: a cloud arriving whole
    // would show as a step.
    let style = Style::default();
    let mut previous = weight(&Sky::at(0.0, style));
    let mut worst = 0.0f64;
    let mut at = 0.0;
    // A tenth of a second at pace one, over twenty minutes, which crosses several hundred
    // handovers across the three bands.
    for step in 1..12_000 {
        let clock = step as f32 * 0.1;
        let now = weight(&Sky::at(clock, style));
        let jump = (now - previous).abs() / previous.max(1e-9);
        if jump > worst {
            worst = jump;
            at = clock;
        }
        previous = now;
    }
    // A whole cloud is about a percent of the sky and one crest about a third of that, so the
    // gate is set below both. What it measures in practice is around a fifth of a percent, all
    // of it circles growing and shrinking smoothly.
    assert!(
        worst < 0.004,
        "the sky changed by {:.3}% in a tenth of a second at {at}, which is a cloud appearing",
        worst * 100.0
    );
}

#[test]
fn a_whole_compass_of_weather_fits_down_the_bus() {
    // Only what is in front of you is sent, and the buffer it is sent in is a fixed size. This
    // watches the margin between the two: if a change to the bands ever pushes a frame past the
    // buffer, groups are dropped and clouds go missing rather than the picture simply costing
    // more. Measured at the widest arc setting, which is where the circles are.
    let style = Style {
        arcs: ARCS_MAX,
        ..Default::default()
    };
    let mut slab = Slab::boxed();
    let mut most_groups = 0;
    let mut most_discs = 0;
    for clock in moments() {
        let sky = Sky::at(clock, style);
        for turn in 0..24 {
            let weather = Weather {
                clock,
                yaw: turn as f32 * std::f32::consts::TAU / 24.0,
                ..Default::default()
            };
            let uniforms = compose(&weather, &sky, &LOOKS[0], style, (2560, 1440), &mut slab);
            let groups = uniforms.counts[0] as usize;
            let discs: usize = (0..groups).map(|g| slab.span[g][1] as usize).sum();
            most_groups = most_groups.max(groups);
            most_discs = most_discs.max(discs);
        }
    }
    assert!(
        most_groups * 2 < MAX_GROUPS && most_discs * 2 < MAX_DISCS,
        "a frame reached {most_groups} groups and {most_discs} circles, \
         over half of the {MAX_GROUPS} and {MAX_DISCS} there is room for"
    );
}

#[test]
fn culling_leaves_the_order_alone() {
    // Order is the whole of the depth in this piece. Dropping the groups behind you must not
    // move the ones in front of each other.
    let sky = Sky::at(613.0, Style::default());
    let mut slab = Slab::boxed();
    let weather = Weather {
        clock: 613.0,
        yaw: 1.1,
        ..Default::default()
    };
    let kept = cull(&sky, weather.eye().forward, 1.6, &mut slab);
    assert!(kept > 0, "nothing at all was in front of the eye");

    let mut walk = sky.groups.iter();
    for group in 0..kept {
        let tint = slab.span[group][2] as u32;
        let count = slab.span[group][1] as u32;
        // The kept groups are a subsequence of the built ones, in order.
        let found = walk
            .by_ref()
            .find(|built| built.tint == tint && built.count == count);
        assert!(found.is_some(), "group {group} is out of order or missing");
    }
}

#[test]
fn the_bands_are_stacked_and_not_shuffled() {
    // The three bands are the whole depth cue, so they have to be told apart at a glance: a near
    // band that reaches down to where the far band sits is three bands drawn as one.
    for pair in BANDS.windows(2) {
        let (near, far) = (&pair[0], &pair[1]);
        assert!(
            near.elevation.0 > far.elevation.0 && near.elevation.1 > far.elevation.1,
            "a nearer band has to sit higher in the sky"
        );
        assert!(
            near.scale.0 > far.scale.0 && near.scale.1 > far.scale.1,
            "a nearer band has to be bigger"
        );
        assert!(near.rate > far.rate, "a nearer band has to cross faster");
        assert!(
            near.stretch < far.stretch,
            "a further band is seen more nearly edge on, so it is drawn out further"
        );
    }
}
