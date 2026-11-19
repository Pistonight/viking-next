//! These macros are re-exported from [`viking`](../viking/index.html)
//! and should not be imported from here

/// Macro for `ui::print_step` that allows formatting
#[macro_export]
macro_rules! step {
    ($($arg:tt)*) => { viking::ui::print_step(&format!($($arg)*)) };
}

/// Macro for `ui::print_note` that allows formatting
#[macro_export]
macro_rules! note {
    ($($arg:tt)*) => { viking::ui::print_note(&format!($($arg)*)) };
}

/// Macro for `ui::print_warning` that allows formatting
#[macro_export]
macro_rules! warn_ {
    ($($arg:tt)*) => { viking::ui::print_warning(&format!($($arg)*)) };
}

/// Macro for `ui::print_error` that allows formatting
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => { viking::ui::print_error(&format!($($arg)*)) };
}
