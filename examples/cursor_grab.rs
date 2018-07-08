extern crate winit;

fn main() {
    let mut event_loop = winit::EventLoop::new();

    let window = winit::WindowBuilder::new()
        .with_title("Super Cursor Grab'n'Hide Simulator 9000")
        .build(&event_loop)
        .unwrap();

    event_loop.run(move |event| {
        if let winit::Event::WindowEvent { event, .. } = event {
            use winit::WindowEvent::*;
            match event {
                CloseRequested => return winit::ControlFlow::Exit,
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
                        Escape => return winit::ControlFlow::Exit,
                        G => window.grab_cursor(!modifiers.shift).unwrap(),
                        H => window.hide_cursor(!modifiers.shift),
                        _ => (),
                    }
                }
                _ => (),
            }
        }
        winit::ControlFlow::Wait
    });
}
