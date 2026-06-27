use std::{
	cell::{Cell, RefCell},
	collections::HashMap,
	rc::Rc,
	time::Instant,
};

use anyhow::{Context as _, anyhow};
use rlay::{Engine, LayoutResult, Point, Size};
use skia_safe::{
	Color, ColorType,
	gpu::{self, DirectContext, direct_contexts::make_gl, ganesh::gl::backend_render_targets},
};
use tab_app_framework::{
	AxisOrientation, AxisPhase, AxisSource, CharEvent, Config, GestureEvent, GlApplication,
	GlEventContext, GlInitContext, GlTabAppFramework, KeyEvent, Monitor, MonitorRemovedEvent,
	MouseDownEvent, MouseMoveEvent, MouseUpEvent, PointerAxisEvent, PointerMoveEvent, PointerType,
	RenderEvent, RenderMode, SessionEvent, TouchEvent,
};

use crate::{
	Component, Element, GlobalClosure, InputManager, Key, NamedKey, PointerKind, REQUEST_REDRAW,
	RenderContext, ShiftInputManager,
	clay_renderer::{rlay_skia_render, rlay_to_skia_rrect},
	focus_system::GLOBAL_FOCUS_MANAGER,
	font_manager::{self, FontManager},
	hooks, image,
	render_context::InteractionState,
	util::frame_pool::FramePool,
};

pub type ShiftEventContext<'c, 'g> = GlEventContext<'c, 'g, ShiftApp>;

type RootFactory = Rc<dyn for<'a, 'c, 'g> Fn(ShiftRootProps<'a, 'c, 'g>) -> Box<dyn Element>>;

pub struct ShiftRootProps<'a, 'c, 'g> {
	pub monitor: &'a Monitor,
	pub shift: &'a mut ShiftEventContext<'c, 'g>,
}

impl Default for ShiftRootProps<'_, '_, '_> {
	fn default() -> Self {
		panic!("ShiftRootProps can only be created by the Shift backend")
	}
}

thread_local! {
	static SHIFT_ROOT_FACTORY: RefCell<Option<RootFactory>> = RefCell::new(None);
}

pub fn create_window_shift(
	component: impl Clone
	+ Copy
	+ for<'a, 'c, 'g> Fn(ShiftRootProps<'a, 'c, 'g>) -> Box<dyn Element>
	+ 'static,
) -> anyhow::Result<()> {
	let root_factory: RootFactory =
		Rc::new(move |props| Box::new(Component::new_with_props(component, props)) as Box<dyn Element>);

	SHIFT_ROOT_FACTORY.with(|factory| {
		*factory.borrow_mut() = Some(root_factory);
	});

	let mut app = GlTabAppFramework::<ShiftApp>::init(|config: &mut Config| {
		config.opengl_es_version(3, 0);
		config.set_render_mode(RenderMode::Eager);
	})?;
	app.run()?;
	Ok(())
}

pub struct ShiftApp {
	root_factory: RootFactory,
	font_manager: FontManager,
	image_manager: image::ImageManager,
	monitor_states: HashMap<String, MonitorUiState>,
	touch_contacts: HashMap<i32, (String, Point)>,
	skia_surface_cache: SkiaSurfaceCache,
	skia_context: DirectContext,
	redraw_requested: Rc<Cell<bool>>,
	measure_fonts: Rc<RefCell<Vec<skia_safe::Typeface>>>,
}

struct SkiaSurfaceCache {
	surfaces_by_fbo: HashMap<u32, skia_safe::Surface>,
	width: i32,
	height: i32,
	target_fbo: u32,
}

struct MonitorUiState {
	rlay: Engine,
	previous_layout: LayoutResult,
	input_manager: Rc<RefCell<ShiftInputManager>>,
	frame_pool: FramePool<'static>,
	previous_frame: Option<Instant>,
	scroll_active: bool,
}

impl GlApplication for ShiftApp {
	fn init(ctx: &mut GlInitContext) -> anyhow::Result<Self> {
		let root_factory = SHIFT_ROOT_FACTORY
			.with(|factory| factory.borrow_mut().take())
			.ok_or_else(|| anyhow!("create_window_shift must initialize the root component"))?;

		let font_manager = FontManager::new();
		let measure_fonts = font_manager.measure_handle();
		let redraw_requested = Rc::new(Cell::new(false));

		REQUEST_REDRAW.set({
			let redraw_requested = Rc::clone(&redraw_requested);
			Box::new(move || {
				redraw_requested.set(true);
			})
		});

		let interface = gpu::gl::Interface::new_load_with_cstr(|name| {
			let Ok(name) = name.to_str() else {
				return std::ptr::null();
			};
			ctx.gl().load_proc(name).unwrap_or(std::ptr::null()).cast()
		})
		.context("failed to create Skia GL interface")?;
		let skia_context = make_gl(interface, None).context("failed to create Skia DirectContext")?;

		Ok(Self {
			root_factory,
			font_manager,
			image_manager: image::ImageManager::default(),
			monitor_states: HashMap::new(),
			touch_contacts: HashMap::new(),
			skia_surface_cache: SkiaSurfaceCache::default(),
			skia_context,
			redraw_requested,
			measure_fonts,
		})
	}

	fn on_render(&mut self, ctx: &mut GlEventContext<'_, '_, Self>, ev: RenderEvent) {
		if ev.width <= 0 || ev.height <= 0 {
			return;
		}

		let Some(monitor) = ctx.monitor(&ev.monitor_id).cloned() else {
			return;
		};
		let cursor = monitor.cursor_relative_position(ctx.cursor_position());
		let layout_size = Size::new(monitor.width as f32, monitor.height as f32);
		let monitor_id = ev.monitor_id.clone();
		let mut state = self.take_monitor_state(&monitor_id);
		state
			.input_manager
			.borrow_mut()
			.set_mouse_position(cursor.0 as f32, cursor.1 as f32);

		// The Shift GL framework binds the imported DMA-BUF framebuffer before
		// calling us. Tell Skia that external GL state changed so cached Ganesh
		// state, including glyph atlas bindings, is not reused against stale state.
		self.skia_context.reset(None);

		let target_fbo = ctx.current_fbo() as u32;
		let surface = self.skia_surface_cache.ensure_surface_target(
			&mut self.skia_context,
			ev.width,
			ev.height,
			target_fbo,
		);
		surface.canvas().clear(Color::TRANSPARENT);
		let canvas = surface.canvas();
		canvas.save();
		canvas.scale((monitor.scale as f32, monitor.scale as f32));

		let layout = render_monitor_frame(
			&self.root_factory,
			&mut self.font_manager,
			&mut self.image_manager,
			&mut state,
			ctx,
			canvas,
			layout_size,
			&monitor,
		);

		canvas.restore();
		self.skia_context.flush(None);

		let needs_animation_frame = layout.needs_animation_frame;
		state.previous_layout = layout;
		self.monitor_states.insert(monitor_id, state);

		if self.redraw_requested.replace(false) || needs_animation_frame {
			ctx.schedule_frame(ev.monitor_id);
		}
	}

	fn on_monitor_added(
		&mut self,
		ctx: &mut GlEventContext<'_, '_, Self>,
		_ev: tab_app_framework::MonitorAddedEvent,
	) {
		let state = self.new_monitor_state();
		self.monitor_states.insert(_ev.monitor.id.clone(), state);
		ctx.schedule_all_frames();
	}

	fn on_session_state(&mut self, ctx: &mut GlEventContext<'_, '_, Self>, _ev: SessionEvent) {
		ctx.schedule_all_frames();
	}

	fn on_monitor_removed(
		&mut self,
		_ctx: &mut GlEventContext<'_, '_, Self>,
		ev: MonitorRemovedEvent,
	) {
		self.monitor_states.remove(&ev.monitor_id);
	}

	fn on_mouse_move(&mut self, ctx: &mut GlEventContext<'_, '_, Self>, ev: MouseMoveEvent) {
		self.pointer_move(ctx, ev.new_position, PointerType::Mouse);
	}

	fn on_pointer_move(&mut self, ctx: &mut GlEventContext<'_, '_, Self>, ev: PointerMoveEvent) {
		self.pointer_move(ctx, ev.new_position, ev.pointer_type);
	}

	fn on_mouse_down(&mut self, ctx: &mut GlEventContext<'_, '_, Self>, ev: MouseDownEvent) {
		self.mouse_button(ctx, ev.position, ev.button, true);
	}

	fn on_mouse_up(&mut self, ctx: &mut GlEventContext<'_, '_, Self>, ev: MouseUpEvent) {
		self.mouse_button(ctx, ev.position, ev.button, false);
	}

	fn on_pointer_axis(&mut self, ctx: &mut GlEventContext<'_, '_, Self>, ev: PointerAxisEvent) {
		let Some((monitor_id, x, y)) = monitor_at(ctx, ev.position) else {
			return;
		};

		let state = self.monitor_state(&monitor_id);
		state
			.input_manager
			.borrow_mut()
			.set_mouse_position(x as f32, y as f32);
		state
			.input_manager
			.borrow_mut()
			.set_pointer_kind(PointerKind::Mouse);
		state
			.rlay
			.input_mut()
			.set_mouse_position(Point::new(x as f32, y as f32));

		let delta = match ev.orientation {
			AxisOrientation::Horizontal => rlay::Vector::new(ev.delta as f32, 0.0),
			AxisOrientation::Vertical => rlay::Vector::new(0.0, ev.delta as f32),
		};
		let phase =
			matches!(ev.source, AxisSource::Finger | AxisSource::Continuous).then(|| match ev.phase {
				AxisPhase::Started => rlay::TouchPhase::Started,
				AxisPhase::Moved => rlay::TouchPhase::Moved,
				AxisPhase::Ended => rlay::TouchPhase::Ended,
				AxisPhase::Cancelled => rlay::TouchPhase::Cancelled,
			});
		state
			.rlay
			.input_mut()
			.add_scroll_delta_with_phase(rlay::PointerId::Mouse, delta, phase);
		ctx.schedule_all_frames();
	}

	fn on_key(&mut self, ctx: &mut GlEventContext<'_, '_, Self>, ev: KeyEvent) {
		for state in self.monitor_states.values_mut() {
			state
				.input_manager
				.borrow_mut()
				.handle_key(ev.key, ev.is_pressed());
		}
		ctx.schedule_all_frames();
	}

	fn on_char(&mut self, ctx: &mut GlEventContext<'_, '_, Self>, ev: CharEvent) {
		for state in self.monitor_states.values_mut() {
			state.input_manager.borrow_mut().handle_text(&ev.text);
		}
		ctx.schedule_all_frames();
	}

	fn on_touch(&mut self, ctx: &mut GlEventContext<'_, '_, Self>, ev: TouchEvent) {
		match ev {
			TouchEvent::Down { contact, .. } | TouchEvent::Motion { contact, .. } => {
				if contact.id < 0 {
					ctx.schedule_all_frames();
					return;
				}
				if let Some((monitor_id, x, y)) =
					transformed_touch_position(ctx, contact.x_transformed, contact.y_transformed)
				{
					let point = Point::new(x as f32, y as f32);
					let state = self.monitor_state(&monitor_id);
					state
						.input_manager
						.borrow_mut()
						.set_touch_point(contact.id as u64, point.x, point.y);
					state
						.rlay
						.input_mut()
						.set_touch(contact.id as u64, point, true);
					self.touch_contacts.insert(contact.id, (monitor_id, point));
				}
			}
			TouchEvent::Up { contact_id, .. } => {
				if contact_id < 0 {
					ctx.schedule_all_frames();
					return;
				}
				if let Some((monitor_id, point)) = self.touch_contacts.remove(&contact_id) {
					let state = self.monitor_state(&monitor_id);
					state
						.input_manager
						.borrow_mut()
						.remove_touch_point(contact_id as u64);
					state
						.rlay
						.input_mut()
						.set_touch(contact_id as u64, point, false);
				}
			}
			TouchEvent::Cancel { .. } => {
				let contacts = self.touch_contacts.drain().collect::<Vec<_>>();
				for (contact_id, (monitor_id, point)) in contacts {
					let state = self.monitor_state(&monitor_id);
					state.input_manager.borrow_mut().clear_touch_points();
					state
						.rlay
						.input_mut()
						.set_touch(contact_id as u64, point, false);
				}
			}
			TouchEvent::Frame { .. } => {}
		}
		ctx.schedule_all_frames();
	}

	fn on_gesture(&mut self, ctx: &mut GlEventContext<'_, '_, Self>, ev: GestureEvent) {
		match ev {
			GestureEvent::SwipeBegin { .. } => {
				if let Some((monitor_id, _, _)) = monitor_at(ctx, ctx.cursor_position()) {
					self.monitor_state(&monitor_id).scroll_active = true;
				}
			}
			GestureEvent::SwipeUpdate { dx, dy, .. } => {
				if let Some((monitor_id, _, _)) = monitor_at(ctx, ctx.cursor_position()) {
					let state = self.monitor_state(&monitor_id);
					if state.scroll_active {
						let (mx, my) = state.input_manager.borrow().mouse_position();
						state
							.rlay
							.input_mut()
							.set_mouse_position(Point::new(mx, my));
						state.rlay.input_mut().add_scroll_delta_with_phase(
							rlay::PointerId::Mouse,
							rlay::Vector::new(dx as f32, dy as f32),
							Some(rlay::TouchPhase::Moved),
						);
					}
				}
			}
			GestureEvent::SwipeEnd { .. } => {
				for state in self.monitor_states.values_mut() {
					if state.scroll_active {
						state.scroll_active = false;
						state.rlay.input_mut().add_scroll_delta_with_phase(
							rlay::PointerId::Mouse,
							rlay::Vector::new(0.0, 0.0),
							Some(rlay::TouchPhase::Ended),
						);
					}
				}
			}
			_ => {}
		}
		ctx.schedule_all_frames();
	}
}

impl ShiftApp {
	fn new_monitor_state(&self) -> MonitorUiState {
		let measure_fonts = Rc::clone(&self.measure_fonts);
		MonitorUiState {
			rlay: Engine::new(move |text, style| font_manager::measure_text(&measure_fonts, text, style)),
			previous_layout: LayoutResult::default(),
			input_manager: Rc::new(RefCell::new(ShiftInputManager::new())),
			frame_pool: FramePool::new(),
			previous_frame: None,
			scroll_active: false,
		}
	}

	fn take_monitor_state(&mut self, monitor_id: &str) -> MonitorUiState {
		self
			.monitor_states
			.remove(monitor_id)
			.unwrap_or_else(|| self.new_monitor_state())
	}

	fn monitor_state(&mut self, monitor_id: &str) -> &mut MonitorUiState {
		if !self.monitor_states.contains_key(monitor_id) {
			let state = self.new_monitor_state();
			self.monitor_states.insert(monitor_id.to_string(), state);
		}
		self.monitor_states.get_mut(monitor_id).unwrap()
	}

	fn pointer_move(
		&mut self,
		ctx: &mut GlEventContext<'_, '_, Self>,
		position: (f64, f64),
		pointer_type: PointerType,
	) {
		let Some((monitor_id, x, y)) = monitor_at(ctx, position) else {
			return;
		};
		match pointer_type {
			PointerType::Mouse | PointerType::Pen | PointerType::Unknown => {
				let state = self.monitor_state(&monitor_id);
				state
					.input_manager
					.borrow_mut()
					.set_pointer_kind(shift_pointer_kind(pointer_type));
				state
					.input_manager
					.borrow_mut()
					.set_mouse_position(x as f32, y as f32);
				state
					.rlay
					.input_mut()
					.set_mouse_position(Point::new(x as f32, y as f32));
			}
			PointerType::Touch => {}
		}
		ctx.schedule_all_frames();
	}

	fn mouse_button(
		&mut self,
		ctx: &mut GlEventContext<'_, '_, Self>,
		position: (f64, f64),
		button: u32,
		pressed: bool,
	) {
		let Some((monitor_id, x, y)) = monitor_at(ctx, position) else {
			return;
		};
		let ui_button = shift_mouse_button(button);
		let state = self.monitor_state(&monitor_id);

		state
			.input_manager
			.borrow_mut()
			.set_pointer_kind(PointerKind::Mouse);
		state
			.input_manager
			.borrow_mut()
			.set_mouse_position(x as f32, y as f32);
		state
			.input_manager
			.borrow_mut()
			.set_mouse_button(mouse_button_index(button), pressed);
		state
			.rlay
			.input_mut()
			.set_mouse_button(Point::new(x as f32, y as f32), ui_button, pressed);
		ctx.schedule_all_frames();
	}
}

fn render_monitor_frame(
	root_factory: &RootFactory,
	font_manager: &mut FontManager,
	image_manager: &mut image::ImageManager,
	state: &mut MonitorUiState,
	shift: &mut ShiftEventContext<'_, '_>,
	canvas: &skia_safe::Canvas,
	layout_size: Size,
	monitor: &Monitor,
) -> LayoutResult {
	let now = Instant::now();
	let delta_time = state
		.previous_frame
		.replace(now)
		.map_or(0.0, |previous| (now - previous).as_secs_f32());

	state.frame_pool.reset();
	let frame_alloc = state.frame_pool.begin_alloc();

	{
		let input_manager_ref = state.input_manager.borrow();
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

	let _input_scope = hooks::push_input_manager(Rc::clone(&state.input_manager) as _);
	let root_component = (root_factory)(ShiftRootProps { monitor, shift });
	let mut custom_elements = Vec::new();

	let layout = {
		state.rlay.apply_input_scroll(&state.previous_layout);
		let input_manager_ref = state.input_manager.borrow();
		let interaction = InteractionState {
			pointers: state.previous_layout.pointer_hits(state.rlay.input()),
			enter_pressed: input_manager_ref.is_key_just_pressed(Key::Named(NamedKey::Enter)),
			enter_down: input_manager_ref.is_key_pressed(Key::Named(NamedKey::Enter)),
			context_menu_pressed: input_manager_ref
				.is_key_just_pressed(Key::Named(NamedKey::ContextMenu)),
			context_menu_down: input_manager_ref.is_key_pressed(Key::Named(NamedKey::ContextMenu)),
		};
		let mut frame = state.rlay.begin(layout_size);

		let mut render_ctx = RenderContext {
			frame: &mut frame,
			previous_layout: &state.previous_layout,
			interaction: &interaction,
			font_manager,
			image_manager,
			custom_elements: &mut custom_elements,
			frame_alloc: &frame_alloc,
		};
		root_component.render(&mut render_ctx);
		drop(render_ctx);

		frame.end(delta_time).unwrap_or_default()
	};

	if layout.needs_animation_frame {
		REQUEST_REDRAW.call();
	}

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
				let rrect = rlay_to_skia_rrect(command.bounds, radius);

				canvas.save();
				canvas.clip_rrect(rrect, ClipOp::Intersect, true);

				let mut paint = Paint::default();
				paint.set_anti_alias(true);
				if let Some(filter) = skia_safe::image_filters::blur((sigma, sigma), None, None, None) {
					paint.set_image_filter(filter);
				}
				canvas.draw_image(snapshot, (0.0, 0.0), Some(&paint));
				canvas.restore();
			}
		},
		&fonts,
		image_manager,
	);

	state.input_manager.borrow_mut().update();
	layout
}

impl Default for SkiaSurfaceCache {
	fn default() -> Self {
		Self {
			surfaces_by_fbo: HashMap::new(),
			width: 0,
			height: 0,
			target_fbo: 0,
		}
	}
}

impl SkiaSurfaceCache {
	fn ensure_surface_target(
		&mut self,
		gr_context: &mut DirectContext,
		width: i32,
		height: i32,
		fboid: u32,
	) -> &mut skia_safe::Surface {
		let size_changed = self.width != width || self.height != height;
		if size_changed {
			self.surfaces_by_fbo.clear();
			self.width = width;
			self.height = height;
		}

		self.target_fbo = fboid;
		if !self.surfaces_by_fbo.contains_key(&fboid) {
			let surface = make_skia_surface(gr_context, fboid, width, height);
			self.surfaces_by_fbo.insert(fboid, surface);
		}

		self
			.surfaces_by_fbo
			.get_mut(&self.target_fbo)
			.expect("active Shift Skia target surface missing")
	}
}

fn make_skia_surface(
	gr_context: &mut DirectContext,
	fboid: u32,
	width: i32,
	height: i32,
) -> skia_safe::Surface {
	let fb_info = gpu::gl::FramebufferInfo {
		fboid,
		format: gpu::gl::Format::RGBA8.into(),
		protected: gpu::Protected::No,
	};
	let backend_render_target = backend_render_targets::make_gl((width, height), 0, 8, fb_info);

	gpu::surfaces::wrap_backend_render_target(
		gr_context,
		&backend_render_target,
		gpu::SurfaceOrigin::TopLeft,
		ColorType::RGBA8888,
		None,
		None,
	)
	.expect("failed to create Skia surface")
}

fn monitor_at(
	ctx: &GlEventContext<'_, '_, ShiftApp>,
	position: (f64, f64),
) -> Option<(String, f64, f64)> {
	ctx.monitors().find_map(|monitor| {
		let inside = position.0 >= monitor.x as f64
			&& position.0 < (monitor.x + monitor.width) as f64
			&& position.1 >= monitor.y as f64
			&& position.1 < (monitor.y + monitor.height) as f64;
		if !inside {
			return None;
		}

		let local = monitor.cursor_relative_position(position);
		Some((monitor.id.clone(), local.0, local.1))
	})
}

fn transformed_touch_position(
	ctx: &GlEventContext<'_, '_, ShiftApp>,
	mut x: f64,
	mut y: f64,
) -> Option<(String, f64, f64)> {
	if x > 1.0 || y > 1.0 {
		x /= 65535.0;
		y /= 65535.0;
	}

	let max_x = ctx
		.monitors()
		.map(|monitor| monitor.x.saturating_add(monitor.width))
		.max()
		.unwrap_or(0)
		.max(1) as f64;
	let max_y = ctx
		.monitors()
		.map(|monitor| monitor.y.saturating_add(monitor.height))
		.max()
		.unwrap_or(0)
		.max(1) as f64;

	let position = (x.clamp(0.0, 1.0) * max_x, y.clamp(0.0, 1.0) * max_y);
	monitor_at(ctx, position)
}

fn shift_pointer_kind(pointer_type: PointerType) -> PointerKind {
	match pointer_type {
		PointerType::Mouse => PointerKind::Mouse,
		PointerType::Pen => PointerKind::Pen,
		PointerType::Touch => PointerKind::Touch,
		PointerType::Unknown => PointerKind::Unknown,
	}
}

fn shift_mouse_button(button: u32) -> rlay::MouseButton {
	match button {
		0 | 1 | 272 => rlay::MouseButton::Left,
		2 | 273 => rlay::MouseButton::Right,
		3 | 274 => rlay::MouseButton::Middle,
		other => rlay::MouseButton::Other(other as u16),
	}
}

fn mouse_button_index(button: u32) -> u16 {
	match button {
		0 | 1 | 272 => 0,
		2 | 273 => 1,
		3 | 274 => 2,
		other => other as u16,
	}
}
