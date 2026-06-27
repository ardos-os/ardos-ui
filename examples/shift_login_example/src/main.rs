#![allow(non_snake_case)]

use std::rc::Rc;

use ardos_ui::{
	Align, Direction, Element, ElementExt, FloatingAttachPointType, Input, InputRenderProps, InputRenderer, PointerCaptureMode, ShiftRootProps, rsml, use_input,
};

#[derive(Default)]
struct StatusPillProps {
	label: String,
}

fn StatusPill(props: StatusPillProps) -> Box<dyn Element> {
	rsml! {
		<container
			direction={Direction::Row}
			gap={8}
			weird_padding={(14, 14, 8, 8)}
			rounded={999.0}
			background_color={(0x18, 0x24, 0x34, 0xcc)}
			border_width={1}
			border_color={(0x72, 0x86, 0xa0, 0x70)}
			center
			h_fit
		>
			<container
				w_fixed={8.0}
				h_fixed={8.0}
				rounded={999.0}
				background_color={(0x76, 0xe4, 0xb7, 0xff)}
			/>
			<text font_size={13} color={(0xd9, 0xe4, 0xf2, 0xff)} font_weight={500}>
				{props.label}
			</text>
		</container>
	}
}

fn PasswordInput(_: ()) -> Box<dyn Element> {
	let input_id = "shift-login-password-input".to_string();
	let anchor_id = input_id.clone();
	let renderer: InputRenderer = Rc::new(move |input: InputRenderProps| {
		let border_color = if input.focused {
			(0x9d, 0xc7, 0xff, 0x99)
		} else {
			(0x72, 0x86, 0xa0, 0x55)
		};
		let show_placeholder = input.value.is_empty() && !input.focused;
		let has_selection = !input.selected_text.is_empty();

		let before_text = if has_selection {
			mask(&input.before_selection)
		} else {
			mask(&input.before_cursor)
		};
		let selected_text = mask(&input.selected_text);
		let after_text = if has_selection {
			mask(&input.after_selection)
		} else {
			mask(&input.after_cursor)
		};

		rsml! {
			<container
				id={input_id.clone()}
				direction={Direction::Column}
				gap={10}
				padding_all={14}
				rounded={6.0}
				background_color={(0x12, 0x1b, 0x29, 0xff)}
				style_if_hovered={|s| s.background_color((0x20, 0x1b, 0x29, 0xff))}
				border_width={1}
				border_color={border_color}
				clickable_ref={Rc::clone(&input.clickable_ref)}
				focusable
			>
				<text font_size={12} font_weight={600} color={(0x8f, 0xa1, 0xb8, 0xff)}>
					{"PASSWORD"}
				</text>

				<container
					direction={Direction::Row}
					gap={7}
					h_fixed={28.0}
					align={Align::Center}
					clip_x={true}
				>
					{if show_placeholder {
						rsml! {
							<text font_size={18} color={(0x8f, 0xa1, 0xb8, 0x88)}>
								{"Enter password"}
							</text>
						}.boxed()
					} else {
						rsml! {
							<container direction={Direction::Row} align={Align::Center} h_expand>
								{(!before_text.is_empty()).then(|| rsml! {
									<text font_size={22} color={(0xd6, 0xdf, 0xea, 0xff)}>
										{before_text}
									</text>
								})}

								{(input.cursor_visible && !has_selection).then(|| rsml! {
									<container
										w_fixed={2.0}
										h_fixed={22.0}
										background_color={(0x76, 0xe4, 0xb7, 0xff)}
									/>
								})}

								{(!selected_text.is_empty()).then(|| rsml! {
									<container
										direction={Direction::Row}
										background_color={(0x3d, 0x7d, 0xff, 0x99)}
										w_fit
										h_expand
										align={Align::Center}
									>
										<text font_size={22} color={(0xff, 0xff, 0xff, 0xff)}>
											{selected_text.clone()}
										</text>
									</container>
								})}

								{(!input.ime_buffer.is_empty()).then(|| rsml! {
									<text font_size={18} color={(0xd6, 0xdf, 0xea, 0xbb)}>
										{input.ime_buffer}
									</text>
								})}

								{(!after_text.is_empty()).then(|| rsml! {
									<text font_size={22} color={(0xd6, 0xdf, 0xea, 0xff)}>
										{after_text}
									</text>
								})}
							</container>
						}.boxed()
					}}
				</container>
			</container>
		}
	});

	rsml! {
		<Input
			ime_anchor_id={Some(anchor_id)}
			render={renderer}
		/>
	}
}

fn mask(value: &str) -> String {
	"*".repeat(value.chars().count())
}

#[derive(Default)]
struct LoginCardProps {
	monitor_label: String,
}

fn LoginCard(props: LoginCardProps) -> Box<dyn Element> {
	rsml! {
		<container
			w_fixed={420.0}
			h_fit
			direction={Direction::Column}
			gap={22}
			padding_all={28}
			rounded={8.0}
			background_color={(0x0f, 0x16, 0x22, 0xe8)}
			border_width={1}
			border_color={(0xb6, 0xc4, 0xd6, 0x40)}
		>
			<container direction={Direction::Column} gap={14} center>
				<container
					w_fixed={78.0}
					h_fixed={78.0}
					rounded={999.0}
					background_color={(0xe6, 0xf0, 0xff, 0xff)}
					center
				>
					<text font_size={28} font_weight={700} color={(0x1a, 0x2a, 0x3d, 0xff)} text_center>
						{"NP"}
					</text>
				</container>

				<container direction={Direction::Column} gap={5} center>
					<text font_size={28} font_weight={650} color={(0xf5, 0xf8, 0xfc, 0xff)} text_center>
						{"Nova Play"}
					</text>
					<text font_size={14} color={(0xb5, 0xc2, 0xd2, 0xff)} text_center>
						{props.monitor_label}
					</text>
				</container>
			</container>

			<PasswordInput />

			<container
				h_fixed={46.0}
				rounded={6.0}
				background_color={(0xe8, 0xf1, 0xff, 0xff)}
				center
			>
				<text font_size={15} font_weight={700} color={(0x13, 0x21, 0x31, 0xff)} text_center>
					{"Sign in"}
				</text>
			</container>

			<container direction={Direction::Row} gap={10} center>
				<StatusPill label={"Network".to_string()} />
				<StatusPill label={"Power".to_string()} />
				<StatusPill label={"Session".to_string()} />
			</container>
		</container>
	}
}

fn App(props: ShiftRootProps) -> Box<dyn Element> {
	let monitor = props.monitor;
	let monitor_label = format!(
		"{} - {}x{} @ {}Hz",
		monitor.name, monitor.width, monitor.height, monitor.refresh_rate
	);
	let footer = format!("Shift monitor id: {}", monitor.id);
	let input = use_input();
	rsml! {
		<container
			w_expand
			h_expand
			direction={Direction::Column}
			background_color={(0x07, 0x0c, 0x13, 0xff)}
		>
			<container
				w_expand
				h_fixed={76.0}
				direction={Direction::Row}
				padding_all={24}
				background_color={(0x0a, 0x12, 0x1d, 0xff)}
				border_bottom={1}
				border_color={(0x72, 0x86, 0xa0, 0x35)}
			>
				<container direction={Direction::Column} gap={3} h_expand>
					<text font_size={18} font_weight={700} color={(0xf5, 0xf8, 0xfc, 0xff)}>
						{"Nova OS"}
					</text>
					<text font_size={12} color={(0x8f, 0xa1, 0xb8, 0xff)}>
						{"Shift native login stub"}
					</text>
				</container>
			</container>

			<container
				w_expand
				h_expand
				direction={Direction::Column}
				center
				gap={24}
				padding_all={32}
			>
				<LoginCard monitor_label={monitor_label} />

				<text font_size={13} color={(0x8f, 0xa1, 0xb8, 0xff)} text_center>
					{footer}
				</text>
			</container>

			<container background_color={(255,0,0,255)} floating w_fixed={16.} h_fixed={16.} floating_offset={input.with(|i| i.mouse_position())} floating_pointer_capture_mode={PointerCaptureMode::Passthrough} floating_attach_points={(FloatingAttachPointType::LeftTop, FloatingAttachPointType::LeftTop)}>

			</container>
		</container>
	}
}

fn main() {
	ardos_ui::create_window_shift(App).expect("failed to start Shift login screen");
}
