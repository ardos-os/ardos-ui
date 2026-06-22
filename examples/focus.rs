#![allow(non_snake_case)]

use ardos_ui::*;
fn Button(_: ()) -> Box<dyn Element> {
	let (count, set_count) = use_state(0);
	let clickable_ref = use_ref(ClickableState::default());
	// dbg!(clickable_ref.borrow().focus_node_id);
	dbg!(clickable_ref.borrow().is_focused());
	// clickable_ref.unwrap().
	//                        ------------------------------
	//                        | is_hovered: boolean        |
	//                        | is_down: boolean           |
	//                        | is_focused: boolean        |
	//                        | is_disabled: boolean       |
	//                        -----------------------------
	rsml! {
		<container
			padding_all={8}
			rounded={4.}
			w_fit
			border_width={1}
			border_color={(0xff, 0xff, 0xff, 0x20)}
			on_click={move || set_count(count + 1)}

			style_if_hovered={|s| s.background_color((0xff, 0xff, 0xff, 0x20))}
			style_if_pressed={|s| s.background_color((0xff, 0xff, 0xff, 0x40))}
			style_if_focused={|s| s.border_width(4).border_color((0x04, 0x36, 0x82, 0xff))}
			clickable_ref={clickable_ref}

			focusable
		>


			<text color={(255, 255, 255, 255)} font_family="UbuntuSans NF">
				{format!("Count: {}", count)}
			</text>
		</container>
	}
}

fn Root(_: ()) -> Box<dyn Element> {
	rsml! {
		<container h_expand w_expand background_color={(0x0b, 0x0b, 0x0b)} center gap={3*4} direction={Direction::Row}>
			<Button/>
			<Button/>
		</container>
	}
}

fn main() {
	ardos_ui::create_window(
		Root,
		WindowOptions {
			title: "Focus Example".into(),
			..Default::default()
		},
	)
}
