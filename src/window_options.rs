use winit::dpi::LogicalSize;
use winit::icon::RgbaIcon;
use winit::monitor::Fullscreen;
#[cfg(all(unix, not(target_os = "android")))]
use winit::platform::wayland::WindowAttributesWayland;
use winit::window::WindowAttributes;

#[derive(Clone, Copy, Default)]
pub struct Anchor(u32);

impl Anchor {
	pub const TOP: Self = Self(1 << 0);
	pub const BOTTOM: Self = Self(1 << 1);
	pub const LEFT: Self = Self(1 << 2);
	pub const RIGHT: Self = Self(1 << 3);

	pub fn empty() -> Self {
		Self(0)
	}
}

impl std::ops::BitOr for Anchor {
	type Output = Self;

	fn bitor(self, rhs: Self) -> Self::Output {
		Self(self.0 | rhs.0)
	}
}

#[derive(Clone, Copy, Default)]
pub enum KeyboardInteractivity {
	#[default]
	None,
	Exclusive,
	OnDemand,
}

#[derive(Clone)]
pub struct LayerShellOptions {
	pub anchor: Anchor,
	pub exclusive_zone: i32,
	pub margin: (i32, i32, i32, i32),
	pub keyboard_interactivity: KeyboardInteractivity,
	pub output: Option<u64>,
}
impl Default for LayerShellOptions {
	fn default() -> Self {
		Self {
			anchor: Anchor::empty(),
			exclusive_zone: 0,
			margin: (0, 0, 0, 0),
			keyboard_interactivity: KeyboardInteractivity::None,
			output: None,
		}
	}
}
#[derive(Default, Clone)]
pub struct WindowOptions<'a> {
	pub title: String,
	pub min_size: (f64, f64),
	pub preferred_size: (f64, f64),
	pub max_size: (f64, f64),
	pub enable_layer_shell: Option<LayerShellOptions>,
	pub opaque: bool,
	pub allow_backdrop_blur: bool,
	pub wayland_name: Option<&'a str>,
	pub no_border: bool,
	pub fullscreen: bool,
	pub icon: Option<RgbaIcon>,
}
impl From<WindowOptions<'_>> for WindowAttributes {
	fn from(options: WindowOptions) -> Self {
		let mut winit_opt = WindowAttributes::default()
			.with_blur(options.allow_backdrop_blur)
			.with_transparent(!options.opaque)
			.with_decorations(!options.no_border)
			.with_fullscreen(if options.fullscreen {
				Some(Fullscreen::Borderless(None))
			} else {
				None
			})
			.with_title(if options.title.is_empty() {
				"<Untitled>".to_string()
			} else {
				options.title
			})
			.with_window_icon(options.icon.map(|i| i.into()));
		if options.min_size != (0., 0.) {
			winit_opt =
				winit_opt.with_min_surface_size(LogicalSize::new(options.min_size.0, options.min_size.1));
		}
		if options.preferred_size != (0., 0.) {
			winit_opt = winit_opt.with_surface_size(LogicalSize::new(
				options.preferred_size.0,
				options.preferred_size.1,
			))
		}
		if options.max_size != (0., 0.) {
			winit_opt =
				winit_opt.with_max_surface_size(LogicalSize::new(options.max_size.0, options.max_size.1))
		}

		winit_opt = apply_wayland_options(winit_opt, options.enable_layer_shell, options.wayland_name);
		winit_opt
	}
}

#[cfg(all(unix, not(target_os = "android")))]
fn apply_wayland_options(
	mut winit_opt: WindowAttributes,
	layer_shell: Option<LayerShellOptions>,
	wayland_name: Option<&str>,
) -> WindowAttributes {
	let mut wayland_opts = WindowAttributesWayland::default();
	let mut has_wl_opts = false;
	if let Some(l) = layer_shell {
		wayland_opts = wayland_opts
			.with_layer_shell()
			.with_margin(l.margin.0, l.margin.1, l.margin.2, l.margin.3)
			.with_anchor(l.anchor.into())
			.with_exclusive_zone(l.exclusive_zone);
		if let Some(output) = l.output {
			wayland_opts = wayland_opts.with_output(output);
		}
		has_wl_opts = true;
	}
	if let Some(wayland_name) = wayland_name {
		wayland_opts = wayland_opts.with_name(wayland_name, "");
		has_wl_opts = true;
	}
	if has_wl_opts {
		winit_opt = winit_opt.with_platform_attributes(Box::new(wayland_opts));
	}
	winit_opt
}

#[cfg(not(all(unix, not(target_os = "android"))))]
fn apply_wayland_options(
	winit_opt: WindowAttributes,
	_layer_shell: Option<LayerShellOptions>,
	_wayland_name: Option<&str>,
) -> WindowAttributes {
	winit_opt
}

#[cfg(all(unix, not(target_os = "android")))]
impl From<Anchor> for winit::platform::wayland::Anchor {
	fn from(anchor: Anchor) -> Self {
		let mut out = Self::empty();
		if anchor.0 & Anchor::TOP.0 != 0 {
			out |= Self::TOP;
		}
		if anchor.0 & Anchor::BOTTOM.0 != 0 {
			out |= Self::BOTTOM;
		}
		if anchor.0 & Anchor::LEFT.0 != 0 {
			out |= Self::LEFT;
		}
		if anchor.0 & Anchor::RIGHT.0 != 0 {
			out |= Self::RIGHT;
		}
		out
	}
}
