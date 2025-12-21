use std::cell::RefCell;
use std::rc::Rc;
mod clickable;
use crate::focus_system::GLOBAL_FOCUS_MANAGER;
use crate::render_context::RenderContext;
use crate::{Component, element::Element};
use crate::{begin_component, end_component, use_ref};
use clay_layout::{
	Color, Declaration,
	elements::{FloatingAttachPointType, FloatingAttachToElement, PointerCaptureMode},
	layout::{Alignment, LayoutDirection, Padding},
	math::{Dimensions, Vector2},
};
use clickable::Clickable;
pub use clickable::ClickableState;
pub type Justify = clay_layout::layout::LayoutAlignmentX;
pub type Align = clay_layout::layout::LayoutAlignmentY;
pub type Sizing = clay_layout::layout::Sizing;

// Note: we intentionally use `clay_layout`'s floating enums directly (imported above)
// to avoid type mismatches when forwarding to `Declaration::floating()`.

#[derive(Debug, Clone, Copy)]
pub struct FloatingAttachPoints {
	pub element: FloatingAttachPointType,
	pub parent: FloatingAttachPointType,
}

impl Default for FloatingAttachPoints {
	fn default() -> Self {
		Self {
			element: FloatingAttachPointType::LeftTop,
			parent: FloatingAttachPointType::LeftTop,
		}
	}
}

/// Options forwarded to Clay's floating configuration via `Declaration::floating()`.
#[derive(Debug, Clone)]
pub struct FloatingOptions {
	pub offset: Vector2,
	pub dimensions: Dimensions,
	pub z_index: i16,
	pub parent_id: u32,
	pub attach_points: FloatingAttachPoints,
	pub attach_to: FloatingAttachToElement,
	pub pointer_capture_mode: PointerCaptureMode,
}

impl Default for FloatingOptions {
	fn default() -> Self {
		Self {
			offset: Vector2::new(0.0, 0.0),
			dimensions: Dimensions::new(0.0, 0.0),
			z_index: 0,
			parent_id: 0,
			attach_points: Default::default(),
			attach_to: FloatingAttachToElement::None,
			pointer_capture_mode: PointerCaptureMode::Capture,
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
	#[default]
	Row,
	Column,
}
#[derive(Copy, Clone, Debug, Default)]
pub struct BorderWidth {
	/// Border width on the left side.
	pub left: u16,
	/// Border width on the right side.
	pub right: u16,
	/// Border width on the top side.
	pub top: u16,
	/// Border width on the bottom side.
	pub bottom: u16,
	/// Border width between child elements.
	pub between_children: u16,
}

#[derive(Copy, Clone, Debug)]
pub struct Border {
	pub width: BorderWidth,
	pub color: Color,
}
impl Default for Border {
	fn default() -> Self {
		Self {
			width: Default::default(),
			color: Color::rgb(0., 0., 0.),
		}
	}
}
#[derive(Debug, Clone)]
pub struct ContainerStyle {
	pub background_color: Color,
	pub border_radius: (f32, f32, f32, f32),
	pub size: (Sizing, Sizing),
	pub gap: u16,
	pub align: Align,
	pub justify: Justify,
	pub direction: Direction,
	pub padding: (u16, u16, u16, u16),
	pub border: Border,
	pub scroll_x: bool,
	pub scroll_y: bool,
	pub clip_x: bool,
	pub clip_y: bool,

	/// If set, enables Clay "floating" mode and forwards these options into the `Declaration`.
	pub floating: Option<FloatingOptions>,
}
impl Default for ContainerStyle {
	fn default() -> Self {
		Self {
			padding: (0, 0, 0, 0),
			background_color: Color::rgba(0., 0., 0., 0.),
			border_radius: (0., 0., 0., 0.),
			size: (Sizing::Grow(0., f32::MAX), Sizing::Fit(0., f32::MAX)),
			gap: 0,
			align: Align::Top,
			justify: Justify::Left,
			direction: Direction::Column,
			border: Default::default(),
			scroll_x: false,
			scroll_y: false,
			clip_x: false,
			clip_y: false,

			floating: None,
		}
	}
}
impl ContainerStyle {
	pub fn clip_x(mut self, clip_x: bool) -> Self {
		self.clip_x = clip_x;
		self
	}

	/// Enables Clay floating mode for this container.
	pub fn floating(mut self, floating: FloatingOptions) -> Self {
		self.floating = Some(floating);
		self
	}

	pub fn clip_y(mut self, clip_y: bool) -> Self {
		self.clip_y = clip_y;
		self
	}

	pub fn background_color(mut self, color: impl Into<Color>) -> Self {
		self.background_color = color.into();
		self
	}

	pub fn border_radius(
		mut self,
		top_left: f32,
		top_right: f32,
		bottom_left: f32,
		bottom_right: f32,
	) -> Self {
		self.border_radius = (top_left, top_right, bottom_left, bottom_right);
		self
	}

	pub fn size(mut self, width: Sizing, height: Sizing) -> Self {
		self.size = (width, height);
		self
	}

	pub fn gap(mut self, gap: u16) -> Self {
		self.gap = gap;
		self
	}

	pub fn align(mut self, align: Align) -> Self {
		self.align = align;
		self
	}

	pub fn justify(mut self, justify: Justify) -> Self {
		self.justify = justify;
		self
	}

	pub fn direction(mut self, direction: Direction) -> Self {
		self.direction = direction;
		self
	}

	pub fn padding(mut self, left: u16, right: u16, top: u16, bottom: u16) -> Self {
		self.padding = (left, right, top, bottom);
		self
	}

	pub fn border(mut self, border: Border) -> Self {
		self.border = border;
		self
	}

	pub fn border_color(mut self, color: impl Into<Color>) -> Self {
		self.border.color = color.into();
		self
	}

	pub fn border_width(mut self, width: u16) -> Self {
		self.border.width.left = width;
		self.border.width.right = width;
		self.border.width.top = width;
		self.border.width.bottom = width;
		self
	}

	pub fn border_left(mut self, width: u16) -> Self {
		self.border.width.left = width;
		self
	}

	pub fn border_right(mut self, width: u16) -> Self {
		self.border.width.right = width;
		self
	}

	pub fn border_top(mut self, width: u16) -> Self {
		self.border.width.top = width;
		self
	}

	pub fn border_bottom(mut self, width: u16) -> Self {
		self.border.width.bottom = width;
		self
	}

	pub fn border_between_children(mut self, width: u16) -> Self {
		self.border.width.between_children = width;
		self
	}
	pub fn scroll_x(mut self, scroll_x: bool) -> Self {
		self.scroll_x = scroll_x;
		self
	}

	pub fn scroll_y(mut self, scroll_y: bool) -> Self {
		self.scroll_y = scroll_y;
		self
	}
}

/// A generic container element that can hold other elements.
///
/// This container element is designed to be flexible and can be used to create a variety of layouts.
/// It supports various styling options such as background color, border radius, size, gap, alignment, and direction.
/// This is the equivalent of a `<div>` element with `display: flex´ in HTML, and can be used to build a variety of different components.
///
/// If you need the container to be interactive, you can nest a `Clickable` element to handle user interactions.
pub struct Container {
	pub children: Vec<Box<dyn Element>>,
	pub style: ContainerStyle,
	pub style_if_hovered: Box<dyn Fn(ContainerStyle) -> ContainerStyle>,
	pub style_if_pressed: Box<dyn Fn(ContainerStyle) -> ContainerStyle>,
	pub style_if_focused: Box<dyn Fn(ContainerStyle) -> ContainerStyle>,
	pub(crate) clickable: Option<Clickable>,
	pub(crate) clickable_state: Rc<RefCell<ClickableState>>,
}

impl Default for Container {
	fn default() -> Self {
		begin_component("container");
		let clickable_state = use_ref(ClickableState::default());
		end_component();
		Self {
			children: Vec::new(),
			style: ContainerStyle::default(),
			style_if_hovered: Box::new(|style| style),
			style_if_pressed: Box::new(|style| style),
			style_if_focused: Box::new(|style| style),

			clickable: None,
			clickable_state,
		}
	}
}

impl Container {
	pub fn new() -> Self {
		Self::default()
	}

	// --- Floating (Clay) ----------------------------------------------------
	//
	// These are Container builder methods (not ContainerStyle) so they can be used
	// directly from RSML attributes like:
	// <container floating floating_offset={(10.0, 20.0)} floating_z_index={10} />
	//
	// Each setter lazily initializes `style.floating` to defaults.

	/// Enables/disables Clay floating mode for this container.
	///
	/// HTML-like semantics in RSML:
	/// - `floating` => `floating(true)`
	/// - `floating={true}` => `floating(true)`
	/// - `floating={false}` => `floating(false)`
	pub fn floating(mut self, enabled: bool) -> Self {
		if enabled {
			if self.style.floating.is_none() {
				self.style.floating = Some(FloatingOptions::default());
			}
		} else {
			self.style.floating = None;
		}
		self
	}

	/// Sets Clay floating offset. Accepts either a `Vector2` or `(f32, f32)`.
	pub fn floating_offset(mut self, offset: impl Into<Vector2>) -> Self {
		let floating = self.style.floating.get_or_insert_with(FloatingOptions::default);
		floating.offset = offset.into();
		self
	}

	/// Sets Clay floating dimensions. Accepts either `Dimensions` or `(f32, f32)`.
	pub fn floating_dimensions(mut self, dimensions: impl Into<Dimensions>) -> Self {
		let floating = self.style.floating.get_or_insert_with(FloatingOptions::default);
		floating.dimensions = dimensions.into();
		self
	}

	/// Sets Clay floating Z-index.
	pub fn floating_z_index(mut self, z_index: i16) -> Self {
		let floating = self.style.floating.get_or_insert_with(FloatingOptions::default);
		floating.z_index = z_index;
		self
	}

	/// Sets Clay floating parent ID.
	pub fn floating_parent_id(mut self, parent_id: u32) -> Self {
		let floating = self.style.floating.get_or_insert_with(FloatingOptions::default);
		floating.parent_id = parent_id;
		self
	}

	/// Sets Clay floating attach points (element attach point, then parent attach point).
	pub fn floating_attach_points(
		mut self,
		element: FloatingAttachPointType,
		parent: FloatingAttachPointType,
	) -> Self {
		let floating = self.style.floating.get_or_insert_with(FloatingOptions::default);
		floating.attach_points = FloatingAttachPoints { element, parent };
		self
	}

	/// Sets Clay floating attach-to mode.
	pub fn floating_attach_to(mut self, attach_to: FloatingAttachToElement) -> Self {
		let floating = self.style.floating.get_or_insert_with(FloatingOptions::default);
		floating.attach_to = attach_to;
		self
	}

	/// Sets Clay floating pointer capture mode.
	pub fn floating_pointer_capture_mode(mut self, mode: PointerCaptureMode) -> Self {
		let floating = self.style.floating.get_or_insert_with(FloatingOptions::default);
		floating.pointer_capture_mode = mode;
		self
	}

	pub fn clickable_ref(mut self, state: Rc<RefCell<ClickableState>>) -> Self {
		self.clickable_state = state;
		self
	}
	pub fn child(mut self, element: impl Element + 'static) -> Self {
		if let Some(clickable) = self.clickable.as_mut() {
			if let Some(focus_node_id) = clickable.focus_node_id {
				let nodes = element.focus_nodes();
				GLOBAL_FOCUS_MANAGER.with_borrow_mut(move |f| {
					f.set_parent(nodes, focus_node_id);
				})
			}
		}
		self.children.push(Box::new(element));
		self
	}
	pub fn component(mut self, component: impl Into<Component>) -> Self {
		self.children.push(Box::new(component.into()));
		self
	}
	pub fn background_color(mut self, color: impl Into<Color>) -> Self {
		self.style.background_color = color.into();
		self
	}

	pub fn w_expand(mut self) -> Self {
		self.style.size.0 = match self.style.size.0 {
			Sizing::Fit(min, max) => Sizing::Grow(min, max),
			Sizing::Fixed(size) => Sizing::Grow(size, size),
			Sizing::Grow(min, max) => Sizing::Grow(min, max),
			o => o,
		};
		self
	}
	pub fn h_expand(mut self) -> Self {
		self.style.size.1 = match self.style.size.1 {
			Sizing::Fit(min, max) => Sizing::Grow(min, max),
			Sizing::Fixed(size) => Sizing::Grow(size, size),
			Sizing::Grow(min, max) => Sizing::Grow(min, max),
			o => o,
		};
		self
	}
	pub fn w_fit(mut self) -> Self {
		self.style.size.0 = match self.style.size.0 {
			Sizing::Fit(min, max) => Sizing::Fit(min, max),
			Sizing::Fixed(size) => Sizing::Fit(size, size),
			Sizing::Grow(min, max) => Sizing::Fit(min, max),
			o => o,
		};
		self
	}
	pub fn h_fit(mut self) -> Self {
		self.style.size.1 = match self.style.size.1 {
			Sizing::Fit(min, max) => Sizing::Fit(min, max),
			Sizing::Fixed(size) => Sizing::Fit(size, size),
			Sizing::Grow(min, max) => Sizing::Fit(min, max),
			o => o,
		};
		self
	}
	pub fn w_fixed(mut self, width: f32) -> Self {
		self.style.size.0 = Sizing::Fixed(width);
		self
	}
	pub fn h_fixed(mut self, height: f32) -> Self {
		self.style.size.1 = Sizing::Fixed(height);
		self
	}

	pub fn w_percent(mut self, percentage: f32) -> Self {
		self.style.size.0 = Sizing::Percent(percentage);
		self
	}

	pub fn h_percent(mut self, percentage: f32) -> Self {
		self.style.size.1 = Sizing::Percent(percentage);
		self
	}

	pub fn min_width(mut self, width: f32) -> Self {
		self.style.size.0 = match self.style.size.0 {
			Sizing::Fit(_, max) => Sizing::Fit(width, max),
			Sizing::Fixed(size) => Sizing::Fixed(size.min(width)),
			Sizing::Grow(_, max) => Sizing::Grow(width, max),
			o => o,
		};
		self
	}

	pub fn min_height(mut self, height: f32) -> Self {
		self.style.size.1 = match self.style.size.1 {
			Sizing::Fit(_, max) => Sizing::Fit(height, max),
			Sizing::Fixed(size) => Sizing::Fixed(size.min(height)),
			Sizing::Grow(_, max) => Sizing::Grow(height, max),
			o => o,
		};
		self
	}

	pub fn max_width(mut self, width: f32) -> Self {
		self.style.size.0 = match self.style.size.0 {
			Sizing::Fit(min, _) => Sizing::Fit(min, width),
			Sizing::Fixed(size) => Sizing::Fixed(size.min(width)),
			Sizing::Grow(min, _) => Sizing::Grow(min, width),
			o => o,
		};
		self
	}

	pub fn max_height(mut self, height: f32) -> Self {
		self.style.size.1 = match self.style.size.1 {
			Sizing::Fit(min, _) => Sizing::Fit(min, height),
			Sizing::Fixed(size) => Sizing::Fixed(size.min(height)),
			Sizing::Grow(min, _) => Sizing::Grow(min, height),
			o => o,
		};
		self
	}
	pub fn fixed_width(mut self, width: f32) -> Self {
		self.style.size.0 = Sizing::Fixed(width);
		self
	}
	pub fn fixed_height(mut self, height: f32) -> Self {
		self.style.size.1 = Sizing::Fixed(height);
		self
	}
	pub fn gap(mut self, gap: u16) -> Self {
		self.style.gap = gap;
		self
	}

	pub fn align(mut self, align: Align) -> Self {
		self.style.align = align;
		self
	}

	pub fn justify(mut self, justify: Justify) -> Self {
		self.style.justify = justify;
		self
	}

	pub fn center(mut self) -> Self {
		self.style.align = Align::Center;
		self.style.justify = Justify::Center;
		self
	}

	pub fn direction(mut self, direction: Direction) -> Self {
		self.style.direction = direction;
		self
	}

	// Convenience methods for common patterns
	pub fn row() -> Self {
		Self::new().direction(Direction::Row)
	}

	pub fn column() -> Self {
		Self::new().direction(Direction::Column)
	}

	pub fn weird_padding(mut self, top: u16, right: u16, bottom: u16, left: u16) -> Self {
		self.style.padding = (left, right, top, bottom);
		self
	}

	pub fn symmetric_padding(mut self, horizontal: u16, vertical: u16) -> Self {
		self.style.padding = (horizontal, horizontal, vertical, vertical);
		self
	}

	pub fn padding_all(mut self, all: u16) -> Self {
		self.style.padding = (all, all, all, all);
		self
	}
	pub fn rounded_l(mut self, left_radius: f32) -> Self {
		self.style.border_radius.0 = left_radius;
		self.style.border_radius.2 = left_radius;
		self
	}
	pub fn rounded_r(mut self, right_radius: f32) -> Self {
		self.style.border_radius.1 = right_radius;
		self.style.border_radius.3 = right_radius;
		self
	}
	pub fn rounded_b(mut self, bottom_radius: f32) -> Self {
		self.style.border_radius.2 = bottom_radius;
		self.style.border_radius.3 = bottom_radius;
		self
	}
	pub fn rounded_t(mut self, top_radius: f32) -> Self {
		self.style.border_radius.0 = top_radius;
		self.style.border_radius.1 = top_radius;
		self
	}

	pub fn rounded(mut self, radius: f32) -> Self {
		self.style.border_radius.0 = radius;
		self.style.border_radius.1 = radius;
		self.style.border_radius.2 = radius;
		self.style.border_radius.3 = radius;
		self
	}
	pub fn style_if_hovered<F>(mut self, f: F) -> Self
	where
		F: Fn(ContainerStyle) -> ContainerStyle + 'static,
	{
		self.style_if_hovered = Box::new(f);
		self
	}
	pub fn style_if_pressed<F>(mut self, f: F) -> Self
	where
		F: Fn(ContainerStyle) -> ContainerStyle + 'static,
	{
		self.style_if_pressed = Box::new(f);
		self
	}
	pub fn style_if_focused<F>(mut self, f: F) -> Self
	where
		F: Fn(ContainerStyle) -> ContainerStyle + 'static,
	{
		self.style_if_focused = Box::new(f);
		self
	}

	pub fn border_color(mut self, color: impl Into<Color>) -> Self {
		self.style.border.color = color.into();
		self
	}

	pub fn border_width(mut self, width: u16) -> Self {
		self.style.border.width.bottom = width;
		self.style.border.width.top = width;
		self.style.border.width.left = width;
		self.style.border.width.right = width;
		self
	}

	pub fn border_left(mut self, width: u16) -> Self {
		self.style.border.width.left = width;
		self
	}

	pub fn border_right(mut self, width: u16) -> Self {
		self.style.border.width.right = width;
		self
	}

	pub fn border_top(mut self, width: u16) -> Self {
		self.style.border.width.top = width;
		self
	}

	pub fn border_bottom(mut self, width: u16) -> Self {
		self.style.border.width.bottom = width;
		self
	}

	pub fn border_between_children(mut self, width: u16) -> Self {
		self.style.border.width.between_children = width;
		self
	}

	pub fn scroll_x(mut self, scroll_x: bool) -> Self {
		self.style.scroll_x = scroll_x;
		self
	}

	pub fn scroll_y(mut self, scroll_y: bool) -> Self {
		self.style.scroll_y = scroll_y;
		self
	}
	pub fn clip_x(mut self, clip_x: bool) -> Self {
		self.style.clip_x = clip_x;
		self
	}
	pub fn clip_y(mut self, clip_y: bool) -> Self {
		self.style.clip_y = clip_y;
		self
	}
}

impl Element for Container {
	fn render<'clay: 'render, 'render>(&'render self, ctx: &mut RenderContext<'clay, 'render, '_>) {
		ctx.c.with_styling(
			|c| {
				let mut clickable_state = self.clickable_state.borrow_mut();
				if let Some(clickable) = &self.clickable {
					clickable.update(ctx.input_manager, &mut clickable_state, c.hovered());
				}
				let mut declaration = Declaration::new();
				let mut effective_style = self.style.clone();
				if c.hovered() {
					effective_style = (self.style_if_hovered)(effective_style);
				}

				if clickable_state.down {
					effective_style = (self.style_if_pressed)(effective_style);
				}
				if clickable_state.is_focused() {
					effective_style = (self.style_if_focused)(effective_style);
				}
				declaration
					.layout()
					.direction(match effective_style.direction {
						Direction::Row => LayoutDirection::LeftToRight,
						Direction::Column => LayoutDirection::TopToBottom,
					})
					.width(effective_style.size.0)
					.height(effective_style.size.1)
					.child_gap(effective_style.gap)
					.child_alignment(Alignment::new(
						effective_style.justify,
						effective_style.align,
					))
					.padding(Padding::new(
						effective_style.padding.0,
						effective_style.padding.1,
						effective_style.padding.2,
						effective_style.padding.3,
					))
					.end()
					.corner_radius()
					.top_left(effective_style.border_radius.0)
					.top_right(effective_style.border_radius.1)
					.bottom_left(effective_style.border_radius.2)
					.bottom_right(effective_style.border_radius.3)
					.end()
					.border()
					.between_children(effective_style.border.width.between_children)
					.color(effective_style.border.color)
					.top(effective_style.border.width.top)
					.right(effective_style.border.width.right)
					.bottom(effective_style.border.width.bottom)
					.left(effective_style.border.width.left)
					.end()
					.background_color(effective_style.background_color);

				if let Some(floating) = &effective_style.floating {
					declaration
						.floating()
						.offset(floating.offset)
						.dimensions(floating.dimensions)
						.z_index(floating.z_index)
						.parent_id(floating.parent_id)
						.attach_points(floating.attach_points.element, floating.attach_points.parent)
						.attach_to(floating.attach_to.clone())
						.pointer_capture_mode(floating.pointer_capture_mode)
						.end();
				}
				let mut scroll_offset = c.scroll_offset();
				if !effective_style.scroll_x {
					scroll_offset.x = 0.;
				}
				if !effective_style.scroll_y {
					scroll_offset.y = 0.;
				}
				declaration.clip(effective_style.scroll_x || effective_style.clip_x, effective_style.scroll_y || effective_style.clip_y, scroll_offset);
				declaration
			},
			|c| {
				let mut child_ctx = RenderContext {
					c,
					font_manager: &mut *ctx.font_manager,
					input_manager: ctx.input_manager,
				};
				for child in &self.children {
					child.render(&mut child_ctx);
				}
			},
		);
	}
	fn focus_nodes(&self) -> std::collections::HashSet<uuid::Uuid> {
		let mut nodes = self.children.focus_nodes();
		if let Some(focus_node_id) = self.clickable.as_ref().and_then(|c| c.focus_node_id) {
			nodes.insert(focus_node_id);
		}
		nodes
	}
}
