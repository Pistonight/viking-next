/// Create a handle class that stores a raw pointer leaked from Rust.
///
/// The benefits over generating a #[pyclass] binding:
/// - Force internal state to be not clonable to Python
/// - No restriction on the type to be Sync
/// - No lifetime restriction
///
/// The only downside is we are not checking borrowing rules
/// if mut references
macro_rules! make_handle_class {
    (
        $(#[doc = $doc:literal])*
        #[handle($inner_type:ident, module = $module:literal)]
        $($rest:tt)*
) => {
        $crate::interop::make_handle_class!(__struct (
            $(#[doc = $doc])*
            #[pyclass(from_py_object, module = $module)]
            #[derive(Clone)]
        ) [$inner_type] $($rest)*);
    };
    (__struct ($($attr:tt)*) [$inner_type:ident] struct $handle_type:ident ; ) => {
        $($attr)*
        struct $handle_type(usize);
        $crate::interop::make_handle_class!(__impl [$inner_type, $handle_type]);
    };
    (__struct ($($attr:tt)*) [$inner_type:ident] pub struct $handle_type:ident ; ) => {
        $($attr)*
        pub struct $handle_type(usize);
        $crate::interop::make_handle_class!(__impl [$inner_type, $handle_type]);
    };
    (__struct ($($attr:tt)*) [$inner_type:ident] pub mut struct $handle_type:ident ; ) => {
        $($attr)*
        pub struct $handle_type(usize);
        $crate::interop::make_handle_class!(__impl [$inner_type, $handle_type]);
        $crate::interop::make_handle_class!(__impl_mut [$inner_type, $handle_type]);
    };
    (__struct ($($attr:tt)*) [$inner_type:ident] mut struct $handle_type:ident ; ) => {
        $($attr)*
        struct $handle_type(usize);
        $crate::interop::make_handle_class!(__impl [$inner_type, $handle_type]);
        $crate::interop::make_handle_class!(__impl_mut [$inner_type, $handle_type]);
    };
    (__impl [$inner_type:ident, $handle_type:ident]) => {
        impl ::std::ops::Deref for $handle_type {
            type Target = $inner_type;
            fn deref(&self) -> &Self::Target {
                unsafe { &*(self.0 as *const $inner_type) }
            }
        }
        impl $handle_type {
            /// Create a raw handle for the underlying value.
            ///
            /// The handle always references a valid object,
            /// but does NOT check borrow rules.
            #[allow(unused)]
            pub fn new(value: $inner_type) -> Self {
                Self(Box::into_raw(Box::new(value)) as usize)
            }

            /// Claim the ownership of the underlying object
            ///
            /// # Safety
            /// Any reference in Python to this object becomes dangling
            /// and must not be passed to Rust again.
            #[allow(unused)]
            pub unsafe fn claim(self) -> Box<$inner_type> {
                unsafe { Box::from_raw(self.0 as *mut $inner_type) }
            }

            /// Claim the ownership of the underlying object
            ///
            /// # Safety
            /// Any reference in Python to this object becomes dangling
            /// and must not be passed to Rust again.
            #[allow(unused)]
            pub unsafe fn claim_by_ref(&self) -> Box<$inner_type> {
                unsafe { Box::from_raw(self.0 as *mut $inner_type) }
            }
        }
    };
    (__impl_mut [$inner_type:ident, $handle_type:ident]) => {
        impl ::std::ops::DerefMut for $handle_type {
            fn deref_mut(&mut self) -> &mut Self::Target {
                unsafe { &mut *(self.0 as *mut $inner_type) }
            }
        }
    };
}
pub(crate) use make_handle_class;
