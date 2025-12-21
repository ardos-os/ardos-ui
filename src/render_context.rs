use crate::{CustomElementData, FrameAllocator, InputManager, font_manager::FontManager};
use clay_layout::ClayLayoutScope;
use skia_safe::Image;

/// Per-frame rendering context passed down the element tree.
///
/// - `c` uses `CustomElementData` as Clay's `CustomElementData` generic so we can emit custom
///   render commands (e.g. backdrop blur).
/// - `frame_alloc` is used to allocate per-frame custom payloads whose references are stored
///   inside Clay declarations (Clay keeps raw pointers for the duration of the frame).
pub struct RenderContext<'clay: 'render, 'render: 'a, 'a> {
	pub c: &'a mut ClayLayoutScope<'clay, 'render, Image, CustomElementData>,
	pub font_manager: &'a mut FontManager,
	pub input_manager: &'a dyn InputManager,

	/// Per-frame allocator for custom payloads and other frame-scoped allocations.
	pub frame_alloc: &'render FrameAllocator<'render>,
}
