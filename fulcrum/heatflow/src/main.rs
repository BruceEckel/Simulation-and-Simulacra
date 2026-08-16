//! Statistical Heat Flow, windowed: everything you can see. The gas lives in `game.rs` and
//! stays renderer-free; this binary dresses its atoms and draws the instruments.
//!
//! `cargo run -p heatflow`
//!
//! - `q`/`a` raise and lower the left surface, `e`/`d` the right (hold to ramp)
//! - `s` sets both surfaces to their average, exactly equal
//! - `c` switches the atoms between local-temperature and own-energy coloring
//! - hold `n` to add atoms, `m` to remove them
//! - `space` pauses, `up`/`down` change the simulation speed, `0` resets it
//! - `r` refills the box with a fresh uniform gas

use fulcrum::prelude::*;
use heatflow::game::{
    self, ATOM_RADIUS, Atom, Census, Court, DEFAULT_COURT, GamePlugin, Meter, PROFILE_BINS, Paused,
    Side, Speed, TEMPERATURE_MAX, Velocity, Walls, kinetic_energy, temperature_of,
};
use simulacra_assets::assets;

/// Readout text height in world units. The built-in pixel font is sharpest at multiples of 8.
const READOUT_SIZE: f32 = 8.0;
/// Gap between the readout and the box's top-left corner.
const READOUT_MARGIN: f32 = 12.0;
/// How wide the two surfaces are drawn.
const WALL_THICKNESS: f32 = 14.0;
/// Temperature the color ramp and the profile plot top out at.
const SCALE_TOP: f32 = TEMPERATURE_MAX * 0.85;
/// Fraction of the box height the profile plot spans.
const PLOT_HEIGHT: f32 = 0.86;

/// The shared white texture every rectangle here is a tinted copy of.
#[derive(Resource)]
struct White(Handle<Texture>);

/// Marks the statistics readout.
#[derive(Component)]
struct Readout;

/// Marks one of the two surface bars.
#[derive(Component)]
struct Surface(Side);

/// Marks one column of the temperature profile plot.
#[derive(Component)]
struct ProfileDot(usize);

/// Marks the backing panel that keeps the readout legible over a busy gas.
#[derive(Component)]
struct Panel;

/// Size of that panel, in world units. Fixed rather than measured: the readout's shape does
/// not change, only its numbers.
const PANEL: Vec2 = Vec2::new(470.0, 104.0);

/// Cold to hot: deep blue, cyan, yellow, red. Chosen over a hue sweep so that "hotter" reads
/// as a single direction rather than a trip around a color wheel.
fn thermal(fraction: f32) -> Color {
    let stops = [
        (0.00, [0.15, 0.25, 0.85]),
        (0.35, [0.20, 0.85, 0.95]),
        (0.65, [0.98, 0.90, 0.35]),
        (1.00, [1.00, 0.25, 0.20]),
    ];
    let position = fraction.clamp(0.0, 1.0);
    for pair in stops.windows(2) {
        let (low, high) = (pair[0], pair[1]);
        if position <= high.0 {
            let span = (high.0 - low.0).max(1e-6);
            let blend = (position - low.0) / span;
            return Color::rgb(
                low.1[0] + (high.1[0] - low.1[0]) * blend,
                low.1[1] + (high.1[1] - low.1[1]) * blend,
                low.1[2] + (high.1[2] - low.1[2]) * blend,
            );
        }
    }
    Color::rgb(1.0, 0.25, 0.2)
}

/// Load the texture and put up the instruments: the two surfaces and the profile plot's
/// columns, both of which are repositioned rather than respawned as the box changes.
fn setup(mut commands: Commands, mut assets: AssetLoader) {
    let white = assets.load("white.png");
    commands.insert_resource(White(white));
    commands.spawn((
        Panel,
        Sprite::new(white)
            .with_size(PANEL)
            .with_color(Color::rgba(0.02, 0.02, 0.04, 0.72))
            .with_z(9.0),
        Transform2D::default(),
    ));
    commands.spawn((
        Readout,
        Text::new("").with_size(READOUT_SIZE).with_z(10.0),
        Transform2D::from_xy(
            -DEFAULT_COURT.x / 2.0 + READOUT_MARGIN,
            DEFAULT_COURT.y / 2.0 - READOUT_MARGIN,
        ),
    ));
    for side in [Side::Left, Side::Right] {
        commands.spawn((
            Surface(side),
            Sprite::new(white).with_z(3.0),
            Transform2D::default(),
        ));
    }
    for column in 0..PROFILE_BINS {
        commands.spawn((
            ProfileDot(column),
            Sprite::new(white).with_size(Vec2::splat(5.0)).with_z(5.0),
            Transform2D::default(),
        ));
    }
}

/// Keep window, camera, and box in step. Same split as `boids` and `rally`: the zoom is
/// cosmetic and applied here, the resize itself is a command the simulation applies.
fn fit_window(
    window: Res<WindowInfo>,
    court: Res<Court>,
    mut camera: ResMut<Camera2D>,
    mut outbox: ResMut<CommandOutbox>,
    mut requested: Local<Option<Vec2>>,
    mut readouts: Query<&mut Transform2D, With<Readout>>,
    mut panels: Query<&mut Transform2D, (With<Panel>, Without<Readout>)>,
) {
    let size = vec2(window.width as f32, window.height as f32);
    if size.x < 1.0 || size.y < 1.0 {
        return; // minimized
    }
    let wanted = game::court_for_window(size);
    if wanted != court.0 && *requested != Some(wanted) {
        outbox.send(game::COURT_COMMAND, game::court_payload(wanted));
        *requested = Some(wanted);
    }
    camera.zoom = (size.x / court.0.x).max(size.y / court.0.y);
    let corner = vec2(
        -court.0.x / 2.0 + READOUT_MARGIN,
        court.0.y / 2.0 - READOUT_MARGIN,
    );
    for mut readout in &mut readouts {
        readout.translation = corner;
    }
    // The text hangs down and to the right of its corner; the panel is centered on that block.
    for mut panel in &mut panels {
        panel.translation = corner + vec2(PANEL.x / 2.0 - READOUT_MARGIN, -PANEL.y / 2.0 + 6.0);
    }
}

/// Give every new atom a sprite. Runs each frame, so atoms poured in mid-run are dressed as
/// they appear.
fn dress_atoms(
    mut commands: Commands,
    white: Option<Res<White>>,
    atoms: Query<Entity, (With<Atom>, Without<Sprite>)>,
) {
    let Some(white) = white else { return };
    for atom in &atoms {
        commands.entity(atom).try_insert(
            Sprite::new(white.0)
                .with_size(Vec2::splat(ATOM_RADIUS * 2.0))
                .with_z(2.0),
        );
    }
}

/// Which quantity the atoms are colored by. Two genuinely different pictures of the same gas,
/// and the difference between them is the whole point of the word "statistical".
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
enum Coloring {
    /// Every atom takes the temperature measured for the column it is in. Equilibrium looks
    /// uniform and a gradient looks like a gradient, because this is the field quantity.
    #[default]
    Local,
    /// Every atom takes its own kinetic energy. Shows the Maxwell-Boltzmann spread that a
    /// temperature is the *average* of — which never goes away, however long it settles.
    Individual,
}

impl Coloring {
    /// Its name for the readout.
    fn label(self) -> &'static str {
        match self {
            Coloring::Local => "local temperature",
            Coloring::Individual => "atom energy",
        }
    }
}

/// C switches how the atoms are colored. Debounced against the previous frame rather than
/// using `just_pressed`, since a frame system can see the same tick's edge twice.
fn coloring_toggle(input: Res<Input>, mut coloring: ResMut<Coloring>, mut was_down: Local<bool>) {
    let down = input.pressed(Key::C);
    if down && !*was_down {
        *coloring = match *coloring {
            Coloring::Local => Coloring::Individual,
            Coloring::Individual => Coloring::Local,
        };
    }
    *was_down = down;
}

/// Color the gas.
///
/// In [`Coloring::Local`] an atom wears the temperature of the column it is in, so the box
/// reads as one field: flat color at equilibrium, a left-to-right ramp when heat is flowing.
///
/// In [`Coloring::Individual`] it wears its own kinetic energy. That view never settles into
/// one color no matter how equal the walls are, because a gas in equilibrium holds a
/// Maxwell-Boltzmann spread of energies — at 600 K the coldest tenth of the atoms sit near
/// 66 K and the hottest tenth above 1300 K. The spread is the equilibrium, not a departure
/// from it.
fn tint_atoms(
    mut atoms: Query<(&Transform2D, &Velocity, &mut Sprite), With<Atom>>,
    court: Res<Court>,
    meter: Res<Meter>,
    coloring: Res<Coloring>,
) {
    for (transform, velocity, mut sprite) in &mut atoms {
        let temperature = match *coloring {
            Coloring::Local => meter.profile[game::profile_bin(court.0, transform.translation.x)],
            Coloring::Individual => temperature_of(kinetic_energy(velocity.0)),
        };
        sprite.color = thermal(temperature / SCALE_TOP);
    }
}

/// Draw the two reservoirs at the box edges, colored by their temperature.
fn draw_surfaces(
    mut surfaces: Query<(&Surface, &mut Sprite, &mut Transform2D)>,
    court: Res<Court>,
    walls: Res<Walls>,
) {
    for (surface, mut sprite, mut transform) in &mut surfaces {
        let temperature = walls.of(surface.0);
        sprite.custom_size = Some(vec2(WALL_THICKNESS, court.0.y));
        sprite.color = thermal(temperature / SCALE_TOP);
        // Inside the box, not outside it: the camera shows exactly the court, so a bar drawn
        // beyond the edge is a bar nobody sees.
        transform.translation = vec2(
            surface.0.sign() * (court.0.x / 2.0 - WALL_THICKNESS / 2.0),
            0.0,
        );
    }
}

/// Plot the measured temperature across the box: one dot per column, height by temperature,
/// colored on the same ramp. A flat line means the gas is in equilibrium; a slope is
/// conduction, and its steepness is the gradient driving the flow.
fn draw_profile(
    mut dots: Query<(&ProfileDot, &mut Sprite, &mut Transform2D)>,
    court: Res<Court>,
    meter: Res<Meter>,
) {
    for (dot, mut sprite, mut transform) in &mut dots {
        let temperature = meter.profile[dot.0];
        let fraction = (temperature / SCALE_TOP).clamp(0.0, 1.0);
        let column = (dot.0 as f32 + 0.5) / PROFILE_BINS as f32;
        transform.translation = vec2(
            -court.0.x / 2.0 + column * court.0.x,
            -court.0.y / 2.0 + fraction * court.0.y * PLOT_HEIGHT,
        );
        // Wide, flat segments rather than dots: against a field of small square atoms, shape
        // is what separates the instrument from the thing it is measuring. Washed toward
        // white for the same reason — the reading has to be legible over the gas that
        // produced it.
        sprite.custom_size = Some(vec2(court.0.x / PROFILE_BINS as f32 * 0.92, 4.0));
        let color = thermal(fraction);
        let wash = 0.55;
        sprite.color = Color::rgba(
            color.r + (1.0 - color.r) * wash,
            color.g + (1.0 - color.g) * wash,
            color.b + (1.0 - color.b) * wash,
            // Dim until the column has actually been sampled, so an empty box doesn't draw a
            // flat line along the floor as though it had measured one.
            if meter.seen[dot.0] { 1.0 } else { 0.0 },
        );
    }
}

/// The instrument panel: what the surfaces are set to, what the gas is doing, and the heat
/// crossing each face.
#[allow(clippy::too_many_arguments)] // a readout reads everything, by definition
fn readout(
    census: Res<Census>,
    walls: Res<Walls>,
    meter: Res<Meter>,
    speed: Res<Speed>,
    paused: Res<Paused>,
    coloring: Res<Coloring>,
    mut texts: Query<&mut Text, With<Readout>>,
) {
    let Ok(mut text) = texts.single_mut() else {
        return;
    };
    // Both surfaces report positive when heating the gas, so a steady state shows one of each
    // sign; the average magnitude is the heat current through the gas.
    let current = (meter.left_flux.abs() + meter.right_flux.abs()) / 2.0;
    let edge = |bins: &[f32]| bins.iter().sum::<f32>() / bins.len().max(1) as f32;
    let near_left = edge(&meter.profile[..4]);
    let near_right = edge(&meter.profile[PROFILE_BINS - 4..]);
    text.value = format!(
        "STATISTICAL HEAT FLOW      {:>7.1}s   speed {:>4.2}x{}\n\n\
         left surface  {:>6.1} K      right surface {:>6.1} K{}\n\
         gas at left   {:>6.0} K      gas at right  {:>6.0} K      mean {:.0} K\n\
         heat current  {:>8.0}        left {:>+9.0}   right {:>+9.0}\n\
         atoms {:>5}      collisions {}      wall hits {}\n\n\
         q/a left surface   e/d right surface   s equalize   hold n/m atoms\n\
         c color by {}   space pause   up/down speed   0 normal   r refill",
        meter.elapsed,
        speed.0,
        if paused.0 { "   PAUSED" } else { "" },
        walls.left,
        walls.right,
        if walls.left == walls.right {
            "   EQUAL"
        } else {
            ""
        },
        near_left,
        near_right,
        meter.mean_temperature(),
        current,
        meter.left_flux,
        meter.right_flux,
        census.atoms,
        meter.collisions,
        meter.wall_hits,
        coloring.label(),
    );
}

fn main() {
    env_logger::init();
    Fulcrum::with_config(FulcrumConfig {
        title: "Statistical Heat Flow".into(),
        window_size: (DEFAULT_COURT.x as u32, DEFAULT_COURT.y as u32),
        clear_color: Color::rgb(0.04, 0.04, 0.06),
        ..Default::default()
    })
    .insert_resource(assets!())
    .with_plugin(DefaultPlugins)
    // Before the game plugin, so the grid rebuild runs first each tick and the collision pass
    // sees this tick's positions. A cell a few atom-diameters across keeps the candidate
    // lists short without making the grid itself expensive.
    .with_plugin(SpatialPlugin {
        cell_size: ATOM_RADIUS * 8.0,
    })
    .with_plugin(GamePlugin)
    .insert_resource(Coloring::default())
    .add_startup(setup)
    .add_frame_system(fit_window)
    .add_frame_system(dress_atoms)
    .add_frame_system(coloring_toggle)
    .add_frame_system(tint_atoms)
    .add_frame_system(draw_surfaces)
    .add_frame_system(draw_profile)
    .add_frame_system(readout)
    .run();
}
