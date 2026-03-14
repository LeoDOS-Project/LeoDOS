use crate::comptime_assert_align_le;
use crate::comptime_assert_size_le;
use crate::error::CfsError;
use core::future::Future;
use core::marker::PhantomData;
use core::pin::Pin;
use core::task::Context;
use core::task::Poll;

pub type TaskResult = Result<(), CfsError>;
pub const DEFAULT_MAX_TASK_SIZE: usize = 512;

#[repr(align(16))]
struct AlignedStorage<const N: usize>([u8; N]);

pub(crate) struct Task<'a, const MAX_TASK_SIZE: usize = DEFAULT_MAX_TASK_SIZE> {
    storage: AlignedStorage<MAX_TASK_SIZE>, // Changed from [u8; MAX_TASK_SIZE]
    poll_fn: Option<unsafe fn(*mut (), &mut Context<'_>) -> Poll<TaskResult>>,
    drop_fn: Option<unsafe fn(*mut ())>,
    is_done: bool,
    _phantom: PhantomData<&'a ()>,
}

impl<'a, const MAX_TASK_SIZE: usize> Task<'a, MAX_TASK_SIZE> {
    pub(crate) fn new<F>(future: F) -> Self
    where
        F: Future<Output = TaskResult> + 'a,
    {
        comptime_assert_size_le!(F, MAX_TASK_SIZE);
        comptime_assert_align_le!(F, 16);

        unsafe fn poll_fn<F: Future<Output = TaskResult>>(
            storage: *mut (),
            cx: &mut Context<'_>,
        ) -> Poll<TaskResult> {
            Pin::new_unchecked(&mut *(storage as *mut F)).poll(cx)
        }

        unsafe fn drop_fn<F>(storage: *mut ()) {
            core::ptr::drop_in_place(storage as *mut F);
        }

        let mut task = Self {
            storage: AlignedStorage([0; MAX_TASK_SIZE]),
            poll_fn: Some(poll_fn::<F>),
            drop_fn: Some(drop_fn::<F>),
            is_done: false,
            _phantom: PhantomData,
        };

        unsafe {
            // Write to the beginning of the buffer.
            // The compiler guarantees this pointer is 16-byte aligned.
            let ptr = task.storage.0.as_mut_ptr() as *mut F;
            ptr.write(future);
        }

        task
    }

    pub(crate) fn storage_ptr(&mut self) -> *mut () {
        self.storage.0.as_mut_ptr() as *mut ()
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
