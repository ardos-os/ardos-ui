use rlay::{
	Border, Color as RlayColor, CommandKind, Padding, Radius, Rect as RlayRect, RenderCommand,
};
use skia_safe::{Canvas, ClipOp, Color4f, Font, Paint, Path, Point, RRect, Rect, Typeface};

pub fn rlay_to_skia_color(color: RlayColor) -> Color4f {
	Color4f::new(
		color.r / 255.,
		color.g / 255.,
		color.b / 255.,
		color.a / 255.,
	)
}

pub fn rlay_to_skia_rect(rect: RlayRect) -> Rect {
	Rect::from_xywh(rect.x, rect.y, rect.width, rect.height)
}

pub fn rlay_to_skia_rrect(rect: RlayRect, radius: Radius) -> RRect {
	RRect::new_rect_radii(
		rlay_to_skia_rect(rect),
		&[
			Point::new(radius.top_left, radius.top_left),
			Point::new(radius.top_right, radius.top_right),
			Point::new(radius.bottom_left, radius.bottom_left),
			Point::new(radius.bottom_right, radius.bottom_right),
		],
	)
}

pub fn rlay_skia_render(
	canvas: &Canvas,
	render_commands: impl Iterator<Item = RenderCommand>,
	mut render_custom_element: impl FnMut(&RenderCommand, u64, Radius, &Canvas),
	fonts: &[Typeface],
) {
	for command in render_commands {
		match &command.kind {
			CommandKind::Text { text, style } => {
				let Some(typeface) = fonts.get(style.font_id as usize) else {
					continue;
				};
				let mut paint = Paint::default();
				paint.set_color4f(rlay_to_skia_color(style.color), None);
				let font = Font::new(typeface, style.font_size);
				let metrics = font.metrics().1;
				let text_height = metrics.bottom - metrics.top;
				let pos = Point::new(
					command.bounds.x,
					command.bounds.y + (command.bounds.height - text_height) / 2.0 - metrics.top,
				);
				canvas.draw_str(text, pos, &font, &paint);
			}
			CommandKind::Image(_) => {}
			CommandKind::ClipStart { .. } => {
				canvas.save();
				canvas.clip_rect(rlay_to_skia_rect(command.bounds), ClipOp::Intersect, true);
			}
			CommandKind::ClipEnd => {
				canvas.restore();
			}
			CommandKind::Rectangle { color, radius } => {
				let mut paint = Paint::default();
				paint.set_color4f(rlay_to_skia_color(*color), None);
				paint.set_anti_alias(true);
				paint.set_style(skia_safe::PaintStyle::Fill);
				if *radius == Radius::default() {
					canvas.draw_rect(rlay_to_skia_rect(command.bounds), &paint);
				} else {
					canvas.draw_rrect(rlay_to_skia_rrect(command.bounds, *radius), &paint);
				}
			}
			CommandKind::Border(border) => draw_border(canvas, command.bounds, *border),
			CommandKind::Custom(value, radius) => {
				render_custom_element(&command, *value, *radius, canvas)
			}
			CommandKind::OverlayStart(color) => {
				let mut paint = Paint::default();
				paint.set_color4f(rlay_to_skia_color(*color), None);
				canvas.draw_rect(rlay_to_skia_rect(command.bounds), &paint);
			}
			CommandKind::OverlayEnd | CommandKind::DebugOverlay { .. } => {}
		}
	}
}

fn draw_border(canvas: &Canvas, bounds: RlayRect, border: Border) {
	fn draw_side_border_rrect(
		canvas: &Canvas,
		bounds: Rect,
		rrect: &RRect,
		center: Point,
		side: usize,
		stroke_width: f32,
		color: Color4f,
		width: Padding,
	) {
		let mut path = Path::new();
		match side {
			0 => {
				path.move_to(center);
				path.line_to(Point::new(bounds.left, bounds.top));
				path.line_to(Point::new(bounds.left, bounds.bottom));
				path.close();
			}
			1 => {
				path.move_to(center);
				path.line_to(Point::new(bounds.left, bounds.top));
				path.line_to(Point::new(bounds.right, bounds.top));
				path.close();
			}
			2 => {
				path.move_to(center);
				path.line_to(Point::new(bounds.right, bounds.top));
				path.line_to(Point::new(bounds.right, bounds.bottom));
				path.close();
			}
			3 => {
				path.move_to(center);
				path.line_to(Point::new(bounds.left, bounds.bottom));
				path.line_to(Point::new(bounds.right, bounds.bottom));
				path.close();
			}
			_ => {}
		}
		canvas.save();
		canvas.clip_path(&path, ClipOp::Intersect, false);

		let mut paint = Paint::default();
		paint.set_color4f(color, None);
		paint.set_anti_alias(true);
		paint.set_style(skia_safe::PaintStyle::Stroke);
		paint.set_stroke_width(stroke_width);
		let rrect = RRect::new_rect_radii(
			Rect::from_ltrb(
				rrect.rect().left + width.left / 2.0,
				rrect.rect().top + width.top / 2.0,
				rrect.rect().right - width.right / 2.0,
				rrect.rect().bottom - width.bottom / 2.0,
			),
			rrect.radii_ref(),
		);
		canvas.draw_rrect(rrect, &paint);
		canvas.restore();
	}

	let bounds = rlay_to_skia_rect(bounds);

	let rrect = RRect::new_rect_radii(
		bounds,
		&[
			Point::new(border.radius.top_left, border.radius.top_left),
			Point::new(border.radius.top_right, border.radius.top_right),
			Point::new(border.radius.bottom_right, border.radius.bottom_right),
			Point::new(border.radius.bottom_left, border.radius.bottom_left),
		],
	);
	let center = Point::new(
		bounds.left + bounds.width() / 2.0,
		bounds.top + bounds.height() / 2.0,
	);
	let color = rlay_to_skia_color(border.color);
	let widths = [
		border.width.left,
		border.width.top,
		border.width.right,
		border.width.bottom,
	];

	for (side, width) in widths.into_iter().enumerate() {
		if width > 0.0 {
			draw_side_border_rrect(
				canvas,
				bounds,
				&rrect,
				center,
				side,
				width,
				color,
				border.width,
			);
		}
	}
}
