use std::{sync::{Mutex, MutexGuard}, thread::JoinHandle};

// a clonable proxy for std::io::Error
#[derive(Clone, Debug, thiserror::Error)]
#[error("{message}")]
pub struct IoError {
    kind: std::io::ErrorKind,
    message: String,
}
impl From<std::io::Error> for IoError {
    fn from(error: std::io::Error) -> Self {
        IoError {
            kind: error.kind(),
            message: format!("{error}"),
        }
    }
}

pub trait LockOrPanic<T: ?Sized> {
    fn lock_or_panic(&self) -> MutexGuard<'_, T>;
}

impl<T: ?Sized> LockOrPanic<T> for Mutex<T> {
    fn lock_or_panic(&self) -> MutexGuard<'_, T> {
        self.lock().expect("code in other threads doesn't panic")
    }
}

pub trait JoinOrPanic<T> {
    fn join_or_panic(self) -> T;
}

impl<T> JoinOrPanic<T> for JoinHandle<T> {
    fn join_or_panic(self) -> T {
        self.join().expect("code in other threads doesn't panic")
    }
}
