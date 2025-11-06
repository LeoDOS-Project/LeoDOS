//! Representations of WebAssembly values.

use crate::{Result, WamrError, ffi};

/// Represents a WebAssembly value type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WasmValueKind {
    I32 = ffi::wasm_valkind_enum_WASM_I32 as u8,
    I64 = ffi::wasm_valkind_enum_WASM_I64 as u8,
    F32 = ffi::wasm_valkind_enum_WASM_F32 as u8,
    F64 = ffi::wasm_valkind_enum_WASM_F64 as u8,
    FuncRef = ffi::wasm_valkind_enum_WASM_FUNCREF as u8,
    ExternRef = ffi::wasm_valkind_enum_WASM_EXTERNREF as u8,
}

impl TryFrom<ffi::wasm_valkind_t> for WasmValueKind {
    type Error = WamrError;
    fn try_from(value: ffi::wasm_valkind_t) -> Result<Self> {
        match value as u32 {
            ffi::wasm_valkind_enum_WASM_I32 => Ok(WasmValueKind::I32),
            ffi::wasm_valkind_enum_WASM_I64 => Ok(WasmValueKind::I64),
            ffi::wasm_valkind_enum_WASM_F32 => Ok(WasmValueKind::F32),
            ffi::wasm_valkind_enum_WASM_F64 => Ok(WasmValueKind::F64),
            ffi::wasm_valkind_enum_WASM_FUNCREF => Ok(WasmValueKind::FuncRef),
            ffi::wasm_valkind_enum_WASM_EXTERNREF => Ok(WasmValueKind::ExternRef),
            _ => Err(WamrError::InvalidWasmValue),
        }
    }
}

/// Represents a WebAssembly value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WasmValue {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
}

impl WasmValue {
    pub fn kind(&self) -> WasmValueKind {
        match self {
            WasmValue::I32(_) => WasmValueKind::I32,
            WasmValue::I64(_) => WasmValueKind::I64,
            WasmValue::F32(_) => WasmValueKind::F32,
            WasmValue::F64(_) => WasmValueKind::F64,
        }
    }
}

impl From<WasmValue> for ffi::wasm_val_t {
    fn from(val: WasmValue) -> Self {
        let mut ffi_val = ffi::wasm_val_t {
            kind: val.kind() as u8,
            ..Default::default()
        };
        match val {
            WasmValue::I32(i) => ffi_val.of.i32_ = i,
            WasmValue::I64(i) => ffi_val.of.i64_ = i,
            WasmValue::F32(f) => ffi_val.of.f32_ = f,
            WasmValue::F64(f) => ffi_val.of.f64_ = f,
        }
        ffi_val
    }
}

impl TryFrom<ffi::wasm_val_t> for WasmValue {
    type Error = WamrError;

    fn try_from(val: ffi::wasm_val_t) -> Result<Self> {
        unsafe {
            match val.kind as u32 {
                ffi::wasm_valkind_enum_WASM_I32 => Ok(WasmValue::I32(val.of.i32_)),
                ffi::wasm_valkind_enum_WASM_I64 => Ok(WasmValue::I64(val.of.i64_)),
                ffi::wasm_valkind_enum_WASM_F32 => Ok(WasmValue::F32(val.of.f32_)),
                ffi::wasm_valkind_enum_WASM_F64 => Ok(WasmValue::F64(val.of.f64_)),
                _ => Err(WamrError::InvalidWasmValue),
            }
        }
    }
}
