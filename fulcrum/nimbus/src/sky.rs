//! The two render passes, and the wgpu plumbing that holds them up.
//!
//! Nothing in here decides what the sky looks like. It owns pipelines, bind groups, an
//! intermediate target and a uniform buffer, and it draws when it is told to. What to draw is
//! [`Uniforms`], which the binary fills in from the weather and the palette.
//!
//! It is written against a bare device and queue rather than against the engine, so the same
//! code runs under the window and under the headless device the `nimbus_still` example builds. That is
//! the only way this piece could be tuned at all: a shader that can only be seen by running the
//! program is a shader that gets debugged by guessing.

use crate::game::{self, Weather};
use crate::look::Look;
use crate::noise::Volume;

// ---------------------------------------------------------------------------------------
// how the march is run
// ---------------------------------------------------------------------------------------

/// Most steps one ray is allowed. The ceiling only bites near the horizon, where a ray can be
/// inside the layer for seventy kilometres and there is no step count that would do it justice.
pub const MAX_STEPS: f32 = 140.0;
/// Steps taken towards the sun from each lit sample. Six, each half again as long as the last,
/// which reaches the top of the layer from anywhere inside it.
pub const SUN_STEPS: f32 = 6.0;
/// How far the march goes before it gives up and lets the haze have the rest, in metres.
pub const MAX_DISTANCE: f32 = 70_000.0;
/// How long one step is, in metres, where there is room for the full count.
pub const STEP_LENGTH: f32 = 52.0;

/// Extinction per metre inside a cloud at full density. Sets how quickly a cloud goes from
/// translucent to solid, and with it how deep into one the light gets before it is used up.
pub const EXTINCTION: f32 = 0.075;
/// How hard the detail volume eats into the base shape.
pub const ERODE: f32 = 0.22;
/// Where the anvil begins to flatten, as a fraction of the layer's thickness.
pub const ANVIL: f32 = 0.68;
/// How forward-throwing the scattering is. Near three quarters, which is about right for water
/// droplets and is what puts the silver lining on a cloud in front of the sun.
pub const FORWARD_SCATTER: f32 = 0.72;
/// How much of the powder effect to apply: the dark rind on a lit cloud edge.
pub const POWDER: f32 = 0.6;
/// A multiplier on the optical depth the sun march measures. Below one on purpose: the march
/// only samples six points, and taking it at face value makes every cloud a silhouette.
pub const SUN_ABSORB: f32 = 0.9;
/// How fast distance turns things into haze, per metre.
pub const HAZE_PER_METRE: f32 = 0.000_026;
/// The cosine of the sun's angular radius. A little wider than the real half degree, because a
/// real sun is three pixels across and this one is worth looking at.
pub const SUN_ANGULAR: f32 = 0.999_75;

/// Everything the shader needs for one frame, gathered from the weather, the palette and the
/// window.
///
/// The one place the three halves of the piece meet, and deliberately a plain function: the
/// binary and the headless `nimbus_still` example both call it, so a frame drawn on screen and a frame
/// written to a PNG cannot drift apart.
pub fn compose(
    weather: &Weather,
    look: &Look,
    screen: (u32, u32),
    internal: (u32, u32),
    bands: f32,
    ink: f32,
) -> Uniforms {
    let eye = weather.eye();
    let aspect = screen.0.max(1) as f32 / screen.1.max(1) as f32;
    let wide = |rgb: [f32; 3], last: f32| [rgb[0], rgb[1], rgb[2], last];
    Uniforms {
        origin: wide(eye.at, (game::FOV * 0.5).tan()),
        forward: wide(eye.forward, aspect),
        right: wide(eye.right, internal.0 as f32),
        up: wide(eye.up, internal.1 as f32),
        sun: wide(look.sun_direction(), SUN_ANGULAR),
        wind: wide(weather.drift, game::DETAIL_DRIFT),
        shape: [weather.coverage(), EXTINCTION, ERODE, ANVIL],
        march: [MAX_STEPS, SUN_STEPS, MAX_DISTANCE, STEP_LENGTH],
        layer: [
            game::CLOUD_BOTTOM,
            game::CLOUD_TOP,
            game::PLANET_RADIUS,
            game::SHAPE_SCALE,
        ],
        tune: [game::DETAIL_SCALE, FORWARD_SCATTER, POWDER, SUN_ABSORB],
        mix0: [look.ambient_power, 0.0, HAZE_PER_METRE, look.exposure],
        sky_zenith: wide(look.sky_zenith, 0.0),
        sky_horizon: wide(look.sky_horizon, 0.0),
        sun_colour: wide(look.sun, look.sun_power),
        ambient: wide(look.ambient, 0.0),
        ground_near: wide(look.ground_near, 0.0),
        ground_far: wide(look.ground_far, 0.0),
        haze: wide(look.haze, 0.0),
        screen: [
            screen.0 as f32,
            screen.1 as f32,
            1.0 / screen.0.max(1) as f32,
            1.0 / screen.1.max(1) as f32,
        ],
        finish: [bands, ink, 0.0, 0.0],
    }
}

/// Everything the shader is told, in one buffer.
///
/// Every field is a `vec4` whether it needs four numbers or not. That is not laziness: uniform
/// buffers align members to sixteen bytes, and a struct of nothing but `vec4` has the same
/// layout in WGSL and in Rust with no padding to get wrong. The comments say what the loose
/// components carry.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniforms {
    /// xyz: the eye, in metres. w: tan of half the vertical field of view.
    pub origin: [f32; 4],
    /// xyz: where it looks. w: aspect ratio.
    pub forward: [f32; 4],
    /// xyz: its right. w: internal width in pixels.
    pub right: [f32; 4],
    /// xyz: its up. w: internal height in pixels.
    pub up: [f32; 4],
    /// xyz: towards the sun. w: cosine of the sun's angular radius.
    pub sun: [f32; 4],
    /// xyz: how far the weather has blown, in metres. w: how much faster the detail blows.
    pub wind: [f32; 4],
    /// coverage, extinction per metre, erosion, where the anvil flattens.
    pub shape: [f32; 4],
    /// most steps, sun steps, furthest the march reaches, length of one step.
    pub march: [f32; 4],
    /// cloud base, cloud top, planet radius, metres to one turn of the shape volume.
    pub layer: [f32; 4],
    /// detail scale in metres, forward scattering, powder, sun absorption.
    pub tune: [f32; 4],
    /// ambient strength, spare, haze per metre, exposure.
    pub mix0: [f32; 4],
    /// The sky straight up.
    pub sky_zenith: [f32; 4],
    /// The sky at the horizon.
    pub sky_horizon: [f32; 4],
    /// rgb: sunlight. w: its strength.
    pub sun_colour: [f32; 4],
    /// The light the sky throws back into the clouds.
    pub ambient: [f32; 4],
    /// Sand, lit.
    pub ground_near: [f32; 4],
    /// Sand, in its other tone.
    pub ground_far: [f32; 4],
    /// What distance fades into.
    pub haze: [f32; 4],
    /// width, height, 1/width, 1/height of the window.
    pub screen: [f32; 4],
    /// bands (zero for none), ink strength, spare, spare.
    pub finish: [f32; 4],
}

/// What the finished frame is written in. sRGB, so the shader can work in light and let the
/// hardware do the conversion on the way out.
pub const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// What the march is written in. Sixteen-bit float, because the march's output is light and
/// not colour: a sunlit cloud edge is several times brighter than white and clamping it before
/// the tone map would flatten the one part of the picture worth looking at.
const SCENE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// The pipelines, the volumes and the buffer between the two passes.
pub struct Renderer {
    march: wgpu::RenderPipeline,
    finish: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    frame_bind: wgpu::BindGroup,
    volume_bind: wgpu::BindGroup,
    scene_layout: wgpu::BindGroupLayout,
    scene_sampler: wgpu::Sampler,
    scene: Option<wgpu::TextureView>,
    scene_bind: Option<wgpu::BindGroup>,
    /// Size of the intermediate the march runs at.
    internal: (u32, u32),
}

impl Renderer {
    /// Build the pipelines and upload the noise volumes. Done once.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        shape: &Volume,
        detail: &Volume,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("clouds.wgsl"));

        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("nimbus frame layout"),
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
        let volume_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("nimbus volume layout"),
            entries: &[
                volume_entry(0),
                volume_entry(1),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let scene_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("nimbus scene layout"),
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

        let march = pipeline(
            device,
            &shader,
            "nimbus march",
            &[&frame_layout, &volume_layout],
            "fs_clouds",
            SCENE_FORMAT,
        );
        let finish = pipeline(
            device,
            &shader,
            "nimbus finish",
            &[&frame_layout, &scene_layout],
            "fs_finish",
            OUTPUT_FORMAT,
        );

        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("nimbus uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let frame_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("nimbus frame"),
            layout: &frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            }],
        });

        let shape_view = upload(device, queue, shape, "nimbus shape");
        let detail_view = upload(device, queue, detail, "nimbus detail");
        // Repeat on every axis: the volumes tile, and the whole point of that is that the
        // cloud field can run to the horizon out of eight megabytes of noise.
        let volume_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("nimbus volume sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let volume_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("nimbus volumes"),
            layout: &volume_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&shape_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&detail_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&volume_sampler),
                },
            ],
        });

        let scene_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("nimbus scene sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        Self {
            march,
            finish,
            uniforms,
            frame_bind,
            volume_bind,
            scene_layout,
            scene_sampler,
            scene: None,
            scene_bind: None,
            internal: (0, 0),
        }
    }

    /// What size the march is currently running at.
    pub fn internal(&self) -> (u32, u32) {
        self.internal
    }

    /// Make sure the intermediate is `width` by `height`, rebuilding it if it is not.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let wanted = (width.max(1), height.max(1));
        if self.internal == wanted && self.scene.is_some() {
            return;
        }
        self.internal = wanted;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("nimbus scene"),
            size: wgpu::Extent3d {
                width: wanted.0,
                height: wanted.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SCENE_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.scene_bind = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("nimbus scene"),
            layout: &self.scene_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.scene_sampler),
                },
            ],
        }));
        self.scene = Some(view);
    }

    /// March the clouds into the intermediate, then finish the intermediate into `target`.
    ///
    /// Both passes are one triangle with no vertex buffer, so the whole frame is two draw calls
    /// of three vertices, and everything that costs anything happens in the fragment shader.
    pub fn draw(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        uniforms: &Uniforms,
        target: &wgpu::TextureView,
    ) {
        let (Some(scene), Some(scene_bind)) = (&self.scene, &self.scene_bind) else {
            return;
        };
        queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(uniforms));

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("nimbus"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("nimbus march"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: scene,
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
            pass.set_pipeline(&self.march);
            pass.set_bind_group(0, &self.frame_bind, &[]);
            pass.set_bind_group(1, &self.volume_bind, &[]);
            pass.draw(0..3, 0..1);
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("nimbus finish"),
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
            pass.set_pipeline(&self.finish);
            pass.set_bind_group(0, &self.frame_bind, &[]);
            pass.set_bind_group(1, scene_bind, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit([encoder.finish()]);
    }
}

/// One three-dimensional texture in a bind group layout.
fn volume_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D3,
            multisampled: false,
        },
        count: None,
    }
}

/// One of the two passes: a full-screen triangle into one colour target.
fn pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    label: &str,
    layouts: &[&wgpu::BindGroupLayout],
    entry: &str,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let bind_group_layouts: Vec<Option<&wgpu::BindGroupLayout>> =
        layouts.iter().map(|layout| Some(*layout)).collect();
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &bind_group_layouts,
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(entry),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

/// Put a noise volume on the GPU.
fn upload(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    volume: &Volume,
    label: &str,
) -> wgpu::TextureView {
    let size = wgpu::Extent3d {
        width: volume.size,
        height: volume.size,
        depth_or_array_layers: volume.size,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D3,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &volume.data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * volume.size),
            rows_per_image: Some(volume.size),
        },
        size,
    );
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}
