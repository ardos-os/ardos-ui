use crate as ardos_ui;

use crate::{
	Element,
	element::container::{
		Container, FloatingAttachPointType, FloatingAttachToElement, PointerCaptureMode,
	},
	rsml, use_input,
};

pub struct CursorProps {
	pub children: Vec<Box<dyn Element>>,
	pub offset: (f32, f32),
	pub z_index: i16,
}

impl Default for CursorProps {
	fn default() -> Self {
		Self {
			children: Vec::new(),
			offset: (0.0, 0.0),
			z_index: i16::MAX,
		}
	}
}

#[allow(non_snake_case)]
pub fn Cursor(props: CursorProps) -> Box<dyn Element> {
	let input = use_input();
	let children = props.children;
	let Some(position) = input.with(|input| input.cursor_visible().then(|| input.mouse_position()))
	else {
		return Box::new(None::<Container>);
	};
	let position = (position.0 + props.offset.0, position.1 + props.offset.1);

	rsml! {
		<container
			floating
			w_fit
			h_fit
			floating_offset={position}
			floating_z_index={props.z_index}
			floating_attach_to={FloatingAttachToElement::Root}
			floating_attach_points={(FloatingAttachPointType::LeftTop, FloatingAttachPointType::LeftTop)}
			floating_pointer_capture_mode={PointerCaptureMode::Passthrough}
		>
			{children}
		</container>
	}
}
