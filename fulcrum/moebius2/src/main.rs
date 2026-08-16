//! Moebius, windowed: the keys, the readout and the frame the pass draws into.
//!
//! `cargo run -p moebius2 --release`
//!
//! - `up` and `down` set the pace, `left` and `right` turn your head, `Space` holds the sky
//! - `Z` and `X` set the weight of the line around a cloud
//! - `N` and `M` set how many arcs an element laid over a cloud is built from
//! - `P` and `O` walk the twenty palettes, `H` puts the readout away, `F11` leaves fullscreen
//!
//! The window opens borderless over the whole display and the drawing is computed at **one
//! sample per physical pixel**. The engine never draws the sky; it draws one sprite, and the
//! sprite is the texture the pass wrote.

use fulcrum::prelude::*;
use fulcrum_render::{GpuContext, WhitePixel, WindowHandle};
use moebius2::cloud::{ARCS_MAX, ARCS_MIN, INK_MAX, INK_MIN, Sky, Style};
use moebius2::game::{GamePlugin, Weather};
use moebius2::look::LOOKS;
use moebius2::sky::{self, OUTPUT_FORMAT, Renderer, Slab};
use simulacra_assets::assets;

/// Readout text height in world units, before HiDPI scaling.
const READOUT_SIZE: f32 = 8.0;
/// Gap between the readout and the corner of the window.
const READOUT_MARGIN: f32 = 12.0;
/// Size of the readout's backing panel, in world units, before HiDPI scaling.
///
/// The font is fixed-pitch and one unit of its size wide, so a panel holds [`LINE_WIDTH`] times
/// [`READOUT_SIZE`] of text and no more. The longest line here is the list of keys.
const PANEL: Vec2 = Vec2::new(768.0, 86.0);
/// How many characters wide the readout is written to be.
const LINE_WIDTH: usize = 94;

/// How many pixels the cloud line changes by per second of holding the key. Slow enough to stop
/// on a number, since the difference between 1.6 and 2.2 is the difference between a drawing and
/// a woodcut.
const INK_RATE: f32 = 2.2;

/// Everything that is a matter of taste rather than of weather.
#[derive(Resource)]
struct Painter {
    /// Which of [`LOOKS`] is in use.
    palette: usize,
    /// The line weight and the arc count, which are what the keys here are mostly for.
    style: Style,
    /// Whether the readout is shown.
    readout: bool,
}

impl Default for Painter {
    fn default() -> Self {
        Self {
            palette: 0,
            style: Style::default(),
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
    /// Handle of the texture the pass writes.
    handle: Option<Handle<Texture>>,
    /// A view of it, kept so the pass can be pointed at it without going through the asset
    /// store every frame.
    view: Option<wgpu::TextureView>,
    /// Its current size.
    size: (u32, u32),
    /// The sprite showing it.
    sprite: Option<Entity>,
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

/// Build the pipeline, once, on the first frame that has a device.
///
/// Not a startup system: the GPU does not exist until the window does, and the window does not
/// exist until the event loop has run once.
fn ensure_renderer(
    mut commands: Commands,
    gpu: Option<Res<GpuContext>>,
    mut screen: ResMut<Screen>,
    mut textures: ResMut<Assets<Texture>>,
    mut sprites: Query<&mut Sprite>,
    window: Res<WindowInfo>,
) {
    let Some(gpu) = gpu else { return };
    if screen.renderer.is_none() {
        screen.renderer = Some(Renderer::new(&gpu.device));
        screen.slab = Some(Slab::boxed());
    }
    if window.width == 0 || window.height == 0 {
        return;
    }
    let size = (window.width, window.height);
    if screen.size == size && screen.handle.is_some() {
        return;
    }
    screen.size = size;

    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("moebius2 frame"),
        size: wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: OUTPUT_FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let wrapped = Texture {
        texture,
        view: view.clone(),
        width: size.0,
        height: size.1,
    };
    match screen.handle {
        Some(handle) => textures.replace(handle, wrapped),
        None => screen.handle = Some(textures.insert_with_path("<moebius2>", wrapped)),
    }
    screen.view = Some(view);

    let handle = screen.handle.expect("just set");
    let extent = vec2(size.0 as f32, size.1 as f32);
    match screen
        .sprite
        .and_then(|entity| sprites.get_mut(entity).ok())
    {
        Some(mut sprite) => sprite.custom_size = Some(extent),
        None => {
            screen.sprite = Some(
                commands
                    .spawn((
                        Sprite::new(handle).with_size(extent).with_z(1.0),
                        Transform2D::default(),
                    ))
                    .id(),
            );
        }
    }
}

/// Draw the sky: build this frame's circles, work out the uniforms and submit the pass.
fn draw(
    gpu: Option<Res<GpuContext>>,
    weather: Res<Weather>,
    painter: Res<Painter>,
    window: Res<WindowInfo>,
    time: Res<Time>,
    mut screen: ResMut<Screen>,
) {
    let Some(gpu) = gpu else { return };
    if window.width == 0 || window.height == 0 {
        return;
    }
    // Eased, because a readout that flickers between 5 and 9 tells you less than one that
    // settles on 7.
    screen.pace += 0.06 * (time.frame_delta * 1000.0 - screen.pace);

    screen.sky.build(weather.clock, painter.style);
    let Screen {
        renderer: Some(renderer),
        slab: Some(slab),
        view: Some(view),
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
        painter.style,
        (window.width, window.height),
        slab,
    );
    renderer.draw(&gpu.device, &gpu.queue, &uniforms, slab, view);
    screen.groups = uniforms.counts[0] as usize;
}

/// The keys that change the drawing rather than the weather.
///
/// `Z` and `X` thin and thicken the line around a cloud, and they run while held rather than
/// stepping, since the useful range is a couple of pixels wide and the difference worth seeing
/// is a tenth of one. Everything else here steps: `N` and `M` set how many arcs an element is
/// built from, `P` and `O` walk the palettes, `H` puts the readout away and `F11` toggles
/// fullscreen.
///
/// The stepping keys are debounced against the previous frame rather than using `just_pressed`,
/// since a frame system can see the same tick's edge twice.
fn painter_controls(
    input: Res<Input>,
    time: Res<Time>,
    mut painter: ResMut<Painter>,
    window: Option<Res<WindowHandle>>,
    mut held: Local<[bool; 6]>,
) {
    let delta = time.frame_delta;
    if input.pressed(Key::Z) {
        painter.style.cloud_ink = (painter.style.cloud_ink - INK_RATE * delta).max(INK_MIN);
    }
    if input.pressed(Key::X) {
        painter.style.cloud_ink = (painter.style.cloud_ink + INK_RATE * delta).min(INK_MAX);
    }

    let down = [
        input.pressed(Key::N),
        input.pressed(Key::M),
        input.pressed(Key::P),
        input.pressed(Key::O),
        input.pressed(Key::H),
        input.pressed(Key::F11),
    ];
    if down[0] && !held[0] {
        painter.style.arcs = painter.style.arcs.saturating_sub(1).max(ARCS_MIN);
    }
    if down[1] && !held[1] {
        painter.style.arcs = (painter.style.arcs + 1).min(ARCS_MAX);
    }
    if down[2] && !held[2] {
        painter.palette = (painter.palette + 1) % LOOKS.len();
    }
    if down[3] && !held[3] {
        painter.palette = (painter.palette + LOOKS.len() - 1) % LOOKS.len();
    }
    if down[4] && !held[4] {
        painter.readout = !painter.readout;
    }
    if down[5]
        && !held[5]
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

    let look = &LOOKS[painter.palette % LOOKS.len()];
    let title = format!(
        "MOEBIUS 2  {}  ({} of {})",
        look.name,
        painter.palette % LOOKS.len() + 1,
        LOOKS.len()
    );
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
         line {:.1} px   {} arcs an element\n\
         up/down pace  left/right look  Space hold  Z/X line  N/M arcs  P/O palette  H hide  F11 window",
        window.width,
        window.height,
        screen.groups,
        weather.clock,
        screen.pace,
        painter.style.cloud_ink,
        painter.style.arcs,
    );
}

fn main() {
    env_logger::init();
    Fulcrum::with_config(FulcrumConfig {
        title: "Moebius 2".into(),
        window_size: (1600, 1000),
        clear_color: Color::rgb(0.0, 0.0, 0.0),
        ..Default::default()
    })
    .insert_resource(assets!())
    .with_plugin(DefaultPlugins)
    .with_plugin(GamePlugin)
    .insert_resource(Painter::default())
    .insert_resource(Screen::default())
    .add_startup(setup)
    .add_frame_system(fit_window)
    .add_frame_system(painter_controls)
    // Chained: the second of these draws with what the first builds.
    .add_frame_system((ensure_renderer, draw).chain())
    .add_frame_system(readout)
    .run();
}
