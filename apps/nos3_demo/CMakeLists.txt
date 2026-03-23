cmake_minimum_required(VERSION 3.10)

# Resolve symlinks so Cargo sees the real path (relative deps work).
get_filename_component(REAL_SOURCE_DIR ${CMAKE_CURRENT_SOURCE_DIR} REALPATH)

# cFS requires at least one C source to create the shared library
# target. The Rust cargo build overwrites it in the POST_BUILD step.
file(WRITE ${CMAKE_CURRENT_BINARY_DIR}/placeholder.c "typedef int placeholder;\n")

add_cfe_app(nos3_demo ${CMAKE_CURRENT_BINARY_DIR}/placeholder.c)

set(NOS3_DEMO_OUTPUT_LIB ${CMAKE_CURRENT_BINARY_DIR}/cargo_build/release/libnos3_demo.so)

add_custom_command(
    OUTPUT ${NOS3_DEMO_OUTPUT_LIB}
    COMMAND ${CMAKE_COMMAND} -E env
        CFE_DIR=${MISSION_SOURCE_DIR}/cfe
        OSAL_DIR=${MISSION_SOURCE_DIR}/osal
        PSP_DIR=${MISSION_SOURCE_DIR}/psp
        BUILD_DIR=${MISSION_BINARY_DIR}
        HWLIB_DIR=/cFS/libs/nos3/fsw/apps/hwlib
        NOS3_COMPONENTS_DIR=/cFS/libs/nos3/components
        CARGO_TARGET_DIR=${CMAKE_CURRENT_BINARY_DIR}/cargo_build
        cargo build --release --manifest-path=${REAL_SOURCE_DIR}/fsw/Cargo.toml
    WORKING_DIRECTORY ${REAL_SOURCE_DIR}/fsw
    COMMENT "Building nos3_demo app with Cargo"
    DEPENDS ${REAL_SOURCE_DIR}/fsw/Cargo.toml
            ${REAL_SOURCE_DIR}/fsw/src/lib.rs
    VERBATIM
)

add_custom_target(nos3_demo_lib ALL DEPENDS ${NOS3_DEMO_OUTPUT_LIB})

add_dependencies(nos3_demo nos3_demo_lib)

add_custom_command(TARGET nos3_demo POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy
        ${NOS3_DEMO_OUTPUT_LIB}
        $<TARGET_FILE:nos3_demo>
    COMMENT "Copying Cargo-built library to target"
)
