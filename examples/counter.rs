use ardos_ui::{ContainerStyle, Element, WindowOptions, rsml, use_state};

fn counter_component(_: ()) -> Box<dyn Element> {
	let (count, set_count) = use_state(0);

	rsml! {
			<container
					direction={ardos_ui::Direction::Column}
					padding_all={20}
					background_color={(0x1a, 0x1a, 0x1a)}
					h_expand
					gap={10}
					center>

					<text
							font_size={20}
							color={(255, 255, 255, 255)}
							font_family="UbuntuSans NF"
							text_center
						>
							RSML Counter Test
					</text>

					<container
							background_color={(0x00, 0x7a, 0xcc)}
							padding_all={16}
							rounded={8.0}
							on_click={move || set_count(count + 1)}
							style_if_hovered={|s| ContainerStyle {background_color: (0x00, 0x7a/2, 0xcc/2).into(), ..s}}
							style_if_pressed={|s| ContainerStyle {background_color: (0x00, 0x7a/3, 0xcc/3).into(), ..s}}
							center>
								<text
										font_size={16}
										color={(255, 255, 255, 255)}
										font_family="UbuntuSans NF"
										text_center
										>
										{format!("Count: {}", count)}
								</text>
					</container>

					<text
							font_size={14}
							color={(200, 200, 200, 255)}
							font_family="UbuntuSans NF"
							text_center
							>
							Click the button to increment!
					</text>
			</container>
	}
}

fn main() {
	env_logger::init();

	ardos_ui::create_window(
		counter_component,
		WindowOptions {
			title: "Welcome to Ardos UI".into(),
			preferred_size: (400.0, 300.0),
			..Default::default()
		},
	);
}
