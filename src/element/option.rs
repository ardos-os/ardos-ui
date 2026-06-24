use std::collections::HashSet;

use uuid::Uuid;

use crate::Element;

impl<T: Element> Element for Option<T> {
	fn render<'clay: 'render, 'render>(
		&'render self,
		ctx: &mut crate::RenderContext<'clay, 'render, '_>,
	) {
		match self {
			Some(e) => e.render(ctx),
			None => {}
		}
	}
	
	fn focus_nodes(&self) -> HashSet<Uuid> {
		match self {
			Some(e) => e.focus_nodes(),
			None => HashSet::new()
		}
	}
}
