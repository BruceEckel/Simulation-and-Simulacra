//! The one render pass, and the wgpu plumbing that holds it up.
//!
//! One triangle, one fragment program, straight into the frame. There is no intermediate buffer
//! and no finishing step: the coats are stacked in the same shader that decides how many of them
//! there are.
//!
//! Written against a bare device and queue rather than against the engine, so the same code runs
//! under the window and under the headless device the `parrish_still` example builds.

use crate::field::{SHEET_SIZE, Sheet};
use crate::game::{self, Weather};
use crate::look::Look;

// ---------------------------------------------------------------------------------------
// what the sky is made of
// ---------------------------------------------------------------------------------------

/// The three decks: altitude in metres, how many metres one tile of the sheet covers, the
/// coverage threshold, and how many coats of distance the deck already sits behind. Lowest first.
///
/// Lower is nearer. At any angle above the horizon the lower plane is the closer of the two, so
/// the low deck carries the big shapes overhead and the high ones stack up behind it towards the
/// horizon. What decides how large a deck's clouds *look* is its tile against its altitude, not
/// either on its own.
pub const DECKS: [[f32; 4]; 3] = [
    [1700.0, 12_000.0, 0.800, 0.00],
    [3000.0, 15_000.0, 0.825, 0.30],
    [4600.0, 17_000.0, 0.850, 0.62],
];

/// How many metres a cloud stands up off its deck, over the full range of the field.
///
/// The number that makes these clouds monuments rather than slicks. A deck is a horizontal plane
/// and a plane seen from underneath is foreshortened by the sine of the angle you are looking up
/// at, so a cloud painted on one is a six-to-one smear at twenty degrees. The only thing that
/// stands it back up is height: a ray meets the top of a tall cloud a long way before it meets
/// the plane underneath, and the taller the cloud the earlier that happens and the more of its
/// flank you see.
pub const TOWER: f32 = 2600.0;

/// How many metres of relief the light thinks the field has.
///
/// Deliberately not [`TOWER`]. This one multiplies the field's slope to make a normal, and a
/// normal built from the full height of the tower is a cliff everywhere: every cloud would have
/// one blazing face and one black one with no turn between them. This is the number that decides
/// how fast the light falls away around a lobe, which is a question about modelling rather than
/// about how tall the cloud is.
pub const RELIEF: f32 = 2400.0;
/// How far the shadow walk reaches towards the sun, as a fraction of a deck's tile.
pub const SHADOW_REACH: f32 = 0.062;
/// How hard what it finds counts against the light.
pub const SHADOW_GAIN: f32 = 3.0;
/// How much field above the coverage threshold counts as a cloud at full height.
pub const CROWN: f32 = 0.30;

/// Coats of the warm tint on a cloud's lit side. Few: the lit side of one of these clouds is
/// very nearly the bare ground, and that is where the light in the picture comes from.
pub const LIGHT_COATS: f32 = 0.55;
/// Coats of the cool tint on the side turned away.
pub const SHADOW_COATS: f32 = 1.00;
/// Coats of the deepest tint in the body of a thick cloud.
pub const DEEP_COATS: f32 = 0.60;
/// Extra coats where a neighbouring cloud stands between this one and the sun.
pub const CAST_COATS: f32 = 0.95;

/// How many coats are lifted back off along a cloud's edge with the light behind it.
pub const RIM_LIFT: f32 = 1.05;
/// And how tightly that is confined to the part of the sky the sun is in.
pub const SILVER_TIGHT: f32 = 9.0;
/// The cosine of the sun's angular radius. A little wider than the real half degree.
pub const SUN_ANGULAR: f32 = 0.999_95;

/// Coats of distance per metre. At the far edge of the world this is most of a coat, which is
/// what turns the last range of hills blue.
pub const AIR_COATS: f32 = 1.1e-5;
/// Coats of water over the reflection where the water is steepest underfoot.
pub const WATER_COATS: f32 = 2.20;
/// How much of the reflection survives at the near edge of the water, where you are looking
/// almost straight down into it. Grazing water is a mirror; water underfoot is a hole.
pub const MIRROR_FLOOR: f32 = 0.12;

/// How much of the field's range the cloud edge is squeezed into, either side of the middle.
///
/// Gradient noise comes out soft, and a soft field thresholded gives a cloud with a woolly
/// outline. Squeezing it puts the whole transition into a narrow band of the range: the level
/// sets come out rounded and well separated, and the edge of a cloud arrives in a couple of
/// pixels instead of twenty. It is what turns the field from weather into something carved.
pub const CONTRAST: f32 = 0.21;

/// How far the light wraps past the terminator, as a fraction.
///
/// A cloud is not an opaque solid: light entering the lit side scatters through and comes out
/// some way round the shoulder, which is why a real cumulus has no hard terminator on it and why
/// its whole upper surface reads as lit under a sun only a few degrees up. Without this the flat
/// top of a deck under a low sun is as dark as its underside, and the light lands only on the
/// flanks that happen to face the sun.
pub const WRAP: f32 = 0.45;

/// How high a ripple tilts the water, in units of `dir.y`.
pub const RIPPLE: f32 = 0.010;
/// How far the ledge across the bottom of the frame stands below the horizon, in units of
/// `dir.y`. A rock a hundred metres out, seen from fourteen metres up, which is what puts it
/// across the bottom fifth of the frame.
pub const LEDGE_RISE: f32 = 0.20;
/// How far the hills on the horizon rise, in the same units.
pub const RIDGE_RISE: f32 = 0.090;

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
    /// xyz: how far the wind has carried the sky. w: how far the second field has gone past the
    /// first.
    pub wind: [f32; 4],
    /// The low deck: altitude, tile, threshold, coats of distance.
    pub deck_a: [f32; 4],
    /// The middle one.
    pub deck_b: [f32; 4],
    /// The high one.
    pub deck_c: [f32; 4],
    /// how tall a cloud stands, how much relief the light sees, shadow reach, shadow gain.
    pub puff: [f32; 4],
    /// Coats: light, shadow, deep, cast.
    pub coats: [f32; 4],
    /// rim lift, silver tightness, sun disc cosine, crown height.
    pub edge: [f32; 4],
    /// zenith coats, horizon coats, glow coats, glow tightness.
    pub dial: [f32; 4],
    /// coats of distance per metre, water coats, mirror floor, edge contrast.
    pub air: [f32; 4],
    /// ledge rise, ridge rise, ripple height, light wrap.
    pub land: [f32; 4],
    /// The ground everything is painted on.
    pub ground: [f32; 4],
    /// The blue the sky is glazed with.
    pub sky_high: [f32; 4],
    /// The warm wash at the horizon.
    pub sky_low: [f32; 4],
    /// What is laid in where coats are lifted off.
    pub glow: [f32; 4],
    /// The warm coat on a cloud's lit side.
    pub cloud_light: [f32; 4],
    /// The cool coat on the side turned away.
    pub cloud_shadow: [f32; 4],
    /// The deepest coat, in the body of the cloud.
    pub cloud_deep: [f32; 4],
    /// Distance, as a glaze.
    pub distance: [f32; 4],
    /// What the reflection is seen through.
    pub water: [f32; 4],
    /// The far range of hills.
    pub ridge_far: [f32; 4],
    /// The near one.
    pub ridge_near: [f32; 4],
    /// The rock across the bottom of the frame.
    pub ledge: [f32; 4],
    /// width, height, 1/width, 1/height.
    pub screen: [f32; 4],
    /// planet radius, furthest anything is drawn, water tile, sheet size in texels.
    pub world: [f32; 4],
}

/// Everything the shader needs for one frame, gathered from the weather, the palette and the
/// window. The binary and the headless example both call it, so a frame on screen and a frame in
/// a PNG cannot drift apart.
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
        puff: [TOWER, RELIEF, SHADOW_REACH, SHADOW_GAIN],
        coats: [LIGHT_COATS, SHADOW_COATS, DEEP_COATS, CAST_COATS],
        edge: [RIM_LIFT, SILVER_TIGHT, SUN_ANGULAR, CROWN],
        dial: [
            look.sky_depth,
            look.horizon_depth,
            look.glow_depth,
            // Tight in a low sun and loose in a high one, because the glow around a low sun is
            // the whole horizon and the glow around a high one is a halo.
            6.0 + 20.0 * look.elevation,
        ],
        air: [AIR_COATS, WATER_COATS, MIRROR_FLOOR, CONTRAST],
        land: [LEDGE_RISE, RIDGE_RISE, RIPPLE, WRAP],
        ground: wide(look.ground, 0.0),
        sky_high: wide(look.sky_high, 0.0),
        sky_low: wide(look.sky_low, 0.0),
        glow: wide(look.glow, 0.0),
        cloud_light: wide(look.cloud_light, 0.0),
        cloud_shadow: wide(look.cloud_shadow, 0.0),
        cloud_deep: wide(look.cloud_deep, 0.0),
        distance: wide(look.distance, 0.0),
        water: wide(look.water, 0.0),
        ridge_far: wide(look.ridge_far, 0.0),
        ridge_near: wide(look.ridge_near, 0.0),
        ledge: wide(look.ledge, 0.0),
        screen: [
            screen.0 as f32,
            screen.1 as f32,
            1.0 / screen.0.max(1) as f32,
            1.0 / screen.1.max(1) as f32,
        ],
        world: [
            game::PLANET_RADIUS,
            game::HORIZON_DISTANCE,
            game::WATER_TILE,
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
        let shader = device.create_shader_module(wgpu::include_wgsl!("parrish.wgsl"));

        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("parrish frame layout"),
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
            label: Some("parrish sheet layout"),
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
            label: Some("parrish"),
            bind_group_layouts: &[Some(&frame_layout), Some(&sheet_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("parrish"),
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
            label: Some("parrish uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let frame_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("parrish frame"),
            layout: &frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            }],
        });

        let view = upload(device, queue, sheet);
        // Repeat, and trilinear between the levels: the sheet is laid over a sky ninety
        // kilometres across, and a deck seen edge-on near the horizon is minified by a factor of
        // hundreds. Without the mip chain that band of the picture is glitter.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("parrish sheet sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let sheet_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("parrish sheet"),
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
            label: Some("parrish"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("parrish"),
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
        label: Some("parrish sheet"),
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
