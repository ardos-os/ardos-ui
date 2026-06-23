use std::{cell::RefCell, collections::HashMap};
use uuid::Uuid;

#[derive(Clone, Copy)]
enum Parent {
	Root,
	Parent(Uuid),
	Undefined,
}

#[derive(Clone, Copy)]
struct Node {
	parent: Parent,
	prev: Option<Uuid>,
	next: Option<Uuid>,
	skip: bool,
}

pub struct FocusManager {
	focus_nodes: HashMap<Uuid, Node>,
	current: Option<Uuid>,
	first: Option<Uuid>,
	last: Option<Uuid>,
}

impl FocusManager {
	pub(crate) fn new() -> Self {
		Self {
			focus_nodes: HashMap::new(),
			current: None,
			last: None,
			first: None,
		}
	}
	pub fn blur(&mut self) {
		self.current = None;
	}
	fn remove_dangling_nodes(&mut self) {
		if let Some(current) = self.current {
			if !self.focus_nodes.contains_key(&current) {
				self.current = None;
			} else if self.focus_nodes[&current].skip {
				self.focus_next();
			}
		}
	}

	pub(crate) fn new_frame(&mut self) {
		self.remove_dangling_nodes();

		self.first = None;
		self.last = None;
		self.focus_nodes.clear();
	}

	pub fn add_node(&mut self, id: Uuid, skip: bool) -> Uuid {
		let node_id = id;

		if let Some(node) = self.focus_nodes.get_mut(&node_id) {
			// já existe → apenas atualiza
			node.skip = skip;
		} else {
			// novo nó
			self.focus_nodes.insert(
				node_id,
				Node {
					parent: Parent::Undefined,
					prev: self.last,
					next: None,
					skip,
				},
			);
			if let Some(prev) = self.last {
				self.focus_nodes.get_mut(&prev).unwrap().next = Some(node_id);
			}
			if self.first.is_none() {
				self.first = Some(node_id);
			}
			self.last = Some(node_id);
		}

		node_id
	}

	pub fn set_node_skip(&mut self, id: Uuid, skip: bool) {
		if let Some(node) = self.focus_nodes.get_mut(&id) {
			node.skip = skip;
		}
	}

	pub fn set_parent(&mut self, children: impl IntoIterator<Item = Uuid>, parent: Uuid) -> Uuid {
		for child_id in children {
			if let Some(node) = self.focus_nodes.get_mut(&child_id) {
				if let Parent::Undefined = node.parent {
					node.parent = Parent::Parent(parent);
				}
			}
		}
		parent
	}

	pub(crate) fn add_root(&mut self) {
		for node in self.focus_nodes.values_mut() {
			if let Parent::Undefined = node.parent {
				node.parent = Parent::Root;
			}
		}
	}

	pub fn set_focus(&mut self, id: Uuid) {
		if self.focus_nodes.contains_key(&id) {
			self.current = Some(id);
		}
	}

	pub fn focus_next(&mut self) {
		let mut next = self
			.current
			.and_then(|cur| self.focus_nodes[&cur].next)
			.or(self.first);

		while let Some(id) = next {
			if let Some(node) = self.focus_nodes.get(&id) {
				if !node.skip {
					self.current = Some(id);
					return;
				}
				next = node.next.or(self.first); // wrap-around
				if Some(id) == self.first {
					break; // ciclo completo
				}
			} else {
				break;
			}
		}

		self.current = None;
	}

	pub fn focus_prev(&mut self) {
		let mut prev = self
			.current
			.and_then(|cur| self.focus_nodes[&cur].prev)
			.or(self.last);

		while let Some(id) = prev {
			if let Some(node) = self.focus_nodes.get(&id) {
				if !node.skip {
					self.current = Some(id);
					return;
				}
				prev = node.prev.or(self.last); // wrap-around
				if Some(id) == self.last {
					break; // ciclo completo
				}
			} else {
				break;
			}
		}

		self.current = None;
	}

	pub fn focused(&self) -> Option<Uuid> {
		self.current
	}

	pub fn has_focused_child(&self, parent_id: Uuid) -> bool {
		let Some(current) = self.current else {
			return false;
		};
		let mut cur = current;
		loop {
			if cur == parent_id {
				return true;
			}
			let Some(node) = self.focus_nodes.get(&cur) else {
				return false;
			};
			match node.parent {
				Parent::Parent(pid) => cur = pid,
				Parent::Root | Parent::Undefined => return false,
			}
		}
	}
}

thread_local! {
		pub static GLOBAL_FOCUS_MANAGER: RefCell<FocusManager> = RefCell::new(FocusManager::new());
}
