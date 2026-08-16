//! The weather, and where you are standing in it.
//!
//! Five numbers: how far the wind has carried the sky, how far the second cloud field has been
//! carried past the first, which way you are looking, how fast time is running, and whether it
//! is running at all. Everything you see is a function of those five, evaluated per pixel.
//!
//! Nothing in here knows about wgpu. It hands out an eye and a wind; `sky.rs` turns those into
//! uniforms and `ligne.wgsl` turns the uniforms into a drawing.

use fulcrum::prelude::*;

// ---------------------------------------------------------------------------------------
// the world, in metres
// ---------------------------------------------------------------------------------------

/// How high the eye stands above the sand.
pub const EYE_HEIGHT: f32 = 45.0;

/// How far above the horizon the view is tilted, in radians.
///
/// Higher than it looks like it should be, and for a reason worth writing down. A deck of cloud
/// is a horizontal plane, so a cloud on it is squashed on the screen by the sine of the angle
/// you are looking up at: at ten degrees a round cloud is a six-to-one smear. Everything in
/// this piece that reads as a cloud rather than as a slick is up above twenty degrees, so that
/// is where the frame is pointed, with the horizon left along the bottom edge for the clouds to
/// be measured against.
pub const PITCH: f32 = 0.27;

/// Vertical field of view, in radians. Wide enough to hold the horizon and the round part of
/// the sky in the same frame.
pub const FOV: f32 = 0.74;

/// The planet the decks are wrapped around. A flat deck of cloud has a hard edge at the
/// horizon; a curved one runs away into the haze the way a real sky does.
pub const PLANET_RADIUS: f32 = 6_371_000.0;

/// How far anything is drawn before the haze has it, in metres.
pub const HORIZON_DISTANCE: f32 = 90_000.0;

/// How many metres one tile of the sheet covers on the ground.
pub const GROUND_TILE: f32 = 5_200.0;

/// Which way the wind blows, in the horizontal plane. Across the view and a little towards the
/// eye, so clouds pass rather than recede.
pub const WIND: [f32; 2] = [-0.976, -0.218];

/// How fast it blows at pace one, in metres a second.
pub const WIND_SPEED: f32 = 30.0;

/// How fast the second cloud field is carried past the first, in metres a second.
///
/// The one number that decides whether this is weather or wallpaper. Two fields moving
/// together are a picture being slid across the window; two fields moving apart are a picture
/// being redrawn, and clouds that grow, lean, split and close again as they cross.
pub const BOIL_SPEED: f32 = 3.4;

// ---------------------------------------------------------------------------------------
// the weather
// ---------------------------------------------------------------------------------------

/// Slowest the weather is allowed to run.
pub const PACE_MIN: f32 = 0.05;
/// Fastest. Past this the clouds cross faster than they can be looked at.
pub const PACE_MAX: f32 = 24.0;
/// How much the pace is multiplied per second of holding the key.
const PACE_RAMP: f32 = 3.2;

/// How fast the head turns, in radians a second.
const TURN_RATE: f32 = 0.5;

/// Everything that moves.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct Weather {
    /// How far the sky has been carried, in metres, in the horizontal plane.
    pub drift: [f32; 2],
    /// How far the second field has been carried past the first, in metres.
    pub boil: f32,
    /// Where the eye is looking, in radians.
    pub yaw: f32,
    /// A multiplier on both of those.
    pub pace: f32,
    /// Whether the sky is held still.
    pub held: bool,
}

impl Default for Weather {
    fn default() -> Self {
        Self {
            drift: [0.0; 2],
            boil: 0.0,
            yaw: 0.0,
            pace: 1.0,
            held: false,
        }
    }
}

impl Weather {
    /// The eye: where it is, and the three directions it faces.
    pub fn eye(&self) -> Eye {
        let (sin, cos) = self.yaw.sin_cos();
        let (rise, run) = PITCH.sin_cos();
        Eye {
            at: [0.0, EYE_HEIGHT, 0.0],
            forward: [sin * run, rise, cos * run],
            right: [cos, 0.0, -sin],
            up: [-sin * rise, run, -cos * rise],
        }
    }
}

/// A camera, as a position and three unit vectors. Right-handed, `y` up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Eye {
    /// Where it stands, in metres.
    pub at: [f32; 3],
    /// Where it looks.
    pub forward: [f32; 3],
    /// Its right.
    pub right: [f32; 3],
    /// Its up.
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

/// Carry the sky downwind, and the second field past the first.
fn blow(time: Res<Time>, mut weather: ResMut<Weather>) {
    if weather.held {
        return;
    }
    let step = time.fixed_delta * weather.pace;
    for (drift, wind) in weather.drift.iter_mut().zip(WIND) {
        *drift -= wind * step * WIND_SPEED;
    }
    weather.boil += step * BOIL_SPEED;
}
