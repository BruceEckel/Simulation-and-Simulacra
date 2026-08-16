//! One traveller, crossing.
//!
//! A Moebius desert is not empty. There is a figure in it, usually one, usually small, usually
//! going somewhere the panel does not say. The whole point of the emptiness is that something is
//! crossing it, and until there is a figure the desert is a backdrop rather than a distance.
//!
//! This is that figure: a man on a horse, walking, drawn the way everything else here is drawn.
//! One flat colour with a line around it. He is worked out on the CPU and drawn in the shader,
//! which does the shape as a union of rounded segments, so he is the same kind of drawing as a
//! cloud: a distance field, filled where it is inside and inked where it is near nought.
//!
//! He rides a circle around you rather than a line across the sand, which keeps him at one
//! distance and so at one size, and it makes his walk honest: the pace of the weather carries him,
//! so the sky and the horse run off the same clock.
//!
//! The circle is the width of the frame. He walks out of the right of the picture and in at the
//! left of it, so there is a figure in the desert at every moment and whatever you have done with
//! your head. A circle the width of the compass is the honest one and it is the wrong one for a
//! panel: he would be out of the picture for nine tenths of an hour at a time, and a desert with
//! nothing crossing it is a backdrop. The frame is what the picture is, so the frame is what he
//! goes round.
//!
//! The fold is a bearing, and the drawing takes it from here as one: the shader is given the width
//! along with him and draws him again a frame to either side, so the half of him leaving one edge
//! is the half arriving at the other rather than a jump.
//!
//! Everything in here is a function of the clock and of where you are looking, like everything
//! else in this piece.

use crate::game::EYE_HEIGHT;
use std::f32::consts::TAU;

/// How far away he rides, in metres.
///
/// The frame is pointed up at the clouds, so the desert in it runs from the horizon down to about
/// a tenth of a radian below, which is everything further off than four hundred metres or so.
/// Anything nearer than that is under the bottom edge of the picture. This is set inside that
/// band and about two thirds of the way down it.
pub const DISTANCE: f32 = 620.0;

/// How tall he is drawn, in metres, from the sand to the top of his hat.
///
/// Nine times life, and it is worth saying why rather than hiding it. At the only distances this
/// frame shows the ground at all, a true-sized man on a horse is four pixels: not a small figure
/// but a mark, and a mark is not what the desert is empty for. It has to hold a line around it as
/// well, and the line is the same weight as every other line in the picture, so anything under
/// about forty pixels comes out as a stick of ink with no flat colour left inside it. A comic
/// artist draws the figure at the size it has to read at and lets the horizon look after itself,
/// so that is what this does. Nothing else in the picture is scaled, and nothing else needs to be.
pub const HEIGHT: f32 = 20.0;

/// How fast he goes, in metres a second, at pace one. A walking horse.
pub const SPEED: f32 = 1.6;

/// How far he goes in one stride, in metres, which is what the legs are timed against.
pub const STRIDE: f32 = 2.4;

/// Where he is and what he is doing, at a moment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rider {
    /// The direction from the eye to the sand under his hooves, folded into the frame.
    pub dir: [f32; 3],
    /// How tall he is on the sky, in radians.
    pub size: f32,
    /// Where he is in his stride, in radians, so the legs and the bob come off one number.
    pub gait: f32,
    /// How far round the fold takes him, which is the width of the frame in bearing. The drawing
    /// needs it to put the copy of him that is arriving next to the copy that is leaving.
    pub span: f32,
}

impl Rider {
    /// Him, at a moment, seen by a head turned to `yaw` through a frame `span` radians wide.
    pub fn at(clock: f32, yaw: f32, span: f32) -> Self {
        let span = span.max(1e-3);
        // Walking. The bearing is the arc he has covered divided by the radius he covers it on,
        // which is the whole of the trigonometry: a circle is the one path where the distance
        // never changes and so neither does his size.
        let bearing = 0.7 + clock * SPEED / DISTANCE;
        // And folded into the frame: how far he is off the middle of the picture, brought back
        // into half a frame either side of it. He leaves one edge and arrives at the other.
        let off = (bearing - yaw + span * 0.5).rem_euclid(span) - span * 0.5;
        // Flat ground under a level eye: the drop is the eye height over the distance. The world
        // is drawn on a curved planet, but over six hundred metres the curve is a centimetre and
        // the difference is a thousandth of the figure.
        let drop = EYE_HEIGHT / DISTANCE;
        let (across, along) = (yaw + off).sin_cos();
        let reach = (1.0 + drop * drop).sqrt();
        Self {
            dir: [across / reach, -drop / reach, along / reach],
            size: HEIGHT / DISTANCE,
            gait: clock * SPEED / STRIDE * TAU,
            span,
        }
    }
}
