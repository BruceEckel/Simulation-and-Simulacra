//! RTS slice, windowed: camera, selection, orders, and battlefield dressing. Everything here
//! is cosmetic or command-emitting — the simulation lives in `game.rs` and consumes only
//! commands and events, so replays (R to record/save, `--replay file.freplay` to watch one)
//! reproduce battles exactly.

use fulcrum::prelude::*;
use rts_slice::game::{
    self, Corpse, GamePlugin, Health, Mobility, MoveCommand, Team, UnitDefs, UnitKind,
};
use simulacra_assets::assets;

const RECORDING_PATH: &str = "rts-slice.freplay";

fn main() {
    env_logger::init();
    let mut app = Fulcrum::with_config(FulcrumConfig {
        title: "rts-slice".into(),
        window_size: (1280, 720),
        clear_color: Color::rgb(0.05, 0.07, 0.05),
        record_replays: true, // R saves the battle so far
        gizmos_enabled: true, // selection rings + drag box are gameplay UI, not debug overlay
        ..Default::default()
    })
    .insert_resource(assets!())
    .with_plugin(DefaultPlugins)
    .with_plugin(SpatialPlugin { cell_size: 64.0 })
    .with_plugin(ModPlugin::from_dir(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/mods"
    )));
    app = game::register_components(app)
        .with_plugin(GamePlugin)
        .insert_resource(Selection::default())
        .add_startup(setup_camera)
        .add_frame_system(camera_control)
        .add_frame_system(dress_units)
        .add_frame_system(dress_corpses)
        // Per-tick, not per-frame: just_pressed/just_released edges last exactly one tick, and
        // a slow frame runs several ticks back-to-back — an Update system would miss clicks.
        // Still cosmetic-plus-commands: selection state is local; only orders enter the sim.
        .add_system(selection_and_orders)
        .add_system(toggle_recording)
        .add_frame_system(draw_selection)
        .add_frame_system(battle_effects);
    if std::env::var_os("FULCRUM_AUTOPILOT").is_some() {
        app = app.add_system(autopilot);
    }

    if let Some(path) = replay_arg() {
        let replay = Replay::load(&path).unwrap_or_else(|error| {
            eprintln!("cannot load replay {path}: {error}");
            std::process::exit(1);
        });
        println!("playing back {path} ({} ticks)", replay.ticks.len());
        app.start_playback(replay);
    }
    app.run();
}

fn replay_arg() -> Option<String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--replay" {
            return args.next();
        }
    }
    None
}

// --- camera ---------------------------------------------------------------------------------

fn setup_camera(mut camera: ResMut<Camera2D>) {
    camera.center = vec2(-600.0, 0.0); // start over the player's army
    camera.zoom = 0.75;
}

/// WASD/arrow + screen-edge pan, scroll zoom. Frame-rate based: the camera is cosmetic.
fn camera_control(
    mut camera: ResMut<Camera2D>,
    input: Res<Input>,
    window: Res<WindowInfo>,
    time: Res<Time>,
) {
    let mut pan = Vec2::ZERO;
    if input.pressed(Key::A) || input.pressed(Key::Left) {
        pan.x -= 1.0;
    }
    if input.pressed(Key::D) || input.pressed(Key::Right) {
        pan.x += 1.0;
    }
    if input.pressed(Key::S) || input.pressed(Key::Down) {
        pan.y -= 1.0;
    }
    if input.pressed(Key::W) || input.pressed(Key::Up) {
        pan.y += 1.0;
    }
    let mouse = input.mouse_screen();
    let (width, height) = (window.width as f32, window.height as f32);
    const EDGE: f32 = 12.0;
    if mouse.x < EDGE {
        pan.x -= 1.0;
    }
    if mouse.x > width - EDGE {
        pan.x += 1.0;
    }
    if mouse.y < EDGE {
        pan.y += 1.0; // screen +Y is down
    }
    if mouse.y > height - EDGE {
        pan.y -= 1.0;
    }
    let zoom = camera.zoom;
    camera.center += pan.clamp_length_max(1.0) * 700.0 * time.frame_delta / zoom;
    camera.center = camera
        .center
        .clamp(vec2(-1024.0, -768.0), vec2(1024.0, 768.0));
    if input.scroll_delta() != 0.0 {
        camera.zoom = (camera.zoom * (1.0 + input.scroll_delta() * 0.1)).clamp(0.4, 2.5);
    }
}

// --- battlefield dressing (sim spawns data; the binary attaches visuals) ---------------------

/// The unit disc texture, loaded once.
#[derive(Resource)]
struct UnitArt(Handle<Texture>);

type UndressedUnits<'w, 's> =
    Query<'w, 's, (Entity, &'static UnitKind, &'static Team), (With<Health>, Without<Sprite>)>;

fn dress_units(
    undressed: UndressedUnits,
    defs: Res<UnitDefs>,
    art: Option<Res<UnitArt>>,
    mut assets: AssetLoader,
    mut commands: Commands,
) {
    let texture = match &art {
        Some(art) => art.0,
        None => {
            let handle = assets.load("unit.png");
            commands.insert_resource(UnitArt(handle));
            handle
        }
    };
    for (entity, kind, team) in &undressed {
        let Some(def) = defs.0.get(&kind.0) else {
            continue;
        };
        let mut color = def.color;
        if team.0 != 1 {
            // Attackers get a hostile cast regardless of unit type.
            color = Color::rgba(
                (color.r * 0.6 + 0.4).min(1.0),
                color.g * 0.45,
                color.b * 0.45,
                color.a,
            );
        }
        let mut sprite = Sprite::new(texture);
        sprite.color = color;
        sprite.custom_size = Some(Vec2::splat(def.radius * 2.2));
        sprite.z = 1.0;
        commands.entity(entity).try_insert(sprite);
    }
}

fn dress_corpses(
    undressed: Query<(Entity, &Corpse), Without<Sprite>>,
    mut fading: Query<(&Corpse, &mut Sprite)>,
    art: Option<Res<UnitArt>>,
    mut commands: Commands,
) {
    let Some(art) = art else { return };
    for (entity, _) in &undressed {
        let mut sprite = Sprite::new(art.0);
        sprite.color = Color::rgba(0.25, 0.2, 0.2, 0.8);
        sprite.custom_size = Some(Vec2::splat(16.0));
        sprite.z = -1.0; // under the living
        commands.entity(entity).try_insert(sprite);
    }
    for (corpse, mut sprite) in &mut fading {
        sprite.color.a = 0.8 * corpse.ticks_left as f32 / corpse.max.max(1) as f32;
    }
}

// --- selection + orders (local, cosmetic — only commands reach the sim) ----------------------

/// Local selection state. Deliberately NOT a registered/sim component: in a lockstep world
/// each player has their own.
#[derive(Resource, Default)]
struct Selection {
    units: Vec<Entity>,
    drag_from: Option<Vec2>,
}

fn selection_and_orders(
    mut selection: ResMut<Selection>,
    input: Res<Input>,
    grid: Res<SpatialGrid>,
    teams: Query<&Team, With<Mobility>>,
    mut outbox: ResMut<CommandOutbox>,
) {
    let cursor = input.mouse_world();
    if input.mouse_just_pressed(MouseButton::Left) {
        selection.drag_from = Some(cursor);
    }
    if input.mouse_just_released(MouseButton::Left)
        && let Some(from) = selection.drag_from.take()
    {
        let min = from.min(cursor) - Vec2::splat(6.0); // a bare click still selects
        let max = from.max(cursor) + Vec2::splat(6.0);
        selection.units = grid
            .query_rect(Rect { min, max })
            .into_iter()
            .filter(|&e| teams.get(e).is_ok_and(|team| team.0 == 1))
            .collect();
        log::debug!("selected {} units", selection.units.len());
    }
    if input.mouse_just_pressed(MouseButton::Right) && !selection.units.is_empty() {
        let command = MoveCommand {
            units: selection.units.iter().map(|e| e.to_bits()).collect(),
            x: cursor.x,
            y: cursor.y,
        };
        if let Ok(payload) = ron::to_string(&command) {
            log::debug!(
                "move {} units to {:.0},{:.0}",
                selection.units.len(),
                cursor.x,
                cursor.y
            );
            outbox.send("move", payload);
        }
    }
}

fn draw_selection(
    selection: Res<Selection>,
    input: Res<Input>,
    bodies: Query<(&Transform2D, &Mobility)>,
    mut gizmos: ResMut<Gizmos>,
) {
    for &unit in &selection.units {
        if let Ok((transform, mobility)) = bodies.get(unit) {
            gizmos.circle(
                transform.translation,
                mobility.radius + 3.0,
                Color::rgba(0.4, 1.0, 0.4, 0.9),
            );
        }
    }
    if let Some(from) = selection.drag_from {
        let to = input.mouse_world();
        gizmos.rect(
            Rect {
                min: from.min(to),
                max: from.max(to),
            },
            Color::rgba(0.5, 1.0, 0.5, 0.8),
        );
    }
}

// --- effects + audio (cosmetic consumers of sim events) --------------------------------------

/// Sound handles, loaded lazily.
#[derive(Resource)]
struct BattleSounds {
    hit: Handle<Sound>,
    death: Handle<Sound>,
}

#[allow(clippy::too_many_arguments)]
fn battle_effects(
    mut events: EventReader<ModEvent>,
    mut effects: EffectLoader,
    mut commands: Commands,
    mut audio: ResMut<Audio>,
    mut sounds: SoundLoader,
    handles: Option<Res<BattleSounds>>,
    mut counter: Local<u32>,
) {
    let handles = match handles {
        Some(handles) => BattleSounds {
            hit: handles.hit,
            death: handles.death,
        },
        None => {
            let loaded = BattleSounds {
                hit: sounds.load("sounds/hit.wav"),
                death: sounds.load("sounds/death.wav"),
            };
            let copy = BattleSounds {
                hit: loaded.hit,
                death: loaded.death,
            };
            commands.insert_resource(loaded);
            copy
        }
    };
    for event in events.read() {
        let at = |payload: &ron::Value| {
            Some(vec2(
                payload_f64(payload, "x")? as f32,
                payload_f64(payload, "y")? as f32,
            ))
        };
        *counter = counter.wrapping_add(1);
        let pitch = 0.9 + (*counter % 5) as f32 * 0.05;
        match event.name.as_str() {
            "unit_hit" => {
                if let Some(position) = at(&event.payload)
                    && let Ok(effect) = effects.load("effects/hit.fx.ron")
                {
                    commands.spawn_effect_at(effect, position);
                    audio.play_with(
                        sounds.assets(),
                        handles.hit,
                        PlayParams {
                            volume: 0.25,
                            pitch,
                            ..Default::default()
                        },
                    );
                }
            }
            "unit_died" => {
                if let Some(position) = at(&event.payload)
                    && let Ok(effect) = effects.load("effects/death.fx.ron")
                {
                    commands.spawn_effect_at(effect, position);
                    audio.play_with(
                        sounds.assets(),
                        handles.death,
                        PlayParams {
                            volume: 0.5,
                            pitch,
                            ..Default::default()
                        },
                    );
                }
            }
            // Mods request arbitrary effects (sample_mod's berserker explosion).
            "spawn_effect" => {
                if let Some(position) = at(&event.payload)
                    && let Some(path) = payload_str(&event.payload, "effect")
                    && let Ok(effect) = effects.load(&path)
                {
                    commands.spawn_effect_at(effect, position);
                    audio.play_with(
                        sounds.assets(),
                        handles.death,
                        PlayParams {
                            volume: 0.8,
                            pitch: 0.7,
                            ..Default::default()
                        },
                    );
                }
            }
            _ => {}
        }
    }
}

fn payload_f64(payload: &ron::Value, key: &str) -> Option<f64> {
    let ron::Value::Map(map) = payload else {
        return None;
    };
    map.iter().find_map(|(k, v)| match (k, v) {
        (ron::Value::String(s), ron::Value::Number(n)) if s == key => Some(n.into_f64()),
        _ => None,
    })
}

fn payload_str(payload: &ron::Value, key: &str) -> Option<String> {
    let ron::Value::Map(map) = payload else {
        return None;
    };
    map.iter().find_map(|(k, v)| match (k, v) {
        (ron::Value::String(s), ron::Value::String(value)) if s == key => Some(value.clone()),
        _ => None,
    })
}

// --- replay control ---------------------------------------------------------------------------

/// R: save the battle so far as `rts-slice.freplay` (recording continues, so a later R saves
/// a longer one). Skipped during playback. The file write is out-of-band but touches no sim
/// state, so it's replay-safe.
fn toggle_recording(world: &mut World) {
    if !world.resource::<Input>().just_pressed(Key::R)
        || world.resource::<ReplayPlayback>().active()
    {
        return;
    }
    let tick = world.resource::<Time>().tick;
    match save_replay(world, RECORDING_PATH) {
        Ok(()) => log::info!("saved {RECORDING_PATH} ({tick} ticks)"),
        Err(error) => log::error!("replay save failed: {error}"),
    }
}

/// `FULCRUM_AUTOPILOT=1`: scripted mouse input for screenshot-driven verification — pushes the
/// same pending events the winit runner would, so selection, orders, and rings exercise the
/// real input path even on a machine where synthetic X11 clicks can't reach the window.
fn autopilot(mut input: ResMut<Input>, time: Res<Time>) {
    match time.tick {
        60 => {
            input.push_cursor(vec2(460.0, 300.0));
            input.push_mouse_button(MouseButton::Left, true);
        }
        tick @ 61..=90 => input.push_cursor(vec2(
            460.0 + (tick - 60) as f32 * 7.0,
            300.0 + (tick - 60) as f32 * 6.0,
        )),
        91 => input.push_mouse_button(MouseButton::Left, false),
        180 => {
            input.push_cursor(vec2(760.0, 560.0));
            input.push_mouse_button(MouseButton::Right, true);
        }
        182 => input.push_mouse_button(MouseButton::Right, false),
        // Meet the first wave head-on, then save the battle as a replay.
        600 => {
            input.push_cursor(vec2(1100.0, 320.0));
            input.push_mouse_button(MouseButton::Right, true);
        }
        602 => input.push_mouse_button(MouseButton::Right, false),
        2400 => input.push_key(Key::R, true),
        2402 => input.push_key(Key::R, false),
        _ => {}
    }
}
