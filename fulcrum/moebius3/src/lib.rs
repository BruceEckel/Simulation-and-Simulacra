//! Moebius, a third time: clouds drawn the way they are drawn on paper, as outlines built from
//! circular arcs, now with shading in them and somebody underneath them.
//!
//! The first version fixed every decision in the source. The second put the line weight, the
//! number of arcs an element is built from and the palette on keys. This one raises the ceiling
//! on the arcs from six to two dozen, hatches the shaded side of every element, and puts a man on
//! a horse in the desert.
//!
//! The shading is the change worth arguing about. The first two versions shade nothing anywhere
//! and say so: no normal, no slope, no tone. This one works out where the shadow falls and draws
//! it, and the whole of the argument is that it draws it with lines. A pen has one colour, so
//! hatching is shading a flat drawing can hold, and the region it fills is still an enclosed area
//! of one value with a decision behind it rather than a gradient.
//!
//! It is worked out per billow rather than per cloud. The distance loop already finds the nearest
//! circle at every pixel, which is the part of the cloud that pixel stands on, so keeping it
//! gives a surface to light. Turned away from the sun is one set of lines; turned towards the
//! ground is two, because nothing under the horizon lights a downward face and the flat base of a
//! cumulus is the darkest thing in the sky.
//!
//! The library holds everything that is not a window. [`cloud`] builds the sky as a list of
//! overlapping circles and the order to draw them in, and owns [`cloud::Style`], the settings
//! that change the drawing rather than the weather; [`game`] is the weather that moves them;
//! [`look`] is twenty palettes as flat colour; [`rider`] is where the traveller has got to;
//! [`sky`] owns the single render pass and the shader beside it. The binary adds the window, the
//! readout and the keys, and the `moebius3_still` example draws a frame with no window.

pub mod cloud;
pub mod game;
pub mod look;
pub mod rider;
pub mod sky;
