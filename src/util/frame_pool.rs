use std::any::Any;
use std::cell::UnsafeCell;
use std::marker::PhantomData;

/// A per-frame allocation pool.
///
/// This lets you allocate arbitrary `'static` values and obtain references tied to a
/// frame lifetime. At the end of the frame, call [`FramePool::reset`] to drop
/// everything at once.
///
/// ## Safety & design notes
///
/// - Internally uses an `UnsafeCell<Vec<Box<dyn Any>>>` so allocations can be pushed
///   through a shared reference (`&self`) while still enforcing correct lifetimes
///   at the API boundary via `FrameAllocator<'render>`.
/// - **Not thread-safe**. Intended for single-threaded UI rendering.
pub struct FramePool<'p> {
	storage: UnsafeCell<Vec<Box<dyn Any>>>,
	_marker: PhantomData<&'p ()>,
}

impl<'p> FramePool<'p> {
	#[inline]
	pub fn new() -> Self {
		Self {
			storage: UnsafeCell::new(Vec::new()),
			_marker: PhantomData,
		}
	}

	/// Begins a frame allocation scope.
	///
	/// The returned allocator borrows the pool for `'render`, preventing `reset()`
	/// from being called while the allocator exists.
	#[inline]
	pub fn begin_alloc<'render>(&'render self) -> FrameAllocator<'render>
	where
		'p: 'render,
	{
		FrameAllocator { pool: self }
	}

	/// Drops all allocated objects from this pool.
	///
	/// This should be called once per frame, after you are sure nobody can still
	/// reference any data allocated from this pool.
	#[inline]
	pub fn reset(&mut self) {
		unsafe { &mut *self.storage.get() }.clear();
	}
}

impl Drop for FramePool<'_> {
	#[inline]
	fn drop(&mut self) {
		self.reset();
	}
}

/// An allocator tied to a single frame.
///
/// Created by [`FramePool::begin_alloc`]. You can allocate arbitrary `'static`
/// values and receive mutable references tied to the allocator's lifetime.
#[derive(Clone, Copy)]
pub struct FrameAllocator<'render> {
	pool: &'render FramePool<'render>,
}

impl<'render> FrameAllocator<'render> {
	/// Allocate a value in the pool and get a mutable reference tied to `'render`.
	#[inline]
	pub fn alloc<T: 'static>(&self, value: T) -> &'render mut T {
		let storage = unsafe { &mut *self.pool.storage.get() };
		storage.push(Box::new(value));
		storage
			.last_mut()
			.expect("just pushed")
			.downcast_mut::<T>()
			.expect("type mismatch on downcast")
	}
}

#[macro_export]
macro_rules! frame_alloc_format {
	($frame_alloc:expr, $($arg:tt)*) => {
		$frame_alloc.alloc(format!($($arg)*)).as_str()
	};
}

#[macro_export]
macro_rules! format_id {
	($c:expr, $frame_alloc:expr, $($arg:tt)*) => {
		$c.id($frame_alloc.alloc(format!($($arg)*)).as_str())
	};
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn basic_allocation() {
		let mut pool = FramePool::new();
		{
			let alloc = pool.begin_alloc();
			let x = alloc.alloc(42);
			let y = alloc.alloc("hello".to_string());

			assert_eq!(*x, 42);
			assert_eq!(y, "hello");
		}
		pool.reset();
		assert!(unsafe { &*pool.storage.get() }.is_empty());
	}

	#[test]
	fn multiple_allocations_same_frame() {
		let mut pool = FramePool::new();
		{
			let alloc = pool.begin_alloc();
			let a = alloc.alloc(1);
			let b = alloc.alloc(2);
			let c = alloc.alloc(3);

			assert_eq!(*a + *b + *c, 6);
		}
		pool.reset();
		assert!(unsafe { &*pool.storage.get() }.is_empty());
	}

	#[test]
	fn allocator_scope_prevents_reset() {
		let mut pool = FramePool::new();
		let alloc = pool.begin_alloc();
		let _x = alloc.alloc(10);

		// pool.reset(); // borrow checker prevents calling reset while allocator exists

		drop(alloc);
		pool.reset();
		assert!(unsafe { &*pool.storage.get() }.is_empty());
	}
}
