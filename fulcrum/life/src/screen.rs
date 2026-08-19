//! The one render pass, and the wgpu plumbing that holds it up.
//!
//! The engine draws sprites, and a sprite for every cell was never going to work here: the
//! resolution control goes down to one cell per physical pixel, and a display's worth of that
//! is millions of cells. Even at a comfortable size it would be tens of thousands of entities
//! rebuilt whenever the window moved.
//!
//! So the field goes up as a **texture** and is coloured in a fragment program. Two bytes a
//! cell — what the cell is, and how long it has been that — written by [`crate::game::Board`]
//! into a buffer that is already in the layout the GPU wants. The shader turns a pixel into a
//! cell with one division, reads those two bytes, and picks a colour. Nothing is interpolated
//! and nothing is sampled: `textureLoad` at integer coordinates, so a cell has exactly one
//! colour and there are no soft edges anywhere, at any resolution.
//!
//! What that costs, and it is the only cost: two bytes per cell go over the bus once per
//! generation. At one cell to the pixel on a large display that is a few megabytes, and it
//! happens only when a generation has actually been computed — a held field uploads nothing.
//!
//! Written against a bare device and queue rather than against the engine, so there is nothing
//! in here that a headless test could not build.

use crate::game::Board;
use crate::look::Look;

/// What the frame is written in.
///
/// sRGB, which means the hardware applies the display curve to whatever the shader hands it.
/// The palettes in `look.rs` are written as display values — the numbers you would type into a
/// paint program — so the shader undoes that curve on the one colour it has arrived at, at the
/// very end. Doing it there rather than to the palette on the way in is deliberate: it means
/// every blend between two of these colours happens between the numbers as written, so a ramp
/// from a red to a near-black passes through the dark reds a person would expect rather than
/// through the ones the physics of adding light would give.
pub const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// What the field is uploaded as: two unsigned bytes a cell, read rather than sampled.
pub const FIELD_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg8Unorm;

/// How dark the line between cells is drawn, against whatever the cell's colour is.
const EDGE_INK: f32 = 0.55;

/// The smallest a cell may be, in pixels, before the line between cells is not drawn at all.
/// Below four the line is most of the cell, and the field turns into a grid with nothing in it.
const EDGE_FLOOR: f32 = 4.0;

/// The parts of the picture that are a matter of taste rather than of the rule.
#[derive(Clone, Copy, Debug)]
pub struct Reading {
    /// Colour live cells by how long they have been alive.
    pub ageing: bool,
    /// Leave a fading trail where cells have recently been.
    pub ghosts: bool,
    /// Draw a line between cells, when the cells are big enough to have one.
    pub edges: bool,
}

impl Default for Reading {
    fn default() -> Self {
        Self {
            ageing: true,
            ghosts: true,
            edges: false,
        }
    }
}

/// Everything the shader is told, in one buffer.
///
/// Every field is a `vec4` whether it needs four numbers or not, so that the layout is the same
/// in WGSL and in Rust with no padding to get wrong. The comments say what the loose components
/// carry, and `tests/screen.rs` checks that they land where the shader reads them.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniforms {
    /// cells across, cells down, pixels to a cell, cells between texture rows.
    pub field: [f32; 4],
    /// window width, window height, whether cell edges are drawn, how dark they are.
    pub screen: [f32; 4],
    /// whether age is read, whether trails are read, the smallest cell that gets an edge, spare.
    pub reading: [f32; 4],
    /// An empty cell.
    pub back: [f32; 4],
    /// A live cell that has held for a while.
    pub live: [f32; 4],
    /// A live cell born this generation.
    pub fresh: [f32; 4],
    /// A cell part-way through a Generations rule's dying states.
    pub dying: [f32; 4],
    /// Where a cell recently was.
    pub trail: [f32; 4],
    /// The line between cells.
    pub ink: [f32; 4],
}

/// Everything the shader needs for one frame, gathered from the field, the palette and the
/// window.
pub fn compose(
    board: &Board,
    cell: u32,
    look: &Look,
    reading: Reading,
    screen: (u32, u32),
) -> Uniforms {
    let wide = |rgb: [f32; 3]| [rgb[0], rgb[1], rgb[2], 1.0];
    Uniforms {
        field: [
            board.width as f32,
            board.height as f32,
            cell.max(1) as f32,
            board.stride as f32,
        ],
        screen: [
            screen.0 as f32,
            screen.1 as f32,
            f32::from(u8::from(reading.edges)),
            EDGE_INK,
        ],
        reading: [
            f32::from(u8::from(reading.ageing)),
            f32::from(u8::from(reading.ghosts)),
            EDGE_FLOOR,
            0.0,
        ],
        back: wide(look.back),
        live: wide(look.live),
        fresh: wide(look.fresh),
        dying: wide(look.dying),
        trail: wide(look.trail),
        ink: wide(look.ink),
    }
}

/// How big the texture the pass draws into should be: big enough for the window, and never
/// smaller than it already was.
///
/// This is a one-line policy with a bug behind it, so it is worth saying what it is for.
///
/// The engine's sprite renderer builds one GPU bind group per texture, keyed by the asset
/// handle's id, and builds it once. `Assets::replace` puts new contents behind the same handle
/// and therefore the same id, so the cached bind group is never rebuilt and keeps pointing at
/// the texture that was replaced. The engine pairs its own `replace` calls with an invalidation
/// for exactly this reason, but that call is not public. So from out here a texture a sprite is
/// drawing must never be replaced: a new handle has to be made instead, and the picture freezes
/// on the last frame before the resize if it is not.
///
/// A new handle means a texture nothing will free, so the rule is to need one as rarely as
/// possible. Taking the largest display straight away means going fullscreen costs nothing, and
/// never shrinking means the ordinary case — dragging an edge, which produces a new size every
/// frame — costs nothing either. What is left is a handful of allocations in the life of a
/// process, in exchange for a picture that keeps moving.
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

// ---------------------------------------------------------------------------------------
// the pass
// ---------------------------------------------------------------------------------------

/// The pipeline, the uniform buffer, and the texture the field is carried in.
pub struct Renderer {
    pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    frame_bind: wgpu::BindGroup,
    field_layout: wgpu::BindGroupLayout,
    field_bind: Option<wgpu::BindGroup>,
    field: Option<wgpu::Texture>,
    field_size: (u32, u32),
    /// The `revision` of the board whose cells are on the GPU now.
    carried: Option<u64>,
}

impl Renderer {
    /// Build the pipeline. Done once, on the first frame that has a device.
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("life.wgsl"));

        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("life frame layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        // No sampler: the shader reads texels at integer coordinates rather than sampling
        // between them, which is the whole reason a cell has one colour and not a smear.
        let field_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("life field layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("life"),
            bind_group_layouts: &[Some(&frame_layout), Some(&field_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("life"),
            layout: Some(&layout),
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
            label: Some("life uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let frame_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("life frame"),
            layout: &frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            }],
        });

        Self {
            pipeline,
            uniforms,
            frame_bind,
            field_layout,
            field_bind: None,
            field: None,
            field_size: (0, 0),
            carried: None,
        }
    }

    /// Put the field on the GPU, if what is up there is not this generation already.
    ///
    /// The texture is as wide as the board's stride rather than its width, which is what makes
    /// every row a whole number of 256-byte blocks and lets the field be any width at all. The
    /// few cells of slack at the end of each row are never read: the shader stops at the width.
    pub fn carry(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, board: &Board) {
        let wanted = (board.stride, board.height);
        if self.field_size != wanted || self.field.is_none() {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("life field"),
                size: wgpu::Extent3d {
                    width: wanted.0,
                    height: wanted.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: FIELD_FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.field_bind = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("life field"),
                layout: &self.field_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                }],
            }));
            self.field = Some(texture);
            self.field_size = wanted;
            self.carried = None;
        }
        if self.carried == Some(board.revision) {
            return;
        }
        let Some(texture) = &self.field else { return };
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &board.pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(board.stride * 2),
                rows_per_image: Some(board.height),
            },
            wgpu::Extent3d {
                width: wanted.0,
                height: wanted.1,
                depth_or_array_layers: 1,
            },
        );
        self.carried = Some(board.revision);
    }

    /// Draw one frame into `target`.
    ///
    /// `target` is usually bigger than the window — see `ensure_renderer` in the binary for why
    /// — so the pass scissors itself to the top-left corner that is actually on screen. Without
    /// it the shader would run over several million fragments a frame that nothing can see.
    pub fn draw(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        uniforms: &Uniforms,
        target: &wgpu::TextureView,
        window: (u32, u32),
    ) {
        let Some(field_bind) = &self.field_bind else {
            return;
        };
        queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(uniforms));
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("life"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("life"),
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
            pass.set_scissor_rect(0, 0, window.0.max(1), window.1.max(1));
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.frame_bind, &[]);
            pass.set_bind_group(1, field_bind, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit([encoder.finish()]);
    }
}
