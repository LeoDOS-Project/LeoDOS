cmake_minimum_required(VERSION 3.10)

file(WRITE ${CMAKE_CURRENT_BINARY_DIR}/placeholder.c "typedef int placeholder;\n")
add_cfe_app(ping ${CMAKE_CURRENT_BINARY_DIR}/placeholder.c)

set(PING_OUTPUT_LIB ${CMAKE_CURRENT_BINARY_DIR}/cargo_build/release/libping.so)

add_custom_command(
    OUTPUT ${PING_OUTPUT_LIB}
    COMMAND ${CMAKE_COMMAND} -E env
        CFE_DIR=${MISSION_SOURCE_DIR}/cfe
        OSAL_DIR=${MISSION_SOURCE_DIR}/osal
        PSP_DIR=${MISSION_SOURCE_DIR}/psp
        BUILD_DIR=${MISSION_BINARY_DIR}
        HWLIB_DIR=/cFS/libs/nos3/fsw/apps/hwlib
        NOS3_COMPONENTS_DIR=/cFS/libs/nos3/components
        CARGO_TARGET_DIR=${CMAKE_CURRENT_BINARY_DIR}/cargo_build
        cargo build --release --manifest-path=${CMAKE_CURRENT_SOURCE_DIR}/fsw/Cargo.toml
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/fsw
    COMMENT "Building ping with Cargo"
    DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/fsw/Cargo.toml
            ${CMAKE_CURRENT_SOURCE_DIR}/fsw/src/lib.rs
    VERBATIM
)

add_custom_target(ping_lib ALL DEPENDS ${PING_OUTPUT_LIB})
add_dependencies(ping ping_lib)

add_custom_command(TARGET ping POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy
        ${PING_OUTPUT_LIB}
        $<TARGET_FILE:ping>
    COMMENT "Copying Cargo-built library to target"
)
