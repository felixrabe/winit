extern crate winit;

use winit::dpi::LogicalSize;

fn main() {
    let mut event_loop = winit::event_loop::EventsLoop::new();

    let window = winit::WindowBuilder::new()
        .build(&event_loop)
        .unwrap();

    window.set_min_dimensions(Some(LogicalSize::new(400.0, 200.0)));
    window.set_max_dimensions(Some(LogicalSize::new(800.0, 400.0)));

    event_loop.run_forever(|event| {
        println!("{:?}", event);

        match event {
            winit::Event::WindowEvent { event: winit::WindowEvent::CloseRequested, .. } => winit::event_loop::ControlFlow::Break,
            _ => winit::event_loop::ControlFlow::Continue,
        }
    });
}
