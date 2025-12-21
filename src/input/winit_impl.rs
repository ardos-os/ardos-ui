use std::{collections::HashMap, sync::atomic::AtomicBool};

use winit::{
	event::{ElementState, Ime, KeyEvent},
	keyboard::Key,
};

use crate::input::InputManager;

pub struct WinitInputManager {
	mouse_position: (f32, f32),
	mouse_buttons_current: HashMap<u16, bool>,
	mouse_buttons_previous: HashMap<u16, bool>,
	mouse_buttons_pressed: HashMap<u16, bool>,
	keys_current: HashMap<super::Key, bool>,
	keys_previous: HashMap<super::Key, bool>,
	text_input: String,
	text_ime_buffer: String,
	text_ime_buffer_cursor: (usize, usize),
	ime_editing: bool,
	bytes_to_remove: (usize, usize),
	has_clicked_on_something: AtomicBool,
}

impl WinitInputManager {
	pub fn new() -> Self {
		Self {
			mouse_position: (0.0, 0.0),
			mouse_buttons_current: HashMap::new(),
			mouse_buttons_previous: HashMap::new(),
			mouse_buttons_pressed: HashMap::new(),
			keys_current: HashMap::new(),
			keys_previous: HashMap::new(),
			text_input: String::new(),
			text_ime_buffer: String::new(),
			text_ime_buffer_cursor: (0, 0),
			ime_editing: false,
			bytes_to_remove: (0, 0),
			has_clicked_on_something: Default::default(),
		}
	}

	pub fn update(&mut self) {
		// Move current state to previous
		self.mouse_buttons_previous = self.mouse_buttons_current.clone();
		self.mouse_buttons_pressed = self.mouse_buttons_current.clone();
		self.mouse_buttons_pressed.clear();
		self.keys_previous = self.keys_current.clone();
		self.text_input.clear();
		self.bytes_to_remove = (0, 0);
	}

	pub fn set_mouse_position(&mut self, x: f32, y: f32) {
		self.mouse_position = (x, y);
	}

	pub fn set_mouse_button(&mut self, button: u16, pressed: bool) {
		self.mouse_buttons_current.insert(button, pressed);
		self.mouse_buttons_pressed.insert(button, pressed);
	}

	pub fn handle_key_event(&mut self, event: KeyEvent) {
		// Block the app from trying to handle keyboard shortcuts while IME is active (for example Tab for focus)
		if self.ime_editing {
			return;
		}
		self
			.text_ime_buffer
			.push_str(&event.text.map(|t| t.to_string()).unwrap_or_default());
		let pressed = match event.state {
			ElementState::Pressed => true,
			ElementState::Released => false,
		};
		self.keys_current.insert(event.logical_key, pressed);
	}
	pub fn handle_ime_event(&mut self, ime: Ime) {
		match ime {
			Ime::Enabled => {
				self.ime_editing = true;
			}
			Ime::Preedit(new_preedit, cursor) => {
				self.text_ime_buffer_cursor = cursor.unwrap_or_default();
				self.text_ime_buffer = new_preedit;
			}
			Ime::Commit(text) => {
				self.ime_editing = false;
				self.text_input.push_str(&text);
			}
			Ime::DeleteSurrounding {
				before_bytes,
				after_bytes,
			} => {
				self.bytes_to_remove.0 += before_bytes;
				self.bytes_to_remove.1 += after_bytes;
			}
			Ime::Disabled => {
				self.ime_editing = false;
			}
		}
	}
}

impl InputManager for WinitInputManager {
	fn cursor_hit_something(&self) -> bool {
		self
			.has_clicked_on_something
			.swap(false, std::sync::atomic::Ordering::Relaxed)
	}
	fn set_cursor_clicked_something(&self) {
		self
			.has_clicked_on_something
			.store(true, std::sync::atomic::Ordering::Relaxed);
	}
	fn mouse_position(&self) -> (f32, f32) {
		self.mouse_position
	}

	fn is_mouse_button_pressed(&self, button: u16) -> bool {
		self
			.mouse_buttons_current
			.get(&button)
			.copied()
			.unwrap_or(false)
	}

	fn is_mouse_button_just_pressed(&self, button: u16) -> bool {
		let current = self
			.mouse_buttons_current
			.get(&button)
			.copied()
			.unwrap_or(false);
		let previous = self
			.mouse_buttons_previous
			.get(&button)
			.copied()
			.unwrap_or(false);
		(current && !previous)
			|| self
				.mouse_buttons_pressed
				.get(&button)
				.map_or(false, |&b| b)
	}

	fn is_mouse_button_just_released(&self, button: u16) -> bool {
		let current = self
			.mouse_buttons_current
			.get(&button)
			.copied()
			.unwrap_or(false);
		let previous = self
			.mouse_buttons_previous
			.get(&button)
			.copied()
			.unwrap_or(false);
		(!current && previous)
			|| self
				.mouse_buttons_pressed
				.get(&button)
				.map_or(false, |&b| !b)
	}

	fn is_key_pressed(&self, key: Key) -> bool {
		self.keys_current.get(&key).copied().unwrap_or(false)
	}

	fn is_key_just_pressed(&self, key: Key) -> bool {
		let current = self.keys_current.get(&key).copied().unwrap_or(false);
		let previous = self.keys_previous.get(&key).copied().unwrap_or(false);
		current && !previous
	}

	fn is_key_just_released(&self, key: Key) -> bool {
		let current = self.keys_current.get(&key).copied().unwrap_or(false);
		let previous = self.keys_previous.get(&key).copied().unwrap_or(false);
		!current && previous
	}

	fn text_input(&self) -> &str {
		&self.text_input
	}

	fn ime_buffer(&self) -> &str {
		&self.text_ime_buffer
	}

	fn ime_is_editing(&self) -> bool {
		self.ime_editing
	}

	fn bytes_to_remove(&self) -> (usize, usize) {
		self.bytes_to_remove
	}
}
