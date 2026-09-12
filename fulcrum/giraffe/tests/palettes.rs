//! The palettes, held to the one rule they all follow: voice is hue and stance is lightness, so
//! the sharer who speaks is the lightest and warmest tone on the board and the silent taker is
//! the darkest. A reader who has learned one palette has learned all six.

use giraffe::game::Strategy;
use giraffe::look::{LOOKS, Palette, blend, linear, luminance, palette, to_linear, tone, warmth};

#[test]
fn every_palette_reads_the_same_way() {
    for look in &LOOKS {
        let name = look.name;
        let p = palette(look);
        let [quiet, jackal, giraffe, hawk] = p.tones;

        assert!(
            warmth(giraffe) > warmth(quiet),
            "{name}: the speaking sharer should be warmer than the silent one"
        );
        assert!(
            warmth(hawk) > warmth(jackal),
            "{name}: the speaking taker should be warmer than the silent one"
        );

        assert!(
            luminance(quiet) > luminance(jackal),
            "{name}: the silent sharer should be lighter than the silent taker"
        );
        assert!(
            luminance(giraffe) > luminance(hawk),
            "{name}: the speaking sharer should be lighter than the speaking taker"
        );

        for (i, t) in p.tones.iter().enumerate() {
            assert!(
                luminance(p.night) < luminance(*t),
                "{name}: the night should be darker than tone {i}"
            );
        }

        for (i, t) in p.tones.iter().enumerate().filter(|(i, _)| *i != 2) {
            assert!(
                luminance(giraffe) > luminance(*t),
                "{name}: the speaking sharer should be the lightest tone, and tone {i} is lighter"
            );
            assert!(
                warmth(giraffe) > warmth(*t),
                "{name}: the speaking sharer should be the warmest tone, and tone {i} is warmer"
            );
        }
    }
}

#[test]
fn the_names_are_distinct_and_the_first_is_dusk() {
    assert_eq!(LOOKS[0].name, "dusk");
    for (i, look) in LOOKS.iter().enumerate() {
        for other in LOOKS.iter().skip(i + 1) {
            assert_ne!(look.name, other.name, "two palettes share a name");
        }
    }
}

#[test]
fn blend_is_the_ends_at_the_ends() {
    let a = palette(&LOOKS[0]);
    let b = palette(&LOOKS[1]);
    assert_eq!(blend(&a, &b, 0.0), a);
    assert_eq!(blend(&a, &b, 1.0), b);

    let half = blend(&a, &b, 0.5);
    let fields = |p: &Palette| {
        let mut all = vec![p.night, p.lamp];
        all.extend(p.tones);
        all
    };
    for ((got, from), to) in fields(&half).iter().zip(fields(&a)).zip(fields(&b)) {
        for i in 0..3 {
            let want = (from[i] + to[i]) / 2.0;
            assert!(
                (got[i] - want).abs() < 1e-6,
                "the midpoint should be the midpoint: {} against {want}",
                got[i]
            );
        }
    }
}

#[test]
fn tone_indexes_by_strategy() {
    let p = palette(&LOOKS[0]);
    assert_eq!(tone(&p, Strategy::QUIET), p.tones[0]);
    assert_eq!(tone(&p, Strategy::JACKAL), p.tones[1]);
    assert_eq!(tone(&p, Strategy::GIRAFFE), p.tones[2]);
    assert_eq!(tone(&p, Strategy::HAWK), p.tones[3]);
}

#[test]
fn the_display_curve_is_monotone_and_pinned() {
    assert_eq!(linear(0.0), 0.0);
    assert!((linear(1.0) - 1.0).abs() < 1e-6, "white should stay white");
    assert_eq!(to_linear([0.0, 0.0, 0.0]), [0.0, 0.0, 0.0]);

    let mut previous = linear(0.0);
    for step in 1..=20 {
        let channel = step as f32 * 0.05;
        let got = linear(channel);
        assert!(got > previous, "the curve should climb at {channel}");
        previous = got;
    }
}
