use std::{cell::RefCell, rc::Rc};

use crate::{
	ClickableState, Element, Key, NamedKey, use_clipboard, use_entity, use_input, use_ref,
};
pub type InputRenderer = Rc<dyn Fn(InputRenderProps) -> Box<dyn Element>>;
#[derive(Clone)]
pub struct InputProps {
	pub initial_value: String,
	pub disabled: bool,
	pub repeat: bool,
	pub ime_anchor_id: Option<String>,
	pub on_change: Option<Rc<dyn Fn(String)>>,
	pub render: InputRenderer,
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
	pub before_selection: String,
	pub selected_text: String,
	pub after_selection: String,
	pub before_cursor: String,
	pub ime_buffer: String,
	pub after_cursor: String,
	pub cursor: usize,
	pub cursor_visible: bool,
	pub focused: bool,
	pub disabled: bool,
	pub clickable_ref: Rc<RefCell<ClickableState>>,
}

#[derive(Clone)]
struct InputState {
	value: String,
	cursor: usize,
	selection_anchor: usize,
	selection_focus: usize,
}

impl InputState {
	fn collapsed(value: String) -> Self {
		let cursor = chars_count(&value);
		Self {
			value,
			cursor,
			selection_anchor: cursor,
			selection_focus: cursor,
		}
	}

	fn clear_selection(&mut self) {
		self.selection_anchor = self.cursor;
		self.selection_focus = self.cursor;
	}

	fn set_cursor(&mut self, cursor: usize, selecting: bool) {
		self.cursor = cursor.min(chars_count(&self.value));
		if selecting {
			self.selection_focus = self.cursor;
		} else {
			self.clear_selection();
		}
	}

	fn selected_range(&self) -> Option<(usize, usize)> {
		normalized_selection(self.selection_anchor, self.selection_focus)
	}
}

#[allow(non_snake_case)]
pub fn Input(props: InputProps) -> Box<dyn Element> {
	let (state, set_state) = use_entity(|| InputState::collapsed(props.initial_value.clone()));
	let clickable_ref = use_ref(ClickableState::default());
	let input = use_input();
	let clipboard = use_clipboard();

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
			let old_value = next.value.clone();
			let mut state_changed = false;

			let (before, after) = input.bytes_to_remove();
			if before > 0 || after > 0 {
				state_changed |= remove_bytes_around_cursor(&mut next, before, after);
			}

			let modifiers = input.modifiers();
			let select = modifiers.shift;
			let by_word = if cfg!(target_os = "macos") {
				modifiers.alt
			} else {
				modifiers.ctrl
			};

			if input.primary_modifier_pressed() && character_key_pressed(input, "a", props.repeat) {
				next.selection_anchor = 0;
				next.selection_focus = chars_count(&next.value);
				next.cursor = next.selection_focus;
				state_changed = true;
			} else if input.primary_modifier_pressed() && character_key_pressed(input, "c", props.repeat) {
				if let Some(text) = selected_text(&next) {
					clipboard.set_text(&text);
				}
			} else if input.primary_modifier_pressed() && character_key_pressed(input, "x", props.repeat) {
				if let Some(text) = selected_text(&next) {
					clipboard.set_text(&text);
					delete_selection(&mut next);
					state_changed = true;
				}
			} else if input.primary_modifier_pressed() && character_key_pressed(input, "v", props.repeat) {
				if let Some(text) = clipboard.get_text() {
					insert_text(&mut next, &text);
					state_changed = true;
				}
			} else if key_pressed(input, Key::Named(NamedKey::Backspace), props.repeat) {
				if !delete_selection(&mut next) && next.cursor > 0 {
					let byte_index = char_index_to_byte_index(&next.value, next.cursor - 1);
					next.value.remove(byte_index);
					next.cursor -= 1;
					next.clear_selection();
				}
				state_changed = true;
			} else if key_pressed(input, Key::Named(NamedKey::Delete), props.repeat) {
				if !delete_selection(&mut next) && next.cursor < chars_count(&next.value) {
					let byte_index = char_index_to_byte_index(&next.value, next.cursor);
					next.value.remove(byte_index);
					next.clear_selection();
				}
				state_changed = true;
			} else if key_pressed(input, Key::Named(NamedKey::ArrowLeft), props.repeat) {
				let cursor = if by_word {
					previous_word_index(&next.value, next.cursor)
				} else {
					next.cursor.saturating_sub(1)
				};
				next.set_cursor(cursor, select);
				state_changed = true;
			} else if key_pressed(input, Key::Named(NamedKey::ArrowRight), props.repeat) {
				let value_chars = chars_count(&next.value);
				let cursor = if by_word {
					next_word_index(&next.value, next.cursor)
				} else {
					(next.cursor + 1).min(value_chars)
				};
				next.set_cursor(cursor, select);
				state_changed = true;
			} else if key_pressed(input, Key::Named(NamedKey::Home), props.repeat) {
				next.set_cursor(0, select);
				state_changed = true;
			} else if key_pressed(input, Key::Named(NamedKey::End), props.repeat) {
				next.set_cursor(chars_count(&next.value), select);
				state_changed = true;
			}

			let text_input = if props.repeat {
				input.text_input()
			} else {
				input.text_input_without_repeat()
			};

			if !text_input.is_empty() && !input.primary_modifier_pressed() {
				insert_text(&mut next, text_input);
				state_changed = true;
			}

			if state_changed {
				let value_changed = next.value != old_value;
				let value = next.value.clone();
				set_state(&|state| *state = next.clone());
				if value_changed {
					if let Some(on_change) = &props.on_change {
						on_change(value);
					}
				}
			}
		});
	}

	let state = state.borrow();
	let cursor_byte = char_index_to_byte_index(&state.value, state.cursor);
	let (selection_start, selection_end) = state.selected_range().unwrap_or((state.cursor, state.cursor));
	let selection_start_byte = char_index_to_byte_index(&state.value, selection_start);
	let selection_end_byte = char_index_to_byte_index(&state.value, selection_end);

	(props.render)(InputRenderProps {
		value: state.value.clone(),
		before_selection: state.value[..selection_start_byte].to_string(),
		selected_text: state.value[selection_start_byte..selection_end_byte].to_string(),
		after_selection: state.value[selection_end_byte..].to_string(),
		before_cursor: state.value[..cursor_byte].to_string(),
		ime_buffer: input.with(|input| input.ime_buffer().to_string()),
		after_cursor: state.value[cursor_byte..].to_string(),
		cursor: state.cursor,
		cursor_visible: focused && state.selected_range().is_none(),
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

fn normalized_selection(anchor: usize, focus: usize) -> Option<(usize, usize)> {
	if anchor == focus {
		None
	} else if anchor < focus {
		Some((anchor, focus))
	} else {
		Some((focus, anchor))
	}
}

fn selected_text(state: &InputState) -> Option<String> {
	let (start, end) = state.selected_range()?;
	let start = char_index_to_byte_index(&state.value, start);
	let end = char_index_to_byte_index(&state.value, end);
	Some(state.value[start..end].to_string())
}

fn delete_selection(state: &mut InputState) -> bool {
	let Some((start, end)) = state.selected_range() else {
		return false;
	};
	let start_byte = char_index_to_byte_index(&state.value, start);
	let end_byte = char_index_to_byte_index(&state.value, end);
	state.value.replace_range(start_byte..end_byte, "");
	state.cursor = start;
	state.clear_selection();
	true
}

fn insert_text(state: &mut InputState, text: &str) {
	delete_selection(state);
	let byte_index = char_index_to_byte_index(&state.value, state.cursor);
	state.value.insert_str(byte_index, text);
	state.cursor += chars_count(text);
	state.clear_selection();
}

fn remove_bytes_around_cursor(state: &mut InputState, before: usize, after: usize) -> bool {
	let cursor_byte = char_index_to_byte_index(&state.value, state.cursor);
	let start = cursor_byte.saturating_sub(before);
	let end = (cursor_byte + after).min(state.value.len());

	if start < end && state.value.is_char_boundary(start) && state.value.is_char_boundary(end) {
		state.value.replace_range(start..end, "");
		state.cursor = chars_count(&state.value[..start]);
		state.clear_selection();
		return true;
	}

	false
}

fn previous_word_index(s: &str, cursor: usize) -> usize {
	let chars: Vec<char> = s.chars().collect();
	let mut i = cursor.min(chars.len());
	while i > 0 && chars[i - 1].is_whitespace() {
		i -= 1;
	}
	while i > 0 && !chars[i - 1].is_whitespace() {
		i -= 1;
	}
	i
}

fn next_word_index(s: &str, cursor: usize) -> usize {
	let chars: Vec<char> = s.chars().collect();
	let mut i = cursor.min(chars.len());
	while i < chars.len() && !chars[i].is_whitespace() {
		i += 1;
	}
	while i < chars.len() && chars[i].is_whitespace() {
		i += 1;
	}
	i
}

fn key_pressed(input: &dyn crate::InputManager, key: Key, repeat: bool) -> bool {
	input.is_key_just_pressed(key.clone()) || (repeat && input.is_key_repeated(key))
}

fn character_key_pressed(input: &dyn crate::InputManager, key: &str, repeat: bool) -> bool {
	key_pressed(input, Key::Character(key.into()), repeat)
		|| key_pressed(input, Key::Character(key.to_uppercase().into()), repeat)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn char_index_to_byte_index_handles_utf8() {
		assert_eq!(char_index_to_byte_index("aé日", 0), 0);
		assert_eq!(char_index_to_byte_index("aé日", 1), 1);
		assert_eq!(char_index_to_byte_index("aé日", 2), 3);
		assert_eq!(char_index_to_byte_index("aé日", 3), 6);
	}

	#[test]
	fn normalized_selection_sorts_and_ignores_empty_selection() {
		assert_eq!(normalized_selection(2, 2), None);
		assert_eq!(normalized_selection(1, 3), Some((1, 3)));
		assert_eq!(normalized_selection(3, 1), Some((1, 3)));
	}

	#[test]
	fn delete_and_insert_replace_selection() {
		let mut state = InputState {
			value: "hello world".into(),
			cursor: 5,
			selection_anchor: 0,
			selection_focus: 5,
		};
		insert_text(&mut state, "bye");
		assert_eq!(state.value, "bye world");
		assert_eq!(state.cursor, 3);
		assert_eq!(state.selected_range(), None);
	}

	#[test]
	fn word_navigation_skips_words_and_spaces() {
		assert_eq!(previous_word_index("one two", 7), 4);
		assert_eq!(previous_word_index("one  two", 5), 0);
		assert_eq!(next_word_index("one two", 0), 4);
		assert_eq!(next_word_index("one  two", 3), 5);
	}
}
