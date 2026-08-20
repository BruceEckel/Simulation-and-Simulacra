//! Life, windowed: everything you can see. The rule lives in `game.rs` and has no opinion
//! about colour; this binary is the half that does.
//!
//! `cargo run -p life --release`
//!
//! - `N` and `M` walk the forty-four rules, `Tab` jumps to the next family
//! - `1`-`0` choose how the field is started, `R` starts it again, `C` empties it
//! - `Space` holds it, `S` takes one generation at a time, `up`/`down` set the pace
//! - `Z` and `X` set the resolution, from sixty-four pixels a cell down to one
//! - `O` and `P` walk the twelve colour schemes, `A`, `G` and `E` change how it is read
//! - the mouse draws: left fills cells, right empties them
//! - `H` puts the readout away, `F11` goes fullscreen and back
//!
//! It opens in an ordinary window. `F11` takes the whole display with no border, and `F11`
//! again gives the window back; the field goes on running either way, and the pattern on it
//! survives the change of size.

use fulcrum::prelude::*;
use fulcrum_render::{GlyphCache, GpuContext, WhitePixel, WindowHandle};
use life::game::{self, Board, Dials, GamePlugin};
use life::look::LOOKS;
use life::rules::RULES;
use life::screen::{OUTPUT_FORMAT, Reading, Renderer, compose};
use simulacra_assets::assets;
use simulacra_frame::{Frame, FramePlugin, fit_frame};

/// Readout text height in world units, before HiDPI scaling. The built-in font is sharpest at
/// multiples of 8.
const READOUT_SIZE: f32 = 8.0;
/// Gap between the readout and the corner of the window.
const READOUT_MARGIN: f32 = 12.0;
/// How much wider and taller the readout's backing panel is than the text on it, in physical
/// pixels.
///
/// The panel is *measured* rather than fixed at a size, which is not what the other pieces here
/// do. They can fix it because their readouts are the same shape every frame with only the
/// numbers changing. This one is not: it names the rule and prints a line about it, and a line
/// about Gnarl is not the length of a line about Bosco's rule. Measuring is the difference
/// between a panel that fits and a panel that has to be drawn big enough for the worst case and
/// therefore looks wrong for all the others.
const PANEL_PAD: Vec2 = Vec2::new(20.0, 14.0);
/// What the panel is before the font has loaded and there is anything to measure.
const PANEL_GUESS: Vec2 = Vec2::new(776.0, 124.0);
/// How many characters wide the readout is written to be. Only used to push the note about
/// which rule this is out to the right-hand end of the title line.
const LINE_WIDTH: usize = 88;

/// How quickly the two measured numbers settle, per frame. A readout that flickers between
/// five and nine tells you less than one that settles on seven.
const EASE: f32 = 0.06;

/// Everything that is a matter of taste rather than of the rule.
#[derive(Resource, Default)]
struct Painter {
    /// Which of [`LOOKS`] is in use.
    palette: usize,
    /// How the field is read: age, trails, cell edges.
    reading: Reading,
    /// Whether the readout is shown.
    readout: bool,
    /// Frame time, eased, in milliseconds.
    frame: f32,
    /// Generations a second actually achieved, eased.
    rate: f32,
    /// The generation the last frame saw, for measuring that.
    seen: u64,
    /// How big the readout's panel needs to be, in physical pixels, as the last frame measured
    /// it. Read by `fit_window`, which puts the panel and the text where it says.
    panel: Vec2,
}

/// Marks the readout text.
#[derive(Component)]
struct Readout;

/// Marks the panel that keeps the readout legible over a bright field.
#[derive(Component)]
struct Panel;

/// Put the readout up. The window is left alone: this one opens as an ordinary window, and
/// `F11` is what takes the display.
fn setup(mut commands: Commands, white: Option<Res<WhitePixel>>) {
    let white = white.map(|white| white.0).unwrap_or(Handle::INVALID);
    commands.spawn((
        Panel,
        Sprite::new(white)
            .with_size(PANEL_GUESS)
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

/// Tell the simulation how big the window is, and put the readout in the corner of whatever
/// shape it has ended up.
///
/// The size goes over the replayable command channel rather than being read out of the
/// renderer, which is what lets a headless run reshape the field exactly as a windowed one
/// does — and is why going fullscreen is an ordinary resize as far as the rule is concerned.
fn fit_window(
    window: Res<WindowInfo>,
    painter: Res<Painter>,
    mut outbox: ResMut<CommandOutbox>,
    mut requested: Local<Option<Vec2>>,
    mut readouts: Query<&mut Transform2D, With<Readout>>,
    mut panels: Query<&mut Transform2D, (With<Panel>, Without<Readout>)>,
) {
    let size = vec2(window.width as f32, window.height as f32);
    if size.x < 1.0 || size.y < 1.0 {
        return; // minimized
    }
    if *requested != Some(size) {
        outbox.send(game::RESIZE_COMMAND, game::window_payload(size));
        *requested = Some(size);
    }

    let scale = window.scale_factor.round().max(1.0);
    let panel = if painter.panel.y > 0.0 {
        painter.panel
    } else {
        PANEL_GUESS * scale
    };
    let corner = vec2(
        -size.x / 2.0 + READOUT_MARGIN * scale,
        -size.y / 2.0 + panel.y + READOUT_MARGIN * scale,
    );
    for mut readout in &mut readouts {
        readout.translation = corner;
    }
    // The text hangs down and to the right of its corner; the panel is centred on that block,
    // and sits half its padding up and left of where the text starts.
    for mut transform in &mut panels {
        transform.translation = corner
            + vec2(
                panel.x / 2.0 - PANEL_PAD.x * scale / 2.0,
                -panel.y / 2.0 + PANEL_PAD.y * scale / 2.0,
            );
    }
}

/// The one thing left that is this piece's own: the pipeline, built on the first frame that
/// has a device. The texture it draws into, and the sprite showing it, are `simulacra-frame`'s
/// — see that crate for why they are not here.
#[derive(Resource, Default)]
struct Pass(Option<Renderer>);

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

/// Carry this generation to the GPU if it is not up there already, and draw it.
fn draw(
    gpu: Option<Res<GpuContext>>,
    board: Res<Board>,
    dials: Res<Dials>,
    painter: Res<Painter>,
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
    let uniforms = compose(
        &board,
        dials.cell(),
        &LOOKS[painter.palette % LOOKS.len()],
        painter.reading,
        frame.window(),
    );
    renderer.carry(&gpu.device, &gpu.queue, &board);
    renderer.draw(&gpu.device, &gpu.queue, &uniforms, view, frame.window());
}

/// The keys that change nothing about the rule: the palette, how the field is read, whether the
/// readout is up, and whether this is a window or the whole display.
///
/// Debounced against the previous frame rather than using `just_pressed`, since a frame system
/// can see the same tick's edge twice.
fn painter_controls(
    input: Res<Input>,
    mut painter: ResMut<Painter>,
    window: Option<Res<WindowHandle>>,
    mut held: Local<[bool; 7]>,
) {
    let down = [
        input.pressed(Key::P),
        input.pressed(Key::O),
        input.pressed(Key::A),
        input.pressed(Key::G),
        input.pressed(Key::E),
        input.pressed(Key::H),
        input.pressed(Key::F11),
    ];
    let looks = LOOKS.len();
    if down[0] && !held[0] {
        painter.palette = (painter.palette + 1) % looks;
    }
    if down[1] && !held[1] {
        painter.palette = (painter.palette + looks - 1) % looks;
    }
    if down[2] && !held[2] {
        painter.reading.ageing = !painter.reading.ageing;
    }
    if down[3] && !held[3] {
        painter.reading.ghosts = !painter.reading.ghosts;
    }
    if down[4] && !held[4] {
        painter.reading.edges = !painter.reading.edges;
    }
    if down[5] && !held[5] {
        painter.readout = !painter.readout;
    }
    // Fullscreen is a window operation and nothing else: the event loop keeps running, the
    // fixed tick keeps firing, and the only thing the simulation hears about it is the resize
    // that `fit_window` sends a moment later. The field carries straight on.
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

/// Measure what the field is actually managing, as against what the pace was set to.
///
/// Worth showing separately, because at one cell to the pixel on a large display the two come
/// apart: a generation is tens of millions of cell updates, and asking for four hundred a
/// second does not make the machine able to do them.
fn measure(time: Res<Time>, board: Res<Board>, mut painter: ResMut<Painter>) {
    painter.frame += EASE * (time.frame_delta * 1000.0 - painter.frame);
    let taken = board.generation.saturating_sub(painter.seen) as f32;
    painter.seen = board.generation;
    let instant = if time.frame_delta > 0.0 {
        taken / time.frame_delta
    } else {
        0.0
    };
    painter.rate += EASE * (instant - painter.rate);
}

/// Thousands separators, because at one cell to the pixel the cell count is half the boast.
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

/// What the field is doing, and how to lean on it.
#[expect(clippy::too_many_arguments, reason = "a readout reads everything")]
fn readout(
    board: Res<Board>,
    dials: Res<Dials>,
    mut painter: ResMut<Painter>,
    window: Res<WindowInfo>,
    fonts: Res<Assets<Font>>,
    default_font: Option<Res<DefaultFont>>,
    mut texts: Query<&mut Text, With<Readout>>,
    mut panels: Query<&mut Sprite, With<Panel>>,
) {
    let Ok(mut text) = texts.single_mut() else {
        return;
    };
    let scale = window.scale_factor.round().max(1.0);
    text.size = READOUT_SIZE * scale;
    for mut panel in &mut panels {
        panel.color.a = if painter.readout { 0.66 } else { 0.0 };
    }
    if !painter.readout {
        text.value = String::new();
        return;
    }

    let rule = dials.rule();
    let title = format!("LIFE  {}  {}", rule.name, rule.rulestring());
    let place = format!("{}/{} {}", dials.rule + 1, RULES.len(), rule.family.name());
    let width = LINE_WIDTH.saturating_sub(title.chars().count());

    let area = board.area().max(1);
    let percent = 100.0 * board.population as f32 / area as f32;
    let motion = match (board.population, board.period) {
        (0, _) => "empty".to_string(),
        (_, Some(1)) => "still".to_string(),
        (_, Some(period)) => format!("period {period}"),
        (_, None) => "changing".to_string(),
    };
    let pacing = if dials.running {
        format!(
            "{:.2} gen/s asked, {:.1} achieved",
            dials.pace, painter.rate
        )
    } else {
        "HELD".to_string()
    };

    text.value = format!(
        "{title}{place:>width$}\n\
         {}\n\
         \n\
         generation {}   population {} ({percent:.1}%)   +{} -{}   {motion}\n\
         {} x {} = {} cells at {} px   sown: {}   {}{}\n\
         {pacing}   {:.1} ms a frame{}\n\
         \n\
         N/M rule   Tab family   1-0 how it starts   R sow   C clear   Space hold   S step\n\
         Z/X cells   up/down pace   O/P colours   A age   G ghosts   E edges   T torus\n\
         K self-restart   H hide   F11 window   left mouse draws, right mouse erases",
        rule.blurb,
        grouped(board.generation),
        grouped(u64::from(board.population)),
        board.births,
        board.deaths,
        board.width,
        board.height,
        grouped(area as u64),
        dials.cell(),
        dials.start.name(),
        LOOKS[painter.palette % LOOKS.len()].name,
        readings(&painter, dials.wrap),
        painter.frame,
        if dials.restart {
            "   sows itself again when it settles"
        } else {
            ""
        },
    );

    // Measured with the engine's own metrics, on the text that is actually about to be drawn,
    // so the panel fits whatever the longest line of this rule's description turned out to be.
    let measured = default_font
        .and_then(|handle| fonts.get(handle.0).map(|font| (handle.0, font)))
        .map(|(_, font)| GlyphCache::measure(font, &text.value, text.size));
    if let Some(measured) = measured {
        painter.panel = measured + PANEL_PAD * scale;
    }
    let panel = if painter.panel.y > 0.0 {
        painter.panel
    } else {
        PANEL_GUESS * scale
    };
    for mut sprite in &mut panels {
        sprite.custom_size = Some(panel);
    }
}

/// The switches that are on, written out, so the readout says why the picture looks like that.
fn readings(painter: &Painter, wrap: bool) -> String {
    let mut on = Vec::new();
    if painter.reading.ageing {
        on.push("age");
    }
    if painter.reading.ghosts {
        on.push("ghosts");
    }
    if painter.reading.edges {
        on.push("edges");
    }
    on.push(if wrap { "torus" } else { "walled" });
    format!(", {}", on.join(", "))
}

fn main() {
    env_logger::init();
    Fulcrum::with_config(FulcrumConfig {
        title: "Life".into(),
        window_size: (game::DEFAULT_WINDOW.x as u32, game::DEFAULT_WINDOW.y as u32),
        clear_color: Color::rgb(0.0, 0.0, 0.0),
        ..Default::default()
    })
    .insert_resource(assets!())
    .with_plugin(DefaultPlugins)
    .with_plugin(FramePlugin::new("life", OUTPUT_FORMAT))
    .with_plugin(GamePlugin)
    .insert_resource(Painter {
        readout: true,
        ..Default::default()
    })
    .insert_resource(Pass::default())
    .add_startup(setup)
    .add_frame_system(fit_window)
    .add_frame_system(painter_controls)
    .add_frame_system(measure)
    // Chained: the second draws with what the first builds, and both come after the shared
    // frame system, which is what decides the texture they are drawing into.
    .add_frame_system((ensure_renderer, draw).chain().after(fit_frame))
    .add_frame_system(readout)
    .run();
}
