mod renderer;
mod dyn_result;

use crate::renderer::Renderer;
use crate::dyn_result::DynResult;

fn main() -> DynResult<()> {
    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::Window::new(&event_loop)?;
    let renderer = Renderer::new(&window);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = winit::event_loop::ControlFlow::Wait;

        match event {
            winit::event::Event::WindowEvent {
                event: winit::event::WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => {
                let _ = &renderer; // so we can drop the renderer
                *control_flow = winit::event_loop::ControlFlow::Exit
            }
            _ => (),
        }
    });
}
