use std::{cell::RefCell, rc::Rc, sync::Arc};

use rlay::{AxisSize, Color, Layout, Node, Radius, Sizing};

use crate::{
	Element, ImageHandle, ImageKey, ImageProviderBuilder, ImageProviderInstance, ImageProviderState,
	NetworkImage, RenderContext,
	image::{DynImageProviderBuilder, ImageProvider},
	use_ref,
};

struct ImageElementState {
	key: Option<ImageKey>,
	instance: Option<Box<dyn ImageProviderInstance>>,
	last_state: ImageProviderState,
}

impl Default for ImageElementState {
	fn default() -> Self {
		Self {
			key: None,
			instance: None,
			last_state: ImageProviderState::Loading,
		}
	}
}

pub struct Image {
	provider: Option<Box<dyn DynImageProviderBuilder>>,
	state: Rc<RefCell<ImageElementState>>,
	id: Option<String>,
	width: AxisSize,
	height: AxisSize,
	aspect_ratio: Option<f32>,
	background_color: Color,
	corner_radius: Radius,
}

impl Default for Image {
	fn default() -> Self {
		Self {
			provider: None,
			state: use_ref(ImageElementState::default()),
			id: None,
			width: AxisSize::FIT,
			height: AxisSize::FIT,
			aspect_ratio: None,
			background_color: Color::TRANSPARENT,
			corner_radius: Radius::default(),
		}
	}
}

impl Image {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn source<P: ImageProviderBuilder>(mut self, provider: P) -> Self {
		self.provider = Some(Box::new(ImageProvider(provider)));
		self
	}
	pub fn url(self, url: impl Into<Arc<str>>) -> Self {
		self.source(NetworkImage::new(url))
	}
	pub fn id(mut self, id: impl Into<String>) -> Self {
		self.id = Some(id.into());
		self
	}

	pub fn w_fixed(mut self, width: f32) -> Self {
		self.width = AxisSize::fixed(width);
		self
	}

	pub fn h_fixed(mut self, height: f32) -> Self {
		self.height = AxisSize::fixed(height);
		self
	}

	pub fn w_expand(mut self) -> Self {
		self.width = AxisSize::GROW;
		self
	}

	pub fn h_expand(mut self) -> Self {
		self.height = AxisSize::GROW;
		self
	}

	pub fn w_percent(mut self, percentage: f32) -> Self {
		self.width = AxisSize::Percent(percentage);
		self
	}

	pub fn h_percent(mut self, percentage: f32) -> Self {
		self.height = AxisSize::Percent(percentage);
		self
	}

	pub fn aspect_ratio(mut self, ratio: f32) -> Self {
		self.aspect_ratio = (ratio > 0.0).then_some(ratio);
		self
	}

	pub fn background_color(mut self, color: impl Into<Color>) -> Self {
		self.background_color = color.into();
		self
	}

	pub fn rounded(mut self, radius: f32) -> Self {
		self.corner_radius = Radius::all(radius);
		self
	}

	pub fn state(&self) -> ImageProviderState {
		self.state.borrow().last_state.clone()
	}
}

impl Element for Image {
	fn render<'clay: 'render, 'render>(&'render self, ctx: &mut RenderContext<'clay, 'render, '_>) {
		let Some(provider) = &self.provider else {
			render_node(self, ctx, None);
			return;
		};

		let key = provider.key();
		let mut state = self.state.borrow_mut();
		if state.key.as_ref() != Some(&key) {
			state.key = Some(key);
			state.instance = Some(provider.build(&ctx.image_manager.provider_context()));
			state.last_state = ImageProviderState::Loading;
		}

		let next_state = state
			.instance
			.as_mut()
			.map_or(ImageProviderState::Loading, |instance| {
				instance.poll(&mut ctx.image_manager.poll_context())
			});
		state.last_state = next_state.clone();
		let handle = match next_state {
			ImageProviderState::Ready(handle) => Some(handle),
			ImageProviderState::Loading | ImageProviderState::Error(_) => None,
		};
		drop(state);

		render_node(self, ctx, handle);
	}
}

fn render_node(image: &Image, ctx: &mut RenderContext<'_, '_, '_>, handle: Option<ImageHandle>) {
	let mut node = handle.map_or_else(Node::new, |handle| Node::image(handle.id()));
	node = node
		.layout(Layout {
			sizing: Sizing {
				width: image.width,
				height: image.height,
			},
			..Layout::default()
		})
		.background(image.background_color)
		.radius(image.corner_radius)
		.aspect_ratio(image.aspect_ratio.unwrap_or(0.0));
	if let Some(id) = &image.id {
		node = node.id(id);
	}
	ctx.frame.child(node);
}
