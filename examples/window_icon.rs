extern crate winit;
extern crate image;

use std::path::Path;
use winit::Icon;

fn main() {
    // You'll have to choose an icon size at your own discretion. On X11, the desired size varies
    // by WM, and on Windows, you still have to account for screen scaling. Here we use 32px,
    // since it seems to work well enough in most cases. Be careful about going too high, or
    // you'll be bitten by the low-quality downscaling built into the WM.
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/icon.png");

    let icon = load_icon(Path::new(path));

    let mut event_loop = winit::event_loop::EventsLoop::new();

    let window = winit::WindowBuilder::new()
        .with_title("An iconic window!")
        // At present, this only does anything on Windows and X11, so if you want to save load
        // time, you can put icon loading behind a function that returns `None` on other platforms.
        .with_window_icon(Some(icon))
        .build(&event_loop)
        .unwrap();

    event_loop.run_forever(|event| {
        if let winit::Event::WindowEvent { event, .. } = event {
            use winit::WindowEvent::*;
            match event {
                CloseRequested => return winit::event_loop::ControlFlow::Break,
                DroppedFile(path) => {
                    use image::GenericImageView;

                    window.set_window_icon(Some(load_icon(&path)));
                },
                _ => (),
            }
        }
        winit::event_loop::ControlFlow::Continue
    });
}

fn load_icon(path: &Path) -> Icon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::open(path).expect("Failed to open icon path");
        use image::{GenericImageView, Pixel};
        let (width, height) = image.dimensions();
        let mut rgba = Vec::with_capacity((width * height) as usize * 4);
        for (_, _, pixel) in image.pixels() {
            rgba.extend_from_slice(&pixel.to_rgba().data);
        }
        (rgba, width, height)
    };
    Icon::from_rgba(icon_rgba, icon_width, icon_height).expect("Failed to open icon")
}
