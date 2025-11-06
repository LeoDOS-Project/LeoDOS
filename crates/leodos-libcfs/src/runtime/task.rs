//! Task storage and management.
use crate::error::Error;
use core::future::Future;
use core::marker::PhantomData;
use core::mem;
use core::mem::MaybeUninit;
use core::pin::Pin;
use core::task::Context;
use core::task::Poll;

pub type TaskResult = Result<(), Error>;
pub const DEFAULT_MAX_TASK_SIZE: usize = 512;

pub(crate) struct Task<'a, const MAX_TASK_SIZE: usize = DEFAULT_MAX_TASK_SIZE> {
    storage: [u8; MAX_TASK_SIZE],
    aligned_offset: usize,
    poll_fn: Option<unsafe fn(*mut (), &mut Context<'_>) -> Poll<TaskResult>>,
    drop_fn: Option<unsafe fn(*mut ())>,
    is_done: bool,
    _phantom: PhantomData<&'a ()>,
}

pub const fn check_future_size<F, const MAX_TASK_SIZE: usize>() {
    trait FutureSizeCheck {
        const IS_TOO_LARGE: bool;
        const TRIGGER_CHECK: ();
    }

    impl<T, const N: usize> FutureSizeCheck for (T, [(); N]) {
        const IS_TOO_LARGE: bool = core::mem::size_of::<T>() > N;

        const TRIGGER_CHECK: () = {
            if Self::IS_TOO_LARGE {
                panic!("Future is too large for task storage");
            }
        };
    }

    let _ = <(F, [(); MAX_TASK_SIZE]) as FutureSizeCheck>::TRIGGER_CHECK;
}

impl<'a, const MAX_TASK_SIZE: usize> Task<'a, MAX_TASK_SIZE> {
    pub(crate) fn new<F>(future: F) -> Result<Self, Error>
    where
        F: Future<Output = TaskResult> + 'a,
    {
        // Trigger compile-time check
        check_future_size::<F, MAX_TASK_SIZE>();

        let size = mem::size_of::<F>();
        let align = mem::align_of::<F>();

        if size + align - 1 > MAX_TASK_SIZE {
            return Err(Error::TaskTooLarge);
        }

        unsafe fn poll_fn<F: Future<Output = TaskResult>>(
            storage: *mut (),
            cx: &mut Context<'_>,
        ) -> Poll<TaskResult> {
            Pin::new_unchecked(&mut *(storage as *mut F)).poll(cx)
        }

        unsafe fn drop_fn<F>(storage: *mut ()) {
            core::ptr::drop_in_place(storage as *mut F);
        }

        let (storage, aligned_offset) = unsafe {
            let mut storage_mu = MaybeUninit::<[u8; MAX_TASK_SIZE]>::uninit();
            let base_ptr = storage_mu.as_mut_ptr() as *mut u8;
            let base_addr = base_ptr as usize;
            let aligned_addr = (base_addr + align - 1) & !(align - 1);
            let offset = aligned_addr - base_addr;
            let f_ptr = base_ptr.add(offset) as *mut F;
            f_ptr.write(future);
            (storage_mu.assume_init(), offset)
        };

        Ok(Self {
            storage,
            aligned_offset,
            poll_fn: Some(poll_fn::<F>),
            drop_fn: Some(drop_fn::<F>),
            is_done: false,
            _phantom: PhantomData,
        })
    }

    pub(crate) fn storage_ptr(&mut self) -> *mut () {
        unsafe { self.storage.as_mut_ptr().add(self.aligned_offset) as *mut () }
    }

    pub(crate) fn cleanup(&mut self) {
        if !self.is_done {
            if let Some(drop_fn) = self.drop_fn.take() {
                unsafe { drop_fn(self.storage_ptr()) };
            }
            self.poll_fn = None;
            self.is_done = true;
        }
    }

    pub(crate) fn is_done(&self) -> bool {
        self.is_done
    }

    pub(crate) fn poll(&mut self, cx: &mut Context<'_>) -> Poll<TaskResult> {
        if let Some(poll_fn) = self.poll_fn {
            unsafe { poll_fn(self.storage_ptr(), cx) }
        } else {
            Poll::Ready(Ok(()))
        }
    }
}

impl<'a, const MAX_TASK_SIZE: usize> Drop for Task<'_, MAX_TASK_SIZE> {
    fn drop(&mut self) {
        self.cleanup();
    }
}
