use std::any::Any;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::Deref;
use std::rc::Rc;

use crate::GlobalClosure;

thread_local! {
	pub(crate) static HOOK_PATH: RefCell<Vec<(usize, String)>> = RefCell::new(Vec::new());
	pub(crate) static HOOK_INDEX: RefCell<usize> = RefCell::new(0);
	pub(crate) static HOOK_STATES: RefCell<HashMap<HookKey, Box<dyn Any>>> = RefCell::new(HashMap::new());
	pub(crate) static HOOK_VISITED_STATES: RefCell<HashSet<HookKey>> = RefCell::new(HashSet::new());
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct HookKey {
	path: Vec<(usize, String)>,
	hook_index: usize,
}

/// Must be called at the start of every component render.
/// This sets up the internal path and hook index for the current component.
/// Should be paired with [`end_component`] at the end of the component render.
pub fn begin_component(key: impl Into<String>) {
	let key = key.into();
	HOOK_PATH.with(move |path| {
		let mut path = path.borrow_mut();
		if let Some(last) = path.last_mut() {
			last.0 += 1;
		}
		path.push((0, key));
	});
	HOOK_INDEX.with(|idx| *idx.borrow_mut() = 0);
}

/// Must be called at the end of every component render.
/// This pops the current component from the internal path stack.
/// Should be paired with [`begin_component`] at the start of the component render.
pub fn end_component() {
	HOOK_PATH.with(|path| {
		path.borrow_mut().pop();
		if path.borrow().is_empty() {
			// Garbage collect states that were not visited this frame
			HOOK_STATES.with(|states| {
				HOOK_VISITED_STATES.with(|visited| {
					let mut states = states.borrow_mut();
					let visited = visited.borrow();
					states.retain(|k, _| visited.contains(k));
				});
			});
			HOOK_VISITED_STATES.with(|visited| visited.borrow_mut().clear());
		}
	});
}

#[derive(Clone)]
pub struct StateSetter<T>(Rc<dyn Fn(T)>);

impl<T> StateSetter<T> {
	#[inline]
	pub fn new(f: Rc<dyn Fn(T)>) -> Self {
		Self(f)
	}

	#[inline]
	pub fn set(&self, value: T) {
		(self.0)(value)
	}
}

impl<T> Deref for StateSetter<T> {
	type Target = dyn Fn(T);

	#[inline]
	fn deref(&self) -> &Self::Target {
		&*self.0
	}
}

pub type State<T> = (T, StateSetter<T>);

pub type Entity<T> = (Rc<RefCell<T>>, Box<dyn Fn(&dyn Fn(&mut T))>);
/// React-style state hook for persistent, reactive state in a component.
///
/// The state is stable for each unique component position and hook call order.
/// When the setter is called, the window is scheduled for re-render.
///
/// # Example
/// ```rust,no_run
/// # use hyprui::use_state;
/// let (count, set_count) = use_state(0);
/// set_count(count + 1);
/// ```
pub fn use_state<T: Clone + 'static>(initial: T) -> State<T> {
	let path = HOOK_PATH.with(|p| p.borrow().clone());
	let idx = HOOK_INDEX.with(|i| {
		let v = *i.borrow();
		*i.borrow_mut() += 1;
		v
	});
	let key = HookKey {
		path,
		hook_index: idx,
	};
	HOOK_VISITED_STATES.with(|visited| {
		visited.borrow_mut().insert(key.clone());
	});
	let current_value = HOOK_STATES.with(|states| {
		let mut states = states.borrow_mut();

		states
			.entry(key.clone())
			.or_insert_with(|| Box::new(initial.clone()))
			.downcast_ref::<T>()
			.unwrap()
			.clone()
	});

	let key_for_setter = key.clone();
	let setter_rc: Rc<dyn Fn(T)> = Rc::new(move |new_value: T| {
		HOOK_STATES.with(|states| {
			let mut states = states.borrow_mut();
			states.insert(key_for_setter.clone(), Box::new(new_value));
		});

		crate::REQUEST_REDRAW.call();
	});

	(current_value, StateSetter::new(setter_rc))
}

pub fn use_entity<T: 'static>(initial: impl FnOnce() -> T) -> Entity<T> {
	let value = use_memo(|| RefCell::new(initial()), ());
	let setter_rc = value.clone();
	let setter = move |updater: &dyn Fn(&mut T)| {
		let mut entity = setter_rc.borrow_mut();
		updater(&mut entity);
		crate::REQUEST_REDRAW.call();
	};
	(value, Box::new(setter))
}

/// Runs side effects when the `deps` hash changes
pub fn use_effect<D, F>(effect: F, deps: &D)
where
	D: Hash + 'static,
	F: FnOnce() + 'static,
{
	let hash = {
		let mut hasher = DefaultHasher::new();
		deps.hash(&mut hasher);
		hasher.finish()
	};

	let (last_hash, set_last_hash) = crate::hooks::use_state(None);

	if last_hash != Some(hash) {
		effect();
		set_last_hash(Some(hash));
	}
}

pub fn use_ref<T: 'static>(initial: T) -> Rc<RefCell<T>> {
	let path = HOOK_PATH.with(|p| p.borrow().clone());
	let idx = HOOK_INDEX.with(|i| {
		let v = *i.borrow();
		*i.borrow_mut() += 1;
		v
	});
	let key = HookKey {
		path,
		hook_index: idx,
	};

	HOOK_VISITED_STATES.with(|visited| {
		visited.borrow_mut().insert(key.clone());
	});
	HOOK_STATES.with(|states| {
		let mut states = states.borrow_mut();
		let entry = states
			.entry(key.clone())
			.or_insert_with(|| Box::new(Rc::new(RefCell::new(initial))));
		entry.downcast_ref::<Rc<RefCell<T>>>().unwrap().clone()
	})
}

/// See useMemo from react: https://react.dev/reference/react/useMemo
pub fn use_memo<T, D, F>(f: F, deps: D) -> Rc<T>
where
	T: 'static,
	D: Hash + 'static,
	F: FnOnce() -> T,
{
	let hash = {
		let mut hasher = DefaultHasher::new();
		deps.hash(&mut hasher);
		hasher.finish()
	};

	let memoized_value = use_ref::<Option<(u64, Rc<T>)>>(None);

	if memoized_value.borrow().is_none() || memoized_value.borrow().as_ref().unwrap().0 != hash {
		let value = f();
		*memoized_value.borrow_mut() = Some((hash, Rc::new(value)));
	}
	memoized_value.borrow().as_ref().unwrap().1.clone()
}
#[cfg(test)]
mod tests {
	use super::*;

	fn reset_all() {
		HOOK_PATH.with(|p| p.borrow_mut().clear());
		HOOK_INDEX.with(|i| *i.borrow_mut() = 0);
		HOOK_STATES.with(|s| s.borrow_mut().clear());
	}
	mod use_state {
		use super::*;

		#[test]
		fn test_use_state_persists_between_frames() {
			reset_all();

			begin_component("component-a");
			let (v1, set_v1) = use_state(10);
			end_component();
			assert_eq!(v1, 10);

			set_v1(42);

			begin_component("component-a");
			let (v2, _set_v2) = use_state(10);
			end_component();
			assert_eq!(v2, 42);
		}

		#[test]
		fn test_multiple_components_and_hooks() {
			reset_all();
			// Component Root
			begin_component("root");
			// Component 1
			begin_component("component-1");
			let (a, set_a) = use_state(1);
			let (b, set_b) = use_state(2);
			end_component();

			// Component 2
			begin_component("component-2");
			let (c, set_c) = use_state(3);
			end_component();
			end_component();

			assert_eq!(a, 1);
			assert_eq!(b, 2);
			assert_eq!(c, 3);

			set_a(10);
			set_b(20);
			set_c(30);

			// Next frame
			// Component Root
			begin_component("root");
			// Component 1
			begin_component("component-1");
			let (a2, b2) = (use_state(1).0, use_state(2).0);
			end_component();
			// Component 2
			begin_component("component-2");
			let c2 = use_state(3).0;
			end_component();
			end_component();

			assert_eq!(a2, 10);
			assert_eq!(b2, 20);
			assert_eq!(c2, 30);
		}

		#[test]
		fn test_state_is_isolated_between_components() {
			reset_all();
			// Component Root
			begin_component("root");
			// Component 1
			begin_component("component-1");
			let (_a, set_a) = use_state(100);
			end_component();

			// Component 2
			begin_component("component-2");
			let (_b, set_b) = use_state(200);
			end_component();
			end_component();

			set_a(111);
			set_b(222);

			// Next frame
			// Component Root
			begin_component("root");
			// Component 1
			begin_component("component-1");
			let a2 = use_state(100).0;
			end_component();
			// Component 2
			begin_component("component-2");
			let b2 = use_state(200).0;
			end_component();
			end_component();

			assert_eq!(a2, 111);
			assert_eq!(b2, 222);
		}
	}
}
