#![allow(non_snake_case)]

use ardos_ui::{
	Align, ContainerStyle, Direction, Element, StateSetter, Transition, TransitionProperties,
	WindowOptions, rsml, use_state,
};

#[derive(Clone)]
struct Item {
	id: u32,
	label: &'static str,
	color: (u8, u8, u8, u8),
}

fn move_item(items: &[Item], index: usize, offset: isize) -> Vec<Item> {
	let target = index as isize + offset;
	if !(0..items.len() as isize).contains(&target) {
		return items.to_vec();
	}

	let mut reordered = items.to_vec();
	reordered.swap(index, target as usize);
	reordered
}

fn ListItem(
	item: Item,
	index: usize,
	item_count: usize,
	items: Vec<Item>,
	set_items: StateSetter<Vec<Item>>,
) -> Box<dyn Element> {
	let can_move_up = index > 0;
	let can_move_down = index + 1 < item_count;

	let items_for_up = items.clone();
	let set_items_up = set_items.clone();
	let items_for_down = items;
	let set_items_down = set_items;

	rsml! {
		<container
			id={format!("reorder-item-{}", item.id)}
			transition={Transition::ease_out(0.25, TransitionProperties::POSITION)}
			w_expand
			h_fixed={72.0}
			direction={Direction::Row}
			align={Align::Center}
			gap={12}
			padding_all={12}
			rounded={12.0}
			background_color={item.color}
			border_width={1}
			border_color={(0xff, 0xff, 0xff, 0x24)}
		>
			<container
				w_fixed={40.0}
				h_fixed={40.0}
				rounded={20.0}
				background_color={(0x00, 0x00, 0x00, 0x24)}
				center
			>
				<text font_size={16} color={(0xff, 0xff, 0xff, 0xff)} text_center>
					{item.id}
				</text>
			</container>

			<container w_expand direction={Direction::Column} gap={3}>
				<text font_size={17} color={(0xff, 0xff, 0xff, 0xff)}>
					{item.label}
				</text>
				<text font_size={12} color={(0xff, 0xff, 0xff, 0xa8)}>
					{format!("Posição {}", index + 1)}
				</text>
			</container>

			<container
				w_fixed={40.0}
				h_fixed={40.0}
				rounded={8.0}
				background_color={if can_move_up {
					(0xff, 0xff, 0xff, 0x20)
				} else {
					(0xff, 0xff, 0xff, 0x0a)
				}}
				style_if_hovered={move |style| if can_move_up {
					ContainerStyle {
						background_color: (0xff, 0xff, 0xff, 0x36).into(),
						..style
					}
				} else {
					style
				}}
				on_click={move || {
					if can_move_up {
						set_items_up(move_item(&items_for_up, index, -1));
					}
				}}
				center
			>
				<text
					font_size={20}
					color={if can_move_up {
						(0xff, 0xff, 0xff, 0xff)
					} else {
						(0xff, 0xff, 0xff, 0x40)
					}}
					text_center
				>
					{"/\\"}
				</text>
			</container>

			<container
				w_fixed={40.0}
				h_fixed={40.0}
				rounded={8.0}
				background_color={if can_move_down {
					(0xff, 0xff, 0xff, 0x20)
				} else {
					(0xff, 0xff, 0xff, 0x0a)
				}}
				style_if_hovered={move |style| if can_move_down {
					ContainerStyle {
						background_color: (0xff, 0xff, 0xff, 0x36).into(),
						..style
					}
				} else {
					style
				}}
				on_click={move || {
					if can_move_down {
						set_items_down(move_item(&items_for_down, index, 1));
					}
				}}
				center
			>
				<text
					font_size={20}
					color={if can_move_down {
						(0xff, 0xff, 0xff, 0xff)
					} else {
						(0xff, 0xff, 0xff, 0x40)
					}}
					text_center
				>
					{"\\/"}
				</text>
			</container>
		</container>
	}
}

fn App(_: ()) -> Box<dyn Element> {
	let (items, set_items) = use_state(vec![
		Item {
			id: 1,
			label: "Design system",
			color: (0x36, 0x55, 0xd6, 0xff),
		},
		Item {
			id: 2,
			label: "Layout engine",
			color: (0x72, 0x3d, 0xc6, 0xff),
		},
		Item {
			id: 3,
			label: "Renderer Skia",
			color: (0xc1, 0x47, 0x78, 0xff),
		},
		Item {
			id: 4,
			label: "Input e eventos",
			color: (0xc2, 0x68, 0x35, 0xff),
		},
		Item {
			id: 5,
			label: "Aplicação",
			color: (0x28, 0x8b, 0x78, 0xff),
		},
	]);

	let item_count = items.len();
	let rendered_items = items
		.iter()
		.cloned()
		.enumerate()
		.map(|(index, item)| ListItem(item, index, item_count, items.clone(), set_items.clone()))
		.collect::<Vec<_>>();

	rsml! {
		<container
			w_expand
			h_expand
			direction={Direction::Column}
			align={Align::Center}
			padding_all={28}
			background_color={(0x0d, 0x10, 0x18, 0xff)}
		>
			<container
				w_expand
				h_expand
				direction={Direction::Column}
				gap={16}
			>
				<container direction={Direction::Column} gap={5}>
					<text font_size={26} color={(0xff, 0xff, 0xff, 0xff)}>
						{"Reordenar itens"}
					</text>
					<text font_size={14} color={(0xff, 0xff, 0xff, 0xa0)}>
						{"Use as setas para alterar a ordem."}
					</text>
				</container>

				<container
					id="reorder-list"
					w_expand
					h_expand
					direction={Direction::Column}
					gap={10}
					scroll_y={true}
				>
					{rendered_items}
				</container>
			</container>
		</container>
	}
}

fn main() {
	env_logger::init();

	ardos_ui::create_window_winit(
		App,
		WindowOptions {
			title: "Ardos UI — Reorder Transition".into(),
			preferred_size: (720.0, 620.0),
			..Default::default()
		},
	);
}
