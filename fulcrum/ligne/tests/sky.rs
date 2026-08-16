//! The half of the piece that can be checked without a GPU: the camera the shader is handed,
//! the palettes it draws with, and the buffer the two meet in.

use ligne::game::Weather;
use ligne::look::LOOKS;
use ligne::sky::{CLOUD_BANDS, DECKS, compose};

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
    // The shader builds a ray out of these three vectors and nothing else. If they are not unit
    // length and square to each other the whole picture is sheared, and there is no way to see
    // that it is: a sheared sky still looks like a sky.
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
                "{name} is not unit length"
            );
        }
        assert!(dot(eye.forward, eye.right).abs() < 1e-4, "not square");
        assert!(dot(eye.forward, eye.up).abs() < 1e-4, "not square");
        assert!(dot(eye.right, eye.up).abs() < 1e-4, "not square");
        assert!(eye.forward[1] > 0.0, "the view should tilt upwards");
    }
}

#[test]
fn the_decks_stack() {
    // Lower is nearer, so the decks have to be listed lowest first: the shader draws them in
    // reverse and would otherwise paint the far ones over the near ones. Each one further off
    // is also further into the haze, which is the only depth cue a flat drawing has.
    for step in 1..DECKS.len() {
        let (near, far) = (DECKS[step - 1], DECKS[step]);
        assert!(far[0] > near[0], "deck {step} is not higher than the last");
        assert!(far[3] > near[3], "deck {step} is not hazier than the last");
        assert!(
            far[2] >= near[2],
            "deck {step} is not thinner than the last"
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

        // The cloud ramp is walked from nought to one as the tone rises, so it has to get
        // brighter as it goes: a ramp that dips in the middle puts a dark band across the lit
        // side of every cloud in the sky.
        let luma = |c: [f32; 3]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        for stop in 1..look.cloud.len() {
            assert!(
                luma(look.cloud[stop]) > luma(look.cloud[stop - 1]),
                "{}: cloud stop {stop} is darker than the one before it",
                look.name
            );
        }
        // The ink has to be darker than everything it is drawn over, or it is a highlight.
        let darkest = look
            .cloud
            .iter()
            .chain([&look.sky_zenith, &look.ground_near])
            .map(|c| luma(*c))
            .fold(f32::MAX, f32::min);
        assert!(
            luma(look.ink) < darkest,
            "{}: the ink is not the darkest thing in the palette",
            look.name
        );
        // What distance fades into has to be what the horizon already is, or the horizon comes
        // out as a seam between two different skies.
        for channel in 0..3 {
            let gap = (look.haze[channel] - look.sky_horizon[channel]).abs();
            assert!(gap < 0.2, "{}: haze and horizon disagree", look.name);
        }
    }
}

#[test]
fn the_uniforms_carry_what_the_shader_reads() {
    // A spot check on the one place the halves of the piece meet. Everything in the buffer is a
    // bare `vec4` with no names on the far side, so a field written into the wrong slot is a
    // silent and very confusing mistake.
    let weather = Weather {
        drift: [120.0, -45.0],
        boil: 9.5,
        ..Default::default()
    };
    let look = &LOOKS[0];
    let u = compose(&weather, look, (1600, 900));

    assert_eq!(u.forward[3], 1600.0 / 900.0, "aspect ratio");
    assert_eq!(u.right[3], 1600.0, "window width");
    assert_eq!(u.up[3], 900.0, "window height");
    // The wind is a two-dimensional drift written into the x and z of a three-dimensional slot,
    // with the boil in the fourth. Getting that packing wrong is silent and looks like a bug in
    // the noise.
    assert_eq!(u.wind[0], 120.0);
    assert_eq!(u.wind[1], 0.0, "the wind does not blow upwards");
    assert_eq!(u.wind[2], -45.0);
    assert_eq!(u.wind[3], 9.5, "the boil");
    assert_eq!(u.deck_a, DECKS[0]);
    assert_eq!(u.deck_c, DECKS[2]);
    assert_eq!(u.shade[0], CLOUD_BANDS, "band count");
    assert_eq!(u.cloud_3[0], look.cloud[3][0], "the lit crown");
    assert!(u.world[0] > 1e6, "the planet is a planet");
    assert!(u.world[3] >= 256.0, "the sheet size, for the mip level");
    assert!(u.origin[3] > 0.0, "the field of view opens");
}
