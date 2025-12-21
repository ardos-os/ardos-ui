use uuid::Uuid;

use crate::{
	Container, Element, InputManager, NamedKey, begin_component, end_component,
	focus_system::GLOBAL_FOCUS_MANAGER, input::Key, use_entity, use_memo, use_state,
};

/// Estado interno do Clickable para tracking de hover/press
#[derive(Default, Clone, Copy)]
pub struct ClickableState {
	pub hovered: bool,
	pub pressed: bool,
	pub down: bool,
	pub right_down: bool,
	pub right_pressed: bool,
	pub focus_node_id: Option<Uuid>,
}

impl ClickableState {
	pub fn is_focused(&self) -> bool {
		if let Some(focus_node_id) = self.focus_node_id {
			GLOBAL_FOCUS_MANAGER.with_borrow(|f| f.focused() == Some(focus_node_id))
		} else {
			false
		}
	}
	pub fn is_indirectly_focused(&self) -> bool {
		if let Some(focus_node_id) = self.focus_node_id {
			GLOBAL_FOCUS_MANAGER.with_borrow(|f| f.has_focused_child(focus_node_id))
		} else {
			false
		}
	}
	pub fn set_focus(&self) {
		if let Some(focus_node_id) = self.focus_node_id {
			GLOBAL_FOCUS_MANAGER.with_borrow_mut(|f| f.set_focus(focus_node_id))
		}
	}
}

/// Turns the parent container into a clickable element.

pub(crate) struct Clickable {
	pub(crate) on_click: Option<Box<dyn Fn()>>,
	pub(crate) on_mouse_enter: Option<Box<dyn Fn()>>,
	pub(crate) on_mouse_leave: Option<Box<dyn Fn()>>,
	pub(crate) on_right_click: Option<Box<dyn Fn()>>,
	pub(crate) focus_node_id: Option<Uuid>,
}

impl Clickable {
	pub fn new() -> Self {
		Self {
			on_click: None,
			on_mouse_enter: None,
			on_mouse_leave: None,
			on_right_click: None,
			focus_node_id: None,
		}
	}
	pub fn update(
		&self,
		input_manager: &dyn InputManager,
		state: &mut ClickableState,
		is_hovered: bool,
	) {
		state.focus_node_id = self.focus_node_id;
		state.down = (input_manager.is_mouse_button_pressed(0) && is_hovered)
			|| (input_manager.is_key_pressed(Key::Named(NamedKey::Enter)) && state.is_focused());
		state.right_down = (input_manager.is_mouse_button_pressed(1) && is_hovered)
			|| (input_manager.is_key_pressed(Key::Named(NamedKey::ContextMenu)) && state.is_focused());
		let is_clicked = (input_manager.is_mouse_button_just_pressed(0) && is_hovered)
			|| (input_manager.is_key_just_pressed(Key::Named(NamedKey::Enter)) && state.is_focused());
		if is_clicked != state.pressed {
			state.pressed = is_clicked;
		}
		if let Some(on_click) = &self.on_click {
			if is_clicked {
				state.set_focus();
				on_click();
			}
		}
		let is_right_clicked = (input_manager.is_mouse_button_just_pressed(1) && is_hovered)
			|| (input_manager.is_key_just_pressed(Key::Named(NamedKey::ContextMenu))
				&& state.is_focused());
		if is_right_clicked != state.right_pressed {
			state.right_pressed = is_right_clicked;
		}
		if let Some(on_right_click) = &self.on_right_click {
			if is_right_clicked {
				state.set_focus();
				input_manager.set_cursor_clicked_something();
				on_right_click();
			}
		}
		if is_hovered != state.hovered {
			state.hovered = is_hovered;
			if is_hovered {
				if let Some(on_mouse_enter) = &self.on_mouse_enter {
					on_mouse_enter();
				}
			} else {
				if let Some(on_mouse_leave) = &self.on_mouse_leave {
					on_mouse_leave();
				}
			}
		}
	}
}
impl Container {
	fn ensure_clickable(&mut self) {
		if self.clickable.is_none() {
			self.clickable = Some(Clickable::new());
		}
	}
	pub fn on_click(mut self, handler: impl Fn() + 'static) -> Self {
		self.ensure_clickable();
		self.clickable.as_mut().unwrap().on_click = Some(Box::new(handler));
		self
	}

	pub fn on_mouse_enter(mut self, handler: impl Fn() + 'static) -> Self {
		self.ensure_clickable();
		self.clickable.as_mut().unwrap().on_mouse_enter = Some(Box::new(handler));
		self
	}

	pub fn on_mouse_leave(mut self, handler: impl Fn() + 'static) -> Self {
		self.ensure_clickable();
		self.clickable.as_mut().unwrap().on_mouse_leave = Some(Box::new(handler));
		self
	}

	pub fn on_right_click(mut self, handler: impl Fn() + 'static) -> Self {
		self.ensure_clickable();
		self.clickable.as_mut().unwrap().on_right_click = Some(Box::new(handler));
		self
	}
	fn add_focus_node(mut self, skip: bool) -> Self {
		self.ensure_clickable();
		let clickable = self.clickable.as_mut().unwrap();
		if let Some(focus_node_id) = clickable.focus_node_id {
			GLOBAL_FOCUS_MANAGER.with_borrow_mut(|f| {
				f.set_node_skip(focus_node_id, skip);
			});
		} else {
			begin_component(format!("builtin/clickable/focus_node/{skip}"));
			let focus_node_id = *use_memo(Uuid::new_v4, ());

			GLOBAL_FOCUS_MANAGER.with_borrow_mut(|f| {
				f.add_node(focus_node_id, skip);
				f.set_parent(self.children.focus_nodes(), focus_node_id);
			});
			clickable.focus_node_id = Some(focus_node_id);
			end_component();
		}
		self
	}
	pub fn focusable(mut self) -> Self {
		self.add_focus_node(false)
	}
	pub fn focus_container(mut self) -> Self {
		self.add_focus_node(true)
	}
}
