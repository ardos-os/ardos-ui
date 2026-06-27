use rlay::{Size, TextStyle};
use skia_safe::{FontMgr, FontStyle, Typeface};
use std::{
	cell::RefCell,
	collections::HashMap,
	fs,
	hash::{Hash, Hasher},
	path::PathBuf,
	rc::Rc,
};

#[derive(Debug, Clone)]
pub enum FontFamily {
	System(String),
	Path(PathBuf),
	StaticBytes(&'static [u8]),
}

impl PartialEq for FontFamily {
	fn eq(&self, other: &Self) -> bool {
		match (self, other) {
			(Self::System(a), Self::System(b)) => a == b,
			(Self::Path(a), Self::Path(b)) => a == b,
			(Self::StaticBytes(a), Self::StaticBytes(b)) => {
				a.as_ptr() == b.as_ptr() && a.len() == b.len()
			}
			_ => false,
		}
	}
}

impl Eq for FontFamily {}

impl Hash for FontFamily {
	fn hash<H: Hasher>(&self, state: &mut H) {
		match self {
			Self::System(family) => {
				0u8.hash(state);
				family.hash(state);
			}
			Self::Path(path) => {
				1u8.hash(state);
				path.hash(state);
			}
			// Optimization: instead of hashing the entire contents of the font, which would be a performance bottleneck
			// We just hash the pointer, because we know it is static and it won't change through out the program
			// So if we have the same pointer, it's always the same content
			Self::StaticBytes(bytes) => {
				2u8.hash(state);
				bytes.as_ptr().hash(state);
				bytes.len().hash(state);
			}
		}
	}
}

/// Cache key for font requests.
///
/// Important: this must be based on the *requested* family/style, not on the returned
/// `Typeface` properties, because font matching is fuzzy and the returned typeface
/// may not report the same family/style that was requested.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct FontKey {
	family: FontFamily,
	weight: i32,
	width: i32,
	slant: i32,
}

impl FontKey {
	fn new(family: FontFamily, style: FontStyle) -> Self {
		// skia-safe returns strong types for weight/width/slant; convert to stable primitive keys.
		let weight: i32 = *style.weight();
		let width: i32 = *style.width();
		let slant: i32 = style.slant() as i32;

		Self {
			family: family,
			weight,
			width,
			slant,
		}
	}
}

pub struct FontManager {
	fonts: Rc<RefCell<Vec<Typeface>>>,
	font_mgr: FontMgr,

	/// Maps a requested (family, style) to an already-loaded font id.
	cache: HashMap<FontKey, u16>,
}

impl FontManager {
	pub fn new() -> Self {
		FontManager {
			fonts: Rc::new(RefCell::new(Vec::new())),
			font_mgr: FontMgr::new(),
			cache: HashMap::new(),
		}
	}

	pub fn measure_handle(&self) -> Rc<RefCell<Vec<Typeface>>> {
		Rc::clone(&self.fonts)
	}

	/// Loads a font by family and style, appends it if not already present, and returns its numeric ID (0-based).
	pub fn get(&mut self, family: FontFamily, style: FontStyle) -> u16 {
		let key = FontKey::new(family.clone(), style);

		// Cache hit by request key (fast path)
		if let Some(&id) = self.cache.get(&key) {
			return id;
		}

		if self.fonts.borrow().len() > u16::MAX as usize {
			panic!("Too many fonts loaded");
		}

		// Cache miss: resolve via font manager and append
		let typeface = self.resolve_typeface(family, style);

		self.fonts.borrow_mut().push(typeface);

		let id = (self.fonts.borrow().len() as u16) - 1;
		self.cache.insert(key, id);
		id
	}

	/// Returns a slice of all loaded fonts.
	pub fn get_fonts(&self) -> Rc<RefCell<Vec<Typeface>>> {
		Rc::clone(&self.fonts)
	}

	fn resolve_typeface(&self, family: FontFamily, style: FontStyle) -> Typeface {
		match &family {
			FontFamily::System(family) => {
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
			}
			FontFamily::Path(path_buf) => {
				if let Ok(bytes) = fs::read(path_buf) {
					if let Some(typeface) = self.font_mgr.new_from_data(&bytes, None) {
						return typeface;
					}
				}
			}
			FontFamily::StaticBytes(bytes) => {
				if let Some(typeface) = self.font_mgr.new_from_data(&bytes, None) {
					return typeface;
				}
			}
		}
		panic!("Font '{:?}' with style {:?} not found", family, style);
	}
}

pub fn measure_text(fonts: &Rc<RefCell<Vec<Typeface>>>, text: &str, style: &TextStyle) -> Size {
	let fonts = fonts.borrow();
	let Some(typeface) = fonts.get(style.font_id as usize) else {
		return Size::new(0.0, style.font_size);
	};
	let font = skia_safe::Font::new(typeface, style.font_size);
	let width = font.measure_str(text, None).0;
	Size::new(width, font.metrics().1.bottom - font.metrics().1.top)
}
