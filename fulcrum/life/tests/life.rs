//! The rule itself, held to what is known about it.
//!
//! Two kinds of test in here, and the second is the one that matters most.
//!
//! The first kind checks the famous facts: a blinker has period two, a block sits still, a
//! glider is one cell down and one cell right after four generations, an acorn is still going
//! after a thousand. If any of those come out wrong then whatever this is, it is not Life.
//!
//! The second cross-checks all three neighbour counters against an obvious one. The fast paths
//! in `game.rs` — the row sums, the summed-area table over a padded copy — exist for speed and
//! are the kind of code that can be wrong in a way that still looks plausible on screen. So
//! every rule in the table, on both boundaries, is run against a naive count written the way
//! the definition reads, and the two must agree cell for cell.

use life::game::{Board, Start};
use life::rules::{Counts, Family, RULES, Rule, Shape};

// ---------------------------------------------------------------------------------------
// a naive rule, written the way the definition reads
// ---------------------------------------------------------------------------------------

/// One generation, counting every neighbourhood one cell at a time. Slow on purpose: there is
/// nothing in here to be clever about and therefore nothing to be wrong about.
fn naive(rule: &Rule, cells: &[u8], width: i32, height: i32, wrap: bool) -> Vec<u8> {
    let live = |x: i32, y: i32| -> u32 {
        let (x, y) = if wrap {
            (x.rem_euclid(width), y.rem_euclid(height))
        } else if x < 0 || y < 0 || x >= width || y >= height {
            return 0;
        } else {
            (x, y)
        };
        u32::from(cells[(y * width + x) as usize] == 1)
    };

    let mut out = vec![0u8; cells.len()];
    for y in 0..height {
        for x in 0..width {
            let mut count = 0;
            match rule.shape {
                Shape::Moore(radius) => {
                    let radius = radius as i32;
                    for offset_y in -radius..=radius {
                        for offset_x in -radius..=radius {
                            if offset_x == 0 && offset_y == 0 && !rule.centre {
                                continue;
                            }
                            count += live(x + offset_x, y + offset_y);
                        }
                    }
                }
                Shape::VonNeumann => {
                    for (offset_x, offset_y) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                        count += live(x + offset_x, y + offset_y);
                    }
                    if rule.centre {
                        count += live(x, y);
                    }
                }
            }
            let index = (y * width + x) as usize;
            out[index] = match cells[index] {
                0 => u8::from(rule.birth.holds(count)),
                1 => {
                    if rule.survive.holds(count) {
                        1
                    } else if rule.states > 2 {
                        2
                    } else {
                        0
                    }
                }
                dying => {
                    let older = u32::from(dying) + 1;
                    if older >= rule.states { 0 } else { older as u8 }
                }
            };
        }
    }
    out
}

/// A tiny xorshift, so the fields these tests run on are the same fields every time without
/// dragging the engine's RNG into a test of the rule.
struct Dice(u64);

impl Dice {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// A field with every one of the rule's states somewhere on it, so that the dying states are
/// exercised as well as the live ones.
fn muddle(rule: &Rule, width: u32, height: u32, seed: u64) -> Board {
    let mut board = Board::new(width, height);
    let mut dice = Dice(seed);
    for y in 0..height as i32 {
        for x in 0..width as i32 {
            let roll = dice.next();
            // Mostly empty, a third alive, and a sprinkling of whatever else the rule has.
            let state = match roll % 3 {
                0 => 1,
                1 => 0,
                _ => (roll >> 8) as u32 % rule.states,
            };
            board.set(x, y, state as u8);
        }
    }
    board
}

/// Draw a pattern on an empty board from a picture of it.
fn draw(board: &mut Board, left: i32, top: i32, picture: &[&str]) {
    for (row, line) in picture.iter().enumerate() {
        for (column, mark) in line.chars().enumerate() {
            if mark != '.' {
                board.set(left + column as i32, top + row as i32, 1);
            }
        }
    }
}

/// Where the live cells are, as a sorted list, so two fields can be compared by their shape
/// rather than by where they happen to sit.
fn live_cells(board: &Board) -> Vec<(i32, i32)> {
    let mut found = Vec::new();
    for y in 0..board.height as i32 {
        for x in 0..board.width as i32 {
            if board.at(x, y) == 1 {
                found.push((x, y));
            }
        }
    }
    found
}

/// Conway's Life, which is the first rule in the table.
fn conway() -> &'static Rule {
    RULES
        .iter()
        .find(|rule| rule.name == "Life")
        .expect("the table has Life in it")
}

/// A rule by name, for the tests that want a particular one.
fn named(name: &str) -> &'static Rule {
    RULES
        .iter()
        .find(|rule| rule.name == name)
        .unwrap_or_else(|| panic!("no rule called {name}"))
}

// ---------------------------------------------------------------------------------------
// the famous facts
// ---------------------------------------------------------------------------------------

#[test]
fn a_block_sits_still() {
    let mut board = Board::new(16, 16);
    draw(&mut board, 5, 5, &["XX", "XX"]);
    let before = live_cells(&board);
    for _ in 0..20 {
        board.step(conway(), false);
    }
    assert_eq!(live_cells(&board), before, "a block is a still life");
}

#[test]
fn a_blinker_has_period_two() {
    let mut board = Board::new(16, 16);
    draw(&mut board, 5, 5, &["XXX"]);
    let flat = live_cells(&board);

    board.step(conway(), false);
    let upright = live_cells(&board);
    assert_ne!(upright, flat, "a blinker moves");
    assert_eq!(
        upright,
        vec![(6, 4), (6, 5), (6, 6)],
        "and it stands up on its middle"
    );

    board.step(conway(), false);
    assert_eq!(live_cells(&board), flat, "and lies back down again");
}

#[test]
fn a_glider_moves_one_cell_diagonally_every_four_generations() {
    let mut board = Board::new(40, 40);
    draw(&mut board, 4, 4, &[".X.", "..X", "XXX"]);
    let before = live_cells(&board);

    for _ in 0..4 {
        board.step(conway(), false);
    }
    let after = live_cells(&board);
    let moved: Vec<(i32, i32)> = before.iter().map(|(x, y)| (x + 1, y + 1)).collect();
    assert_eq!(
        after, moved,
        "four generations should be the same glider, one cell down and one cell right"
    );
    assert_eq!(board.population, 5, "and still five cells");
}

#[test]
fn the_r_pentomino_is_still_going_after_a_thousand_generations() {
    // The famous one: five cells that take eleven hundred and three generations to settle.
    let mut board = Board::new(200, 200);
    let mut rng = fulcrum::prelude::SimRng::seeded(1);
    board.sow(conway(), Start::Pentomino, &mut rng);
    assert_eq!(board.population, 5, "an R-pentomino is five cells");

    for _ in 0..1_000 {
        board.step(conway(), false);
    }
    assert!(
        board.population > 50,
        "an R-pentomino should still be busy at a thousand generations, not {}",
        board.population
    );
    assert!(
        board.period.is_none(),
        "and it should not have settled yet: found period {:?}",
        board.period
    );
}

#[test]
fn the_diehard_dies() {
    // Seven cells that leave nothing at all behind them, after a hundred and thirty
    // generations. A famously good way to catch a boundary that is quietly wrong.
    let mut board = Board::new(120, 120);
    let mut rng = fulcrum::prelude::SimRng::seeded(1);
    board.sow(conway(), Start::Diehard, &mut rng);
    assert_eq!(board.population, 7, "a diehard is seven cells");

    for _ in 0..129 {
        board.step(conway(), false);
    }
    assert!(
        board.population > 0,
        "the diehard should still be alive one generation before the end"
    );
    board.step(conway(), false);
    assert_eq!(
        board.population, 0,
        "and gone completely at a hundred and thirty"
    );
}

#[test]
fn the_glider_gun_grows_without_limit() {
    let mut board = Board::new(200, 200);
    let mut rng = fulcrum::prelude::SimRng::seeded(1);
    board.sow(conway(), Start::Gun, &mut rng);
    assert_eq!(board.population, 36, "Gosper's gun is thirty-six cells");

    // Every thirty generations it emits a glider, so the population climbs by five each time
    // until the first one reaches the wall.
    for _ in 0..120 {
        board.step(conway(), false);
    }
    assert!(
        board.population >= 36 + 4 * 5,
        "four gliders should have been fired by a hundred and twenty, leaving at least {} \
         cells, not {}",
        36 + 4 * 5,
        board.population
    );
}

#[test]
fn a_wall_and_a_torus_are_not_the_same_field() {
    // A glider heading for the corner: against a wall it is destroyed, on a torus it comes out
    // of the opposite side and carries on forever.
    let make = || {
        let mut board = Board::new(24, 24);
        draw(&mut board, 18, 18, &[".X.", "..X", "XXX"]);
        board
    };

    let mut walled = make();
    let mut torus = make();
    for _ in 0..200 {
        walled.step(conway(), false);
        torus.step(conway(), true);
    }
    assert_eq!(
        walled.population, 4,
        "a glider run into a corner leaves a block behind it"
    );
    assert_eq!(walled.period, Some(1), "and a block does not move");
    assert_eq!(
        torus.population, 5,
        "the same glider on a torus is still a glider"
    );
    assert_eq!(
        torus.period, None,
        "and still travelling: it comes back round every ninety-six generations, which is \
         further back than the field remembers"
    );
}

#[test]
fn a_dying_cell_is_not_a_neighbour() {
    // The whole of what Generations adds. Three cells in a row under Brian's Brain: nothing
    // survives, so all three stop being alive at once, and the four cells that saw exactly two
    // of them are born above and below the ends.
    let brain = named("Brian's Brain");
    assert_eq!(brain.states, 3);

    let mut board = Board::new(16, 16);
    draw(&mut board, 5, 5, &["XXX"]);
    board.step(brain, false);
    assert_eq!(board.at(6, 5), 2, "the row itself is now dying, not empty");
    assert_eq!(
        live_cells(&board),
        vec![(5, 4), (7, 4), (5, 6), (7, 6)],
        "and four cells are born where exactly two of the row could be seen"
    );

    board.step(brain, false);
    assert_eq!(board.at(6, 5), 0, "a dying cell is empty on the next step");

    // And directly, which is the one thing that separates Generations from Life: a cell with
    // exactly two dying neighbours and no live ones is not born, because a dying cell is in
    // the way but is not a neighbour.
    let mut sparse = Board::new(16, 16);
    sparse.set(5, 5, 2);
    sparse.set(7, 5, 2);
    sparse.step(brain, false);
    assert_eq!(
        sparse.population, 0,
        "two dying cells are not two neighbours"
    );
}

#[test]
fn nothing_survives_seeds() {
    let seeds = named("Seeds");
    let mut board = Board::new(32, 32);
    draw(&mut board, 10, 10, &["XX", "XX"]);
    let mut previous: Vec<(i32, i32)> = live_cells(&board);
    for _ in 0..12 {
        board.step(seeds, false);
        let now = live_cells(&board);
        for cell in &now {
            assert!(
                !previous.contains(cell),
                "no cell may be alive two generations running under Seeds, but {cell:?} was"
            );
        }
        previous = now;
    }
}

#[test]
fn a_symmetric_field_stays_symmetric() {
    // Every rule here treats the four reflections alike, so this holds for all of them. It is
    // also a sharp test of the counters: an asymmetry in an edge case shows up at once.
    let mut rng = fulcrum::prelude::SimRng::seeded(7);
    for rule in RULES {
        let mut board = Board::new(64, 48);
        board.sow(rule, Start::Symmetry, &mut rng);
        for _ in 0..6 {
            board.step(rule, true);
        }
        let (width, height) = (board.width as i32, board.height as i32);
        for y in 0..height {
            for x in 0..width {
                assert_eq!(
                    board.at(x, y),
                    board.at(width - 1 - x, y),
                    "{} lost its left-right symmetry at ({x}, {y})",
                    rule.name
                );
                assert_eq!(
                    board.at(x, y),
                    board.at(x, height - 1 - y),
                    "{} lost its top-bottom symmetry at ({x}, {y})",
                    rule.name
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// the counters, against an obvious one
// ---------------------------------------------------------------------------------------

#[test]
fn every_rule_agrees_with_a_naive_count() {
    for (index, rule) in RULES.iter().enumerate() {
        for wrap in [true, false] {
            // Wide enough that a radius-ten neighbourhood is not simply the whole field, and
            // not square, so a row and a column being swapped somewhere would show.
            let mut board = muddle(rule, 52, 37, 0x5eed_0000 + index as u64);
            let mut reference = board.cells.clone();
            for generation in 0..3 {
                reference = naive(
                    rule,
                    &reference,
                    board.width as i32,
                    board.height as i32,
                    wrap,
                );
                board.step(rule, wrap);
                assert_eq!(
                    board.cells,
                    reference,
                    "{} ({}) disagreed with a naive count on generation {} with wrap={wrap}",
                    rule.name,
                    rule.rulestring(),
                    generation + 1,
                );
            }
        }
    }
}

#[test]
fn the_population_is_the_live_cells() {
    for rule in RULES {
        let mut board = muddle(rule, 33, 21, 99);
        for _ in 0..4 {
            board.step(rule, true);
            let counted = board.cells.iter().filter(|cell| **cell == 1).count() as u32;
            assert_eq!(
                board.population, counted,
                "{} reported the wrong population",
                rule.name
            );
        }
    }
}

// ---------------------------------------------------------------------------------------
// the field around the rule
// ---------------------------------------------------------------------------------------

#[test]
fn a_still_field_reports_that_it_is_still() {
    let mut board = Board::new(24, 24);
    draw(&mut board, 5, 5, &["XX", "XX"]);
    for _ in 0..60 {
        board.step(conway(), false);
    }
    assert_eq!(board.period, Some(1), "a block is still");

    let mut blinking = Board::new(24, 24);
    draw(&mut blinking, 5, 5, &["XXX"]);
    for _ in 0..60 {
        blinking.step(conway(), false);
    }
    assert_eq!(blinking.period, Some(2), "a blinker has period two");
}

#[test]
fn a_running_field_does_not_report_a_period() {
    let mut board = Board::new(160, 160);
    let mut rng = fulcrum::prelude::SimRng::seeded(3);
    board.sow(conway(), Start::Acorn, &mut rng);
    for _ in 0..400 {
        board.step(conway(), false);
    }
    assert_eq!(
        board.period, None,
        "an acorn at four hundred generations has not settled"
    );
}

#[test]
fn resizing_keeps_the_pattern() {
    // Changing the resolution must not destroy what is on the field: the pattern is copied
    // across with its middle on the new middle, at its own size.
    let mut board = Board::new(40, 40);
    draw(&mut board, 18, 18, &[".X.", "..X", "XXX"]);
    let before = live_cells(&board);

    board.reshape(120, 100);
    let after = live_cells(&board);
    assert_eq!(after.len(), before.len(), "no cells may be lost");

    // Every cell moved by the same amount, which is what "copied, not resampled" means.
    let shift = (after[0].0 - before[0].0, after[0].1 - before[0].1);
    for (was, now) in before.iter().zip(&after) {
        assert_eq!(
            (now.0 - was.0, now.1 - was.1),
            shift,
            "the pattern was distorted rather than moved"
        );
    }

    // And it is still a glider afterwards.
    for _ in 0..4 {
        board.step(conway(), false);
    }
    let moved: Vec<(i32, i32)> = after.iter().map(|(x, y)| (x + 1, y + 1)).collect();
    assert_eq!(live_cells(&board), moved);
}

#[test]
fn drawing_puts_cells_where_the_mouse_is() {
    let mut board = Board::new(64, 64);
    board.brush((20, 20), 3, true);
    assert!(board.at(20, 20) == 1, "the middle of the brush is filled");
    assert!(board.at(23, 20) == 1, "and so is its edge");
    assert!(board.at(25, 20) == 0, "but not past its edge");
    let filled = board.population;
    assert!(filled > 20, "a disc of radius three is more than a dot");

    board.stroke((20, 20), (40, 20), 3, true);
    assert!(
        board.population > filled,
        "a stroke should be longer than a dab"
    );
    assert_eq!(board.at(30, 20), 1, "and joined up in the middle");

    board.stroke((20, 20), (40, 20), 3, false);
    assert_eq!(
        board.at(30, 20),
        0,
        "the right button takes them away again"
    );
}

#[test]
fn every_start_puts_something_on_the_field() {
    let mut rng = fulcrum::prelude::SimRng::seeded(11);
    for start in life::game::STARTS {
        let mut board = Board::new(96, 72);
        board.sow(conway(), *start, &mut rng);
        assert_eq!(board.generation, 0, "sowing starts the count again");
        if *start == Start::Empty {
            assert_eq!(board.population, 0, "the empty start is empty");
        } else {
            assert!(
                board.population > 0,
                "{} put nothing on the field",
                start.name()
            );
        }
    }
}

// ---------------------------------------------------------------------------------------
// the table itself
// ---------------------------------------------------------------------------------------

#[test]
fn the_rules_are_grouped_by_family_and_named_once_each() {
    let mut seen: Vec<&str> = Vec::new();
    for rule in RULES {
        assert!(!seen.contains(&rule.name), "two rules called {}", rule.name);
        seen.push(rule.name);
    }

    // Grouped, because the family key walks the table expecting them to be.
    let mut families: Vec<Family> = Vec::new();
    for rule in RULES {
        if families.last() != Some(&rule.family) {
            assert!(
                !families.contains(&rule.family),
                "{} is not next to the rest of its family",
                rule.name
            );
            families.push(rule.family);
        }
    }
    assert_eq!(families.len(), 3, "three families");
}

#[test]
fn there_are_as_many_rules_as_the_writing_says() {
    // The prose says "forty-four" in several places, and prose does not recompile. This is the
    // thing that notices when a rule is added and one of them is left saying otherwise.
    let count = |family| RULES.iter().filter(|rule| rule.family == family).count();
    assert_eq!(count(Family::LifeLike), 22, "Life-like");
    assert_eq!(count(Family::Generations), 15, "Generations");
    assert_eq!(count(Family::LargerThanLife), 7, "Larger than Life");
    assert_eq!(RULES.len(), 44, "forty-four altogether");
}

#[test]
fn the_family_key_lands_on_the_first_rule_of_the_next_family() {
    let mut index = 0;
    let mut visited = Vec::new();
    for _ in 0..3 {
        index = life::rules::next_family(index);
        assert!(
            index == 0 || RULES[index - 1].family != RULES[index].family,
            "landed part-way into a family at {index}"
        );
        visited.push(RULES[index].family);
    }
    assert_eq!(visited[0], Family::Generations);
    assert_eq!(visited[1], Family::LargerThanLife);
    assert_eq!(visited[2], Family::LifeLike, "and round again");
}

#[test]
fn every_rule_is_written_the_way_its_family_writes_it() {
    for rule in RULES {
        let written = rule.rulestring();
        match rule.family {
            Family::LifeLike => {
                assert!(written.starts_with("B3/S23") || written.starts_with('B'));
                assert!(
                    written.matches('/').count() == 1,
                    "{}: {written}",
                    rule.name
                );
                assert_eq!(rule.states, 2, "{} is not two-state", rule.name);
                assert_eq!(rule.shape, Shape::Moore(1), "{} is not eight", rule.name);
                assert!(!rule.centre, "{} counts itself", rule.name);
            }
            Family::Generations => {
                assert!(written.ends_with(&format!("/{}", rule.states)), "{written}");
                assert!(rule.states > 2, "{} has no dying states", rule.name);
            }
            Family::LargerThanLife => {
                assert!(written.starts_with('R'), "{written}");
                assert!(
                    matches!(rule.birth, Counts::Range(..)),
                    "{} is written in bands",
                    rule.name
                );
            }
        }
        // Nothing may ask a question its neighbourhood cannot answer, and nothing may want a
        // count the field cannot hold in the sixteen bits it is counted into.
        assert!(rule.ceiling() < u16::MAX as u32, "{}", rule.name);
        assert!(rule.states >= 2 && rule.states <= 255, "{}", rule.name);
    }
    assert_eq!(conway().rulestring(), "B3/S23");
    assert_eq!(named("Brian's Brain").rulestring(), "B2/S/3");
    assert_eq!(named("Bugs").rulestring(), "R5,C0,M1,S34..58,B34..45,NM");
}
