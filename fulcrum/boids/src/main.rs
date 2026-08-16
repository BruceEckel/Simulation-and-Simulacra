//! Boids, windowed: everything you can see. The flock lives in `game.rs` and stays
//! renderer-free; this binary dresses its entities and draws the HUD.
//!
//! `cargo run -p boids` — 1/2/3 toggle separation/alignment/cohesion, P parks the predator,
//! R scatters a new flock.

use boids::game::{
    self, Arena, Boid, DEFAULT_ARENA, GamePlugin, MAX_SPEED, MIN_SPEED, Predator, Rules, Velocity,
};
use fulcrum::prelude::*;
use simulacra_assets::assets;

/// Boid sprite size in world units (the texture is 16x16).
const BOID_SIZE: f32 = 14.0;
/// Predator sprite size.
const HAWK_SIZE: f32 = 26.0;

/// Texture handles, loaded once and reused when the flock is reset.
#[derive(Resource)]
struct Art {
    boid: Handle<Texture>,
    hawk: Handle<Texture>,
}

/// Marks the rules readout.
#[derive(Component)]
struct HudText;

/// Load the art and put up the HUD. Dressing the entities themselves is [`dress_new`]'s job,
/// which also covers the flock R spawns later.
fn setup(mut commands: Commands, mut assets: AssetLoader) {
    commands.insert_resource(Art {
        boid: assets.load("boid.png"),
        hawk: assets.load("hawk.png"),
    });
    commands.spawn((
        HudText,
        Text::new("").with_size(16.0).with_z(10.0),
        Transform2D::from_xy(-DEFAULT_ARENA.x / 2.0 + 16.0, DEFAULT_ARENA.y / 2.0 - 28.0),
    ));
}

/// Keep window, camera, and arena in step — the piece that makes a resize actually mean
/// something.
///
/// Two halves, and the split matters. The **cosmetic** half is the zoom: the camera is left in
/// its default `Stretch` mode and zoomed so the arena exactly covers the window, which is what
/// stops black bars from appearing. The **simulation** half is the resize itself, and it does
/// not happen here: this system only *asks*, by putting a command in the outbox, because the
/// flock's world is simulation state and a frame system writing it directly would make the
/// game unplayable headless and unreplayable everywhere.
///
/// The command is sent once per distinct size rather than once per frame, so dragging a window
/// edge doesn't stuff a replay with thousands of duplicate orders.
fn fit_window(
    window: Res<WindowInfo>,
    arena: Res<Arena>,
    mut camera: ResMut<Camera2D>,
    mut outbox: ResMut<CommandOutbox>,
    mut requested: Local<Option<Vec2>>,
    mut huds: Query<&mut Transform2D, With<HudText>>,
) {
    let size = vec2(window.width as f32, window.height as f32);
    if size.x < 1.0 || size.y < 1.0 {
        return; // minimized: nothing to fit, and the aspect ratio would be nonsense
    }

    let wanted = game::arena_for_window(size);
    if wanted != arena.0 && *requested != Some(wanted) {
        outbox.send(game::ARENA_COMMAND, game::arena_payload(wanted));
        *requested = Some(wanted);
    }

    // Cover the window with the arena the simulation actually has right now (not the one just
    // requested — that lands next tick). `max` errs toward cropping a sub-pixel sliver, since
    // rounding the arena to whole units the other way would show a hairline bar.
    camera.zoom = (size.x / arena.0.x).max(size.y / arena.0.y);

    for mut hud in &mut huds {
        hud.translation = vec2(-arena.0.x / 2.0 + 16.0, arena.0.y / 2.0 - 28.0);
    }
}

/// Give any undressed boid or predator its sprite. Runs every frame so a reset flock gets
/// dressed the moment the simulation spawns it.
fn dress_new(
    mut commands: Commands,
    art: Option<Res<Art>>,
    boids: Query<Entity, (With<Boid>, Without<Sprite>)>,
    predators: Query<Entity, (With<Predator>, Without<Sprite>)>,
) {
    let Some(art) = art else { return };
    for boid in &boids {
        commands.entity(boid).try_insert(
            Sprite::new(art.boid)
                .with_size(Vec2::splat(BOID_SIZE))
                .with_color(Color::rgb(0.45, 0.8, 1.0)),
        );
    }
    for predator in &predators {
        commands.entity(predator).try_insert(
            Sprite::new(art.hawk)
                .with_size(Vec2::splat(HAWK_SIZE))
                .with_color(Color::rgb(1.0, 0.35, 0.3))
                .with_z(1.0),
        );
    }
}

/// Tint each boid by speed — cool when cruising, hot when sprinting — and fade the predator
/// out while it is parked. Presentation only: it reads simulation state, never writes it.
fn tint(
    mut boids: Query<(&mut Sprite, &Velocity), With<Boid>>,
    mut predators: Query<&mut Sprite, (With<Predator>, Without<Boid>)>,
    rules: Res<Rules>,
) {
    for (mut sprite, velocity) in &mut boids {
        let t = ((velocity.0.length() - MIN_SPEED) / (MAX_SPEED - MIN_SPEED)).clamp(0.0, 1.0);
        sprite.color = Color::rgb(0.35 + 0.6 * t, 0.78 + 0.16 * t, 1.0 - 0.28 * t);
    }
    let alpha = if rules.predator { 1.0 } else { 0.0 };
    for mut sprite in &mut predators {
        sprite.color = Color::rgba(1.0, 0.35, 0.3, alpha);
    }
}

/// Mirror the rule switches and the flock size into the HUD.
fn hud(
    rules: Res<Rules>,
    flock: Query<(), With<Boid>>,
    mut texts: Query<&mut Text, With<HudText>>,
) {
    let Ok(mut text) = texts.single_mut() else {
        return;
    };
    let mark = |on: bool| if on { "on " } else { "off" };
    text.value = format!(
        "boids {}\n1 separation {}\n2 alignment  {}\n3 cohesion   {}\np predator   {}\n{}",
        flock.iter().count(),
        mark(rules.separation),
        mark(rules.alignment),
        mark(rules.cohesion),
        mark(rules.predator),
        "r new flock",
    );
}

fn main() {
    env_logger::init();
    Fulcrum::with_config(FulcrumConfig {
        title: "Boids".into(),
        window_size: (DEFAULT_ARENA.x as u32, DEFAULT_ARENA.y as u32),
        clear_color: Color::rgb(0.04, 0.05, 0.09),
        ..Default::default()
    })
    .insert_resource(assets!())
    .with_plugin(DefaultPlugins)
    // Before the game plugin, so the grid rebuild runs first each tick and neighbor
    // queries see this tick's positions.
    .with_plugin(SpatialPlugin {
        cell_size: game::NEIGHBOR_RADIUS,
    })
    .with_plugin(GamePlugin)
    .add_startup(setup)
    .add_frame_system(fit_window)
    .add_frame_system(dress_new)
    .add_frame_system(tint)
    .add_frame_system(hud)
    .run();
}
