//! Structured concurrency primitives.
//!
//! [`join!`] and [`try_join!`] poll multiple futures
//! concurrently until all complete. `try_join!` returns
//! early on the first error.
//!
//! ```ignore
//! use leodos_libcfs::join;
//! use leodos_libcfs::try_join;
//!
//! let (a, b) = join!(fut_a, fut_b).await;
//! let (a, b) = try_join!(fut_a, fut_b).await?;
//! ```

/// Polls futures concurrently, returning a tuple of all
/// outputs when every future has completed.
#[macro_export]
macro_rules! join {
    ($f1:expr, $f2:expr $(,)?) => {
        async {
            use ::core::future::Future;
            let f1 = $f1;
            let f2 = $f2;
            $crate::runtime::pin_mut!(f1, f2);
            let (mut s1, mut s2) = (None, None);
            ::core::future::poll_fn(|cx| {
                if s1.is_none() { if let ::core::task::Poll::Ready(v) = f1.as_mut().poll(cx) { s1 = Some(v); } }
                if s2.is_none() { if let ::core::task::Poll::Ready(v) = f2.as_mut().poll(cx) { s2 = Some(v); } }
                if s1.is_some() && s2.is_some() {
                    ::core::task::Poll::Ready((s1.take().unwrap(), s2.take().unwrap()))
                } else { ::core::task::Poll::Pending }
            }).await
        }
    };
    ($f1:expr, $f2:expr, $f3:expr $(,)?) => {
        async {
            use ::core::future::Future;
            let f1 = $f1;
            let f2 = $f2;
            let f3 = $f3;
            $crate::runtime::pin_mut!(f1, f2, f3);
            let (mut s1, mut s2, mut s3) = (None, None, None);
            ::core::future::poll_fn(|cx| {
                if s1.is_none() { if let ::core::task::Poll::Ready(v) = f1.as_mut().poll(cx) { s1 = Some(v); } }
                if s2.is_none() { if let ::core::task::Poll::Ready(v) = f2.as_mut().poll(cx) { s2 = Some(v); } }
                if s3.is_none() { if let ::core::task::Poll::Ready(v) = f3.as_mut().poll(cx) { s3 = Some(v); } }
                if s1.is_some() && s2.is_some() && s3.is_some() {
                    ::core::task::Poll::Ready((s1.take().unwrap(), s2.take().unwrap(), s3.take().unwrap()))
                } else { ::core::task::Poll::Pending }
            }).await
        }
    };
    ($f1:expr, $f2:expr, $f3:expr, $f4:expr $(,)?) => {
        async {
            use ::core::future::Future;
            let f1 = $f1;
            let f2 = $f2;
            let f3 = $f3;
            let f4 = $f4;
            $crate::runtime::pin_mut!(f1, f2, f3, f4);
            let (mut s1, mut s2, mut s3, mut s4) = (None, None, None, None);
            ::core::future::poll_fn(|cx| {
                if s1.is_none() { if let ::core::task::Poll::Ready(v) = f1.as_mut().poll(cx) { s1 = Some(v); } }
                if s2.is_none() { if let ::core::task::Poll::Ready(v) = f2.as_mut().poll(cx) { s2 = Some(v); } }
                if s3.is_none() { if let ::core::task::Poll::Ready(v) = f3.as_mut().poll(cx) { s3 = Some(v); } }
                if s4.is_none() { if let ::core::task::Poll::Ready(v) = f4.as_mut().poll(cx) { s4 = Some(v); } }
                if s1.is_some() && s2.is_some() && s3.is_some() && s4.is_some() {
                    ::core::task::Poll::Ready((s1.take().unwrap(), s2.take().unwrap(), s3.take().unwrap(), s4.take().unwrap()))
                } else { ::core::task::Poll::Pending }
            }).await
        }
    };
}

/// Polls futures concurrently, returning `Ok(tuple)` when
/// all complete successfully, or the first `Err` encountered.
#[macro_export]
macro_rules! try_join {
    ($f1:expr, $f2:expr $(,)?) => {
        async {
            use ::core::future::Future;
            let f1 = $f1;
            let f2 = $f2;
            $crate::runtime::pin_mut!(f1, f2);
            let (mut s1, mut s2) = (None, None);
            ::core::future::poll_fn(|cx| {
                if s1.is_none() { if let ::core::task::Poll::Ready(v) = f1.as_mut().poll(cx) {
                    match v { Ok(v) => s1 = Some(v), Err(e) => return ::core::task::Poll::Ready(Err(e)) }
                }}
                if s2.is_none() { if let ::core::task::Poll::Ready(v) = f2.as_mut().poll(cx) {
                    match v { Ok(v) => s2 = Some(v), Err(e) => return ::core::task::Poll::Ready(Err(e)) }
                }}
                if s1.is_some() && s2.is_some() {
                    ::core::task::Poll::Ready(Ok((s1.take().unwrap(), s2.take().unwrap())))
                } else { ::core::task::Poll::Pending }
            }).await
        }
    };
    ($f1:expr, $f2:expr, $f3:expr $(,)?) => {
        async {
            use ::core::future::Future;
            let f1 = $f1;
            let f2 = $f2;
            let f3 = $f3;
            $crate::runtime::pin_mut!(f1, f2, f3);
            let (mut s1, mut s2, mut s3) = (None, None, None);
            ::core::future::poll_fn(|cx| {
                if s1.is_none() { if let ::core::task::Poll::Ready(v) = f1.as_mut().poll(cx) {
                    match v { Ok(v) => s1 = Some(v), Err(e) => return ::core::task::Poll::Ready(Err(e)) }
                }}
                if s2.is_none() { if let ::core::task::Poll::Ready(v) = f2.as_mut().poll(cx) {
                    match v { Ok(v) => s2 = Some(v), Err(e) => return ::core::task::Poll::Ready(Err(e)) }
                }}
                if s3.is_none() { if let ::core::task::Poll::Ready(v) = f3.as_mut().poll(cx) {
                    match v { Ok(v) => s3 = Some(v), Err(e) => return ::core::task::Poll::Ready(Err(e)) }
                }}
                if s1.is_some() && s2.is_some() && s3.is_some() {
                    ::core::task::Poll::Ready(Ok((s1.take().unwrap(), s2.take().unwrap(), s3.take().unwrap())))
                } else { ::core::task::Poll::Pending }
            }).await
        }
    };
    ($f1:expr, $f2:expr, $f3:expr, $f4:expr $(,)?) => {
        async {
            use ::core::future::Future;
            let f1 = $f1;
            let f2 = $f2;
            let f3 = $f3;
            let f4 = $f4;
            $crate::runtime::pin_mut!(f1, f2, f3, f4);
            let (mut s1, mut s2, mut s3, mut s4) = (None, None, None, None);
            ::core::future::poll_fn(|cx| {
                if s1.is_none() { if let ::core::task::Poll::Ready(v) = f1.as_mut().poll(cx) {
                    match v { Ok(v) => s1 = Some(v), Err(e) => return ::core::task::Poll::Ready(Err(e)) }
                }}
                if s2.is_none() { if let ::core::task::Poll::Ready(v) = f2.as_mut().poll(cx) {
                    match v { Ok(v) => s2 = Some(v), Err(e) => return ::core::task::Poll::Ready(Err(e)) }
                }}
                if s3.is_none() { if let ::core::task::Poll::Ready(v) = f3.as_mut().poll(cx) {
                    match v { Ok(v) => s3 = Some(v), Err(e) => return ::core::task::Poll::Ready(Err(e)) }
                }}
                if s4.is_none() { if let ::core::task::Poll::Ready(v) = f4.as_mut().poll(cx) {
                    match v { Ok(v) => s4 = Some(v), Err(e) => return ::core::task::Poll::Ready(Err(e)) }
                }}
                if s1.is_some() && s2.is_some() && s3.is_some() && s4.is_some() {
                    ::core::task::Poll::Ready(Ok((s1.take().unwrap(), s2.take().unwrap(), s3.take().unwrap(), s4.take().unwrap())))
                } else { ::core::task::Poll::Pending }
            }).await
        }
    };
}
