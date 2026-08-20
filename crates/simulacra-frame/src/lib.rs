//! The offscreen frame a simulation's own render pass draws into, shown by the engine as one
//! sprite — without going stale when the window changes size.
//!
//! Eight of the simulations here do not draw through the engine's sprite batcher. They compute
//! every pixel themselves, in a shader or on the CPU, write it into a texture, and hand the
//! engine one full-window sprite showing that texture. The engine has no notion of such a
//! thing, so each of them grew its own copy of the same forty lines. This is those forty lines,
//! written once.
//!
//! # The bug this exists to prevent
//!
//! The obvious way to handle a resize is to build a texture the new size and put it behind the
//! same asset handle with [`Assets::replace`], which is what the engine offers for exactly that
//! shape of problem. **It does not work for a texture a sprite is drawing.**
//!
//! The engine's sprite renderer caches one GPU bind group per texture, keyed by the handle's
//! id, and builds it once:
//!
//! ```ignore
//! self.texture_bind_groups.entry(texture_id).or_insert_with(|| ...)
//! ```
//!
//! `replace` puts new contents behind the *same* handle, so the id does not change, so the
//! cached bind group is never rebuilt and goes on pointing at the texture that was replaced.
//! The engine pairs its own `replace` calls with an `invalidate_texture` for precisely this
//! reason, but that call is `pub(crate)` and hot reload is its only caller. There is no way to
//! reach it from a game.
//!
//! The symptom is nasty because it does not look like a rendering bug: the picture freezes on
//! the last frame drawn before the resize and never recovers, while the simulation underneath
//! carries on perfectly happily. It looks like the simulation stopped.
//!
//! # What this does instead
//!
//! **The handle is never reused.** A new texture gets a new handle, which gets its own bind
//! group. Since a new handle also means a texture nothing will ever free, one is made as rarely
//! as possible:
//!
//! - the frame is allocated once at the size of the **largest display** the window could be
//!   dragged onto, so going fullscreen costs no allocation at all;
//! - and it **only ever grows**, so dragging an edge — which produces a new window size every
//!   frame, and would otherwise allocate a full-screen texture every frame — costs nothing.
//!
//! What is left is a handful of allocations in the life of a process.
//!
//! # Living with a frame bigger than the window
//!
//! The sprite is pinned by its **top-left corner** to the top-left corner of the window and
//! drawn at one texel to the pixel, so texel `(0, 0)` is window pixel `(0, 0)` whatever size
//! either of them is, and the overhang falls off the right and bottom edges where the viewport
//! clips it. Nothing has to know about the extra room.
//!
//! Both kinds of writer then work unchanged, as long as they use the *window* size rather than
//! the texture size and start at the origin:
//!
//! - a render pass draws into [`Frame::view`] and should scissor itself to [`Frame::window`],
//!   so it is not shading several million fragments nobody can see;
//! - a CPU uploader writes its rows into the same corner with `queue.write_texture`, giving the
//!   extent it actually has rather than the extent of the texture.

use fulcrum::prelude::*;
use fulcrum_render::{GpuContext, WindowHandle};

/// How big the frame texture should be: big enough for the window, and never smaller than it
/// already was.
///
/// `current` is what is allocated now, `(0, 0)` for nothing yet. `display` is the largest
/// display the window could be moved to. See the module docs for why it grows and never
/// shrinks — the whole point is that an ordinary resize does not need a new texture, because a
/// new texture means a new handle and a new handle means one more that is never freed.
pub fn frame_size(current: (u32, u32), window: (u32, u32), display: (u32, u32)) -> (u32, u32) {
    if current == (0, 0) {
        // Nothing allocated yet: take the biggest display this window could be dragged onto,
        // and the window itself in case it is somehow larger still.
        return (
            display.0.max(window.0).max(1),
            display.1.max(window.1).max(1),
        );
    }
    (current.0.max(window.0), current.1.max(window.1))
}

/// The size of the largest display the window could be moved to, or `(0, 0)` when there is no
/// window to ask — a headless run, or the frames before one exists.
pub fn largest_display(handle: Option<&WindowHandle>) -> (u32, u32) {
    let Some(handle) = handle else {
        return (0, 0);
    };
    handle
        .0
        .available_monitors()
        .fold((0, 0), |biggest, monitor| {
            let size = monitor.size();
            (biggest.0.max(size.width), biggest.1.max(size.height))
        })
}

/// The frame, and the sprite showing it.
///
/// Inserted by [`FramePlugin`]. Read [`view`](Self::view) to draw into it and
/// [`window`](Self::window) for the part of it that is on screen.
#[derive(Resource)]
pub struct Frame {
    /// What the texture is called, for the debug label and the asset path.
    label: &'static str,
    /// What it is written in.
    format: wgpu::TextureFormat,
    /// What the sprite showing it is drawn at.
    z: f32,
    /// The texture, as the engine's asset store knows it. A *new* handle every time the frame
    /// is allocated, never a replacement: see the module docs.
    handle: Option<Handle<Texture>>,
    /// A view of it, so a pass can be pointed at it without going through the asset store.
    view: Option<wgpu::TextureView>,
    /// The sprite showing it.
    sprite: Option<Entity>,
    /// How big the texture is.
    size: (u32, u32),
    /// How much of it is on screen, which is the window.
    window: (u32, u32),
}

impl Frame {
    /// A view of the texture, once there is one. What a render pass draws into.
    pub fn view(&self) -> Option<&wgpu::TextureView> {
        self.view.as_ref()
    }

    /// The texture itself, once there is one. What a CPU uploader writes into.
    pub fn handle(&self) -> Option<Handle<Texture>> {
        self.handle
    }

    /// How big the texture is, which is at least the window and usually more.
    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// The part of the texture that is on screen: the window, in physical pixels. **This** is
    /// the extent to draw and to upload, not [`size`](Self::size).
    pub fn window(&self) -> (u32, u32) {
        self.window
    }

    /// Is there a frame to draw into yet?
    pub fn ready(&self) -> bool {
        self.view.is_some() && self.window.0 > 0 && self.window.1 > 0
    }
}

/// Installs a [`Frame`] and the system that keeps it fitting the window.
///
/// The system runs in `Update`, so anything that draws into the frame has to run after it:
///
/// ```ignore
/// Fulcrum::with_config(config)
///     .with_plugin(DefaultPlugins)
///     .with_plugin(FramePlugin::new("ligne", OUTPUT_FORMAT))
///     .add_frame_system(draw.after(simulacra_frame::fit_frame))
///     .run();
/// ```
pub struct FramePlugin {
    label: &'static str,
    format: wgpu::TextureFormat,
    z: f32,
}

impl FramePlugin {
    /// A frame called `label`, written in `format`, drawn at `z = 1` — under the readout every
    /// one of these pieces puts over the top of it.
    pub fn new(label: &'static str, format: wgpu::TextureFormat) -> Self {
        Self {
            label,
            format,
            z: 1.0,
        }
    }

    /// The same, drawn at a different depth.
    pub fn at_z(mut self, z: f32) -> Self {
        self.z = z;
        self
    }
}

impl Plugin for FramePlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut().insert_resource(Frame {
            label: self.label,
            format: self.format,
            z: self.z,
            handle: None,
            view: None,
            sprite: None,
            size: (0, 0),
            window: (0, 0),
        });
        app.add_systems(Update, fit_frame);
    }
}

/// Keep the frame big enough for the window, and the sprite over the window.
///
/// Public so that a piece can order its own drawing after it.
pub fn fit_frame(
    mut commands: Commands,
    gpu: Option<Res<GpuContext>>,
    handle: Option<Res<WindowHandle>>,
    window: Res<WindowInfo>,
    mut frame: ResMut<Frame>,
    mut textures: ResMut<Assets<Texture>>,
    mut sprites: Query<(&mut Sprite, &mut Transform2D)>,
) {
    let Some(gpu) = gpu else { return };
    if window.width == 0 || window.height == 0 {
        return; // minimized
    }
    let seen = (window.width, window.height);
    frame.window = seen;

    let wanted = frame_size(frame.size, seen, largest_display(handle.as_deref()));
    if wanted != frame.size || frame.handle.is_none() {
        frame.size = wanted;
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(frame.label),
            size: wgpu::Extent3d {
                width: wanted.0,
                height: wanted.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: frame.format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        // A fresh handle every time, never `Assets::replace`: see the module docs. The path
        // carries the size so that two frames never collide under one name.
        frame.handle = Some(textures.insert_with_path(
            format!("<{} {}x{}>", frame.label, wanted.0, wanted.1),
            Texture {
                texture,
                view: view.clone(),
                width: wanted.0,
                height: wanted.1,
            },
        ));
        frame.view = Some(view);
    }

    let texture = frame.handle.expect("just set");
    let extent = vec2(wanted.0 as f32, wanted.1 as f32);
    // The window's top-left corner. World units are physical pixels in these pieces, and the
    // camera sits on the middle of the window.
    let corner = vec2(-(seen.0 as f32) / 2.0, seen.1 as f32 / 2.0);
    match frame.sprite.and_then(|entity| sprites.get_mut(entity).ok()) {
        Some((mut sprite, mut transform)) => {
            sprite.texture = texture;
            sprite.custom_size = Some(extent);
            transform.translation = corner;
        }
        None => {
            let z = frame.z;
            frame.sprite = Some(
                commands
                    .spawn((
                        Sprite {
                            // Pinned by its top-left corner rather than its middle, so that
                            // texel (0, 0) is window pixel (0, 0) however much bigger the
                            // frame is than the window.
                            anchor: vec2(0.0, 1.0),
                            ..Sprite::new(texture).with_size(extent).with_z(z)
                        },
                        Transform2D::from_xy(corner.x, corner.y),
                    ))
                    .id(),
            );
        }
    }
}
