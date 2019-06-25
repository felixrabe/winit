extern crate winit;

use std::collections::HashMap;

fn main() {
    let mut event_loop = winit::event_loop::EventLoop::new();

    let mut windows = HashMap::new();
    for _ in 0..3 {
        let window = winit::Window::new(&event_loop).unwrap();
        windows.insert(window.id(), window);
    }

    event_loop.run_forever(|event| {
        match event {
            winit::Event::WindowEvent {
                event: winit::WindowEvent::CloseRequested,
                window_id,
            } => {
                println!("Window {:?} has received the signal to close", window_id);

                // This drops the window, causing it to close.
                windows.remove(&window_id);

                if windows.is_empty() {
                    return winit::event_loop::ControlFlow::Break;
                }
            }
            _ => (),
        }
        winit::event_loop::ControlFlow::Continue
    })
}
