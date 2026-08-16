//! The half of the piece that can be checked without a GPU: the camera the shader is handed,
//! the dials it is handed, and the fact that every palette is a whole one.

use nimbus::game::{COVERAGE, Weather};
use nimbus::look::LOOKS;
use nimbus::sky::compose;

/// Length of a vector.
fn length(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

/// Dot of two of them.
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[test]
fn the_eye_is_a_camera() {
    // The shader builds a ray out of these three vectors and nothing else, so if they are not
    // unit length and square to each other the whole picture is sheared and there is no way to
    // see that it is: a sheared sky still looks like a sky.
    for step in 0..16 {
        let weather = Weather {
            yaw: step as f32 * 0.4,
            ..Default::default()
        };
        let eye = weather.eye();
        for (name, axis) in [
            ("forward", eye.forward),
            ("right", eye.right),
            ("up", eye.up),
        ] {
            assert!(
                (length(axis) - 1.0).abs() < 1e-4,
                "{name} is {} long",
                length(axis)
            );
        }
        assert!(dot(eye.forward, eye.right).abs() < 1e-4, "not square");
        assert!(dot(eye.forward, eye.up).abs() < 1e-4, "not square");
        assert!(dot(eye.right, eye.up).abs() < 1e-4, "not square");
        assert!(eye.forward[1] > 0.0, "the view should tilt upwards");
    }
}

#[test]
fn turning_all_the_way_round_comes_back() {
    let weather = Weather {
        yaw: std::f32::consts::TAU,
        ..Default::default()
    };
    let turned = weather.eye();
    let straight = Weather::default().eye();
    for axis in 0..3 {
        assert!((turned.forward[axis] - straight.forward[axis]).abs() < 1e-4);
    }
}

#[test]
fn the_coverage_dial_stays_on_its_dial() {
    for step in 0..64 {
        let weather = Weather {
            swell: step as f32 * 0.1,
            ..Default::default()
        };
        let coverage = weather.coverage();
        assert!(
            (COVERAGE.0..=COVERAGE.1).contains(&coverage),
            "coverage wandered to {coverage}"
        );
    }
}

#[test]
fn every_palette_is_a_whole_sky() {
    for look in LOOKS {
        let sun = look.sun_direction();
        assert!(
            (length(sun) - 1.0).abs() < 1e-4,
            "{}: the sun is not a direction",
            look.name
        );
        assert!(sun[1] > 0.0, "{}: the sun is below the horizon", look.name);
        assert!(look.exposure > 0.0, "{}: no exposure", look.name);
        // The haze is what distance fades into, and the sky at the horizon is what a ray that
        // goes to the distance ends on. If those two disagree the horizon comes out as a seam.
        for channel in 0..3 {
            let gap = (look.haze[channel] - look.sky_horizon[channel]).abs();
            assert!(
                gap < 0.35,
                "{}: haze and horizon disagree on channel {channel} by {gap}",
                look.name
            );
        }
    }
}

#[test]
fn the_uniforms_carry_what_the_shader_reads() {
    // A spot check on the one place the three halves of the piece meet. Everything in the
    // buffer is a bare `vec4` with no names on the far side, so a field written into the wrong
    // slot is a silent, and very confusing, mistake.
    let weather = Weather::default();
    let look = &LOOKS[0];
    let u = compose(&weather, look, (1600, 900), (800, 450), 6.0, 0.8);

    assert_eq!(u.forward[3], 1600.0 / 900.0, "aspect ratio");
    assert_eq!(u.right[3], 800.0, "internal width");
    assert_eq!(u.up[3], 450.0, "internal height");
    assert_eq!(u.screen[0], 1600.0);
    assert!((u.screen[2] - 1.0 / 1600.0).abs() < 1e-9, "inverse width");
    assert_eq!(u.shape[0], weather.coverage(), "coverage");
    assert_eq!(u.finish[0], 6.0, "band count");
    assert_eq!(u.sun_colour[3], look.sun_power, "the sun's own brightness");
    assert!(u.layer[0] < u.layer[1], "the cloud base is under its top");
    assert!(u.layer[2] > u.layer[1] * 100.0, "the planet is a planet");
    assert!(u.origin[3] > 0.0, "the field of view opens");
}
