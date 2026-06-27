#![allow(non_snake_case)]

use ardos_ui::{Direction, Element, MemoryImage, WindowOptions, rsml};

const RED_PIXEL_PNG: &'static [u8] =
	compile_time_run::run_command!("convert", "-size", "64x64", "xc:red", "png:-");

use lucide_icons::Icon as LucideIcon;

pub struct IconProps {
	pub icon: LucideIcon,
	pub size: u16,
	pub color: (u8, u8, u8, u8),
}

impl Default for IconProps {
	fn default() -> Self {
		Self {
			icon: LucideIcon::Heart,
			size: 24,
			color: (0xff, 0xff, 0xff, 0xff),
		}
	}
}

pub fn Icon(props: IconProps) -> Box<dyn Element> {
	let glyph = char::from(props.icon).to_string();

	rsml! {
	<text
		font_bytes={lucide_icons::LUCIDE_FONT_BYTES}
		font_size={props.size}
		color={props.color}
		text_center
	>
		{glyph}
	</text>
	}
}
fn App(_: ()) -> Box<dyn Element> {
	rsml! {
		<container
			w_expand
			h_expand
			direction={Direction::Column}
			gap={16}
			padding_all={24}
			background_color={(0x0d, 0x10, 0x18, 0xff)}
		>
			<text font_size={24} color={(0xff, 0xff, 0xff, 0xff)} font_weight={600}>
				{"Ardos UI Images"}
			</text>

			<image
				id="memory-image"
				source={MemoryImage::new(RED_PIXEL_PNG)}
				w_fixed={240.0}
				aspect_ratio={16.0 / 9.0}
				rounded={16.0}
			/>

			<Icon icon={LucideIcon::ArrowDownLeft} />
		</container>
	}
}

fn main() {
	env_logger::init();
	ardos_ui::create_window_winit(
		App,
		WindowOptions {
			title: "Ardos UI — Images & Icons".into(),
			preferred_size: (480.0, 360.0),
			..Default::default()
		},
	);
}
