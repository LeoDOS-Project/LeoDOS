cmake_minimum_required(VERSION 3.10)

set(APP_SRC_FILES
  fsw/src/placeholder.c
)

add_cfe_app(spacecomp ${APP_SRC_FILES})

set(SPACECOMP_OUTPUT_LIB ${CMAKE_CURRENT_BINARY_DIR}/cargo_build/release/libspacecomp.so)

add_custom_command(
    OUTPUT ${SPACECOMP_OUTPUT_LIB}
    COMMAND ${CMAKE_COMMAND} -E env
        CFE_DIR=$ENV{CFE_DIR}
        OSAL_DIR=$ENV{OSAL_DIR}
        PSP_DIR=$ENV{PSP_DIR}
        BUILD_DIR=${MISSION_BINARY_DIR}
        CARGO_TARGET_DIR=${CMAKE_CURRENT_BINARY_DIR}/cargo_build
        cargo build --release --manifest-path=${CMAKE_CURRENT_SOURCE_DIR}/fsw/Cargo.toml
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/fsw
    COMMENT "Building spacecomp with Cargo"
    DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/fsw/Cargo.toml
            ${CMAKE_CURRENT_SOURCE_DIR}/fsw/src/lib.rs
    VERBATIM
)

add_custom_target(spacecomp_lib ALL DEPENDS ${SPACECOMP_OUTPUT_LIB})

add_dependencies(spacecomp spacecomp_lib)

add_custom_command(TARGET spacecomp POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy
        ${SPACECOMP_OUTPUT_LIB}
        $<TARGET_FILE:spacecomp>
    COMMENT "Copying Cargo-built library to target"
)
