//! The palettes, and the buffer they are handed to the shader in.
//!
//! A palette here is not a tint over a picture that would look much the same without it: every
//! area in the drawing is filled with one of these colours, so a palette that does not hold
//! together is a picture that does not. These hold each one to being a whole sky.

use moebius::game::Weather;
use moebius::look::{LOOKS, Look};
use moebius::sky::{INK_WIDTH, MESAS, SUN_RADIUS, SUN_RING, Slab, compose, rocks};

/// How light a colour is, near enough for an ordering.
fn light(colour: [f32; 3]) -> f32 {
    colour[0] * 0.30 + colour[1] * 0.59 + colour[2] * 0.11
}

/// How far apart two colours are.
fn apart(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

#[test]
fn the_line_is_the_darkest_thing_there_is() {
    // One colour draws every line in the picture, over the sky, over the sand, over the rock and
    // over every cloud. Anything it is not clearly darker than is somewhere the drawing goes
    // quiet.
    for look in LOOKS {
        let ink = light(look.ink);
        let others = look
            .sky
            .iter()
            .chain(&look.sand)
            .chain(&look.cloud)
            .chain(std::iter::once(&look.mesa))
            .chain(std::iter::once(&look.sun));
        for colour in others {
            assert!(
                light(*colour) > ink + 0.06,
                "{}: the line is not dark enough against {colour:?}",
                look.name
            );
        }
    }
}

#[test]
fn the_sky_darkens_upwards() {
    // The horizon is the lit end in all five of these, so the five sky colours have to run one
    // way. A pair out of order puts a band of daylight halfway up a night sky.
    for look in LOOKS {
        for pair in look.sky.windows(2) {
            assert!(
                light(pair[0]) > light(pair[1]) + 0.02,
                "{}: the sky does not darken from {:?} to {:?}",
                look.name,
                pair[0],
                pair[1]
            );
        }
    }
}

#[test]
fn the_sand_lightens_towards_the_horizon() {
    for look in LOOKS {
        for pair in look.sand.windows(2) {
            assert!(
                light(pair[0]) < light(pair[1]),
                "{}: the sand does not lighten with distance",
                look.name
            );
        }
    }
}

#[test]
fn the_cloud_colours_are_told_apart() {
    // The four are a separation, not a ramp off a light value, but they still have to be four
    // and not two: a band whose colour is the colour of the band in front of it has lost the
    // only depth cue this piece has.
    for look in LOOKS {
        for (index, pair) in look.cloud.windows(2).enumerate() {
            assert!(
                apart(pair[0], pair[1]) > 0.07,
                "{}: cloud colours {index} and {} are the same colour",
                look.name,
                index + 1
            );
        }
    }
}

#[test]
fn a_cloud_shows_against_the_sky_behind_it() {
    // The near band is drawn over every part of the sky there is, so it has to be visible
    // against all five of them as a shape and not only as a line around one.
    for look in LOOKS {
        for sky in &look.sky {
            assert!(
                apart(look.cloud[3], *sky) > 0.16,
                "{}: the near clouds vanish into the sky at {sky:?}",
                look.name
            );
        }
    }
}

#[test]
fn the_sun_stands_in_the_part_of_the_sky_it_lights() {
    // The palette says which way the light comes from and the sun is drawn there. It is the one
    // place in this piece where a light direction is used at all, and it is used to place a
    // disc rather than to shade anything.
    for look in LOOKS {
        let towards = look.sun_direction();
        let length =
            (towards[0] * towards[0] + towards[1] * towards[1] + towards[2] * towards[2]).sqrt();
        assert!((length - 1.0).abs() < 1e-5, "{}: crooked sun", look.name);
        assert!(
            towards[1] > 0.0,
            "{}: the sun is below the horizon",
            look.name
        );
    }
}

#[test]
fn the_rock_is_rock_and_not_a_fence() {
    let rock = rocks();
    let standing: Vec<_> = rock.iter().filter(|r| r[2] > 0.0).collect();
    assert!(
        standing.len() >= 6 && standing.len() < MESAS,
        "{} pieces of rock around the whole compass",
        standing.len()
    );
    let widest = standing.iter().map(|r| r[1]).fold(0.0f32, f32::max);
    let narrowest = standing.iter().map(|r| r[1]).fold(f32::MAX, f32::min);
    assert!(
        widest > narrowest * 2.5,
        "every piece of rock is the same width, which is a fence"
    );
}

#[test]
fn the_uniforms_land_where_the_shader_reads_them() {
    // Every loose component of the buffer, checked against the comment that says what it holds.
    // The shader reads these by index and cannot say when one has moved.
    let sky = moebius::cloud::Sky::at(500.0);
    let mut slab = Slab::boxed();
    let weather = Weather {
        clock: 500.0,
        yaw: 0.8,
        ..Default::default()
    };
    let look: &Look = &LOOKS[2];
    let out = compose(&weather, &sky, look, (1920, 1200), &mut slab);

    assert_eq!(out.forward[3], 1920.0 / 1200.0, "aspect");
    assert_eq!(out.right[3], 1920.0, "width");
    assert_eq!(out.up[3], 1200.0, "height");
    assert!((out.sun[3] - SUN_RADIUS.cos()).abs() < 1e-6, "sun radius");
    assert!(
        (out.counts[1] - (SUN_RADIUS * SUN_RING).cos()).abs() < 1e-6,
        "the ring around the sun"
    );
    assert!(out.counts[1] < out.sun[3], "the ring is inside the disc");
    assert_eq!(out.pen[0], INK_WIDTH, "line width");
    assert_eq!(out.screen[2], 1.0 / 1920.0, "one over the width");
    assert_eq!(
        out.sky_0,
        [look.sky[0][0], look.sky[0][1], look.sky[0][2], 0.0]
    );
    assert_eq!(
        out.cloud_3,
        [look.cloud[3][0], look.cloud[3][1], look.cloud[3][2], 0.0]
    );
    let groups = out.counts[0] as usize;
    assert!(groups > 0, "an empty sky");
    // Every group in the slab points at circles that are in the slab.
    for group in 0..groups {
        let first = slab.span[group][0] as usize;
        let count = slab.span[group][1] as usize;
        assert!(count > 0, "an empty group");
        assert!(
            first + count <= slab.disc.len(),
            "group {group} points past the end of the circles"
        );
        assert!(
            slab.span[group][2] < 4.0,
            "group {group} asks for a cloud colour that is not there"
        );
    }
}
