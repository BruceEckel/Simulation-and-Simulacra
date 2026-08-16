//! The weather, and where you are standing in it.
//!
//! There is very little state in this piece, and that is the honest shape of it: the clouds are
//! not a simulation being drawn, they are a *function* being evaluated, and everything the
//! function needs is a wind offset, a heading and a slowly turning dial of coverage. Six
//! numbers, advanced on the fixed tick, and the shader does the rest sixty times a second.
//!
//! Nothing in here knows about wgpu. It hands out an eye and a wind; `sky.rs` turns those into
//! uniforms and `clouds.wgsl` turns the uniforms into a sky.

use fulcrum::prelude::*;

// ---------------------------------------------------------------------------------------
// the world, in metres
// ---------------------------------------------------------------------------------------

/// How high the eye stands above the sand. High enough that the desert has some depth to it
/// rather than being one line across the bottom of the frame.
pub const EYE_HEIGHT: f32 = 55.0;

/// How far above the horizon the view is tilted, in radians. The subject is overhead.
pub const PITCH: f32 = 0.145;

/// Vertical field of view, in radians. Narrow, near a long lens: it flattens the perspective
/// and lets a cloud fill the frame without the frame bending around it.
pub const FOV: f32 = 0.60;

/// The underside of the cloud layer, in metres.
pub const CLOUD_BOTTOM: f32 = 1500.0;
/// The ceiling the anvils spread under, in metres.
pub const CLOUD_TOP: f32 = 5400.0;
/// The planet the layer is wrapped around. Earth's radius, and it is not decoration: a flat
/// slab of cloud has a hard edge at the horizon, and a curved one runs away into the haze the
/// way a real sky does.
pub const PLANET_RADIUS: f32 = 6_371_000.0;

/// How many metres one turn of the shape volume covers. This is the size of the weather:
/// the distance from one cloud to the next is a fraction of it.
pub const SHAPE_SCALE: f32 = 11_000.0;
/// The same for the volume that erodes the edges.
pub const DETAIL_SCALE: f32 = 780.0;

/// Which way the wind blows, as a direction in the horizontal plane. Across the view and a
/// little towards the eye, so clouds pass rather than recede.
pub const WIND: [f32; 3] = [-0.976, 0.0, -0.218];
/// How fast it blows at pace one, in metres a second. A real cloud base moves at about this,
/// and at this distance that is a stately drift.
pub const WIND_SPEED: f32 = 24.0;

/// How fast the fine detail is dragged along against the shape. Slightly faster, which reads
/// as the cloud boiling as it travels rather than sliding along rigid.
pub const DETAIL_DRIFT: f32 = 1.35;

// ---------------------------------------------------------------------------------------
// the weather
// ---------------------------------------------------------------------------------------

/// How much of the sky is cloud at the bottom and the top of the swell.
pub const COVERAGE: (f32, f32) = (0.07, 0.18);
/// How long the swell takes to come round, in seconds at pace one.
pub const SWELL_PERIOD: f32 = 240.0;

/// Slowest and fastest the weather is allowed to run.
pub const PACE_MIN: f32 = 0.05;
/// Fastest the weather is allowed to run.
pub const PACE_MAX: f32 = 24.0;
/// How much the pace is multiplied per second of holding the key.
const PACE_RAMP: f32 = 3.2;

/// How fast the head turns, in radians a second.
const TURN_RATE: f32 = 0.55;

/// Everything that moves.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct Weather {
    /// How far the cloud field has been carried, in metres. The only thing that makes the sky
    /// change: the volume stands still and the sampling point walks through it.
    pub drift: [f32; 3],
    /// Where the eye is looking, in radians clockwise from the wind's back.
    pub yaw: f32,
    /// A multiplier on the wind.
    pub pace: f32,
    /// Whether the sky is held still.
    pub held: bool,
    /// How far round the coverage swell has come, in radians.
    pub swell: f32,
}

impl Default for Weather {
    fn default() -> Self {
        Self {
            drift: [0.0; 3],
            yaw: 0.0,
            pace: 1.0,
            held: false,
            swell: 0.0,
        }
    }
}

impl Weather {
    /// How much of the sky is cloud, right now.
    pub fn coverage(&self) -> f32 {
        let swing = (self.swell.sin() * 0.5 + 0.5).clamp(0.0, 1.0);
        COVERAGE.0 + (COVERAGE.1 - COVERAGE.0) * swing
    }

    /// The eye: where it is, and the three directions it is facing.
    pub fn eye(&self) -> Eye {
        let (sin, cos) = self.yaw.sin_cos();
        let (up_sin, up_cos) = PITCH.sin_cos();
        Eye {
            at: [0.0, EYE_HEIGHT, 0.0],
            forward: [sin * up_cos, up_sin, cos * up_cos],
            right: [cos, 0.0, -sin],
            up: [-sin * up_sin, up_cos, -cos * up_sin],
        }
    }
}

/// A camera, as three unit vectors and a position. Right-handed, `y` up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Eye {
    /// Where it stands, in metres.
    pub at: [f32; 3],
    /// Where it looks.
    pub forward: [f32; 3],
    /// Its right.
    pub right: [f32; 3],
    /// Its up, which is the cross of the other two and so is tilted with the pitch.
    pub up: [f32; 3],
}

/// Installs the weather.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut().insert_resource(Weather::default());
        app.add_systems(FixedUpdate, (steer, blow).chain());
    }
}

/// Up and down set the pace, left and right turn the head, `Space` holds the sky still.
fn steer(input: Res<Input>, time: Res<Time>, mut weather: ResMut<Weather>) {
    let delta = time.fixed_delta;
    let ramp = PACE_RAMP.powf(delta);
    if input.pressed(Key::Up) {
        weather.pace = (weather.pace * ramp).min(PACE_MAX);
    }
    if input.pressed(Key::Down) {
        weather.pace = (weather.pace / ramp).max(PACE_MIN);
    }
    if input.pressed(Key::Left) {
        weather.yaw -= TURN_RATE * delta;
    }
    if input.pressed(Key::Right) {
        weather.yaw += TURN_RATE * delta;
    }
    if input.just_pressed(Key::Space) {
        weather.held = !weather.held;
    }
    weather.yaw = weather.yaw.rem_euclid(std::f32::consts::TAU);
}

/// Carry the cloud field downwind, and turn the coverage dial.
fn blow(time: Res<Time>, mut weather: ResMut<Weather>) {
    if weather.held {
        return;
    }
    let step = time.fixed_delta * weather.pace * WIND_SPEED;
    for (drift, wind) in weather.drift.iter_mut().zip(WIND) {
        *drift -= wind * step;
    }
    let turn = std::f32::consts::TAU / SWELL_PERIOD * time.fixed_delta * weather.pace;
    weather.swell = (weather.swell + turn).rem_euclid(std::f32::consts::TAU);
}
