extern crate winit;

fn main() {
    let mut events_loop = winit::events_loop::EventsLoop::new();

    let window = winit::WindowBuilder::new()
        .with_title("Super Cursor Grab'n'Hide Simulator 9000")
        .build(&events_loop)
        .unwrap();

    events_loop.run_forever(|event| {
        if let winit::Event::WindowEvent { event, .. } = event {
            use winit::WindowEvent::*;
            match event {
                CloseRequested => return winit::events_loop::ControlFlow::Break,
                KeyboardInput {
                    input: winit::KeyboardInput {
                        state: winit::ElementState::Released,
                        virtual_keycode: Some(key),
                        modifiers,
                        ..
                    },
                    ..
                } => {
                    use winit::VirtualKeyCode::*;
                    match key {
                        Escape => return winit::events_loop::ControlFlow::Break,
                        G => window.grab_cursor(!modifiers.shift).unwrap(),
                        H => window.hide_cursor(!modifiers.shift),
                        _ => (),
                    }
                }
                _ => (),
            }
        }
        winit::events_loop::ControlFlow::Continue
    });
}
