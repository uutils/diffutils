#[cfg(windows)]
#[macro_export]
macro_rules! duwriteln {
  ($dst:expr) => (write!($dst, "\r\n"));
  ($dst:expr, $arg:expr) => (write!($dst, concat!($arg, "\r\n")));
  ($dst:expr, $fmt:expr, $($arg:tt)*) => (write!($dst, concat!($fmt, "\r\n"), $($arg)*));
}
#[cfg(not(windows))]
#[macro_export]
macro_rules! duwriteln {
  ($dst:expr, $($arg:tt)*) => (writeln!($dst, $($arg)*));
}

#[macro_export]
macro_rules! multiwriteln {
  ($dst:expr, $arg:expr) => (duwriteln!($dst, $arg));
  ($dst:expr, $arg:expr, $($args:tt)*) => {
    duwriteln!($dst, $arg);
    multiwriteln!($dst, $($args)*)
  }
}

#[cfg(windows)]
#[macro_export]
macro_rules! split_at_eol {
  ($buf:expr) => (crate::utils::split_at_win_eol($buf));
}

#[cfg(not(windows))]
#[macro_export]
macro_rules! split_at_eol {
  ($buf:expr) => (buf.split(|&c| c == b'\n').collect::<Vec<_>>());
}
