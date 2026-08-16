//! The weather, and where you are standing in it.
//!
//! Four numbers: how long the sky has been running, which way you are looking, how fast time is
//! going, and whether it is going at all. Every circle in the drawing is a function of the first
//! of those, which is why a still can be rendered at any moment in the weather without playing
//! the weather up to it.
//!
//! What the drawing looks like is not in here. The line weight, the number of arcs an element is
//! built from and which palette is on are a matter of taste rather than of weather, so they live
//! in [`crate::cloud::Style`] and in the binary's palette setting, and the keys for them are
//! read on the frame rather than on the tick.
//!
//! Nothing in here knows about wgpu. It hands out an eye and a clock; `cloud.rs` turns the clock
//! into circles, `sky.rs` turns those into uniforms, and `moebius3.wgsl` turns the uniforms into
//! a drawing.

use fulcrum::prelude::*;

// ---------------------------------------------------------------------------------------
// the world, in metres
// ---------------------------------------------------------------------------------------

/// How high the eye stands above the sand.
pub const EYE_HEIGHT: f32 = 45.0;

/// How far above the horizon the view is tilted, in radians. It puts the horizon along the
/// bottom of the frame with the whole depth of the sky above it, which is the shape of nearly
/// every Moebius desert panel: a low line and a great deal of air.
pub const PITCH: f32 = 0.27;

/// Vertical field of view, in radians.
pub const FOV: f32 = 0.74;

/// How wide the frame is along its bottom edge, in bearing, for a window of a given shape.
///
/// The horizontal field of view, worked out rather than written down: [`FOV`] is the vertical one
/// and the window says how much wider than tall it is. What the tilt does is the part worth
/// stating. A view pointed up at the sky is not a rectangle on the compass: the top edge of the
/// frame covers more bearing than the bottom edge does, because the two are the same width on the
/// projection plane and the bottom one is further round towards straight ahead. A quarter of a
/// radian of tilt is enough to make that a tenth of the width.
///
/// The bottom edge is the narrow one, so that is the one given here. It is what the traveller's
/// walk is folded into, and he rides along the bottom of the picture: folded into the middle width
/// instead, he would be past the corner of the frame and off it for a quarter of a minute every
/// lap, which is the one thing the fold is there to stop.
pub fn frame_span(aspect: f32) -> f32 {
    let up = (FOV * 0.5).tan();
    2.0 * (up * aspect.max(1e-3) / (PITCH.cos() + up * PITCH.sin())).atan()
}

/// The planet the desert is wrapped around. A flat one has a hard edge at the horizon; a curved
/// one runs away into the haze the way a real one does.
pub const PLANET_RADIUS: f32 = 6_371_000.0;

/// How far anything on the ground is drawn before the haze has it, in metres.
pub const HORIZON_DISTANCE: f32 = 90_000.0;

/// How many metres one band of sand is scaled against.
pub const GROUND_TILE: f32 = 5_200.0;

// ---------------------------------------------------------------------------------------
// the weather
// ---------------------------------------------------------------------------------------

/// Slowest the weather is allowed to run.
pub const PACE_MIN: f32 = 0.05;
/// Fastest. Past this a cloud is born and gone before it can be looked at.
pub const PACE_MAX: f32 = 32.0;
/// How much the pace is multiplied per second of holding the key.
const PACE_RAMP: f32 = 3.2;

/// How fast the head turns, in radians a second.
const TURN_RATE: f32 = 0.5;

/// Everything that moves.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct Weather {
    /// How long the sky has been running, in seconds of its own time.
    pub clock: f32,
    /// Where the eye is looking, in radians.
    pub yaw: f32,
    /// A multiplier on the passage of the clock.
    pub pace: f32,
    /// Whether the sky is held still.
    pub held: bool,
}

impl Default for Weather {
    fn default() -> Self {
        Self {
            clock: 0.0,
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

/// Run the clock, which is the only thing the sky is made of.
fn blow(time: Res<Time>, mut weather: ResMut<Weather>) {
    if weather.held {
        return;
    }
    weather.clock += time.fixed_delta * weather.pace;
}
