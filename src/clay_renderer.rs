use rlay::{
    Border, Color as RlayColor, CommandKind, Padding, Radius, Rect as RlayRect, RenderCommand,
    TextOverflowMode,
};
use skia_safe::{
	Canvas, ClipOp, Color4f, FilterMode, Font, MipmapMode, Paint, PathBuilder, Point, RRect, Rect,
	SamplingOptions, Typeface,
};

use crate::image::{ImageManager, ResolvedImage};

pub(crate) fn rlay_to_skia_color(color: RlayColor) -> Color4f {
	Color4f::new(
		color.r / 255.,
		color.g / 255.,
		color.b / 255.,
		color.a / 255.,
	)
}

pub(crate) fn rlay_to_skia_rect(rect: RlayRect) -> Rect {
	Rect::from_xywh(rect.x, rect.y, rect.width, rect.height)
}

pub(crate) fn rlay_to_skia_rrect(rect: RlayRect, radius: Radius) -> RRect {
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

fn text_baseline(bounds_y: f32, bounds_height: f32, metrics_top: f32, metrics_bottom: f32) -> f32 {
	let glyph_height = metrics_bottom - metrics_top;
	bounds_y + (bounds_height - glyph_height) / 2.0 - metrics_top
}

fn image_sampling() -> SamplingOptions {
	SamplingOptions::new(FilterMode::Linear, MipmapMode::Linear)
}

pub(crate) fn rlay_skia_render(
	canvas: &Canvas,
	render_commands: impl Iterator<Item = RenderCommand>,
	mut render_custom_element: impl FnMut(&RenderCommand, u64, Radius, &Canvas),
	fonts: &[Typeface],
	image_manager: &ImageManager,
) {
	for command in render_commands {
		match &command.kind {
				CommandKind::Text { text, style } => {
					let Some(typeface) = fonts.get(style.font_id as usize) else {
						continue;
					};
					let clip_text = style.text_overflow == TextOverflowMode::Cut;
					if clip_text {
						canvas.save();
						canvas.clip_rect(rlay_to_skia_rect(command.bounds), ClipOp::Intersect, true);
					}
					let mut paint = Paint::default();
					paint.set_color4f(rlay_to_skia_color(style.color), None);
					let font = Font::new(typeface, style.font_size);
					let metrics = font.metrics().1;
				let pos = Point::new(
					command.bounds.x,
					text_baseline(
						command.bounds.y,
						command.bounds.height,
						metrics.top,
						metrics.bottom,
						),
					);
					canvas.draw_str(text, pos, &font, &paint);
					if clip_text {
						canvas.restore();
					}
				}
			CommandKind::Image(image_data) => {
				let Some(image) = image_manager.resolve_id(image_data.image_id.get()) else {
					continue;
				};
				let bounds = rlay_to_skia_rect(command.bounds);

				canvas.save();
				if image_data.corner_radius == Radius::default() {
					canvas.clip_rect(bounds, ClipOp::Intersect, true);
				} else {
					canvas.clip_rrect(
						rlay_to_skia_rrect(command.bounds, image_data.corner_radius),
						ClipOp::Intersect,
						true,
					);
				}
				match image {
					ResolvedImage::Raster(image) => {
						let mut paint = Paint::default();
						paint.set_anti_alias(true);
						canvas.draw_image_rect_with_sampling_options(
							image,
							None,
							bounds,
							image_sampling(),
							&paint,
						);
					}
					ResolvedImage::Svg(dom) => {
						let intrinsic = dom.root().intrinsic_size();
						let width = intrinsic.width.max(1.0);
						let height = intrinsic.height.max(1.0);
						canvas.translate((bounds.left, bounds.top));
						canvas.scale((bounds.width() / width, bounds.height() / height));
						dom.render(canvas);
					}
				}
				canvas.restore();
			}
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

#[cfg(test)]
mod tests {
	use skia_safe::{FilterMode, MipmapMode};

	use super::{image_sampling, text_baseline};

	#[test]
	fn baseline_centers_font_metrics_inside_line_box() {
		let baseline = text_baseline(10.0, 20.0, -12.0, 4.0);

		assert!((baseline - 24.0).abs() <= f32::EPSILON);
		assert!((baseline - 12.0 - 12.0).abs() <= f32::EPSILON);
		assert!((baseline + 4.0 - 28.0).abs() <= f32::EPSILON);
	}

	#[test]
	fn images_use_linear_filtering_and_mipmaps() {
		let sampling = image_sampling();

		assert_eq!(sampling.filter, FilterMode::Linear);
		assert_eq!(sampling.mipmap, MipmapMode::Linear);
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
		let mut path = PathBuilder::new();
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
		canvas.clip_path(&path.detach(), ClipOp::Intersect, false);

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
