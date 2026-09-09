//! The uniforms, held to the layout the shader reads.
//!
//! The shader and the Rust struct each say where every number is, and nothing checks that
//! they agree except this. A field in the wrong slot does not fail to compile: it draws the
//! wrong picture, quietly.

use pond::game::Water;
use pond::look::{LOOKS, blend, sample};
use pond::screen::{
    HEIGHT_TINT, LANTERN_RADIUS, REFRACTION, SLOPE_GAIN, SPECULAR, Scene, Uniforms, VIGNETTE,
    compose,
};

/// A scene with nothing special about it.
fn scene() -> Scene {
    Scene {
        lantern: [300.0, 200.0],
        glow: 0.9,
        stops: LOOKS[0].stops,
        drift: 0.25,
        time: 12.5,
        scale: 1.0,
    }
}

#[test]
fn the_uniforms_are_eleven_vec4s_with_no_padding() {
    assert_eq!(std::mem::size_of::<Uniforms>(), 11 * 16);
    assert_eq!(std::mem::align_of::<Uniforms>(), 4);
}

#[test]
fn every_number_is_where_the_shader_reads_it() {
    let water = Water::new(fulcrum::prelude::vec2(1280.0, 800.0));
    let u = compose(&water, &scene(), (1280, 800));
    assert_eq!(u.field, [640.0, 400.0, 2.0, 640.0]);
    assert_eq!(u.screen, [1280.0, 800.0, 12.5, 0.25]);
    assert_eq!(u.lantern, [300.0, 200.0, 0.9, LANTERN_RADIUS]);
    assert_eq!(u.light[3], SPECULAR);
    assert_eq!(u.tune, [SLOPE_GAIN, REFRACTION, HEIGHT_TINT, VIGNETTE]);
    for (stop, wanted) in u.stops.iter().zip(LOOKS[0].stops.iter()) {
        assert_eq!(&stop[..3], wanted);
        assert_eq!(stop[3], 1.0);
    }
    assert_eq!(u.glow[3], 1.0);
}

#[test]
fn a_hidpi_display_gets_the_same_lantern() {
    let water = Water::new(fulcrum::prelude::vec2(2560.0, 1600.0));
    let mut hidpi = scene();
    hidpi.scale = 2.0;
    let u = compose(&water, &hidpi, (2560, 1600));
    assert_eq!(u.lantern[3], LANTERN_RADIUS * 2.0);
    assert_eq!(u.tune[1], REFRACTION * 2.0);
}

#[test]
fn the_stride_is_what_is_uploaded() {
    // A width that is not a multiple of the row alignment: the texture is the stride wide.
    let water = Water::new(fulcrum::prelude::vec2(1000.0, 700.0));
    let u = compose(&water, &scene(), (1000, 700));
    assert_eq!(u.field[0], 500.0);
    assert_eq!(u.field[3], 512.0);
    assert_eq!(water.height.len(), 512 * 350);
}

#[test]
fn palettes_blend_and_sample_sanely() {
    let same = blend(&LOOKS[0], &LOOKS[0], 0.5);
    assert_eq!(same, LOOKS[0].stops);
    let half = blend(&LOOKS[0], &LOOKS[1], 0.5);
    for ((got, from), to) in half.iter().zip(&LOOKS[0].stops).zip(&LOOKS[1].stops) {
        for ((channel, a), b) in got.iter().zip(from).zip(to) {
            let want = (a + b) / 2.0;
            assert!((channel - want).abs() < 1e-6);
        }
    }
    assert_eq!(sample(&LOOKS[0].stops, 0.0), LOOKS[0].stops[0]);
    assert_eq!(sample(&LOOKS[0].stops, 1.0), LOOKS[0].stops[4]);
    assert_eq!(sample(&LOOKS[0].stops, 5.0), LOOKS[0].stops[4]);
}

#[test]
fn every_palette_climbs_from_dark_to_light() {
    for look in &LOOKS {
        let brightness = |rgb: &[f32; 3]| 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
        for pair in look.stops.windows(2) {
            assert!(
                brightness(&pair[1]) > brightness(&pair[0]),
                "{}: the stops should get brighter as they go",
                look.name
            );
        }
        assert!(
            brightness(&look.stops[0]) < 0.12,
            "{} starts too bright",
            look.name
        );
        assert!(
            brightness(&look.stops[4]) > 0.6,
            "{} ends too dark",
            look.name
        );
    }
}
