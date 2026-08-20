//! Nimbus, windowed: the keys, the readout and the frame the render passes draw into.
//!
//! `cargo run -p nimbus --release`
//!
//! - `up` and `down` set the pace, `left` and `right` turn your head, `Space` holds the sky
//! - `P` changes palette, `B` steps the picture into flat colour, `1`-`5` set how many rays the
//!   march runs, `H` puts the readout away, `F11` leaves fullscreen
//!
//! The window opens borderless over the whole display. Every frame, the piece submits two of
//! its own render passes: a raymarch of the cloud volume into a floating-point buffer, and a
//! finishing pass at the full size of the window. The engine never draws the sky; it draws one
//! sprite, and the sprite is the texture those two passes wrote.

use fulcrum::prelude::*;
use fulcrum_render::{GpuContext, WhitePixel, WindowHandle};
use nimbus::game::{GamePlugin, Weather};
use nimbus::look::LOOKS;
use nimbus::noise::{detail_volume, shape_volume};
use nimbus::sky::{self, OUTPUT_FORMAT, Renderer, Uniforms};
use simulacra_assets::assets;
use simulacra_frame::{Frame, FramePlugin, fit_frame};

/// Readout text height in world units, before HiDPI scaling.
const READOUT_SIZE: f32 = 8.0;
/// Gap between the readout and the corner of the window.
const READOUT_MARGIN: f32 = 12.0;
/// Size of the readout's backing panel, in world units, before HiDPI scaling.
const PANEL: Vec2 = Vec2::new(700.0, 64.0);
/// How many characters wide the readout is written to be.
const LINE_WIDTH: usize = 80;

/// How many pixels the march is aimed at, before the viewer says otherwise.
///
/// The march is the whole cost of this piece and it is paid per pixel, so the honest default is
/// a pixel count rather than a fraction: a fraction that is comfortable on a laptop panel is
/// four times too expensive on the wall-sized one next to it. This is about a million, which a
/// modest integrated part can raymarch inside a frame with room to spare.
const TARGET_PIXELS: f32 = 850_000.0;

/// What the number keys set the march's resolution to, as a fraction of the window.
const SCALES: [f32; 5] = [0.35, 0.5, 0.7, 0.85, 1.0];

/// How many bands the flat-colour finish steps the picture into.
const BANDS: f32 = 6.0;
/// How dark the line it draws between them is.
const INK: f32 = 0.8;

/// Everything that is a matter of taste rather than of weather.
#[derive(Resource)]
struct Painter {
    /// Which of [`LOOKS`] is in use.
    palette: usize,
    /// Whether the picture is stepped into flat colour and inked.
    flat: bool,
    /// Whether the readout is shown.
    readout: bool,
    /// Which of [`SCALES`] the march runs at, or `None` to pick one from [`TARGET_PIXELS`].
    scale: Option<usize>,
}

impl Default for Painter {
    fn default() -> Self {
        Self {
            palette: 0,
            flat: false,
            readout: true,
            scale: None,
        }
    }
}

/// The passes, the texture they draw into, and the sprite that shows it.
#[derive(Resource, Default)]
struct Screen {
    /// Built on the first frame, once there is a device to build it against.
    renderer: Option<Renderer>,
    /// What the march ran at last frame, for the readout.
    internal: (u32, u32),
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
            .with_color(Color::rgba(0.02, 0.02, 0.04, 0.66))
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
        let shape = shape_volume(7);
        let detail = detail_volume(11);
        screen.renderer = Some(Renderer::new(&gpu.device, &gpu.queue, &shape, &detail));
    }
}

/// Draw the sky: work out this frame's uniforms and submit the two passes.
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
    let screen_size = frame.window();
    let fraction = match painter.scale {
        Some(index) => SCALES[index % SCALES.len()],
        None => {
            let pixels = (screen_size.0 * screen_size.1) as f32;
            (TARGET_PIXELS / pixels).sqrt().clamp(0.35, 1.0)
        }
    };
    let internal = (
        ((screen_size.0 as f32 * fraction) as u32).max(1),
        ((screen_size.1 as f32 * fraction) as u32).max(1),
    );
    screen.internal = internal;
    // Eased, because a readout that flickers between 11 and 17 tells you less than one that
    // settles on 14.
    screen.pace += 0.06 * (time.frame_delta * 1000.0 - screen.pace);

    let uniforms: Uniforms = sky::compose(
        &weather,
        &LOOKS[painter.palette % LOOKS.len()],
        screen_size,
        internal,
        if painter.flat { BANDS } else { 0.0 },
        INK,
    );
    let Some(view) = frame.view() else { return };
    let Some(renderer) = screen.renderer.as_mut() else {
        return;
    };
    renderer.resize(&gpu.device, internal.0, internal.1);
    renderer.draw(&gpu.device, &gpu.queue, &uniforms, view, screen_size);
}

/// `P` changes palette, `B` flattens the colour, `1`-`5` set the march's resolution, `H` puts
/// the readout away, `F11` toggles fullscreen.
///
/// Debounced against the previous frame rather than using `just_pressed`, since a frame system
/// can see the same tick's edge twice.
fn painter_controls(
    input: Res<Input>,
    mut painter: ResMut<Painter>,
    window: Option<Res<WindowHandle>>,
    mut held: Local<[bool; 4]>,
) {
    let down = [
        input.pressed(Key::P),
        input.pressed(Key::B),
        input.pressed(Key::H),
        input.pressed(Key::F11),
    ];
    if down[0] && !held[0] {
        painter.palette = (painter.palette + 1) % LOOKS.len();
    }
    if down[1] && !held[1] {
        painter.flat = !painter.flat;
    }
    if down[2] && !held[2] {
        painter.readout = !painter.readout;
    }
    if down[3]
        && !held[3]
        && let Some(window) = &window
    {
        let fullscreen = window.0.fullscreen().is_none();
        window
            .0
            .set_fullscreen(fullscreen.then_some(winit::window::Fullscreen::Borderless(None)));
    }
    // The number keys take the resolution out of the piece's hands and put it in the viewer's,
    // which is the honest way round: how many rays a frame can afford is a fact about their
    // machine and not about their sky.
    let picks = [
        Key::Digit1,
        Key::Digit2,
        Key::Digit3,
        Key::Digit4,
        Key::Digit5,
    ];
    for (index, key) in picks.into_iter().enumerate() {
        if input.just_pressed(key) {
            painter.scale = Some(index);
        }
    }
    *held = down;
}

/// Thousands separators, because the pixel count is half the piece's boast.
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
        panel.color.a = if painter.readout { 0.66 } else { 0.0 };
    }
    if !painter.readout {
        text.value = String::new();
        return;
    }

    let look = &LOOKS[painter.palette % LOOKS.len()];
    let title = format!(
        "NIMBUS  {}{}",
        look.name,
        if painter.flat { " / flat" } else { "" }
    );
    let pace = if weather.held {
        "HELD".to_string()
    } else {
        format!("pace x{:.2}", weather.pace)
    };
    let width = LINE_WIDTH.saturating_sub(title.chars().count());
    let samples = grouped((screen.internal.0 as u64) * (screen.internal.1 as u64));
    text.value = format!(
        "{title}{pace:>width$}\n\
         marched {}x{} = {samples} rays into {}x{}   {:.0}% cloud   {:.1} ms\n\
         up/down pace   left/right look   Space hold   P palette   B flat   1-5 rays   H hide   F11 window",
        screen.internal.0,
        screen.internal.1,
        window.width,
        window.height,
        weather.coverage() * 100.0,
        screen.pace,
    );
}

fn main() {
    env_logger::init();
    Fulcrum::with_config(FulcrumConfig {
        title: "Nimbus".into(),
        window_size: (1600, 1000),
        clear_color: Color::rgb(0.0, 0.0, 0.0),
        ..Default::default()
    })
    .insert_resource(assets!())
    .with_plugin(DefaultPlugins)
    .with_plugin(FramePlugin::new("nimbus", OUTPUT_FORMAT))
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
