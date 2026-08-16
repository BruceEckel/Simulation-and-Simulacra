//! One frame, rendered on a device with no window attached, written to a PNG.
//!
//! ```sh
//! cargo run -p moebius3 --release --example moebius3_still -- still.png 1920 1200 0 900 40 3 1.8 0.10
//! ```
//!
//! The arguments are the file, the size in pixels, which palette, how far into the weather to
//! look in seconds, which way to face in degrees, how many arcs an element is built from, how
//! wide the line around a cloud is in pixels, and how far apart the hatch lines run as a fraction
//! of an element's radius. A hatch of nought turns the shading off. The clock is the whole state
//! of the sky, and of where the traveller has got to: every circle in the drawing is a function
//! of it, so any moment can be rendered without running up to it, and the last three are the
//! settings the keys move while the window is open.
//!
//! A shader is the one part of a program you cannot read your way to correctness in: the only
//! question worth asking about a drawing is what it looks like, and the answer is a picture.
//! This renders through exactly the same pass the window does, and times eight frames on the
//! way, so it doubles as the benchmark.

use moebius3::cloud::{Sky, Style};
use moebius3::game::Weather;
use moebius3::look::LOOKS;
use moebius3::sky::{OUTPUT_FORMAT, Renderer, Slab, compose};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap_or_else(|| "still.png".to_string());
    let width = number(args.next(), 1920);
    let height = number(args.next(), 1200);
    let palette = number(args.next(), 0) as usize % LOOKS.len();
    let clock = number(args.next(), 400) as f32;
    let yaw = number(args.next(), 0) as f32 * std::f32::consts::TAU / 360.0;
    // Taken one at a time and in the order they are written on the command line, because they
    // come off one iterator and a struct that reads them out of order would read them out of
    // order from the arguments too.
    let arcs = number(args.next(), Style::default().arcs);
    let cloud_ink = decimal(args.next(), Style::default().cloud_ink);
    let hatch = decimal(args.next(), Style::default().hatch);
    let style = Style {
        arcs,
        cloud_ink,
        // Nought is not a spacing, so it is the way to ask for no shading at all.
        shade: hatch > 0.0,
        hatch,
    }
    .clamped();

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .expect("no GPU adapter");
    println!("adapter: {}", adapter.get_info().name);
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("moebius3 still"),
        ..Default::default()
    }))
    .expect("no device");

    let renderer = Renderer::new(&device);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("moebius3 still"),
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
        clock,
        yaw,
        ..Default::default()
    };
    let started = std::time::Instant::now();
    let sky = Sky::at(weather.clock, style);
    let mut slab = Slab::boxed();
    let uniforms = compose(
        &weather,
        &sky,
        &LOOKS[palette],
        style,
        (width, height),
        &mut slab,
    );
    println!(
        "sky: {:?} for {} circles in {} outlines, {} of them in frame",
        started.elapsed(),
        sky.discs.len(),
        sky.groups.len(),
        uniforms.counts[0] as usize,
    );

    // Once to warm the pipeline up, then a handful of frames with a fence after them, so the
    // time printed is the time a frame really takes rather than the time to queue one.
    renderer.draw(&device, &queue, &uniforms, &slab, &view);
    wait(&device);
    let timed = std::time::Instant::now();
    const FRAMES: u32 = 8;
    for _ in 0..FRAMES {
        renderer.draw(&device, &queue, &uniforms, &slab, &view);
    }
    wait(&device);
    let each = timed.elapsed() / FRAMES;
    println!(
        "{width}x{height}: {:.2} ms a frame ({:.0} fps)",
        each.as_secs_f32() * 1000.0,
        1.0 / each.as_secs_f32()
    );

    save(&device, &queue, &target, width, height, &path);
    println!(
        "{path}: palette {}, {} arcs an element, {:.1} px line, {}",
        LOOKS[palette].name,
        style.arcs,
        style.cloud_ink,
        if style.shade {
            format!("hatch every {:.3} of a radius", style.hatch)
        } else {
            "no shading".to_string()
        }
    );
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
        label: Some("moebius3 readback"),
        size: u64::from(stride) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("moebius3 readback"),
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

/// One whole-number argument, or a default.
fn number(arg: Option<String>, fallback: u32) -> u32 {
    arg.and_then(|value| value.parse().ok()).unwrap_or(fallback)
}

/// One argument with a fraction in it, or a default.
fn decimal(arg: Option<String>, fallback: f32) -> f32 {
    arg.and_then(|value| value.parse().ok()).unwrap_or(fallback)
}
