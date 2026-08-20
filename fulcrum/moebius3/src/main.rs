//! Moebius, windowed: the keys, the readout and the frame the pass draws into.
//!
//! `cargo run -p moebius3 --release`
//!
//! - `up` and `down` set the pace, `left` and `right` turn your head, `Space` holds the sky
//! - `Z` and `X` set the weight of the line around a cloud
//! - `N` and `M` set how many arcs an element laid over a cloud is built from
//! - `S` turns the hatching on the shaded side of a cloud on and off, `V` and `B` space it
//! - `P` and `O` walk the twenty palettes, `H` puts the readout away, `F11` leaves fullscreen
//!
//! The window opens borderless over the whole display and the drawing is computed at **one
//! sample per physical pixel**. The engine never draws the sky; it draws one sprite, and the
//! sprite is the texture the pass wrote.

use fulcrum::prelude::*;
use fulcrum_render::{GpuContext, WhitePixel, WindowHandle};
use moebius3::cloud::{ARCS_MAX, ARCS_MIN, HATCH_MAX, HATCH_MIN, INK_MAX, INK_MIN, Sky, Style};
use moebius3::game::{GamePlugin, Weather};
use moebius3::look::LOOKS;
use moebius3::sky::{self, OUTPUT_FORMAT, Renderer, Slab};
use simulacra_assets::assets;
use simulacra_frame::{Frame, FramePlugin, fit_frame};

/// Readout text height in world units, before HiDPI scaling.
const READOUT_SIZE: f32 = 8.0;
/// Gap between the readout and the corner of the window.
const READOUT_MARGIN: f32 = 12.0;
/// Size of the readout's backing panel, in world units, before HiDPI scaling.
///
/// The font is fixed-pitch and square, so a panel this wide holds ninety-four characters and a
/// panel this tall holds nine lines. The keys are on two lines because one line long enough to
/// hold them all would need a panel across half the picture, and the panel is there to be read
/// on rather than to be looked at.
const PANEL: Vec2 = Vec2::new(752.0, 76.0);
/// How many characters wide the readout is written to be.
const LINE_WIDTH: usize = 87;

/// How many pixels the cloud line changes by per second of holding the key. Slow enough to stop
/// on a number, since the difference between 1.6 and 2.2 is the difference between a drawing and
/// a woodcut.
const INK_RATE: f32 = 2.2;

/// How much the hatch spacing is multiplied by per second of holding the key.
const HATCH_RATE: f32 = 3.0;

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

    screen.sky.build(weather.clock, painter.style);
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
        painter.style,
        window,
        slab,
    );
    renderer.draw(&gpu.device, &gpu.queue, &uniforms, slab, view, window);
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
    mut held: Local<[bool; 7]>,
) {
    let delta = time.frame_delta;
    if input.pressed(Key::Z) {
        painter.style.cloud_ink = (painter.style.cloud_ink - INK_RATE * delta).max(INK_MIN);
    }
    if input.pressed(Key::X) {
        painter.style.cloud_ink = (painter.style.cloud_ink + INK_RATE * delta).min(INK_MAX);
    }
    // The hatching runs on a ratio rather than on a step, so that one press moves it by the same
    // proportion wherever it is: the difference between a twentieth and a fifteenth is a lot of
    // lines, and the difference between a fifth and a quarter is one.
    let spread = HATCH_RATE.powf(delta);
    if input.pressed(Key::V) {
        painter.style.hatch = (painter.style.hatch * spread).min(HATCH_MAX);
    }
    if input.pressed(Key::B) {
        painter.style.hatch = (painter.style.hatch / spread).max(HATCH_MIN);
    }

    let down = [
        input.pressed(Key::N),
        input.pressed(Key::M),
        input.pressed(Key::P),
        input.pressed(Key::O),
        input.pressed(Key::S),
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
        painter.style.shade = !painter.style.shade;
    }
    if down[5] && !held[5] {
        painter.readout = !painter.readout;
    }
    if down[6]
        && !held[6]
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

    text.value = readout_text(
        &weather,
        &painter,
        (window.width, window.height),
        screen.groups,
        screen.pace,
    );
}

/// The readout, as five lines of text.
///
/// Split out of the system above so that a test can measure it. The font is fixed-pitch, and the
/// panel behind the text is a fixed size, so a line that grows past [`LINE_WIDTH`] characters
/// hangs off the end of its own background and is read against whatever sky is behind it. That
/// has happened twice while these pieces were being written, both times by adding a key.
fn readout_text(
    weather: &Weather,
    painter: &Painter,
    window: (u32, u32),
    outlines: usize,
    frame_ms: f32,
) -> String {
    let look = &LOOKS[painter.palette % LOOKS.len()];
    let title = format!(
        "MOEBIUS 3  {}  ({} of {})",
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
    let pixels = grouped((window.0 as u64) * (window.1 as u64));
    let shade = if painter.style.shade {
        format!(
            "hatched every {:.0}th of a radius",
            1.0 / painter.style.hatch
        )
    } else {
        "unshaded".to_string()
    };
    format!(
        "{title}{pace:>width$}\n\
         drawn once per pixel: {}x{} = {pixels}   {outlines} outlines   {:.0} s of weather   {frame_ms:.1} ms\n\
         line {:.1} px   {} arcs an element   {shade}\n\
         up/down pace   left/right look   Space hold   P/O palette   H hide   F11 window\n\
         Z/X line   N/M arcs   S shade on and off   V/B how far apart the hatching runs",
        window.0, window.1, weather.clock, painter.style.cloud_ink, painter.style.arcs,
    )
}

fn main() {
    env_logger::init();
    Fulcrum::with_config(FulcrumConfig {
        title: "Moebius 3".into(),
        window_size: (1600, 1000),
        clear_color: Color::rgb(0.0, 0.0, 0.0),
        ..Default::default()
    })
    .insert_resource(assets!())
    .with_plugin(DefaultPlugins)
    .with_plugin(FramePlugin::new("moebius3", OUTPUT_FORMAT))
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

#[cfg(test)]
mod tests {
    use super::*;
    use moebius3::cloud::{ARCS_MAX, HATCH_MIN, INK_MAX};

    /// How wide one character is, and how far apart two lines are, as a multiple of the text
    /// size. The readout is set in PressStart2P, which is fixed-pitch and square: at eight units
    /// a character is eight units wide and a line is eight units high, measured off the font
    /// rather than assumed.
    const CHARACTER: f32 = 1.0;

    #[test]
    fn the_readout_fits_on_the_panel_behind_it() {
        // The worst case the piece can reach: the longest palette name, a display nobody has yet,
        // a clock that has been running for a day, and every setting at the end of its range that
        // spells longest.
        let widest = LOOKS
            .iter()
            .enumerate()
            .max_by_key(|(_, look)| look.name.len())
            .map(|(index, _)| index)
            .expect("there is at least one palette");
        let painter = Painter {
            palette: widest,
            style: Style {
                arcs: ARCS_MAX,
                cloud_ink: INK_MAX,
                shade: true,
                hatch: HATCH_MIN,
            },
            readout: true,
        };
        let weather = Weather {
            clock: 86_400.0,
            pace: 32.0,
            ..Default::default()
        };
        let text = readout_text(&weather, &painter, (7680, 4320), 384, 999.9);

        let lines: Vec<&str> = text.split('\n').collect();
        let longest = lines
            .iter()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0);
        assert!(
            longest as f32 * CHARACTER * READOUT_SIZE <= PANEL.x,
            "the readout is {} units wide and its panel is {}",
            longest as f32 * CHARACTER * READOUT_SIZE,
            PANEL.x
        );
        assert!(
            lines.len() as f32 * READOUT_SIZE <= PANEL.y,
            "the readout is {} lines and its panel holds {}",
            lines.len(),
            (PANEL.y / READOUT_SIZE) as usize
        );
        // And the panel is not a black bar across a picture it is only there to be read on.
        const { assert!(PANEL.x < 900.0, "the panel has grown into the drawing") };
    }

    #[test]
    fn every_key_is_in_the_readout() {
        // The list at the bottom is the only place the keys are written down where somebody
        // running the piece can see them, so a key added without a line here is a key nobody
        // finds.
        let text = readout_text(
            &Weather::default(),
            &Painter::default(),
            (1600, 1000),
            54,
            13.3,
        );
        for key in [
            "up/down",
            "left/right",
            "Space",
            "P/O",
            "H ",
            "F11",
            "Z/X",
            "N/M",
            "S ",
            "V/B",
        ] {
            assert!(text.contains(key), "the readout never mentions {key}");
        }
    }
}
