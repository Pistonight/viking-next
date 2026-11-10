
macro_rules! make_handle_class {
    (__impl $inner_type:ident, $handle_type:ident) => {
    };
    (__impl $inner_type:ident, $handle_type:ident mut) => {
        impl ::std::ops::DerefMut for $handle_type {
            fn deref_mut(&mut self) -> &mut Self::Target {
                unsafe { &mut *(self.0 as *mut $inner_type) }
            }
        }
        impl $handle_type {
            /// Claim the ownership of the underlying object
            ///
            /// # Safety
            /// Any reference in Python to this object becomes dangling
            /// and must not be passed to Rust again.
            pub unsafe fn claim_by_ref(&self) -> Box<$inner_type> {
                unsafe { Box::from_raw(self.0 as *mut $inner_type) }
            }
        }
    };
    ($inner_type:ident, $handle_type:ident $($imp_tokens:tt)*) => {
        #[pyclass(from_py_object)]
        #[derive(Clone)]
        pub struct $handle_type(usize);
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
            pub fn new(value: $inner_type) -> Self {
                Self(Box::into_raw(Box::new(value)) as usize)
            }

            /// Claim the ownership of the underlying object
            ///
            /// # Safety
            /// Any reference in Python to this object becomes dangling
            /// and must not be passed to Rust again.
            pub unsafe fn claim(self) -> Box<$inner_type> {
                unsafe { Box::from_raw(self.0 as *mut $inner_type) }
            }
        }
        $crate::interop::make_handle_class!(__impl $inner_type, $handle_type $($imp_tokens)*);
    };
}
pub(crate) use make_handle_class;
