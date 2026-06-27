use crate::input::keyboard::Key;

pub mod keyboard;
#[cfg(feature = "shift")]
pub(crate) mod shift_impl;
#[cfg(feature = "winit")]
pub(crate) mod winit_impl;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
	pub shift: bool,
	pub ctrl: bool,
	pub alt: bool,
	pub super_key: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PointerKind {
	#[default]
	Mouse,
	Pen,
	Touch,
	Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TouchPoint {
	pub id: u64,
	pub position: (f32, f32),
}

pub trait InputManager {
	/// Get current mouse position
	fn mouse_position(&self) -> (f32, f32);

	/// Returns the kind of pointer device that most recently drove pointer input.
	fn pointer_kind(&self) -> PointerKind {
		PointerKind::Mouse
	}

	/// Whether applications should render a software cursor for the current input mode.
	fn cursor_visible(&self) -> bool {
		!matches!(self.pointer_kind(), PointerKind::Touch)
	}

	/// Active touch contacts in local UI coordinates.
	fn touch_points(&self) -> Vec<TouchPoint> {
		Vec::new()
	}

	/// Check if mouse button is currently pressed
	fn is_mouse_button_pressed(&self, button: u16) -> bool;

	/// Check if mouse button was just pressed this frame
	fn is_mouse_button_just_pressed(&self, button: u16) -> bool;

	/// Check if mouse button was just released this frame
	fn is_mouse_button_just_released(&self, button: u16) -> bool;

	/// Check if key is currently pressed
	fn is_key_pressed(&self, key: Key) -> bool;

	/// Check if key was just pressed this frame
	fn is_key_just_pressed(&self, key: Key) -> bool;

	/// Check if key generated a repeat press this frame.
	fn is_key_repeated(&self, key: Key) -> bool;

	/// Check if key was just released this frame
	fn is_key_just_released(&self, key: Key) -> bool;

	fn modifiers(&self) -> Modifiers;

	fn primary_modifier_pressed(&self) -> bool {
		let modifiers = self.modifiers();
		if cfg!(target_os = "macos") {
			modifiers.super_key
		} else {
			modifiers.ctrl
		}
	}

	/// Get text input for this frame, including repeated text events.
	fn text_input(&self) -> &str;

	/// Get text input for this frame, excluding repeated text events.
	fn text_input_without_repeat(&self) -> &str;

	/// Get the buffer that the user is still editing in the IME
	/// This needs to be displayed in the text input with an underline at the cursor position
	fn ime_buffer(&self) -> &str;

	/// Get the number of bytes to remove from the IME buffer
	fn bytes_to_remove(&self) -> (usize, usize);

	/// Check if the user is currently using an IME
	fn ime_is_editing(&self) -> bool;

	/// Request IME activation for the current frame.
	fn request_ime(&self);

	/// Anchor the IME candidate window to an element id for the current frame.
	fn set_ime_anchor(&self, id: &str);

	fn set_cursor_clicked_something(&self);
	fn cursor_hit_something(&self) -> bool;
}
