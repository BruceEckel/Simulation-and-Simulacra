//! The one render pass, and the wgpu plumbing that holds it up.
//!
//! Two uniform buffers and one triangle. The first buffer is the frame: the eye, the palette and
//! the handful of numbers the desert is drawn from. The second is the sky, rebuilt every frame:
//! the circles the clouds are made of and the groups over them, in the order they are drawn.
//!
//! Only the circles in front of you are sent. The whole compass holds two or three hundred
//! groups, of which about a fifth are in the frame at any moment, and the shader walks that
//! fifth per pixel. Throwing the rest away on the CPU costs a dot product each and saves the GPU
//! from making the same test a few million times.
//!
//! Written against a bare device and queue rather than against the engine, so the same code runs
//! under the window and under the headless device the `moebius_still` example builds.

use crate::cloud::{MAX_DISCS, MAX_GROUPS, Sky};
use crate::game::{self, Weather};
use crate::look::Look;

// ---------------------------------------------------------------------------------------
// what the drawing is made of
// ---------------------------------------------------------------------------------------

/// How wide every line in the picture is, in pixels.
///
/// One number for every line there is: the cloud silhouettes, the arcs inside them, the sky's
/// bands, the desert's, the horizon, the rock and the ring around the sun. Uniform line weight
/// is not a saving here, it is the style.
pub const INK_WIDTH: f32 = 1.8;

/// How much softness the edge of a line or a fill is given, in pixels. Enough to stop a stair,
/// not enough to read as a blur.
pub const FEATHER: f32 = 0.6;

/// How many flat bands the sky is cut into, which is how many colours the palette gives it.
pub const SKY_BANDS: f32 = 5.0;
/// And the desert, which gets fewer: a foreground cut into four is a corrugated roof, and what
/// is wanted is one flat floor with a couple of long dune lines drawn across it.
pub const GROUND_BANDS: f32 = 3.0;

/// How far the rock on the horizon rises, in units of `dir.y`, which near the horizon is very
/// nearly the angle itself.
pub const MESA_RISE: f32 = 0.085;
/// How many pieces of rock stand around the compass.
pub const MESAS: usize = 24;

/// The sun's angular radius, in radians. Four degrees, eight times the real thing: a true sun is
/// three pixels across, and this one has to hold its own against a cloud.
pub const SUN_RADIUS: f32 = 0.070;
/// Where the ring around it stands, as a multiple of that.
pub const SUN_RING: f32 = 1.34;

/// Everything the shader is told about the frame, in one buffer.
///
/// Every field is a `vec4` whether it needs four numbers or not, so that the layout is the same
/// in WGSL and in Rust with no padding to get wrong. The comments say what the loose components
/// carry, and `tests/sky.rs` checks that they land where the shader reads them.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniforms {
    /// xyz: the eye, in metres. w: tan of half the vertical field of view.
    pub origin: [f32; 4],
    /// xyz: where it looks. w: aspect ratio.
    pub forward: [f32; 4],
    /// xyz: its right. w: window width in pixels.
    pub right: [f32; 4],
    /// xyz: its up. w: window height in pixels.
    pub up: [f32; 4],
    /// xyz: towards the sun. w: cosine of its angular radius.
    pub sun: [f32; 4],
    /// line width in pixels, sky bands, ground bands, how far the rock rises.
    pub pen: [f32; 4],
    /// planet radius, furthest the ground is drawn, ground tile, how much a line is feathered.
    pub world: [f32; 4],
    /// How many groups of circles are in the sky buffer, the cosine of where the ring around the
    /// sun stands, and two spare.
    pub counts: [f32; 4],
    /// The sky at the horizon.
    pub sky_0: [f32; 4],
    /// And up from there.
    pub sky_1: [f32; 4],
    /// Halfway.
    pub sky_2: [f32; 4],
    /// Nearly overhead.
    pub sky_3: [f32; 4],
    /// The zenith.
    pub sky_4: [f32; 4],
    /// Sand underfoot.
    pub sand_0: [f32; 4],
    /// And away from there.
    pub sand_1: [f32; 4],
    /// Sand at the horizon.
    pub sand_2: [f32; 4],
    /// The sun's disc.
    pub sun_colour: [f32; 4],
    /// The line.
    pub ink: [f32; 4],
    /// Rock.
    pub mesa: [f32; 4],
    /// The four flat colours a cloud is filled with, far band first.
    pub cloud_0: [f32; 4],
    /// Second.
    pub cloud_1: [f32; 4],
    /// Third.
    pub cloud_2: [f32; 4],
    /// The one the clouds overhead are filled with.
    pub cloud_3: [f32; 4],
    /// width, height, 1/width, 1/height.
    pub screen: [f32; 4],
    /// The rock: where round the compass it stands, how wide it is, how high, and how far its
    /// top is tipped. A zero height is a piece of rock that is not there.
    pub rock: [[f32; 4]; MESAS],
}

impl Default for Uniforms {
    fn default() -> Self {
        bytemuck::Zeroable::zeroed()
    }
}

/// The sky, laid out for the shader: the visible groups, and the circles they are made of.
///
/// Big enough to be worth keeping rather than making, so the caller holds one and it is filled
/// again every frame. Nothing past `counts.x` groups is read, so nothing needs clearing.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Slab {
    /// xyz: the centre of a cap holding the whole group. w: its angular radius.
    pub cap: [[f32; 4]; MAX_GROUPS],
    /// first circle, how many, which cloud colour, spare.
    pub span: [[f32; 4]; MAX_GROUPS],
    /// xyz: the normal of the half-space the group is cut back to. w: where it is cut.
    pub plane: [[f32; 4]; MAX_GROUPS],
    /// xyz: a unit direction. w: an angular radius.
    pub disc: [[f32; 4]; MAX_DISCS],
}

impl Default for Slab {
    fn default() -> Self {
        bytemuck::Zeroable::zeroed()
    }
}

impl Slab {
    /// One, on the heap, since it is thirty kilobytes and has no business on a stack.
    pub fn boxed() -> Box<Self> {
        Box::new(Self::default())
    }
}

/// The rock on the horizon, which does not move and so is worked out once.
///
/// Flat-topped and straight-sided, because that is what Moebius draws: the desert in those
/// panels is a ruled line with a few blocks standing on it, and every curve in the picture
/// belongs to the sky.
pub fn rocks() -> [[f32; 4]; MESAS] {
    let mut seed = crate::cloud::Rng::new(0x9f1a, 0x33d7, 0x0451);
    let mut out = [[0.0f32; 4]; MESAS];
    for (index, rock) in out.iter_mut().enumerate() {
        // Spread around the compass, then nudged, so they are neither in a row nor in a heap.
        let slot = index as f32 / MESAS as f32 * std::f32::consts::TAU;
        let here = slot + seed.range(-0.12, 0.12);
        // Anything from a tower to a wall. A skyline whose blocks are all one width is a fence,
        // and one whose blocks are all narrow is a picket fence.
        let width = 0.035 + 0.16 * seed.draw().powf(1.2);
        // More than half the slots are empty. The clouds are the subject here and the rock is
        // the thing they are measured against, so there wants to be enough of it to stand on the
        // horizon and not so much that it becomes a wall along the bottom of the panel.
        let height = if seed.draw() < 0.45 {
            seed.range(0.45, 1.0)
        } else {
            0.0
        };
        *rock = [here, width, height, seed.range(-0.18, 0.18)];
    }
    out
}

/// Everything the shader needs for one frame, gathered from the weather, the sky, the palette
/// and the window. The binary and the headless example both call it, so a frame on screen and a
/// frame in a PNG cannot drift apart.
///
/// The sky goes in through `slab`, cut down on the way to the groups that are in front of you.
pub fn compose(
    weather: &Weather,
    sky: &Sky,
    look: &Look,
    screen: (u32, u32),
    slab: &mut Slab,
) -> Uniforms {
    let eye = weather.eye();
    let aspect = screen.0.max(1) as f32 / screen.1.max(1) as f32;
    let wide = |rgb: [f32; 3], last: f32| [rgb[0], rgb[1], rgb[2], last];

    let groups = cull(sky, eye.forward, aspect, slab);

    Uniforms {
        origin: wide(eye.at, (game::FOV * 0.5).tan()),
        forward: wide(eye.forward, aspect),
        right: wide(eye.right, screen.0 as f32),
        up: wide(eye.up, screen.1 as f32),
        sun: wide(look.sun_direction(), SUN_RADIUS.cos()),
        pen: [INK_WIDTH, SKY_BANDS, GROUND_BANDS, MESA_RISE],
        world: [
            game::PLANET_RADIUS,
            game::HORIZON_DISTANCE,
            game::GROUND_TILE,
            FEATHER,
        ],
        counts: [groups as f32, (SUN_RADIUS * SUN_RING).cos(), 0.0, 0.0],
        sky_0: wide(look.sky[0], 0.0),
        sky_1: wide(look.sky[1], 0.0),
        sky_2: wide(look.sky[2], 0.0),
        sky_3: wide(look.sky[3], 0.0),
        sky_4: wide(look.sky[4], 0.0),
        sand_0: wide(look.sand[0], 0.0),
        sand_1: wide(look.sand[1], 0.0),
        sand_2: wide(look.sand[2], 0.0),
        sun_colour: wide(look.sun, 0.0),
        ink: wide(look.ink, 0.0),
        mesa: wide(look.mesa, 0.0),
        cloud_0: wide(look.cloud[0], 0.0),
        cloud_1: wide(look.cloud[1], 0.0),
        cloud_2: wide(look.cloud[2], 0.0),
        cloud_3: wide(look.cloud[3], 0.0),
        screen: [
            screen.0 as f32,
            screen.1 as f32,
            1.0 / screen.0.max(1) as f32,
            1.0 / screen.1.max(1) as f32,
        ],
        rock: rocks(),
    }
}

/// Copy the groups that are in front of you into the slab, in the order they were built, and
/// say how many there were.
///
/// Order is the whole of the depth here: a group drawn later covers what is under it, and that
/// is how a crest erases the arc it sits on and how a near cloud passes in front of a far one.
/// Dropping groups out of the middle leaves the rest in the same order, so nothing moves.
pub fn cull(sky: &Sky, forward: [f32; 3], aspect: f32, slab: &mut Slab) -> usize {
    let tan = (game::FOV * 0.5).tan();
    // The angle from the middle of the frame to a corner of it, which is how far a cloud can be
    // off-axis and still have a piece of itself on the screen.
    let reach = ((tan * aspect).hypot(tan)).atan() + 0.02;

    let mut kept = 0usize;
    let mut discs = 0usize;
    for group in &sky.groups {
        if kept >= MAX_GROUPS || discs + group.count as usize > MAX_DISCS {
            break;
        }
        let towards =
            forward[0] * group.cap[0] + forward[1] * group.cap[1] + forward[2] * group.cap[2];
        if towards.clamp(-1.0, 1.0).acos() > reach + group.cap[3] {
            continue;
        }
        let from = group.first as usize;
        for (slot, disc) in slab.disc[discs..discs + group.count as usize]
            .iter_mut()
            .zip(&sky.discs[from..from + group.count as usize])
        {
            *slot = *disc;
        }
        slab.cap[kept] = group.cap;
        slab.span[kept] = [discs as f32, group.count as f32, group.tint as f32, 0.0];
        slab.plane[kept] = group.plane;
        discs += group.count as usize;
        kept += 1;
    }
    kept
}

// ---------------------------------------------------------------------------------------
// the pass
// ---------------------------------------------------------------------------------------

/// What the frame is written in. sRGB, so the palette's numbers go in as they are written and
/// the hardware does the conversion on the way out.
pub const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// The pipeline and the two buffers behind it.
pub struct Renderer {
    pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    slab: wgpu::Buffer,
    bind: wgpu::BindGroup,
}

impl Renderer {
    /// Build the pipeline. Done once.
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("moebius.wgsl"));

        let uniform_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("moebius layout"),
            entries: &[uniform_entry(0), uniform_entry(1)],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("moebius"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("moebius"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: OUTPUT_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("moebius frame"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let slab = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("moebius sky"),
            size: std::mem::size_of::<Slab>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("moebius"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: slab.as_entire_binding(),
                },
            ],
        });

        Self {
            pipeline,
            uniforms,
            slab,
            bind,
        }
    }

    /// Draw one frame into `target`.
    pub fn draw(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        uniforms: &Uniforms,
        slab: &Slab,
        target: &wgpu::TextureView,
    ) {
        queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(uniforms));
        queue.write_buffer(&self.slab, 0, bytemuck::bytes_of(slab));
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("moebius"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("moebius"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit([encoder.finish()]);
    }
}
