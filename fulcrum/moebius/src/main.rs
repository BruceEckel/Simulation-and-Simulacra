//! Moebius, windowed: the keys, the readout and the frame the pass draws into.
//!
//! `cargo run -p moebius --release`
//!
//! - `up` and `down` set the pace, `left` and `right` turn your head, `Space` holds the sky
//! - `P` changes palette, `H` puts the readout away, `F11` leaves fullscreen
//!
//! The window opens borderless over the whole display and the drawing is computed at **one
//! sample per physical pixel**. The engine never draws the sky; it draws one sprite, and the
//! sprite is the texture the pass wrote.

use fulcrum::prelude::*;
use fulcrum_render::{GpuContext, WhitePixel, WindowHandle};
use moebius::cloud::Sky;
use moebius::game::{GamePlugin, Weather};
use moebius::look::LOOKS;
use moebius::sky::{self, OUTPUT_FORMAT, Renderer, Slab};
use simulacra_assets::assets;
use simulacra_frame::{Frame, FramePlugin, fit_frame};

/// Readout text height in world units, before HiDPI scaling.
const READOUT_SIZE: f32 = 8.0;
/// Gap between the readout and the corner of the window.
const READOUT_MARGIN: f32 = 12.0;
/// Size of the readout's backing panel, in world units, before HiDPI scaling.
const PANEL: Vec2 = Vec2::new(668.0, 64.0);
/// How many characters wide the readout is written to be.
const LINE_WIDTH: usize = 76;

/// Everything that is a matter of taste rather than of weather.
#[derive(Resource)]
struct Painter {
    /// Which of [`LOOKS`] is in use.
    palette: usize,
    /// Whether the readout is shown.
    readout: bool,
}

impl Default for Painter {
    fn default() -> Self {
        Self {
            palette: 0,
            readout: true,
        }
    }
}

/// The pass, the texture it draws into, and the sprite that shows it.
#[derive(Resource, Default)]
struct Screen {
    /// Built on the first frame, once there is a device to build it against.
    renderer: Option<Renderer>,
    /// This frame's circles, rebuilt in place so the sky costs no allocations.
    sky: Sky,
    /// And the same circles laid out for the shader, cut down to what is in front of you.
    slab: Option<Box<Slab>>,
    /// How many groups of circles were drawn last frame.
    groups: usize,
    /// Frame time, eased, in milliseconds.
    pace: f32,
}

/// Marks the readout text.
#[derive(Component)]
struct Readout;

/// Marks the panel that keeps the readout legible over a bright sky.
#[derive(Component)]
struct Panel;

/// Go borderless over the whole display and put the readout up.
fn setup(
    mut commands: Commands,
    window: Option<Res<WindowHandle>>,
    white: Option<Res<WhitePixel>>,
) {
    if let Some(window) = &window {
        window
            .0
            .set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
    }
    let white = white.map(|white| white.0).unwrap_or(Handle::INVALID);
    commands.spawn((
        Panel,
        Sprite::new(white)
            .with_size(PANEL)
            .with_color(Color::rgba(0.02, 0.02, 0.04, 0.62))
            .with_z(9.0),
        Transform2D::default(),
    ));
    commands.spawn((
        Readout,
        Text::new("").with_size(READOUT_SIZE).with_z(10.0),
        Transform2D::default(),
    ));
}

/// Keep the readout in the corner of whatever shape the window has ended up.
fn fit_window(
    window: Res<WindowInfo>,
    mut readouts: Query<&mut Transform2D, With<Readout>>,
    mut panels: Query<&mut Transform2D, (With<Panel>, Without<Readout>)>,
) {
    if window.width == 0 || window.height == 0 {
        return; // minimized
    }
    let scale = window.scale_factor.round().max(1.0);
    let corner = vec2(
        -(window.width as f32) / 2.0 + READOUT_MARGIN * scale,
        -(window.height as f32) / 2.0 + (PANEL.y + READOUT_MARGIN) * scale,
    );
    for mut readout in &mut readouts {
        readout.translation = corner;
    }
    let panel = PANEL * scale;
    for mut transform in &mut panels {
        transform.translation = corner
            + vec2(
                panel.x / 2.0 - READOUT_MARGIN * scale,
                -panel.y / 2.0 + 6.0 * scale,
            );
    }
}

/// Build the pipeline, once there is a device to build it against.
///
/// Not a startup system: the GPU does not exist until the window does, and the window does not
/// exist until the event loop has run once. The texture it draws into, and the sprite showing
/// it, belong to `simulacra-frame` — see that crate for why they are not kept here.
fn ensure_renderer(gpu: Option<Res<GpuContext>>, mut screen: ResMut<Screen>) {
    let Some(gpu) = gpu else { return };
    if screen.renderer.is_none() {
        screen.renderer = Some(Renderer::new(&gpu.device));
        screen.slab = Some(Slab::boxed());
    }
}

/// Draw the sky: build this frame's circles, work out the uniforms and submit the pass.
fn draw(
    gpu: Option<Res<GpuContext>>,
    weather: Res<Weather>,
    painter: Res<Painter>,
    frame: Res<Frame>,
    time: Res<Time>,
    mut screen: ResMut<Screen>,
) {
    let Some(gpu) = gpu else { return };
    if !frame.ready() {
        return;
    }
    // Eased, because a readout that flickers between 5 and 9 tells you less than one that
    // settles on 7.
    screen.pace += 0.06 * (time.frame_delta * 1000.0 - screen.pace);

    screen.sky.build(weather.clock);
    let Some(view) = frame.view() else { return };
    let window = frame.window();
    let Screen {
        renderer: Some(renderer),
        slab: Some(slab),
        sky,
        ..
    } = &mut *screen
    else {
        return;
    };
    let uniforms = sky::compose(
        &weather,
        sky,
        &LOOKS[painter.palette % LOOKS.len()],
        window,
        slab,
    );
    renderer.draw(&gpu.device, &gpu.queue, &uniforms, slab, view, window);
    screen.groups = uniforms.counts[0] as usize;
}

/// `P` changes palette, `H` puts the readout away, `F11` toggles fullscreen.
///
/// Debounced against the previous frame rather than using `just_pressed`, since a frame system
/// can see the same tick's edge twice.
fn painter_controls(
    input: Res<Input>,
    mut painter: ResMut<Painter>,
    window: Option<Res<WindowHandle>>,
    mut held: Local<[bool; 3]>,
) {
    let down = [
        input.pressed(Key::P),
        input.pressed(Key::H),
        input.pressed(Key::F11),
    ];
    if down[0] && !held[0] {
        painter.palette = (painter.palette + 1) % LOOKS.len();
    }
    if down[1] && !held[1] {
        painter.readout = !painter.readout;
    }
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
}

/// Thousands separators, because the pixel count is half the boast.
fn grouped(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// What the sky is doing and how to lean on it.
fn readout(
    weather: Res<Weather>,
    painter: Res<Painter>,
    screen: Res<Screen>,
    window: Res<WindowInfo>,
    mut texts: Query<&mut Text, With<Readout>>,
    mut panels: Query<&mut Sprite, With<Panel>>,
) {
    let Ok(mut text) = texts.single_mut() else {
        return;
    };
    let scale = window.scale_factor.round().max(1.0);
    text.size = READOUT_SIZE * scale;
    for mut panel in &mut panels {
        panel.custom_size = Some(PANEL * scale);
        panel.color.a = if painter.readout { 0.62 } else { 0.0 };
    }
    if !painter.readout {
        text.value = String::new();
        return;
    }

    let title = format!("MOEBIUS  {}", LOOKS[painter.palette % LOOKS.len()].name);
    let pace = if weather.held {
        "HELD".to_string()
    } else {
        format!("pace x{:.2}", weather.pace)
    };
    let width = LINE_WIDTH.saturating_sub(title.chars().count());
    let pixels = grouped((window.width as u64) * (window.height as u64));
    text.value = format!(
        "{title}{pace:>width$}\n\
         drawn once per pixel: {}x{} = {pixels}   {} outlines   {:.0} s of weather   {:.1} ms\n\
         up/down pace   left/right look   Space hold   P palette   H hide   F11 window",
        window.width, window.height, screen.groups, weather.clock, screen.pace,
    );
}

fn main() {
    env_logger::init();
    Fulcrum::with_config(FulcrumConfig {
        title: "Moebius".into(),
        window_size: (1600, 1000),
        clear_color: Color::rgb(0.0, 0.0, 0.0),
        ..Default::default()
    })
    .insert_resource(assets!())
    .with_plugin(DefaultPlugins)
    .with_plugin(FramePlugin::new("moebius", OUTPUT_FORMAT))
    .with_plugin(GamePlugin)
    .insert_resource(Painter::default())
    .insert_resource(Screen::default())
    .add_startup(setup)
    .add_frame_system(fit_window)
    .add_frame_system(painter_controls)
    // Chained: the second of these draws with what the first builds.
    // Chained: the second draws with what the first builds, and both come after the shared
    // frame system, which is what decides the texture they draw into.
    .add_frame_system((ensure_renderer, draw).chain().after(fit_frame))
    .add_frame_system(readout)
    .run();
}
