cmake_minimum_required(VERSION 3.10)

# Resolve symlinks so Cargo sees the real path (relative deps work).
get_filename_component(REAL_SOURCE_DIR ${CMAKE_CURRENT_SOURCE_DIR} REALPATH)

# cFS requires at least one C source to create the shared library
# target. The Rust cargo build overwrites it in the POST_BUILD step.
file(WRITE ${CMAKE_CURRENT_BINARY_DIR}/placeholder.c "typedef int placeholder;\n")

add_cfe_app(gossip ${CMAKE_CURRENT_BINARY_DIR}/placeholder.c)

set(GOSSIP_OUTPUT_LIB ${CMAKE_CURRENT_BINARY_DIR}/cargo_build/release/libgossip.so)

add_custom_command(
    OUTPUT ${GOSSIP_OUTPUT_LIB}
    COMMAND ${CMAKE_COMMAND} -E env
        CFE_DIR=${MISSION_SOURCE_DIR}/cfe
        OSAL_DIR=${MISSION_SOURCE_DIR}/osal
        PSP_DIR=${MISSION_SOURCE_DIR}/psp
        BUILD_DIR=${MISSION_BINARY_DIR}
        CARGO_TARGET_DIR=${CMAKE_CURRENT_BINARY_DIR}/cargo_build
        cargo build --release --manifest-path=${REAL_SOURCE_DIR}/fsw/Cargo.toml
    WORKING_DIRECTORY ${REAL_SOURCE_DIR}/fsw
    COMMENT "Building gossip app with Cargo"
    DEPENDS ${REAL_SOURCE_DIR}/fsw/Cargo.toml
            ${REAL_SOURCE_DIR}/fsw/src/lib.rs
    VERBATIM
)

add_custom_target(gossip_lib ALL DEPENDS ${GOSSIP_OUTPUT_LIB})

add_dependencies(gossip gossip_lib)

add_custom_command(TARGET gossip POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy
        ${GOSSIP_OUTPUT_LIB}
        $<TARGET_FILE:gossip>
    COMMENT "Copying Cargo-built library to target"
)
