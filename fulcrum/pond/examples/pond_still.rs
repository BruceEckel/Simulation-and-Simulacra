//! One frame, rendered on a device with no window attached, written to a PNG.
//!
//! ```sh
//! cargo run -p pond --release --example pond_still -- still.png 1920 1200 0 240
//! ```
//!
//! The arguments are the file, the size in pixels, which palette, and how many ticks to run
//! before the picture is taken. The run is scripted: the pointer comes in from the left and
//! draws across the water, taps twice, and holds once, so the picture has a wake, a couple
//! of rings, and a bowl in it.
//!
//! A shader is the one part of a program you cannot read your way to correctness in: the only
//! question worth asking about a drawing is what it looks like, and the answer is a picture.
//! This renders through the same pass the window does, and it times the tick on the
//! CPU and the frame on the GPU on the way.

use fulcrum::prelude::*;
use pond::game::{GamePlugin, Lantern, Water};
use pond::look::LOOKS;
use pond::screen::{OUTPUT_FORMAT, Renderer, Scene, compose};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap_or_else(|| "still.png".to_string());
    let width = number(args.next(), 1920);
    let height = number(args.next(), 1200);
    let palette = number(args.next(), 0) as usize % LOOKS.len();
    let ticks = number(args.next(), 240);

    let mut app = Fulcrum::with_config(FulcrumConfig {
        seed: 7,
        window_size: (width, height),
        ..Default::default()
    })
    .with_plugin(GamePlugin);
    app.run_startup();
    app.world_mut().resource_mut::<CommandOutbox>().send(
        pond::game::FIELD_COMMAND,
        pond::game::field_payload(vec2(width as f32, height as f32)),
    );

    let (w, h) = (width as f32, height as f32);
    let started = std::time::Instant::now();
    for tick in 0..ticks {
        {
            let mut input = app.world_mut().resource_mut::<Input>();
            let t = tick as f32 / 60.0;
            // A slow stroke across, then a tap, a held press, and another tap.
            let at = if t < 2.5 {
                vec2(
                    w * (0.15 + 0.5 * t / 2.5),
                    h * (0.55 - 0.15 * (t * 1.3).sin()),
                )
            } else {
                vec2(w * 0.65, h * 0.35)
            };
            input.push_cursor(at);
            let down = (tick == 100) || (140..=200).contains(&tick) || (tick == 215);
            let was = input.mouse_pressed(MouseButton::Left);
            if down != was {
                input.push_mouse_button(MouseButton::Left, down);
            }
            if tick == 100 || tick == 215 {
                // A tap elsewhere: put the pointer there for the one tick.
                input.push_cursor(vec2(w * 0.3, h * 0.3));
            }
            input.sample(|screen| screen);
        }
        app.tick();
    }
    let each = started.elapsed() / ticks.max(1);
    let water = app.world_mut().resource::<Water>().clone();
    let lantern = *app.world_mut().resource::<Lantern>();
    println!(
        "{} x {} cells at {} px: {:.2} ms a tick, peak {:.3}",
        water.across,
        water.down,
        water.cell,
        each.as_secs_f32() * 1000.0,
        water.peak(),
    );

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .expect("no GPU adapter");
    println!("adapter: {}", adapter.get_info().name);
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("pond still"),
        ..Default::default()
    }))
    .expect("no device");

    let mut renderer = Renderer::new(&device);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("pond still"),
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

    let scene = Scene {
        lantern: [lantern.at.x, lantern.at.y],
        glow: 1.0,
        stops: LOOKS[palette].stops,
        drift: 0.0,
        time: ticks as f32 / 60.0,
        scale: 1.0,
    };
    let uniforms = compose(&water, &scene, (width, height));
    renderer.carry(&device, &queue, &water);

    // Once to warm the pipeline up, then a handful of frames with a fence after them, so the
    // time printed is the time a frame really takes rather than the time to queue one.
    renderer.draw(&device, &queue, &uniforms, &view, (width, height));
    wait(&device);
    let timed = std::time::Instant::now();
    const FRAMES: u32 = 8;
    for _ in 0..FRAMES {
        renderer.draw(&device, &queue, &uniforms, &view, (width, height));
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
        label: Some("pond readback"),
        size: u64::from(stride) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("pond readback"),
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
