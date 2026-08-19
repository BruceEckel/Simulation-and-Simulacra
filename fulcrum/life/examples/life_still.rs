//! One frame, rendered on a device with no window attached, written to a PNG.
//!
//! ```sh
//! cargo run -p life --release --example life_still -- still.png 1920 1200 0 5 0 200
//! ```
//!
//! The arguments are the file, the size in pixels, which rule, which cell size, which palette,
//! how many generations to run before the picture is taken, and how the field is read as three
//! bits: 1 for age, 2 for ghost trails, 4 for the line between cells.
//!
//! A shader is the one part of a program you cannot read your way to correctness in: the only
//! question worth asking about a drawing is what it looks like, and the answer is a picture.
//! This renders through exactly the same pass the window does, and it times both halves on the
//! way — the generations on the CPU and the frame on the GPU — so it doubles as the benchmark
//! for the case the whole piece is built around, which is one cell to one physical pixel.

use fulcrum::prelude::SimRng;
use life::game::{Board, CELL_SIZES, Start, grid_for};
use life::look::LOOKS;
use life::rules::RULES;
use life::screen::{OUTPUT_FORMAT, Reading, Renderer, compose};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap_or_else(|| "still.png".to_string());
    let width = number(args.next(), 1920);
    let height = number(args.next(), 1200);
    let rule = &RULES[number(args.next(), 0) as usize % RULES.len()];
    let size = (number(args.next(), 5) as usize).min(CELL_SIZES.len() - 1);
    let palette = number(args.next(), 0) as usize % LOOKS.len();
    let generations = number(args.next(), 200);
    let read = number(args.next(), 3);
    let reading = Reading {
        ageing: read & 1 != 0,
        ghosts: read & 2 != 0,
        edges: read & 4 != 0,
    };

    let cell = CELL_SIZES[size];
    let (across, down) = grid_for(fulcrum::prelude::vec2(width as f32, height as f32), cell);
    let mut board = Board::new(across, down);
    let mut rng = SimRng::seeded(0x11f3);
    board.sow(rule, Start::Native, &mut rng);

    println!(
        "{}  {}  {} x {} = {} cells at {} px",
        rule.name,
        rule.rulestring(),
        across,
        down,
        across as u64 * down as u64,
        cell,
    );

    let started = std::time::Instant::now();
    for _ in 0..generations {
        board.step(rule, true);
    }
    let each = started.elapsed() / generations.max(1);
    println!(
        "{generations} generations: {:.2} ms each ({:.0} a second), population {}",
        each.as_secs_f32() * 1000.0,
        1.0 / each.as_secs_f32().max(f32::EPSILON),
        board.population,
    );
    board.repaint(rule);

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .expect("no GPU adapter");
    println!("adapter: {}", adapter.get_info().name);
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("life still"),
        ..Default::default()
    }))
    .expect("no device");

    let mut renderer = Renderer::new(&device);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("life still"),
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

    let uniforms = compose(&board, cell, &LOOKS[palette], reading, (width, height));
    renderer.carry(&device, &queue, &board);

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
    let frame = timed.elapsed() / FRAMES;
    println!(
        "{width}x{height}: {:.2} ms a frame ({:.0} fps)",
        frame.as_secs_f32() * 1000.0,
        1.0 / frame.as_secs_f32().max(f32::EPSILON),
    );

    save(&device, &queue, &target, width, height, &path);
    println!("{path}: {}", LOOKS[palette].name);
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
        label: Some("life readback"),
        size: u64::from(stride) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("life readback"),
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
