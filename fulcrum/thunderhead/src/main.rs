//! Thunderhead, windowed: the colours, the readout and the keys. The desert and the weather
//! live in `game.rs` and have no opinion about any of them.
//!
//! `cargo run -p thunderhead --release`
//!
//! - `up` and `down` set the pace, `Space` holds the sky still
//! - `P` changes palette, `H` puts the readout away, `F11` leaves fullscreen
//!
//! The window opens borderless over the whole display and the picture is computed at **one
//! cell per physical pixel**: every frame, every pixel of the monitor is looked up through a
//! table of sixty-six colours and uploaded as one texture on one full-screen sprite. There is no
//! sprite per cloud, no shader and no scaling anywhere in the piece, which is what keeps the
//! ink line one pixel wide on a wall-sized display.

use fulcrum::prelude::*;
use fulcrum_render::{GpuContext, WhitePixel, WindowHandle};
use simulacra_assets::assets;
use simulacra_frame::{Frame, FramePlugin, fit_frame};
use thunderhead::game::{
    self, DRIFTERS, Field, GamePlugin, Motion, RESIZE_COMMAND, Sky, window_payload,
};
use thunderhead::look::{LOOKS, lut};

/// What the picture is written in: sRGB, so the palette's bytes go in untouched and the GPU
/// handles both ends of the linear-light conversion.
const FRAME_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// Readout text height in world units, before HiDPI scaling. The built-in pixel font is
/// sharpest at multiples of 8.
const READOUT_SIZE: f32 = 8.0;
/// Gap between the readout and the corner of the window.
const READOUT_MARGIN: f32 = 12.0;
/// Size of the readout's backing panel, in world units, before HiDPI scaling.
const PANEL: Vec2 = Vec2::new(672.0, 64.0);
/// How many characters wide the readout is written to be. Only used to push the pace out to
/// the right-hand end of the title line.
const LINE_WIDTH: usize = 78;

/// How much bigger the readout draws on this screen. World units are physical pixels here, so
/// on a HiDPI display an unscaled readout comes out ant-sized; whole-number steps keep the
/// pixel font on the pixel grid.
fn readout_scale(window: &WindowInfo) -> f32 {
    window.scale_factor.round().max(1.0)
}

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

/// The one texture the whole picture is uploaded through, and the buffers that feed it.
#[derive(Resource)]
struct Screen {
    /// One RGBA word per pixel, rebuilt every frame. Kept allocated between frames: at one
    /// cell per pixel this is sixteen megabytes, not something to allocate at 60 Hz.
    pixels: Vec<u32>,
    /// Every material's colour under the current palette, as packed RGBA. The frame loop must
    /// be a table lookup and nothing else.
    table: [u32; 256],
    /// Which palette the table was built for.
    table_for: Option<usize>,
}

impl Default for Screen {
    fn default() -> Self {
        Self {
            pixels: Vec::new(),
            table: [0; 256],
            table_for: None,
        }
    }
}

/// Marks the readout text.
#[derive(Component)]
struct Readout;

/// Marks the panel that keeps the readout legible over a bright sky.
#[derive(Component)]
struct Panel;

/// Go borderless over the whole display and put the readout up.
///
/// Fullscreen is the opening state rather than an option on a key, for the same reason the
/// picture is computed per pixel: a piece about enormous things wants the whole wall.
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

/// Tell the simulation what size the screen really is, and keep the readout in the corner of
/// whatever shape that turns out to be.
fn fit_window(
    window: Res<WindowInfo>,
    mut outbox: ResMut<CommandOutbox>,
    mut requested: Local<Option<(u32, u32)>>,
    mut readouts: Query<&mut Transform2D, With<Readout>>,
    mut panels: Query<&mut Transform2D, (With<Panel>, Without<Readout>)>,
) {
    if window.width == 0 || window.height == 0 {
        return; // minimized
    }
    let size = (window.width, window.height);
    if *requested != Some(size) {
        outbox.send(RESIZE_COMMAND, window_payload(size.0, size.1));
        *requested = Some(size);
    }
    let scale = readout_scale(&window);
    // The readout sits at the bottom, where the desert is: the whole point of the frame is the
    // sky, and text across the sky would be text across the piece.
    let corner = vec2(
        -(window.width as f32) / 2.0 + READOUT_MARGIN * scale,
        -(window.height as f32) / 2.0 + (PANEL.y + READOUT_MARGIN) * scale,
    );
    for mut readout in &mut readouts {
        readout.translation = corner;
    }
    // The text hangs down and to the right of its corner; the panel is centred on that block.
    let panel = PANEL * scale;
    for mut transform in &mut panels {
        transform.translation = corner
            + vec2(
                panel.x / 2.0 - READOUT_MARGIN * scale,
                -panel.y / 2.0 + 6.0 * scale,
            );
    }
}

/// Colour every pixel and upload the picture: the whole field through the table into one
/// texture on one full-screen sprite, every frame.
///
/// The texture's format is sRGB, so the palette's sRGB bytes go in untouched and the GPU
/// handles both ends of the linear-light conversion.
fn paint(
    field: Res<Field>,
    painter: Res<Painter>,
    gpu: Option<Res<GpuContext>>,
    frame: Res<Frame>,
    textures: Res<Assets<Texture>>,
    mut screen: ResMut<Screen>,
) {
    let Some(gpu) = gpu else { return };
    if field.cells.is_empty() || !frame.ready() {
        return;
    }

    if screen.table_for != Some(painter.palette) {
        screen.table_for = Some(painter.palette);
        let colours = lut(&LOOKS[painter.palette % LOOKS.len()]);
        for (slot, colour) in screen.table.iter_mut().zip(colours) {
            *slot = u32::from_le_bytes(colour);
        }
    }

    let Screen {
        ref mut pixels,
        ref table,
        ..
    } = *screen;
    pixels.resize(field.cells.len(), 0);
    for (pixel, &cell) in pixels.iter_mut().zip(&field.cells) {
        *pixel = table[cell as usize];
    }

    // Into the top-left corner of the shared frame, which is usually bigger than this: the
    // extent given is the field's, not the texture's, so the rest of it is never touched.
    let Some(handle) = frame.handle() else { return };
    let Some(texture) = textures.get(handle) else {
        return;
    };
    gpu.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture.texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        bytemuck::cast_slice(&screen.pixels),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * field.width),
            rows_per_image: Some(field.height),
        },
        wgpu::Extent3d {
            width: field.width,
            height: field.height,
            depth_or_array_layers: 1,
        },
    );
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

/// Thousands separators, because the pixel count is half the piece's boast and it should be
/// legible.
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
    field: Res<Field>,
    sky: Res<Sky>,
    motion: Res<Motion>,
    painter: Res<Painter>,
    window: Res<WindowInfo>,
    mut texts: Query<&mut Text, With<Readout>>,
    mut panels: Query<&mut Sprite, With<Panel>>,
) {
    let Ok(mut text) = texts.single_mut() else {
        return;
    };
    let scale = readout_scale(&window);
    text.size = READOUT_SIZE * scale;
    for mut panel in &mut panels {
        panel.custom_size = Some(PANEL * scale);
        panel.color.a = if painter.readout { 0.66 } else { 0.0 };
    }
    if !painter.readout {
        text.value = String::new();
        return;
    }

    let pixels = grouped(field.cells.len() as u64);
    let title = format!("THUNDERHEAD  {}", LOOKS[painter.palette % LOOKS.len()].name);
    let pace = if motion.held {
        "HELD".to_string()
    } else {
        format!("pace x{:.2}", motion.pace)
    };
    let width = LINE_WIDTH.saturating_sub(title.chars().count());
    text.value = format!(
        "{title}{pace:>width$}\n\
         every pixel: {}x{} = {pixels}   clouds {DRIFTERS} of {} texels, {} grown\n\
         up/down pace   Space hold   P palette   H hide   F11 window",
        field.width,
        field.height,
        grouped(sky.texels()),
        sky.grown,
    );
}

fn main() {
    env_logger::init();
    Fulcrum::with_config(FulcrumConfig {
        title: "Thunderhead".into(),
        window_size: game::DEFAULT_WINDOW,
        clear_color: Color::rgb(0.0, 0.0, 0.0),
        ..Default::default()
    })
    .insert_resource(assets!())
    .with_plugin(DefaultPlugins)
    .with_plugin(FramePlugin::new("thunderhead", FRAME_FORMAT))
    .with_plugin(GamePlugin)
    .insert_resource(Painter::default())
    .insert_resource(Screen::default())
    .add_startup(setup)
    .add_frame_system(fit_window)
    .add_frame_system(painter_controls)
    .add_frame_system(paint.after(fit_frame))
    .add_frame_system(readout)
    .run();
}
