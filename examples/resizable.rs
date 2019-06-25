extern crate winit;

fn main() {
    let mut event_loop = winit::event_loop::EventsLoop::new();

    let mut resizable = false;

    let window = winit::WindowBuilder::new()
        .with_title("Hit space to toggle resizability.")
        .with_dimensions((400, 200).into())
        .with_resizable(resizable)
        .build(&event_loop)
        .unwrap();

    event_loop.run_forever(|event| {
        match event {
            winit::Event::WindowEvent { event, .. } => match event {
                winit::WindowEvent::CloseRequested => return winit::event_loop::ControlFlow::Break,
                winit::WindowEvent::KeyboardInput {
                    input:
                        winit::KeyboardInput {
                            virtual_keycode: Some(winit::VirtualKeyCode::Space),
                            state: winit::ElementState::Released,
                            ..
                        },
                    ..
                } => {
                    resizable = !resizable;
                    println!("Resizable: {}", resizable);
                    window.set_resizable(resizable);
                }
                _ => (),
            },
            _ => (),
        };
        winit::event_loop::ControlFlow::Continue
    });
}
