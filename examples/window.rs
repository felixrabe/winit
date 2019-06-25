extern crate winit;

fn main() {
    let mut event_loop = winit::event_loop::EventLoop::new();

    let _window = winit::WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&event_loop)
        .unwrap();

    event_loop.run_forever(|event| {
        println!("{:?}", event);

        match event {
            winit::Event::WindowEvent {
                event: winit::WindowEvent::CloseRequested,
                ..
            } => winit::event_loop::ControlFlow::Break,
            _ => winit::event_loop::ControlFlow::Continue,
        }
    });
}
