cmake_minimum_required(VERSION 3.10)

set(APP_SRC_FILES
  fsw/src/placeholder.c
)

add_cfe_app(srspp_receiver ${APP_SRC_FILES})

set(SRSPP_RECEIVER_OUTPUT_LIB ${CMAKE_CURRENT_BINARY_DIR}/cargo_build/release/libsrspp_receiver.so)

add_custom_command(
    OUTPUT ${SRSPP_RECEIVER_OUTPUT_LIB}
    COMMAND ${CMAKE_COMMAND} -E env
        CFE_DIR=$ENV{CFE_DIR}
        OSAL_DIR=$ENV{OSAL_DIR}
        PSP_DIR=$ENV{PSP_DIR}
        BUILD_DIR=${MISSION_BINARY_DIR}
        CARGO_TARGET_DIR=${CMAKE_CURRENT_BINARY_DIR}/cargo_build
        cargo build --release --manifest-path=${CMAKE_CURRENT_SOURCE_DIR}/fsw/Cargo.toml
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/fsw
    COMMENT "Building srspp_receiver with Cargo"
    DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/fsw/Cargo.toml
            ${CMAKE_CURRENT_SOURCE_DIR}/fsw/src/lib.rs
    VERBATIM
)

add_custom_target(srspp_receiver_lib ALL DEPENDS ${SRSPP_RECEIVER_OUTPUT_LIB})

add_dependencies(srspp_receiver srspp_receiver_lib)

add_custom_command(TARGET srspp_receiver POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy
        ${SRSPP_RECEIVER_OUTPUT_LIB}
        $<TARGET_FILE:srspp_receiver>
    COMMENT "Copying Cargo-built library to target"
)
