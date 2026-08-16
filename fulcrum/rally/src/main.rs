//! Rally, windowed: everything you can see. The court lives in `game.rs` and stays
//! renderer-free; this binary dresses its entities and draws the readout.
//!
//! `cargo run -p rally` — space pauses, up/down change the speed (0 resets it), holding B or
//! P pours in balls or paddles, R starts over.

use fulcrum::prelude::*;
use rally::game::{
    self, Ball, Census, Court, DEFAULT_COURT, GamePlugin, Paddle, Paused, Speed, Stats, ball_size,
    paddle_shape,
};
use simulacra_assets::assets;

/// Readout text height in world units. The built-in pixel font is sharpest at multiples of 8.
const READOUT_SIZE: f32 = 8.0;
/// Gap between the readout and the court's top-left corner.
const READOUT_MARGIN: f32 = 12.0;

/// The shared white texture every rectangle here is a tinted copy of.
#[derive(Resource)]
struct White(Handle<Texture>);

/// Marks the statistics readout.
#[derive(Component)]
struct Readout;

/// Load the texture and put up the readout. The court's entities are dressed by
/// [`dress_new`], which also covers everything the simulation adds later.
fn setup(mut commands: Commands, mut assets: AssetLoader) {
    commands.insert_resource(White(assets.load("white.png")));
    commands.spawn((
        Readout,
        // The built-in pixel font is sharpest at multiples of 8.
        Text::new("").with_size(READOUT_SIZE).with_z(10.0),
        Transform2D::from_xy(
            -DEFAULT_COURT.x / 2.0 + READOUT_MARGIN,
            DEFAULT_COURT.y / 2.0 - READOUT_MARGIN,
        ),
    ));
}

/// Keep window, camera, and court in step. Same split as `boids`: the zoom is cosmetic and
/// applied here, while the resize itself is a command the simulation applies on its own tick.
fn fit_window(
    window: Res<WindowInfo>,
    court: Res<Court>,
    mut camera: ResMut<Camera2D>,
    mut outbox: ResMut<CommandOutbox>,
    mut requested: Local<Option<Vec2>>,
    mut readouts: Query<&mut Transform2D, With<Readout>>,
) {
    let size = vec2(window.width as f32, window.height as f32);
    if size.x < 1.0 || size.y < 1.0 {
        return; // minimized
    }
    let wanted = game::court_for_window(size);
    if wanted != court.0 && *requested != Some(wanted) {
        outbox.send(game::COURT_COMMAND, game::court_payload(wanted));
        *requested = Some(wanted);
    }
    camera.zoom = (size.x / court.0.x).max(size.y / court.0.y);
    for mut readout in &mut readouts {
        readout.translation = vec2(
            -court.0.x / 2.0 + READOUT_MARGIN,
            court.0.y / 2.0 - READOUT_MARGIN,
        );
    }
}

/// Give every new ball and paddle a sprite. Runs each frame, so whatever the schedule adds is
/// dressed the moment it appears.
fn dress_new(
    mut commands: Commands,
    white: Option<Res<White>>,
    balls: Query<Entity, (With<Ball>, Without<Sprite>)>,
    paddles: Query<Entity, (With<Paddle>, Without<Sprite>)>,
) {
    let Some(white) = white else { return };
    for ball in &balls {
        // Size comes from `style_balls`, which re-reads it every frame as the court fills.
        commands
            .entity(ball)
            .try_insert(Sprite::new(white.0).with_z(2.0));
    }
    for entity in &paddles {
        // Size and color both come from `shape_paddles`, which re-reads them every frame as
        // the walls fill up.
        commands
            .entity(entity)
            .try_insert(Sprite::new(white.0).with_z(1.0));
    }
}

/// Paddles shrink as they multiply, so their sprites are re-sized every frame from the same
/// geometry function the simulation collides against — one source of truth, and no chance of
/// a paddle looking bigger than the thing that actually returns balls.
///
/// Each wall is also its own spectrum, spread bottom to top by slot, so a filling wall reads
/// as a gradient and you can see at a glance which stretch of it a ball is heading for.
fn shape_paddles(
    mut paddles: Query<(&Paddle, &mut Sprite)>,
    court: Res<Court>,
    census: Res<Census>,
) {
    for (paddle, mut sprite) in &mut paddles {
        let shape = paddle_shape(court.0, *census, paddle.side, paddle.slot);
        sprite.custom_size = Some(vec2(shape.half_thickness * 2.0, shape.half_length * 2.0));
        let span = census.on(paddle.side).max(1) as f32;
        sprite.color = spectrum(paddle.slot as f32 / span);
    }
}

/// Balls shrink as the court fills, and the set is spread across the color spectrum by spawn
/// order. The spread is over the *current* population, so the court always shows a complete
/// spectrum rather than creeping along it as balls are added — the first ball is red whether
/// it has one companion or forty.
fn style_balls(mut balls: Query<(&Ball, &mut Sprite)>, census: Res<Census>) {
    let size = Vec2::splat(ball_size(census.balls));
    let span = census.balls.max(1) as f32;
    for (ball, mut sprite) in &mut balls {
        sprite.custom_size = Some(size);
        sprite.color = spectrum(ball.index as f32 / span);
    }
}

/// Hue (in turns) to a saturated, bright color. A small HSV conversion beats a hand-picked
/// palette here: the population has no ceiling at all, and evenly spaced
/// hues stay distinguishable at any count.
fn spectrum(hue: f32) -> Color {
    let position = hue.rem_euclid(1.0) * 6.0;
    let sector = position.floor();
    let offset = position - sector;
    // Full value, held-back saturation: pastel enough to read against the dark court without
    // the neon edge full saturation gives.
    let (value, saturation) = (1.0, 0.68);
    let low = value * (1.0 - saturation);
    let falling = value * (1.0 - saturation * offset);
    let rising = value * (1.0 - saturation * (1.0 - offset));
    let (red, green, blue) = match sector as u32 % 6 {
        0 => (value, rising, low),
        1 => (falling, value, low),
        2 => (low, value, rising),
        3 => (low, falling, value),
        4 => (rising, low, value),
        _ => (value, low, falling),
    };
    Color::rgb(red, green, blue)
}

/// The readout: population, what the schedule is about to add, and how the court is doing.
fn readout(
    census: Res<Census>,
    stats: Res<Stats>,
    paused: Res<Paused>,
    speed: Res<Speed>,
    mut texts: Query<&mut Text, With<Readout>>,
) {
    let Ok(mut text) = texts.single_mut() else {
        return;
    };
    let seconds = |ticks: u64| ticks as f32 / 60.0;
    let until = |every: u64| every - stats.ticks % every;
    let rate = if stats.saves + stats.misses == 0 {
        100.0
    } else {
        stats.saves as f32 / (stats.saves + stats.misses) as f32 * 100.0
    };
    // Countdowns are in simulated seconds, so they tick down faster on a sped-up court.
    text.value = format!(
        "{:>6.1}s   speed {:>4.2}x{}\nballs   {:>4}  (+1 in {:>4.1}s)\npaddles {:>4}  ({} left, {} right, +1 in {:>4.1}s)\nsaves  {:>6}   misses {}   returned {:.0}%\n\nspace pause   up/down speed   0 normal\nhold b for balls   hold p for paddles   r restart",
        seconds(stats.ticks),
        speed.0,
        if paused.0 { "   PAUSED" } else { "" },
        census.balls,
        seconds(until(game::BALL_EVERY)),
        census.paddles(),
        census.left,
        census.right,
        seconds(until(game::PADDLE_EVERY)),
        stats.saves,
        stats.misses,
        rate,
    );
}

fn main() {
    env_logger::init();
    Fulcrum::with_config(FulcrumConfig {
        title: "Rally".into(),
        window_size: (DEFAULT_COURT.x as u32, DEFAULT_COURT.y as u32),
        clear_color: Color::rgb(0.05, 0.06, 0.08),
        ..Default::default()
    })
    .insert_resource(assets!())
    .with_plugin(DefaultPlugins)
    .with_plugin(GamePlugin)
    .add_startup(setup)
    .add_frame_system(fit_window)
    .add_frame_system(dress_new)
    .add_frame_system(shape_paddles)
    .add_frame_system(style_balls)
    .add_frame_system(readout)
    .run();
}
