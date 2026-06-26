use std::{cell::RefCell, rc::Rc, time::Instant};

mod clay_renderer;
mod clipboard;
mod element;
mod focus_system;
mod font_manager;
mod hooks;
mod image;
mod input;
mod render_context;
mod util;
mod window_options;
mod winit;
#[cfg(target_os = "android")]
pub use ::winit::platform::android::activity::AndroidApp;
pub use ardos_ui_rsml_compiler::rsml;
pub use clipboard::{Clipboard, ClipboardHandle, use_clipboard};
pub use element::{
	Element, ElementExt, component::Component, container::*, image::Image, input::*, text::Text, text::TextOverflowMode, text::TextAlignment
};
pub use hooks::*;
pub use image::{
	AssetImage, FileImage, ImageError, ImageHandle, ImageKey, ImageProviderBuilder,
	ImageProviderContext, ImageProviderInstance, ImageProviderPollContext, ImageProviderState,
	MemoryImage, SvgImage, NetworkImage
};
pub(crate) use input::winit_impl::WinitInputManager;
pub use input::{InputManager, NamedKey, NativeKey};
use render_context::InteractionState;
pub use render_context::RenderContext;
pub use util::frame_pool::{FrameAllocator, FramePool};
pub use window_options::WindowOptions;

#[cfg(all(unix, not(target_os = "android")))]
use crate::clipboard::WaylandClipboard;
use crate::{
	clay_renderer::{rlay_skia_render, rlay_to_skia_rrect},
	focus_system::GLOBAL_FOCUS_MANAGER,
	font_manager::FontManager,
	input::Key,
	winit::{Callbacks, ImeFrameRequest, WinitApp},
};
use ::winit::event::{ButtonSource, MouseButton, PointerSource, TouchPhase};
#[cfg(all(unix, not(target_os = "android")))]
use ::winit::raw_window_handle::{HasDisplayHandle, RawDisplayHandle};
use rlay::{Engine, LayoutResult, Point, Size};

/// Internal helpers used by the `rsml!` macro expansion.
///
/// These are intentionally small identity macros so the expanded code contains
/// a "real" Rust macro invocation around expressions/booleans, which can improve
/// tooling behavior in some editors.
#[doc(hidden)]
#[macro_export]
macro_rules! __rsml_expr {
	($e:expr) => {
		$e
	};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __rsml_bool {
	($b:expr) => {
		$b
	};
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
pub fn create_window<Props: Default + Clone + 'static>(
	component: impl Clone + Copy + Fn(Props) -> Box<dyn Element> + 'static,
	options: WindowOptions,
) {
	build_window(component, options).run();
}

#[cfg(target_os = "android")]
pub fn create_window_android<Props: Default + Clone + 'static>(
	component: impl Clone + Copy + Fn(Props) -> Box<dyn Element> + 'static,
	app: ::winit::platform::android::activity::AndroidApp,
	options: WindowOptions,
) {
	build_window(component, options).run_android(app);
}

fn build_window<Props: Default + Clone + 'static>(
	component: impl Clone + Copy + Fn(Props) -> Box<dyn Element> + 'static,
	options: WindowOptions,
) -> WinitApp {
	color_eyre::install().ok();

	let initial_size = if options.preferred_size != (0.0, 0.0) {
		options.preferred_size
	} else {
		(800.0, 600.0)
	};
	let mut font_manager = FontManager::new();
	let mut image_manager = image::ImageManager::default();
	let measure_fonts = font_manager.measure_handle();
	let rlay = Rc::new(RefCell::new(Engine::new(move |text, style| {
		font_manager::measure_text(&measure_fonts, text, style)
	})));
	let layout_size = Rc::new(RefCell::new(Size::new(
		initial_size.0 as f32,
		initial_size.1 as f32,
	)));
	let previous_layout = Rc::new(RefCell::new(LayoutResult::default()));
	let input_manager = Rc::new(RefCell::new(WinitInputManager::new()));
	let clipboard: Rc<RefCell<Option<ClipboardHandle>>> = Rc::new(RefCell::new(None));
	let props = Props::default();

	let winit_app = WinitApp::new(
		options,
		Callbacks {
			on_render_callback: {
				let rlay = Rc::clone(&rlay);
				let layout_size = Rc::clone(&layout_size);
				let previous_layout = Rc::clone(&previous_layout);
				let input_manager = Rc::clone(&input_manager);
				let clipboard = Rc::clone(&clipboard);

				// `FramePool` lives for the lifetime of this callback and is reset every frame,
				// matching the `tibs` pattern.
				let mut frame_pool: FramePool<'static> = FramePool::new();
				let mut previous_frame = None;

				Box::new(move |canvas, window| {
					let now = Instant::now();
					let delta_time = previous_frame
						.replace(now)
						.map_or(0.0, |previous| (now - previous).as_secs_f32());
					let clipboard = clipboard_for_window(&clipboard, window);

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
					input_manager.borrow().reset_ime_request();
					let _input_scope = hooks::push_input_manager(Rc::clone(&input_manager));
					let _clipboard_scope = clipboard.map(clipboard::push_clipboard);
					let root_component = Component::new_with_props(component, props.clone());

					let (layout, custom_elements, ime_request) = {
						let mut rlay = rlay.borrow_mut();
						let input_manager_ref = input_manager.borrow();
						let previous_layout_ref = previous_layout.borrow();
						rlay.apply_input_scroll(&previous_layout_ref);
						let interaction = InteractionState {
							pointers: previous_layout_ref.pointer_hits(rlay.input()),
							enter_pressed: input_manager_ref.is_key_just_pressed(Key::Named(NamedKey::Enter)),
							enter_down: input_manager_ref.is_key_pressed(Key::Named(NamedKey::Enter)),
							context_menu_pressed: input_manager_ref
								.is_key_just_pressed(Key::Named(NamedKey::ContextMenu)),
							context_menu_down: input_manager_ref
								.is_key_pressed(Key::Named(NamedKey::ContextMenu)),
						};
						let mut frame = rlay.begin(*layout_size.borrow());
						let mut custom_elements = Vec::new();

						let mut render_ctx = RenderContext {
							frame: &mut frame,
							previous_layout: &previous_layout_ref,
							interaction: &interaction,
							font_manager: &mut font_manager,
							image_manager: &mut image_manager,
							custom_elements: &mut custom_elements,
							frame_alloc: &frame_alloc,
						};
						root_component.render(&mut render_ctx);

						let ime_requested = input_manager_ref.ime_requested();
						let ime_anchor = input_manager_ref.ime_anchor();
						drop(render_ctx);
						drop(previous_layout_ref);

						let layout = frame.end(delta_time).unwrap_or_default();
						let ime_cursor_area = ime_anchor.and_then(|anchor| {
							layout.element(&anchor).map(|element| {
								let bb = element.bounds;
								(
									::winit::dpi::LogicalPosition::new(bb.x as f64, bb.y as f64).into(),
									::winit::dpi::LogicalSize::new(bb.width as f64, bb.height as f64).into(),
								)
							})
						});
						if layout.needs_animation_frame {
							REQUEST_REDRAW.call();
						}

						(
							layout,
							custom_elements,
							ImeFrameRequest {
								requested: ime_requested,
								cursor_area: ime_cursor_area,
							},
						)
					};

					let fonts = font_manager.get_fonts();
					let fonts = fonts.borrow();
					rlay_skia_render(
						canvas,
						layout.commands.iter().cloned(),
						|command, custom_id, radius, canvas| {
							use skia_safe::{ClipOp, Paint};

							let Some(mut surface) = (unsafe { canvas.surface() }) else {
								return;
							};
							let snapshot = surface.image_snapshot();

							let Some(custom) = custom_elements.get(custom_id as usize) else {
								return;
							};

							if let Some(sigma) = custom.backdrop_blur {
								let bb = command.bounds;
								let rrect = rlay_to_skia_rrect(bb, radius);

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
						&image_manager,
					);

					input_manager.borrow_mut().update();
					*previous_layout.borrow_mut() = layout;
					ime_request
				})
			},
			on_pointer_move: {
				let rlay = Rc::clone(&rlay);
				let input_manager = Rc::clone(&input_manager);
				Box::new(move |x, y, source| {
					let mut rlay = rlay.borrow_mut();
					match source {
						PointerSource::Mouse => {
							input_manager
								.borrow_mut()
								.set_mouse_position(x as f32, y as f32);
							rlay
								.input_mut()
								.set_mouse_position(Point::new(x as _, y as _));
						}
						PointerSource::Touch { finger_id, .. } => {
							rlay.input_mut().set_touch(
								finger_id.into_raw() as _,
								Point::new(x as _, y as _),
								true,
							);
						}
						_ => {}
					}
				})
			},

			on_pointer_button: {
				let rlay = Rc::clone(&rlay);
				let input_manager = Rc::clone(&input_manager);
				Box::new(move |pressed, x, y, source| {
					let mut rlay = rlay.borrow_mut();
					match source {
						ButtonSource::Mouse(MouseButton::Left) => {
							rlay.input_mut().set_mouse_button(
								Point::new(x as _, y as _),
								rlay::MouseButton::Left,
								pressed,
							);
							input_manager
								.borrow_mut()
								.set_mouse_position(x as f32, y as f32);
							input_manager.borrow_mut().set_mouse_button(0, pressed);
						}
						ButtonSource::Mouse(b) => {
							let button = match b {
								MouseButton::Left => rlay::MouseButton::Left,
								MouseButton::Right => rlay::MouseButton::Right,
								MouseButton::Middle => rlay::MouseButton::Middle,
								MouseButton::Back => rlay::MouseButton::Other(3),
								MouseButton::Forward => rlay::MouseButton::Other(4),
								other => rlay::MouseButton::Other(other as u16),
							};
							rlay
								.input_mut()
								.set_mouse_button(Point::new(x as _, y as _), button, pressed);
							input_manager
								.borrow_mut()
								.set_mouse_position(x as f32, y as f32);
							input_manager.borrow_mut().set_mouse_button(b as _, pressed);
						}
						ButtonSource::Touch { finger_id, .. } => {
							rlay.input_mut().set_touch(
								finger_id.into_raw() as _,
								Point::new(x as _, y as _),
								pressed,
							);
						}
						_ => {}
					}
				})
			},

			on_mouse_wheel: {
				let rlay = Rc::clone(&rlay);
				let input_manager = Rc::clone(&input_manager);
				Box::new(move |x, y, phase| {
					let (mx, my) = input_manager.borrow().mouse_position();
					let mut rlay = rlay.borrow_mut();
					rlay.input_mut().set_mouse_position(Point::new(mx, my));
					let phase = match phase {
						TouchPhase::Started => rlay::TouchPhase::Started,
						TouchPhase::Moved => rlay::TouchPhase::Moved,
						TouchPhase::Ended => rlay::TouchPhase::Ended,
						TouchPhase::Cancelled => rlay::TouchPhase::Cancelled,
					};
					rlay.input_mut().add_scroll_delta_with_phase(
						rlay::PointerId::Mouse,
						rlay::Vector::new(x, y),
						Some(phase),
					);
				})
			},

			on_key_event: {
				let input_manager = Rc::clone(&input_manager);
				Box::new(move |event| {
					input_manager.borrow_mut().handle_key_event(event);
				})
			},
			on_modifiers_changed: {
				let input_manager = Rc::clone(&input_manager);
				Box::new(move |modifiers| {
					input_manager.borrow_mut().set_modifiers(modifiers);
				})
			},
			on_ime_event: {
				let input_manager = Rc::clone(&input_manager);
				Box::new(move |ime| {
					input_manager.borrow_mut().handle_ime_event(ime);
				})
			},
			on_window_resize: Box::new(move |w, h| {
				layout_size.replace(rlay::Size::new(w as _, h as _));
			}),
		},
	);

	winit_app
}

#[cfg(all(unix, not(target_os = "android")))]
fn clipboard_for_window(
	clipboard: &Rc<RefCell<Option<ClipboardHandle>>>,
	window: &dyn ::winit::window::Window,
) -> Option<ClipboardHandle> {
	if let Some(clipboard) = clipboard.borrow().as_ref().cloned() {
		return Some(clipboard);
	}

	let display = match window.display_handle().ok()?.as_raw() {
		RawDisplayHandle::Wayland(handle) => handle.display.as_ptr(),
		_ => return None,
	};

	// SAFETY: the pointer comes from the live winit Wayland window/display and the
	// clipboard is dropped with the UI state before the window backend is torn down.
	let next = Rc::new(unsafe { WaylandClipboard::new(display) }) as ClipboardHandle;
	*clipboard.borrow_mut() = Some(next.clone());
	Some(next)
}

#[cfg(not(all(unix, not(target_os = "android"))))]
fn clipboard_for_window(
	_clipboard: &Rc<RefCell<Option<ClipboardHandle>>>,
	_window: &dyn ::winit::window::Window,
) -> Option<ClipboardHandle> {
	None
}
