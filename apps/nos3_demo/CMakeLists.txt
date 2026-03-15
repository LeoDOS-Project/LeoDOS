cmake_minimum_required(VERSION 3.10)

# Resolve symlinks so Cargo sees the real path (relative deps work).
get_filename_component(REAL_SOURCE_DIR ${CMAKE_CURRENT_SOURCE_DIR} REALPATH)

set(APP_SRC_FILES
  fsw/src/placeholder.c
)

add_cfe_app(nos3_demo ${APP_SRC_FILES})

set(NOS3_DEMO_OUTPUT_LIB ${CMAKE_CURRENT_BINARY_DIR}/cargo_build/release/libnos3_demo.so)

add_custom_command(
    OUTPUT ${NOS3_DEMO_OUTPUT_LIB}
    COMMAND ${CMAKE_COMMAND} -E env
        CFE_DIR=$ENV{CFE_DIR}
        OSAL_DIR=$ENV{OSAL_DIR}
        PSP_DIR=$ENV{PSP_DIR}
        BUILD_DIR=${MISSION_BINARY_DIR}
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
