mod dyn_result;
mod renderer;

use crate::dyn_result::DynResult;
use crate::renderer::Renderer;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::Window;

fn main() -> DynResult<()> {
    let event_loop = EventLoop::new();
    let window = Window::new(&event_loop)?;
    let mut renderer = Renderer::new(&window)?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => {
                let _ = &renderer; // so we can drop the renderer
                *control_flow = ControlFlow::Exit
            }
            Event::MainEventsCleared => {
                renderer.render().unwrap();
            }

            _ => (),
        }
    });
}
