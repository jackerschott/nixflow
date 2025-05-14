use std::sync::{Mutex, MutexGuard};

pub trait LockOrPanic<T: ?Sized> {
    fn lock_or_panic(&self) -> MutexGuard<'_, T>;
}

impl<T: ?Sized> LockOrPanic<T> for Mutex<T> {
    fn lock_or_panic(&self) -> MutexGuard<'_, T> {
        self.lock().expect("expected no panics from other threads")
    }
}

pub trait IntoInnerOrPanic<T: Sized> {
    fn into_inner_or_panic(self) -> T;
}

impl<T: Sized> IntoInnerOrPanic<T> for Mutex<T> {
    fn into_inner_or_panic(self) -> T {
        self.into_inner()
            .expect("expected no panics from other threads")
    }
}
