extern crate winit;

fn main() {
    let mut event_loop = winit::event_loop::EventLoop::new();

    let window = winit::WindowBuilder::new()
        .with_title("Super Cursor Grab'n'Hide Simulator 9000")
        .build(&event_loop)
        .unwrap();

    event_loop.run_forever(|event| {
        if let winit::Event::WindowEvent { event, .. } = event {
            use winit::WindowEvent::*;
            match event {
                CloseRequested => return winit::event_loop::ControlFlow::Break,
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
                        Escape => return winit::event_loop::ControlFlow::Break,
                        G => window.grab_cursor(!modifiers.shift).unwrap(),
                        H => window.hide_cursor(!modifiers.shift),
                        _ => (),
                    }
                }
                _ => (),
            }
        }
        winit::event_loop::ControlFlow::Continue
    });
}
