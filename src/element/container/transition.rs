use std::cell::RefCell;

use bitflags::bitflags;

bitflags! {
	#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
	pub struct TransitionProperties: u16 {
		const X = 1;
		const Y = 2;
		const POSITION = Self::X.bits() | Self::Y.bits();
		const WIDTH = 4;
		const HEIGHT = 8;
		const DIMENSIONS = Self::WIDTH.bits() | Self::HEIGHT.bits();
		const BOUNDS = Self::POSITION.bits() | Self::DIMENSIONS.bits();
		const BACKGROUND_COLOR = 16;
		const OVERLAY_COLOR = 32;
		const CORNER_RADIUS = 64;
		const BORDER_COLOR = 128;
		const BORDER_WIDTH = 256;
		const BORDER = Self::BORDER_COLOR.bits() | Self::BORDER_WIDTH.bits();
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TransitionBounds {
	pub x: f32,
	pub y: f32,
	pub width: f32,
	pub height: f32,
}

impl TransitionBounds {
	pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
		Self {
			x,
			y,
			width,
			height,
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TransitionColor {
	pub red: f32,
	pub green: f32,
	pub blue: f32,
	pub alpha: f32,
}

impl TransitionColor {
	pub const TRANSPARENT: Self = Self::rgba(0.0, 0.0, 0.0, 0.0);

	pub const fn rgba(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
		Self {
			red,
			green,
			blue,
			alpha,
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TransitionRadius {
	pub top_left: f32,
	pub top_right: f32,
	pub bottom_left: f32,
	pub bottom_right: f32,
}

impl TransitionRadius {
	pub const fn all(radius: f32) -> Self {
		Self {
			top_left: radius,
			top_right: radius,
			bottom_left: radius,
			bottom_right: radius,
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TransitionBorderWidth {
	pub left: f32,
	pub right: f32,
	pub top: f32,
	pub bottom: f32,
}

impl TransitionBorderWidth {
	pub const fn all(width: f32) -> Self {
		Self {
			left: width,
			right: width,
			top: width,
			bottom: width,
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TransitionValues {
	pub bounds: TransitionBounds,
	pub background: TransitionColor,
	pub overlay: TransitionColor,
	pub radius: TransitionRadius,
	pub border_color: TransitionColor,
	pub border_width: TransitionBorderWidth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransitionState {
	#[default]
	Idle,
	Entering,
	Transitioning,
	Exiting,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransitionArgs {
	pub state: TransitionState,
	pub initial: TransitionValues,
	pub current: TransitionValues,
	pub target: TransitionValues,
	pub elapsed: f32,
	pub duration: f32,
	pub properties: TransitionProperties,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransitionFrame {
	pub values: TransitionValues,
	pub complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransitionInteraction {
	#[default]
	Disable,
	Allow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransitionEnterTrigger {
	#[default]
	SkipOnFirstParentFrame,
	OnFirstParentFrame,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransitionExitTrigger {
	#[default]
	SkipWhenParentExits,
	WhenParentExits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransitionExitOrdering {
	#[default]
	Underneath,
	Natural,
	Above,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TransitionEnter {
	pub initial: Option<fn(TransitionValues, TransitionProperties) -> TransitionValues>,
	pub trigger: TransitionEnterTrigger,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TransitionExit {
	pub target: Option<fn(TransitionValues, TransitionProperties) -> TransitionValues>,
	pub trigger: TransitionExitTrigger,
	pub sibling_ordering: TransitionExitOrdering,
}

#[derive(Debug, Clone, Copy)]
pub struct Transition {
	pub handler: fn(TransitionArgs) -> TransitionFrame,
	pub duration: f32,
	pub properties: TransitionProperties,
	pub interaction: TransitionInteraction,
	pub enter: TransitionEnter,
	pub exit: TransitionExit,
}

impl Transition {
	pub const fn ease_out(duration: f32, properties: TransitionProperties) -> Self {
		Self {
			handler: ease_out,
			duration,
			properties,
			interaction: TransitionInteraction::Disable,
			enter: TransitionEnter {
				initial: None,
				trigger: TransitionEnterTrigger::SkipOnFirstParentFrame,
			},
			exit: TransitionExit {
				target: None,
				trigger: TransitionExitTrigger::SkipWhenParentExits,
				sibling_ordering: TransitionExitOrdering::Underneath,
			},
		}
	}

	pub(super) fn into_rlay(self) -> rlay::Transition {
		let data = register_callbacks(self);
		rlay::Transition {
			handler: rlay::ease_out,
			duration: self.duration,
			properties: rlay::TransitionProperties::from_bits_retain(self.properties.bits()),
			interaction: match self.interaction {
				TransitionInteraction::Disable => rlay::TransitionInteraction::Disable,
				TransitionInteraction::Allow => rlay::TransitionInteraction::Allow,
			},
			enter: rlay::TransitionEnter {
				initial: None,
				trigger: match self.enter.trigger {
					TransitionEnterTrigger::SkipOnFirstParentFrame => {
						rlay::TransitionEnterTrigger::SkipOnFirstParentFrame
					}
					TransitionEnterTrigger::OnFirstParentFrame => {
						rlay::TransitionEnterTrigger::OnFirstParentFrame
					}
				},
			},
			exit: rlay::TransitionExit {
				target: None,
				trigger: match self.exit.trigger {
					TransitionExitTrigger::SkipWhenParentExits => {
						rlay::TransitionExitTrigger::SkipWhenParentExits
					}
					TransitionExitTrigger::WhenParentExits => rlay::TransitionExitTrigger::WhenParentExits,
				},
				sibling_ordering: match self.exit.sibling_ordering {
					TransitionExitOrdering::Underneath => rlay::TransitionExitOrdering::Underneath,
					TransitionExitOrdering::Natural => rlay::TransitionExitOrdering::Natural,
					TransitionExitOrdering::Above => rlay::TransitionExitOrdering::Above,
				},
			},
			adapter: Some(rlay::TransitionAdapter {
				data,
				handler: handler_bridge,
				enter: self.enter.initial.map(|_| enter_bridge as _),
				exit: self.exit.target.map(|_| exit_bridge as _),
			}),
		}
	}
}

#[must_use]
pub fn ease_out(args: TransitionArgs) -> TransitionFrame {
	let ratio = if args.duration > 0.0 {
		(args.elapsed / args.duration).clamp(0.0, 1.0)
	} else {
		1.0
	};
	let amount = 1.0 - (1.0 - ratio).powi(3);
	let mut values = args.current;
	let properties = args.properties;

	if properties.contains(TransitionProperties::X) {
		values.bounds.x = lerp(args.initial.bounds.x, args.target.bounds.x, amount);
	}
	if properties.contains(TransitionProperties::Y) {
		values.bounds.y = lerp(args.initial.bounds.y, args.target.bounds.y, amount);
	}
	if properties.contains(TransitionProperties::WIDTH) {
		values.bounds.width = lerp(args.initial.bounds.width, args.target.bounds.width, amount);
	}
	if properties.contains(TransitionProperties::HEIGHT) {
		values.bounds.height = lerp(
			args.initial.bounds.height,
			args.target.bounds.height,
			amount,
		);
	}
	if properties.contains(TransitionProperties::BACKGROUND_COLOR) {
		values.background = lerp_color(args.initial.background, args.target.background, amount);
	}
	if properties.contains(TransitionProperties::OVERLAY_COLOR) {
		values.overlay = lerp_color(args.initial.overlay, args.target.overlay, amount);
	}
	if properties.contains(TransitionProperties::CORNER_RADIUS) {
		values.radius = TransitionRadius {
			top_left: lerp(
				args.initial.radius.top_left,
				args.target.radius.top_left,
				amount,
			),
			top_right: lerp(
				args.initial.radius.top_right,
				args.target.radius.top_right,
				amount,
			),
			bottom_left: lerp(
				args.initial.radius.bottom_left,
				args.target.radius.bottom_left,
				amount,
			),
			bottom_right: lerp(
				args.initial.radius.bottom_right,
				args.target.radius.bottom_right,
				amount,
			),
		};
	}
	if properties.contains(TransitionProperties::BORDER_COLOR) {
		values.border_color = lerp_color(args.initial.border_color, args.target.border_color, amount);
	}
	if properties.contains(TransitionProperties::BORDER_WIDTH) {
		values.border_width = TransitionBorderWidth {
			left: lerp(
				args.initial.border_width.left,
				args.target.border_width.left,
				amount,
			),
			right: lerp(
				args.initial.border_width.right,
				args.target.border_width.right,
				amount,
			),
			top: lerp(
				args.initial.border_width.top,
				args.target.border_width.top,
				amount,
			),
			bottom: lerp(
				args.initial.border_width.bottom,
				args.target.border_width.bottom,
				amount,
			),
		};
	}

	TransitionFrame {
		values,
		complete: ratio >= 1.0,
	}
}

#[derive(Clone, Copy)]
struct Callbacks {
	handler: fn(TransitionArgs) -> TransitionFrame,
	enter: Option<fn(TransitionValues, TransitionProperties) -> TransitionValues>,
	exit: Option<fn(TransitionValues, TransitionProperties) -> TransitionValues>,
}

thread_local! {
	// ponytail: callback combinations are static in UI code; retain one entry per combination.
	static CALLBACKS: RefCell<Vec<Callbacks>> = const { RefCell::new(Vec::new()) };
}

fn register_callbacks(transition: Transition) -> usize {
	let callbacks = Callbacks {
		handler: transition.handler,
		enter: transition.enter.initial,
		exit: transition.exit.target,
	};
	CALLBACKS.with_borrow_mut(|registered| {
		registered
			.iter()
			.position(|existing| callback_key(*existing) == callback_key(callbacks))
			.unwrap_or_else(|| {
				registered.push(callbacks);
				registered.len() - 1
			})
	})
}

fn callback_key(callbacks: Callbacks) -> (usize, Option<usize>, Option<usize>) {
	(
		callbacks.handler as usize,
		callbacks.enter.map(|callback| callback as usize),
		callbacks.exit.map(|callback| callback as usize),
	)
}

fn callbacks(data: usize) -> Callbacks {
	CALLBACKS.with_borrow(|callbacks| callbacks[data])
}

fn handler_bridge(args: rlay::TransitionArgs, data: usize) -> rlay::TransitionFrame {
	let frame = (callbacks(data).handler)(from_rlay_args(args));
	rlay::TransitionFrame {
		values: to_rlay_values(frame.values),
		complete: frame.complete,
	}
}

fn enter_bridge(
	values: rlay::TransitionValues,
	properties: rlay::TransitionProperties,
	data: usize,
) -> rlay::TransitionValues {
	callbacks(data).enter.map_or(values, |enter| {
		to_rlay_values(enter(
			from_rlay_values(values),
			TransitionProperties::from_bits_retain(properties.bits()),
		))
	})
}

fn exit_bridge(
	values: rlay::TransitionValues,
	properties: rlay::TransitionProperties,
	data: usize,
) -> rlay::TransitionValues {
	callbacks(data).exit.map_or(values, |exit| {
		to_rlay_values(exit(
			from_rlay_values(values),
			TransitionProperties::from_bits_retain(properties.bits()),
		))
	})
}

fn from_rlay_args(args: rlay::TransitionArgs) -> TransitionArgs {
	TransitionArgs {
		state: match args.state {
			rlay::TransitionState::Idle => TransitionState::Idle,
			rlay::TransitionState::Entering => TransitionState::Entering,
			rlay::TransitionState::Transitioning => TransitionState::Transitioning,
			rlay::TransitionState::Exiting => TransitionState::Exiting,
		},
		initial: from_rlay_values(args.initial),
		current: from_rlay_values(args.current),
		target: from_rlay_values(args.target),
		elapsed: args.elapsed,
		duration: args.duration,
		properties: TransitionProperties::from_bits_retain(args.properties.bits()),
	}
}

fn from_rlay_values(values: rlay::TransitionValues) -> TransitionValues {
	TransitionValues {
		bounds: TransitionBounds {
			x: values.bounds.x,
			y: values.bounds.y,
			width: values.bounds.width,
			height: values.bounds.height,
		},
		background: from_rlay_color(values.background),
		overlay: from_rlay_color(values.overlay),
		radius: TransitionRadius {
			top_left: values.radius.top_left,
			top_right: values.radius.top_right,
			bottom_left: values.radius.bottom_left,
			bottom_right: values.radius.bottom_right,
		},
		border_color: from_rlay_color(values.border_color),
		border_width: TransitionBorderWidth {
			left: values.border_width.left,
			right: values.border_width.right,
			top: values.border_width.top,
			bottom: values.border_width.bottom,
		},
	}
}

fn to_rlay_values(values: TransitionValues) -> rlay::TransitionValues {
	rlay::TransitionValues {
		bounds: rlay::Rect::new(
			values.bounds.x,
			values.bounds.y,
			values.bounds.width,
			values.bounds.height,
		),
		background: to_rlay_color(values.background),
		overlay: to_rlay_color(values.overlay),
		radius: rlay::Radius {
			top_left: values.radius.top_left,
			top_right: values.radius.top_right,
			bottom_left: values.radius.bottom_left,
			bottom_right: values.radius.bottom_right,
		},
		border_color: to_rlay_color(values.border_color),
		border_width: rlay::Padding::new(
			values.border_width.left,
			values.border_width.right,
			values.border_width.top,
			values.border_width.bottom,
		),
	}
}

fn from_rlay_color(color: rlay::Color) -> TransitionColor {
	TransitionColor {
		red: color.r,
		green: color.g,
		blue: color.b,
		alpha: color.a,
	}
}

fn to_rlay_color(color: TransitionColor) -> rlay::Color {
	rlay::Color::rgba(color.red, color.green, color.blue, color.alpha)
}

fn lerp(from: f32, to: f32, amount: f32) -> f32 {
	from + (to - from) * amount
}

fn lerp_color(from: TransitionColor, to: TransitionColor, amount: f32) -> TransitionColor {
	TransitionColor {
		red: lerp(from.red, to.red, amount),
		green: lerp(from.green, to.green, amount),
		blue: lerp(from.blue, to.blue, amount),
		alpha: lerp(from.alpha, to.alpha, amount),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn custom_handler(mut args: TransitionArgs) -> TransitionFrame {
		args.current.bounds.x = 42.0;
		TransitionFrame {
			values: args.current,
			complete: true,
		}
	}

	fn custom_enter(
		mut values: TransitionValues,
		_properties: TransitionProperties,
	) -> TransitionValues {
		values.bounds.y = 24.0;
		values
	}

	#[test]
	fn custom_callbacks_cross_the_private_rlay_adapter() {
		let transition = Transition {
			handler: custom_handler,
			enter: TransitionEnter {
				initial: Some(custom_enter),
				..TransitionEnter::default()
			},
			..Transition::ease_out(1.0, TransitionProperties::POSITION)
		}
		.into_rlay();
		let adapter = transition.adapter.unwrap();
		let values = rlay::TransitionValues::default();
		let entered =
			adapter.enter.unwrap()(values, rlay::TransitionProperties::POSITION, adapter.data);
		let frame = (adapter.handler)(
			rlay::TransitionArgs {
				state: rlay::TransitionState::Entering,
				initial: entered,
				current: entered,
				target: values,
				elapsed: 0.0,
				duration: 1.0,
				properties: rlay::TransitionProperties::POSITION,
			},
			adapter.data,
		);

		assert_eq!(entered.bounds.y, 24.0);
		assert_eq!(frame.values.bounds.x, 42.0);
		assert!(frame.complete);
	}
}
