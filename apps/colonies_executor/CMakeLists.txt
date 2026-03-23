cmake_minimum_required(VERSION 3.10)

# cFS requires at least one C source to create the shared library
# target. The Rust cargo build overwrites it in the POST_BUILD step.
file(WRITE ${CMAKE_CURRENT_BINARY_DIR}/placeholder.c "typedef int placeholder;\n")

add_cfe_app(colonies_executor ${CMAKE_CURRENT_BINARY_DIR}/placeholder.c)

set(RUST_OUTPUT_LIB
  ${CMAKE_CURRENT_BINARY_DIR}/cargo_build/release/libcolonies_executor.so
)

add_custom_command(
    OUTPUT ${RUST_OUTPUT_LIB}
    COMMAND ${CMAKE_COMMAND} -E env
        CFE_DIR=$ENV{CFE_DIR}
        OSAL_DIR=$ENV{OSAL_DIR}
        PSP_DIR=$ENV{PSP_DIR}
        BUILD_DIR=${MISSION_BINARY_DIR}
        CARGO_TARGET_DIR=${CMAKE_CURRENT_BINARY_DIR}/cargo_build
        cargo build --release --manifest-path=${CMAKE_CURRENT_SOURCE_DIR}/fsw/Cargo.toml
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/fsw
    COMMENT "Building Colonies App with Cargo"
    DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/fsw/Cargo.toml
            ${CMAKE_CURRENT_SOURCE_DIR}/fsw/src/lib.rs
            ${CMAKE_CURRENT_SOURCE_DIR}/fsw/src/functions.rs
            ${CMAKE_CURRENT_SOURCE_DIR}/fsw/src/transport.rs
    VERBATIM
)

add_custom_target(colonies_executor_lib ALL DEPENDS ${RUST_OUTPUT_LIB})
add_dependencies(colonies_executor colonies_executor_lib)

add_custom_command(TARGET colonies_executor POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy
        ${RUST_OUTPUT_LIB}
        $<TARGET_FILE:colonies_executor>
    COMMENT "Copying Cargo-built library to target"
)
