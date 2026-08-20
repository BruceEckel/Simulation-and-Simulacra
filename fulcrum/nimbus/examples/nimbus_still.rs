//! One frame, rendered on a device with no window attached, written to a PNG.
//!
//! ```sh
//! cargo run -p nimbus --release --example nimbus_still -- still.png 1920 1200 0 1 1
//! ```
//!
//! The arguments are the file, the size in pixels, which palette, the internal scale as a
//! percentage of that size, whether to finish in flat bands, and how far the wind has carried
//! the weather.
//!
//! This is not a toy. A shader is the one part of a program you cannot read your way to
//! correctness in: the only question worth asking about a cloud is what it looks like, and the
//! answer is a picture. Being able to get that picture in two seconds, off the same code the
//! window runs, is what makes the thing tunable at all. It also renders every frame it is asked
//! for at whatever size it is asked for, so it doubles as the timing harness.

use nimbus::game::Weather;
use nimbus::look::LOOKS;
use nimbus::noise::{detail_volume, shape_volume};
use nimbus::sky::{OUTPUT_FORMAT, Renderer, compose};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap_or_else(|| "still.png".to_string());
    let width = number(args.next(), 1920);
    let height = number(args.next(), 1200);
    let palette = number(args.next(), 0) as usize % LOOKS.len();
    let scale = (number(args.next(), 100) as f32 / 100.0).clamp(0.1, 1.0);
    let banded = number(args.next(), 0) != 0;
    let drift = number(args.next(), 0) as f32;

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .expect("no GPU adapter");
    println!("adapter: {}", adapter.get_info().name);
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("nimbus still"),
        ..Default::default()
    }))
    .expect("no device");

    let started = std::time::Instant::now();
    let shape = shape_volume(7);
    let detail = detail_volume(11);
    println!("volumes: {:?}", started.elapsed());

    let mut renderer = Renderer::new(&device, &queue, &shape, &detail);
    let internal = (
        ((width as f32 * scale) as u32).max(1),
        ((height as f32 * scale) as u32).max(1),
    );
    renderer.resize(&device, internal.0, internal.1);

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("nimbus still"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: OUTPUT_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());

    let weather = Weather {
        drift: [-drift, 0.0, -drift * 0.22],
        ..Default::default()
    };
    let uniforms = compose(
        &weather,
        &LOOKS[palette],
        (width, height),
        internal,
        if banded { 6.0 } else { 0.0 },
        0.85,
    );

    // Once to warm the pipelines up, then a handful of frames with a fence between them, so the
    // time printed is the time a frame really takes rather than the time to queue one.
    renderer.draw(&device, &queue, &uniforms, &view, (width, height));
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        })
        .ok();
    let timed = std::time::Instant::now();
    const FRAMES: u32 = 8;
    for _ in 0..FRAMES {
        renderer.draw(&device, &queue, &uniforms, &view, (width, height));
    }
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        })
        .ok();
    let each = timed.elapsed() / FRAMES;
    println!(
        "{width}x{height} at {}x{} internal: {:.2} ms a frame ({:.0} fps)",
        internal.0,
        internal.1,
        each.as_secs_f32() * 1000.0,
        1.0 / each.as_secs_f32()
    );

    save(&device, &queue, &target, width, height, &path);
    println!("{path}: palette {}", LOOKS[palette].name);
}

/// Pull the finished texture back across the bus and write it out.
fn save(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    target: &wgpu::Texture,
    width: u32,
    height: u32,
    path: &str,
) {
    // Copies out of a texture want their rows aligned, so the buffer is padded and the padding
    // is dropped again on the way into the PNG.
    let stride = (width * 4).div_ceil(256) * 256;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("nimbus readback"),
        size: u64::from(stride) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("nimbus readback"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);

    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |result| {
        result.expect("map the readback buffer");
    });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        })
        .ok();

    let mapped = slice.get_mapped_range();
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for row in 0..height {
        let start = (row * stride) as usize;
        pixels.extend_from_slice(&mapped[start..start + (width * 4) as usize]);
    }
    drop(mapped);
    buffer.unmap();

    image::save_buffer(
        path,
        &pixels,
        width,
        height,
        image::ExtendedColorType::Rgba8,
    )
    .expect("write the still");
}

/// One numeric argument, or a default.
fn number(arg: Option<String>, fallback: u32) -> u32 {
    arg.and_then(|value| value.parse().ok()).unwrap_or(fallback)
}
