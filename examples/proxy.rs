extern crate winit;

fn main() {
    let mut event_loop = winit::event_loop::EventLoop::new();

    let _window = winit::WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&event_loop)
        .unwrap();

    let proxy = event_loop.create_proxy();

    std::thread::spawn(move || {
        // Wake up the `event_loop` once every second.
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            proxy.wakeup().unwrap();
        }
    });

    event_loop.run_forever(|event| {
        println!("{:?}", event);
        match event {
            winit::Event::WindowEvent { event: winit::WindowEvent::CloseRequested, .. } =>
                winit::event_loop::ControlFlow::Break,
            _ => winit::event_loop::ControlFlow::Continue,
        }
    });
}
