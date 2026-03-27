cmake_minimum_required(VERSION 3.10)

get_filename_component(REAL_SOURCE_DIR ${CMAKE_CURRENT_SOURCE_DIR} REALPATH)

file(WRITE ${CMAKE_CURRENT_BINARY_DIR}/placeholder.c "typedef int placeholder;\n")

add_cfe_app(spacecomp_wildfire ${CMAKE_CURRENT_BINARY_DIR}/placeholder.c)

set(OUTPUT_LIB ${CMAKE_CURRENT_BINARY_DIR}/cargo_build/release/libspacecomp_wildfire.so)

add_custom_command(
    OUTPUT ${OUTPUT_LIB}
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
    COMMENT "Building spacecomp_wildfire with Cargo"
    DEPENDS ${REAL_SOURCE_DIR}/fsw/Cargo.toml
            ${REAL_SOURCE_DIR}/fsw/src/lib.rs
    VERBATIM
)

add_custom_target(spacecomp_wildfire_lib ALL DEPENDS ${OUTPUT_LIB})

add_dependencies(spacecomp_wildfire spacecomp_wildfire_lib)

add_custom_command(TARGET spacecomp_wildfire POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy
        ${OUTPUT_LIB}
        $<TARGET_FILE:spacecomp_wildfire>
    COMMENT "Copying Cargo-built library to target"
)
