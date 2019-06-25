extern crate winit;

fn main() {
    let mut event_loop = winit::event_loop::EventLoop::new();

    let window = winit::WindowBuilder::new().with_decorations(false)
                                                 .with_transparency(true)
                                                 .build(&event_loop).unwrap();

    window.set_title("A fantastic window!");

    event_loop.run_forever(|event| {
        println!("{:?}", event);

        match event {
            winit::Event::WindowEvent { event: winit::WindowEvent::CloseRequested, .. } => winit::event_loop::ControlFlow::Break,
            _ => winit::event_loop::ControlFlow::Continue,
        }
    });
}
