//! CFDP Circular linked list types and functions.

use crate::ffi;

/// Node in a circular linked list.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct CListNode(pub(crate) ffi::CF_CListNode_t);

impl Default for CListNode {
    fn default() -> Self {
        let mut node = Self(unsafe { core::mem::zeroed() });
        node.init();
        node
    }
}

impl CListNode {
    /// Initializes the node (sets prev/next to self).
    pub fn init(&mut self) {
        unsafe { ffi::CF_CList_InitNode(&mut self.0) }
    }

    /// Returns a pointer to the next node.
    pub fn next(&self) -> *mut CListNode {
        self.0.next as *mut CListNode
    }

    /// Returns a pointer to the previous node.
    pub fn prev(&self) -> *mut CListNode {
        self.0.prev as *mut CListNode
    }
}

/// Circular linked list head pointer wrapper.
pub struct CList {
    head: *mut ffi::CF_CListNode_t,
}

impl Default for CList {
    fn default() -> Self {
        Self::new()
    }
}

impl CList {
    /// Creates a new empty list.
    pub const fn new() -> Self {
        Self {
            head: core::ptr::null_mut(),
        }
    }

    /// Returns true if the list is empty.
    pub fn is_empty(&self) -> bool {
        self.head.is_null()
    }

    /// Inserts a node at the front of the list.
    ///
    /// # Safety
    /// The node must remain valid for the lifetime of its membership in the list.
    pub unsafe fn insert_front(&mut self, node: &mut CListNode) {
        let head_ptr = &mut self.head as *mut *mut ffi::CF_CListNode_t;
        ffi::CF_CList_InsertFront(head_ptr, &mut node.0);
    }

    /// Inserts a node at the back of the list.
    ///
    /// # Safety
    /// The node must remain valid for the lifetime of its membership in the list.
    pub unsafe fn insert_back(&mut self, node: &mut CListNode) {
        let head_ptr = &mut self.head as *mut *mut ffi::CF_CListNode_t;
        ffi::CF_CList_InsertBack(head_ptr, &mut node.0);
    }

    /// Removes a node from the list.
    ///
    /// # Safety
    /// The node must be a member of this list.
    pub unsafe fn remove(&mut self, node: &mut CListNode) {
        let head_ptr = &mut self.head as *mut *mut ffi::CF_CListNode_t;
        ffi::CF_CList_Remove(head_ptr, &mut node.0);
    }

    /// Pops and returns the first node from the list.
    pub fn pop(&mut self) -> Option<*mut CListNode> {
        let head_ptr = &mut self.head as *mut *mut ffi::CF_CListNode_t;
        let node = unsafe { ffi::CF_CList_Pop(head_ptr) };
        if node.is_null() {
            None
        } else {
            Some(node as *mut CListNode)
        }
    }

    /// Returns a pointer to the head node, if any.
    pub fn head(&self) -> Option<*mut CListNode> {
        if self.head.is_null() {
            None
        } else {
            Some(self.head as *mut CListNode)
        }
    }

    /// Inserts a node after another node.
    ///
    /// # Safety
    /// Both nodes must be valid and `after` must be a member of this list.
    pub unsafe fn insert_after(&mut self, after: &mut CListNode, node: &mut CListNode) {
        let head_ptr = &mut self.head as *mut *mut ffi::CF_CListNode_t;
        ffi::CF_CList_InsertAfter(head_ptr, &mut after.0, &mut node.0);
    }

    /// Traverses the list forward, calling the callback for each node.
    ///
    /// # Safety
    /// The callback must not modify the list structure.
    pub unsafe fn traverse<F>(&self, mut callback: F)
    where
        F: FnMut(*mut CListNode, *mut core::ffi::c_void) -> super::CListTraverseStatus,
    {
        unsafe extern "C" fn trampoline<F>(
            node: *mut ffi::CF_CListNode_t,
            context: *mut core::ffi::c_void,
        ) -> ffi::CF_CListTraverse_Status_t
        where
            F: FnMut(*mut CListNode, *mut core::ffi::c_void) -> super::CListTraverseStatus,
        {
            let callback = &mut *(context as *mut F);
            callback(node as *mut CListNode, core::ptr::null_mut()) as ffi::CF_CListTraverse_Status_t
        }
        let callback_ptr = &mut callback as *mut F as *mut core::ffi::c_void;
        ffi::CF_CList_Traverse(self.head, Some(trampoline::<F>), callback_ptr);
    }

    /// Traverses the list in reverse, calling the callback for each node.
    ///
    /// # Safety
    /// The callback must not modify the list structure.
    pub unsafe fn traverse_reverse<F>(&self, mut callback: F)
    where
        F: FnMut(*mut CListNode, *mut core::ffi::c_void) -> super::CListTraverseStatus,
    {
        unsafe extern "C" fn trampoline<F>(
            node: *mut ffi::CF_CListNode_t,
            context: *mut core::ffi::c_void,
        ) -> ffi::CF_CListTraverse_Status_t
        where
            F: FnMut(*mut CListNode, *mut core::ffi::c_void) -> super::CListTraverseStatus,
        {
            let callback = &mut *(context as *mut F);
            callback(node as *mut CListNode, core::ptr::null_mut()) as ffi::CF_CListTraverse_Status_t
        }
        let callback_ptr = &mut callback as *mut F as *mut core::ffi::c_void;
        ffi::CF_CList_Traverse_R(self.head, Some(trampoline::<F>), callback_ptr);
    }
}
