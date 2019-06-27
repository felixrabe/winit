extern crate winit;

fn main() {
    let mut events_loop = winit::events_loop::EventsLoop::new();

    let _window = winit::WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&events_loop)
        .unwrap();

    events_loop.run_forever(|event| {
        println!("{:?}", event);

        match event {
            winit::Event::WindowEvent {
                event: winit::WindowEvent::CloseRequested,
                ..
            } => winit::events_loop::ControlFlow::Break,
            _ => winit::events_loop::ControlFlow::Continue,
        }
    });
}
