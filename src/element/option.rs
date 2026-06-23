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
}
