use std::{cell::RefCell, rc::Rc};

use crate::{ClickableState, Element, Key, NamedKey, use_entity, use_input, use_ref};

#[derive(Clone)]
pub struct InputProps {
	pub initial_value: String,
	pub disabled: bool,
	pub repeat: bool,
	pub ime_anchor_id: Option<String>,
	pub on_change: Option<Rc<dyn Fn(String)>>,
	pub render: Rc<dyn Fn(InputRenderProps) -> Box<dyn Element>>,
}

impl Default for InputProps {
	fn default() -> Self {
		Self {
			initial_value: String::new(),
			disabled: false,
			repeat: true,
			ime_anchor_id: None,
			on_change: None,
			render: Rc::new(|_| Box::new(crate::Container::new())),
		}
	}
}

#[derive(Clone)]
pub struct InputRenderProps {
	pub value: String,
	pub before_cursor: String,
	pub ime_buffer: String,
	pub after_cursor: String,
	pub cursor: usize,
	pub focused: bool,
	pub disabled: bool,
	pub clickable_ref: Rc<RefCell<ClickableState>>,
}

#[derive(Clone)]
struct InputState {
	value: String,
	cursor: usize,
}

#[allow(non_snake_case)]
pub fn Input(props: InputProps) -> Box<dyn Element> {
	let (state, set_state) = use_entity(|| InputState {
		value: props.initial_value.clone(),
		cursor: chars_count(&props.initial_value),
	});
	let clickable_ref = use_ref(ClickableState::default());
	let input = use_input();

	let focused = clickable_ref.borrow().is_focused();
	if focused && !props.disabled {
		input.with(|input| {
			input.request_ime();
			if let Some(anchor_id) = &props.ime_anchor_id {
				input.set_ime_anchor(anchor_id);
			}
		});
	}

	if focused && !props.disabled {
		input.with(|input| {
			let mut next = state.borrow().clone();
			let mut changed = false;

			let (before, after) = input.bytes_to_remove();
			if before > 0 || after > 0 {
				changed |= remove_bytes_around_cursor(&mut next, before, after);
			}

			let value_chars = chars_count(&next.value);
			if key_pressed(input, Key::Named(NamedKey::Backspace), props.repeat) && next.cursor > 0 {
				let byte_index = char_index_to_byte_index(&next.value, next.cursor - 1);
				next.value.remove(byte_index);
				next.cursor -= 1;
				changed = true;
			} else if key_pressed(input, Key::Named(NamedKey::Delete), props.repeat) && next.cursor < value_chars {
				let byte_index = char_index_to_byte_index(&next.value, next.cursor);
				next.value.remove(byte_index);
				changed = true;
			} else if key_pressed(input, Key::Named(NamedKey::ArrowLeft), props.repeat) && next.cursor > 0 {
				next.cursor -= 1;
				changed = true;
			} else if key_pressed(input, Key::Named(NamedKey::ArrowRight), props.repeat)
				&& next.cursor < value_chars
			{
				next.cursor += 1;
				changed = true;
			} else if key_pressed(input, Key::Named(NamedKey::Home), props.repeat) {
				next.cursor = 0;
				changed = true;
			} else if key_pressed(input, Key::Named(NamedKey::End), props.repeat) {
				next.cursor = value_chars;
				changed = true;
			}

			let text_input = if props.repeat {
				input.text_input()
			} else {
				input.text_input_without_repeat()
			};

			if !text_input.is_empty() {
				let byte_index = char_index_to_byte_index(&next.value, next.cursor);
				next.value.insert_str(byte_index, text_input);
				next.cursor += chars_count(text_input);
				changed = true;
			}

			if changed {
				let value = next.value.clone();
				set_state(&|state| *state = next.clone());
				if let Some(on_change) = &props.on_change {
					on_change(value);
				}
			}
		});
	}

	let state = state.borrow();
	let cursor_byte = char_index_to_byte_index(&state.value, state.cursor);
	(props.render)(InputRenderProps {
		value: state.value.clone(),
		before_cursor: state.value[..cursor_byte].to_string(),
		ime_buffer: input.with(|input| input.ime_buffer().to_string()),
		after_cursor: state.value[cursor_byte..].to_string(),
		cursor: state.cursor,
		focused,
		disabled: props.disabled,
		clickable_ref,
	})
}

fn chars_count(s: &str) -> usize {
	s.chars().count()
}

fn char_index_to_byte_index(s: &str, char_index: usize) -> usize {
	if char_index == 0 {
		return 0;
	}

	s.char_indices()
		.nth(char_index)
		.map(|(i, _)| i)
		.unwrap_or(s.len())
}

fn remove_bytes_around_cursor(state: &mut InputState, before: usize, after: usize) -> bool {
	let cursor_byte = char_index_to_byte_index(&state.value, state.cursor);
	let start = cursor_byte.saturating_sub(before);
	let end = (cursor_byte + after).min(state.value.len());

	if start < end && state.value.is_char_boundary(start) && state.value.is_char_boundary(end) {
		state.value.replace_range(start..end, "");
		state.cursor = chars_count(&state.value[..start]);
		return true;
	}

	false
}

fn key_pressed(input: &dyn crate::InputManager, key: Key, repeat: bool) -> bool {
	input.is_key_just_pressed(key.clone()) || (repeat && input.is_key_repeated(key))
}
