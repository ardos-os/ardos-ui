use color_eyre::eyre::eyre;
use glutin::config::{ColorBufferType, Config, ConfigTemplateBuilder, GetGlConfig, GlConfig};
use glutin::context::{
	ContextApi, ContextAttributesBuilder, NotCurrentContext, PossiblyCurrentContext, Version,
};
use glutin::display::GetGlDisplay;
use glutin::prelude::{GlDisplay, NotCurrentGlContext, PossiblyCurrentGlContext};
use glutin::surface::{GlSurface, Surface, SwapInterval, WindowSurface};
use glutin_winit::DisplayBuilder;
use glutin_winit::GlWindow;
use skia_safe::gpu::direct_contexts::make_gl;
use skia_safe::gpu::ganesh::gl::backend_render_targets;
use skia_safe::gpu::gl::Format;
use skia_safe::gpu::{self, DirectContext};
use skia_safe::{Color, ColorType};
use std::num::NonZeroU32;
use std::rc::Rc;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize, Position, Size};
use winit::event::{
	ButtonSource, ElementState, Ime, KeyEvent, MouseScrollDelta, PointerSource, TouchPhase,
	WindowEvent,
};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::ModifiersState;
use winit::raw_window_handle::HasWindowHandle;
use winit::window::{
	ImeCapabilities, ImeEnableRequest, ImeHint, ImePurpose, ImeRequest, ImeRequestData,
	ImeRequestError, Window, WindowAttributes, WindowId,
};

use crate::REQUEST_REDRAW;
impl ApplicationHandler for WinitApp {
	fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
		let (window, gl_config) = match DisplayBuilder::new()
			.with_window_attributes(Some(self.window_options.clone()))
			.build(event_loop, self.template.clone(), gl_config_picker)
		{
			Ok((window, gl_config)) => (window.unwrap(), gl_config),
			Err(err) => {
				self.exit_state = Err(eyre!("{:#?}", err));
				event_loop.exit();
				return;
			}
		};
		log::trace!("Picked a config with {} samples", gl_config.num_samples());
		self.post_opengl_init(window, gl_config);
	}
	fn resumed(&mut self, event_loop: &dyn ActiveEventLoop) {
		log::trace!("Recreating window in `resumed`");
		if self.pending_window.is_some() {
			return;
		}
		if self.gl_context.is_none() {
			self.can_create_surfaces(event_loop);
			return;
		}

		// Pick the config which we already use for the context.
		let gl_config = self.gl_context.as_ref().unwrap().config();
		let window =
			match glutin_winit::finalize_window(event_loop, self.window_options.clone(), &gl_config) {
				Ok(window) => window,
				Err(err) => {
					self.exit_state = Err(err.into());
					event_loop.exit();
					return;
				}
			};

		self.post_opengl_init(window, gl_config);
	}

	fn suspended(&mut self, _event_loop: &dyn ActiveEventLoop) {
		log::trace!("Android window removed");
		self.pending_window = None;
		self.window = None;
		if self.gl_context.is_none() {
			return;
		}

		// Make context not current.
		self.gl_context = Some(
			self
				.gl_context
				.take()
				.unwrap()
				.make_not_current()
				.unwrap()
				.treat_as_possibly_current(),
		);
	}

	fn window_event(
		&mut self,
		event_loop: &dyn ActiveEventLoop,
		_window_id: WindowId,
		event: WindowEvent,
	) {
		match event {
			WindowEvent::Ime(ime) => {
				let Some(SurfaceAndWindow { window, .. }) = self.window.as_mut() else {
					return;
				};
				(self.callbacks.on_ime_event)(ime);
				window.request_redraw();
			}
			WindowEvent::KeyboardInput { event, .. } => {
				let Some(SurfaceAndWindow { window, .. }) = self.window.as_mut() else {
					return;
				};
				(self.callbacks.on_key_event)(event);
				window.request_redraw();
			}
			WindowEvent::ModifiersChanged(modifiers) => {
				(self.callbacks.on_modifiers_changed)(modifiers.state());
			}
			WindowEvent::SurfaceResized(size) if size.width != 0 && size.height != 0 => {
				if self.window.is_none() {
					if let Some((window, gl_config)) = self.pending_window.take() {
						self.post_opengl_init(window, gl_config);
					}
					return;
				}

				let Some(SurfaceAndWindow {
					gl_surface,
					window,
					mut skia_context,
					skia_surface: _,
				}) = self.window.take()
				else {
					return;
				};

				let gl_context = self.gl_context.take().unwrap();
				let skia_surface = self.make_skia_surface(
					&gl_surface,
					&gl_context.config(),
					&mut skia_context,
					size.width,
					size.height,
				);
				gl_surface.resize(
					&gl_context,
					NonZeroU32::new(size.width).unwrap(),
					NonZeroU32::new(size.height).unwrap(),
				);
				self.gl_context = gl_context.into();
				let size = size.to_logical(window.scale_factor());
				(self.callbacks.on_window_resize)(size.width, size.height);
				self.window = SurfaceAndWindow {
					gl_surface,
					skia_surface,
					skia_context,
					window,
				}
				.into();
			}
			WindowEvent::CloseRequested => event_loop.exit(),
			WindowEvent::RedrawRequested => {
				let Some(SurfaceAndWindow {
					skia_surface,
					skia_context,
					gl_surface,
					window,
				}) = self.window.as_mut()
				else {
					return;
				};
				window.request_redraw();

				skia_surface.canvas().clear(Color::TRANSPARENT);
				let canvas = skia_surface.canvas();
				let scale_factor = window.scale_factor() as f32;
				canvas.save();
				canvas.scale((scale_factor, scale_factor));
				let ime_request = (self.callbacks.on_render_callback)(canvas, window.as_ref());
				canvas.restore();
				update_window_ime(&mut self.ime_enabled, window.as_ref(), ime_request);
				skia_context.flush_and_submit();
				gl_surface
					.swap_buffers(self.gl_context.as_ref().unwrap())
					.unwrap();
			}
			WindowEvent::PointerMoved {
				device_id: _,
				position,
				primary: _,
				source,
			} => {
				let Some(SurfaceAndWindow { window, .. }) = self.window.as_mut() else {
					return;
				};
				let position = position.to_logical(window.scale_factor());
				(self.callbacks.on_pointer_move)(position.x, position.y, source);
				window.request_redraw();
			}
			WindowEvent::PointerButton {
				device_id: _,
				state,
				position,
				primary: _,
				button,
			} => {
				let Some(SurfaceAndWindow { window, .. }) = self.window.as_mut() else {
					return;
				};
				window.request_redraw();

				let position = position.to_logical(window.scale_factor());
				let pressed = matches!(state, ElementState::Pressed);
				(self.callbacks.on_pointer_button)(pressed, position.x, position.y, button);
			}
			WindowEvent::MouseWheel {
				device_id: _,
				delta,
				phase,
			} => {
				let Some(SurfaceAndWindow { window, .. }) = self.window.as_mut() else {
					return;
				};
				let (x, y) = match delta {
					MouseScrollDelta::LineDelta(x, y) => (-x * 40.0, -y * 40.0),
					MouseScrollDelta::PixelDelta(position) => {
						let position = position.to_logical::<f64>(window.scale_factor());
						(-position.x as f32, -position.y as f32)
					}
				};
				(self.callbacks.on_mouse_wheel)(x, y, phase);
				window.request_redraw();
			}
			_ => {
				let Some(SurfaceAndWindow { window, .. }) = self.window.as_mut() else {
					return;
				};
				window.request_redraw();
			}
		}
	}

	fn destroy_surfaces(&mut self, _event_loop: &dyn ActiveEventLoop) {
		self.pending_window = None;
		let Some(gl_context) = self.gl_context.take() else {
			self.window = None;
			return;
		};
		let _gl_display = gl_context.display();

		self.window = None;
		if let glutin::display::Display::Egl(display) = _gl_display {
			unsafe {
				display.terminate();
			}
		}
	}
}

fn create_gl_context(window: &dyn Window, gl_config: &Config) -> NotCurrentContext {
	let raw_window_handle = window.window_handle().ok().map(|wh| wh.as_raw());

	let context_attributes = ContextAttributesBuilder::new().build(raw_window_handle);

	let fallback_context_attributes = ContextAttributesBuilder::new()
		.with_context_api(ContextApi::Gles(None))
		.build(raw_window_handle);

	let legacy_context_attributes = ContextAttributesBuilder::new()
		.with_context_api(ContextApi::OpenGl(Some(Version::new(2, 1))))
		.build(raw_window_handle);

	let gl_display = gl_config.display();

	unsafe {
		gl_display
			.create_context(gl_config, &context_attributes)
			.unwrap_or_else(|_| {
				gl_display
					.create_context(gl_config, &fallback_context_attributes)
					.unwrap_or_else(|_| {
						gl_display
							.create_context(gl_config, &legacy_context_attributes)
							.expect("failed to create context")
					})
			})
	}
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ImeFrameRequest {
	pub requested: bool,
	pub cursor_area: Option<(Position, Size)>,
}

fn update_window_ime(ime_enabled: &mut bool, window: &dyn Window, request: ImeFrameRequest) {
	if *ime_enabled == request.requested && request.cursor_area.is_none() {
		return;
	}

	let ime_request = if request.requested {
		let capabilities = ImeCapabilities::new()
			.with_hint_and_purpose()
			.with_cursor_area();
		let request_data = ImeRequestData::default()
			.with_hint_and_purpose(ImeHint::NONE, ImePurpose::Normal)
			.with_cursor_area_opt(request.cursor_area);

		let Some(enable_request) = ImeEnableRequest::new(capabilities, request_data) else {
			return;
		};
		ImeRequest::Enable(enable_request)
	} else {
		ImeRequest::Disable
	};

	match window.request_ime_update(ime_request) {
		Ok(()) => *ime_enabled = request.requested,
		Err(ImeRequestError::AlreadyEnabled) => *ime_enabled = true,
		Err(ImeRequestError::NotEnabled) => *ime_enabled = false,
		Err(ImeRequestError::NotSupported) => {}
		Err(_) => {}
	}
}

trait ImeRequestDataExt {
	fn with_cursor_area_opt(self, cursor_area: Option<(Position, Size)>) -> Self;
}

impl ImeRequestDataExt for ImeRequestData {
	fn with_cursor_area_opt(self, cursor_area: Option<(Position, Size)>) -> Self {
		let (position, size) = cursor_area.unwrap_or_else(|| {
			(
				LogicalPosition::new(0.0, 0.0).into(),
				LogicalSize::new(0.0, 0.0).into(),
			)
		});
		self.with_cursor_area(position, size)
	}
}

pub(crate) struct Callbacks {
	pub on_render_callback: Box<dyn FnMut(&skia_safe::Canvas, &dyn Window) -> ImeFrameRequest>,
	pub on_pointer_move: Box<dyn FnMut(f64, f64, PointerSource)>,
	pub on_window_resize: Box<dyn FnMut(f64, f64)>,
	pub on_pointer_button: Box<dyn FnMut(bool, f64, f64, ButtonSource)>,
	pub on_mouse_wheel: Box<dyn FnMut(f32, f32, TouchPhase)>,
	pub on_key_event: Box<dyn FnMut(KeyEvent)>,
	pub on_modifiers_changed: Box<dyn FnMut(ModifiersState)>,
	pub on_ime_event: Box<dyn FnMut(Ime)>,
}
pub(crate) struct WinitApp {
	template: ConfigTemplateBuilder,
	gl_context: Option<PossiblyCurrentContext>,
	exit_state: color_eyre::Result<()>,
	window_options: WindowAttributes,
	pending_window: Option<(Box<dyn Window>, Config)>,
	window: Option<SurfaceAndWindow>,
	callbacks: Callbacks,
	ime_enabled: bool,
}

impl WinitApp {
	pub(crate) fn new(options: impl Into<WindowAttributes>, callbacks: Callbacks) -> Self {
		let options = options.into();
		Self {
			template: ConfigTemplateBuilder::new()
				.with_alpha_size(8)
				.with_transparency(true),
			window_options: options.clone(),
			exit_state: Ok(()),
			gl_context: None,
			pending_window: None,
			window: None,
			callbacks,
			ime_enabled: false,
		}
	}
	fn post_opengl_init(&mut self, window: Box<dyn Window>, gl_config: Config) {
		let size = window.surface_size();
		if size.width == 0 || size.height == 0 {
			self.pending_window = Some((window, gl_config));
			return;
		}

		// Create gl context.
		self.gl_context =
			Some(create_gl_context(window.as_ref(), &gl_config).treat_as_possibly_current());

		let attrs = window
			.build_surface_attributes(Default::default())
			.expect("Failed to build surface attributes");
		let gl_surface = unsafe {
			gl_config
				.display()
				.create_window_surface(&gl_config, &attrs)
				.unwrap()
		};

		// The context needs to be current for the Renderer to set up shaders and
		// buffers. It also performs function loading, which needs a current context on
		// WGL.
		let gl_context = self.gl_context.as_ref().unwrap();
		gl_context.make_current(&gl_surface).unwrap();
		// Try setting vsync.
		if let Err(res) =
			gl_surface.set_swap_interval(gl_context, SwapInterval::Wait(NonZeroU32::new(1).unwrap()))
		{
			log::error!("Error setting vsync: {res:?}");
		}
		let window: Rc<dyn Window> = window.into();
		REQUEST_REDRAW.set({
			let window = Rc::downgrade(&window);
			Box::new(move || {
				let Some(window) = window.upgrade() else {
					return;
				};
				window.request_redraw();
			})
		});
		let (skia_surface, skia_context) = self.initialize_skia(&gl_config, &gl_surface);
		self.window = Some(SurfaceAndWindow {
			gl_surface,
			window,
			skia_surface,
			skia_context,
		});
	}
	pub(crate) fn initialize_skia(
		&mut self,
		gl_config: &Config,
		gl_surface: &Surface<WindowSurface>,
	) -> (skia_safe::Surface, skia_safe::gpu::DirectContext) {
		// Interface GL automática (sem crate gl)
		let interface = gpu::gl::Interface::new_load_with_cstr(|name| {
			if name == c"eglGetCurrentDisplay" {
				return std::ptr::null();
			}
			gl_surface.display().get_proc_address(name)
		})
		.expect("Failed to create Skia GL interface");

		// Contexto GPU ligado ao OpenGL ativo
		let mut gr_context = make_gl(interface, None).expect("Failed to create Skia DirectContext");

		return (
			self.make_skia_surface(gl_surface, gl_config, &mut gr_context, 0, 0),
			gr_context,
		);
	}
	fn make_skia_surface(
		&self,
		gl_surface: &Surface<WindowSurface>,
		gl_config: &Config,
		gr_context: &mut DirectContext,
		width: u32,
		height: u32,
	) -> skia_safe::Surface {
		// Pega tamanho da janela
		let width = if width != 0 {
			width
		} else {
			gl_surface.width().unwrap()
		};
		let height = if height != 0 {
			height
		} else {
			gl_surface.height().unwrap()
		};
		type GlGetIntegerv = unsafe extern "system" fn(pname: u32, data: *mut i32);
		const GL_FRAMEBUFFER_BINDING: u32 = 0x8CA6;
		let gl_get_integerv: GlGetIntegerv =
			unsafe { std::mem::transmute(gl_surface.display().get_proc_address(c"glGetIntegerv")) };
		let mut fboid: i32 = 0;
		unsafe {
			gl_get_integerv(GL_FRAMEBUFFER_BINDING, &mut fboid);
		}
		let (format, color_type) =
			color_buffer_to_skia(gl_config.color_buffer_type().expect("fuck you"));
		let fb_info = gpu::gl::FramebufferInfo {
			fboid: fboid as _, // default framebuffer
			format: format.into(),
			protected: gpu::Protected::No,
		};

		let num_samples = gl_config.num_samples() as usize;
		let stencil_size = gl_config.stencil_size() as usize;
		let backend_render_target = backend_render_targets::make_gl(
			(width as _, height as _),
			num_samples,  // samples
			stencil_size, // stencil bits
			fb_info,
		);

		gpu::surfaces::wrap_backend_render_target(
			&mut *gr_context,
			&backend_render_target,
			gpu::SurfaceOrigin::BottomLeft,
			color_type,
			None,
			None,
		)
		.expect("Failed to create Skia surface")
	}
	pub(crate) fn run(self) {
		let event_loop = EventLoop::new().unwrap();
		event_loop.set_control_flow(ControlFlow::Wait);
		event_loop.run_app(self).unwrap();
	}

	#[cfg(target_os = "android")]
	pub(crate) fn run_android(self, app: winit::platform::android::activity::AndroidApp) {
		use winit::error::EventLoopError;
		use winit::platform::android::EventLoopBuilderExtAndroid;

		let event_loop = match EventLoop::builder().with_android_app(app).build() {
			Ok(event_loop) => event_loop,
			Err(EventLoopError::RecreationAttempt) => return,
			Err(err) => panic!("{err}"),
		};
		event_loop.set_control_flow(ControlFlow::Wait);
		event_loop.run_app(self).unwrap();
	}
}

struct SurfaceAndWindow {
	skia_surface: skia_safe::Surface,
	skia_context: skia_safe::gpu::DirectContext,
	gl_surface: Surface<WindowSurface>,
	// NOTE: Window should be dropped after all resources created using its
	// raw-window-handle.
	window: Rc<dyn Window>,
}

fn gl_config_picker(configs: Box<dyn Iterator<Item = Config> + '_>) -> Config {
	configs
		.reduce(|accum, config| {
			let transparency_check = config.supports_transparency().unwrap_or(false)
				& !accum.supports_transparency().unwrap_or(false);

			if transparency_check || config.num_samples() < accum.num_samples() {
				config
			} else {
				accum
			}
		})
		.unwrap()
}

fn color_buffer_to_skia(color_buffer: ColorBufferType) -> (Format, ColorType) {
	match color_buffer {
		ColorBufferType::Rgb {
			r_size,
			g_size,
			b_size,
		} => match (r_size, g_size, b_size) {
			(8, 8, 8) => (Format::RGBA8, ColorType::RGBA8888),
			(10, 10, 10) => (Format::RGB10_A2, ColorType::RGBA1010102),
			(5, 6, 5) => (Format::RGB565, ColorType::RGB565),
			_ => (Format::Unknown, ColorType::Unknown),
		},
		ColorBufferType::Luminance(size) => match size {
			8 => (Format::R8, ColorType::Gray8),
			_ => (Format::Unknown, ColorType::Unknown),
		},
	}
}
