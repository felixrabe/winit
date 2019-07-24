use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() {
    let event_loop = EventLoop::new();

    let mut window = Some(WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&event_loop)
        .unwrap());

    event_loop.run(move |event, _, control_flow| {
        // println!("{:?}", event);

        match &window {
            Some(w) => {
                match event {
                    Event::WindowEvent {
                        event: WindowEvent::CloseRequested,
                        window_id,
                    } if window_id == w.id() => {
                        window = None;
                        *control_flow = ControlFlow::Exit;
                    },
                    _ => *control_flow = ControlFlow::Wait,
                }
            }
            None => {
                *control_flow = ControlFlow::Exit;
            }
        }
    });
}
