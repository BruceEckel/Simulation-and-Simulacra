//! The half of the piece that can be checked without a GPU: the camera the shader is handed, the
//! palettes it paints with, and the buffer the two meet in.

use parrish::game::Weather;
use parrish::look::{LOOKS, Look};
use parrish::sky::{CROWN, DECKS, RELIEF, TOWER, compose};

/// Length of a vector.
fn length(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

/// Dot of two of them.
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// How much light a coat lets through, all told.
fn luma(c: [f32; 3]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

/// Every glaze in one palette, named, in no particular order.
fn glazes(look: &Look) -> [(&'static str, [f32; 3]); 11] {
    [
        ("sky_high", look.sky_high),
        ("sky_low", look.sky_low),
        ("glow", look.glow),
        ("cloud_light", look.cloud_light),
        ("cloud_shadow", look.cloud_shadow),
        ("cloud_deep", look.cloud_deep),
        ("distance", look.distance),
        ("water", look.water),
        ("ridge_far", look.ridge_far),
        ("ridge_near", look.ridge_near),
        ("ledge", look.ledge),
    ]
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
        assert!(eye.at[1] > 0.0, "the eye should stand above the water");
    }
}

#[test]
fn the_decks_stack() {
    // Lower is nearer, so the decks have to be listed lowest first: the shader draws them in
    // reverse and would otherwise paint the far ones over the near ones. Each one further off is
    // also thinner and further into the blue, which is the only depth cue a sky has.
    for step in 1..DECKS.len() {
        let (near, far) = (DECKS[step - 1], DECKS[step]);
        assert!(far[0] > near[0], "deck {step} is not higher than the last");
        assert!(
            far[3] > near[3],
            "deck {step} is not further off than the last"
        );
        assert!(
            far[2] >= near[2],
            "deck {step} is not thinner than the last"
        );
    }
}

#[test]
fn a_cloud_stands_taller_than_it_is_modelled() {
    // Two numbers that want to be one and must not be. [`TOWER`] is how far a cloud stands off
    // its deck, and it has to be large or the deck reads as a pattern on a ceiling. [`RELIEF`] is
    // how much height the light thinks it is walking over, and it has to be smaller or every
    // cloud is a cliff with one blazing face and one black one.
    //
    // Asked of the buffer rather than of the two constants, which is both the stronger question
    // and one the compiler cannot fold away: they share a slot, and the shader tells them apart
    // by which component of it they landed in.
    let u = compose(&Weather::default(), &LOOKS[0], (16, 9));
    let lowest = DECKS.iter().map(|deck| deck[0]).fold(f32::MAX, f32::min);
    assert!(
        u.puff[0] > u.puff[1],
        "a cloud modelled deeper than it stands has nothing to catch the light on"
    );
    // And the tower has to fit under the deck above it, or one deck grows through the next.
    assert!(
        u.puff[0] < lowest * 2.0 + u.puff[1],
        "the low deck grows into the middle one"
    );
}

#[test]
fn every_palette_is_a_stack_of_glazes() {
    for look in LOOKS {
        // A transmittance is a fraction of the light, so it lives in nought to one. Above one it
        // is a lamp rather than a glaze, and at nought it is a hole in the picture.
        for (name, tint) in glazes(look) {
            for (channel, value) in tint.iter().enumerate() {
                assert!(
                    (0.02..=1.0).contains(value),
                    "{}: {name} channel {channel} is {value}, which is not a transmittance",
                    look.name
                );
            }
            assert!(
                tint.iter().any(|value| *value < 0.98),
                "{}: {name} lets everything through, so laying it on does nothing",
                look.name
            );
        }

        // Nothing is brighter than the ground, because nothing in a glazed picture can be.
        let ground = look.ground.iter().copied().fold(f32::MAX, f32::min);
        assert!(
            ground > 0.9,
            "{}: the ground is not white enough to paint on",
            look.name
        );

        // Warm light against cool shadow. This is the whole signature: it is why a Parrish shadow
        // is a saturated blue rather than a grey, and a palette that fails it may be pretty but
        // it is not one of these.
        let warmth = |c: [f32; 3]| c[0] / c[2];
        assert!(
            warmth(look.cloud_light) > warmth(look.cloud_shadow) * 1.5,
            "{}: the light on a cloud is not warmer than the shadow on it",
            look.name
        );
        assert!(
            look.sky_high[2] > look.sky_high[0],
            "{}: the sky is glazed with something that is not blue",
            look.name
        );

        // The three silhouettes deepen as they come forward. A Parrish distance is a stack of
        // flat shapes, each one a shade deeper than the one behind it, and the rock across the
        // bottom is the deepest thing in the picture.
        assert!(
            luma(look.ridge_far) > luma(look.ridge_near),
            "{}: the far range is not paler than the near one",
            look.name
        );
        assert!(
            luma(look.ridge_near) > luma(look.ledge),
            "{}: the near range is not paler than the rock in front of it",
            look.name
        );
        let deepest = glazes(look)
            .iter()
            .map(|(_, tint)| luma(*tint))
            .fold(f32::MAX, f32::min);
        assert!(
            luma(look.ledge) <= deepest,
            "{}: something is laid on deeper than the rock",
            look.name
        );

        // Distance thins the paint rather than adding to it, so the coat that stands for
        // distance has to be paler than the near things seen through it. A distance glaze darker
        // than the hills would push the far range towards the front of the picture.
        assert!(
            luma(look.distance) > luma(look.ridge_far),
            "{}: distance darkens instead of thinning",
            look.name
        );

        // The sun.
        let sun = look.sun_direction();
        assert!(
            (length(sun) - 1.0).abs() < 1e-4,
            "{}: the sun is not a direction",
            look.name
        );
        assert!(sun[1] > 0.0, "{}: the sun is below the horizon", look.name);
        for (name, coats) in [
            ("sky", look.sky_depth),
            ("horizon", look.horizon_depth),
            ("glow", look.glow_depth),
        ] {
            assert!(
                coats > 0.0 && coats < 4.0,
                "{}: {name} is {coats} coats deep",
                look.name
            );
        }
    }
}

#[test]
fn the_uniforms_carry_what_the_shader_reads() {
    // A spot check on the one place the halves of the piece meet. Everything in the buffer is a
    // bare `vec4` with no names on the far side, so a field written into the wrong slot is a
    // silent and very confusing mistake. Three of these slots were rearranged once already, when
    // the tower and the relief stopped being the same number.
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
    assert_eq!(u.puff[0], TOWER, "how tall a cloud stands");
    assert_eq!(u.puff[1], RELIEF, "how much relief the light sees");
    assert_eq!(u.edge[3], CROWN, "crown height");
    assert_eq!(u.sky_high[0], look.sky_high[0], "the blue of the sky");
    assert_eq!(u.ledge[2], look.ledge[2], "the rock in front");
    assert_eq!(u.dial[0], look.sky_depth, "coats at the zenith");
    assert!(u.dial[3] > 0.0, "the glow has to fall off somehow");
    assert!(u.air[3] > 0.0 && u.air[3] < 0.5, "the edge squeeze");
    assert!(u.land[3] > 0.0, "the light has to wrap a little");
    assert!(u.world[0] > 1e6, "the planet is a planet");
    assert!(u.world[3] >= 256.0, "the sheet size, for the mip level");
    assert!(u.origin[3] > 0.0, "the field of view opens");
}
