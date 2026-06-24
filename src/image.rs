use std::{
	collections::{HashMap, hash_map::DefaultHasher}, fmt, hash::{Hash, Hasher}, path::{Path, PathBuf}, sync::{Arc, OnceLock, mpsc}, thread, time::Duration,
};

use skia_safe::{Data, FontMgr, Image as SkiaImage, svg::Dom};

use crate::{GlobalClosure, REQUEST_REDRAW};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageKey(Arc<str>);

impl ImageKey {
	pub fn new(key: impl Into<Arc<str>>) -> Self {
		Self(key.into())
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageHandle(u64);

impl ImageHandle {
	pub const fn id(self) -> u64 {
		self.0
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageError {
	message: Arc<str>,
}

impl ImageError {
	pub fn new(message: impl Into<Arc<str>>) -> Self {
		Self {
			message: message.into(),
		}
	}
}

impl fmt::Display for ImageError {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter.write_str(&self.message)
	}
}

impl std::error::Error for ImageError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageProviderState {
	Loading,
	Ready(ImageHandle),
	Error(ImageError),
}

pub trait ImageProviderBuilder: Clone + 'static {
	type Instance: ImageProviderInstance;

	fn key(&self) -> ImageKey;
	fn build(&self, ctx: &ImageProviderContext<'_>) -> Self::Instance;
}

pub trait ImageProviderInstance: 'static {
	fn poll(&mut self, ctx: &mut ImageProviderPollContext<'_>) -> ImageProviderState;
}

pub struct ImageProviderContext<'a> {
	manager: &'a ImageManager,
}

impl ImageProviderContext<'_> {
	pub fn cached(&self, key: &ImageKey) -> Option<ImageHandle> {
		self.manager.cached(key)
	}
}

pub struct ImageProviderPollContext<'a> {
	manager: &'a mut ImageManager,
}

impl ImageProviderPollContext<'_> {
	pub fn cached(&self, key: &ImageKey) -> Option<ImageHandle> {
		self.manager.cached(key)
	}

	pub fn store(&mut self, key: ImageKey, bytes: Arc<[u8]>) -> ImageHandle {
		self
			.manager
			.store_loaded(key, load_image(bytes))
			.unwrap_or(ImageHandle(0))
	}

	pub fn store_image(&mut self, key: ImageKey, image: SkiaImage) -> ImageHandle {
		self
			.manager
			.store_loaded(key, LoadedImage::Raster(image))
			.unwrap_or(ImageHandle(0))
	}

	pub fn request_redraw(&self) {
		REQUEST_REDRAW.call();
	}
}

pub(crate) trait DynImageProviderBuilder {
	fn key(&self) -> ImageKey;
	fn build(&self, ctx: &ImageProviderContext<'_>) -> Box<dyn ImageProviderInstance>;
}

pub(crate) struct ImageProvider<P>(pub P);

impl<P: ImageProviderBuilder> DynImageProviderBuilder for ImageProvider<P> {
	fn key(&self) -> ImageKey {
		self.0.key()
	}

	fn build(&self, ctx: &ImageProviderContext<'_>) -> Box<dyn ImageProviderInstance> {
		Box::new(self.0.build(ctx))
	}
}

#[derive(Debug, Clone)]
pub struct AssetImage {
	path: PathBuf,
}

impl AssetImage {
	pub fn new(path: impl Into<PathBuf>) -> Self {
		Self { path: path.into() }
	}
}

impl ImageProviderBuilder for AssetImage {
	type Instance = FileImageInstance;

	fn key(&self) -> ImageKey {
		ImageKey::new(format!("asset:{}", self.path.display()))
	}

	fn build(&self, ctx: &ImageProviderContext<'_>) -> Self::Instance {
		FileImage::new(&self.path).build_with_key(ctx, self.key())
	}
}

#[derive(Debug, Clone)]
pub struct FileImage {
	path: PathBuf,
}

impl FileImage {
	pub fn new(path: impl Into<PathBuf>) -> Self {
		Self { path: path.into() }
	}

	fn build_with_key(&self, ctx: &ImageProviderContext<'_>, key: ImageKey) -> FileImageInstance {
		if let Some(handle) = ctx.cached(&key) {
			return FileImageInstance {
				key,
				load: ImageLoad::Ready(handle),
			};
		}

		let path = self.path.clone();
		let (sender, receiver) = mpsc::channel();
		thread::spawn(move || {
			let result = std::fs::read(&path)
				.map_err(|error| {
					ImageError::new(format!("failed to load image {}: {error}", path.display()))
				})
				.map(Arc::<[u8]>::from)
				.map(load_image);
			let _ = sender.send(result);
		});

		FileImageInstance {
			key,
			load: ImageLoad::Pending(receiver),
		}
	}
}

impl ImageProviderBuilder for FileImage {
	type Instance = FileImageInstance;

	fn key(&self) -> ImageKey {
		ImageKey::new(format!("file:{}", self.path.display()))
	}

	fn build(&self, ctx: &ImageProviderContext<'_>) -> Self::Instance {
		self.build_with_key(ctx, self.key())
	}
}

pub struct FileImageInstance {
	key: ImageKey,
	load: ImageLoad,
}

enum ImageLoad {
	Pending(mpsc::Receiver<Result<LoadedImage, ImageError>>),
	Ready(ImageHandle),
	Error(ImageError),
}

impl ImageProviderInstance for FileImageInstance {
	fn poll(&mut self, ctx: &mut ImageProviderPollContext<'_>) -> ImageProviderState {
		match &self.load {
			ImageLoad::Pending(receiver) => match receiver.try_recv() {
				Ok(Ok(image)) => match ctx.manager.store_loaded(self.key.clone(), image) {
					Ok(handle) => {
						self.load = ImageLoad::Ready(handle);
						ctx.request_redraw();
						ImageProviderState::Ready(handle)
					}
					Err(error) => {
						self.load = ImageLoad::Error(error.clone());
						ctx.request_redraw();
						ImageProviderState::Error(error)
					}
				}
				Ok(Err(error)) => {
					self.load = ImageLoad::Error(error.clone());
					ctx.request_redraw();
					ImageProviderState::Error(error)
				}
				Err(mpsc::TryRecvError::Empty) => ImageProviderState::Loading,
				Err(mpsc::TryRecvError::Disconnected) => {
					let error = ImageError::new("image loader stopped before returning a result");
					self.load = ImageLoad::Error(error.clone());
					ImageProviderState::Error(error)
				}
			},
			ImageLoad::Ready(handle) => ImageProviderState::Ready(*handle),
			ImageLoad::Error(error) => ImageProviderState::Error(error.clone()),
		}
	}
}

#[derive(Debug, Clone)]
pub struct MemoryImage {
	bytes: Arc<[u8]>,
	key: ImageKey,
}

impl MemoryImage {
	pub fn new(bytes: impl Into<Arc<[u8]>>) -> Self {
		let bytes = bytes.into();
		let mut hasher = DefaultHasher::new();
		bytes.hash(&mut hasher);
		let key = ImageKey::new(format!("memory:{}:{}", bytes.len(), hasher.finish()));
		Self { bytes, key }
	}
}

impl ImageProviderBuilder for MemoryImage {
	type Instance = MemoryImageInstance;

	fn key(&self) -> ImageKey {
		self.key.clone()
	}

	fn build(&self, ctx: &ImageProviderContext<'_>) -> Self::Instance {
		if let Some(handle) = ctx.cached(&self.key) {
			return MemoryImageInstance {
				key: self.key.clone(),
				load: ImageLoad::Ready(handle),
			};
		}

		let bytes = self.bytes.clone();
		let (sender, receiver) = mpsc::channel();
		thread::spawn(move || {
			let _ = sender.send(Ok(load_image(bytes)));
		});
		MemoryImageInstance {
			key: self.key.clone(),
			load: ImageLoad::Pending(receiver),
		}
	}
}

pub struct MemoryImageInstance {
	key: ImageKey,
	load: ImageLoad,
}

impl ImageProviderInstance for MemoryImageInstance {
	fn poll(&mut self, ctx: &mut ImageProviderPollContext<'_>) -> ImageProviderState {
		match &self.load {
			ImageLoad::Pending(receiver) => match receiver.try_recv() {
				Ok(Ok(image)) => match ctx.manager.store_loaded(self.key.clone(), image) {
					Ok(handle) => {
						self.load = ImageLoad::Ready(handle);
						ctx.request_redraw();
						ImageProviderState::Ready(handle)
					}
					Err(error) => {
						self.load = ImageLoad::Error(error.clone());
						ctx.request_redraw();
						ImageProviderState::Error(error)
					}
				}
				Ok(Err(error)) => {
					self.load = ImageLoad::Error(error.clone());
					ctx.request_redraw();
					ImageProviderState::Error(error)
				}
				Err(mpsc::TryRecvError::Empty) => ImageProviderState::Loading,
				Err(mpsc::TryRecvError::Disconnected) => {
					let error = ImageError::new("image decoder stopped before returning a result");
					self.load = ImageLoad::Error(error.clone());
					ImageProviderState::Error(error)
				}
			},
			ImageLoad::Ready(handle) => ImageProviderState::Ready(*handle),
			ImageLoad::Error(error) => ImageProviderState::Error(error.clone()),
		}
	}
}

#[derive(Debug, Clone)]
pub struct SvgImage {
	svg: Arc<[u8]>,
	key: ImageKey,
}

impl SvgImage {
	pub fn new(svg: impl AsRef<str>) -> Self {
		let svg = Arc::<[u8]>::from(svg.as_ref().as_bytes());
		let mut hasher = DefaultHasher::new();
		svg.hash(&mut hasher);
		let key = ImageKey::new(format!("svg:{}:{}", svg.len(), hasher.finish()));
		Self { svg, key }
	}
}

impl ImageProviderBuilder for SvgImage {
	type Instance = MemoryImageInstance;

	fn key(&self) -> ImageKey {
		self.key.clone()
	}

	fn build(&self, ctx: &ImageProviderContext<'_>) -> Self::Instance {
		if let Some(handle) = ctx.cached(&self.key) {
			return MemoryImageInstance {
				key: self.key.clone(),
				load: ImageLoad::Ready(handle),
			};
		}

		let svg = self.svg.clone();
		let (sender, receiver) = mpsc::channel();
		thread::spawn(move || {
			let _ = sender.send(Ok(LoadedImage::Svg(svg)));
		});
		MemoryImageInstance {
			key: self.key.clone(),
			load: ImageLoad::Pending(receiver),
		}
	}
}

#[derive(Debug, Clone)]
pub struct NetworkImage {
	url: Arc<str>,
	user_agent: Arc<str>,
}

impl NetworkImage {
	pub fn new(url: impl Into<Arc<str>>) -> Self {
		Self {
			url: url.into(),
			user_agent: Arc::from("Ardos UI"),
		}
	}


	pub fn user_agent(mut self, user_agent: impl Into<Arc<str>>) -> Self {
		self.user_agent = user_agent.into();
		self
	}
}

impl ImageProviderBuilder for NetworkImage {
	type Instance = FileImageInstance;

	fn key(&self) -> ImageKey {
		ImageKey::new(format!("network:{}", self.url))
	}

	fn build(&self, ctx: &ImageProviderContext<'_>) -> Self::Instance {
		let key = self.key();

		if let Some(handle) = ctx.cached(&key) {
			return FileImageInstance {
				key,
				load: ImageLoad::Ready(handle),
			};
		}

		let url = Arc::clone(&self.url);
		let user_agent = Arc::clone(&self.user_agent);
		let (sender, receiver) = mpsc::channel();

		thread::spawn(move || {
			let result = load_network_image(&url, &user_agent);
			let _ = sender.send(result);
		});

		FileImageInstance {
			key,
			load: ImageLoad::Pending(receiver),
		}
	}
}

fn network_agent() -> ureq::Agent {
	static AGENT: OnceLock<ureq::Agent> = OnceLock::new();

	AGENT
		.get_or_init(|| {
			ureq::Agent::config_builder()
				.timeout_global(Some(Duration::from_secs(20)))
				.build()
				.into()
		})
		.clone()
}

fn load_network_image(
	url: &str,
	user_agent: &str,
) -> Result<LoadedImage, ImageError> {
	let mut response = network_agent()
		.get(url)
		.header("User-Agent", user_agent)
		.call()
		.map_err(|error| {
			ImageError::new(format!("failed to download image {url}: {error}"))
		})?;

	let bytes = response
		.body_mut()
		.with_config()
		.read_to_vec()
		.map_err(|error| {
			ImageError::new(format!("failed to read image response {url}: {error}"))
		})?;

	Ok(load_image(Arc::<[u8]>::from(bytes)))
}
enum LoadedImage {
	Raster(SkiaImage),
	Svg(Arc<[u8]>),
}

pub(crate) enum ResolvedImage {
	Raster(SkiaImage),
	Svg(Dom),
}

pub(crate) struct ImageManager {
	next_handle: u64,
	by_key: HashMap<ImageKey, ImageHandle>,
	decoded: HashMap<ImageHandle, ResolvedImage>,
}

impl Default for ImageManager {
	fn default() -> Self {
		Self {
			next_handle: 1,
			by_key: HashMap::new(),
			decoded: HashMap::new(),
		}
	}
}

impl ImageManager {
	pub fn provider_context(&self) -> ImageProviderContext<'_> {
		ImageProviderContext { manager: self }
	}

	pub fn poll_context(&mut self) -> ImageProviderPollContext<'_> {
		ImageProviderPollContext { manager: self }
	}

	fn cached(&self, key: &ImageKey) -> Option<ImageHandle> {
		self.by_key.get(key).copied()
	}

	fn store_loaded(
		&mut self,
		key: ImageKey,
		image: LoadedImage,
	) -> Result<ImageHandle, ImageError> {
		if let Some(handle) = self.cached(&key) {
			return Ok(handle);
		}
		let image = match image {
			LoadedImage::Raster(image) => ResolvedImage::Raster(image),
			LoadedImage::Svg(bytes) => ResolvedImage::Svg(
				Dom::from_bytes(&bytes, FontMgr::default())
					.map_err(|_| ImageError::new("unsupported or invalid SVG image"))?,
			),
		};
		let handle = ImageHandle(self.next_handle);
		self.next_handle += 1;
		self.by_key.insert(key, handle);
		self.decoded.insert(handle, image);
		Ok(handle)
	}

	pub fn resolve(&self, handle: ImageHandle) -> Option<&ResolvedImage> {
		self.decoded.get(&handle)
	}

	pub fn resolve_id(&self, id: u64) -> Option<&ResolvedImage> {
		self.resolve(ImageHandle(id))
	}
}

fn load_image(bytes: Arc<[u8]>) -> LoadedImage {
	if is_svg(&bytes) {
		LoadedImage::Svg(bytes)
	} else {
		SkiaImage::from_encoded(Data::new_copy(&bytes))
			.map_or(LoadedImage::Svg(bytes), LoadedImage::Raster)
	}
}

fn is_svg(bytes: &[u8]) -> bool {
	let bytes = bytes
		.strip_prefix(&[0xef, 0xbb, 0xbf])
		.unwrap_or(bytes);
	let Ok(text) = std::str::from_utf8(bytes) else {
		return false;
	};
	let text = text.trim_start();
	text.starts_with("<svg") || (text.starts_with("<?xml") && text.contains("<svg"))
}

impl From<&Path> for AssetImage {
	fn from(path: &Path) -> Self {
		Self::new(path)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	const PNG: &[u8] = &[
		137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 4, 0,
		0, 0, 181, 28, 12, 2, 0, 0, 0, 11, 73, 68, 65, 84, 120, 218, 99, 252, 255, 31, 0, 2, 235, 1,
		245, 105, 118, 197, 151, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
	];

	#[test]
	fn memory_provider_reuses_cached_handle() {
		let provider = MemoryImage::new(PNG);
		let mut manager = ImageManager::default();

		let mut first = provider.build(&manager.provider_context());
		let first = poll_until_ready(&mut first, &mut manager);
		let ImageProviderState::Ready(first) = first else {
			panic!("memory image should be ready, got {first:?}");
		};

		let mut second = provider.build(&manager.provider_context());
		let second = second.poll(&mut manager.poll_context());
		let ImageProviderState::Ready(second) = second else {
			panic!("cached memory image should be ready");
		};

		assert_eq!(first, second);
		assert!(manager.resolve(first).is_some());
	}

	fn poll_until_ready(
		instance: &mut impl ImageProviderInstance,
		manager: &mut ImageManager,
	) -> ImageProviderState {
		for _ in 0..100 {
			let state = instance.poll(&mut manager.poll_context());
			if !matches!(state, ImageProviderState::Loading) {
				return state;
			}
			std::thread::sleep(std::time::Duration::from_millis(1));
		}
		ImageProviderState::Loading
	}

	#[test]
	fn different_memory_contents_have_different_keys() {
		assert_ne!(
			MemoryImage::new([1_u8]).key(),
			MemoryImage::new([2_u8]).key()
		);
	}

	#[test]
	fn svg_provider_is_cached_as_vector_content() {
		let provider = SvgImage::new(
			r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
				<path d="M2 12h20"/>
			</svg>"#,
		);
		let mut manager = ImageManager::default();
		let mut instance = provider.build(&manager.provider_context());
		let state = poll_until_ready(&mut instance, &mut manager);
		let ImageProviderState::Ready(handle) = state else {
			panic!("SVG should be ready, got {state:?}");
		};

		assert!(matches!(
			manager.resolve(handle),
			Some(ResolvedImage::Svg(_))
		));
	}

	#[test]
	fn detects_svg_with_xml_declaration_and_bom() {
		assert!(is_svg(b"\xef\xbb\xbf  <?xml version=\"1.0\"?><svg/>"));
		assert!(!is_svg(PNG));
	}
}
