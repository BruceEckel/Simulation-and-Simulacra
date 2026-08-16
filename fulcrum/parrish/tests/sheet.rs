//! The sheet held to the three things the shader assumes about it: that it tiles, that it comes
//! with a mip chain, and that every channel fills the byte it is stored in.

use parrish::field::{SHEET_SIZE, gradient_noise, sheet};

#[test]
fn the_sheet_tiles() {
    // Read with repeat addressing over a sky ninety kilometres across, so a seam would be a
    // ruled line drawn through the weather every few tiles.
    for cells in [2u32, 3, 5, 13] {
        for step in 0..8 {
            let t = step as f32 / 8.0;
            let inside = gradient_noise([0.0, t], cells, 3);
            let wrapped = gradient_noise([1.0, t], cells, 3);
            assert!(
                (inside - wrapped).abs() < 1e-5,
                "{cells} cells does not wrap across: {inside} against {wrapped}"
            );
            let above = gradient_noise([t, 0.0], cells, 3);
            let below = gradient_noise([t, 1.0], cells, 3);
            assert!(
                (above - below).abs() < 1e-5,
                "{cells} cells does not wrap down"
            );
        }
    }
}

#[test]
fn the_ring_the_land_is_read_off_closes() {
    // The hills and the near rock are profiles read around a circle laid on the sheet rather
    // than off an angle, so that there is no seam behind you where the angle wraps. That only
    // works while the sheet tiles in both directions at once, which is a stronger claim than
    // either edge wrapping on its own: walking the circle crosses both.
    let ring = |turn: f32, radius: f32| {
        let angle = turn * std::f32::consts::TAU;
        gradient_noise(
            [angle.cos() * radius + 0.5, angle.sin() * radius + 0.5],
            3,
            9,
        )
    };
    for radius in [0.21f32, 0.37] {
        let start = ring(0.0, radius);
        let round = ring(1.0, radius);
        assert!(
            (start - round).abs() < 1e-5,
            "a ring of radius {radius} does not close: {start} against {round}"
        );
    }
}

#[test]
fn the_mip_chain_goes_all_the_way_down() {
    let sheet = sheet(3);
    assert_eq!(sheet.size, SHEET_SIZE);
    // Nine halvings from 512 to 1, and the level itself makes ten.
    assert_eq!(sheet.levels.len(), 10);
    let mut size = SHEET_SIZE;
    for level in &sheet.levels {
        assert_eq!(level.len(), (size as usize).pow(2) * 4, "level of {size}");
        size = (size / 2).max(1);
    }
}

#[test]
fn every_channel_fills_its_byte() {
    // Every threshold in the shader is a threshold against these numbers, and this one has two
    // of them stacked: the coverage threshold, and the squeeze that sharpens the edge. A channel
    // that only used the middle of its range would come out of the squeeze as a solid overcast
    // with no holes in it anywhere.
    let sheet = sheet(5);
    for channel in 0..4 {
        let mut low = 255u8;
        let mut high = 0u8;
        for y in (0..SHEET_SIZE).step_by(3) {
            for x in (0..SHEET_SIZE).step_by(3) {
                let value = sheet.at(x, y, channel);
                low = low.min(value);
                high = high.max(value);
            }
        }
        assert!(low < 20, "channel {channel} never gets dark: {low}");
        assert!(high > 235, "channel {channel} never gets bright: {high}");
    }
}

#[test]
fn the_clouds_are_not_the_same_field_twice() {
    // The two cloud channels are added together and scrolled apart, which only makes weather if
    // they are actually different fields. If they ever came out correlated, the sum would just be
    // a brighter copy of one of them and the sky would slide instead of boil.
    let sheet = sheet(5);
    let mut agree = 0u32;
    let mut total = 0u32;
    for y in (0..SHEET_SIZE).step_by(7) {
        for x in (0..SHEET_SIZE).step_by(7) {
            let first = sheet.at(x, y, 0) as i32 - 128;
            let second = sheet.at(x, y, 1) as i32 - 128;
            agree += u32::from(first.signum() == second.signum());
            total += 1;
        }
    }
    let share = agree as f32 / total as f32;
    assert!(
        (0.35..0.65).contains(&share),
        "the two cloud fields agree {:.0}% of the time",
        share * 100.0
    );
}

#[test]
fn the_weather_map_is_the_smoothest_channel() {
    // It decides which districts of the sky are cloudy, over tens of kilometres. Detail in it is
    // worse than useless: it would cut holes in individual clouds rather than between them.
    let sheet = sheet(11);
    let roughness = |channel: usize| {
        let mut total = 0u64;
        let mut count = 0u64;
        for y in (0..SHEET_SIZE).step_by(5) {
            for x in (0..SHEET_SIZE).step_by(5) {
                let here = i32::from(sheet.at(x, y, channel));
                let next = i32::from(sheet.at(x + 1, y, channel));
                total += (here - next).unsigned_abs() as u64;
                count += 1;
            }
        }
        total as f64 / count as f64
    };
    let weather = roughness(2);
    for channel in [0usize, 1, 3] {
        assert!(
            weather < roughness(channel),
            "the weather map is rougher than channel {channel}"
        );
    }
}
