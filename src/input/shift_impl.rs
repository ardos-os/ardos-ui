use std::{
	cell::RefCell,
	collections::HashMap,
	sync::atomic::{AtomicBool, Ordering},
};

use smol_str::SmolStr;

use crate::{
	Key, NamedKey,
	input::{InputManager, Modifiers},
};

pub struct ShiftInputManager {
	mouse_position: (f32, f32),
	mouse_buttons_current: HashMap<u16, bool>,
	mouse_buttons_previous: HashMap<u16, bool>,
	mouse_buttons_pressed: HashMap<u16, bool>,
	keys_current: HashMap<Key, bool>,
	keys_previous: HashMap<Key, bool>,
	keys_repeated: HashMap<Key, bool>,
	text_input: String,
	text_input_without_repeat: String,
	modifiers: Modifiers,
	has_clicked_on_something: AtomicBool,
	ime_requested: AtomicBool,
	ime_anchor: RefCell<Option<String>>,
}

impl ShiftInputManager {
	pub fn new() -> Self {
		Self {
			mouse_position: (0.0, 0.0),
			mouse_buttons_current: HashMap::new(),
			mouse_buttons_previous: HashMap::new(),
			mouse_buttons_pressed: HashMap::new(),
			keys_current: HashMap::new(),
			keys_previous: HashMap::new(),
			keys_repeated: HashMap::new(),
			text_input: String::new(),
			text_input_without_repeat: String::new(),
			modifiers: Modifiers::default(),
			has_clicked_on_something: Default::default(),
			ime_requested: Default::default(),
			ime_anchor: Default::default(),
		}
	}

	pub fn update(&mut self) {
		self.mouse_buttons_previous = self.mouse_buttons_current.clone();
		self.mouse_buttons_pressed.clear();
		self.keys_previous = self.keys_current.clone();
		self.keys_repeated.clear();
		self.text_input.clear();
		self.text_input_without_repeat.clear();
		self.ime_requested.store(false, Ordering::Relaxed);
		*self.ime_anchor.borrow_mut() = None;
	}

	pub fn set_mouse_position(&mut self, x: f32, y: f32) {
		self.mouse_position = (x, y);
	}

	pub fn set_mouse_button(&mut self, button: u16, pressed: bool) {
		self.mouse_buttons_current.insert(button, pressed);
		self.mouse_buttons_pressed.insert(button, pressed);
	}

	pub fn handle_key(&mut self, linux_keycode: u32, pressed: bool) {
		let Some(key) = linux_keycode_to_key(linux_keycode) else {
			return;
		};

		if pressed && self.keys_current.get(&key).copied().unwrap_or(false) {
			self.keys_repeated.insert(key.clone(), true);
		}

		self.update_modifiers(&key, pressed);
		self.keys_current.insert(key, pressed);
	}

	pub fn handle_text(&mut self, text: &str) {
		let printable = text
			.chars()
			.filter(|ch| !ch.is_control())
			.collect::<String>();
		self.text_input.push_str(&printable);
		self.text_input_without_repeat.push_str(&printable);
	}

	fn update_modifiers(&mut self, key: &Key, pressed: bool) {
		match key {
			Key::Named(NamedKey::Shift) => self.modifiers.shift = pressed,
			Key::Named(NamedKey::Control) => self.modifiers.ctrl = pressed,
			Key::Named(NamedKey::Alt) | Key::Named(NamedKey::AltGraph) => self.modifiers.alt = pressed,
			Key::Named(NamedKey::Meta) | Key::Named(NamedKey::Super) => {
				self.modifiers.super_key = pressed
			}
			_ => {}
		}
	}
}

impl InputManager for ShiftInputManager {
	fn cursor_hit_something(&self) -> bool {
		self.has_clicked_on_something.swap(false, Ordering::Relaxed)
	}

	fn set_cursor_clicked_something(&self) {
		self.has_clicked_on_something.store(true, Ordering::Relaxed);
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
				.copied()
				.unwrap_or(false)
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
				.map_or(false, |pressed| !pressed)
	}

	fn is_key_pressed(&self, key: Key) -> bool {
		self.keys_current.get(&key).copied().unwrap_or(false)
	}

	fn is_key_just_pressed(&self, key: Key) -> bool {
		let current = self.keys_current.get(&key).copied().unwrap_or(false);
		let previous = self.keys_previous.get(&key).copied().unwrap_or(false);
		current && !previous
	}

	fn is_key_repeated(&self, key: Key) -> bool {
		self.keys_repeated.get(&key).copied().unwrap_or(false)
	}

	fn is_key_just_released(&self, key: Key) -> bool {
		let current = self.keys_current.get(&key).copied().unwrap_or(false);
		let previous = self.keys_previous.get(&key).copied().unwrap_or(false);
		!current && previous
	}

	fn modifiers(&self) -> Modifiers {
		self.modifiers
	}

	fn text_input(&self) -> &str {
		&self.text_input
	}

	fn text_input_without_repeat(&self) -> &str {
		&self.text_input_without_repeat
	}

	fn ime_buffer(&self) -> &str {
		""
	}

	fn bytes_to_remove(&self) -> (usize, usize) {
		(0, 0)
	}

	fn ime_is_editing(&self) -> bool {
		false
	}

	fn request_ime(&self) {
		self.ime_requested.store(true, Ordering::Relaxed);
	}

	fn set_ime_anchor(&self, id: &str) {
		*self.ime_anchor.borrow_mut() = Some(id.to_string());
	}
}

fn linux_keycode_to_key(keycode: u32) -> Option<Key> {
	let named = match keycode {
		1 => NamedKey::Escape,
		14 => NamedKey::Backspace,
		15 => NamedKey::Tab,
		28 => NamedKey::Enter,
		29 | 97 => NamedKey::Control,
		42 | 54 => NamedKey::Shift,
		56 => NamedKey::Alt,
		100 => NamedKey::AltGraph,
		102 => NamedKey::Home,
		103 => NamedKey::ArrowUp,
		104 => NamedKey::PageUp,
		105 => NamedKey::ArrowLeft,
		106 => NamedKey::ArrowRight,
		107 => NamedKey::End,
		108 => NamedKey::ArrowDown,
		109 => NamedKey::PageDown,
		110 => NamedKey::Insert,
		111 => NamedKey::Delete,
		125 | 126 => NamedKey::Super,
		59..=68 => return function_key(keycode - 58),
		87 => NamedKey::F11,
		88 => NamedKey::F12,
		_ => return printable_key(keycode),
	};

	Some(Key::Named(named))
}

fn function_key(number: u32) -> Option<Key> {
	let key = match number {
		1 => NamedKey::F1,
		2 => NamedKey::F2,
		3 => NamedKey::F3,
		4 => NamedKey::F4,
		5 => NamedKey::F5,
		6 => NamedKey::F6,
		7 => NamedKey::F7,
		8 => NamedKey::F8,
		9 => NamedKey::F9,
		10 => NamedKey::F10,
		_ => return None,
	};
	Some(Key::Named(key))
}

fn printable_key(keycode: u32) -> Option<Key> {
	let text = match keycode {
		2 => "1",
		3 => "2",
		4 => "3",
		5 => "4",
		6 => "5",
		7 => "6",
		8 => "7",
		9 => "8",
		10 => "9",
		11 => "0",
		12 => "-",
		13 => "=",
		16 => "q",
		17 => "w",
		18 => "e",
		19 => "r",
		20 => "t",
		21 => "y",
		22 => "u",
		23 => "i",
		24 => "o",
		25 => "p",
		26 => "[",
		27 => "]",
		30 => "a",
		31 => "s",
		32 => "d",
		33 => "f",
		34 => "g",
		35 => "h",
		36 => "j",
		37 => "k",
		38 => "l",
		39 => ";",
		40 => "'",
		41 => "`",
		43 => "\\",
		44 => "z",
		45 => "x",
		46 => "c",
		47 => "v",
		48 => "b",
		49 => "n",
		50 => "m",
		51 => ",",
		52 => ".",
		53 => "/",
		57 => " ",
		_ => return None,
	};

	Some(Key::Character(SmolStr::new(text)))
}
