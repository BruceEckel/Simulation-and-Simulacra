//! The mathematics, and the framing that decides whether you get to see any of it.
//!
//! A fractal viewer fails quietly. Get the iteration wrong and you still get a picture; point
//! the view at empty space and you get a picture of that. So these tests check both: that the
//! rules are the rules, and that every fractal's opening view has the fractal in it.

use fractal::game::{
    CLOUD_POINTS, COARSEST_LEVEL, Cloud, Court, Depth, FRACTALS, Field, Fractal, GamePlugin, Grid,
    JULIA_START, Motion, SPAN_MAX, SPAN_MIN, Sample, Selection, View, sample_at,
};
use fulcrum::prelude::*;

/// A grid to survey with. Not the running one — this is about the mathematics, and a fixed
/// resolution keeps the counts below comparable between fractals.
const SURVEY: Grid = Grid {
    width: 240,
    height: 150,
};

/// Iterations to survey with. Well above the running default, so that a point called a member
/// here is one on the merits rather than one that ran out of road.
const SURVEY_DEPTH: u32 = 600;

/// Every sample of a fractal over a view.
fn survey(kind: Fractal, view: View, depth: u32) -> Vec<Sample> {
    (0..SURVEY.height)
        .flat_map(|y| (0..SURVEY.width).map(move |x| (x, y)))
        .map(|(x, y)| {
            let (real, imaginary) = view.complex_at(SURVEY, x, y);
            sample_at(kind, real, imaginary, depth, JULIA_START)
        })
        .collect()
}

/// A viewer opened on one fractal, at that fractal's own opening view.
fn viewer(kind: Fractal, view: View) -> Fulcrum {
    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed: 11,
        window_size: (1280, 800),
        ..Default::default()
    })
    .with_plugin(GamePlugin);
    app.world_mut().insert_resource(Selection(kind));
    app.world_mut().insert_resource(view);
    app.run_startup();
    app
}

/// Tick until the picture stops changing, or give up.
fn settle(app: &mut Fulcrum, limit: u32) -> u32 {
    for tick in 0..limit {
        app.tick();
        let done = if app.world_mut().resource::<Selection>().0.is_cloud() {
            app.world_mut().resource::<Cloud>().done
        } else {
            app.world_mut().resource::<Field>().done
        };
        if done {
            return tick + 1;
        }
    }
    panic!("the picture never finished inside {limit} ticks");
}

// ---------------------------------------------------------------------------------------
// the escape-time rules
// ---------------------------------------------------------------------------------------

#[test]
fn mandelbrot_knows_its_own_set() {
    let inside = |x, y| sample_at(Fractal::Mandelbrot, x, y, SURVEY_DEPTH, 0.0).inside;
    // The two fixed landmarks: the cardioid runs out at exactly 1/4 on the real axis, and the
    // set as a whole runs out at -2.
    assert!(inside(0.0, 0.0), "the origin is in the set");
    assert!(inside(0.25, 0.0), "the cardioid reaches 1/4");
    assert!(!inside(0.26, 0.0), "and stops there");
    assert!(inside(-1.0, 0.0), "the period-2 bulb is in the set");
    assert!(inside(-2.0, 0.0), "so is the very tip");
    assert!(!inside(-2.01, 0.0), "and nothing past it");
    assert!(!inside(2.0, 0.0), "nor anything out here");
    assert!(!inside(0.4, 0.4), "nor off in this direction");
}

#[test]
fn a_point_that_escapes_gets_a_fractional_count() {
    // The whole reason for the large escape radius. Without the fractional part the picture is
    // a staircase of flat bands, and every palette in the binary would show its steps.
    let view = Fractal::Mandelbrot.home(SURVEY);
    let escaped: Vec<f32> = survey(Fractal::Mandelbrot, view, SURVEY_DEPTH)
        .iter()
        .filter(|sample| !sample.inside)
        .map(|sample| sample.value)
        .collect();
    assert!(escaped.len() > 1_000, "the view should have escapees in it");
    let whole = escaped
        .iter()
        .filter(|value| (*value - value.round()).abs() < 1.0e-4)
        .count();
    assert!(
        whole * 20 < escaped.len(),
        "escape counts should be smooth, but {whole} of {} landed on whole numbers",
        escaped.len()
    );
}

#[test]
fn more_iterations_only_ever_shrinks_the_set() {
    // A point is called a member only because nothing has disproved it yet, so raising the
    // limit can take members away and can never add any. Getting this backwards is the classic
    // way to write an escape test that is subtly wrong.
    let view = Fractal::Mandelbrot.home(SURVEY);
    let brief = survey(Fractal::Mandelbrot, view, 60);
    let patient = survey(Fractal::Mandelbrot, view, 600);
    for (brief, patient) in brief.iter().zip(&patient) {
        assert!(
            brief.inside || !patient.inside,
            "a point ruled out early cannot come back later"
        );
    }
    let shrunk = brief.iter().filter(|s| s.inside).count();
    let settled = patient.iter().filter(|s| s.inside).count();
    assert!(
        settled < shrunk,
        "600 iterations should rule out points that 60 could not ({settled} vs {shrunk})"
    );
}

#[test]
fn julia_is_connected_only_where_its_constant_is_in_the_mandelbrot_set() {
    // This is the whole relationship between the two sets, and it is worth pinning down: the
    // Mandelbrot set is exactly the catalogue of constants whose Julia set holds together.
    let view = Fractal::Julia.home(SURVEY);
    let interior = |phase: f64| {
        survey_at_phase(view, phase)
            .iter()
            .filter(|sample| sample.inside)
            .count()
    };
    let joined = interior(std::f64::consts::PI); // c = -0.7885, inside the period-2 bulb
    let dust = interior(0.0); // c = +0.7885, well outside the set
    assert!(
        joined > SURVEY.cells() / 10,
        "a constant inside the set should give a Julia set with a real interior, got {joined}"
    );
    assert!(
        dust < SURVEY.cells() / 1_000,
        "a constant outside it should give dust, got {dust}"
    );
}

/// A Julia survey at one constant.
fn survey_at_phase(view: View, phase: f64) -> Vec<Sample> {
    (0..SURVEY.height)
        .flat_map(|y| (0..SURVEY.width).map(move |x| (x, y)))
        .map(|(x, y)| {
            let (real, imaginary) = view.complex_at(SURVEY, x, y);
            sample_at(Fractal::Julia, real, imaginary, SURVEY_DEPTH, phase)
        })
        .collect()
}

#[test]
fn the_burning_ship_is_not_the_mandelbrot_set() {
    // The two rules agree wherever both parts of the orbit stay positive, and part company
    // everywhere else, which is what makes one smooth and the other all masts and spars.
    let view = Fractal::Mandelbrot.home(SURVEY);
    let smooth = survey(Fractal::Mandelbrot, view, SURVEY_DEPTH);
    let jagged = survey(Fractal::BurningShip, view, SURVEY_DEPTH);
    let differ = smooth
        .iter()
        .zip(&jagged)
        .filter(|(a, b)| a.inside != b.inside)
        .count();
    assert!(
        differ > SURVEY.cells() / 50,
        "throwing the signs away should change the picture, but only {differ} cells moved"
    );
}

#[test]
fn newton_finds_all_three_roots() {
    let view = Fractal::Newton.home(SURVEY);
    let samples = survey(Fractal::Newton, view, SURVEY_DEPTH);
    let undecided = samples.iter().filter(|sample| sample.inside).count();
    assert!(
        undecided < SURVEY.cells() / 500,
        "Newton's method solves z^3 = 1 from almost anywhere, but {undecided} cells gave up"
    );
    for root in 0..3u8 {
        let basin = samples
            .iter()
            .filter(|sample| !sample.inside && sample.band == root)
            .count();
        assert!(
            basin > SURVEY.cells() / 8,
            "root {root} should own a fair share of the picture, got {basin}"
        );
    }
}

#[test]
fn every_escape_time_home_view_shows_something() {
    // A view can be perfectly valid and still be a picture of nothing: too far out and the
    // fractal is a speck, too far in and it is one flat color. Both failures look like a
    // single value dominating the picture.
    for kind in FRACTALS.into_iter().filter(|kind| !kind.is_cloud()) {
        let samples = survey(kind, kind.home(SURVEY), SURVEY_DEPTH);
        // The same buckets the palette would draw, so "varied" here means varied on screen.
        let mut buckets = [0usize; 33];
        for sample in &samples {
            let slot = if sample.inside {
                32
            } else {
                ((sample.value.max(0.0).sqrt() * 2.0) as usize).min(31)
            };
            buckets[slot] += 1;
        }
        let used = buckets.iter().filter(|count| **count > 0).count();
        let biggest = buckets.iter().max().copied().unwrap_or(0);
        assert!(
            used >= 6,
            "{}'s home view should have some variety in it, got {used} distinct bands",
            kind.name()
        );
        assert!(
            biggest * 10 < samples.len() * 9,
            "{}'s home view is {}% one flat color",
            kind.name(),
            biggest * 100 / samples.len()
        );
    }
}

// ---------------------------------------------------------------------------------------
// the progressive renderer
// ---------------------------------------------------------------------------------------

#[test]
fn the_coarse_pass_covers_the_whole_picture_at_once() {
    // The point of the coarse pass is that you never see a half-drawn screen, so one tick has
    // to be enough to reach every cell.
    let mut app = viewer(
        Fractal::Mandelbrot,
        Fractal::Mandelbrot.home(Grid::default()),
    );
    app.tick();
    let field = app.world_mut().resource::<Field>().clone();
    assert!(
        field.level < COARSEST_LEVEL,
        "one tick should finish the coarsest pass, but it is still on level {}",
        field.level
    );
    let escaped = field.samples.iter().filter(|s| !s.inside).count();
    assert!(
        escaped > field.samples.len() / 3,
        "after one tick the whole grid should already be filled in, got {escaped} of {}",
        field.samples.len()
    );
}

#[test]
fn refining_lands_on_the_exact_picture() {
    // Whatever the coarse passes put on screen along the way, what they converge to has to be
    // the same picture a straightforward one-cell-at-a-time renderer would have produced.
    let grid = Grid::default();
    let view = Fractal::Mandelbrot.home(grid);
    let mut app = viewer(Fractal::Mandelbrot, view);
    let ticks = settle(&mut app, 400);
    let depth = app.world_mut().resource::<Depth>().0;
    let field = app.world_mut().resource::<Field>().clone();

    for y in 0..grid.height {
        for x in 0..grid.width {
            let (real, imaginary) = view.complex_at(grid, x, y);
            let expected = sample_at(Fractal::Mandelbrot, real, imaginary, depth, JULIA_START);
            assert_eq!(
                field.samples[grid.index(x, y)],
                expected,
                "cell ({x}, {y}) after {ticks} ticks"
            );
        }
    }
}

#[test]
fn a_moving_julia_only_steps_between_finished_pictures() {
    // The constant is tied to finished pictures rather than to the clock, so that no single
    // frame is ever half one constant and half the next.
    let mut app = viewer(Fractal::Julia, Fractal::Julia.home(Grid::default()));
    let mut seen = Vec::new();
    for _ in 0..200 {
        app.tick();
        let field = app.world_mut().resource::<Field>().clone();
        let phase = app.world_mut().resource::<Motion>().phase;
        seen.push((field.done, phase));
    }
    let moves = seen.windows(2).filter(|w| w[0].1 != w[1].1).count();
    assert!(
        moves > 2,
        "the constant should have drifted, it moved {moves} times"
    );
    for pair in seen.windows(2) {
        if pair[0].1 != pair[1].1 {
            // The step happens in the same tick the picture finished, after it finished, so
            // it is the tick the constant arrives on that has to report a finished picture.
            assert!(
                pair[1].0,
                "the constant moved on a tick that had not finished its picture"
            );
        }
    }
}

#[test]
fn a_picture_arrives_quickly_enough_to_watch() {
    // The budget buys sharpness with time, so it is worth pinning down how much time. These
    // are tick counts and not seconds, which is the whole point: they are the same on every
    // machine, and a slower one simply takes longer over each tick.
    for kind in FRACTALS {
        let mut app = viewer(kind, kind.home(Grid::default()));
        let ticks = settle(&mut app, 900);
        println!("{:<20} finished in {ticks:>4} ticks", kind.name());
        assert!(
            ticks < 240,
            "{} took {ticks} ticks to draw, which is too long to sit through",
            kind.name()
        );
    }
}

#[test]
fn holding_still_stops_the_drift() {
    let mut app = viewer(Fractal::Julia, Fractal::Julia.home(Grid::default()));
    settle(&mut app, 400);
    app.world_mut().resource_mut::<Motion>().running = false;
    let before = app.world_mut().resource::<Motion>().phase;
    for _ in 0..120 {
        app.tick();
    }
    assert_eq!(
        app.world_mut().resource::<Motion>().phase,
        before,
        "nothing should move while it is held"
    );
}

// ---------------------------------------------------------------------------------------
// the chaos games
// ---------------------------------------------------------------------------------------

#[test]
fn every_cloud_fills_its_home_view() {
    for kind in FRACTALS.into_iter().filter(|kind| kind.is_cloud()) {
        let grid = Grid::default();
        let view = kind.home(grid);
        let mut app = viewer(kind, view);
        settle(&mut app, 600);
        let cloud = app.world_mut().resource::<Cloud>().clone();
        assert_eq!(
            cloud.points.len(),
            CLOUD_POINTS,
            "{} should have filled its cloud",
            kind.name()
        );

        let (mut low_x, mut high_x) = (f32::MAX, f32::MIN);
        let (mut low_y, mut high_y) = (f32::MAX, f32::MIN);
        for speck in &cloud.points {
            low_x = low_x.min(speck.x);
            high_x = high_x.max(speck.x);
            low_y = low_y.min(speck.y);
            high_y = high_y.max(speck.y);
        }
        let (wide, tall) = view.extents(grid);
        // Filling the view along its short side is what "framed" means here. A shape can be
        // far narrower than a wide window and still be framed, so only one axis has to pass.
        let fill_x = (high_x - low_x) as f64 / wide;
        let fill_y = (high_y - low_y) as f64 / tall;
        assert!(
            fill_x.max(fill_y) > 0.75,
            "{} only fills {:.0}% x {:.0}% of its home view",
            kind.name(),
            fill_x * 100.0,
            fill_y * 100.0
        );
        // And it has to be inside the view, not spilling out of it.
        assert!(
            view.holds(grid, low_x as f64, low_y as f64)
                && view.holds(grid, high_x as f64, high_y as f64),
            "{} spills outside its home view",
            kind.name()
        );
    }
}

#[test]
fn every_cloud_uses_every_map_it_has() {
    // A weight typed wrong, or a map that never gets picked, shows up here rather than as a
    // fern with no stem that nobody notices.
    for kind in FRACTALS.into_iter().filter(|kind| kind.is_cloud()) {
        let spec = kind.cloud().expect("a cloud fractal has maps");
        let mut app = viewer(kind, kind.home(Grid::default()));
        settle(&mut app, 600);
        let cloud = app.world_mut().resource::<Cloud>().clone();
        for map in 0..spec.maps.len() as u8 {
            let placed = cloud.points.iter().filter(|s| s.band == map).count();
            assert!(
                placed > 0,
                "{}'s map {map} never placed a point",
                kind.name()
            );
        }
    }
}

#[test]
fn the_snowflake_has_three_fold_symmetry() {
    // The snowflake is one Koch curve turned onto three sides. If the placement or the turns
    // are wrong the result is still a plausible-looking tangle, so this checks the symmetry
    // that makes it a snowflake rather than whatever else it might be.
    let grid = Grid::default();
    let mut app = viewer(Fractal::Snowflake, Fractal::Snowflake.home(grid));
    settle(&mut app, 600);
    let cloud = app.world_mut().resource::<Cloud>().clone();

    const BINS: usize = 96;
    let extent = 1.4f32;
    let bin = |x: f32, y: f32| -> Option<usize> {
        let column = ((x / extent + 0.5) * BINS as f32) as isize;
        let row = ((y / extent + 0.5) * BINS as f32) as isize;
        ((0..BINS as isize).contains(&column) && (0..BINS as isize).contains(&row))
            .then(|| row as usize * BINS + column as usize)
    };
    let mut filled = vec![false; BINS * BINS];
    for speck in &cloud.points {
        if let Some(slot) = bin(speck.x, speck.y) {
            filled[slot] = true;
        }
    }
    let third = std::f32::consts::TAU / 3.0;
    let (sin, cos) = third.sin_cos();
    let landed = cloud
        .points
        .iter()
        .filter_map(|speck| bin(speck.x * cos - speck.y * sin, speck.x * sin + speck.y * cos))
        .filter(|slot| filled[*slot])
        .count();
    assert!(
        landed * 20 > cloud.points.len() * 19,
        "turning the snowflake a third of the way round should leave it where it was, \
         but only {landed} of {} points landed back on it",
        cloud.points.len()
    );
}

#[test]
fn a_cloud_refills_when_you_zoom_into_it() {
    // The chaos game visits the whole attractor whatever the view, so keeping only what is on
    // screen is what makes these zoomable at all. Without it, zooming in just spreads the same
    // points further apart until the picture is a handful of specks.
    let grid = Grid::default();
    let mut view = Fractal::Fern.home(grid);
    view.zoom_about((0.0, 1.2), 0.08); // down into the base of the stem
    let mut app = viewer(Fractal::Fern, view);
    settle(&mut app, 900);
    let cloud = app.world_mut().resource::<Cloud>().clone();
    assert_eq!(
        cloud.points.len(),
        CLOUD_POINTS,
        "a zoomed-in fern should still fill up"
    );
}

#[test]
fn a_cloud_gives_up_on_a_view_with_nothing_in_it() {
    // And the other half of that bargain: a view the attractor never visits has to stop
    // costing anything rather than hunt forever.
    let mut view = Fractal::Fern.home(Grid::default());
    view.center_x += 400.0;
    let mut app = viewer(Fractal::Fern, view);
    let ticks = settle(&mut app, 3_000);
    let cloud = app.world_mut().resource::<Cloud>().clone();
    assert!(cloud.points.is_empty(), "there is no fern out here");
    println!("gave up after {ticks} ticks");
}

// ---------------------------------------------------------------------------------------
// getting about
// ---------------------------------------------------------------------------------------

#[test]
fn zooming_holds_what_is_under_the_pointer() {
    let mut view = Fractal::Mandelbrot.home(Grid::default());
    let anchor = (-0.743_643_887_037, 0.131_825_904_205);
    for _ in 0..40 {
        view.zoom_about(anchor, 0.7);
    }
    let grid = Grid::default();
    let (wide, _) = view.extents(grid);
    assert!(
        (view.center_x - anchor.0).abs() < wide,
        "the anchor should still be on screen after forty steps of zoom"
    );
    assert!(view.holds(grid, anchor.0, anchor.1), "and inside the view");
}

#[test]
fn zooming_stops_at_the_ends_of_the_range() {
    let mut view = Fractal::Mandelbrot.home(Grid::default());
    for _ in 0..4_000 {
        view.zoom_about((0.0, 0.0), 0.5);
    }
    assert_eq!(view.span, SPAN_MIN, "should have bottomed out");
    for _ in 0..4_000 {
        view.zoom_about((0.0, 0.0), 2.0);
    }
    assert_eq!(view.span, SPAN_MAX, "and topped out");
}

#[test]
fn the_mouse_and_the_grid_agree_where_a_point_is() {
    let grid = Grid::default();
    let court = Court::default().0;
    let view = Fractal::Mandelbrot.home(grid);
    for (x, y) in [(0, 0), (40, 90), (grid.width - 1, grid.height - 1)] {
        let (real, imaginary) = view.complex_at(grid, x, y);
        let world = view.world_of_complex(grid, court, real, imaginary);
        let (back_real, back_imaginary) = view.complex_of_world(grid, court, world);
        assert!(
            (back_real - real).abs() < view.span * 1.0e-5
                && (back_imaginary - imaginary).abs() < view.span * 1.0e-5,
            "cell ({x}, {y}) round-tripped to ({back_real}, {back_imaginary}) from \
             ({real}, {imaginary})"
        );
    }
}

#[test]
fn a_tall_home_view_is_framed_for_a_tall_window() {
    // The fern is twice as tall as it is wide and the dragon is the other way about, so the
    // opening view cannot just be a fixed width.
    let wide_window = Grid {
        width: 300,
        height: 120,
    };
    let tall_window = Grid {
        width: 120,
        height: 300,
    };
    let fern_wide = Fractal::Fern.home(wide_window);
    let fern_tall = Fractal::Fern.home(tall_window);
    assert!(
        fern_wide.span > fern_tall.span * 3.0,
        "a wide window has to open much wider to fit the fern's height"
    );
    for grid in [wide_window, tall_window] {
        let (_, tall) = Fractal::Fern.home(grid).extents(grid);
        assert!(
            tall > 10.5,
            "the whole fern is 10 units tall and has to fit, got {tall:.2}"
        );
    }
}
