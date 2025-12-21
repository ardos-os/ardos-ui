use std::{cell::RefCell, ops::Deref, rc::Rc};

mod clay_renderer;
mod element;
mod focus_system;
mod font_manager;
mod input;
mod render_context;
mod window_options;
mod winit;
use clay_layout::{
	Declaration, grow,
	layout::Alignment,
	math::{Dimensions, Vector2},
};
mod hooks;
pub use element::{Element, component::Component, container::*, text::Text};
pub use hooks::*;
pub use hyprui_rsml_compiler::rsml;
pub(crate) use input::winit_impl::WinitInputManager;
pub use input::{InputManager, NamedKey, NativeKey};
pub use render_context::RenderContext;
pub use window_options::WindowOptions;

use crate::{
	clay_renderer::clay_skia_render,
	focus_system::GLOBAL_FOCUS_MANAGER,
	font_manager::FontManager,
	input::Key,
	winit::{Callbacks, WinitApp},
};

pub mod layer_shell {
	pub use crate::window_options::{Anchor, KeyboardInteractivity, LayerShellOptions};
}
thread_local! {
		static REQUEST_REDRAW: RefCell<Box<dyn Fn()>> = RefCell::new(Box::new(|| {}));
}

pub(crate) trait GlobalClosure {
	fn call(&'static self);
}

impl GlobalClosure for std::thread::LocalKey<RefCell<Box<dyn Fn()>>> {
	fn call(&'static self) {
		self.with(|r| r.borrow()())
	}
}
/// Creates and displays a HyprUI window with a declarative root component.
///
/// This function sets up the entire environment required to render a graphical interface
/// using HyprUI's component system. It manages the window lifecycle, rendering,
/// font management, user input, and automatic UI updates.
///
/// # Parameters
///
/// - `component`: A function or closure representing the root component of your UI.
///   It must accept the given `props` and return a `Box<dyn Element>`.
///   The component will be automatically wrapped in a [`Component`] to ensure context and state isolation.
/// - `props`: The initial properties to be passed to the root component.
///   Must implement [`Clone`] and `'static`.
/// - `options`: Window configuration options such as title, preferred size, layer mode, etc.
///   See [`WindowOptions`] for details.
///
/// # Example
///
/// ```rust,no_run
/// use hyprui::{create_window, WindowOptions, Text};
///
/// fn root_component(_: ()) -> Box<dyn hyprui::Element> {
///     Box::new(Text::new("Hello, HyprUI!"))
/// }
///
/// fn main() {
///     create_window(
///         root_component,
///         (),
///         WindowOptions {
///             title: "My HyprUI App".into(),
///             preferred_size: (400.0, 300.0),
///             ..Default::default()
///         },
///     );
/// }
/// ```
///
/// # Notes
///
/// - The window and renderer lifecycle are fully managed by this function.
/// - The root component will be called every frame to update the UI.
/// - Mouse, keyboard, and IME input are handled transparently.
/// - For proper state isolation, always use [`Component::new`] for dynamic child components.
///
/// # Panics
///
/// May panic if there is an error initializing the graphics system or event loop.
///
/// # Requirements
///
/// - A graphical environment must be available.
/// - HyprUI must be properly compiled for the target operating system.
///
/// # See also
///
/// - [`Component`]
/// - [`WindowOptions`]
/// - [`Element`]
pub fn create_window<Props: Clone + 'static>(
	component: impl Clone + Copy + Fn(Props) -> Box<dyn Element> + 'static,
	props: Props,
	options: WindowOptions,
) {
	color_eyre::install().ok();

	let clay = Rc::new(RefCell::new(clay_layout::Clay::new((0.0, 0.0).into())));
	let mut font_manager = FontManager::new();
	let input_manager = Rc::new(RefCell::new(WinitInputManager::new()));

	let winit_app = WinitApp::new(
		options,
		Callbacks {
			on_render_callback: {
				let clay = Rc::clone(&clay);
				let props = props.clone();
				let input_manager = Rc::clone(&input_manager);
				Box::new(move |canvas| {
					let mut clay = clay.borrow_mut();
					let mut input_manager_ref = input_manager.borrow_mut();
					GLOBAL_FOCUS_MANAGER.with_borrow_mut(|f| {
						f.add_root();
						if input_manager_ref.is_key_just_pressed(Key::Named(NamedKey::Tab)) {
							if input_manager_ref.is_key_pressed(Key::Named(NamedKey::Shift)) {
								f.focus_prev();
							} else {
								f.focus_next();
							}
						}

						if (!input_manager_ref.cursor_hit_something()
							&& (input_manager_ref.is_mouse_button_just_pressed(0)
								|| input_manager_ref.is_mouse_button_just_pressed(1)))
							|| input_manager_ref.is_key_just_pressed(Key::Named(NamedKey::Escape))
						{
							f.blur();
						}
						f.new_frame();
					});
					font_manager.update_clay_measure_function(&mut clay);
					let root_component = Component::new(component, props.clone());

					{
						let mut c = clay.begin();

						let mut render_ctx = RenderContext {
							c: &mut c,
							font_manager: &mut font_manager,
							input_manager: input_manager_ref.deref(),
						};
						root_component.render(&mut render_ctx);

						clay_skia_render::<()>(canvas, c.end(), |_, _, _| {}, &font_manager.get_fonts());
					}
					input_manager_ref.update();
				})
			},
			on_mouse_move: {
				let clay = Rc::clone(&clay);
				let input_manager = Rc::clone(&input_manager);
				Box::new(move |x, y| {
					input_manager
						.borrow_mut()
						.set_mouse_position(x as f32, y as f32);

					let clay = clay.borrow_mut();
					let (mx, my) = input_manager.borrow().mouse_position();
					let pressed = input_manager.borrow().is_mouse_button_pressed(0); // 0 = botão esquerdo
					clay.pointer_state(Vector2::new(mx, my), pressed);
				})
			},
			on_mouse_button: {
				let clay = Rc::clone(&clay);
				let input_manager = Rc::clone(&input_manager);
				Box::new(move |pressed, button| {
					input_manager.borrow_mut().set_mouse_button(button, pressed);

					let clay = clay.borrow_mut();
					let (mx, my) = input_manager.borrow().mouse_position();
					let pressed = input_manager.borrow().is_mouse_button_pressed(0); // 0 = botão esquerdo
					clay.pointer_state(Vector2::new(mx, my), pressed);
				})
			},
			on_key_event: {
				let input_manager = Rc::clone(&input_manager);
				Box::new(move |event| {
					input_manager.borrow_mut().handle_key_event(event);
				})
			},
			on_ime_event: {
				let input_manager = Rc::clone(&input_manager);
				Box::new(move |ime| {
					input_manager.borrow_mut().handle_ime_event(ime);
				})
			},
			on_window_resize: {
				let clay = Rc::clone(&clay);
				Box::new(move |width, height| {
					let clay = clay.borrow_mut();
					clay.set_layout_dimensions(Dimensions::new(width as _, height as _));
				})
			},
		},
	);

	winit_app.run();
}
