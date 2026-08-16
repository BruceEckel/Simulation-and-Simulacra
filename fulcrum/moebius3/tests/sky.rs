//! The palettes, the settings, and the buffer they are handed to the shader in.
//!
//! A palette here is not a tint over a picture that would look much the same without it: every
//! area in the drawing is filled with one of these colours, so a palette that does not hold
//! together is a picture that does not. These hold each one of the twenty to being a whole sky,
//! which is what makes adding the next one a matter of writing fifteen colours down and running
//! the tests rather than of looking at it and hoping.

use moebius3::cloud::{
    ARCS_MAX, ARCS_MIN, HATCH_MAX, HATCH_MIN, INK_MAX, INK_MIN, MAX_DISCS, MAX_GROUPS, Style,
};
use moebius3::game::Weather;
use moebius3::look::{LOOKS, Look};
use moebius3::rider::Rider;
use moebius3::sky::{
    MESAS, PICTURE_INK, SHADE_ANGLE, SHADE_UNDER, SUN_RADIUS, SUN_RING, Slab, compose, rocks,
};

/// How light a colour is, near enough for an ordering.
fn light(colour: [f32; 3]) -> f32 {
    colour[0] * 0.30 + colour[1] * 0.59 + colour[2] * 0.11
}

/// How far apart two colours are.
fn apart(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

#[test]
fn there_are_plenty_of_them_and_they_are_all_different() {
    assert!(LOOKS.len() >= 20, "only {} palettes", LOOKS.len());
    for (index, look) in LOOKS.iter().enumerate() {
        assert!(!look.name.is_empty(), "a palette with no name");
        for other in &LOOKS[index + 1..] {
            assert_ne!(look.name, other.name, "two palettes called {}", look.name);
            assert!(
                apart(look.sky[4], other.sky[4]) > 0.02
                    || apart(look.sand[0], other.sand[0]) > 0.02,
                "{} and {} are the same sky",
                look.name,
                other.name
            );
        }
    }
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
    // The horizon is the lit end in all of these, so the five sky colours have to run one way. A
    // pair out of order puts a band of daylight halfway up a night sky.
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
fn the_settings_are_held_inside_what_the_drawing_can_take() {
    // Both keys ramp, and both ends of both ramps have to stop somewhere the picture still works:
    // an element of one circle is a bubble, and a line of no width is not a line.
    let wild = Style {
        arcs: 999,
        cloud_ink: 40.0,
        hatch: 9.0,
        ..Default::default()
    }
    .clamped();
    assert_eq!(wild.arcs, ARCS_MAX);
    assert_eq!(wild.cloud_ink, INK_MAX);
    assert_eq!(wild.hatch, HATCH_MAX);
    let starved = Style {
        arcs: 0,
        cloud_ink: -3.0,
        hatch: 0.0,
        ..Default::default()
    }
    .clamped();
    assert_eq!(starved.arcs, ARCS_MIN);
    assert_eq!(starved.cloud_ink, INK_MIN);
    assert_eq!(starved.hatch, HATCH_MIN);
    const { assert!(ARCS_MIN >= 2, "one circle is a bubble") };
    let default = Style::default();
    assert_eq!(default, default.clamped(), "the default is out of range");
}

#[test]
fn the_shader_and_the_buffer_agree_on_how_big_the_sky_is() {
    // The sizes are written down twice, once in Rust and once in WGSL, and nothing but this
    // checks that they are the same number. A shader that declares a smaller array than the
    // buffer bound to it does not fail: it reads the wrong circles, or none, for everything past
    // its own end, so the picture quietly loses clouds on exactly the busiest frames. This has
    // already happened once, when the ceiling on the arcs went up and only one of the two numbers
    // followed.
    let source = include_str!("../src/moebius3.wgsl");
    for (field, count) in [
        ("cap", MAX_GROUPS),
        ("span", MAX_GROUPS),
        ("plane", MAX_GROUPS),
        ("disc", MAX_DISCS),
    ] {
        let declaration = format!("{field}: array<vec4<f32>, {count}>,");
        assert!(
            source.contains(&declaration),
            "the shader does not declare `{declaration}`"
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
    let style = Style {
        arcs: 4,
        cloud_ink: 3.5,
        shade: true,
        hatch: 0.12,
    };
    let sky = moebius3::cloud::Sky::at(500.0, style);
    let mut slab = Slab::boxed();
    let weather = Weather {
        clock: 500.0,
        yaw: 0.8,
        ..Default::default()
    };
    let look: &Look = &LOOKS[2];
    let out = compose(&weather, &sky, look, style, (1920, 1200), &mut slab);

    assert_eq!(out.forward[3], 1920.0 / 1200.0, "aspect");
    assert_eq!(out.right[3], 1920.0, "width");
    assert_eq!(out.up[3], 1200.0, "height");
    assert!((out.sun[3] - SUN_RADIUS.cos()).abs() < 1e-6, "sun radius");
    assert!(
        (out.counts[1] - (SUN_RADIUS * SUN_RING).cos()).abs() < 1e-6,
        "the ring around the sun"
    );
    assert!(out.counts[1] < out.sun[3], "the ring is inside the disc");
    // The settings, and the one line weight that is not a setting.
    assert_eq!(out.counts[2], style.cloud_ink, "the line around a cloud");
    assert_eq!(out.counts[3], style.arcs as f32, "arcs an element");
    assert_eq!(out.shade[0], style.hatch, "hatch spacing");
    assert_eq!(
        out.shade[1], SHADE_UNDER,
        "how far down before it is crossed"
    );
    assert_eq!(out.shade[2], SHADE_ANGLE, "the angle it is drawn at");
    assert_eq!(out.shade[3], 1.0, "the shading is on");
    assert_eq!(
        out.pen[0], PICTURE_INK,
        "the line everything else is drawn with"
    );

    // The traveller: a unit direction below the horizon, and a size the drawing can see.
    let towards = [out.rider[0], out.rider[1], out.rider[2]];
    let length =
        (towards[0] * towards[0] + towards[1] * towards[1] + towards[2] * towards[2]).sqrt();
    assert!((length - 1.0).abs() < 1e-5, "the rider points nowhere");
    assert!(out.rider[1] < 0.0, "the rider is above the horizon");
    assert!(out.rider[3] > 0.0, "a rider of no size");
    let span = moebius3::game::frame_span(1920.0 / 1200.0);
    assert_eq!(
        out.gait[0],
        Rider::at(500.0, weather.yaw, span).gait,
        "the stride is off the clock"
    );
    assert_eq!(
        out.gait[1], span,
        "the shader is not told how wide the frame is"
    );
    // Folded into the picture: he is within half a frame of where the head is pointed, which is
    // what puts him on the screen at every moment.
    let off = out.rider[0].atan2(out.rider[2]) - weather.yaw;
    let off = (off + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    assert!(
        off.abs() <= span * 0.5 + 1e-4,
        "the rider is {off} rad off the middle of the picture"
    );

    // With the shading off, the shader is told so by the one component it looks at.
    let unshaded = compose(
        &weather,
        &sky,
        look,
        Style {
            shade: false,
            ..style
        },
        (1920, 1200),
        &mut slab,
    );
    assert_eq!(unshaded.shade[3], 0.0, "the shading is off");
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
        assert!(count > 1, "an element of one circle reached the buffer");
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
