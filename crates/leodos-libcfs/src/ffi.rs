/*!
Low-level FFI bindings for cFE, OSAL, and PSP.

This module contains the raw, `unsafe` function and type definitions generated
by `rust-bindgen`. It is not intended for direct use by applications. Instead,
the safe, idiomatic wrappers in other modules of this crate should be used.
*/

#![allow(clippy::all)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(overflowing_literals)]
#![allow(missing_docs)]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

// These were too complex to generate with bindgen for now:
pub const CFE_RESOURCEID_UNDEFINED: CFE_ResourceId_t = 0;
pub const CFE_ES_APPID_UNDEFINED: CFE_ES_AppId_t = CFE_RESOURCEID_UNDEFINED;
pub const CFE_ES_TASKID_UNDEFINED: CFE_ES_TaskId_t = CFE_RESOURCEID_UNDEFINED;
pub const CFE_ES_LIBID_UNDEFINED: CFE_ES_LibId_t = CFE_RESOURCEID_UNDEFINED;
pub const CFE_ES_COUNTERID_UNDEFINED: CFE_ES_CounterId_t = CFE_RESOURCEID_UNDEFINED;
pub const CFE_ES_MEMHANDLE_UNDEFINED: CFE_ES_MemHandle_t = CFE_RESOURCEID_UNDEFINED;
pub const CFE_ES_CDS_BAD_HANDLE: CFE_ES_CDSHandle_t = CFE_RESOURCEID_UNDEFINED;
pub const CFE_SB_INVALID_PIPE: CFE_SB_PipeId_t = CFE_RESOURCEID_UNDEFINED;

#[doc = "\\brief Default Qos macro"]
pub const CFE_SB_DEFAULT_QOS: CFE_SB_Qos_t = CFE_SB_Qos_t {
    Priority: 0,
    Reliability: 0,
};

#[doc = "@brief Initializer for the osal_id_t type which will not match any valid value"]
pub const OS_OBJECT_ID_UNDEFINED: osal_id_t = 0;

#[doc = "@brief Constant that may be passed to OS_ForEachObject()/OS_ForEachObjectOfType() to match any\ncreator (i.e. get all objects)"]
pub const OS_OBJECT_CREATOR_ANY: osal_id_t = OS_OBJECT_ID_UNDEFINED;
