cmake_minimum_required(VERSION 3.10)

# Resolve symlinks so Cargo sees the real path (relative deps work).
get_filename_component(REAL_SOURCE_DIR ${CMAKE_CURRENT_SOURCE_DIR} REALPATH)

# cFS requires at least one C source to create the shared library
# target. The Rust cargo build overwrites it in the POST_BUILD step.
file(WRITE ${CMAKE_CURRENT_BINARY_DIR}/placeholder.c "")

add_cfe_app(fs_srv ${CMAKE_CURRENT_BINARY_DIR}/placeholder.c)

set(FS_SRV_OUTPUT_LIB ${CMAKE_CURRENT_BINARY_DIR}/cargo_build/release/libfs_srv.so)

add_custom_command(
    OUTPUT ${FS_SRV_OUTPUT_LIB}
    COMMAND ${CMAKE_COMMAND} -E env
        CFE_DIR=$ENV{CFE_DIR}
        OSAL_DIR=$ENV{OSAL_DIR}
        PSP_DIR=$ENV{PSP_DIR}
        BUILD_DIR=${MISSION_BINARY_DIR}
        CARGO_TARGET_DIR=${CMAKE_CURRENT_BINARY_DIR}/cargo_build
        cargo build --release --manifest-path=${REAL_SOURCE_DIR}/fsw/Cargo.toml
    WORKING_DIRECTORY ${REAL_SOURCE_DIR}/fsw
    COMMENT "Building fs_srv app with Cargo"
    DEPENDS ${REAL_SOURCE_DIR}/fsw/Cargo.toml
            ${REAL_SOURCE_DIR}/fsw/src/lib.rs
    VERBATIM
)

add_custom_target(fs_srv_lib ALL DEPENDS ${FS_SRV_OUTPUT_LIB})

add_dependencies(fs_srv fs_srv_lib)

add_custom_command(TARGET fs_srv POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy
        ${FS_SRV_OUTPUT_LIB}
        $<TARGET_FILE:fs_srv>
    COMMENT "Copying Cargo-built library to target"
)
