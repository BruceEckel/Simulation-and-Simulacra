//! The piece held to its own claims.
//!
//! The claims: every pixel of the window carries a material the palette knows about; the
//! horizon is a real division, with nothing of the desert above it and nothing of the sky
//! below; a cloud is grown to stand inside its own bitmap, with a flat base and open sky all
//! round it, however far its anvil tries to spread; and a cloud's shadow travels with it.

use fulcrum::prelude::SimRng;
use thunderhead::game::{
    Backdrop, CLOUD_BANDS, CLOUD_FIRST, DEPTHS, Field, GROUND_BANDS, GROUND_FIRST, HORIZON, INK,
    MATERIALS, MESA_FIRST, SHADOW_FIRST, SKY_BANDS, SKY_FIRST, STONE, SUN, Sky, TIER_COUNT, TIERS,
    build_backdrop, compose, forge_now, place, populate, tier_size,
};
use thunderhead::look::{LOOKS, lut};

/// A whole picture, composed the way the binary composes one.
fn picture(width: u32, height: u32, seed: u64) -> (Field, Backdrop, Sky) {
    let mut rng = SimRng::seeded(seed);
    let mut sky = Sky {
        seed: rng.u64(),
        ..Default::default()
    };
    populate(&mut sky, height as f32 * HORIZON, &mut rng);
    place(&mut sky, width as f32);
    let backdrop = build_backdrop(width, height, sky.seed);
    let mut field = Field {
        width,
        height,
        cells: vec![0; (width as usize) * (height as usize)],
    };
    compose(&mut field, &backdrop, &sky);
    (field, backdrop, sky)
}

#[test]
fn every_pixel_carries_a_material_the_palette_knows() {
    let (field, ..) = picture(640, 400, 11);
    let highest = field.cells.iter().copied().max().expect("a picture");
    assert!(
        (highest as usize) < MATERIALS,
        "material {highest} is past the end of the table"
    );
    // And the table really does answer for every one of them, which is the contract between
    // the two halves of the piece.
    let table = lut(&LOOKS[0]);
    for (material, colour) in table.iter().enumerate().take(MATERIALS) {
        assert_eq!(colour[3], 255, "material {material} has no colour");
    }
}

#[test]
fn the_horizon_divides_the_picture() {
    let (_, backdrop, _) = picture(640, 400, 5);
    let sky_end = SKY_FIRST + SKY_BANDS;
    for (index, &cell) in backdrop.cells.iter().enumerate() {
        let row = index as u32 / backdrop.width;
        let above = row < backdrop.horizon;
        if cell == INK {
            continue; // the line is drawn on both halves
        }
        if above {
            let sky = (SKY_FIRST..sky_end).contains(&cell) || cell == SUN;
            let rock = (MESA_FIRST..MESA_FIRST + 2).contains(&cell);
            assert!(sky || rock, "material {cell} is above the horizon");
        } else {
            let sand = (GROUND_FIRST..GROUND_FIRST + GROUND_BANDS).contains(&cell);
            let shade = (SHADOW_FIRST..SHADOW_FIRST + GROUND_BANDS).contains(&cell);
            assert!(
                sand || shade || cell == STONE,
                "material {cell} is below it"
            );
        }
    }
}

#[test]
fn a_cloud_stands_inside_its_own_bitmap() {
    // The whole reason the puffs are measured and fitted after they are laid out. Without it
    // an anvil spreads past the edge of the bitmap and comes back sawn off square.
    let mut rng = SimRng::seeded(3);
    for tier in 0..TIER_COUNT {
        for _ in 0..6 {
            let anvil = forge_now(0, tier, tier_size(tier, 900.0), &mut rng);
            let (w, h) = (anvil.width as usize, anvil.height as usize);
            for x in 0..w {
                assert_eq!(anvil.cells[x], 0, "cloud touches the top edge");
                assert_eq!(anvil.cells[(h - 1) * w + x], 0, "cloud touches the bottom");
            }
            for y in 0..h {
                assert_eq!(anvil.cells[y * w], 0, "cloud touches the left edge");
                assert_eq!(anvil.cells[y * w + w - 1], 0, "cloud touches the right");
            }
        }
    }
}

#[test]
fn a_cloud_has_a_flat_base_and_is_drawn_all_over() {
    let mut rng = SimRng::seeded(9);
    let anvil = forge_now(0, 2, tier_size(2, 900.0), &mut rng);
    let (w, h) = (anvil.width as usize, anvil.height as usize);
    let slack = (anvil.height as f32 * 0.02) as u32;

    let mut lowest = 0u32;
    let mut bands = [0usize; CLOUD_BANDS as usize + 2];
    for y in 0..h {
        for x in 0..w {
            let cell = anvil.cells[y * w + x];
            bands[cell as usize] += 1;
            if cell != 0 {
                lowest = lowest.max(y as u32);
            }
        }
    }
    assert!(
        lowest <= anvil.base + slack,
        "cloud hangs {lowest} below its base at {}",
        anvil.base
    );
    // Every band in use, and the line drawn: a cloud shaded into two of its ten tones would
    // mean the tone range and the palette had come apart.
    for (band, &drawn) in bands.iter().enumerate().skip(1).take(CLOUD_BANDS as usize) {
        assert!(drawn > 0, "band {band} was never drawn");
    }
    assert!(bands[CLOUD_BANDS as usize + 1] > 0, "the cloud has no line");
}

#[test]
fn the_shadow_travels_with_the_cloud() {
    let (mut field, backdrop, mut sky) = picture(900, 600, 21);
    let shadowed = |field: &Field| {
        field
            .cells
            .iter()
            .filter(|&&cell| (SHADOW_FIRST..SHADOW_FIRST + GROUND_BANDS).contains(&cell))
            .count()
    };
    assert!(shadowed(&field) > 0, "no cloud is casting anything");

    let before: Vec<u8> = field.cells.clone();
    for drifter in &mut sky.drifters {
        drifter.x += 90.0;
    }
    compose(&mut field, &backdrop, &sky);
    assert_ne!(before, field.cells, "moving the clouds changed nothing");
    assert!(shadowed(&field) > 0, "the shadows went out");
}

#[test]
fn distance_is_drawn_as_distance() {
    // The three tiers have to stay ordered in every way that reads as depth, since between
    // them they are the only depth cue the piece has.
    assert!(DEPTHS.windows(2).all(|pair| pair[0] <= pair[1]));
    for tier in 1..TIER_COUNT {
        let (near, far) = (TIERS[tier], TIERS[tier - 1]);
        assert!(near.rise > far.rise, "a nearer cloud must be bigger");
        assert!(near.speed > far.speed, "a nearer cloud must cross faster");
        assert!(near.lift > far.lift, "a nearer cloud must ride higher");
        assert!(near.cast > far.cast, "its shadow must fall nearer too");
    }
    // And a bigger sky must grow bigger clouds, since that is what keeps the piece composed
    // the same on a laptop and on a wall.
    for tier in 0..TIER_COUNT {
        let small = tier_size(tier, 600.0);
        let large = tier_size(tier, 1200.0);
        assert!(large.0 > small.0 && large.1 > small.1);
    }
}

#[test]
fn the_clouds_are_the_subject() {
    // A composition claim, and worth a test because every knob in the piece can quietly break
    // it: at the size things are drawn, clouds should cover a good part of the sky.
    let (field, backdrop, _) = picture(1600, 1000, 4);
    let clouds = field
        .cells
        .iter()
        .filter(|&&cell| cell >= CLOUD_FIRST)
        .count();
    let sky = (backdrop.horizon as usize) * (field.width as usize);
    let share = clouds as f32 / sky as f32;
    assert!(
        (0.2..0.85).contains(&share),
        "clouds cover {:.0}% of the sky",
        share * 100.0
    );
}
