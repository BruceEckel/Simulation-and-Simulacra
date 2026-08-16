//! Boids simulation: Reynolds' three rules (separation, alignment, cohesion) plus edge
//! steering and a predator the flock flees. Pure logic — no sprites, no audio — so it runs
//! headless for the determinism test.
//!
//! Neighbor lookups go through [`SpatialGrid`], whose per-tick rebuild is a `FixedUpdate`
//! system: add [`SpatialPlugin`] **before** [`GamePlugin`] so the grid holds this tick's
//! positions when [`steer_boids`] reads it.
//!
//! The arena resizes with the window, which needs care: the simulation must never read
//! renderer state, or the flock would behave differently per window size and headless runs
//! would have nothing to read at all. So the window doesn't touch [`Arena`] directly. The
//! binary watches the window frame-side and sends an [`ARENA_COMMAND`] through
//! [`CommandOutbox`], the replayable order channel; the simulation applies it on the next
//! tick like any other player command, and replays reproduce every resize exactly.

use fulcrum::prelude::*;
use std::f32::consts::TAU;

/// Arena size at startup, and the area every resize preserves.
pub const DEFAULT_ARENA: Vec2 = Vec2::new(1024.0, 768.0);
/// Aspect-ratio limits for a resize, so a sliver of a window can't produce a sliver of a world.
pub const ASPECT_LIMITS: (f32, f32) = (0.3, 3.5);
/// Name of the resize command on the replayable command channel.
pub const ARENA_COMMAND: &str = "arena";
/// How many boids the flock starts with.
pub const FLOCK_SIZE: usize = 240;
/// A boid sees neighbors within this radius. Keep the grid's cell size near it.
pub const NEIGHBOR_RADIUS: f32 = 56.0;
/// Closer than this and a boid actively pushes away.
pub const SEPARATION_RADIUS: f32 = 24.0;
/// Boids never fully stop.
pub const MIN_SPEED: f32 = 70.0;
/// Boid speed ceiling, units/second.
pub const MAX_SPEED: f32 = 190.0;
/// Steering-force cap, units/second².
pub const MAX_FORCE: f32 = 420.0;
/// Weight of the "don't crowd" rule.
pub const SEPARATION_WEIGHT: f32 = 1.7;
/// Weight of the "match headings" rule.
pub const ALIGNMENT_WEIGHT: f32 = 1.0;
/// Weight of the "steer toward the local center" rule.
pub const COHESION_WEIGHT: f32 = 0.9;
/// Weight of the "flee the predator" rule.
pub const FEAR_WEIGHT: f32 = 4.0;
/// A boid notices the predator this far away.
pub const FEAR_RADIUS: f32 = 150.0;
/// Boids start turning back this far from a wall.
pub const EDGE_MARGIN: f32 = 110.0;
/// How hard the walls push, units/second².
pub const EDGE_FORCE: f32 = 640.0;
/// Predator cruising speed — under [`MAX_SPEED`], so a fleeing boid can get away.
pub const PREDATOR_SPEED: f32 = 172.0;
/// The predator turns lazily; this is its steering cap, units/second².
pub const PREDATOR_FORCE: f32 = 210.0;
/// How far the predator can spot a boid.
pub const PREDATOR_SIGHT: f32 = 420.0;

/// The flock's world, in world units. Simulation state, changed only by [`ARENA_COMMAND`].
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct Arena(pub Vec2);

impl Default for Arena {
    fn default() -> Self {
        Self(DEFAULT_ARENA)
    }
}

/// The arena a window of this pixel size should get: the window's aspect ratio at
/// [`DEFAULT_ARENA`]'s area, so flock density and apparent boid size stay put while the shape
/// follows the window. Whole units, so the command payload round-trips exactly.
pub fn arena_for_window(window: Vec2) -> Vec2 {
    let area = DEFAULT_ARENA.x * DEFAULT_ARENA.y;
    let aspect = (window.x / window.y).clamp(ASPECT_LIMITS.0, ASPECT_LIMITS.1);
    vec2(
        (area * aspect).sqrt().round(),
        (area / aspect).sqrt().round(),
    )
}

/// Encode an arena size for [`ARENA_COMMAND`]. Whole units as text: a resize that lands in a
/// replay has to decode back to the identical `f32`, and integers always do.
pub fn arena_payload(size: Vec2) -> String {
    format!("{} {}", size.x as i32, size.y as i32)
}

/// Decode an [`arena_payload`]. `None` for anything malformed or degenerate — a command
/// channel is an input, so it gets treated like one.
pub fn parse_arena(payload: &str) -> Option<Vec2> {
    let (width, height) = payload.split_once(' ')?;
    let size = vec2(
        width.trim().parse::<i32>().ok()? as f32,
        height.trim().parse::<i32>().ok()? as f32,
    );
    (size.x >= 1.0 && size.y >= 1.0).then_some(size)
}

/// One member of the flock.
#[derive(Component)]
pub struct Boid;

/// The hunter. Not [`SpatialIndexed`], so it never shows up as a flock neighbor.
#[derive(Component)]
pub struct Predator;

/// Simulation velocity, units/second.
#[derive(Component)]
pub struct Velocity(pub Vec2);

/// Which rules are switched on. Toggling them at runtime is the point of the demo: turn two
/// off and the third one's contribution becomes obvious.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rules {
    /// Steer away from crowded neighbors.
    pub separation: bool,
    /// Match the neighbors' average heading.
    pub alignment: bool,
    /// Steer toward the neighbors' center of mass.
    pub cohesion: bool,
    /// Run the predator and let the flock fear it.
    pub predator: bool,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            separation: true,
            alignment: true,
            cohesion: true,
            predator: true,
        }
    }
}

/// Installs the boids simulation. Add **after** [`SpatialPlugin`].
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut().insert_resource(Rules::default());
        app.world_mut().insert_resource(Arena::default());
        app.add_systems(Startup, spawn_flock);
        app.add_systems(
            FixedUpdate,
            (
                apply_arena,
                toggle_rules,
                reset_flock,
                steer_boids,
                move_predator,
            )
                .chain(),
        );
    }
}

/// Spawn the flock and the predator. Public so the binary can order its sprite-attachment
/// after it.
pub fn spawn_flock(mut commands: Commands, mut rng: ResMut<SimRng>, arena: Res<Arena>) {
    populate(&mut commands, &mut rng, arena.0);
}

/// The shared spawn used at startup and on reset.
fn populate(commands: &mut Commands, rng: &mut SimRng, arena: Vec2) {
    for _ in 0..FLOCK_SIZE {
        let position = vec2(
            rng.range_f32(-arena.x / 2.0..arena.x / 2.0),
            rng.range_f32(-arena.y / 2.0..arena.y / 2.0),
        );
        let velocity = random_velocity(rng);
        commands.spawn((
            Boid,
            SpatialIndexed,
            facing(position, velocity),
            Velocity(velocity),
        ));
    }
    let velocity = random_velocity(rng);
    commands.spawn((Predator, facing(Vec2::ZERO, velocity), Velocity(velocity)));
}

/// A transform at `position` rotated to point along `velocity`.
fn facing(position: Vec2, velocity: Vec2) -> Transform2D {
    Transform2D {
        translation: position,
        rotation: velocity.to_angle(),
        ..Transform2D::IDENTITY
    }
}

/// A random heading at a random speed in the boid range.
fn random_velocity(rng: &mut SimRng) -> Vec2 {
    Vec2::from_angle(rng.range_f32(0.0..TAU)) * rng.range_f32(MIN_SPEED..MAX_SPEED)
}

/// Reynolds steering: the force that bends `velocity` toward `desired`, capped at `max_force`.
/// `desired` only carries a direction; its length is ignored.
fn steer(desired: Vec2, velocity: Vec2, max_speed: f32, max_force: f32) -> Vec2 {
    let Some(direction) = desired.try_normalize() else {
        return Vec2::ZERO;
    };
    (direction * max_speed - velocity).clamp_length_max(max_force)
}

/// A push back toward the middle, ramping from 0 at [`EDGE_MARGIN`] to 1 at the wall (and
/// past 1 for anything that overshot).
fn edge_push(position: Vec2, arena: Vec2) -> Vec2 {
    let bound = arena / 2.0 - Vec2::splat(EDGE_MARGIN);
    let mut push = Vec2::ZERO;
    if position.x > bound.x {
        push.x -= (position.x - bound.x) / EDGE_MARGIN;
    } else if position.x < -bound.x {
        push.x += (-bound.x - position.x) / EDGE_MARGIN;
    }
    if position.y > bound.y {
        push.y -= (position.y - bound.y) / EDGE_MARGIN;
    } else if position.y < -bound.y {
        push.y += (-bound.y - position.y) / EDGE_MARGIN;
    }
    push
}

/// Hard containment: whatever the steering decided, nothing leaves the arena. The crossing
/// velocity component is reflected so the mover turns around instead of grinding the wall.
fn contain(position: &mut Vec2, velocity: &mut Vec2, arena: Vec2) {
    let limit = arena / 2.0;
    if position.x < -limit.x {
        position.x = -limit.x;
        velocity.x = velocity.x.abs();
    } else if position.x > limit.x {
        position.x = limit.x;
        velocity.x = -velocity.x.abs();
    }
    if position.y < -limit.y {
        position.y = -limit.y;
        velocity.y = velocity.y.abs();
    } else if position.y > limit.y {
        position.y = limit.y;
        velocity.y = -velocity.y.abs();
    }
}

/// Advance one mover by `dt`, clamping speed, containing it, and pointing it where it flies.
fn integrate(
    transform: &mut Transform2D,
    velocity: &mut Vec2,
    acceleration: Vec2,
    dt: f32,
    speed_range: (f32, f32),
    arena: Vec2,
) {
    let mut next = *velocity + acceleration * dt;
    if next.length_squared() < 1e-6 {
        next = Vec2::X * speed_range.0; // degenerate cancel-out; any fixed heading will do
    }
    *velocity = next.clamp_length(speed_range.0, speed_range.1);
    let mut position = transform.translation + *velocity * dt;
    contain(&mut position, velocity, arena);
    transform.translation = position;
    transform.rotation = velocity.to_angle();
}

/// Everything the arena contains: the flock plus the predator.
type Movers<'w, 's> = Query<'w, 's, &'static mut Transform2D, Or<(With<Boid>, With<Predator>)>>;

/// Move the walls when a resize command arrives, and pull anything a shrink stranded outside
/// back in. This is the only writer of [`Arena`].
fn apply_arena(
    mut arena: ResMut<Arena>,
    mut orders: EventReader<CommandEvent>,
    mut movers: Movers,
) {
    let mut resized = false;
    for order in orders.read() {
        if order.name != ARENA_COMMAND {
            continue;
        }
        if let Some(size) = parse_arena(&order.payload) {
            arena.0 = size;
            resized = true;
        }
    }
    if !resized {
        return;
    }
    let limit = arena.0 / 2.0;
    for mut transform in &mut movers {
        transform.translation = transform.translation.clamp(-limit, limit);
    }
}

/// Digit keys switch the rules; P parks the predator.
fn toggle_rules(mut rules: ResMut<Rules>, input: Res<Input>) {
    if input.just_pressed(Key::Digit1) {
        rules.separation = !rules.separation;
    }
    if input.just_pressed(Key::Digit2) {
        rules.alignment = !rules.alignment;
    }
    if input.just_pressed(Key::Digit3) {
        rules.cohesion = !rules.cohesion;
    }
    if input.just_pressed(Key::P) {
        rules.predator = !rules.predator;
    }
}

/// Everything a reset clears out.
type FlockEntities<'w, 's> = Query<'w, 's, Entity, Or<(With<Boid>, With<Predator>)>>;

/// R scatters a fresh flock. The binary dresses the new entities frame-side.
fn reset_flock(
    mut commands: Commands,
    mut rng: ResMut<SimRng>,
    input: Res<Input>,
    arena: Res<Arena>,
    existing: FlockEntities,
) {
    if !input.just_pressed(Key::R) {
        return;
    }
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    populate(&mut commands, &mut rng, arena.0);
}

/// Every boid, read-only: the snapshot neighbors are resolved against.
type FlockSnapshot<'w, 's> =
    Query<'w, 's, (Entity, &'static Transform2D, &'static Velocity), With<Boid>>;
/// Every boid, mutable: the pass that actually steers and moves them.
type FlockMover<'w, 's> =
    Query<'w, 's, (Entity, &'static mut Transform2D, &'static mut Velocity), With<Boid>>;

/// The three rules, plus walls and fear. Neighbor state comes from a snapshot taken at the
/// same instant the grid was indexed, so a boid's result never depends on iteration order.
fn steer_boids(
    mut flock: ParamSet<(FlockMover, FlockSnapshot)>,
    predators: Query<&Transform2D, (With<Predator>, Without<Boid>)>,
    grid: Res<SpatialGrid>,
    rules: Res<Rules>,
    arena: Res<Arena>,
    time: Res<Time>,
) {
    let snapshot: FxHashMap<Entity, (Vec2, Vec2)> = flock
        .p1()
        .iter()
        .map(|(entity, transform, velocity)| (entity, (transform.translation, velocity.0)))
        .collect();
    let threat = rules
        .predator
        .then(|| predators.iter().next().map(|t| t.translation))
        .flatten();
    let dt = time.fixed_delta;

    for (entity, mut transform, mut velocity) in &mut flock.p0() {
        let position = transform.translation;
        let mut away = Vec2::ZERO;
        let mut heading = Vec2::ZERO;
        let mut center = Vec2::ZERO;
        let mut neighbors = 0.0;

        for other in grid.query_circle(position, NEIGHBOR_RADIUS) {
            if other == entity {
                continue;
            }
            let Some((other_position, other_velocity)) = snapshot.get(&other) else {
                continue;
            };
            neighbors += 1.0;
            heading += *other_velocity;
            center += *other_position;
            let offset = position - *other_position;
            let distance = offset.length();
            if distance < SEPARATION_RADIUS {
                // Weight by closeness, so the nearest crowder dominates.
                away += offset / distance.max(0.001) * (1.0 - distance / SEPARATION_RADIUS);
            }
        }

        let mut acceleration = Vec2::ZERO;
        if neighbors > 0.0 {
            if rules.separation {
                acceleration += steer(away, velocity.0, MAX_SPEED, MAX_FORCE) * SEPARATION_WEIGHT;
            }
            if rules.alignment {
                acceleration +=
                    steer(heading / neighbors, velocity.0, MAX_SPEED, MAX_FORCE) * ALIGNMENT_WEIGHT;
            }
            if rules.cohesion {
                let to_center = center / neighbors - position;
                acceleration +=
                    steer(to_center, velocity.0, MAX_SPEED, MAX_FORCE) * COHESION_WEIGHT;
            }
        }
        if let Some(threat) = threat {
            let escape = position - threat;
            if escape.length() < FEAR_RADIUS {
                acceleration += steer(escape, velocity.0, MAX_SPEED, MAX_FORCE) * FEAR_WEIGHT;
            }
        }
        acceleration += edge_push(position, arena.0) * EDGE_FORCE;

        integrate(
            &mut transform,
            &mut velocity.0,
            acceleration,
            dt,
            (MIN_SPEED, MAX_SPEED),
            arena.0,
        );
    }
}

/// The predator chases the nearest boid it can see, and obeys the same walls.
fn move_predator(
    mut predators: Query<(&mut Transform2D, &mut Velocity), With<Predator>>,
    boids: Query<&Transform2D, (With<Boid>, Without<Predator>)>,
    grid: Res<SpatialGrid>,
    rules: Res<Rules>,
    arena: Res<Arena>,
    time: Res<Time>,
) {
    if !rules.predator {
        return;
    }
    for (mut transform, mut velocity) in &mut predators {
        let position = transform.translation;
        let prey = grid
            .nearest(position, PREDATOR_SIGHT, |_| true)
            .and_then(|entity| boids.get(entity).ok())
            .map(|target| target.translation - position)
            .unwrap_or(Vec2::ZERO);
        let acceleration = steer(prey, velocity.0, PREDATOR_SPEED, PREDATOR_FORCE)
            + edge_push(position, arena.0) * EDGE_FORCE;
        integrate(
            &mut transform,
            &mut velocity.0,
            acceleration,
            time.fixed_delta,
            (PREDATOR_SPEED, PREDATOR_SPEED),
            arena.0,
        );
    }
}
