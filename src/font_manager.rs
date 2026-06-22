use super::clay_renderer::create_measure_text_function;
use clay_layout::Clay;
use skia_safe::{FontMgr, FontStyle, Typeface};
use std::{collections::HashMap, fs};

/// Cache key for font requests.
///
/// Important: this must be based on the *requested* family/style, not on the returned
/// `Typeface` properties, because font matching is fuzzy and the returned typeface
/// may not report the same family/style that was requested.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct FontKey {
	family: String,
	weight: i32,
	width: i32,
	slant: i32,
}

impl FontKey {
	fn new(family: &str, style: FontStyle) -> Self {
		// skia-safe returns strong types for weight/width/slant; convert to stable primitive keys.
		let weight: i32 = *style.weight();
		let width: i32 = *style.width();
		let slant: i32 = style.slant() as i32;

		Self {
			family: family.to_string(),
			weight,
			width,
			slant,
		}
	}
}

pub struct FontManager {
	fonts: Vec<Typeface>,
	updated_fonts: bool,
	font_mgr: FontMgr,

	/// Maps a requested (family, style) to an already-loaded font id.
	cache: HashMap<FontKey, u16>,
}

impl FontManager {
	pub fn new() -> Self {
		FontManager {
			fonts: Vec::new(),
			updated_fonts: true,
			font_mgr: FontMgr::new(),
			cache: HashMap::new(),
		}
	}

	/// Loads a font by family and style, appends it if not already present, and returns its numeric ID (0-based).
	pub fn get(&mut self, family: &str, style: FontStyle) -> u16 {
		let key = FontKey::new(family, style);

		// Cache hit by request key (fast path)
		if let Some(&id) = self.cache.get(&key) {
			return id;
		}

		if self.fonts.len() > u16::MAX as usize {
			panic!("Too many fonts loaded");
		}

		// Cache miss: resolve via font manager and append
		let typeface = self.resolve_typeface(family, style);

		self.fonts.push(typeface);
		self.updated_fonts = true;

		let id = (self.fonts.len() as u16) - 1;
		self.cache.insert(key, id);
		id
	}

	/// Returns a slice of all loaded fonts.
	pub fn get_fonts(&self) -> &[Typeface] {
		&self.fonts
	}

	/// Creates a clay measure function using the loaded fonts.
	pub fn update_clay_measure_function(&mut self, clay: &mut Clay) {
		if self.updated_fonts {
			let fonts = self.fonts.clone();
			clay.set_measure_text_function(create_measure_text_function(fonts));
			self.updated_fonts = false;
		}
	}

	fn resolve_typeface(&self, family: &str, style: FontStyle) -> Typeface {
		for family in [family, "sans-serif", "Roboto", ""] {
			if let Some(typeface) = self.font_mgr.match_family_style(family, style) {
				return typeface;
			}
		}

		for path in [
			"/system/fonts/Roboto-Regular.ttf",
			"/system/fonts/NotoSans-Regular.ttf",
			"/system/fonts/DroidSans.ttf",
		] {
			if let Ok(bytes) = fs::read(path) {
				if let Some(typeface) = self.font_mgr.new_from_data(&bytes, None) {
					return typeface;
				}
			}
		}

		panic!("Font '{}' with style {:?} not found", family, style);
	}
}
