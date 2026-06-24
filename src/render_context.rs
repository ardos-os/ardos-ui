use crate::{CustomElementData, FrameAllocator, font_manager::FontManager, image::ImageManager};
use rlay::{Frame, LayoutResult, PointerHit};

pub struct InteractionState {
	pub pointers: Vec<PointerHit>,
	pub enter_pressed: bool,
	pub enter_down: bool,
	pub context_menu_pressed: bool,
	pub context_menu_down: bool,
}

/// Per-frame rendering context passed down the element tree.
///
/// - `c` uses `CustomElementData` as Clay's `CustomElementData` generic so we can emit custom
///   render commands (e.g. backdrop blur).
/// - `frame_alloc` is used to allocate per-frame custom payloads whose references are stored
///   inside Clay declarations (Clay keeps raw pointers for the duration of the frame).
pub struct RenderContext<'clay: 'render, 'render: 'a, 'a> {
	pub frame: &'a mut Frame<'clay>,
	pub previous_layout: &'a LayoutResult,
	pub interaction: &'a InteractionState,
	pub font_manager: &'a mut FontManager,
	pub(crate) image_manager: &'a mut ImageManager,
	pub custom_elements: &'a mut Vec<CustomElementData>,

	/// Per-frame allocator for custom payloads and other frame-scoped allocations.
	pub frame_alloc: &'render FrameAllocator<'render>,
}
