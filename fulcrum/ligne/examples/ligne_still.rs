//! One frame, rendered on a device with no window attached, written to a PNG.
//!
//! ```sh
//! cargo run -p ligne --release --example ligne_still -- still.png 1920 1200 0 0
//! ```
//!
//! The arguments are the file, the size in pixels, which palette, and how far the wind has
//! carried the sky in metres.
//!
//! A shader is the one part of a program you cannot read your way to correctness in: the only
//! question worth asking about a drawing is what it looks like, and the answer is a picture.
//! This renders through exactly the same pass the window does, and times eight frames on the
//! way, so it doubles as the benchmark.

use ligne::field::sheet;
use ligne::game::{WIND, Weather};
use ligne::look::LOOKS;
use ligne::sky::{OUTPUT_FORMAT, Renderer, compose};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap_or_else(|| "still.png".to_string());
    let width = number(args.next(), 1920);
    let height = number(args.next(), 1200);
    let palette = number(args.next(), 0) as usize % LOOKS.len();
    let carried = number(args.next(), 0) as f32;

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .expect("no GPU adapter");
    println!("adapter: {}", adapter.get_info().name);
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("ligne still"),
        ..Default::default()
    }))
    .expect("no device");

    let started = std::time::Instant::now();
    let noise = sheet(7);
    println!(
        "sheet: {:?} for {} levels",
        started.elapsed(),
        noise.levels.len()
    );

    let renderer = Renderer::new(&device, &queue, &noise);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("ligne still"),
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
        drift: [-WIND[0] * carried, -WIND[1] * carried],
        boil: carried * 0.11,
        ..Default::default()
    };
    let uniforms = compose(&weather, &LOOKS[palette], (width, height));

    // Once to warm the pipeline up, then a handful of frames with a fence after them, so the
    // time printed is the time a frame really takes rather than the time to queue one.
    renderer.draw(&device, &queue, &uniforms, &view);
    wait(&device);
    let timed = std::time::Instant::now();
    const FRAMES: u32 = 8;
    for _ in 0..FRAMES {
        renderer.draw(&device, &queue, &uniforms, &view);
    }
    wait(&device);
    let each = timed.elapsed() / FRAMES;
    println!(
        "{width}x{height}: {:.2} ms a frame ({:.0} fps)",
        each.as_secs_f32() * 1000.0,
        1.0 / each.as_secs_f32()
    );

    save(&device, &queue, &target, width, height, &path);
    println!("{path}: palette {}", LOOKS[palette].name);
}

/// Block until the GPU has caught up.
fn wait(device: &wgpu::Device) {
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        })
        .ok();
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
        label: Some("ligne readback"),
        size: u64::from(stride) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("ligne readback"),
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
    wait(device);

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
