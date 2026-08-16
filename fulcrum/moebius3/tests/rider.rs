//! The traveller: where he is, and whether the picture can see him.
//!
//! He is one small figure in a very large frame, and every number that decides whether he is in
//! it at all belongs to something else: how high the eye stands, how far the view is tilted up,
//! and how much sky the lens takes in. Change any of those for the sake of the clouds and he
//! silently walks off the bottom edge, which is a thing you would not notice for a week. So the
//! test is against those numbers rather than against a figure someone wrote down once.

use moebius3::game::{FOV, PITCH, Weather, frame_span};
use moebius3::rider::{DISTANCE, HEIGHT, Rider, SPEED};

/// The shape of window these tests look through, and the width of frame that goes with it.
const ASPECT: f32 = 1600.0 / 1000.0;

/// The elevation of the bottom edge of the frame, in radians, which is as far down the picture
/// as anything can be drawn.
fn bottom_of_frame() -> f32 {
    PITCH - FOV * 0.5
}

/// Him, through a window of that shape, from a head pointed at `yaw`.
fn seen(clock: f32, yaw: f32) -> Rider {
    Rider::at(clock, yaw, frame_span(ASPECT))
}

/// How far he is off the middle of the picture, in bearing.
fn off_centre(rider: &Rider, yaw: f32) -> f32 {
    let bearing = rider.dir[0].atan2(rider.dir[2]);
    let turn = std::f32::consts::TAU;
    (bearing - yaw + std::f32::consts::PI).rem_euclid(turn) - std::f32::consts::PI
}

/// Where he lands on the screen: minus one at the left edge of the picture and one at the right,
/// minus one along the bottom and one along the top.
///
/// The same projection the shader builds its rays from, run the other way. Asking whether a
/// bearing is inside half the width of the frame is not the same question, because a view tilted
/// up at the sky is not a rectangle on the compass: the bottom of the frame covers less bearing
/// than the middle does, and he rides along the bottom.
fn on_screen(rider: &Rider, yaw: f32) -> [f32; 2] {
    let eye = Weather {
        yaw,
        ..Default::default()
    }
    .eye();
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let ahead = dot(rider.dir, eye.forward);
    let up = (FOV * 0.5).tan();
    [
        dot(rider.dir, eye.right) / (ahead * up * ASPECT),
        dot(rider.dir, eye.up) / (ahead * up),
    ]
}

#[test]
fn he_is_in_the_part_of_the_desert_the_frame_shows() {
    // Under the horizon, and above the bottom edge with his hat on. The desert in this piece is
    // a band a tenth of a radian deep along the bottom of the picture, so there is not much room
    // either side of him.
    let rider = seen(1234.0, 0.0);
    let feet = rider.dir[1].asin();
    assert!(feet < -0.005, "he is standing on the sky at {feet} rad");
    assert!(
        feet > bottom_of_frame() + 0.005,
        "his feet are below the bottom edge of the picture at {feet} rad"
    );
    assert!(
        feet + rider.size < 0.0,
        "his hat is over the horizon, which would put a man in the sky"
    );
}

#[test]
fn he_is_big_enough_to_be_a_drawing_and_small_enough_to_be_a_figure() {
    // The line around him is the same weight as every other line in the picture, so under about
    // forty pixels he is a stick of ink with no colour left inside him. And a rider who fills a
    // tenth of the frame is not a man crossing a desert, he is the subject.
    let size = seen(0.0, 0.0).size;
    let frame = FOV;
    assert!(
        size > frame / 40.0,
        "he is {size} rad tall, which is a mark rather than a figure"
    );
    assert!(
        size < frame / 12.0,
        "he is {size} rad tall, which is not a man in the distance"
    );
}

#[test]
fn he_is_never_off_the_picture() {
    // The whole of the fold, and the reason for it. A man crossing a desert is what the desert is
    // empty for, so the picture is never without one: however long the clock has run and wherever
    // the head is pointed, he is within half a frame of the middle of it and at the same height
    // and the same size he always was.
    let lap = std::f32::consts::TAU * DISTANCE / SPEED;
    let first = seen(0.0, 0.0);
    for step in 0..2000 {
        let clock = lap * step as f32 / 2000.0;
        // A head turned to somewhere different every time, so that the fold is tested against the
        // view rather than against a view that never moves.
        let yaw = clock * 0.013;
        let now = seen(clock, yaw);
        let [x, y] = on_screen(&now, yaw);
        assert!(
            x.abs() <= 1.0,
            "he is off the side of the picture at {x} of half its width"
        );
        assert!(
            y.abs() <= 1.0,
            "he is off the top or bottom of the picture at {y} of half its height"
        );
        assert!(
            (now.size - first.size).abs() < 1e-6,
            "he changed size on the way round"
        );
        assert!(
            (now.dir[1] - first.dir[1]).abs() < 1e-6,
            "he changed height on the way round"
        );
    }
}

#[test]
fn the_fold_is_a_walk_rather_than_a_jump() {
    // He crosses the frame at one pace and comes back at the other edge, so the only step in his
    // bearing is the one the size of the frame: anything smaller is him walking, and anything in
    // between would be him teleporting into the middle of the picture.
    let span = frame_span(ASPECT);
    let step = 20.0;
    let mut wraps = 0;
    for tick in 0..400 {
        let clock = tick as f32 * step;
        let here = off_centre(&seen(clock, 0.4), 0.4);
        let next = off_centre(&seen(clock + step, 0.4), 0.4);
        let moved = next - here;
        // What one step of the clock covers, which is a fraction of a degree.
        let walked = step * SPEED / DISTANCE;
        if moved.abs() > walked * 1.5 {
            wraps += 1;
            assert!(
                (moved + span - walked).abs() < 1e-3,
                "he moved {moved} rad, which is neither a step nor a frame"
            );
        }
    }
    assert!(wraps > 0, "he never reached the edge of the picture");
}

#[test]
fn he_crosses_slowly_enough_to_be_watched() {
    // How long he takes to cross the frame at pace one, which is the only thing "slowly" can
    // mean here and is now the whole of his round trip as well. Minutes, not seconds: he is
    // weather rather than an event.
    let seconds = frame_span(ASPECT) / (SPEED / DISTANCE);
    assert!(
        seconds > 120.0,
        "he crosses the frame in {seconds:.0} s, which is a man in a hurry"
    );
    assert!(
        seconds < 3600.0,
        "he crosses the frame in {seconds:.0} s, which is a man who is not moving"
    );
}

#[test]
fn his_legs_are_off_the_same_clock_as_the_sky() {
    // The gait comes out of the clock rather than out of anything kept between frames, so a
    // still of any moment has his legs where they were at that moment.
    assert_eq!(seen(700.0, 0.0), seen(700.0, 0.0), "he moved on his own");
    assert_ne!(
        seen(700.0, 0.0).gait,
        seen(701.0, 0.0).gait,
        "his legs never move"
    );
    // One stride is one turn of the gait, and he covers his own stride in it.
    let stride = moebius3::rider::STRIDE / SPEED;
    let turns = (seen(stride, 0.0).gait - seen(0.0, 0.0).gait) / std::f32::consts::TAU;
    assert!(
        (turns - 1.0).abs() < 1e-4,
        "he took {turns} strides in the time one stride takes"
    );
}

#[test]
fn he_is_drawn_at_a_size_the_horizon_would_not_agree_with() {
    // Stated rather than hidden. A man on a horse is about two metres and a bit; this one is
    // several times that, because at the only distances this frame shows the ground he would
    // otherwise be four pixels. The test is here so the exaggeration is a decision on the record
    // rather than a number somebody drifts.
    const {
        assert!(
            HEIGHT > 3.0 * 2.4,
            "he is nearly life-sized, which at this distance is a full stop"
        )
    };
    const {
        assert!(
            HEIGHT < 12.0 * 2.4,
            "he is so far out of scale that the desert stops reading as distance"
        )
    };
}

#[test]
fn the_weather_carries_him() {
    // He is a function of the same clock the sky is, so holding the sky holds him and running it
    // fast runs him with it. Nothing else in the piece would notice if he had a clock of his own,
    // which is exactly why it is worth a test.
    let weather = Weather {
        clock: 300.0,
        ..Default::default()
    };
    let held = Weather {
        clock: 300.0,
        held: true,
        ..Default::default()
    };
    assert_eq!(
        seen(weather.clock, weather.yaw),
        seen(held.clock, held.yaw),
        "he walked while the sky was held"
    );
}
