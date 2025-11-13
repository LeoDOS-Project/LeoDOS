//! Compile-time assertions for type size and alignment.

/// Asserts at compile-time that `size_of::<T>() <= limit`.
/// Works inside generic functions with const generics.
#[macro_export]
macro_rules! comptime_assert_size_le {
    ($type:ty, $limit:expr) => {{
        trait SizeCheck {
            const CHECK: ();
        }
        impl<T, const N: usize> SizeCheck for (T, [(); N]) {
            const CHECK: () = {
                if core::mem::size_of::<T>() > N {
                    panic!("type size exceeds limit");
                }
            };
        }
        let _ = <($type, [(); $limit]) as SizeCheck>::CHECK;
    }};
}

/// Asserts at compile-time that `align_of::<T>() <= limit`.
/// Works inside generic functions with const generics.
#[macro_export]
macro_rules! comptime_assert_align_le {
    ($type:ty, $limit:expr) => {{
        trait AlignCheck {
            const CHECK: ();
        }
        impl<T, const N: usize> AlignCheck for (T, [(); N]) {
            const CHECK: () = {
                if core::mem::align_of::<T>() > N {
                    panic!("type alignment exceeds limit");
                }
            };
        }
        let _ = <($type, [(); $limit]) as AlignCheck>::CHECK;
    }};
}
