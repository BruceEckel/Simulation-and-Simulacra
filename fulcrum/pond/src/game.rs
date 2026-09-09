//! Pond: a surface of water that answers whatever you do to it, slowly, and asks nothing.
//!
//! There is nothing to win here and nothing to get wrong, and the whole design is in the
//! service of that. Four decisions do the work:
//!
//! **Everything you see is one thing: the water.** The surface is a height field run as the
//! two-dimensional wave equation on a grid, and every visible event is a disturbance of that
//! field. A tap is a dent that rebounds into a ring; a held press is a bowl that grows while
//! you hold it and rebounds into a bigger ring when you let go; the lantern moving under the
//! surface lifts a wake behind it. Because there is one thing, nothing you do can conflict
//! with anything else. Rings pass through rings. See [`Water::step`].
//!
//! **It answers the pointer, not the clock.** Move the pointer and a lantern follows it, by a
//! spring rather than by attachment, so it arrives a moment after you and does not jump. Nothing
//! is timed, nothing needs a quick hand, and a wrong click does not exist. See [`attend`].
//!
//! **Left alone, it walks.** When the pointer has been still for a while the lantern goes for
//! a slow walk of its own, side to side across the water at a pace the eye can follow without
//! effort. That is bilateral stimulation, which is the piece of EMDR this borrows: a light
//! crossing from one side of the visual field to the other and back, over and over, with
//! nothing asked of you but to watch it. Move the pointer and it comes back to you. See
//! [`sweep`].
//!
//! **Nothing ends abruptly.** Waves are damped and the edges of the pond absorb rather than
//! reflect, so every ring dies away to nothing on its own and the surface is always, given a
//! minute, flat again. See [`SHORE`].
//!
//! Pure logic. No sprites, no colour, no audio, so it runs headless for the tests, and a
//! quarter of an hour of it takes a few seconds to simulate.
//!
//! The field is sized to the window through [`FIELD_COMMAND`] on the replayable command
//! channel, not by reading renderer state, so a headless run reshapes the pond as a
//! windowed one does.

use fulcrum::prelude::*;
use std::f32::consts::{PI, TAU};

/// Window size at startup, in pixels.
pub const DEFAULT_WINDOW: Vec2 = Vec2::new(1280.0, 800.0);
/// Name of the resize command on the replayable command channel.
pub const FIELD_COMMAND: &str = "field";

/// Pixels to a cell. Two: fine enough that the lighting in the shader, which is worked out
/// from the slope between neighbouring cells, shows no grid.
pub const CELL: f32 = 2.0;
/// The most cells the pond will hold. Not an aesthetic limit but a guarantee that the tick stays
/// quick: past this the cells grow rather than the count. A 1440p display fits under it at two
/// pixels a cell; a 4K one gets three.
pub const MAX_CELLS: u32 = 1_100_000;
/// Rows of the height field are padded to a multiple of this many cells, so that a row is a
/// whole number of 256-byte blocks and the field can be uploaded to the GPU as it is.
pub const ROW_ALIGN: u32 = 64;

/// How fast a ripple travels, in cells per second. Slow on purpose: a ring that takes several
/// seconds to cross the window is a thing to watch, and a ring that flashes across it is a
/// thing to react to. Bounded above by [`cfl`]: the wave equation goes unstable past it.
pub const WAVE_SPEED: f32 = 24.0;
/// What is left of a wave's motion after one second, from damping alone. A ring is visible
/// for about six seconds before it has gone.
pub const DAMPING_PER_SECOND: f32 = 0.70;
/// How quickly the surface settles back to level, per second. The wave equation conserves the
/// mean height, so a pond that has been pressed on would otherwise stay pressed; this lets it
/// come back up, slowly enough that it is not seen to.
pub const LEVEL_DECAY: f32 = 1.2;
/// How many cells deep the absorbing band at the edge of the pond is, and how much of a
/// wave's motion the innermost cell of that band takes per tick. The band thickens toward the
/// edge, so a wave arriving there is eaten rather than turned around. An edge that reflected
/// would fill the pond with interference within a minute, which is busy, and busy is the one
/// thing this must not be.
pub const SHORE: u32 = 36;
pub const SHORE_SINK: f32 = 0.14;

/// The radius of the bowl a press makes, in cells, at the moment of the press and after it has
/// been held for [`PRESS_GROW`] seconds.
pub const PRESS_RADIUS: (f32, f32) = (5.0, 16.0);
/// How deep the bowl is pressed, in height units.
pub const PRESS_DEPTH: f32 = 1.3;
/// Seconds a press takes to grow from the small bowl to the large one.
pub const PRESS_GROW: f32 = 2.2;
/// How much of the bowl a tap gets at once, so a quick click still leaves a ring; and how
/// quickly a held press deepens toward the bowl's full shape, per second.
pub const TAP_BLEND: f32 = 0.8;
pub const PRESS_RATE: f32 = 9.0;

/// The lantern's wake: how wide it is, in cells, how hard it presses the water down at full
/// speed, and the speed in pixels per second at which it is pressing as hard as it will.
pub const WAKE_RADIUS: f32 = 6.0;
pub const WAKE_FORCE: f32 = 70.0;
pub const WAKE_SPEED: f32 = 500.0;

/// A raindrop: how wide, in cells, and how deep. Small, so a drop is a thing noticed at the
/// edge of attention rather than a thing that interrupts it.
pub const RAIN_RADIUS: f32 = 2.6;
pub const RAIN_DEPTH: f32 = 0.55;
/// Shortest and longest wait between drops, in seconds.
pub const RAIN_GAP: (f32, f32) = (5.0, 11.0);

/// Seconds the pointer must be still before the lantern goes walking.
pub const IDLE_AFTER: f32 = 10.0;
/// How long the lantern takes to answer a move of the pointer. The response of a critically
/// damped spring, so it neither snaps nor overshoots. A second or so of lag is the difference
/// between a light that follows you and a cursor that is you.
pub const LANTERN_RESPONSE: f32 = 1.1;
/// The fastest the lantern will travel, in pixels per second, whatever the spring asks of it.
/// So that a pointer arriving from the other side of the display does not make it flash across.
pub const LANTERN_SPEED: f32 = 1100.0;
/// How close to the edge of the window the lantern will go, in pixels.
pub const LANTERN_INSET: f32 = 48.0;

/// One full walk, there and back, in seconds: the fastest, the starting pace, and the slowest.
/// Four seconds a cycle is fifteen crossings a minute, which is a good deal slower than the
/// rate a clinician would set and is meant to be. This is not a session; it is a place to
/// rest your eyes.
pub const SWEEP_PERIOD: (f32, f32, f32) = (2.0, 4.0, 8.0);
/// How far the walk reaches to either side of the middle, as a fraction of the window's width.
pub const SWEEP_REACH: f32 = 0.36;
/// The walk rises and falls a little as it goes, so that it is a path rather than a rail.
/// How much, as a fraction of the window's height, and how many seconds one rise and fall
/// takes.
pub const SWEEP_BOB: f32 = 0.05;
pub const SWEEP_BOB_PERIOD: f32 = 11.0;
/// How quickly a held pace key moves the period, as a fraction per second.
pub const PACE_RATE: f32 = 0.45;

/// The water: a height and a vertical speed for every cell.
///
/// Simulation state, changed by its own step and by the sources in this module. The
/// height is laid out row by row with [`stride`](Self::stride) cells to a row, which is the
/// layout the GPU requires, so the renderer uploads it without copying.
#[derive(Resource, Clone, Debug)]
pub struct Water {
    /// Cells across and down.
    pub across: u32,
    pub down: u32,
    /// Cells from the start of one row to the start of the next. At least `across`, and a
    /// multiple of [`ROW_ALIGN`]. The cells past `across` in a row are slack, not read.
    pub stride: u32,
    /// Pixels to a cell.
    pub cell: f32,
    /// The window this field was sized to, in pixels.
    pub window: Vec2,
    /// Height of the surface at every cell, `stride * down` of them.
    pub height: Vec<f32>,
    /// Vertical speed of the surface at every cell, height units per second.
    pub speed: Vec<f32>,
    /// Bumped by every change, so a renderer can tell whether what it has is current.
    pub revision: u64,
    /// The absorbing band, precomputed: how much of a cell's motion survives a tick, by
    /// column and by row. A cell's own factor is the smaller of the two.
    shore_x: Vec<f32>,
    shore_y: Vec<f32>,
}

impl Default for Water {
    fn default() -> Self {
        Self::new(DEFAULT_WINDOW)
    }
}

/// The lantern: the light that follows the pointer, and walks when the pointer is still.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Lantern {
    /// Where it is, in pixels.
    pub at: Vec2,
    /// How fast it is going, in pixels per second.
    pub velocity: Vec2,
    /// Whether it is following the pointer, as against walking on its own.
    pub attended: bool,
    /// Seconds since the pointer last moved or pressed.
    pub still_for: f32,
    /// Where the pointer was last tick, so that a move can be noticed.
    pub seen: Option<Vec2>,
    /// Where the walk is in its cycle, in radians.
    pub phase: f32,
    /// Seconds one full walk takes.
    pub period: f32,
    /// Seconds the walk has been going, for the rise and fall.
    pub walked: f32,
}

impl Default for Lantern {
    fn default() -> Self {
        Self {
            at: DEFAULT_WINDOW / 2.0,
            velocity: Vec2::ZERO,
            attended: false,
            still_for: 0.0,
            seen: None,
            phase: 0.0,
            period: SWEEP_PERIOD.1,
            walked: 0.0,
        }
    }
}

/// A press in progress: where the bowl is, in cells, and how long it has been held.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct Touch {
    pub held: Option<(Vec2, f32)>,
}

/// When the next drop falls.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Rain {
    pub due: f32,
}

impl Default for Rain {
    fn default() -> Self {
        Self { due: 3.0 }
    }
}

/// Smoothstep. Every ramp in the piece goes through this: a linear ramp has a corner at each
/// end, and a corner is a small event.
pub fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The Courant number of the scheme: how far a wave travels in one tick, in cells. The
/// explicit wave equation on a square grid is stable while this is under `1 / sqrt(2)`, and
/// the tests hold it well under.
pub fn cfl(dt: f32) -> f32 {
    WAVE_SPEED * dt
}

/// The grid a window of this size gets: cells across, cells down, and pixels to a cell.
///
/// Two pixels a cell until the cell count would pass [`MAX_CELLS`], then the cells grow, in
/// whole pixels, until it does not.
pub fn grid_for(window: Vec2) -> (u32, u32, f32) {
    let window = vec2(window.x.max(1.0), window.y.max(1.0));
    let mut cell = CELL;
    if window.x * window.y / (cell * cell) > MAX_CELLS as f32 {
        cell = (window.x * window.y / MAX_CELLS as f32).sqrt().ceil();
    }
    let across = (window.x / cell).ceil().max(3.0) as u32;
    let down = (window.y / cell).ceil().max(3.0) as u32;
    (across, down, cell)
}

/// The stride a row of this many cells is stored at.
pub fn stride_for(across: u32) -> u32 {
    across.div_ceil(ROW_ALIGN) * ROW_ALIGN
}

/// How much of a cell's motion survives a tick, by its distance from the nearest edge.
///
/// One in the open water; falling toward `1 - SHORE_SINK` over the last [`SHORE`] cells,
/// steeply at the very edge and gently where the band begins, so the band's inner edge is not
/// a thing to reflect from.
pub fn shore(distance: u32) -> f32 {
    if distance >= SHORE {
        return 1.0;
    }
    let depth = 1.0 - distance as f32 / SHORE as f32;
    1.0 - SHORE_SINK * depth * depth
}

/// Encode a window size for [`FIELD_COMMAND`]: whole pixels, so it round-trips without loss.
pub fn field_payload(size: Vec2) -> String {
    format!("{} {}", size.x as i32, size.y as i32)
}

/// Decode a [`field_payload`]. `None` for anything malformed or degenerate.
pub fn parse_field(payload: &str) -> Option<Vec2> {
    let (width, height) = payload.split_once(' ')?;
    let size = vec2(
        width.trim().parse::<i32>().ok()? as f32,
        height.trim().parse::<i32>().ok()? as f32,
    );
    (size.x >= 1.0 && size.y >= 1.0).then_some(size)
}

/// Where the walk is, in pixels, for a window of this size: across the middle of it and back,
/// with a slow rise and fall on top.
pub fn sweep(window: Vec2, phase: f32, walked: f32) -> Vec2 {
    let reach = window.x * SWEEP_REACH;
    let bob = window.y * SWEEP_BOB * (TAU * walked / SWEEP_BOB_PERIOD).sin();
    vec2(window.x / 2.0 + reach * phase.sin(), window.y / 2.0 + bob)
}

/// The phase at which the walk passes through `at`, heading the way `velocity` points.
///
/// So that a lantern setting off on a walk sets off from where it is, in the direction it was
/// going, rather than from wherever the cycle had got to.
pub fn phase_through(window: Vec2, at: Vec2, velocity: Vec2) -> f32 {
    let reach = (window.x * SWEEP_REACH).max(1.0);
    let phase = ((at.x - window.x / 2.0) / reach).clamp(-1.0, 1.0).asin();
    if velocity.x < 0.0 { PI - phase } else { phase }
}

/// Every cell within three radii of `centre`, with its Gaussian weight and its offset from
/// the centre in cells, handed to `visit` with the cell's index into a `stride`-wide field.
/// The box is clipped to the interior, one cell in from the edge, which is where the wave
/// equation runs.
fn footprint(
    across: u32,
    down: u32,
    stride: u32,
    centre: Vec2,
    radius: f32,
    mut visit: impl FnMut(usize, f32, Vec2),
) {
    let radius = radius.max(0.5);
    let reach = (radius * 3.0).ceil() as i32;
    let cx = centre.x.round() as i32;
    let cy = centre.y.round() as i32;
    let x0 = (cx - reach).max(1);
    let x1 = (cx + reach).min(across as i32 - 2);
    let y0 = (cy - reach).max(1);
    let y1 = (cy + reach).min(down as i32 - 2);
    let scale = -0.5 / (radius * radius);
    for y in y0..=y1 {
        let dy = y as f32 + 0.5 - centre.y;
        for x in x0..=x1 {
            let dx = x as f32 + 0.5 - centre.x;
            let weight = ((dx * dx + dy * dy) * scale).exp();
            visit(
                (y as u32 * stride + x as u32) as usize,
                weight,
                vec2(dx, dy),
            );
        }
    }
}

impl Water {
    /// Flat water sized to a window.
    pub fn new(window: Vec2) -> Self {
        let (across, down, cell) = grid_for(window);
        let stride = stride_for(across);
        let cells = (stride * down) as usize;
        Self {
            across,
            down,
            stride,
            cell,
            window: vec2(window.x.max(1.0), window.y.max(1.0)),
            height: vec![0.0; cells],
            speed: vec![0.0; cells],
            revision: 0,
            shore_x: (0..across).map(|x| shore(x.min(across - 1 - x))).collect(),
            shore_y: (0..down).map(|y| shore(y.min(down - 1 - y))).collect(),
        }
    }

    /// Cells in the field, not counting the slack at the end of each row.
    pub fn area(&self) -> u64 {
        u64::from(self.across) * u64::from(self.down)
    }

    /// The height at a cell. Anything off the field is level.
    pub fn at(&self, x: i32, y: i32) -> f32 {
        if x < 0 || y < 0 || x >= self.across as i32 || y >= self.down as i32 {
            return 0.0;
        }
        self.height[(y as u32 * self.stride + x as u32) as usize]
    }

    /// A point in pixels as a point in cells.
    pub fn to_cells(&self, pixels: Vec2) -> Vec2 {
        pixels / self.cell
    }

    /// Where the water is not flat: the sum of squared height and speed, which damping
    /// eats. Zero means still.
    pub fn energy(&self) -> f32 {
        let mut total = 0.0;
        for y in 0..self.down {
            let row = (y * self.stride) as usize;
            for i in row..row + self.across as usize {
                total += self.height[i] * self.height[i] + self.speed[i] * self.speed[i];
            }
        }
        total
    }

    /// The largest height anywhere, in either direction.
    pub fn peak(&self) -> f32 {
        let mut peak: f32 = 0.0;
        for y in 0..self.down {
            let row = (y * self.stride) as usize;
            for &h in &self.height[row..row + self.across as usize] {
                peak = peak.max(h.abs());
            }
        }
        peak
    }

    /// Flatten it.
    pub fn still(&mut self) {
        self.height.fill(0.0);
        self.speed.fill(0.0);
        self.revision += 1;
    }

    /// Press the surface down into a dent, at once: a drop landing.
    pub fn dent(&mut self, centre: Vec2, radius: f32, depth: f32) {
        let height = &mut self.height;
        footprint(
            self.across,
            self.down,
            self.stride,
            centre,
            radius,
            |i, w, _| {
                height[i] -= depth * w;
            },
        );
        self.revision += 1;
    }

    /// Press the surface toward a bowl, by `blend` of the way: a finger, held.
    ///
    /// Toward a shape rather than by a force, so that holding it does not keep deepening
    /// forever: the bowl has a floor, and the held surface arrives at it and stays.
    pub fn press(&mut self, centre: Vec2, radius: f32, depth: f32, blend: f32) {
        let blend = blend.clamp(0.0, 1.0);
        let height = &mut self.height;
        let speed = &mut self.speed;
        footprint(
            self.across,
            self.down,
            self.stride,
            centre,
            radius,
            |i, w, _| {
                let want = -depth * w;
                let pull = blend * w;
                height[i] += (want - height[i]) * pull;
                speed[i] *= 1.0 - pull;
            },
        );
        self.revision += 1;
    }

    /// Push the surface along `heading`: down ahead of the centre and up behind it, by
    /// `force`. The lantern's wake.
    ///
    /// A push and a lift in one, so that the net displacement is nothing. A push alone would
    /// leave a trough along the whole of the lantern's path, which reads as a shadow it drags
    /// behind it; this leaves the two arms of a wake.
    pub fn push(&mut self, centre: Vec2, radius: f32, force: f32, heading: Vec2) {
        let heading = heading.normalize_or_zero();
        let speed = &mut self.speed;
        footprint(
            self.across,
            self.down,
            self.stride,
            centre,
            radius,
            |i, w, offset| {
                let along = (offset.dot(heading) / radius).clamp(-1.5, 1.5);
                speed[i] -= force * w * along;
            },
        );
        self.revision += 1;
    }

    /// One tick of the wave equation, damped, with the shore taking what reaches it.
    ///
    /// Explicit and semi-implicit: the speed is updated from the current heights and then the
    /// heights from the new speeds, which is the ordinary leapfrog and is stable while
    /// [`cfl`] holds. The outermost ring of cells is held level, which the shore band in
    /// front of it makes invisible.
    pub fn step(&mut self, dt: f32) {
        if dt <= 0.0 {
            return;
        }
        let stride = self.stride as usize;
        let c2 = WAVE_SPEED * WAVE_SPEED * dt;
        let decay = DAMPING_PER_SECOND.powf(dt);
        let level = 1.0 - LEVEL_DECAY * dt;
        let height = &self.height;
        let speed = &mut self.speed;
        for y in 1..self.down as usize - 1 {
            let row = y * stride;
            let shore_y = self.shore_y[y];
            for x in 1..self.across as usize - 1 {
                let i = row + x;
                let lap = height[i - 1] + height[i + 1] + height[i - stride] + height[i + stride]
                    - 4.0 * height[i];
                let keep = decay * shore_y.min(self.shore_x[x]);
                speed[i] = (speed[i] + c2 * lap) * keep;
            }
        }
        let speed = &self.speed;
        let height = &mut self.height;
        for y in 1..self.down as usize - 1 {
            let row = y * stride;
            for i in row + 1..row + self.across as usize - 1 {
                height[i] = (height[i] + speed[i] * dt) * level;
            }
        }
        self.revision += 1;
    }

    /// Size the field to a new window, carrying the water across.
    ///
    /// Resampled rather than reset, so that dragging the edge of the window stretches the
    /// picture instead of wiping it: a pond that goes flat when you touch the window is a
    /// pond that punishes you for touching the window.
    pub fn resize(&mut self, window: Vec2) {
        let fresh = Self::new(window);
        if fresh.window == self.window {
            return;
        }
        let mut next = fresh;
        let sx = self.across as f32 / next.across as f32;
        let sy = self.down as f32 / next.down as f32;
        for y in 0..next.down {
            let v = (y as f32 + 0.5) * sy - 0.5;
            let y0 = v.floor();
            let fy = v - y0;
            for x in 0..next.across {
                let u = (x as f32 + 0.5) * sx - 0.5;
                let x0 = u.floor();
                let fx = u - x0;
                let (x0, y0) = (x0 as i32, y0 as i32);
                let i = (y * next.stride + x) as usize;
                let corner = |field: &[f32], cx: i32, cy: i32| {
                    if cx < 0 || cy < 0 || cx >= self.across as i32 || cy >= self.down as i32 {
                        0.0
                    } else {
                        field[(cy as u32 * self.stride + cx as u32) as usize]
                    }
                };
                let lerp = |field: &[f32]| {
                    let top = corner(field, x0, y0) * (1.0 - fx) + corner(field, x0 + 1, y0) * fx;
                    let bottom =
                        corner(field, x0, y0 + 1) * (1.0 - fx) + corner(field, x0 + 1, y0 + 1) * fx;
                    top * (1.0 - fy) + bottom * fy
                };
                next.height[i] = lerp(&self.height);
                next.speed[i] = lerp(&self.speed);
            }
        }
        next.revision = self.revision + 1;
        *self = next;
    }
}

/// Installs the piece.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut().insert_resource(Water::default());
        app.world_mut().insert_resource(Lantern::default());
        app.world_mut().insert_resource(Touch::default());
        app.world_mut().insert_resource(Rain::default());
        app.add_systems(
            FixedUpdate,
            (apply_field, controls, attend, touch, rain, wake, flow).chain(),
        );
    }
}

/// Resize the pond when a resize arrives, and take the lantern with it.
fn apply_field(
    mut water: ResMut<Water>,
    mut lantern: ResMut<Lantern>,
    mut orders: EventReader<CommandEvent>,
) {
    let mut wanted = None;
    for order in orders.read() {
        if order.name != FIELD_COMMAND {
            continue;
        }
        if let Some(size) = parse_field(&order.payload) {
            wanted = Some(size);
        }
    }
    let Some(size) = wanted else { return };
    if size == water.window {
        return;
    }
    let scale = size / water.window;
    water.resize(size);
    lantern.at *= scale;
}

/// The keys. None of them can hurry anything.
fn controls(
    input: Res<Input>,
    time: Res<Time>,
    mut water: ResMut<Water>,
    mut lantern: ResMut<Lantern>,
) {
    let dt = time.fixed_delta;
    // A slower or quicker walk. Held rather than stepped, so the pace slides.
    if input.pressed(Key::Up) {
        lantern.period *= 1.0 - PACE_RATE * dt;
    }
    if input.pressed(Key::Down) {
        lantern.period *= 1.0 + PACE_RATE * dt;
    }
    lantern.period = lantern.period.clamp(SWEEP_PERIOD.0, SWEEP_PERIOD.2);
    // Send the lantern walking now, or call it back.
    if input.just_pressed(Key::Space) {
        if lantern.attended {
            set_off(&mut lantern, water.window);
        } else {
            lantern.attended = true;
            lantern.still_for = 0.0;
        }
    }
    // Still the water.
    if input.just_pressed(Key::R) {
        water.still();
    }
}

/// Begin a walk from where the lantern is.
fn set_off(lantern: &mut Lantern, window: Vec2) {
    lantern.attended = false;
    lantern.still_for = IDLE_AFTER;
    lantern.phase = phase_through(window, lantern.at, lantern.velocity);
    lantern.walked = 0.0;
}

/// Carry the lantern: after the pointer while the pointer is moving, and off on its walk when
/// it has not moved for a while.
///
/// It is a critically damped spring either way, capped in speed, so what changes when the
/// pointer stops or starts is only where the spring is pulling toward. The lantern is not
/// placed; it travels.
fn attend(input: Res<Input>, time: Res<Time>, water: Res<Water>, mut lantern: ResMut<Lantern>) {
    let dt = time.fixed_delta;
    let window = water.window;
    let pointer = input.mouse_screen();
    // The first sighting of the pointer is not a move. Before it, the lantern walks.
    let moved = lantern.seen.is_some_and(|seen| seen != pointer);
    lantern.seen = Some(pointer);
    let touched = moved || input.mouse_pressed(MouseButton::Left);

    if touched {
        lantern.still_for = 0.0;
        lantern.attended = true;
    } else {
        lantern.still_for += dt;
        if lantern.attended && lantern.still_for >= IDLE_AFTER {
            set_off(&mut lantern, window);
        }
    }

    let target = if lantern.attended {
        let inset = vec2(
            LANTERN_INSET.min(window.x / 2.0),
            LANTERN_INSET.min(window.y / 2.0),
        );
        vec2(
            pointer.x.clamp(inset.x, window.x - inset.x),
            pointer.y.clamp(inset.y, window.y - inset.y),
        )
    } else {
        lantern.phase += TAU * dt / lantern.period.max(0.1);
        lantern.walked += dt;
        sweep(window, lantern.phase, lantern.walked)
    };

    let omega = TAU / LANTERN_RESPONSE;
    let pull = (target - lantern.at) * (omega * omega) - lantern.velocity * (2.0 * omega);
    let mut velocity = lantern.velocity + pull * dt;
    let speed = velocity.length();
    if speed > LANTERN_SPEED {
        velocity *= LANTERN_SPEED / speed;
    }
    lantern.velocity = velocity;
    lantern.at += velocity * dt;
}

/// A press: a dent at once, a bowl that grows while it is held, and nothing on
/// release, because the water does the rest.
fn touch(input: Res<Input>, time: Res<Time>, mut water: ResMut<Water>, mut held: ResMut<Touch>) {
    let dt = time.fixed_delta;
    let at = water.to_cells(input.mouse_screen());
    if input.mouse_just_pressed(MouseButton::Left) {
        held.held = Some((at, 0.0));
        water.press(at, PRESS_RADIUS.0, PRESS_DEPTH, TAP_BLEND);
        return;
    }
    if !input.mouse_pressed(MouseButton::Left) {
        held.held = None;
        return;
    }
    let Some((_, mut seconds)) = held.held else {
        return;
    };
    seconds += dt;
    // The bowl follows the pointer, so a press dragged along is a furrow.
    held.held = Some((at, seconds));
    let radius = PRESS_RADIUS.0 + (PRESS_RADIUS.1 - PRESS_RADIUS.0) * ease(seconds / PRESS_GROW);
    water.press(at, radius, PRESS_DEPTH, (PRESS_RATE * dt).min(1.0));
}

/// A drop, now and then, somewhere. It keeps the pond from being quite still, and it
/// shows, without a word, what a press does.
fn rain(
    time: Res<Time>,
    mut rain: ResMut<Rain>,
    mut rng: ResMut<SimRng>,
    mut water: ResMut<Water>,
) {
    rain.due -= time.fixed_delta;
    if rain.due > 0.0 {
        return;
    }
    rain.due = rng.range_f32(RAIN_GAP.0..RAIN_GAP.1);
    let margin = SHORE as f32;
    let at = vec2(
        rng.range_f32(margin..(water.across as f32 - margin).max(margin + 1.0)),
        rng.range_f32(margin..(water.down as f32 - margin).max(margin + 1.0)),
    );
    water.dent(at, RAIN_RADIUS, RAIN_DEPTH);
}

/// The lantern presses the water down as it goes, harder the faster it goes, so a moving
/// lantern leaves a wake and a resting one leaves the water alone.
fn wake(time: Res<Time>, lantern: Res<Lantern>, mut water: ResMut<Water>) {
    let speed = lantern.velocity.length();
    if speed < 1.0 {
        return;
    }
    let force = WAKE_FORCE * (speed / WAKE_SPEED).min(1.0) * time.fixed_delta;
    let at = water.to_cells(lantern.at);
    water.push(at, WAKE_RADIUS, force, lantern.velocity);
}

/// Let the water go.
fn flow(time: Res<Time>, mut water: ResMut<Water>) {
    water.step(time.fixed_delta);
}
