//! The one render pass, and the wgpu plumbing that holds it up.
//!
//! Simpler than the volumetric piece next door by a whole pass: there is no intermediate buffer
//! and no finishing step, because the line is drawn analytically inside the same shader that
//! decides the colour. One triangle, one fragment program, straight into the frame.
//!
//! Written against a bare device and queue rather than against the engine, so the same code
//! runs under the window and under the headless device the `ligne_still` example builds.

use crate::field::{SHEET_SIZE, Sheet};
use crate::game::{self, Weather};
use crate::look::Look;

// ---------------------------------------------------------------------------------------
// what the sky is made of
// ---------------------------------------------------------------------------------------

/// The three decks: altitude in metres, how many metres one tile of the sheet covers, the
/// coverage threshold, and how far into the haze the deck sits. Lowest first.
///
/// Lower is nearer. At any angle above the horizon the lower plane is the closer of the two, so
/// the low deck carries the big shapes overhead and the high ones stack up behind it towards
/// the horizon. What decides how large a deck's clouds *look* is its tile against its altitude,
/// not either on its own.
pub const DECKS: [[f32; 4]; 3] = [
    [1500.0, 12_000.0, 0.690, 0.00],
    [2700.0, 15_000.0, 0.705, 0.22],
    [4300.0, 15_500.0, 0.720, 0.44],
];

/// How many metres of relief the field stands for: the height of a cloud from base to crown, as
/// far as the light is concerned. It is the number that turns a two-dimensional field into
/// something with a normal.
pub const RELIEF: f32 = 2600.0;
/// How far the shadow walk reaches towards the sun, as a fraction of a deck's tile.
pub const SHADOW_REACH: f32 = 0.055;
/// How hard what it finds darkens the cloud.
pub const SHADOW_GAIN: f32 = 2.6;
/// How much field above the coverage threshold counts as a cloud at full height.
pub const CROWN: f32 = 0.28;

/// How many flat tones a cloud is filled in.
pub const CLOUD_BANDS: f32 = 5.0;
/// What a cloud is lit by before any sunlight reaches it.
pub const AMBIENT: f32 = 0.10;
/// How much the light falling on a slope adds on top.
pub const LIGHT: f32 = 0.70;
/// How much of the tone comes from height alone, which is the smooth half of the shading.
pub const CROWN_TONE: f32 = 0.38;

/// How wide the line is, in pixels. The one number the whole style rests on.
pub const INK_WIDTH: f32 = 1.7;
/// How many flat bands the sky is stepped into.
pub const SKY_BANDS: f32 = 9.0;
/// And the desert.
pub const GROUND_BANDS: f32 = 6.0;
/// How far the rock on the horizon rises, in units of `dir.y`, which near the horizon is very
/// nearly the angle itself.
pub const MESA_RISE: f32 = 0.10;

/// The cosine of the sun's angular radius. Wider than the real half degree: a real sun is three
/// pixels across, and this one is drawn with a line around it.
pub const SUN_ANGULAR: f32 = 0.999_6;

/// Everything the shader is told, in one buffer.
///
/// Every field is a `vec4` whether it needs four numbers or not, so that the layout is the same
/// in WGSL and in Rust with no padding to get wrong. The comments say what the loose components
/// carry, and `tests/sky.rs` checks that they land where the shader reads them.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
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
    /// xyz: how far the wind has carried the sky. w: how far the second field has gone past
    /// the first.
    pub wind: [f32; 4],
    /// The low deck: altitude, tile, threshold, haze.
    pub deck_a: [f32; 4],
    /// The middle one.
    pub deck_b: [f32; 4],
    /// The high one.
    pub deck_c: [f32; 4],
    /// relief, shadow reach, shadow strength, crown height.
    pub puff: [f32; 4],
    /// tone bands, ambient, light, how much of the tone is height.
    pub shade: [f32; 4],
    /// line width in pixels, sky bands, ground bands, mesa rise.
    pub line: [f32; 4],
    /// The sky straight up.
    pub sky_zenith: [f32; 4],
    /// The sky at the horizon.
    pub sky_horizon: [f32; 4],
    /// The sun's disc.
    pub sun_colour: [f32; 4],
    /// The line.
    pub ink: [f32; 4],
    /// Sand underfoot.
    pub ground_near: [f32; 4],
    /// Sand at the horizon.
    pub ground_far: [f32; 4],
    /// Rock.
    pub mesa: [f32; 4],
    /// What distance fades into.
    pub haze: [f32; 4],
    /// The cloud ramp, darkest first.
    pub cloud_0: [f32; 4],
    /// Second stop.
    pub cloud_1: [f32; 4],
    /// Third.
    pub cloud_2: [f32; 4],
    /// The lit crown.
    pub cloud_3: [f32; 4],
    /// width, height, 1/width, 1/height.
    pub screen: [f32; 4],
    /// planet radius, furthest anything is drawn, ground tile, sheet size in texels.
    pub world: [f32; 4],
}

/// Everything the shader needs for one frame, gathered from the weather, the palette and the
/// window. The binary and the headless example both call it, so a frame on screen and a frame
/// in a PNG cannot drift apart.
pub fn compose(weather: &Weather, look: &Look, screen: (u32, u32)) -> Uniforms {
    let eye = weather.eye();
    let aspect = screen.0.max(1) as f32 / screen.1.max(1) as f32;
    let wide = |rgb: [f32; 3], last: f32| [rgb[0], rgb[1], rgb[2], last];
    Uniforms {
        origin: wide(eye.at, (game::FOV * 0.5).tan()),
        forward: wide(eye.forward, aspect),
        right: wide(eye.right, screen.0 as f32),
        up: wide(eye.up, screen.1 as f32),
        sun: wide(look.sun_direction(), SUN_ANGULAR),
        wind: [weather.drift[0], 0.0, weather.drift[1], weather.boil],
        deck_a: DECKS[0],
        deck_b: DECKS[1],
        deck_c: DECKS[2],
        puff: [RELIEF, SHADOW_REACH, SHADOW_GAIN, CROWN],
        shade: [CLOUD_BANDS, AMBIENT, LIGHT, CROWN_TONE],
        line: [INK_WIDTH, SKY_BANDS, GROUND_BANDS, MESA_RISE],
        sky_zenith: wide(look.sky_zenith, 0.0),
        sky_horizon: wide(look.sky_horizon, 0.0),
        sun_colour: wide(look.sun, 0.0),
        ink: wide(look.ink, 0.0),
        ground_near: wide(look.ground_near, 0.0),
        ground_far: wide(look.ground_far, 0.0),
        mesa: wide(look.mesa, 0.0),
        haze: wide(look.haze, 0.0),
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
        world: [
            game::PLANET_RADIUS,
            game::HORIZON_DISTANCE,
            game::GROUND_TILE,
            SHEET_SIZE as f32,
        ],
    }
}

// ---------------------------------------------------------------------------------------
// the pass
// ---------------------------------------------------------------------------------------

/// What the frame is written in. sRGB, so the palette's numbers go in as they are written and
/// the hardware does the conversion on the way out.
pub const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// The pipeline, the sheet and the uniform buffer.
pub struct Renderer {
    pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    frame_bind: wgpu::BindGroup,
    sheet_bind: wgpu::BindGroup,
}

impl Renderer {
    /// Build the pipeline and upload the sheet. Done once.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, sheet: &Sheet) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("ligne.wgsl"));

        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ligne frame layout"),
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
        let sheet_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ligne sheet layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ligne"),
            bind_group_layouts: &[Some(&frame_layout), Some(&sheet_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ligne"),
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
            label: Some("ligne uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let frame_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ligne frame"),
            layout: &frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            }],
        });

        let view = upload(device, queue, sheet);
        // Repeat, and trilinear between the levels: the sheet is laid over a sky ninety
        // kilometres across, and a deck seen edge-on near the horizon is minified by a factor
        // of hundreds. Without the mip chain that band of the picture is glitter.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ligne sheet sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let sheet_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ligne sheet"),
            layout: &sheet_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        Self {
            pipeline,
            uniforms,
            frame_bind,
            sheet_bind,
        }
    }

    /// Draw one frame into `target`.
    pub fn draw(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        uniforms: &Uniforms,
        target: &wgpu::TextureView,
    ) {
        queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(uniforms));
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("ligne"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ligne"),
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
            pass.set_bind_group(0, &self.frame_bind, &[]);
            pass.set_bind_group(1, &self.sheet_bind, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit([encoder.finish()]);
    }
}

/// Put the sheet and its mip chain on the GPU.
fn upload(device: &wgpu::Device, queue: &wgpu::Queue, sheet: &Sheet) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("ligne sheet"),
        size: wgpu::Extent3d {
            width: sheet.size,
            height: sheet.size,
            depth_or_array_layers: 1,
        },
        mip_level_count: sheet.levels.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let mut size = sheet.size;
    for (level, texels) in sheet.levels.iter().enumerate() {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: level as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            texels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * size),
                rows_per_image: Some(size),
            },
            wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
        );
        size = (size / 2).max(1);
    }
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}
