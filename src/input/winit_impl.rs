use std::{
	cell::RefCell,
	collections::HashMap,
	sync::atomic::{AtomicBool, Ordering},
};

use winit::{
	event::{ElementState, Ime, KeyEvent},
	keyboard::{Key, ModifiersState},
};

use crate::input::{InputManager, Modifiers};

pub struct WinitInputManager {
	mouse_position: (f32, f32),
	mouse_buttons_current: HashMap<u16, bool>,
	mouse_buttons_previous: HashMap<u16, bool>,
	mouse_buttons_pressed: HashMap<u16, bool>,
	keys_current: HashMap<Key, bool>,
	keys_previous: HashMap<Key, bool>,
	keys_repeated: HashMap<Key, bool>,
	text_input: String,
	text_input_without_repeat: String,
	text_ime_buffer: String,
	text_ime_buffer_cursor: (usize, usize),
	ime_editing: bool,
	bytes_to_remove: (usize, usize),
	modifiers: Modifiers,
	has_clicked_on_something: AtomicBool,
	ime_requested: AtomicBool,
	ime_anchor: RefCell<Option<String>>,
}

#[cfg(feature = "winit")]
impl WinitInputManager {
	pub fn new() -> Self {
		Self {
			mouse_position: (0.0, 0.0),
			mouse_buttons_current: HashMap::new(),
			mouse_buttons_previous: HashMap::new(),
			mouse_buttons_pressed: HashMap::new(),
			keys_current: HashMap::new(),
			keys_previous: HashMap::new(),
			keys_repeated: HashMap::new(),
			text_input: String::new(),
			text_input_without_repeat: String::new(),
			text_ime_buffer: String::new(),
			text_ime_buffer_cursor: (0, 0),
			ime_editing: false,
			bytes_to_remove: (0, 0),
			modifiers: Modifiers::default(),
			has_clicked_on_something: Default::default(),
			ime_requested: Default::default(),
			ime_anchor: Default::default(),
		}
	}

	pub fn update(&mut self) {
		// Move current state to previous
		self.mouse_buttons_previous = self.mouse_buttons_current.clone();
		self.mouse_buttons_pressed = self.mouse_buttons_current.clone();
		self.mouse_buttons_pressed.clear();
		self.keys_previous = self.keys_current.clone();
		self.keys_repeated.clear();
		self.text_input.clear();
		self.text_input_without_repeat.clear();
		self.bytes_to_remove = (0, 0);
	}

	pub fn reset_ime_request(&self) {
		self.ime_requested.store(false, Ordering::Relaxed);
		*self.ime_anchor.borrow_mut() = None;
	}

	pub fn ime_requested(&self) -> bool {
		self.ime_requested.load(Ordering::Relaxed)
	}

	pub fn ime_anchor(&self) -> Option<String> {
		self.ime_anchor.borrow().clone()
	}

	pub fn set_mouse_position(&mut self, x: f32, y: f32) {
		self.mouse_position = (x, y);
	}

	pub fn set_mouse_button(&mut self, button: u16, pressed: bool) {
		self.mouse_buttons_current.insert(button, pressed);
		self.mouse_buttons_pressed.insert(button, pressed);
	}

	pub fn handle_key_event(&mut self, event: KeyEvent) {
		let pressed = match event.state {
			ElementState::Pressed => true,
			ElementState::Released => false,
		};

		if pressed {
			if event.repeat {
				self.keys_repeated.insert(event.logical_key.clone(), true);
			}

			if let Some(text) = &event.text {
				let printable = text
					.chars()
					.filter(|ch| !ch.is_control())
					.collect::<String>();
				self.text_input.push_str(&printable);
				if !event.repeat {
					self.text_input_without_repeat.push_str(&printable);
				}
			}
		}

		self.keys_current.insert(event.logical_key, pressed);
	}

	pub fn set_modifiers(&mut self, modifiers: ModifiersState) {
		self.modifiers = Modifiers {
			shift: modifiers.shift_key(),
			ctrl: modifiers.control_key(),
			alt: modifiers.alt_key(),
			super_key: modifiers.meta_key(),
		};
	}

	pub fn handle_ime_event(&mut self, ime: Ime) {
		match ime {
			Ime::Enabled => {
				self.ime_editing = false;
			}
			Ime::Preedit(new_preedit, cursor) => {
				self.text_ime_buffer_cursor = cursor.unwrap_or_default();
				self.text_ime_buffer = new_preedit;
				self.ime_editing = !self.text_ime_buffer.is_empty();
			}
			Ime::Commit(text) => {
				self.ime_editing = false;
				self.text_ime_buffer.clear();
				self.text_ime_buffer_cursor = (0, 0);
				self.text_input.push_str(&text);
				self.text_input_without_repeat.push_str(&text);
			}
			Ime::DeleteSurrounding {
				before_bytes,
				after_bytes,
			} => {
				self.bytes_to_remove.0 += before_bytes;
				self.bytes_to_remove.1 += after_bytes;
			}
			Ime::Disabled => {
				self.ime_editing = false;
				self.text_ime_buffer.clear();
				self.text_ime_buffer_cursor = (0, 0);
			}
		}
	}
}

#[cfg(feature = "winit")]
impl InputManager for WinitInputManager {
	fn cursor_hit_something(&self) -> bool {
		self
			.has_clicked_on_something
			.swap(false, std::sync::atomic::Ordering::Relaxed)
	}
	fn set_cursor_clicked_something(&self) {
		self
			.has_clicked_on_something
			.store(true, std::sync::atomic::Ordering::Relaxed);
	}
	fn mouse_position(&self) -> (f32, f32) {
		self.mouse_position
	}

	fn is_mouse_button_pressed(&self, button: u16) -> bool {
		self
			.mouse_buttons_current
			.get(&button)
			.copied()
			.unwrap_or(false)
	}

	fn is_mouse_button_just_pressed(&self, button: u16) -> bool {
		let current = self
			.mouse_buttons_current
			.get(&button)
			.copied()
			.unwrap_or(false);
		let previous = self
			.mouse_buttons_previous
			.get(&button)
			.copied()
			.unwrap_or(false);
		(current && !previous)
			|| self
				.mouse_buttons_pressed
				.get(&button)
				.map_or(false, |&b| b)
	}

	fn is_mouse_button_just_released(&self, button: u16) -> bool {
		let current = self
			.mouse_buttons_current
			.get(&button)
			.copied()
			.unwrap_or(false);
		let previous = self
			.mouse_buttons_previous
			.get(&button)
			.copied()
			.unwrap_or(false);
		(!current && previous)
			|| self
				.mouse_buttons_pressed
				.get(&button)
				.map_or(false, |&b| !b)
	}

	fn is_key_pressed(&self, key: crate::Key) -> bool {
		let key = to_winit_key(key);
		self.keys_current.get(&key).copied().unwrap_or(false)
	}

	fn is_key_just_pressed(&self, key: crate::Key) -> bool {
		let key = to_winit_key(key);
		let current = self.keys_current.get(&key).copied().unwrap_or(false);
		let previous = self.keys_previous.get(&key).copied().unwrap_or(false);
		current && !previous
	}

	fn is_key_repeated(&self, key: crate::Key) -> bool {
		let key = to_winit_key(key);
		self.keys_repeated.get(&key).copied().unwrap_or(false)
	}

	fn is_key_just_released(&self, key: crate::Key) -> bool {
		let key = to_winit_key(key);
		let current = self.keys_current.get(&key).copied().unwrap_or(false);
		let previous = self.keys_previous.get(&key).copied().unwrap_or(false);
		!current && previous
	}

	fn modifiers(&self) -> Modifiers {
		self.modifiers
	}

	fn text_input(&self) -> &str {
		&self.text_input
	}

	fn text_input_without_repeat(&self) -> &str {
		&self.text_input_without_repeat
	}

	fn ime_buffer(&self) -> &str {
		&self.text_ime_buffer
	}

	fn ime_is_editing(&self) -> bool {
		self.ime_editing
	}

	fn request_ime(&self) {
		self.ime_requested.store(true, Ordering::Relaxed);
	}

	fn set_ime_anchor(&self, id: &str) {
		*self.ime_anchor.borrow_mut() = Some(id.to_string());
	}

	fn bytes_to_remove(&self) -> (usize, usize) {
		self.bytes_to_remove
	}
}

pub fn to_winit_key(key: crate::Key) -> Key {
	match key {
		crate::Key::Named(named_key) => winit::keyboard::Key::Named(match named_key {
			crate::NamedKey::Alt => winit::keyboard::NamedKey::Alt,
			crate::NamedKey::AltGraph => winit::keyboard::NamedKey::AltGraph,
			crate::NamedKey::CapsLock => winit::keyboard::NamedKey::CapsLock,
			crate::NamedKey::Control => winit::keyboard::NamedKey::Control,
			crate::NamedKey::Fn => winit::keyboard::NamedKey::Fn,
			crate::NamedKey::FnLock => winit::keyboard::NamedKey::FnLock,
			crate::NamedKey::NumLock => winit::keyboard::NamedKey::NumLock,
			crate::NamedKey::ScrollLock => winit::keyboard::NamedKey::ScrollLock,
			crate::NamedKey::Shift => winit::keyboard::NamedKey::Shift,
			crate::NamedKey::Symbol => winit::keyboard::NamedKey::Symbol,
			crate::NamedKey::SymbolLock => winit::keyboard::NamedKey::SymbolLock,
			crate::NamedKey::Meta => winit::keyboard::NamedKey::Meta,
			crate::NamedKey::Hyper => winit::keyboard::NamedKey::Meta,
			crate::NamedKey::Super => winit::keyboard::NamedKey::Meta,
			crate::NamedKey::Enter => winit::keyboard::NamedKey::Enter,
			crate::NamedKey::Tab => winit::keyboard::NamedKey::Tab,
			crate::NamedKey::ArrowDown => winit::keyboard::NamedKey::ArrowDown,
			crate::NamedKey::ArrowLeft => winit::keyboard::NamedKey::ArrowLeft,
			crate::NamedKey::ArrowRight => winit::keyboard::NamedKey::ArrowRight,
			crate::NamedKey::ArrowUp => winit::keyboard::NamedKey::ArrowUp,
			crate::NamedKey::End => winit::keyboard::NamedKey::End,
			crate::NamedKey::Home => winit::keyboard::NamedKey::Home,
			crate::NamedKey::PageDown => winit::keyboard::NamedKey::PageDown,
			crate::NamedKey::PageUp => winit::keyboard::NamedKey::PageUp,
			crate::NamedKey::Backspace => winit::keyboard::NamedKey::Backspace,
			crate::NamedKey::Clear => winit::keyboard::NamedKey::Clear,
			crate::NamedKey::Copy => winit::keyboard::NamedKey::Copy,
			crate::NamedKey::CrSel => winit::keyboard::NamedKey::CrSel,
			crate::NamedKey::Cut => winit::keyboard::NamedKey::Cut,
			crate::NamedKey::Delete => winit::keyboard::NamedKey::Delete,
			crate::NamedKey::EraseEof => winit::keyboard::NamedKey::EraseEof,
			crate::NamedKey::ExSel => winit::keyboard::NamedKey::ExSel,
			crate::NamedKey::Insert => winit::keyboard::NamedKey::Insert,
			crate::NamedKey::Paste => winit::keyboard::NamedKey::Paste,
			crate::NamedKey::Redo => winit::keyboard::NamedKey::Redo,
			crate::NamedKey::Undo => winit::keyboard::NamedKey::Undo,
			crate::NamedKey::Accept => winit::keyboard::NamedKey::Accept,
			crate::NamedKey::Again => winit::keyboard::NamedKey::Again,
			crate::NamedKey::Attn => winit::keyboard::NamedKey::Attn,
			crate::NamedKey::Cancel => winit::keyboard::NamedKey::Cancel,
			crate::NamedKey::ContextMenu => winit::keyboard::NamedKey::ContextMenu,
			crate::NamedKey::Escape => winit::keyboard::NamedKey::Escape,
			crate::NamedKey::Execute => winit::keyboard::NamedKey::Execute,
			crate::NamedKey::Find => winit::keyboard::NamedKey::Find,
			crate::NamedKey::Help => winit::keyboard::NamedKey::Help,
			crate::NamedKey::Pause => winit::keyboard::NamedKey::Pause,
			crate::NamedKey::Play => winit::keyboard::NamedKey::Play,
			crate::NamedKey::Props => winit::keyboard::NamedKey::Props,
			crate::NamedKey::Select => winit::keyboard::NamedKey::Select,
			crate::NamedKey::ZoomIn => winit::keyboard::NamedKey::ZoomIn,
			crate::NamedKey::ZoomOut => winit::keyboard::NamedKey::ZoomOut,
			crate::NamedKey::BrightnessDown => winit::keyboard::NamedKey::BrightnessDown,
			crate::NamedKey::BrightnessUp => winit::keyboard::NamedKey::BrightnessUp,
			crate::NamedKey::Eject => winit::keyboard::NamedKey::Eject,
			crate::NamedKey::LogOff => winit::keyboard::NamedKey::LogOff,
			crate::NamedKey::Power => winit::keyboard::NamedKey::Power,
			crate::NamedKey::PowerOff => winit::keyboard::NamedKey::PowerOff,
			crate::NamedKey::PrintScreen => winit::keyboard::NamedKey::PrintScreen,
			crate::NamedKey::Hibernate => winit::keyboard::NamedKey::Hibernate,
			crate::NamedKey::Standby => winit::keyboard::NamedKey::Standby,
			crate::NamedKey::WakeUp => winit::keyboard::NamedKey::WakeUp,
			crate::NamedKey::AllCandidates => winit::keyboard::NamedKey::AllCandidates,
			crate::NamedKey::Alphanumeric => winit::keyboard::NamedKey::Alphanumeric,
			crate::NamedKey::CodeInput => winit::keyboard::NamedKey::CodeInput,
			crate::NamedKey::Compose => winit::keyboard::NamedKey::Compose,
			crate::NamedKey::Convert => winit::keyboard::NamedKey::Convert,
			crate::NamedKey::FinalMode => winit::keyboard::NamedKey::FinalMode,
			crate::NamedKey::GroupFirst => winit::keyboard::NamedKey::GroupFirst,
			crate::NamedKey::GroupLast => winit::keyboard::NamedKey::GroupLast,
			crate::NamedKey::GroupNext => winit::keyboard::NamedKey::GroupNext,
			crate::NamedKey::GroupPrevious => winit::keyboard::NamedKey::GroupPrevious,
			crate::NamedKey::ModeChange => winit::keyboard::NamedKey::ModeChange,
			crate::NamedKey::NextCandidate => winit::keyboard::NamedKey::NextCandidate,
			crate::NamedKey::NonConvert => winit::keyboard::NamedKey::NonConvert,
			crate::NamedKey::PreviousCandidate => winit::keyboard::NamedKey::PreviousCandidate,
			crate::NamedKey::Process => winit::keyboard::NamedKey::Process,
			crate::NamedKey::SingleCandidate => winit::keyboard::NamedKey::SingleCandidate,
			crate::NamedKey::HangulMode => winit::keyboard::NamedKey::HangulMode,
			crate::NamedKey::HanjaMode => winit::keyboard::NamedKey::HanjaMode,
			crate::NamedKey::JunjaMode => winit::keyboard::NamedKey::JunjaMode,
			crate::NamedKey::Eisu => winit::keyboard::NamedKey::Eisu,
			crate::NamedKey::Hankaku => winit::keyboard::NamedKey::Hankaku,
			crate::NamedKey::Hiragana => winit::keyboard::NamedKey::Hiragana,
			crate::NamedKey::HiraganaKatakana => winit::keyboard::NamedKey::HiraganaKatakana,
			crate::NamedKey::KanaMode => winit::keyboard::NamedKey::KanaMode,
			crate::NamedKey::KanjiMode => winit::keyboard::NamedKey::KanjiMode,
			crate::NamedKey::Katakana => winit::keyboard::NamedKey::Katakana,
			crate::NamedKey::Romaji => winit::keyboard::NamedKey::Romaji,
			crate::NamedKey::Zenkaku => winit::keyboard::NamedKey::Zenkaku,
			crate::NamedKey::ZenkakuHankaku => winit::keyboard::NamedKey::ZenkakuHankaku,
			crate::NamedKey::Soft1 => winit::keyboard::NamedKey::Soft1,
			crate::NamedKey::Soft2 => winit::keyboard::NamedKey::Soft2,
			crate::NamedKey::Soft3 => winit::keyboard::NamedKey::Soft3,
			crate::NamedKey::Soft4 => winit::keyboard::NamedKey::Soft4,
			crate::NamedKey::ChannelDown => winit::keyboard::NamedKey::ChannelDown,
			crate::NamedKey::ChannelUp => winit::keyboard::NamedKey::ChannelUp,
			crate::NamedKey::Close => winit::keyboard::NamedKey::Close,
			crate::NamedKey::MailForward => winit::keyboard::NamedKey::MailForward,
			crate::NamedKey::MailReply => winit::keyboard::NamedKey::MailReply,
			crate::NamedKey::MailSend => winit::keyboard::NamedKey::MailSend,
			crate::NamedKey::MediaClose => winit::keyboard::NamedKey::MediaClose,
			crate::NamedKey::MediaFastForward => winit::keyboard::NamedKey::MediaFastForward,
			crate::NamedKey::MediaPause => winit::keyboard::NamedKey::MediaPause,
			crate::NamedKey::MediaPlay => winit::keyboard::NamedKey::MediaPlay,
			crate::NamedKey::MediaPlayPause => winit::keyboard::NamedKey::MediaPlayPause,
			crate::NamedKey::MediaRecord => winit::keyboard::NamedKey::MediaRecord,
			crate::NamedKey::MediaRewind => winit::keyboard::NamedKey::MediaRewind,
			crate::NamedKey::MediaStop => winit::keyboard::NamedKey::MediaStop,
			crate::NamedKey::MediaTrackNext => winit::keyboard::NamedKey::MediaTrackNext,
			crate::NamedKey::MediaTrackPrevious => winit::keyboard::NamedKey::MediaTrackPrevious,
			crate::NamedKey::New => winit::keyboard::NamedKey::New,
			crate::NamedKey::Open => winit::keyboard::NamedKey::Open,
			crate::NamedKey::Print => winit::keyboard::NamedKey::Print,
			crate::NamedKey::Save => winit::keyboard::NamedKey::Save,
			crate::NamedKey::SpellCheck => winit::keyboard::NamedKey::SpellCheck,
			crate::NamedKey::Key11 => winit::keyboard::NamedKey::Key11,
			crate::NamedKey::Key12 => winit::keyboard::NamedKey::Key12,
			crate::NamedKey::AudioBalanceLeft => winit::keyboard::NamedKey::AudioBalanceLeft,
			crate::NamedKey::AudioBalanceRight => winit::keyboard::NamedKey::AudioBalanceRight,
			crate::NamedKey::AudioBassBoostDown => winit::keyboard::NamedKey::AudioBassBoostDown,
			crate::NamedKey::AudioBassBoostToggle => winit::keyboard::NamedKey::AudioBassBoostToggle,
			crate::NamedKey::AudioBassBoostUp => winit::keyboard::NamedKey::AudioBassBoostUp,
			crate::NamedKey::AudioFaderFront => winit::keyboard::NamedKey::AudioFaderFront,
			crate::NamedKey::AudioFaderRear => winit::keyboard::NamedKey::AudioFaderRear,
			crate::NamedKey::AudioSurroundModeNext => winit::keyboard::NamedKey::AudioSurroundModeNext,
			crate::NamedKey::AudioTrebleDown => winit::keyboard::NamedKey::AudioTrebleDown,
			crate::NamedKey::AudioTrebleUp => winit::keyboard::NamedKey::AudioTrebleUp,
			crate::NamedKey::AudioVolumeDown => winit::keyboard::NamedKey::AudioVolumeDown,
			crate::NamedKey::AudioVolumeUp => winit::keyboard::NamedKey::AudioVolumeUp,
			crate::NamedKey::AudioVolumeMute => winit::keyboard::NamedKey::AudioVolumeMute,
			crate::NamedKey::MicrophoneToggle => winit::keyboard::NamedKey::MicrophoneToggle,
			crate::NamedKey::MicrophoneVolumeDown => winit::keyboard::NamedKey::MicrophoneVolumeDown,
			crate::NamedKey::MicrophoneVolumeUp => winit::keyboard::NamedKey::MicrophoneVolumeUp,
			crate::NamedKey::MicrophoneVolumeMute => winit::keyboard::NamedKey::MicrophoneVolumeMute,
			crate::NamedKey::SpeechCorrectionList => winit::keyboard::NamedKey::SpeechCorrectionList,
			crate::NamedKey::SpeechInputToggle => winit::keyboard::NamedKey::SpeechInputToggle,
			crate::NamedKey::LaunchApplication1 => winit::keyboard::NamedKey::LaunchApplication1,
			crate::NamedKey::LaunchApplication2 => winit::keyboard::NamedKey::LaunchApplication2,
			crate::NamedKey::LaunchCalendar => winit::keyboard::NamedKey::LaunchCalendar,
			crate::NamedKey::LaunchContacts => winit::keyboard::NamedKey::LaunchContacts,
			crate::NamedKey::LaunchMail => winit::keyboard::NamedKey::LaunchMail,
			crate::NamedKey::LaunchMediaPlayer => winit::keyboard::NamedKey::LaunchMediaPlayer,
			crate::NamedKey::LaunchMusicPlayer => winit::keyboard::NamedKey::LaunchMusicPlayer,
			crate::NamedKey::LaunchPhone => winit::keyboard::NamedKey::LaunchPhone,
			crate::NamedKey::LaunchScreenSaver => winit::keyboard::NamedKey::LaunchScreenSaver,
			crate::NamedKey::LaunchSpreadsheet => winit::keyboard::NamedKey::LaunchSpreadsheet,
			crate::NamedKey::LaunchWebBrowser => winit::keyboard::NamedKey::LaunchWebBrowser,
			crate::NamedKey::LaunchWebCam => winit::keyboard::NamedKey::LaunchWebCam,
			crate::NamedKey::LaunchWordProcessor => winit::keyboard::NamedKey::LaunchWordProcessor,
			crate::NamedKey::BrowserBack => winit::keyboard::NamedKey::BrowserBack,
			crate::NamedKey::BrowserFavorites => winit::keyboard::NamedKey::BrowserFavorites,
			crate::NamedKey::BrowserForward => winit::keyboard::NamedKey::BrowserForward,
			crate::NamedKey::BrowserHome => winit::keyboard::NamedKey::BrowserHome,
			crate::NamedKey::BrowserRefresh => winit::keyboard::NamedKey::BrowserRefresh,
			crate::NamedKey::BrowserSearch => winit::keyboard::NamedKey::BrowserSearch,
			crate::NamedKey::BrowserStop => winit::keyboard::NamedKey::BrowserStop,
			crate::NamedKey::AppSwitch => winit::keyboard::NamedKey::AppSwitch,
			crate::NamedKey::Call => winit::keyboard::NamedKey::Call,
			crate::NamedKey::Camera => winit::keyboard::NamedKey::Camera,
			crate::NamedKey::CameraFocus => winit::keyboard::NamedKey::CameraFocus,
			crate::NamedKey::EndCall => winit::keyboard::NamedKey::EndCall,
			crate::NamedKey::GoBack => winit::keyboard::NamedKey::GoBack,
			crate::NamedKey::GoHome => winit::keyboard::NamedKey::GoHome,
			crate::NamedKey::HeadsetHook => winit::keyboard::NamedKey::HeadsetHook,
			crate::NamedKey::LastNumberRedial => winit::keyboard::NamedKey::LastNumberRedial,
			crate::NamedKey::Notification => winit::keyboard::NamedKey::Notification,
			crate::NamedKey::MannerMode => winit::keyboard::NamedKey::MannerMode,
			crate::NamedKey::VoiceDial => winit::keyboard::NamedKey::VoiceDial,
			crate::NamedKey::TV => winit::keyboard::NamedKey::TV,
			crate::NamedKey::TV3DMode => winit::keyboard::NamedKey::TV3DMode,
			crate::NamedKey::TVAntennaCable => winit::keyboard::NamedKey::TVAntennaCable,
			crate::NamedKey::TVAudioDescription => winit::keyboard::NamedKey::TVAudioDescription,
			crate::NamedKey::TVAudioDescriptionMixDown => {
				winit::keyboard::NamedKey::TVAudioDescriptionMixDown
			}
			crate::NamedKey::TVAudioDescriptionMixUp => {
				winit::keyboard::NamedKey::TVAudioDescriptionMixUp
			}
			crate::NamedKey::TVContentsMenu => winit::keyboard::NamedKey::TVContentsMenu,
			crate::NamedKey::TVDataService => winit::keyboard::NamedKey::TVDataService,
			crate::NamedKey::TVInput => winit::keyboard::NamedKey::TVInput,
			crate::NamedKey::TVInputComponent1 => winit::keyboard::NamedKey::TVInputComponent1,
			crate::NamedKey::TVInputComponent2 => winit::keyboard::NamedKey::TVInputComponent2,
			crate::NamedKey::TVInputComposite1 => winit::keyboard::NamedKey::TVInputComposite1,
			crate::NamedKey::TVInputComposite2 => winit::keyboard::NamedKey::TVInputComposite2,
			crate::NamedKey::TVInputHDMI1 => winit::keyboard::NamedKey::TVInputHDMI1,
			crate::NamedKey::TVInputHDMI2 => winit::keyboard::NamedKey::TVInputHDMI2,
			crate::NamedKey::TVInputHDMI3 => winit::keyboard::NamedKey::TVInputHDMI3,
			crate::NamedKey::TVInputHDMI4 => winit::keyboard::NamedKey::TVInputHDMI4,
			crate::NamedKey::TVInputVGA1 => winit::keyboard::NamedKey::TVInputVGA1,
			crate::NamedKey::TVMediaContext => winit::keyboard::NamedKey::TVMediaContext,
			crate::NamedKey::TVNetwork => winit::keyboard::NamedKey::TVNetwork,
			crate::NamedKey::TVNumberEntry => winit::keyboard::NamedKey::TVNumberEntry,
			crate::NamedKey::TVPower => winit::keyboard::NamedKey::TVPower,
			crate::NamedKey::TVRadioService => winit::keyboard::NamedKey::TVRadioService,
			crate::NamedKey::TVSatellite => winit::keyboard::NamedKey::TVSatellite,
			crate::NamedKey::TVSatelliteBS => winit::keyboard::NamedKey::TVSatelliteBS,
			crate::NamedKey::TVSatelliteCS => winit::keyboard::NamedKey::TVSatelliteCS,
			crate::NamedKey::TVSatelliteToggle => winit::keyboard::NamedKey::TVSatelliteToggle,
			crate::NamedKey::TVTerrestrialAnalog => winit::keyboard::NamedKey::TVTerrestrialAnalog,
			crate::NamedKey::TVTerrestrialDigital => winit::keyboard::NamedKey::TVTerrestrialDigital,
			crate::NamedKey::TVTimer => winit::keyboard::NamedKey::TVTimer,
			crate::NamedKey::AVRInput => winit::keyboard::NamedKey::AVRInput,
			crate::NamedKey::AVRPower => winit::keyboard::NamedKey::AVRPower,
			crate::NamedKey::ColorF0Red => winit::keyboard::NamedKey::ColorF0Red,
			crate::NamedKey::ColorF1Green => winit::keyboard::NamedKey::ColorF1Green,
			crate::NamedKey::ColorF2Yellow => winit::keyboard::NamedKey::ColorF2Yellow,
			crate::NamedKey::ColorF3Blue => winit::keyboard::NamedKey::ColorF3Blue,
			crate::NamedKey::ColorF4Grey => winit::keyboard::NamedKey::ColorF4Grey,
			crate::NamedKey::ColorF5Brown => winit::keyboard::NamedKey::ColorF5Brown,
			crate::NamedKey::ClosedCaptionToggle => winit::keyboard::NamedKey::ClosedCaptionToggle,
			crate::NamedKey::Dimmer => winit::keyboard::NamedKey::Dimmer,
			crate::NamedKey::DisplaySwap => winit::keyboard::NamedKey::DisplaySwap,
			crate::NamedKey::DVR => winit::keyboard::NamedKey::DVR,
			crate::NamedKey::Exit => winit::keyboard::NamedKey::Exit,
			crate::NamedKey::FavoriteClear0 => winit::keyboard::NamedKey::FavoriteClear0,
			crate::NamedKey::FavoriteClear1 => winit::keyboard::NamedKey::FavoriteClear1,
			crate::NamedKey::FavoriteClear2 => winit::keyboard::NamedKey::FavoriteClear2,
			crate::NamedKey::FavoriteClear3 => winit::keyboard::NamedKey::FavoriteClear3,
			crate::NamedKey::FavoriteRecall0 => winit::keyboard::NamedKey::FavoriteRecall0,
			crate::NamedKey::FavoriteRecall1 => winit::keyboard::NamedKey::FavoriteRecall1,
			crate::NamedKey::FavoriteRecall2 => winit::keyboard::NamedKey::FavoriteRecall2,
			crate::NamedKey::FavoriteRecall3 => winit::keyboard::NamedKey::FavoriteRecall3,
			crate::NamedKey::FavoriteStore0 => winit::keyboard::NamedKey::FavoriteStore0,
			crate::NamedKey::FavoriteStore1 => winit::keyboard::NamedKey::FavoriteStore1,
			crate::NamedKey::FavoriteStore2 => winit::keyboard::NamedKey::FavoriteStore2,
			crate::NamedKey::FavoriteStore3 => winit::keyboard::NamedKey::FavoriteStore3,
			crate::NamedKey::Guide => winit::keyboard::NamedKey::Guide,
			crate::NamedKey::GuideNextDay => winit::keyboard::NamedKey::GuideNextDay,
			crate::NamedKey::GuidePreviousDay => winit::keyboard::NamedKey::GuidePreviousDay,
			crate::NamedKey::Info => winit::keyboard::NamedKey::Info,
			crate::NamedKey::InstantReplay => winit::keyboard::NamedKey::InstantReplay,
			crate::NamedKey::Link => winit::keyboard::NamedKey::Link,
			crate::NamedKey::ListProgram => winit::keyboard::NamedKey::ListProgram,
			crate::NamedKey::LiveContent => winit::keyboard::NamedKey::LiveContent,
			crate::NamedKey::Lock => winit::keyboard::NamedKey::Lock,
			crate::NamedKey::MediaApps => winit::keyboard::NamedKey::MediaApps,
			crate::NamedKey::MediaAudioTrack => winit::keyboard::NamedKey::MediaAudioTrack,
			crate::NamedKey::MediaLast => winit::keyboard::NamedKey::MediaLast,
			crate::NamedKey::MediaSkipBackward => winit::keyboard::NamedKey::MediaSkipBackward,
			crate::NamedKey::MediaSkipForward => winit::keyboard::NamedKey::MediaSkipForward,
			crate::NamedKey::MediaStepBackward => winit::keyboard::NamedKey::MediaStepBackward,
			crate::NamedKey::MediaStepForward => winit::keyboard::NamedKey::MediaStepForward,
			crate::NamedKey::MediaTopMenu => winit::keyboard::NamedKey::MediaTopMenu,
			crate::NamedKey::NavigateIn => winit::keyboard::NamedKey::NavigateIn,
			crate::NamedKey::NavigateNext => winit::keyboard::NamedKey::NavigateNext,
			crate::NamedKey::NavigateOut => winit::keyboard::NamedKey::NavigateOut,
			crate::NamedKey::NavigatePrevious => winit::keyboard::NamedKey::NavigatePrevious,
			crate::NamedKey::NextFavoriteChannel => winit::keyboard::NamedKey::NextFavoriteChannel,
			crate::NamedKey::NextUserProfile => winit::keyboard::NamedKey::NextUserProfile,
			crate::NamedKey::OnDemand => winit::keyboard::NamedKey::OnDemand,
			crate::NamedKey::Pairing => winit::keyboard::NamedKey::Pairing,
			crate::NamedKey::PinPDown => winit::keyboard::NamedKey::PinPDown,
			crate::NamedKey::PinPMove => winit::keyboard::NamedKey::PinPMove,
			crate::NamedKey::PinPToggle => winit::keyboard::NamedKey::PinPToggle,
			crate::NamedKey::PinPUp => winit::keyboard::NamedKey::PinPUp,
			crate::NamedKey::PlaySpeedDown => winit::keyboard::NamedKey::PlaySpeedDown,
			crate::NamedKey::PlaySpeedReset => winit::keyboard::NamedKey::PlaySpeedReset,
			crate::NamedKey::PlaySpeedUp => winit::keyboard::NamedKey::PlaySpeedUp,
			crate::NamedKey::RandomToggle => winit::keyboard::NamedKey::RandomToggle,
			crate::NamedKey::RcLowBattery => winit::keyboard::NamedKey::RcLowBattery,
			crate::NamedKey::RecordSpeedNext => winit::keyboard::NamedKey::RecordSpeedNext,
			crate::NamedKey::RfBypass => winit::keyboard::NamedKey::RfBypass,
			crate::NamedKey::ScanChannelsToggle => winit::keyboard::NamedKey::ScanChannelsToggle,
			crate::NamedKey::ScreenModeNext => winit::keyboard::NamedKey::ScreenModeNext,
			crate::NamedKey::Settings => winit::keyboard::NamedKey::Settings,
			crate::NamedKey::SplitScreenToggle => winit::keyboard::NamedKey::SplitScreenToggle,
			crate::NamedKey::STBInput => winit::keyboard::NamedKey::STBInput,
			crate::NamedKey::STBPower => winit::keyboard::NamedKey::STBPower,
			crate::NamedKey::Subtitle => winit::keyboard::NamedKey::Subtitle,
			crate::NamedKey::Teletext => winit::keyboard::NamedKey::Teletext,
			crate::NamedKey::VideoModeNext => winit::keyboard::NamedKey::VideoModeNext,
			crate::NamedKey::Wink => winit::keyboard::NamedKey::Wink,
			crate::NamedKey::ZoomToggle => winit::keyboard::NamedKey::ZoomToggle,
			crate::NamedKey::F1 => winit::keyboard::NamedKey::F1,
			crate::NamedKey::F2 => winit::keyboard::NamedKey::F2,
			crate::NamedKey::F3 => winit::keyboard::NamedKey::F3,
			crate::NamedKey::F4 => winit::keyboard::NamedKey::F4,
			crate::NamedKey::F5 => winit::keyboard::NamedKey::F5,
			crate::NamedKey::F6 => winit::keyboard::NamedKey::F6,
			crate::NamedKey::F7 => winit::keyboard::NamedKey::F7,
			crate::NamedKey::F8 => winit::keyboard::NamedKey::F8,
			crate::NamedKey::F9 => winit::keyboard::NamedKey::F9,
			crate::NamedKey::F10 => winit::keyboard::NamedKey::F10,
			crate::NamedKey::F11 => winit::keyboard::NamedKey::F11,
			crate::NamedKey::F12 => winit::keyboard::NamedKey::F12,
			crate::NamedKey::F13 => winit::keyboard::NamedKey::F13,
			crate::NamedKey::F14 => winit::keyboard::NamedKey::F14,
			crate::NamedKey::F15 => winit::keyboard::NamedKey::F15,
			crate::NamedKey::F16 => winit::keyboard::NamedKey::F16,
			crate::NamedKey::F17 => winit::keyboard::NamedKey::F17,
			crate::NamedKey::F18 => winit::keyboard::NamedKey::F18,
			crate::NamedKey::F19 => winit::keyboard::NamedKey::F19,
			crate::NamedKey::F20 => winit::keyboard::NamedKey::F20,
			crate::NamedKey::F21 => winit::keyboard::NamedKey::F21,
			crate::NamedKey::F22 => winit::keyboard::NamedKey::F22,
			crate::NamedKey::F23 => winit::keyboard::NamedKey::F23,
			crate::NamedKey::F24 => winit::keyboard::NamedKey::F24,
			crate::NamedKey::F25 => winit::keyboard::NamedKey::F25,
			crate::NamedKey::F26 => winit::keyboard::NamedKey::F26,
			crate::NamedKey::F27 => winit::keyboard::NamedKey::F27,
			crate::NamedKey::F28 => winit::keyboard::NamedKey::F28,
			crate::NamedKey::F29 => winit::keyboard::NamedKey::F29,
			crate::NamedKey::F30 => winit::keyboard::NamedKey::F30,
			crate::NamedKey::F31 => winit::keyboard::NamedKey::F31,
			crate::NamedKey::F32 => winit::keyboard::NamedKey::F32,
			crate::NamedKey::F33 => winit::keyboard::NamedKey::F33,
			crate::NamedKey::F34 => winit::keyboard::NamedKey::F34,
			crate::NamedKey::F35 => winit::keyboard::NamedKey::F35,
		}),
		crate::Key::Character(c) => winit::keyboard::Key::Character(c),
		crate::Key::Unidentified => {
			winit::keyboard::Key::Unidentified(winit::keyboard::NativeKey::Unidentified)
		}
		crate::Key::Dead(c) => winit::keyboard::Key::Dead(c),
	}
}
