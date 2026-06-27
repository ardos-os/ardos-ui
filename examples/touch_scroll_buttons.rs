use ardos_ui::{Container, ContainerStyle, Direction, Element, Text, WindowOptions, use_state};

fn button(index: usize, set_last_pressed: ardos_ui::StateSetter<usize>) -> Container {
	Container::new()
		.id(format!("button-{index}"))
		.w_expand()
		.h_fixed(56.0)
		.padding_all(14)
		.rounded(8.0)
		.background_color((0x24, 0x2a, 0x31, 0xff))
		.border_width(1)
		.border_color((0xff, 0xff, 0xff, 0x18))
		.style_if_hovered(|style| ContainerStyle {
			background_color: (0x2f, 0x38, 0x42, 0xff).into(),
			..style
		})
		.style_if_pressed(|style| ContainerStyle {
			background_color: (0x14, 0x68, 0xb7, 0xff).into(),
			..style
		})
		.on_click(move || set_last_pressed(index))
		.center()
		.child(
			Text::new(format!("Button {index:02}"))
				.font_size(16)
				.color((0xff, 0xff, 0xff, 0xff)),
		)
}

fn app(_: ()) -> Box<dyn Element> {
	let (last_pressed, set_last_pressed) = use_state(0usize);

	let mut list = Container::new()
		.id("touch-scroll-list")
		.w_expand()
		.h_expand()
		.scroll_y(true)
		.clip_y(true)
		.gap(8)
		.padding_all(12)
		.background_color((0x10, 0x12, 0x15, 0xff));

	for index in 1..=40 {
		list = list.child(button(index, set_last_pressed.clone()));
	}

	Box::new(
		Container::new()
			.w_expand()
			.h_expand()
			.direction(Direction::Column)
			.gap(12)
			.padding_all(16)
			.background_color((0x07, 0x08, 0x0a, 0xff))
			.child(
				Text::new(format!("Last clicked: {last_pressed}"))
					.font_size(18)
					.color((0xff, 0xff, 0xff, 0xff)),
			)
			.child(list),
	)
}

fn main() {
	env_logger::init();
	ardos_ui::create_window_winit(
		app,
		WindowOptions {
			title: "Touch Scroll Buttons".into(),
			preferred_size: (420.0, 720.0),
			..Default::default()
		},
	);
}
