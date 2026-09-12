//! The board held to what the module doc says: what an encounter pays, what a generation does
//! with that, and what the keys, the wheel and the pointer do to the terms.

use fulcrum::prelude::*;
use giraffe::game::{
    ACROSS, ARENA, BOUNTY, CELL, CONFLICT, COST, DIAL_STEP, DOWN, Dial, Field, GamePlugin, PERIOD,
    PLANT_RADIUS, Pace, SHARE, Strategy, TEMPTATION, Terms, cell_of, centre_of, payoff,
};

/// Close enough for two payoffs to be the same figure.
fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-6
}

/// A sown board, with nothing planted on it yet.
fn sown(seed: u64) -> Field {
    let mut field = Field::new();
    field.seed(&mut SimRng::seeded(seed));
    field
}

/// A sown board with a patch of giraffes in the middle, after `generations` under `terms`.
fn patch(seed: u64, terms: &Terms, radius: f32, generations: u32) -> Field {
    let mut field = sown(seed);
    field.plant(
        vec2(ACROSS as f32 / 2.0, DOWN as f32 / 2.0),
        radius,
        Strategy::GIRAFFE,
    );
    for _ in 0..generations {
        field.generation(terms);
    }
    field
}

/// A headless board, ready to tick.
fn board(seed: u64) -> Fulcrum {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed,
        ..Default::default()
    })
    .with_plugin(GamePlugin);
    app.run_startup();
    app
}

/// Tick `ticks` times with `key` held, or with nothing held for `None`, and let the key go at
/// the end. Anything the pointer is holding stays held.
fn hold(app: &mut Fulcrum, key: Option<Key>, ticks: u32) {
    for _ in 0..ticks {
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            if let Some(key) = key {
                input.push_key(key, true);
            }
            input.sample(|screen| screen);
        }
        app.tick();
    }
    if let Some(key) = key {
        let mut input = app.world_mut().resource_mut::<Input>();
        input.push_key(key, false);
        input.sample(|screen| screen);
    }
}

/// Press a key and let it go again: a tick each way.
fn tap(app: &mut Fulcrum, key: Key) {
    for down in [true, false] {
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            input.push_key(key, down);
            input.sample(|screen| screen);
        }
        app.tick();
    }
}

/// One tick with the pointer moved to `at`, and a button change if there is one. The identity
/// mapping means the pointer is already in world units.
fn point(app: &mut Fulcrum, at: Option<Vec2>, button: Option<(MouseButton, bool)>) {
    {
        let mut input = app.world_mut().resource_mut::<Input>();
        if let Some(at) = at {
            input.push_cursor(at);
        }
        if let Some((button, down)) = button {
            input.push_mouse_button(button, down);
        }
        input.sample(|screen| screen);
    }
    app.tick();
}

/// One tick with the wheel turned by `lines`.
fn wheel(app: &mut Fulcrum, lines: f32) {
    {
        let mut input = app.world_mut().resource_mut::<Input>();
        input.push_scroll(lines);
        input.sample(|screen| screen);
    }
    app.tick();
}

#[test]
fn the_payoffs_say_what_the_module_says() {
    let terms = Terms::default();
    let pay = |me, other| payoff(&terms, me, other);
    // Nowak and May, between two silent cells.
    assert!(near(pay(Strategy::QUIET, Strategy::QUIET), SHARE));
    assert!(near(
        pay(Strategy::JACKAL, Strategy::QUIET),
        terms.temptation
    ));
    assert!(near(pay(Strategy::QUIET, Strategy::JACKAL), 0.0));
    assert!(
        near(pay(Strategy::JACKAL, Strategy::JACKAL), 0.0),
        "two takers get nothing"
    );
    // A giraffe pays the cost whoever it is speaking to.
    assert!(near(pay(Strategy::GIRAFFE, Strategy::QUIET), SHARE - COST));
    assert!(near(pay(Strategy::GIRAFFE, Strategy::JACKAL), -COST));
    // And the silent party pays nothing for being spoken to.
    assert!(near(pay(Strategy::QUIET, Strategy::GIRAFFE), SHARE));

    // Needs never truly conflict: two speakers come away with the whole orange each, whatever
    // their stance, since there is nothing to take.
    let terms = Terms {
        conflict: 0.0,
        ..Terms::default()
    };
    let pay = |me, other| payoff(&terms, me, other);
    assert!(near(
        pay(Strategy::GIRAFFE, Strategy::GIRAFFE),
        SHARE + BOUNTY - COST
    ));
    assert!(
        near(
            pay(Strategy::HAWK, Strategy::GIRAFFE),
            SHARE + BOUNTY - COST
        ),
        "a hawk has nothing to take from a giraffe whose needs fit its own"
    );

    // Needs always truly conflict: the pair plays the dilemma it would have played anyway, and
    // the cost is all that speaking comes to.
    let terms = Terms {
        conflict: 1.0,
        ..Terms::default()
    };
    let pay = |me, other| payoff(&terms, me, other);
    assert!(near(
        pay(Strategy::GIRAFFE, Strategy::GIRAFFE),
        SHARE - COST
    ));
    assert!(near(
        pay(Strategy::HAWK, Strategy::GIRAFFE),
        terms.temptation - COST
    ));
}

#[test]
fn the_index_wraps_and_place_undoes_it() {
    assert_eq!(
        Field::index(-1, -1),
        Field::index(ACROSS as i32 - 1, DOWN as i32 - 1),
        "off the bottom left corner is the top right one"
    );
    assert_eq!(Field::index(ACROSS as i32, DOWN as i32), Field::index(0, 0));
    for (x, y) in [(0, 0), (1, 0), (0, 1), (17, 43), (159, 99)] {
        let i = Field::index(x, y);
        assert_eq!(Field::place(i), (x as u32, y as u32), "cell {x},{y}");
    }
}

#[test]
fn a_still_board_stays_still() {
    let mut field = Field::new();
    field.generation(&Terms::default());
    assert_eq!(
        field.count(Strategy::QUIET),
        field.len(),
        "sharers alone have nobody to learn from"
    );
    assert_eq!(field.generation, 1);
    assert!(
        field.age.iter().all(|&age| age == 1),
        "and every cell has held its strategy for that generation"
    );
}

#[test]
fn a_lone_taker_becomes_a_block() {
    // Nowak and May's first move. The taker takes from all eight of its neighbours, which is
    // more than any sharer near it can score, so the eight of them copy it.
    let mut field = Field::new();
    field.plant(vec2(80.5, 50.5), 0.5, Strategy::JACKAL);
    assert_eq!(field.count(Strategy::JACKAL), 1, "one taker to start with");
    field.generation(&Terms::default());
    assert_eq!(
        field.count(Strategy::JACKAL),
        9,
        "the taker and the eight that copied it"
    );
}

#[test]
fn plant_fills_a_disc_and_wraps() {
    let centre = vec2(40.5, 30.5);
    let mut field = Field::new();
    field.plant(centre, 3.0, Strategy::GIRAFFE);
    let mut wanted = 0;
    for y in 0..DOWN {
        for x in 0..ACROSS {
            let middle = vec2(x as f32 + 0.5, y as f32 + 0.5);
            if middle.distance(centre) <= 3.0 {
                wanted += 1;
            }
        }
    }
    assert_eq!(
        field.count(Strategy::GIRAFFE),
        wanted,
        "the disc is every cell whose middle is within the radius"
    );

    // A disc at the corner runs off the board and comes back on the far side.
    field.plant(vec2(0.5, 0.5), 2.0, Strategy::HAWK);
    assert_eq!(
        field.at(-1, -1),
        Strategy::HAWK,
        "the disc wraps round the torus"
    );
    assert_eq!(field.at(1, 0), Strategy::HAWK);
}

#[test]
fn the_board_starts_silent_with_takers_sprinkled() {
    let field = sown(7);
    assert_eq!(
        field.count(Strategy::GIRAFFE),
        0,
        "nobody speaks at the start"
    );
    assert_eq!(field.count(Strategy::HAWK), 0);
    let takers = field.count(Strategy::JACKAL);
    assert!(
        takers > field.len() * 7 / 100 && takers < field.len() * 13 / 100,
        "about a tenth of the board should take, got {takers} of {}",
        field.len()
    );
}

#[test]
fn a_silent_board_keeps_churning() {
    // The starting temptation is the one where a silent board neither freezes into sharers nor
    // is swept by takers: the two of them go on trading cells for as long as you watch.
    let terms = Terms::default();
    let mut field = sown(42);
    for _ in 0..300 {
        field.generation(&terms);
    }
    let sharing = field.sharing();
    assert!(
        (0.2..0.8).contains(&sharing),
        "neither side should have won, got {sharing} sharing"
    );
    let changed = field.age.iter().filter(|&&age| age == 0).count();
    assert!(
        changed > 500,
        "the board should still be moving, got {changed} cells changed"
    );
}

#[test]
fn giraffes_spread_when_needs_rarely_conflict() {
    let terms = Terms {
        conflict: 0.2,
        ..Terms::default()
    };
    let field = patch(42, &terms, 6.5, 150);
    let speaking = field.speaking();
    assert!(
        speaking > 0.5,
        "a planted patch should take most of the board, got {speaking} speaking"
    );
}

#[test]
fn giraffes_die_out_when_needs_always_conflict() {
    // There is nothing to find out, so the cost buys nothing: a speaker is a cell paying to say
    // something that changes no game.
    let terms = Terms {
        conflict: 1.0,
        ..Terms::default()
    };
    let field = patch(42, &terms, 6.5, 150);
    assert_eq!(field.speaking(), 0.0, "the patch should be gone");
}

#[test]
fn the_dial_is_monotone_for_a_planted_patch() {
    // The dial the whole piece turns on: the more often needs truly conflict, the less of the
    // board ends up speaking. The same board and the same patch each time, so the dial is the
    // one thing that differs.
    let mut last = f32::MAX;
    for conflict in [0.0, 0.2, 0.4, 0.6, 0.8, 1.0] {
        let terms = Terms {
            conflict,
            ..Terms::default()
        };
        let speaking = patch(42, &terms, 6.5, 80).speaking();
        assert!(
            speaking <= last + 1e-6,
            "conflict {conflict} left {speaking} speaking, up from {last}"
        );
        last = speaking;
    }
}

#[test]
fn space_pauses_the_generations() {
    let mut app = board(1);
    tap(&mut app, Key::Space);
    let stopped = app.world_mut().resource::<Field>().generation;
    hold(&mut app, None, 200);
    assert_eq!(
        app.world_mut().resource::<Field>().generation,
        stopped,
        "nothing is played while the board is stopped"
    );
    tap(&mut app, Key::Space);
    hold(&mut app, None, 200);
    assert!(
        app.world_mut().resource::<Field>().generation > stopped,
        "and it picks up again"
    );
}

#[test]
fn the_pace_keys_slide_the_period() {
    let mut app = board(1);
    hold(&mut app, Some(Key::Up), 60);
    let quicker = app.world_mut().resource::<Pace>().period;
    assert!(
        quicker < PERIOD.1,
        "a second of up should hurry the board, got {quicker}"
    );
    hold(&mut app, Some(Key::Down), 600);
    let slowest = app.world_mut().resource::<Pace>().period;
    assert!(
        near(slowest, PERIOD.2),
        "and down should stop at the slowest pace, got {slowest}"
    );
    hold(&mut app, Some(Key::Up), 3000);
    let quickest = app.world_mut().resource::<Pace>().period;
    assert!(
        near(quickest, PERIOD.0),
        "and up at the quickest, got {quickest}"
    );
}

#[test]
fn the_wheel_turns_the_dial_and_the_board_follows() {
    let mut app = board(1);
    wheel(&mut app, 2.0);
    hold(&mut app, None, 180);
    let dial = app.world_mut().resource::<Dial>().wanted;
    let conflict = app.world_mut().resource::<Terms>().conflict;
    assert!(
        near(dial, CONFLICT + 2.0 * DIAL_STEP),
        "two notches of the wheel is two steps, got {dial}"
    );
    assert!(
        (conflict - dial).abs() < 1e-3,
        "and three seconds is long enough for the board to catch up, got {conflict}"
    );

    // Far more wheel than there is dial: it stops at the end, and the board arrives there.
    wheel(&mut app, -100.0);
    hold(&mut app, None, 180);
    assert_eq!(app.world_mut().resource::<Dial>().wanted, 0.0);
    assert_eq!(
        app.world_mut().resource::<Terms>().conflict,
        0.0,
        "the board's figure snaps to the dial once it is near enough"
    );
}

#[test]
fn the_arrow_keys_move_the_temptation() {
    let mut app = board(1);
    hold(&mut app, Some(Key::Right), 60);
    let raised = app.world_mut().resource::<Terms>().temptation;
    assert!(
        raised > TEMPTATION.1,
        "right should make taking pay better, got {raised}"
    );
    hold(&mut app, Some(Key::Left), 3000);
    let least = app.world_mut().resource::<Terms>().temptation;
    assert!(
        near(least, TEMPTATION.0),
        "and left should stop where it is no longer a dilemma, got {least}"
    );
}

#[test]
fn r_sows_a_new_board() {
    let mut app = board(3);
    let before = app.world_mut().resource::<Field>().cells.clone();
    tap(&mut app, Key::R);
    let field = app.world_mut().resource::<Field>();
    assert!(field.cells != before, "a new board is a different board");
    assert_eq!(
        field.count(Strategy::GIRAFFE) + field.count(Strategy::HAWK),
        0,
        "and a silent one: the first speaker is yours to plant"
    );
}

#[test]
fn a_press_plants_giraffes_under_the_pointer() {
    let mut app = board(5);
    // Stopped, so that the generations do not take the patch apart while it is counted.
    tap(&mut app, Key::Space);
    point(
        &mut app,
        Some(vec2(4.0, 4.0)),
        Some((MouseButton::Left, true)),
    );
    {
        let field = app.world_mut().resource::<Field>();
        assert_eq!(field.at(80, 50), Strategy::GIRAFFE, "under the pointer");
        assert_eq!(
            field.count(Strategy::GIRAFFE),
            9,
            "a press plants a cell and its eight neighbours, at radius {}",
            PLANT_RADIUS.0
        );
    }

    // Held, the patch grows out to the wider radius.
    hold(&mut app, None, 180);
    let grown = app.world_mut().resource::<Field>().count(Strategy::GIRAFFE);
    assert!(
        (100..=160).contains(&grown),
        "three seconds of holding should fill a disc of radius {}, got {grown} cells",
        PLANT_RADIUS.1
    );
    point(&mut app, None, Some((MouseButton::Left, false)));

    // The right button plants the other kind.
    point(&mut app, None, Some((MouseButton::Right, true)));
    assert_eq!(
        app.world_mut().resource::<Field>().at(80, 50),
        Strategy::JACKAL,
        "the right button plants takers"
    );
}

#[test]
fn off_the_board_nothing_is_planted() {
    let mut app = board(5);
    tap(&mut app, Key::Space);
    point(
        &mut app,
        Some(vec2(-5000.0, -5000.0)),
        Some((MouseButton::Left, true)),
    );
    hold(&mut app, None, 10);
    assert_eq!(
        app.world_mut().resource::<Field>().count(Strategy::GIRAFFE),
        0,
        "a press away from the board plants nothing"
    );
}

#[test]
fn the_pointer_maps_to_the_board() {
    let at = cell_of(vec2(4.0, 4.0));
    assert!(
        near(at.x, 80.5) && near(at.y, 50.5),
        "the middle of the board: {at:?}"
    );
    let round_trip = cell_of(centre_of(80, 50));
    assert!(
        near(round_trip.x, 80.5) && near(round_trip.y, 50.5),
        "a cell's middle maps back to that cell: {round_trip:?}"
    );
    assert_eq!(
        centre_of(0, 0),
        vec2(-ARENA.x / 2.0 + CELL / 2.0, -ARENA.y / 2.0 + CELL / 2.0),
        "cell zero is the bottom left corner"
    );
}
