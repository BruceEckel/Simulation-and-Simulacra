//! Pond, windowed: the water, lit. The water and the lantern live in `game.rs` and stay
//! renderer-free; everything here is colour, light, and the keys.
//!
//! `cargo run -p pond`
//!
//! - move the pointer and the lantern follows; press and hold for a ring; leave it and it walks
//! - scroll drifts the colours, `c` is the next palette
//! - `up` / `down` change the pace of the walk, `space` sends the lantern off or calls it back
//! - `r` stills the water, `h` puts the hint away, `F11` goes fullscreen and back
//!
//! It opens in an ordinary window. `F11` takes the whole display with no border, and `F11`
//! again gives the window back.

use fulcrum::prelude::*;
use fulcrum_render::{GpuContext, WindowHandle};
use pond::game::{self, GamePlugin, Lantern, Water};
use pond::look::{LOOKS, blend, sample};
use pond::screen::{OUTPUT_FORMAT, Renderer, Scene, compose};
use simulacra_assets::assets;
use simulacra_frame::{Frame, FramePlugin, fit_frame};
use std::f32::consts::TAU;

/// Hint text height in world units, before HiDPI scaling. The built-in font is sharpest at
/// multiples of 8.
const HINT_SIZE: f32 = 8.0;
/// Gap between the hint and the bottom of the window.
const HINT_MARGIN: f32 = 28.0;
/// Seconds the hint stays up before it goes away and leaves you alone.
const HINT_LIFE: f32 = 16.0;
/// How bright the hint is while it is up.
const HINT_ALPHA: f32 = 0.55;
/// Seconds a palette takes to cross into the next one.
const PALETTE_CROSSFADE: f32 = 3.0;
/// How far one notch of the scroll wheel moves the colours, in palette lengths, and how long
/// the colours take to get there. Eased, so a spin of the wheel is a slow drift rather than a
/// jump.
const DRIFT_STEP: f32 = 0.04;
const DRIFT_EASE: f32 = 1.2;
/// The lantern breathes: how bright it is, how much brighter at the top of a breath, and how
/// long a breath is. Eleven seconds is five and a half breaths a minute, which is the pace
/// slow-breathing practice settles on; nothing asks you to breathe with it, but you can.
const GLOW: f32 = 0.65;
const GLOW_SWELL: f32 = 0.35;
const BREATH_PERIOD: f32 = 11.0;

/// What the hint says while it is up.
const HINT_LINES: &str = "move  the lantern follows      press and hold  a ring      leave it alone  it walks\n\
                          scroll  colours     c  palette     up / down  pace     space  send it off     r  still     h  hint     F11  window";

/// Everything that is a matter of taste rather than of the water.
#[derive(Resource)]
struct Painter {
    /// Which of [`LOOKS`] is showing, and which it is coming from.
    palette: usize,
    previous: usize,
    /// How far across the crossfade is, `0..1`.
    blend: f32,
    /// Where the colours have drifted to, and where they are heading.
    drift: f32,
    drift_wanted: f32,
    /// Seconds since the start.
    elapsed: f32,
    /// Seconds the hint has been up.
    shown: f32,
    /// Whether the hint is wanted.
    hint: bool,
    /// Whether the pointer has been hidden yet.
    hidden: bool,
}

impl Default for Painter {
    fn default() -> Self {
        Self {
            palette: 0,
            previous: 0,
            blend: 1.0,
            drift: 0.0,
            drift_wanted: 0.0,
            elapsed: 0.0,
            shown: 0.0,
            hint: true,
            hidden: false,
        }
    }
}

/// Marks the hint line.
#[derive(Component)]
struct Hint;

/// The pipeline, built on the first frame that has a device. The texture it draws into, and
/// the sprite showing it, are `simulacra-frame`'s.
#[derive(Resource, Default)]
struct Pass(Option<Renderer>);

/// Put the hint up.
fn setup(mut commands: Commands) {
    commands.spawn((
        Hint,
        Text::new("")
            .with_size(HINT_SIZE)
            .with_align(HAlign::Center)
            .with_z(10.0),
        Transform2D::default(),
    ));
}

/// Tell the water how big the window is, and hold the hint at the bottom of it.
///
/// The size goes over the replayable command channel rather than being read out of the
/// renderer, which lets a headless run reshape the pond as a windowed one does.
fn fit_window(
    window: Res<WindowInfo>,
    mut outbox: ResMut<CommandOutbox>,
    mut requested: Local<Option<Vec2>>,
    mut hints: Query<(&mut Transform2D, &mut Text), With<Hint>>,
) {
    let size = vec2(window.width as f32, window.height as f32);
    if size.x < 1.0 || size.y < 1.0 {
        return; // minimized
    }
    if *requested != Some(size) {
        outbox.send(game::FIELD_COMMAND, game::field_payload(size));
        *requested = Some(size);
    }
    let scale = window.scale_factor.round().max(1.0);
    for (mut transform, mut text) in &mut hints {
        text.size = HINT_SIZE * scale;
        transform.translation = vec2(0.0, -size.y / 2.0 + HINT_MARGIN * scale);
    }
}

/// The keys that change nothing about the water: the palette, the drift, the hint, and
/// whether this is a window or the whole display. Debounced against the previous frame rather
/// than using `just_pressed`, since a frame system can see the same tick's edge twice.
fn look_controls(
    input: Res<Input>,
    time: Res<Time>,
    mut painter: ResMut<Painter>,
    window: Option<Res<WindowHandle>>,
    mut held: Local<[bool; 3]>,
) {
    let dt = time.frame_delta;
    painter.elapsed += dt;

    let down = [
        input.pressed(Key::C),
        input.pressed(Key::H),
        input.pressed(Key::F11),
    ];
    if down[0] && !held[0] {
        painter.previous = painter.palette;
        painter.palette = (painter.palette + 1) % LOOKS.len();
        painter.blend = 0.0;
    }
    if down[1] && !held[1] {
        painter.hint = !painter.hint;
        painter.shown = 0.0;
    }
    // Fullscreen is a window operation: the event loop keeps running, the fixed tick keeps
    // firing, and all the water hears about it is the resize that
    // `fit_window` sends a moment later.
    if down[2]
        && !held[2]
        && let Some(window) = &window
    {
        let fullscreen = window.0.fullscreen().is_none();
        window
            .0
            .set_fullscreen(fullscreen.then_some(winit::window::Fullscreen::Borderless(None)));
    }
    *held = down;

    // The pointer is the lantern. An arrow over the water would be a second thing to look at.
    if !painter.hidden
        && let Some(window) = &window
    {
        window.0.set_cursor_visible(false);
        painter.hidden = true;
    }

    painter.blend = (painter.blend + dt / PALETTE_CROSSFADE).min(1.0);

    painter.drift_wanted += input.scroll_delta() * DRIFT_STEP;
    let gap = painter.drift_wanted - painter.drift;
    painter.drift += gap * (1.0 - (-dt / DRIFT_EASE * TAU).exp());

    // Any key brings the hint back for a while.
    let touched = [
        Key::C,
        Key::H,
        Key::R,
        Key::Space,
        Key::Up,
        Key::Down,
        Key::F11,
    ]
    .iter()
    .any(|key| input.pressed(*key));
    painter.shown = if touched && painter.hint {
        0.0
    } else {
        painter.shown + dt
    };
}

/// Build the pipeline, once there is a device to build it against.
///
/// Not a startup system: the GPU does not exist until the window does, and the window does not
/// exist until the event loop has run once.
fn ensure_renderer(gpu: Option<Res<GpuContext>>, mut pass: ResMut<Pass>) {
    let Some(gpu) = gpu else { return };
    if pass.0.is_none() {
        pass.0 = Some(Renderer::new(&gpu.device));
    }
}

/// The palette as it is right now, part way through a crossfade or not.
fn stops(painter: &Painter) -> [[f32; 3]; 5] {
    let from = &LOOKS[painter.previous % LOOKS.len()];
    let to = &LOOKS[painter.palette % LOOKS.len()];
    blend(from, to, game::ease(painter.blend))
}

/// Carry this tick's water to the GPU if what is up there is older, and draw it.
fn draw(
    gpu: Option<Res<GpuContext>>,
    water: Res<Water>,
    lantern: Res<Lantern>,
    painter: Res<Painter>,
    window: Res<WindowInfo>,
    frame: Res<Frame>,
    mut pass: ResMut<Pass>,
) {
    let Some(gpu) = gpu else { return };
    let Some(renderer) = pass.0.as_mut() else {
        return;
    };
    let Some(view) = frame.view() else { return };
    if !frame.ready() {
        return;
    }
    let breath = 0.5 - 0.5 * (TAU * painter.elapsed / BREATH_PERIOD).cos();
    let scene = Scene {
        lantern: [lantern.at.x, lantern.at.y],
        glow: GLOW * (1.0 + GLOW_SWELL * breath),
        stops: stops(&painter),
        drift: painter.drift,
        time: painter.elapsed,
        scale: window.scale_factor,
    };
    let uniforms = compose(&water, &scene, frame.window());
    renderer.carry(&gpu.device, &gpu.queue, &water);
    renderer.draw(&gpu.device, &gpu.queue, &uniforms, view, frame.window());
}

/// The hint goes away after a while and comes back whenever a key is pressed, so the piece
/// spends nearly all of its life with nothing on it but water.
fn hint(painter: Res<Painter>, lantern: Res<Lantern>, mut hints: Query<&mut Text, With<Hint>>) {
    let fade = if painter.hint {
        ((HINT_LIFE - painter.shown) / 3.0).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let colour = sample(&stops(&painter), 0.72);
    for mut text in &mut hints {
        text.value = format!(
            "{HINT_LINES}\n{}, a walk every {:.0} seconds{}",
            LOOKS[painter.palette % LOOKS.len()].name,
            lantern.period,
            if lantern.attended { "" } else { ", walking" },
        );
        text.color = Color::rgba(colour[0], colour[1], colour[2], HINT_ALPHA * fade);
    }
}

fn main() {
    env_logger::init();
    Fulcrum::with_config(FulcrumConfig {
        title: "Pond".into(),
        window_size: (game::DEFAULT_WINDOW.x as u32, game::DEFAULT_WINDOW.y as u32),
        clear_color: Color::rgb(0.0, 0.0, 0.0),
        ..Default::default()
    })
    .insert_resource(assets!())
    .with_plugin(DefaultPlugins)
    .with_plugin(FramePlugin::new("pond", OUTPUT_FORMAT))
    .with_plugin(GamePlugin)
    .insert_resource(Painter::default())
    .insert_resource(Pass::default())
    .add_startup(setup)
    .add_frame_system(fit_window)
    .add_frame_system(look_controls)
    .add_frame_system((ensure_renderer, draw).chain().after(fit_frame))
    .add_frame_system(hint)
    .run();
}
