//! The rule, and the four things about it worth holding to: it moves grains without losing any,
//! it does not care what order they arrive in, it finds the same level from above and from below,
//! and the avalanches it makes have no typical size.

use avalanche::game::{
    self, CELLS, GamePlugin, Ledger, Sizes, Slide, TALL, THRESHOLD, Table, WIDE,
};
use fulcrum::prelude::*;

/// Topple until nothing is unstable, and hand back the totals.
fn relax(table: &mut Table) -> (u32, u32) {
    let mut topples = 0;
    let mut lost = 0;
    loop {
        let (toppled, gone) = table.wave();
        if toppled == 0 {
            return (topples, lost);
        }
        topples += toppled;
        lost += gone;
    }
}

/// Drop one grain in a random cell and let the table settle, over and over.
fn drive(table: &mut Table, drops: u32, seed: u64) -> u64 {
    let mut rng = SimRng::seeded(seed);
    let mut lost = 0u64;
    for _ in 0..drops {
        let index = rng.range_i32(0..CELLS as i32) as usize;
        table.add(index, 1);
        lost += relax(table).1 as u64;
    }
    lost
}

#[test]
fn a_full_cell_gives_one_to_each_neighbour() {
    let mut table = Table::default();
    let middle = Table::index(80, 60).unwrap();
    table.add(middle, THRESHOLD);
    let (toppled, lost) = relax(&mut table);

    assert_eq!(toppled, 1, "one cell should have gone");
    assert_eq!(lost, 0, "nothing should have left the table");
    assert_eq!(table.at(80, 60), 0, "and it should have kept nothing");
    for (across, up) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        assert_eq!(table.at(80 + across, 60 + up), 1, "each neighbour gets one");
    }
    assert_eq!(table.at(81, 61), 0, "and the diagonals get nothing");
}

#[test]
fn grains_that_go_off_the_edge_are_the_ones_that_are_lost() {
    let mut table = Table::default();
    let corner = Table::index(0, 0).unwrap();
    table.add(corner, THRESHOLD);
    let (_, lost) = relax(&mut table);
    assert_eq!(lost, 2, "the corner has two neighbours and two edges");
    assert_eq!(table.total(), 2);
}

#[test]
fn nothing_is_left_unstable_when_the_table_is_quiet() {
    let mut table = Table::default();
    drive(&mut table, 4_000, 7);
    assert!(!table.busy());
    for index in 0..CELLS {
        assert!(
            table.grains[index] < THRESHOLD,
            "cell {index} still holds {}",
            table.grains[index]
        );
    }
}

#[test]
fn every_grain_is_accounted_for() {
    // Conservation, which is the one thing a rule that moves things around must never get wrong.
    let mut table = Table::default();
    let mut rng = SimRng::seeded(11);
    let mut dropped = 0u64;
    let mut lost = 0u64;
    for _ in 0..6_000 {
        let index = rng.range_i32(0..CELLS as i32) as usize;
        let count = rng.range_i32(1..4) as u16;
        table.add(index, count);
        dropped += count as u64;
        lost += relax(&mut table).1 as u64;
    }
    assert_eq!(
        dropped,
        table.total() + lost,
        "{dropped} dropped, {} on the table, {lost} off the edge",
        table.total()
    );
}

#[test]
fn the_pile_does_not_care_what_order_the_grains_arrive_in() {
    // The surprising theorem about this rule, and the reason it is called abelian: given the same
    // grains, the table ends up in exactly the same state whether they are dropped one at a time,
    // in the opposite order, or all at once before anything is allowed to topple. The pictures on
    // the way are completely different. The final table is not.
    let mut rng = SimRng::seeded(13);
    let drops: Vec<(usize, u16)> = (0..600)
        .map(|_| {
            (
                rng.range_i32(0..CELLS as i32) as usize,
                rng.range_i32(1..5) as u16,
            )
        })
        .collect();

    let mut forwards = Table::default();
    for &(index, count) in &drops {
        forwards.add(index, count);
        relax(&mut forwards);
    }

    let mut backwards = Table::default();
    for &(index, count) in drops.iter().rev() {
        backwards.add(index, count);
        relax(&mut backwards);
    }

    let mut all_at_once = Table::default();
    for &(index, count) in &drops {
        all_at_once.add(index, count);
    }
    relax(&mut all_at_once);

    assert_eq!(
        forwards.grains, backwards.grains,
        "the same grains in the opposite order should leave the same table"
    );
    assert_eq!(
        forwards.grains, all_at_once.grains,
        "and dropping them all before anything topples should too"
    );
}

#[test]
fn it_finds_the_same_level_from_above_and_from_below() {
    // The point of the piece. Nothing in the rule mentions a height, and no matter where the table
    // starts it walks to the same one and stays there.
    let mut empty = Table::default();
    drive(&mut empty, 60_000, 17);

    let mut loaded = Table::default();
    loaded.fill(3);
    drive(&mut loaded, 60_000, 19);

    let from_below = empty.mean();
    let from_above = loaded.mean();
    for (name, mean) in [("from empty", from_below), ("from full", from_above)] {
        assert!(
            (2.0..2.25).contains(&mean),
            "{name} settled at {mean}, which is not where this pile settles"
        );
    }
    assert!(
        (from_below - from_above).abs() < 0.05,
        "{from_below} from below and {from_above} from above should be the same place"
    );
}

#[test]
fn the_avalanches_have_no_typical_size() {
    // What a power law looks like from the inside: sizes spread over many octaves, with the small
    // ones commonest and no bump anywhere that would mark a usual size.
    let mut table = Table::default();
    table.fill(2);
    drive(&mut table, 20_000, 23);

    let mut sizes = Sizes::default();
    let mut biggest = 0;
    let mut rng = SimRng::seeded(29);
    for _ in 0..20_000 {
        let index = rng.range_i32(0..CELLS as i32) as usize;
        table.add(index, 1);
        let (topples, _) = relax(&mut table);
        if topples > 0 {
            sizes.bins[game::bin_of(topples)] += 1;
            biggest = biggest.max(topples);
        }
    }
    let populated = sizes.bins.iter().filter(|&&count| count > 0).count();
    assert!(
        populated >= 8,
        "sizes only spread over {populated} bins, which is not much of a spread"
    );
    assert!(
        biggest > 1_000,
        "the biggest avalanche was {biggest} topples, so nothing much happened"
    );
    // And the claim the piece actually makes: on log axes the sizes fall along a straight line
    // with a negative slope. Fitted here in floating point, which is fine in a test and is why
    // the simulation itself does not do it.
    //
    // Note what is deliberately *not* asserted: that small avalanches outnumber large ones.
    // Counting whole bins, they do not. With an exponent near one, the avalanches of thirty-two
    // topples and up outnumber the ones of five and under, because there are so many more sizes
    // up there. The density is what falls away, and the density is what the line is fitted to.
    let points: Vec<(f32, f32)> = sizes
        .bins
        .iter()
        .enumerate()
        .filter(|&(_, &count)| count >= 4)
        .map(|(bin, &count)| {
            let (low, high) = game::bin_span(bin);
            let middle = ((low as f32) * (high as f32)).sqrt();
            (middle.log2(), (count as f32 / (high - low) as f32).log2())
        })
        .collect();
    assert!(points.len() >= 6, "only {} bins to fit", points.len());
    let n = points.len() as f32;
    let mean_x = points.iter().map(|(x, _)| x).sum::<f32>() / n;
    let mean_y = points.iter().map(|(_, y)| y).sum::<f32>() / n;
    let top: f32 = points
        .iter()
        .map(|(x, y)| (x - mean_x) * (y - mean_y))
        .sum();
    let bottom: f32 = points
        .iter()
        .map(|(x, _)| (x - mean_x) * (x - mean_x))
        .sum();
    let slope = top / bottom;
    assert!(
        (-2.2..-0.4).contains(&slope),
        "the sizes fitted a slope of {slope}, which is not a power law with a tail"
    );
}

#[test]
fn the_bins_cover_every_size_exactly_once() {
    // Integer binning, so the histogram is part of what the determinism gate can check.
    for size in 1u32..5000 {
        let bin = game::bin_of(size);
        let (low, high) = game::bin_span(bin);
        assert!(
            low <= size && size < high,
            "size {size} landed in bin {bin}, which covers {low}..{high}"
        );
    }
    assert_eq!(game::bin_of(1), 0);
    assert!(game::bin_of(1_000_000) < avalanche::game::BINS);
}

#[test]
fn a_cell_can_be_found_under_a_point_and_a_point_under_a_cell() {
    for (column, row) in [
        (0, 0),
        (WIDE as i32 - 1, TALL as i32 - 1),
        (80, 60),
        (3, 117),
    ] {
        let middle = game::cell_at(column, row);
        assert_eq!(
            game::cell_under(middle),
            Some((column, row)),
            "the middle of a cell should be in it"
        );
    }
    let outside = game::cell_at(0, 0) - vec2(game::CELL, game::CELL);
    assert_eq!(game::cell_under(outside), None, "off the table is off it");
}

/// A table driven by the real simulation for `ticks`, with an optional bit of input.
fn running(seed: u64, ticks: u32, mut script: impl FnMut(&mut Input, u32)) -> Fulcrum {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed,
        window_size: (game::ARENA.x as u32, game::ARENA.y as u32),
        ..Default::default()
    })
    .with_plugin(GamePlugin);
    app.run_startup();
    for tick in 0..ticks {
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            script(&mut input, tick);
            input.sample(|screen| screen);
        }
        app.tick();
    }
    app
}

#[test]
fn the_table_feeds_itself_and_measures_what_happens() {
    let mut app = running(31, 60 * 30, |_, _| {});
    let ledger = *app.world_mut().resource::<Ledger>();
    let sizes = app.world_mut().resource::<Sizes>().clone();
    assert!(ledger.poured > 1_000, "the feed should have been dropping");
    assert!(ledger.measured > 100, "and measuring what happened");
    assert_eq!(
        sizes.counted(),
        ledger.measured,
        "every measured avalanche should be in the histogram"
    );
    assert_eq!(ledger.disturbed, 0, "nobody touched it");
}

#[test]
fn pouring_on_a_moving_pile_spoils_the_measurement() {
    // Anything you do while an avalanche is running is welcome and is not evidence, and the
    // histogram is careful about the difference.
    let mut app = running(37, 60 * 20, |input, tick| {
        // Pour on the middle of the table, without a pause, which is exactly what a measurement
        // must not be made from.
        input.push_cursor(game::cell_at(80, 60));
        if tick > 60 {
            input.push_mouse_button(MouseButton::Left, true);
        }
    });
    let ledger = *app.world_mut().resource::<Ledger>();
    assert!(ledger.disturbed > 0, "the pouring should have spoiled some");
    let sizes = app.world_mut().resource::<Sizes>().clone();
    assert_eq!(
        sizes.counted(),
        ledger.measured,
        "and only the clean ones should be counted"
    );
}

#[test]
fn stillness_stops_everything() {
    let mut app = running(41, 60 * 12, |input, tick| {
        if tick == 60 * 12 - 10 {
            input.push_key(Key::Space, true);
        }
        if tick == 60 * 12 - 9 {
            input.push_key(Key::Space, false);
        }
    });
    let before = app.world_mut().resource::<Table>().grains.clone();
    for _ in 0..120 {
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            input.sample(|screen| screen);
        }
        app.tick();
    }
    assert_eq!(
        before,
        app.world_mut().resource::<Table>().grains,
        "a still table should not move"
    );
}

#[test]
fn sweeping_empties_it_and_loading_fills_it() {
    let mut app = running(43, 60 * 10, |input, tick| {
        if tick == 300 {
            input.push_key(Key::R, true);
        }
        if tick == 301 {
            input.push_key(Key::R, false);
        }
    });
    // Swept at tick 300, then fed for another six seconds, so it is nearly empty rather than
    // exactly empty.
    let mean = app.world_mut().resource::<Table>().mean();
    assert!(
        mean < 0.2,
        "sweeping should have emptied it, not left {mean}"
    );
    assert!(!app.world_mut().resource::<Slide>().running);

    let mut app = running(43, 60 * 10, |input, tick| {
        if tick == 590 {
            input.push_key(Key::F, true);
        }
        if tick == 591 {
            input.push_key(Key::F, false);
        }
    });
    let mean = app.world_mut().resource::<Table>().mean();
    assert!(mean > 2.5, "loading should have filled it, not left {mean}");
}
