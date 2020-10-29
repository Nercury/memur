#![allow(dead_code)]
#![macro_use]

macro_rules! debug {
    (target: $target:expr, $($arg:tt)+) => (
        #[cfg(feature = "logging")]
        log::debug!(target: $target, $($arg)+);
    );
    ($($arg:tt)+) => (
        #[cfg(feature = "logging")]
        log::debug!($($arg)+);
    )
}

macro_rules! trace {
    (target: $target:expr, $($arg:tt)+) => (
        #[cfg(feature = "logging")]
        log::trace!(target: $target, $($arg)+);
    );
    ($($arg:tt)+) => (
        #[cfg(feature = "logging")]
        log::trace!($($arg)+);
    )
}