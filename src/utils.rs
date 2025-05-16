use std::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

pub trait LockOrPanic<T: ?Sized> {
    fn lock_or_panic(&self) -> MutexGuard<'_, T>;
}

impl<T: ?Sized> LockOrPanic<T> for Mutex<T> {
    fn lock_or_panic(&self) -> MutexGuard<'_, T> {
        self.lock().expect("expected no panics from other threads")
    }
}

//pub trait IntoInnerOrPanic<T: Sized> {
//    fn into_inner_or_panic(self) -> T;
//}
//
//impl<T: Sized> IntoInnerOrPanic<T> for Mutex<T> {
//    fn into_inner_or_panic(self) -> T {
//        self.into_inner()
//            .expect("expected no panics from other threads")
//    }
//}

pub trait ReadOrPanic<T: Sized> {
    fn read_or_panic(&self) -> RwLockReadGuard<'_, T>;
}

impl<T: Sized> ReadOrPanic<T> for RwLock<T> {
    fn read_or_panic(&self) -> RwLockReadGuard<'_, T> {
        self.read().expect("expected no panics from other threads")
    }
}

pub trait WriteOrPanic<T: Sized> {
    fn write_or_panic(&self) -> RwLockWriteGuard<'_, T>;
}

impl<T: Sized> WriteOrPanic<T> for RwLock<T> {
    fn write_or_panic(&self) -> RwLockWriteGuard<'_, T> {
        self.write().expect("expected no panics from other threads")
    }
}
