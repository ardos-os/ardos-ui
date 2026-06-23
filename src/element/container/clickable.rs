use std::{collections::HashSet, rc::Rc};

use rlay::{MouseButton, PointerGesture, PointerHit, PointerId, PointerPhase};
use uuid::Uuid;

use crate::{
	Container, Element, begin_component, end_component, focus_system::GLOBAL_FOCUS_MANAGER, use_memo,
};

#[derive(Clone)]
pub struct ClickableState {
	pub hovered: bool,
	pub pressed: bool,
	pub down: bool,
	pub right_down: bool,
	pub right_pressed: bool,
	pub focus_node_id: Option<Uuid>,
	pub stable_id: Uuid,
	pub touch_down_inside: HashSet<PointerId>,
}

impl Default for ClickableState {
	fn default() -> Self {
		Self {
			hovered: false,
			pressed: false,
			down: false,
			right_down: false,
			right_pressed: false,
			focus_node_id: None,
			stable_id: Uuid::new_v4(),
			touch_down_inside: HashSet::new(),
		}
	}
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
	pub(crate) on_click: Option<Rc<dyn Fn()>>,
	pub(crate) on_mouse_enter: Option<Rc<dyn Fn()>>,
	pub(crate) on_mouse_leave: Option<Rc<dyn Fn()>>,
	pub(crate) on_right_click: Option<Rc<dyn Fn()>>,
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
		element_id: &str,
		state: &mut ClickableState,
		pointers: &[PointerHit],
		enter_pressed: bool,
		enter_down: bool,
		context_menu_pressed: bool,
		context_menu_down: bool,
	) {
		state.focus_node_id = self.focus_node_id;

		let over = |pointer: &PointerHit| pointer.element_id.as_deref() == Some(element_id);
		let is_hovered = pointers
			.iter()
			.any(|pointer| pointer.pointer_id == PointerId::Mouse && over(pointer));
		let pointer_down = pointers.iter().any(|pointer| {
			over(pointer)
				&& pointer.phase.is_down()
				&& (pointer.pointer_id == PointerId::Mouse || pointer.gesture == PointerGesture::Tap)
		});
		let mouse_clicked = pointers.iter().any(|pointer| {
			over(pointer)
				&& pointer.pointer_id == PointerId::Mouse
				&& pointer.mouse_button == Some(MouseButton::Left)
				&& pointer.phase == PointerPhase::PressedThisFrame
		});
		let touch_clicked = pointers.iter().any(|pointer| {
			state.touch_down_inside.contains(&pointer.pointer_id)
				&& matches!(pointer.pointer_id, PointerId::Touch(_))
				&& pointer.gesture == PointerGesture::Tap
				&& pointer.phase == PointerPhase::ReleasedThisFrame
		});
		let touch_down_inside = pointers
			.iter()
			.filter(|pointer| {
				over(pointer)
					&& matches!(pointer.pointer_id, PointerId::Touch(_))
					&& pointer.gesture == PointerGesture::Tap
					&& pointer.phase.is_down()
			})
			.map(|pointer| pointer.pointer_id)
			.collect::<HashSet<_>>();
		let touch_clicked =
			touch_clicked && !state.touch_down_inside.is_empty() && touch_down_inside.is_empty();
		let keyboard_clicked = enter_pressed && state.is_focused();
		let is_clicked = mouse_clicked || touch_clicked || keyboard_clicked;

		state.down = pointer_down || (enter_down && state.is_focused());
		state.touch_down_inside = touch_down_inside;
		state.right_down = pointers.iter().any(|pointer| {
			over(pointer)
				&& pointer.pointer_id == PointerId::Mouse
				&& pointer.mouse_button == Some(MouseButton::Right)
				&& pointer.phase.is_down()
		}) || (context_menu_down && state.is_focused());
		state.pressed = is_clicked;

		if is_clicked {
			state.set_focus();
			if let Some(on_click) = &self.on_click {
				on_click();
			}
		}

		let is_right_clicked = pointers.iter().any(|pointer| {
			over(pointer)
				&& pointer.pointer_id == PointerId::Mouse
				&& pointer.mouse_button == Some(MouseButton::Right)
				&& pointer.phase == PointerPhase::PressedThisFrame
		}) || (context_menu_pressed && state.is_focused());
		state.right_pressed = is_right_clicked;

		if is_right_clicked {
			state.set_focus();
			if let Some(on_right_click) = &self.on_right_click {
				on_right_click();
			}
		}

		if is_hovered != state.hovered {
			state.hovered = is_hovered;
			if is_hovered {
				if let Some(on_mouse_enter) = &self.on_mouse_enter {
					on_mouse_enter();
				}
			} else if let Some(on_mouse_leave) = &self.on_mouse_leave {
				on_mouse_leave();
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use rlay::{Point, PointerGesture, PointerHit, PointerId, PointerPhase};

	use super::{Clickable, ClickableState};

	#[test]
	fn touch_scroll_gesture_does_not_press_or_hover_clickable() {
		let clickable = Clickable::new();
		let mut state = ClickableState::default();
		let pointers = [PointerHit {
			pointer_id: PointerId::Touch(1),
			position: Point::new(10.0, 10.0),
			phase: PointerPhase::Pressed,
			element_id: Some("button".into()),
			mouse_button: None,
			gesture: PointerGesture::Scroll,
		}];

		clickable.update("button", &mut state, &pointers, false, false, false, false);

		assert!(!state.hovered);
		assert!(!state.down);
		assert!(state.touch_down_inside.is_empty());
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
		self.clickable.as_mut().unwrap().on_click = Some(Rc::new(handler));
		self
	}

	pub fn on_mouse_enter(mut self, handler: impl Fn() + 'static) -> Self {
		self.ensure_clickable();
		self.clickable.as_mut().unwrap().on_mouse_enter = Some(Rc::new(handler));
		self
	}

	pub fn on_mouse_leave(mut self, handler: impl Fn() + 'static) -> Self {
		self.ensure_clickable();
		self.clickable.as_mut().unwrap().on_mouse_leave = Some(Rc::new(handler));
		self
	}

	pub fn on_right_click(mut self, handler: impl Fn() + 'static) -> Self {
		self.ensure_clickable();
		self.clickable.as_mut().unwrap().on_right_click = Some(Rc::new(handler));
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

	pub fn focusable(self) -> Self {
		self.add_focus_node(false)
	}

	pub fn focus_container(self) -> Self {
		self.add_focus_node(true)
	}
}
