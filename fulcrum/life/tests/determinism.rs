//! Determinism gate: same seed and same input twice, bit-identical both times.
//!
//! There is more here to drift than in most of these pieces, because almost everything is on a
//! key. The rule can change under a running field, the resolution can change the field's shape
//! out from under the pattern on it, the mouse can put cells anywhere, the pace decides how
//! many generations a tick runs, and the field sows itself again when it has stopped going
//! anywhere. Any one of those reading the frame's wall-clock delta instead of the fixed one, or
//! reading the window instead of the command channel, would make the piece unreplayable.
//!
//! So the script drives all of it, and the digest covers the whole field rather than a summary
//! of it: every cell, its age, and the trail behind it.

use fulcrum::prelude::*;
use life::game::{Board, Dials, GamePlugin};

/// Scripted input: walk the rules, change the resolution, hold and let go, draw with the
/// mouse, sow it again, change the boundary, and run it at three different paces.
fn script(input: &mut Input, tick: u32) {
    match tick {
        // Faster, then a rule along, then two more.
        10..=40 => input.push_key(Key::Up, true),
        41 => input.push_key(Key::Up, false),
        50 => input.push_key(Key::M, true),
        52 => input.push_key(Key::M, false),
        60 => input.push_key(Key::Tab, true),
        61 => input.push_key(Key::Tab, false),
        // Finer cells, which reshapes the field around whatever is on it.
        70..=80 => input.push_key(Key::X, true),
        81 => input.push_key(Key::X, false),
        // Hold it, step it by hand, let it go.
        90 => input.push_key(Key::Space, true),
        91 => input.push_key(Key::Space, false),
        100 => input.push_key(Key::S, true),
        104 => input.push_key(Key::S, false),
        110 => input.push_key(Key::Space, true),
        111 => input.push_key(Key::Space, false),
        // Draw a stroke, then rub some of it out.
        120 => {
            input.push_cursor(vec2(300.0, 220.0));
            input.push_mouse_button(MouseButton::Left, true);
        }
        121..=140 => input.push_cursor(vec2(300.0 + (tick - 120) as f32 * 9.0, 220.0)),
        141 => input.push_mouse_button(MouseButton::Left, false),
        145 => {
            input.push_cursor(vec2(360.0, 220.0));
            input.push_mouse_button(MouseButton::Right, true);
        }
        150 => input.push_mouse_button(MouseButton::Right, false),
        // The boundary, a coarser field, and a fresh sowing of a named pattern.
        160 => input.push_key(Key::T, true),
        161 => input.push_key(Key::T, false),
        170..=178 => input.push_key(Key::Z, true),
        179 => input.push_key(Key::Z, false),
        190 => input.push_key(Key::Digit6, true),
        191 => input.push_key(Key::Digit6, false),
        // Slower, and back to the rule before this one.
        200..=230 => input.push_key(Key::Down, true),
        231 => input.push_key(Key::Down, false),
        240 => input.push_key(Key::N, true),
        242 => input.push_key(Key::N, false),
        _ => {}
    }
}

/// The window sizes the run is put through, and the ticks they arrive on. This is what going
/// fullscreen and back looks like from inside the simulation: an ordinary resize on the
/// replayable command channel, and nothing else at all.
const RESIZES: &[(u32, Vec2)] = &[
    (30, Vec2::new(1280.0, 800.0)),
    (130, Vec2::new(1920.0, 1080.0)),
    (210, Vec2::new(1000.0, 900.0)),
];

/// Fold the whole field, and the dials that decide what happens to it, into one number.
fn digest(app: &mut Fulcrum) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut eat = |value: u64| {
        hash ^= value;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    };

    {
        let board = app.world().resource::<Board>();
        eat(u64::from(board.width));
        eat(u64::from(board.height));
        eat(board.generation);
        eat(u64::from(board.population));
        eat(u64::from(board.births));
        eat(u64::from(board.deaths));
        eat(u64::from(board.settled));
        eat(board.period.map_or(u64::MAX, u64::from));
        // Every cell, and both of the histories kept beside it. A trail that decayed by a
        // frame-dependent amount would show up here and nowhere else.
        for (cell, (age, trail)) in board
            .cells
            .iter()
            .zip(board.age.iter().zip(board.trail.iter()))
        {
            eat(u64::from(*cell) | u64::from(*age) << 8 | u64::from(*trail) << 16);
        }
    }

    let dials = app.world().resource::<Dials>();
    eat(dials.rule as u64);
    eat(dials.size as u64);
    eat(u64::from(dials.pace.to_bits()));
    eat(u64::from(dials.running));
    eat(u64::from(dials.wrap));
    eat(u64::from(dials.restart));
    hash
}

/// Run the piece headless for `ticks`, taking a digest every so often.
fn run(seed: u64, ticks: u32) -> Vec<u64> {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed,
        ..Default::default()
    })
    .with_plugin(GamePlugin);

    app.run_startup();
    let mut marks = Vec::new();
    for tick in 0..ticks {
        if let Some((_, size)) = RESIZES.iter().find(|(at, _)| *at == tick) {
            let mut outbox = app.world_mut().resource_mut::<CommandOutbox>();
            outbox.send(
                life::game::RESIZE_COMMAND,
                life::game::window_payload(*size),
            );
        }
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            script(&mut input, tick);
            input.sample(|screen| screen);
        }
        app.tick();
        if tick % 20 == 0 {
            marks.push(digest(&mut app));
        }
    }
    marks.push(digest(&mut app));
    marks
}

#[test]
fn determinism_same_input_same_field() {
    let first = run(42, 260);
    let second = run(42, 260);
    assert_eq!(first, second, "same input must give the same field");
}

#[test]
fn a_different_seed_gives_a_different_field() {
    // The sowing is the only thing the seed touches, but it touches the whole field, so two
    // seeds that agreed everywhere would mean the RNG was not being consulted at all.
    let first = run(42, 60);
    let second = run(1_000_003, 60);
    assert_ne!(first, second, "the seed should decide what is sown");
}

#[test]
fn the_field_actually_changes() {
    // A gate that passes because nothing ever happens is not a gate.
    let marks = run(42, 260);
    assert!(
        marks.windows(2).any(|pair| pair[0] != pair[1]),
        "nothing on the field ever moved"
    );
}

#[test]
fn a_resize_does_not_stop_it() {
    // What F11 does, from the simulation's side: the window changes size and the field carries
    // straight on, with the generation count unbroken and the pattern still on it.
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed: 5,
        ..Default::default()
    })
    .with_plugin(GamePlugin);
    app.run_startup();
    for _ in 0..40 {
        app.world_mut()
            .resource_mut::<Input>()
            .sample(|screen| screen);
        app.tick();
    }
    let before = {
        let board = app.world().resource::<Board>();
        (board.generation, board.population)
    };
    assert!(before.0 > 0 && before.1 > 0, "it should be running by now");

    app.world_mut().resource_mut::<CommandOutbox>().send(
        life::game::RESIZE_COMMAND,
        life::game::window_payload(vec2(2560.0, 1440.0)),
    );
    app.world_mut()
        .resource_mut::<Input>()
        .sample(|screen| screen);
    app.tick();

    // The generation count carries across rather than starting again: the field was reshaped,
    // not sown.
    let across = {
        let board = app.world().resource::<Board>();
        assert!(
            board.width > 200,
            "the field should have taken the new window's size"
        );
        assert!(board.population > 0, "with the pattern still on it");
        board.generation
    };
    assert!(
        across >= before.0,
        "the generation count was reset by a resize"
    );

    // And it goes on running. The pace is fifteen generations a second against a sixty-hertz
    // tick, so a handful of ticks is what one generation takes.
    for _ in 0..12 {
        app.world_mut()
            .resource_mut::<Input>()
            .sample(|screen| screen);
        app.tick();
    }
    let board = app.world().resource::<Board>();
    assert!(
        board.generation > across,
        "the field should have gone on running across the resize"
    );
}
