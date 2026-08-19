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

/// The pass, the texture it draws into, and the sprite that shows it.
#[derive(Resource, Default)]
struct Frame {
    /// Built on the first frame, once there is a device to build it against.
    renderer: Option<Renderer>,
    /// Handle of the texture the pass writes.
    handle: Option<Handle<Texture>>,
    /// A view of it, kept so the pass can be pointed at it without going through the asset
    /// store every frame.
    view: Option<wgpu::TextureView>,
    /// Its current size.
    size: (u32, u32),
    /// The sprite showing it.
    sprite: Option<Entity>,
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

/// Build the pipeline and the frame it draws into, once there is a device and a window size.
///
/// Not a startup system: the GPU does not exist until the window does, and the window does not
/// exist until the event loop has run once.
fn ensure_renderer(
    mut commands: Commands,
    gpu: Option<Res<GpuContext>>,
    mut frame: ResMut<Frame>,
    mut textures: ResMut<Assets<Texture>>,
    mut sprites: Query<&mut Sprite>,
    window: Res<WindowInfo>,
) {
    let Some(gpu) = gpu else { return };
    if frame.renderer.is_none() {
        frame.renderer = Some(Renderer::new(&gpu.device));
    }
    if window.width == 0 || window.height == 0 {
        return;
    }
    let size = (window.width, window.height);
    if frame.size == size && frame.handle.is_some() {
        return;
    }
    frame.size = size;

    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("life frame"),
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
    match frame.handle {
        Some(handle) => textures.replace(handle, wrapped),
        None => frame.handle = Some(textures.insert_with_path("<life>", wrapped)),
    }
    frame.view = Some(view);

    let handle = frame.handle.expect("just set");
    let extent = vec2(size.0 as f32, size.1 as f32);
    match frame.sprite.and_then(|entity| sprites.get_mut(entity).ok()) {
        Some(mut sprite) => sprite.custom_size = Some(extent),
        None => {
            frame.sprite = Some(
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

/// Carry this generation to the GPU if it is not up there already, and draw it.
fn draw(
    gpu: Option<Res<GpuContext>>,
    board: Res<Board>,
    dials: Res<Dials>,
    painter: Res<Painter>,
    window: Res<WindowInfo>,
    mut frame: ResMut<Frame>,
) {
    let Some(gpu) = gpu else { return };
    if window.width == 0 || window.height == 0 {
        return;
    }
    let uniforms = compose(
        &board,
        dials.cell(),
        &LOOKS[painter.palette % LOOKS.len()],
        painter.reading,
        (window.width, window.height),
    );
    let Frame {
        renderer: Some(renderer),
        view: Some(view),
        ..
    } = &mut *frame
    else {
        return;
    };
    renderer.carry(&gpu.device, &gpu.queue, &board);
    renderer.draw(&gpu.device, &gpu.queue, &uniforms, view);
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
    .with_plugin(GamePlugin)
    .insert_resource(Painter {
        readout: true,
        ..Default::default()
    })
    .insert_resource(Frame::default())
    .add_startup(setup)
    .add_frame_system(fit_window)
    .add_frame_system(painter_controls)
    .add_frame_system(measure)
    // Chained: the second of these draws with what the first builds.
    .add_frame_system((ensure_renderer, draw).chain())
    .add_frame_system(readout)
    .run();
}
