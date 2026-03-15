cmake_minimum_required(VERSION 3.10)

# cFS requires at least one C source to create the shared library
# target. The Rust cargo build overwrites it in the POST_BUILD step.
file(WRITE ${CMAKE_CURRENT_BINARY_DIR}/placeholder.c "")

add_cfe_app(srspp_sender ${CMAKE_CURRENT_BINARY_DIR}/placeholder.c)

set(SRSPP_SENDER_OUTPUT_LIB ${CMAKE_CURRENT_BINARY_DIR}/cargo_build/release/libsrspp_sender.so)

add_custom_command(
    OUTPUT ${SRSPP_SENDER_OUTPUT_LIB}
    COMMAND ${CMAKE_COMMAND} -E env
        CFE_DIR=$ENV{CFE_DIR}
        OSAL_DIR=$ENV{OSAL_DIR}
        PSP_DIR=$ENV{PSP_DIR}
        BUILD_DIR=${MISSION_BINARY_DIR}
        CARGO_TARGET_DIR=${CMAKE_CURRENT_BINARY_DIR}/cargo_build
        cargo build --release --manifest-path=${CMAKE_CURRENT_SOURCE_DIR}/fsw/Cargo.toml
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/fsw
    COMMENT "Building srspp_sender with Cargo"
    DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/fsw/Cargo.toml
            ${CMAKE_CURRENT_SOURCE_DIR}/fsw/src/lib.rs
    VERBATIM
)

add_custom_target(srspp_sender_lib ALL DEPENDS ${SRSPP_SENDER_OUTPUT_LIB})

add_dependencies(srspp_sender srspp_sender_lib)

add_custom_command(TARGET srspp_sender POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy
        ${SRSPP_SENDER_OUTPUT_LIB}
        $<TARGET_FILE:srspp_sender>
    COMMENT "Copying Cargo-built library to target"
)
