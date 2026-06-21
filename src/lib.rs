use std::{cell::RefCell, ops::Deref, rc::Rc, time::Instant};

mod clay_renderer;
mod element;
mod focus_system;
mod font_manager;
mod input;
mod render_context;
mod util;
mod window_options;
mod winit;
use clay_layout::{
	Declaration, grow,
	layout::Alignment,
	math::{Dimensions, Vector2},
};
mod hooks;
pub use element::{Element, component::Component, container::*, input::*, text::Text};
pub use hooks::*;
pub use ardos_ui_rsml_compiler::rsml;
pub(crate) use input::winit_impl::WinitInputManager;
pub use input::{InputManager, NamedKey, NativeKey};
pub use render_context::RenderContext;
pub use util::frame_pool::{FrameAllocator, FramePool};
pub use window_options::WindowOptions;

use crate::{
	clay_renderer::clay_skia_render,
	focus_system::GLOBAL_FOCUS_MANAGER,
	font_manager::FontManager,
	input::Key,
	winit::{Callbacks, ImeFrameRequest, WinitApp},
};

/// Internal helpers used by the `rsml!` macro expansion.
///
/// These are intentionally small identity macros so the expanded code contains
/// a "real" Rust macro invocation around expressions/booleans, which can improve
/// tooling behavior in some editors.
#[doc(hidden)]
#[macro_export]
macro_rules! __rsml_expr {
	($e:expr) => { $e };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __rsml_bool {
	($b:expr) => { $b };
}

/// Custom render data used to extend Clay with effects that are not supported by
/// the core library (e.g. CSS-like `backdrop-filter` blur).
///
/// This is a struct (not an enum) so multiple effects/properties can be combined
/// on the same element (CSS-like "mix and match").
#[derive(Debug, Clone, Default)]
pub struct CustomElementData {
	/// CSS-like `backdrop-filter: blur(px)`.
	pub backdrop_blur: Option<f32>,
}

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
/// Creates and displays a Ardos UI window with a declarative root component.
///
/// This function sets up the entire environment required to render a graphical interface
/// using Ardos UI's component system. It manages the window lifecycle, rendering,
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
/// use ardos_ui::{create_window, WindowOptions, Text};
///
/// fn root_component(_: ()) -> Box<dyn ardos_ui::Element> {
///     Box::new(Text::new("Hello, Ardos UI!"))
/// }
///
/// fn main() {
///     create_window(
///         root_component,
///         (),
///         WindowOptions {
///             title: "My Ardos UI App".into(),
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
/// - Ardos UI must be properly compiled for the target operating system.
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

	let initial_size = if options.preferred_size != (0.0, 0.0) {
		options.preferred_size
	} else {
		(800.0, 600.0)
	};
	let clay = Rc::new(RefCell::new(clay_layout::Clay::new(
		(initial_size.0 as f32, initial_size.1 as f32).into(),
	)));
	let mut font_manager = FontManager::new();
	let input_manager = Rc::new(RefCell::new(WinitInputManager::new()));

	let winit_app = WinitApp::new(
		options,
		Callbacks {
			on_render_callback: {
				let clay = Rc::clone(&clay);
				let props = props.clone();
				let input_manager = Rc::clone(&input_manager);

				// `FramePool` lives for the lifetime of this callback and is reset every frame,
				// matching the `tibs` pattern.
				let mut frame_pool: FramePool<'static> = FramePool::new();

				Box::new(move |canvas| {

					let mut clay = clay.borrow_mut();

					// Reset frame pool at the start of the frame so allocations from the previous
					// frame are dropped only after they are guaranteed unused.
					frame_pool.reset();
					let frame_alloc = frame_pool.begin_alloc();

					{
						let input_manager_ref = input_manager.borrow();
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
					}
					font_manager.update_clay_measure_function(&mut clay);
					input_manager.borrow().reset_ime_request();
					let instant = Instant::now();
					let _input_scope = hooks::push_input_manager(Rc::clone(&input_manager));
					let root_component = Component::new(component, props.clone());

					let (commands, ime_request) = {
						let mut c = clay.begin();
						let input_manager_ref = input_manager.borrow();

						let mut render_ctx = RenderContext {
							c: &mut c,
							font_manager: &mut font_manager,
							input_manager: input_manager_ref.deref(),
							frame_alloc: &frame_alloc,
						};
						root_component.render(&mut render_ctx);

						let ime_requested = input_manager_ref.ime_requested();
						let ime_anchor = input_manager_ref.ime_anchor();
						let ime_cursor_area = ime_anchor
							.and_then(|anchor| c.bounding_box(c.id(&anchor)))
							.map(|bb| {
								(
									::winit::dpi::LogicalPosition::new(bb.x as f64, bb.y as f64).into(),
									::winit::dpi::LogicalSize::new(bb.width as f64, bb.height as f64)
										.into(),
								)
							});

						(
							c.end(),
							ImeFrameRequest {
								requested: ime_requested,
								cursor_area: ime_cursor_area,
							},
						)
					};

					let elapsed = instant.elapsed();
					let fonts = font_manager.get_fonts();
					clay_skia_render::<CustomElementData>(
						canvas,
						commands,
						|command, custom, canvas| {
							use clay_layout::render_commands::CornerRadii;
							use skia_safe::{ClipOp, Paint, Point, RRect, Rect};

							let Some(mut surface) = (unsafe { canvas.surface() }) else {
								return;
							};
							let snapshot = surface.image_snapshot();

							if let Some(sigma) = custom.data.backdrop_blur {
								let bb = command.bounding_box;
								let bounds = Rect::from_xywh(bb.x, bb.y, bb.width, bb.height);

								let CornerRadii {
									top_left,
									top_right,
									bottom_left,
									bottom_right,
								} = custom.corner_radii.clone();

								let rrect = RRect::new_rect_radii(
									bounds,
									&[
										Point::new(top_left, top_left),
										Point::new(top_right, top_right),
										Point::new(bottom_left, bottom_left),
										Point::new(bottom_right, bottom_right),
									],
								);

								canvas.save();
								canvas.clip_rrect(rrect, ClipOp::Intersect, true);

								let mut paint = Paint::default();
								paint.set_anti_alias(true);

								if let Some(filter) =
									skia_safe::image_filters::blur((sigma, sigma), None, None, None)
								{
									paint.set_image_filter(filter);
								}

								// CSS-like `backdrop-filter`: blur the already-rendered surface behind
								// the element, clipped to its rounded rect.
								canvas.draw_image(snapshot, (0.0, 0.0), Some(&paint));

								canvas.restore();
							}
						},
						&fonts,
					);

					input_manager.borrow_mut().update();
					ime_request
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
