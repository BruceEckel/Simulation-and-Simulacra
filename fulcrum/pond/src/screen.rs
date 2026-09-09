//! The one render pass, and the wgpu plumbing that holds it up.
//!
//! The water is a height in every cell, and the picture is that surface lit and looked
//! through at every pixel. So the field goes up as a **texture**, one float a cell, in the
//! layout [`crate::game::Water`] keeps it in, and a fragment program does the rest:
//! it reads the heights around a pixel, works out which way the surface tilts there, and from
//! the tilt gets both a highlight and the place on the bed of the pond the eye is looking at
//! through the water. The colour comes from the bed and the height; the sparkle comes from
//! the tilt. See `pond.wgsl`.
//!
//! What that costs, and it is the whole cost: four bytes a cell go over the bus once a tick,
//! which at two pixels a cell on a large display is a few megabytes sixty times a second
//! while the water is moving, and nothing while it is still.
//!
//! Written against a bare device and queue rather than against the engine, so there is
//! nothing in here that a headless still could not build.

use crate::game::Water;

/// What the frame is written in.
///
/// sRGB, which means the hardware applies the display curve to whatever the shader hands it.
/// The palettes in `look.rs` are display values, so the shader undoes that curve on the one
/// colour it has arrived at, at the very end, and every blend on the way there is between
/// the colours as written.
pub const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// What the water is uploaded as: one float a cell, read rather than sampled.
pub const FIELD_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;

/// How steeply the surface is drawn as tilting, per unit of height difference between the
/// cells either side of a pixel. Higher makes a ring a sharper edge with a brighter highlight.
pub const SLOPE_GAIN: f32 = 6.0;
/// How far the tilt displaces what is seen through the surface, in pixels, at full tilt. It
/// makes the lantern's light wobble when a ring crosses it.
pub const REFRACTION: f32 = 36.0;
/// How far up the palette a crest is coloured and how far down a trough, per unit of height.
/// This is where a ring gets its colour: a ring is a crest with a trough behind it, so it is
/// drawn as a band of the bright end of the palette beside a band of the dark end.
pub const HEIGHT_TINT: f32 = 1.1;
/// How bright the highlight is where the surface catches the light.
pub const SPECULAR: f32 = 0.8;
/// Where the light comes from: up and to the left, and mostly above.
pub const LIGHT: [f32; 3] = [-0.40, -0.55, 0.73];
/// How much darker the corners are than the middle.
pub const VIGNETTE: f32 = 0.30;
/// Radius of the lantern's glow on the bed, in pixels, before the window's scale.
pub const LANTERN_RADIUS: f32 = 80.0;

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
    /// window width, window height, seconds since the start, where the colours have drifted to.
    pub screen: [f32; 4],
    /// the lantern: x and y in pixels, how bright, and how wide its glow is in pixels.
    pub lantern: [f32; 4],
    /// which way the light is, and how bright the highlight it makes.
    pub light: [f32; 4],
    /// slope gain, refraction in pixels, height tint, vignette.
    pub tune: [f32; 4],
    /// The palette, deepest to brightest.
    pub stops: [[f32; 4]; 5],
    /// The colour of the lantern's light.
    pub glow: [f32; 4],
}

/// What the picture needs beyond the water.
#[derive(Clone, Copy, Debug)]
pub struct Scene {
    /// Where the lantern is, in pixels.
    pub lantern: [f32; 2],
    /// How bright it is, `0..1` and a little over on the swell of a breath.
    pub glow: f32,
    /// The palette, blended if a crossfade is under way.
    pub stops: [[f32; 3]; 5],
    /// Where the colours have drifted to, in palette lengths.
    pub drift: f32,
    /// Seconds since the start, for the slow movement on the bed.
    pub time: f32,
    /// The window's scale factor, so the lantern is the same size on every display.
    pub scale: f32,
}

/// Everything the shader needs for one frame, gathered from the water, the scene and the
/// window.
pub fn compose(water: &Water, scene: &Scene, window: (u32, u32)) -> Uniforms {
    let wide = |rgb: [f32; 3]| [rgb[0], rgb[1], rgb[2], 1.0];
    let glow = crate::look::sample(&scene.stops, 0.86);
    Uniforms {
        field: [
            water.across as f32,
            water.down as f32,
            water.cell,
            water.stride as f32,
        ],
        screen: [window.0 as f32, window.1 as f32, scene.time, scene.drift],
        lantern: [
            scene.lantern[0],
            scene.lantern[1],
            scene.glow,
            LANTERN_RADIUS * scene.scale.max(1.0),
        ],
        light: [LIGHT[0], LIGHT[1], LIGHT[2], SPECULAR],
        tune: [
            SLOPE_GAIN,
            REFRACTION * scene.scale.max(1.0),
            HEIGHT_TINT,
            VIGNETTE,
        ],
        stops: [
            wide(scene.stops[0]),
            wide(scene.stops[1]),
            wide(scene.stops[2]),
            wide(scene.stops[3]),
            wide(scene.stops[4]),
        ],
        glow: wide(glow),
    }
}

// ---------------------------------------------------------------------------------------
// the pass
// ---------------------------------------------------------------------------------------

/// The pipeline, the uniform buffer, and the texture the water is carried in.
pub struct Renderer {
    pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    frame_bind: wgpu::BindGroup,
    field_layout: wgpu::BindGroupLayout,
    field_bind: Option<wgpu::BindGroup>,
    field: Option<wgpu::Texture>,
    field_size: (u32, u32),
    /// The `revision` of the water whose heights are on the GPU now.
    carried: Option<u64>,
}

impl Renderer {
    /// Build the pipeline. Done once, on the first frame that has a device.
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("pond.wgsl"));

        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pond frame layout"),
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
        // No sampler: a 32-bit float texture cannot be filtered everywhere, so the shader reads
        // the four cells around a point and blends them.
        let field_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pond field layout"),
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
            label: Some("pond"),
            bind_group_layouts: &[Some(&frame_layout), Some(&field_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pond"),
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
            label: Some("pond uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let frame_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pond frame"),
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

    /// Put the water on the GPU, if what is up there is not this tick's.
    ///
    /// The texture is as wide as the field's stride rather than its width, which makes
    /// every row a whole number of 256-byte blocks and lets the field be any width. The
    /// few cells of slack at the end of each row are not read: the shader clamps at the
    /// width.
    pub fn carry(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, water: &Water) {
        let wanted = (water.stride, water.down);
        if self.field_size != wanted || self.field.is_none() {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("pond field"),
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
                label: Some("pond field"),
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
        if self.carried == Some(water.revision) {
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
            bytemuck::cast_slice(&water.height),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(water.stride * 4),
                rows_per_image: Some(water.down),
            },
            wgpu::Extent3d {
                width: wanted.0,
                height: wanted.1,
                depth_or_array_layers: 1,
            },
        );
        self.carried = Some(water.revision);
    }

    /// Draw one frame into `target`.
    ///
    /// `target` is usually bigger than the window, since the frame is allocated for the largest
    /// display it could be dragged onto, so the pass is scissored to the top-left corner
    /// that is on screen.
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
            label: Some("pond"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("pond"),
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
