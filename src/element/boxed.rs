use crate::Element;

impl<T: Element> Element for Box<T> {
	fn render<'clay: 'render, 'render>(
		&'render self,
		ctx: &mut crate::RenderContext<'clay, 'render, '_>,
	) {
		self.as_ref().render(ctx);
	}
	fn focus_nodes(&self) -> std::collections::HashSet<uuid::Uuid> {
		self.as_ref().focus_nodes()
	}
}
