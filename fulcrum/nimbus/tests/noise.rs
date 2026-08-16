//! The volumes held to the two things the shader assumes about them: that they tile, and that
//! the field they carry lands where the coverage dial can reach it.

use nimbus::noise::{DETAIL_SIZE, SHAPE_SIZE, Volume, detail_volume, perlin, shape_volume, worley};

/// Read the shape volume the way the shader reads it: the base field, and the three worley
/// channels stacked into one figure that erodes it.
fn base_field(volume: &Volume, x: u32, y: u32, z: u32) -> f32 {
    let channel = |c: usize| volume.at(x, y, z, c) as f32 / 255.0;
    let stacked = channel(1) * 0.625 + channel(2) * 0.25 + channel(3) * 0.125;
    let low = stacked - 1.0;
    ((channel(0) - low) / (1.0 - low)).clamp(0.0, 1.0)
}

#[test]
fn the_volumes_tile() {
    // Sampled with repeat addressing over tens of kilometres of sky, so a seam would be a
    // visible ruled line across the weather every few hundred metres.
    for cells in [4u32, 8, 16] {
        for step in 0..8 {
            let t = step as f32 / 8.0;
            let inside = worley([0.0, t, t * 0.5], cells, 3);
            let wrapped = worley([1.0, t, t * 0.5], cells, 3);
            assert!(
                (inside - wrapped).abs() < 1e-5,
                "worley at {cells} cells does not wrap: {inside} against {wrapped}"
            );
            let smooth = perlin([t, 0.0, t * 0.25], cells, 5);
            let over = perlin([t, 1.0, t * 0.25], cells, 5);
            assert!(
                (smooth - over).abs() < 1e-5,
                "perlin at {cells} cells does not wrap"
            );
        }
    }
}

#[test]
fn the_volumes_are_the_size_they_say() {
    let shape = shape_volume(1);
    let detail = detail_volume(1);
    assert_eq!(shape.size, SHAPE_SIZE);
    assert_eq!(detail.size, DETAIL_SIZE);
    assert_eq!(shape.data.len(), (SHAPE_SIZE as usize).pow(3) * 4);
    assert_eq!(detail.data.len(), (DETAIL_SIZE as usize).pow(3) * 4);
}

#[test]
fn the_coverage_dial_can_reach_the_field() {
    // The shader decides whether a point is cloud by asking whether the base field is above
    // `1 - coverage`. That only works if the field actually spends its time near there: a field
    // that lived between 0.9 and 1.0 would go from clear sky to overcast across two clicks of
    // the dial, and one that lived below 0.2 would never grow a cloud at all.
    let volume = shape_volume(7);
    let mut samples = Vec::new();
    let step = SHAPE_SIZE / 32;
    for z in (0..SHAPE_SIZE).step_by(step as usize) {
        for y in (0..SHAPE_SIZE).step_by(step as usize) {
            for x in (0..SHAPE_SIZE).step_by(step as usize) {
                samples.push(base_field(&volume, x, y, z));
            }
        }
    }
    samples.sort_by(|a, b| a.partial_cmp(b).expect("no NaNs in the field"));
    let at = |share: f32| samples[(share * (samples.len() - 1) as f32) as usize];
    println!(
        "base field percentiles: 1% {:.3}  10% {:.3}  50% {:.3}  90% {:.3}  99% {:.3}",
        at(0.01),
        at(0.10),
        at(0.50),
        at(0.90),
        at(0.99)
    );
    // The useful window: the dial runs over roughly 0.2 to 0.8, so the field has to have real
    // weight on both sides of the middle of that.
    assert!(at(0.10) < 0.55, "the field is too dense to ever clear");
    assert!(at(0.90) > 0.45, "the field is too thin to ever cloud over");
}
