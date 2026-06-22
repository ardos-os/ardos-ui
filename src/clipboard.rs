use std::{cell::RefCell, rc::Rc};

#[cfg(all(unix, not(target_os = "android")))]
use std::ffi::c_void;

thread_local! {
	static CURRENT_CLIPBOARD: RefCell<Option<ClipboardHandle>> = RefCell::new(None);
}

pub trait Clipboard {
	fn get_text(&self) -> Option<String>;
	fn set_text(&self, text: &str);
}

pub type ClipboardHandle = Rc<dyn Clipboard>;

pub struct ClipboardScope {
	previous: Option<ClipboardHandle>,
}

impl Drop for ClipboardScope {
	fn drop(&mut self) {
		CURRENT_CLIPBOARD.with(|current| {
			current.replace(self.previous.take());
		});
	}
}

pub(crate) fn push_clipboard(clipboard: ClipboardHandle) -> ClipboardScope {
	CURRENT_CLIPBOARD.with(|current| {
		let previous = current.replace(Some(clipboard));
		ClipboardScope { previous }
	})
}

pub fn use_clipboard() -> ClipboardHandle {
	CURRENT_CLIPBOARD.with(|current| {
		current
			.borrow()
			.as_ref()
			.cloned()
			.unwrap_or_else(default_clipboard)
	})
}

#[cfg(target_os = "android")]
fn default_clipboard() -> ClipboardHandle {
	Rc::new(AndroidClipboard)
}

#[cfg(not(target_os = "android"))]
fn default_clipboard() -> ClipboardHandle {
	Rc::new(NoopClipboard)
}

struct NoopClipboard;

impl Clipboard for NoopClipboard {
	fn get_text(&self) -> Option<String> {
		None
	}

	fn set_text(&self, _text: &str) {}
}

#[cfg(target_os = "android")]
struct AndroidClipboard;

#[cfg(target_os = "android")]
impl Clipboard for AndroidClipboard {
	fn get_text(&self) -> Option<String> {
		android_clipboard::get_text().ok()
	}

	fn set_text(&self, text: &str) {
		let _ = android_clipboard::set_text(text.to_string());
	}
}

#[cfg(all(unix, not(target_os = "android")))]
pub(crate) struct WaylandClipboard {
	inner: smithay_clipboard::Clipboard,
}

#[cfg(all(unix, not(target_os = "android")))]
impl WaylandClipboard {
	/// # Safety
	///
	/// `display` must be a valid `wl_display` pointer that outlives this clipboard.
	pub(crate) unsafe fn new(display: *mut c_void) -> Self {
		Self {
			inner: unsafe { smithay_clipboard::Clipboard::new(display) },
		}
	}
}

#[cfg(all(unix, not(target_os = "android")))]
impl Clipboard for WaylandClipboard {
	fn get_text(&self) -> Option<String> {
		self.inner.load().ok()
	}

	fn set_text(&self, text: &str) {
		self.inner.store(text);
	}
}
