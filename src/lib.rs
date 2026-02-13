#![feature(allocator_api)]
#![feature(slice_ptr_get)]
#![feature(const_default)]
#![feature(const_trait_impl)]

use std::{
    alloc::{Allocator, GlobalAlloc},
    sync::atomic::AtomicUsize,
};

pub struct LeakDetector<T> {
    inner: T,
    used: AtomicUsize,
}

impl<T: [const] Default> const Default for LeakDetector<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl LeakDetector<std::alloc::System> {
    pub const fn system() -> Self {
        Self::new(std::alloc::System)
    }
}

impl<T> LeakDetector<T> {
    pub const fn new(val: T) -> Self {
        Self {
            inner: val,
            used: AtomicUsize::new(0),
        }
    }
}

unsafe impl<T: Allocator> Allocator for LeakDetector<T> {
    fn allocate(
        &self,
        layout: std::alloc::Layout,
    ) -> Result<std::ptr::NonNull<[u8]>, std::alloc::AllocError> {
        self.inner.allocate(layout).inspect(|_| {
            self.used
                .fetch_add(layout.size(), std::sync::atomic::Ordering::AcqRel);
        })
    }

    unsafe fn deallocate(&self, ptr: std::ptr::NonNull<u8>, layout: std::alloc::Layout) {
        unsafe {
            self.inner.deallocate(ptr, layout);
        }
        self.used
            .fetch_sub(layout.size(), std::sync::atomic::Ordering::AcqRel);
    }

    fn allocate_zeroed(
        &self,
        layout: std::alloc::Layout,
    ) -> Result<std::ptr::NonNull<[u8]>, std::alloc::AllocError> {
        self.inner.allocate_zeroed(layout).inspect(|_| {
            self.used
                .fetch_add(layout.size(), std::sync::atomic::Ordering::AcqRel);
        })
    }

    unsafe fn grow(
        &self,
        ptr: std::ptr::NonNull<u8>,
        old_layout: std::alloc::Layout,
        new_layout: std::alloc::Layout,
    ) -> Result<std::ptr::NonNull<[u8]>, std::alloc::AllocError> {
        unsafe {
            self.inner.grow(ptr, old_layout, new_layout).inspect(|_| {
                self.used.fetch_add(
                    new_layout.size().unchecked_sub(old_layout.size()),
                    std::sync::atomic::Ordering::AcqRel,
                );
            })
        }
    }

    unsafe fn grow_zeroed(
        &self,
        ptr: std::ptr::NonNull<u8>,
        old_layout: std::alloc::Layout,
        new_layout: std::alloc::Layout,
    ) -> Result<std::ptr::NonNull<[u8]>, std::alloc::AllocError> {
        unsafe {
            self.inner
                .grow_zeroed(ptr, old_layout, new_layout)
                .inspect(|_| {
                    self.used.fetch_add(
                        new_layout.size().unchecked_sub(old_layout.size()),
                        std::sync::atomic::Ordering::AcqRel,
                    );
                })
        }
    }

    unsafe fn shrink(
        &self,
        ptr: std::ptr::NonNull<u8>,
        old_layout: std::alloc::Layout,
        new_layout: std::alloc::Layout,
    ) -> Result<std::ptr::NonNull<[u8]>, std::alloc::AllocError> {
        unsafe {
            self.inner.shrink(ptr, old_layout, new_layout).inspect(|_| {
                self.used.fetch_add(
                    old_layout.size().unchecked_sub(new_layout.size()),
                    std::sync::atomic::Ordering::AcqRel,
                );
            })
        }
    }
}

unsafe impl<T: GlobalAlloc> GlobalAlloc for LeakDetector<T> {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        let result = unsafe { self.inner.alloc(layout) };
        self.used
            .fetch_add(layout.size(), std::sync::atomic::Ordering::AcqRel);
        result
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        unsafe {
            self.inner.dealloc(ptr, layout);
        }
        self.used
            .fetch_sub(layout.size(), std::sync::atomic::Ordering::AcqRel);
    }

    unsafe fn alloc_zeroed(&self, layout: std::alloc::Layout) -> *mut u8 {
        let result = unsafe { self.inner.alloc_zeroed(layout) };
        self.used
            .fetch_add(layout.size(), std::sync::atomic::Ordering::AcqRel);
        result
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: std::alloc::Layout, new_size: usize) -> *mut u8 {
        let result = unsafe { self.inner.realloc(ptr, layout, new_size) };
        self.used.update(
            std::sync::atomic::Ordering::Release,
            std::sync::atomic::Ordering::Acquire,
            |x| unsafe { x.unchecked_sub(layout.size()) } + new_size,
        );
        result
    }
}

impl<T> LeakDetector<T> {
    pub fn assert(&self) {
        assert!(self.used.load(std::sync::atomic::Ordering::Acquire) == 0);
    }
}

#[cfg(test)]
mod tests {
    use std::alloc::System;

    use super::*;

    static _GLOBAL: LeakDetector<System> = LeakDetector::new(System);

    #[test]
    fn test() {
        let boxed1 = Box::new_in(10, &_GLOBAL);
        let boxed2 = Box::new_in(130, &_GLOBAL);
        let boxed3 = Box::new_in(120, &_GLOBAL);
        let boxed4 = Box::new_in(140, &_GLOBAL);
        drop((boxed1, boxed2, boxed3, boxed4));
        _GLOBAL.assert();
    }
}
