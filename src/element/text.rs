use std::path::PathBuf;

use skia_safe::{FontStyle, font_style::Width};

use crate::{Element, RenderContext, font_manager::FontFamily};
pub use rlay::TextAlign as TextAlignment;



pub struct Text {
	pub text: String,
	pub font_family: FontFamily,
	pub font_weight: i32,
	pub italic: bool,
	pub font_size: u16,
	pub color: rlay::Color,
	pub alignment: TextAlignment,
}

impl Text {
	pub fn new(text: impl Into<String>) -> Self {
		Self {
			text: text.into(),
			font_family: FontFamily::System("sans-serif".to_string()),
			font_weight: 400,
			font_size: 14,
			color: (0, 0, 0, 255).into(),
			italic: false,
			alignment: TextAlignment::Left,
		}
	}

	pub fn font_weight(mut self, font_weight: i32) -> Self {
		self.font_weight = font_weight;
		self
	}
	pub fn text_center(mut self) -> Self {
		self.alignment = TextAlignment::Center;
		self
	}
	pub fn text_right(mut self) -> Self {
		self.alignment = TextAlignment::Right;
		self
	}
	pub fn text_left(mut self) -> Self {
		self.alignment = TextAlignment::Left;
		self
	}
	pub fn font_size(mut self, size: u16) -> Self {
		self.font_size = size;
		self
	}

	pub fn color(mut self, color: impl Into<rlay::Color>) -> Self {
		self.color = color.into();
		self
	}

	pub fn italic(mut self, italic: bool) -> Self {
		self.italic = italic;
		self
	}

	pub fn font_family(mut self, family: impl Into<String>) -> Self {
		self.font_family = FontFamily::System(family.into());
		self
	}
	pub fn font_path(mut self, family: impl Into<PathBuf>) -> Self {
		self.font_family = FontFamily::Path(family.into());
		self
	}
	pub fn font_bytes(mut self, family: &'static [u8]) -> Self {
		self.font_family = FontFamily::StaticBytes(family);
		self
	}
}

impl Element for Text {
	fn render<'clay: 'render, 'render>(&'render self, ctx: &mut RenderContext<'clay, 'render, '_>) {
		let skia_font_style = FontStyle::new(
			self.font_weight.into(),
			Width::NORMAL,
			if self.italic {
				skia_safe::font_style::Slant::Italic
			} else {
				skia_safe::font_style::Slant::Upright
			},
		);
		ctx.frame.text(
			&self.text,
			rlay::TextStyle {
				font_size: self.font_size as f32,
				color: self.color,
				align: self.alignment,
				font_id: ctx.font_manager.get(self.font_family.clone(), skia_font_style),
				..Default::default()
			},
		);
	}
}
