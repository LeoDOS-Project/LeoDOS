//! Safe wrappers for WAMR type reflection APIs (imports, exports, etc.).

use crate::{c_char_to_string_heapless, ffi, runtime::Function, runtime::Instance, Result, WamrError, value::{WasmValue, WasmValueKind}};
use core::ffi::c_void;
use core::fmt::Write;
use core::marker::PhantomData;
use core::ptr::{self, NonNull};
use heapless::String;

/// Describes a function type.
#[derive(Debug, Clone)]
pub struct FuncType {
    // This is an opaque pointer managed by WAMR, not owned by us.
    // It's valid as long as the Module is valid.
    ptr: NonNull<ffi::WASMFuncType>,
}

impl FuncType {
    /// Get the number of parameters.
    pub fn param_count(&self) -> u32 {
        unsafe { ffi::wasm_func_type_get_param_count(self.ptr.as_ptr()) }
    }

    /// Get the number of results.
    pub fn result_count(&self) -> u32 {
        unsafe { ffi::wasm_func_type_get_result_count(self.ptr.as_ptr()) }
    }
}

/// Describes a table type.
#[derive(Debug, Clone)]
pub struct TableType {
    ptr: NonNull<ffi::WASMTableType>,
}

impl TableType {
    /// Returns the kind of elements in the table.
    pub fn elem_kind(&self) -> Result<WasmValueKind> {
        let kind = unsafe { ffi::wasm_table_type_get_elem_kind(self.ptr.as_ptr()) };
        WasmValueKind::try_from(kind)
    }

    /// Returns true if the table is shared.
    pub fn is_shared(&self) -> bool {
        unsafe { ffi::wasm_table_type_get_shared(self.ptr.as_ptr()) }
    }

    /// Returns the initial size of the table.
    pub fn initial_size(&self) -> u32 {
        unsafe { ffi::wasm_table_type_get_init_size(self.ptr.as_ptr()) }
    }

    /// Returns the maximum size of the table.
    pub fn max_size(&self) -> u32 {
        unsafe { ffi::wasm_table_type_get_max_size(self.ptr.as_ptr()) }
    }
}

/// Describes a memory type.
#[derive(Debug, Clone)]
pub struct MemoryType {
    ptr: NonNull<ffi::WASMMemoryType>,
}

impl MemoryType {
    /// Returns true if the memory is shared.
    pub fn is_shared(&self) -> bool {
        unsafe { ffi::wasm_memory_type_get_shared(self.ptr.as_ptr()) }
    }

    /// Returns the initial page count of the memory.
    pub fn initial_pages(&self) -> u32 {
        unsafe { ffi::wasm_memory_type_get_init_page_count(self.ptr.as_ptr()) }
    }

    /// Returns the maximum page count of the memory.
    pub fn max_pages(&self) -> u32 {
        unsafe { ffi::wasm_memory_type_get_max_page_count(self.ptr.as_ptr()) }
    }
}

/// Describes a global type.
#[derive(Debug, Clone)]
pub struct GlobalType {
    ptr: NonNull<ffi::WASMGlobalType>,
}

impl GlobalType {
    /// Returns the value kind of the global.
    pub fn val_kind(&self) -> Result<WasmValueKind> {
        let kind = unsafe { ffi::wasm_global_type_get_valkind(self.ptr.as_ptr()) };
        WasmValueKind::try_from(kind)
    }

    /// Returns true if the global is mutable.
    pub fn is_mutable(&self) -> bool {
        unsafe { ffi::wasm_global_type_get_mutable(self.ptr.as_ptr()) }
    }
}

/// A handle to an exported global instance.
#[derive(Debug)]
pub struct Global {
    kind: WasmValueKind,
    is_mutable: bool,
    // This is a pointer into the instance's memory, not owned by us.
    global_data: NonNull<c_void>,
}

impl Global {
    pub(crate) fn new(inst: ffi::wasm_global_inst_t) -> Self {
        Self {
            kind: WasmValueKind::try_from(inst.kind).unwrap(), // Should be valid
            is_mutable: inst.is_mutable,
            // This pointer should be valid if wasm_runtime_get_export_global_inst succeeded
            global_data: NonNull::new(inst.global_data).unwrap(),
        }
    }

    pub fn get(&self) -> WasmValue {
        unsafe {
            match self.kind {
                WasmValueKind::I32 => WasmValue::I32(ptr::read(self.global_data.as_ptr() as *const i32)),
                WasmValueKind::I64 => WasmValue::I64(ptr::read(self.global_data.as_ptr() as *const i64)),
                WasmValueKind::F32 => WasmValue::F32(ptr::read(self.global_data.as_ptr() as *const f32)),
                WasmValueKind::F64 => WasmValue::F64(ptr::read(self.global_data.as_ptr() as *const f64)),
                // Other types like funcref/externref are not simple values
                _ => unimplemented!(),
            }
        }
    }

    pub fn set(&mut self, value: WasmValue) -> Result<()> {
        if !self.is_mutable {
            return Err(WamrError::MemoryError(
                String::try_from("Global is not mutable").unwrap(),
            ));
        }

        if self.kind() != value.kind() {
            return Err(WamrError::InvalidWasmValue);
        }

        unsafe {
            match value {
                WasmValue::I32(v) => ptr::write(self.global_data.as_ptr() as *mut i32, v),
                WasmValue::I64(v) => ptr::write(self.global_data.as_ptr() as *mut i64, v),
                WasmValue::F32(v) => ptr::write(self.global_data.as_ptr() as *mut f32, v),
                WasmValue::F64(v) => ptr::write(self.global_data.as_ptr() as *mut f64, v),
            }
        }
        Ok(())
    }

    pub fn kind(&self) -> WasmValueKind {
        self.kind
    }

    pub fn is_mutable(&self) -> bool {
        self.is_mutable
    }
}

/// Describes an imported item. `N` is the max string length for names.
#[derive(Debug, Clone)]
pub struct Import<const N: usize> {
    pub module_name: String<N>,
    pub name: String<N>,
    pub kind: ImportExportKind,
    pub descriptor: ImportExportType,
}

/// Describes an exported item. `N` is the max string length for names.
#[derive(Debug, Clone)]
pub struct Export<const N: usize> {
    pub name: String<N>,
    pub kind: ImportExportKind,
    pub descriptor: ImportExportType,
}

/// The kind of an import or export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportExportKind {
    Func,
    Table,
    Memory,
    Global,
}

/// The specific type descriptor for an import or export.
#[derive(Debug, Clone)]
pub enum ImportExportType {
    Func(FuncType),
    Table(TableType),
    Memory(MemoryType),
    Global(GlobalType),
}

/// A handle to an exported table instance.
pub struct Table<'i> {
    ffi_inst: ffi::wasm_table_inst_t,
    instance: &'i Instance<'i>,
}

impl<'i> Table<'i> {
    pub(crate) fn new(ffi_inst: ffi::wasm_table_inst_t, instance: &'i Instance<'i>) -> Self {
        Self { ffi_inst, instance }
    }

    /// Retrieves a function from the table at the given index.
    pub fn get_func_inst(&self, idx: u32) -> Result<Function<'i>> {
        let func_ptr =
            unsafe { ffi::wasm_table_get_func_inst(self.instance.as_ptr(), &self.ffi_inst, idx) };

        if func_ptr.is_null() {
            let mut err_msg: String<64> = String::new();
            write!(&mut err_msg, "Function not found at table index {}", idx).ok();
            Err(WamrError::NotFound(err_msg))
        } else {
            Ok(Function {
                ptr: NonNull::new(func_ptr).unwrap(),
                instance_ptr: self.instance.ptr,
                _phantom: PhantomData,
            })
        }
    }
}

/// Represents a single frame in the Wasm call stack.
pub struct Frame {
    inner: ffi::WASMCApiFrame,
}

impl Frame {
    pub(crate) fn new(inner: ffi::WASMCApiFrame) -> Self {
        Self { inner }
    }

    pub fn func_index(&self) -> u32 {
        self.inner.func_index
    }

    pub fn func_offset(&self) -> u32 {
        self.inner.func_offset
    }

    pub fn module_offset(&self) -> u32 {
        self.inner.module_offset
    }
}

/// Iterator over the imports of a module.
pub struct ImportIterator<'m, const N: usize> {
    module_ptr: NonNull<ffi::WASMModuleCommon>,
    index: i32,
    count: i32,
    _phantom: PhantomData<&'m crate::runtime::Module<'m>>,
}

impl<'m, const N: usize> ImportIterator<'m, N> {
    pub(crate) fn new(module_ptr: NonNull<ffi::WASMModuleCommon>) -> Self {
        let count = unsafe { ffi::wasm_runtime_get_import_count(module_ptr.as_ptr()) };
        Self {
            module_ptr,
            index: 0,
            count,
            _phantom: PhantomData,
        }
    }
}

impl<'m, const N: usize> Iterator for ImportIterator<'m, N> {
    type Item = Import<N>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.count {
            return None;
        }

        let mut import_ffi = ffi::wasm_import_t::default();
        unsafe {
            ffi::wasm_runtime_get_import_type(self.module_ptr.as_ptr(), self.index, &mut import_ffi)
        };

        self.index += 1;

        // Safely handle potential null pointers from the FFI
        let module_name = c_char_to_string_heapless(import_ffi.module_name).unwrap_or_default();
        let name = c_char_to_string_heapless(import_ffi.name).unwrap_or_default();

        let (kind, descriptor) = unsafe {
            match import_ffi.kind {
                ffi::wasm_import_export_kind_t_WASM_IMPORT_EXPORT_KIND_FUNC => (
                    ImportExportKind::Func,
                    ImportExportType::Func(FuncType {
                        ptr: NonNull::new_unchecked(import_ffi.u.func_type),
                    }),
                ),
                ffi::wasm_import_export_kind_t_WASM_IMPORT_EXPORT_KIND_TABLE => (
                    ImportExportKind::Table,
                    ImportExportType::Table(TableType {
                        ptr: NonNull::new_unchecked(import_ffi.u.table_type),
                    }),
                ),
                ffi::wasm_import_export_kind_t_WASM_IMPORT_EXPORT_KIND_MEMORY => (
                    ImportExportKind::Memory,
                    ImportExportType::Memory(MemoryType {
                        ptr: NonNull::new_unchecked(import_ffi.u.memory_type),
                    }),
                ),
                ffi::wasm_import_export_kind_t_WASM_IMPORT_EXPORT_KIND_GLOBAL => (
                    ImportExportKind::Global,
                    ImportExportType::Global(GlobalType {
                        ptr: NonNull::new_unchecked(import_ffi.u.global_type),
                    }),
                ),
                _ => return self.next(), // Skip unknown kinds
            }
        };

        Some(Import {
            module_name,
            name,
            kind,
            descriptor,
        })
    }
}

/// Iterator over the exports of a module.
pub struct ExportIterator<'m, const N: usize> {
    module_ptr: NonNull<ffi::WASMModuleCommon>,
    index: i32,
    count: i32,
    _phantom: PhantomData<&'m crate::runtime::Module<'m>>,
}

impl<'m, const N: usize> ExportIterator<'m, N> {
    pub(crate) fn new(module_ptr: NonNull<ffi::WASMModuleCommon>) -> Self {
        let count = unsafe { ffi::wasm_runtime_get_export_count(module_ptr.as_ptr()) };
        Self {
            module_ptr,
            index: 0,
            count,
            _phantom: PhantomData,
        }
    }
}

impl<'m, const N: usize> Iterator for ExportIterator<'m, N> {
    type Item = Export<N>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.count {
            return None;
        }

        let mut export_ffi = ffi::wasm_export_t::default();
        unsafe {
            ffi::wasm_runtime_get_export_type(self.module_ptr.as_ptr(), self.index, &mut export_ffi)
        };

        self.index += 1;

        let name = c_char_to_string_heapless(export_ffi.name).unwrap_or_default();
        let (kind, descriptor) = unsafe {
            match export_ffi.kind {
                ffi::wasm_import_export_kind_t_WASM_IMPORT_EXPORT_KIND_FUNC => (
                    ImportExportKind::Func,
                    ImportExportType::Func(FuncType {
                        ptr: NonNull::new_unchecked(export_ffi.u.func_type),
                    }),
                ),
                ffi::wasm_import_export_kind_t_WASM_IMPORT_EXPORT_KIND_TABLE => (
                    ImportExportKind::Table,
                    ImportExportType::Table(TableType {
                        ptr: NonNull::new_unchecked(export_ffi.u.table_type),
                    }),
                ),
                ffi::wasm_import_export_kind_t_WASM_IMPORT_EXPORT_KIND_MEMORY => (
                    ImportExportKind::Memory,
                    ImportExportType::Memory(MemoryType {
                        ptr: NonNull::new_unchecked(export_ffi.u.memory_type),
                    }),
                ),
                ffi::wasm_import_export_kind_t_WASM_IMPORT_EXPORT_KIND_GLOBAL => (
                    ImportExportKind::Global,
                    ImportExportType::Global(GlobalType {
                        ptr: NonNull::new_unchecked(export_ffi.u.global_type),
                    }),
                ),
                _ => return self.next(), // Skip unknown kinds
            }
        };

        Some(Export {
            name,
            kind,
            descriptor,
        })
    }
}
