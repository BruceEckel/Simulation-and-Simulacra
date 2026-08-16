//! RTS simulation: data-driven units, command processing, flow-field movement with separation,
//! and combat. Pure logic — unit stats live in `units/*.unit.ron` (discovered through the VFS,
//! so mods add types by dropping in files), visuals in the binary, wave direction in Lua.
//!
//! The lockstep shape: selection and camera are cosmetic, binary-local state. The simulation
//! consumes only [`CommandEvent`]s (`move`) and [`ModEvent`]s (`spawn_wave` from the Lua wave
//! director) — exactly the streams a replay records.

use fulcrum::prelude::*;
use serde::{Deserialize, Serialize};

// --- data-driven unit definitions ---------------------------------------------------------

/// One `units/<kind>.unit.ron` file. Zero Rust per unit type.
#[derive(Deserialize, Clone, Debug)]
#[serde(rename = "Unit")]
pub struct UnitDef {
    /// Roster display name.
    pub display: String,
    /// Movement speed, world units/second.
    pub speed: f32,
    /// Starting hit points.
    pub hp: i32,
    /// Damage per attack (0 = non-combatant).
    pub damage: i32,
    /// Attack reach in world units.
    pub range: f32,
    /// Ticks between attacks.
    pub cooldown: u32,
    /// Enemy-chasing radius (0 = never chases).
    pub aggro: f32,
    /// Body radius for separation steering.
    pub radius: f32,
    /// Team tint.
    pub color: Color,
}

/// Every unit definition, keyed by kind (the file stem). Loaded once at startup by listing
/// `units/*.unit.ron` through the VFS — base game and mods alike.
#[derive(Resource, Default)]
pub struct UnitDefs(pub FxHashMap<String, UnitDef>);

// --- components (registered => replay-hashed) ----------------------------------------------

/// Which side a unit fights for (1 = player, 2 = attackers).
#[derive(Component, Serialize, Deserialize, Default, Clone, PartialEq, Eq)]
pub struct Team(pub u8);

/// The unit's definition key.
#[derive(Component, Serialize, Deserialize, Default, Clone)]
pub struct UnitKind(pub String);

/// Hit points.
#[derive(Component, Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct Health {
    pub max: i32,
    pub current: i32,
}

/// Movement stats copied from the unit definition at spawn.
#[derive(Component, Serialize, Deserialize, Default, Clone)]
pub struct Mobility {
    pub speed: f32,
    pub radius: f32,
}

/// Attack stats + cooldown state.
#[derive(Component, Serialize, Deserialize, Default, Clone)]
pub struct Combat {
    pub damage: i32,
    pub range: f32,
    pub cooldown: u32,
    pub aggro: f32,
    #[serde(default)]
    pub timer: u32,
}

/// An active move order: follow the flow field until near the target.
#[derive(Component, Serialize, Deserialize, Default, Clone)]
pub struct MoveOrder {
    pub target: Vec2,
    pub field: u32,
}

/// A fading battlefield leftover (sim entity so replays agree on despawn timing).
#[derive(Component, Serialize, Deserialize, Default, Clone)]
pub struct Corpse {
    pub ticks_left: u32,
    pub max: u32,
}

// --- resources ------------------------------------------------------------------------------

/// The battlefield's navigation grid, built from the tilemap at startup.
#[derive(Resource)]
pub struct Nav(pub NavGrid);

/// Flow fields created by move orders, keyed by id (so a hundred units share one field).
#[derive(Resource, Default)]
pub struct FlowFields {
    pub next_id: u32,
    pub fields: FxHashMap<u32, FlowField>,
}

/// A `move` command's payload (RON in the `CommandEvent`).
#[derive(Serialize, Deserialize)]
pub struct MoveCommand {
    /// `Entity::to_bits` of the ordered units.
    pub units: Vec<u64>,
    pub x: f32,
    pub y: f32,
}

pub const TILE: f32 = 32.0;
pub const MAP_ORIGIN: Vec2 = Vec2::new(-1024.0, -768.0);
pub const ARRIVE_DISTANCE: f32 = 28.0;

// --- setup ----------------------------------------------------------------------------------

/// Register the game's components for prefab/replay-hash use.
pub fn register_components(app: Fulcrum) -> Fulcrum {
    app.register_component::<Team>("Team")
        .register_component::<UnitKind>("UnitKind")
        .register_component::<Health>("Health")
        .register_component::<Mobility>("Mobility")
        .register_component::<Combat>("Combat")
        .register_component::<MoveOrder>("MoveOrder")
        .register_component::<Corpse>("Corpse")
}

/// Load the map (data + drawable), build the nav grid, discover unit definitions, and muster
/// the player's starting army.
fn setup_battlefield(world: &mut World) {
    // Tile data through the sim-safe loader (works headless; textures dress it cosmetically).
    let handle = world.resource_scope(
        |world, mut maps: bevy_ecs::world::Mut<Assets<TilemapAsset>>| {
            let server = world.resource::<AssetServer>();
            load_tilemap_data(server, &mut maps, "maps/battlefield.map.ron")
                .expect("battlefield map must parse")
        },
    );
    let nav = {
        let maps = world.resource::<Assets<TilemapAsset>>();
        let asset = maps.get(handle).expect("just loaded");
        // Tile 2 is rock; everything else costs 10 (the A*/flow-field unit step).
        NavGrid::from_tilemap(asset, "ground", MAP_ORIGIN, |tile| {
            (tile != 2).then_some(10)
        })
        .expect("map has a ground layer")
    };
    world.insert_resource(Nav(nav));
    world.spawn((
        Tilemap {
            asset: handle,
            z: -10.0,
        },
        Transform2D::from_translation(MAP_ORIGIN),
    ));

    // Unit roster: every units/*.unit.ron visible through the VFS (mods included).
    let mut defs = UnitDefs::default();
    {
        let server = world.resource::<AssetServer>();
        for path in server.list("units", "ron") {
            if !path.ends_with(".unit.ron") {
                continue;
            }
            let kind = path
                .rsplit('/')
                .next()
                .unwrap_or(&path)
                .trim_end_matches(".unit.ron")
                .to_string();
            let bytes = server.read_bytes(&path).expect("listed file must read");
            let text = std::str::from_utf8(&bytes).expect("unit def must be UTF-8");
            match ron::from_str::<UnitDef>(text) {
                Ok(def) => {
                    log::info!("unit type `{kind}` ({}) from {path}", def.display);
                    defs.0.insert(kind, def);
                }
                Err(error) => log::warn!("skipping {path}: {error}"),
            }
        }
    }

    // The player's starting army: a block of soldiers with worker support, bottom-left.
    let mut spawns: Vec<(String, Vec2)> = Vec::new();
    for i in 0..24 {
        let (col, row) = (i % 6, i / 6);
        spawns.push((
            "soldier".into(),
            vec2(-780.0 + col as f32 * 30.0, -120.0 + row as f32 * 30.0),
        ));
    }
    for i in 0..6 {
        spawns.push(("worker".into(), vec2(-820.0 + i as f32 * 28.0, 40.0)));
    }
    for (kind, position) in spawns {
        spawn_unit(world, &defs, &kind, 1, position);
    }
    world.insert_resource(defs);
}

/// Spawn one unit from its definition. Sim components only — the binary dresses visuals.
pub fn spawn_unit(
    world: &mut World,
    defs: &UnitDefs,
    kind: &str,
    team: u8,
    position: Vec2,
) -> Option<Entity> {
    let def = defs.0.get(kind)?;
    Some(
        world
            .spawn((
                Transform2D::from_translation(position),
                Team(team),
                UnitKind(kind.to_string()),
                Health {
                    max: def.hp,
                    current: def.hp,
                },
                Mobility {
                    speed: def.speed,
                    radius: def.radius,
                },
                Combat {
                    damage: def.damage,
                    range: def.range,
                    cooldown: def.cooldown,
                    aggro: def.aggro,
                    timer: 0,
                },
                SpatialIndexed,
            ))
            .id(),
    )
}

// --- command + event intake -----------------------------------------------------------------

/// Consume `move` commands: build one flow field per order, attach [`MoveOrder`]s.
fn process_move_commands(world: &mut World) {
    let mut cursor = std::mem::take(&mut world.resource_mut::<MoveCommandCursor>().0);
    let commands: Vec<MoveCommand> = cursor
        .read(world.resource::<Events<CommandEvent>>())
        .filter(|c| c.name == "move")
        .filter_map(|c| ron::from_str(&c.payload).ok())
        .collect();
    world.resource_mut::<MoveCommandCursor>().0 = cursor;
    for command in commands {
        order_move(world, &command);
    }
}

/// Cursor for the exclusive command reader above (exclusive systems can't take `EventReader`).
#[derive(Resource, Default)]
struct MoveCommandCursor(bevy_ecs::message::MessageCursor<CommandEvent>);

/// Point `command.units` at `(x, y)` via a shared flow field.
pub fn order_move(world: &mut World, command: &MoveCommand) {
    let target = vec2(command.x, command.y);
    let field_id = {
        let goal = {
            let nav = &world.resource::<Nav>().0;
            match nav.world_to_cell(target) {
                Some(cell) if nav.is_walkable(cell.0, cell.1) => cell,
                _ => {
                    log::info!("move order into a wall or off-map; ignored");
                    return;
                }
            }
        };
        let field = FlowField::compute(&world.resource::<Nav>().0, &[goal]);
        let mut fields = world.resource_mut::<FlowFields>();
        let id = fields.next_id;
        fields.next_id += 1;
        fields.fields.insert(id, field);
        id
    };
    for &bits in &command.units {
        let Some(entity) = Entity::try_from_bits(bits) else {
            continue;
        };
        let Ok(mut unit) = world.get_entity_mut(entity) else {
            continue; // died since the order was issued
        };
        if unit.get::<Mobility>().is_some() {
            unit.insert(MoveOrder {
                target,
                field: field_id,
            });
        }
    }
}

/// Consume `spawn_wave` events (from the Lua wave director or mods): muster units in formation
/// and march them at the target.
fn process_spawn_waves(world: &mut World) {
    let mut cursor = std::mem::take(&mut world.resource_mut::<WaveEventCursor>().0);
    let waves: Vec<(String, u8, u32, Vec2, Vec2)> = {
        cursor
            .read(world.resource::<Events<ModEvent>>())
            .filter(|e| e.name == "spawn_wave")
            .filter_map(|e| {
                Some((
                    payload_str(&e.payload, "kind")?,
                    payload_f64(&e.payload, "team")? as u8,
                    payload_f64(&e.payload, "count")? as u32,
                    vec2(
                        payload_f64(&e.payload, "x")? as f32,
                        payload_f64(&e.payload, "y")? as f32,
                    ),
                    vec2(
                        payload_f64(&e.payload, "target_x")? as f32,
                        payload_f64(&e.payload, "target_y")? as f32,
                    ),
                ))
            })
            .collect()
    };
    world.resource_mut::<WaveEventCursor>().0 = cursor;
    for (kind, team, count, at, target) in waves {
        let defs = std::mem::take(&mut world.resource_mut::<UnitDefs>().0);
        let defs = UnitDefs(defs);
        let mut spawned = Vec::new();
        for i in 0..count {
            let offset = vec2((i % 4) as f32 * 26.0, (i / 4) as f32 * 26.0);
            if let Some(entity) = spawn_unit(world, &defs, &kind, team, at + offset) {
                spawned.push(entity);
            }
        }
        world.resource_mut::<UnitDefs>().0 = defs.0;
        if spawned.is_empty() {
            log::warn!("spawn_wave for unknown unit kind `{kind}`");
            continue;
        }
        order_move(
            world,
            &MoveCommand {
                units: spawned.iter().map(|e| e.to_bits()).collect(),
                x: target.x,
                y: target.y,
            },
        );
    }
}

/// Cursor for the exclusive wave reader.
#[derive(Resource, Default)]
struct WaveEventCursor(bevy_ecs::message::MessageCursor<ModEvent>);

fn payload_entry(payload: &ron::Value, key: &str) -> Option<ron::Value> {
    let ron::Value::Map(map) = payload else {
        return None;
    };
    map.iter()
        .find_map(|(k, v)| matches!(k, ron::Value::String(s) if s == key).then(|| v.clone()))
}

fn payload_f64(payload: &ron::Value, key: &str) -> Option<f64> {
    match payload_entry(payload, key)? {
        ron::Value::Number(n) => Some(n.into_f64()),
        _ => None,
    }
}

fn payload_str(payload: &ron::Value, key: &str) -> Option<String> {
    match payload_entry(payload, key)? {
        ron::Value::String(s) => Some(s),
        _ => None,
    }
}

// --- movement -------------------------------------------------------------------------------

type MoverQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut Transform2D,
        &'static Mobility,
        Option<&'static MoveOrder>,
        &'static Combat,
        &'static Team,
    ),
    With<Health>,
>;

type UnitSnapshotQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Transform2D, &'static Team), (With<Health>, With<Mobility>)>;

/// Flow-field following + aggro chasing + local separation, with wall clamping. Neighbor
/// lookups use a start-of-system position snapshot (same instant the spatial grid indexed), so
/// results don't depend on update order within the tick.
fn move_units(
    mut set: ParamSet<(MoverQuery, UnitSnapshotQuery)>,
    grid: Res<SpatialGrid>,
    fields: Res<FlowFields>,
    nav: Res<Nav>,
    time: Res<Time>,
    mut commands: Commands,
) {
    let dt = time.fixed_delta;
    let snapshot: FxHashMap<Entity, (Vec2, u8)> = set
        .p1()
        .iter()
        .map(|(entity, transform, team)| (entity, (transform.translation, team.0)))
        .collect();
    for (entity, mut transform, mobility, order, combat, team) in &mut set.p0() {
        let position = transform.translation;

        // Aggro: combat units chase the nearest visible enemy, stopping at attack range.
        let mut chase: Option<Vec2> = None;
        if combat.damage > 0 && combat.aggro > 0.0 {
            let enemy = grid.nearest(position, combat.aggro, |other| {
                other != entity
                    && snapshot
                        .get(&other)
                        .is_some_and(|(_, other_team)| *other_team != team.0)
            });
            if let Some(enemy) = enemy
                && let Some((enemy_position, _)) = snapshot.get(&enemy)
            {
                let to_enemy = *enemy_position - position;
                let distance = to_enemy.length();
                chase = Some(if distance > combat.range * 0.9 {
                    to_enemy / distance.max(0.001)
                } else {
                    Vec2::ZERO // in range: hold and let the attack system work
                });
            }
        }

        let mut arrived = false;
        let direction = match (&chase, order) {
            (Some(direction), _) => *direction,
            (None, Some(order)) => {
                if position.distance(order.target) < ARRIVE_DISTANCE {
                    arrived = true;
                    Vec2::ZERO
                } else {
                    fields
                        .fields
                        .get(&order.field)
                        .map(|field| field.sample(&nav.0, position))
                        .unwrap_or(Vec2::ZERO)
                }
            }
            (None, None) => Vec2::ZERO,
        };
        if arrived {
            commands.entity(entity).remove::<MoveOrder>();
        }

        // Separation: push away from close neighbors (query is deterministic + ordered).
        let mut push = Vec2::ZERO;
        for other in grid.query_circle(position, mobility.radius * 2.4) {
            if other == entity {
                continue;
            }
            if let Some((other_position, _)) = snapshot.get(&other) {
                let away = position - *other_position;
                let distance = away.length();
                if distance < mobility.radius * 2.4 {
                    push += away / (distance * distance).max(1.0) * 40.0;
                }
            }
        }
        let velocity = (direction * mobility.speed + push).clamp_length_max(mobility.speed);
        if velocity == Vec2::ZERO {
            continue;
        }

        // Axis-separated wall clamp against the nav grid.
        let step = velocity * dt;
        let mut next = position;
        let candidate_x = vec2(next.x + step.x, next.y);
        if walkable(&nav.0, candidate_x) {
            next = candidate_x;
        }
        let candidate_y = vec2(next.x, next.y + step.y);
        if walkable(&nav.0, candidate_y) {
            next = candidate_y;
        }
        transform.translation = next;
    }
}

fn walkable(nav: &NavGrid, world: Vec2) -> bool {
    nav.world_to_cell(world)
        .is_some_and(|(x, y)| nav.is_walkable(x, y))
}

// --- combat ---------------------------------------------------------------------------------

/// Cooldown ticking + nearest-enemy attacks. Damage applies immediately, in entity-sorted
/// attacker order (the query is deterministic).
fn attack(
    mut attackers: Query<(Entity, &Transform2D, &mut Combat, &Team)>,
    candidates: Query<(&Transform2D, &Team), With<Health>>,
    mut healths: Query<&mut Health>,
    grid: Res<SpatialGrid>,
    mut events: EventWriter<ModEvent>,
) {
    for (entity, transform, mut combat, team) in &mut attackers {
        if combat.timer > 0 {
            combat.timer -= 1;
            continue;
        }
        if combat.damage == 0 {
            continue;
        }
        let position = transform.translation;
        let target = grid.nearest(position, combat.range, |other| {
            other != entity
                && candidates
                    .get(other)
                    .is_ok_and(|(_, other_team)| other_team != team)
        });
        let Some(target) = target else { continue };
        let Ok(mut health) = healths.get_mut(target) else {
            continue;
        };
        health.current -= combat.damage;
        combat.timer = combat.cooldown;
        let at = candidates
            .get(target)
            .map(|(t, _)| t.translation)
            .unwrap_or(position);
        events.write(ModEvent {
            name: "unit_hit".into(),
            payload: xy_payload(at, &[]),
        });
    }
}

/// Despawn the dead: leave a fading corpse, tell the world (mods listen for `unit_died`).
fn reap(world: &mut World) {
    let dead: Vec<(Entity, Vec2, String, u8)> = {
        let mut query = world.query::<(Entity, &Health, &Transform2D, &UnitKind, &Team)>();
        query
            .iter(world)
            .filter(|(_, health, ..)| health.current <= 0)
            .map(|(entity, _, transform, kind, team)| {
                (entity, transform.translation, kind.0.clone(), team.0)
            })
            .collect()
    };
    for (entity, position, kind, team) in dead {
        world.despawn(entity);
        world.spawn((
            Transform2D::from_translation(position),
            Corpse {
                ticks_left: 180,
                max: 180,
            },
            UnitKind(kind.clone()),
            Team(team),
        ));
        let payload = xy_payload(position, &[("kind", ron::Value::String(kind))]);
        world.resource_mut::<Events<ModEvent>>().write(ModEvent {
            name: "unit_died".into(),
            payload,
        });
    }
}

fn xy_payload(at: Vec2, extra: &[(&str, ron::Value)]) -> ron::Value {
    let mut map = ron::value::Map::new();
    map.insert(
        ron::Value::String("x".into()),
        ron::Value::Number(ron::value::Number::new(at.x as f64)),
    );
    map.insert(
        ron::Value::String("y".into()),
        ron::Value::Number(ron::value::Number::new(at.y as f64)),
    );
    for (key, value) in extra {
        map.insert(ron::Value::String((*key).into()), value.clone());
    }
    ron::Value::Map(map)
}

/// Corpses fade out of the simulation on a tick timer.
fn decay_corpses(mut corpses: Query<(Entity, &mut Corpse)>, mut commands: Commands) {
    for (entity, mut corpse) in &mut corpses {
        corpse.ticks_left = corpse.ticks_left.saturating_sub(1);
        if corpse.ticks_left == 0 {
            commands.entity(entity).try_despawn();
        }
    }
}

// --- plugin ---------------------------------------------------------------------------------

/// Installs the RTS simulation. Add after `SpatialPlugin` and `ModPlugin`.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut().insert_resource(FlowFields::default());
        app.world_mut().insert_resource(UnitDefs::default());
        app.world_mut()
            .insert_resource(MoveCommandCursor::default());
        app.world_mut().insert_resource(WaveEventCursor::default());
        app.add_systems(Startup, setup_battlefield);
        app.add_systems(
            FixedUpdate,
            (
                process_move_commands,
                process_spawn_waves,
                move_units,
                attack,
                reap,
                decay_corpses,
            )
                .chain(),
        );
    }
}
