#![allow(non_snake_case)]

use ardos_ui::*;

/// Demonstrates a "glass" overlay using:
/// - `floating={true}` + `floating_*` attributes to place it above content
/// - `backdrop_blur={...}` to blur the already-rendered background behind it
/// - a semi-transparent background color so the blur is visible
fn Root(_: ()) -> Box<dyn Element> {
	let (count, set_count) = use_state(0);

	// `use_state` setter is cloneable, so we can freely reuse it in multiple closures.
	let set_count_inc = set_count.clone();
	let set_count_reset = set_count.clone();

	rsml! {
		<container
			w_expand
			h_expand
			background_color={(0x0b, 0x0b, 0x0b, 0xff)}
			padding_all={16}
			gap={16}
			direction={Direction::Column}
		>
			<container
				direction={Direction::Row}
				gap={12}
			>
				<container
					w_fit
					padding_all={12}
					rounded={10.0}
					background_color={(0x18, 0x18, 0x18, 0xff)}
					border_width={1}
					border_color={(0xff, 0xff, 0xff, 0x20)}
				>
					<text font_family="UbuntuSans NF" color={(0xff, 0xff, 0xff, 0xff)} font_size={18}>
						{"Backdrop blur demo"}
					</text>

					<container gap={6}>
						<text color={(0xff, 0xff, 0xff, 0xb0)} font_size={13}>
							{"The overlay below is floating and blurs what's behind it."}
						</text>
						<text color={(0xff, 0xff, 0xff, 0xb0)} font_size={13}>
							{"Click the button to change content under the blur."}
						</text>
					</container>
				</container>

				<container
					w_expand
					padding_all={12}
					rounded={10.0}
					background_color={(0x12, 0x12, 0x12, 0xff)}
					border_width={1}
					border_color={(0xff, 0xff, 0xff, 0x10)}
					gap={8}
				>
					<text color={(0xff, 0xff, 0xff, 0xff)} font_size={14} font_family="UbuntuSans NF">
						{format!("Counter under blur: {}", count)}
					</text>

					<container direction={Direction::Row} gap={8}>
						<container
							w_fit
							padding_all={10}
							rounded={8.0}
							border_width={1}
							border_color={(0xff, 0xff, 0xff, 0x20)}
							background_color={(0x2a, 0x6f, 0xff, 0x30)}
							on_click={move || set_count_inc(count + 1)}
						>
							<text color={(0xff, 0xff, 0xff, 0xff)} font_size={13} font_family="UbuntuSans NF">
								{"Increment"}
							</text>
						</container>

						<container
							w_fit
							padding_all={10}
							rounded={8.0}
							border_width={1}
							border_color={(0xff, 0xff, 0xff, 0x20)}
							background_color={(0xff, 0x2a, 0x6f, 0x30)}
							on_click={move || set_count_reset(0)}
						>
							<text color={(0xff, 0xff, 0xff, 0xff)} font_size={13} font_family="UbuntuSans NF">
								{"Reset"}
							</text>
						</container>
					</container>

					<container direction={Direction::Row} gap={8}>
						<container w_fit padding_all={8} rounded={8.0} background_color={(0xff, 0x00, 0x88, 0x60)} />
						<container w_fit padding_all={8} rounded={8.0} background_color={(0x00, 0xff, 0xaa, 0x60)} />
						<container w_fit padding_all={8} rounded={8.0} background_color={(0x00, 0xaa, 0xff, 0x60)} />
						<container w_fit padding_all={8} rounded={8.0} background_color={(0xff, 0xaa, 0x00, 0x60)} />
					</container>
				</container>
			</container>

			<container
				floating={true}
				floating_z_index={50}
				floating_attach_to={clay_layout::elements::FloatingAttachToElement::Root}
				floating_attach_points={
					(
						clay_layout::elements::FloatingAttachPointType::CenterCenter,
						clay_layout::elements::FloatingAttachPointType::CenterCenter,
					)
				}
				floating_offset={(0.0, 0.0)}
				w_fit
				h_fit
				floating_pointer_capture_mode={clay_layout::elements::PointerCaptureMode::Passthrough}

				rounded={16.0}
				padding_all={16}
				gap={10}
				direction={Direction::Column}

				backdrop_blur={16.0}

				background_color={(0xff, 0xff, 0xff, 0x18)}
				border_width={1}
				border_color={(0xff, 0xff, 0xff, 0x2a)}
			>
				<text font_family="UbuntuSans NF" font_size={16} color={(0xff, 0xff, 0xff, 0xff)}>
					{"Floating glass overlay"}
				</text>

				<text font_size={13} color={(0xff, 0xff, 0xff, 0xc0)}>
					{"This element is floating above everything and blurs the backdrop, clipped to its border radius."}
				</text>

				<container direction={Direction::Row} gap={10}>
					<container w_fit padding_all={10} rounded={10.0} background_color={(0x00, 0x00, 0x00, 0x30)}>
						<text font_family="UbuntuSans NF" font_size={13} color={(0xff, 0xff, 0xff, 0xff)}>
							{format!("Count = {}", count)}
						</text>
					</container>

					<container w_fit padding_all={10} rounded={10.0} background_color={(0x00, 0x00, 0x00, 0x30)}>
						<text font_family="UbuntuSans NF" font_size={13} color={(0xff, 0xff, 0xff, 0xff)}>
							{"Try moving/scrolling content behind to see the blur update"}
						</text>
					</container>
				</container>
			</container>
		</container>
	}
}

fn main() {
	ardos_ui::create_window(
		Root,
		WindowOptions {
			title: "Backdrop Blur + Floating Example".into(),
			preferred_size: (920.0, 560.0),
			..Default::default()
		},
	)
}
