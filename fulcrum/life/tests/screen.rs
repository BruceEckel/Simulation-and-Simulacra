//! The uniform buffer, held to the shader that reads it.
//!
//! There is one buffer of numbers, declared twice — once as a Rust struct and once as a WGSL
//! one — and nothing in either language checks that the two agree. Insert a field in one and
//! not the other and it still compiles, still runs, and draws a picture: every colour after the
//! insertion point is read from the wrong sixteen bytes. That is a whole class of bug that only
//! ever shows up as "the palette looks wrong", which is exactly the kind of wrong that gets
//! blamed on the palette.
//!
//! So the shader source is read back at test time and its field list is compared with the one
//! the Rust side declares. Every field is a `vec4` in both, which is the other half of the
//! defence: with nothing narrower than sixteen bytes anywhere, there is no padding for the two
//! languages to disagree about.

use life::screen::{Uniforms, frame_size};

/// The fields of the buffer, in the order both declarations must have them in.
const FIELDS: &[&str] = &[
    "field", "screen", "reading", "back", "live", "fresh", "dying", "trail", "ink",
];

/// The shader, as it will be compiled.
const SHADER: &str = include_str!("../src/life.wgsl");

/// The field names of the shader's `Uniforms` struct, in the order it declares them.
fn shader_fields() -> Vec<String> {
    let body = SHADER
        .split_once("struct Uniforms {")
        .expect("the shader declares a Uniforms struct")
        .1
        .split_once('}')
        .expect("and closes it")
        .0;
    body.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.starts_with("//") {
                return None;
            }
            let (name, _) = line.split_once(':')?;
            Some(name.trim().to_string())
        })
        .collect()
}

#[test]
fn the_shader_and_the_struct_declare_the_same_fields_in_the_same_order() {
    assert_eq!(
        shader_fields(),
        FIELDS,
        "life.wgsl and screen.rs have drifted apart"
    );
}

#[test]
fn every_field_is_a_vec4_and_they_are_packed_end_to_end() {
    // Sixteen bytes each, no padding, and the whole buffer is exactly that many. If a field is
    // ever added to `Uniforms` and not to `FIELDS`, this is what notices.
    assert_eq!(
        size_of::<Uniforms>(),
        FIELDS.len() * 16,
        "the buffer is not one vec4 per named field"
    );

    let offsets = [
        std::mem::offset_of!(Uniforms, field),
        std::mem::offset_of!(Uniforms, screen),
        std::mem::offset_of!(Uniforms, reading),
        std::mem::offset_of!(Uniforms, back),
        std::mem::offset_of!(Uniforms, live),
        std::mem::offset_of!(Uniforms, fresh),
        std::mem::offset_of!(Uniforms, dying),
        std::mem::offset_of!(Uniforms, trail),
        std::mem::offset_of!(Uniforms, ink),
    ];
    for (slot, offset) in offsets.iter().enumerate() {
        assert_eq!(
            *offset,
            slot * 16,
            "{} is not where the shader will look for it",
            FIELDS[slot]
        );
    }
}

#[test]
fn the_shader_reads_the_loose_components_this_side_writes() {
    // Three of the vec4s carry numbers rather than a colour, and the shader reaches into them
    // by component. These are the ones a comment could quietly be wrong about, so the uses are
    // named here and checked against the source.
    for use_site in [
        "u.field.z",   // pixels to a cell
        "u.field.x",   // cells across
        "u.field.y",   // cells down
        "u.screen.z",  // whether cell edges are drawn
        "u.screen.w",  // how dark they are
        "u.reading.x", // whether age is read
        "u.reading.y", // whether trails are read
        "u.reading.z", // the smallest cell that gets an edge
    ] {
        assert!(
            SHADER.contains(use_site),
            "the shader no longer reads {use_site}, which this side still fills in"
        );
    }
}

// ---------------------------------------------------------------------------------------
// how big the frame is
// ---------------------------------------------------------------------------------------

#[test]
fn the_frame_is_allocated_once_for_the_largest_display() {
    // Nothing allocated yet: take the whole display straight away, so that going fullscreen
    // later is not an allocation.
    assert_eq!(frame_size((0, 0), (1600, 1000), (2560, 1440)), (2560, 1440));
    // And never come out smaller than the window, even on a machine that reports no monitors.
    assert_eq!(frame_size((0, 0), (1600, 1000), (0, 0)), (1600, 1000));
}

#[test]
fn the_frame_never_shrinks_and_always_covers_the_window() {
    // This is the property the whole thing rests on. A texture a sprite is drawing cannot be
    // replaced behind its handle, so a *new* size means a new handle and a texture that is
    // never freed. Every resize that does not need one must therefore not get one.
    let display = (2560, 1440);
    let mut size = frame_size((0, 0), (1600, 1000), display);
    let first = size;

    // A drag produces a new window size every frame. None of them may move it.
    for width in (600..2560).step_by(7) {
        for height in [400, 900, 1440] {
            let next = frame_size(size, (width, height), display);
            assert_eq!(
                next, size,
                "a window of {width}x{height} reallocated the frame"
            );
            size = next;
        }
    }
    assert_eq!(size, first, "the frame moved during an ordinary drag");

    // A window bigger than any display — a spanned window, or a scale factor the monitor list
    // did not account for — grows it, once, and only on the axis that needed it.
    size = frame_size(size, (3000, 1200), display);
    assert_eq!(size, (3000, 1440));
    assert_eq!(
        frame_size(size, (800, 600), display),
        size,
        "it shrank back again"
    );
}

#[test]
fn the_frame_covers_every_window_it_is_asked_about() {
    let display = (1920, 1080);
    let mut size = frame_size((0, 0), (800, 600), display);
    for window in [
        (800, 600),
        (1920, 1080),
        (2560, 1440),
        (640, 480),
        (3840, 2160),
        (100, 100),
    ] {
        size = frame_size(size, window, display);
        assert!(
            size.0 >= window.0 && size.1 >= window.1,
            "a {window:?} window does not fit in a {size:?} frame"
        );
    }
}
