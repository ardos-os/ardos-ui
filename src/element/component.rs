use std::any::{type_name, type_name_of_val};

use crate::{Element, RenderContext, begin_component, end_component};

// Function component wrapper
pub struct Component {
	pub child: Box<dyn Element>,
}

impl Component {
	/// Creates a new function component.
	///
	/// Wrapping a function component in a `Component` is NOT equivalent to just calling the function directly.
	/// This is because `Component` creates a new context for the component function, allowing hooks to be properly scoped and isolated from the parent component, otherwise, hooks in the component function will be executed on behalf of the parent component, causing undefined behavior.
	pub fn new<Props: Default>(func: impl FnOnce(Props) -> Box<dyn Element>, setup_props: impl FnOnce(&mut Props)) -> Self {
		let mut props = Props::default();
		setup_props(&mut props);
		Self {
			child: {
				begin_component(format!(
					"{}({})",
					type_name_of_val(&func),
					type_name::<Props>()
				));
				let element = (func)(props);
				end_component();
				element
			},
		}
	}
	/// Creates a new function component.
	///
	/// Wrapping a function component in a `Component` is NOT equivalent to just calling the function directly.
	/// This is because `Component` creates a new context for the component function, allowing hooks to be properly scoped and isolated from the parent component, otherwise, hooks in the component function will be executed on behalf of the parent component, causing undefined behavior.
	pub fn new_with_props<Props: Default>(func: impl FnOnce(Props) -> Box<dyn Element>, props: Props) -> Self {
		Self {
			child: {
				begin_component(format!(
					"{}({})",
					type_name_of_val(&func),
					type_name::<Props>()
				));
				let element = (func)(props);
				end_component();
				element
			},
		}
	}
	/// Creates a new function component with a key.
	pub fn new_with_key<Props>(
		func: impl FnOnce(Props) -> Box<dyn Element>,
		props: Props,
		key: String,
	) -> Self {
		Self {
			child: {
				begin_component(format!(
					"{}({}) key = {key}",
					type_name_of_val(&func),
					type_name::<Props>()
				));
				let element = (func)(props);
				end_component();
				element
			},
		}
	}
}
impl<F: FnOnce() -> Box<dyn Element>> From<F> for Component {
	fn from(value: F) -> Self {
		Self::new(|()| value(), |_| {})
	}
}
impl Element for Component {
	fn render<'clay: 'render, 'render>(&'render self, ctx: &mut RenderContext<'clay, 'render, '_>) {
		self.child.render(ctx);
	}
}