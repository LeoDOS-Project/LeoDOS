# Set the minimum required version of CMake.
cmake_minimum_required(VERSION 3.10)

# cFS requires at least one C source to create the shared library
# target. The Rust cargo build overwrites it in the POST_BUILD step.
file(WRITE ${CMAKE_CURRENT_BINARY_DIR}/placeholder.c "typedef int placeholder;\n")

add_cfe_app(sample_rust_app ${CMAKE_CURRENT_BINARY_DIR}/placeholder.c)

# 3. Define the path to where Cargo will place our compiled library.
#    We build in release mode, so the library will be in the `release` subdirectory.
set(SAMPLE_RUST_APP_OUTPUT_LIB ${CMAKE_CURRENT_BINARY_DIR}/cargo_build/release/libsample_rust_app.so)

# 4. Create a custom command to invoke Cargo.
add_custom_command(
    OUTPUT ${SAMPLE_RUST_APP_OUTPUT_LIB}
    COMMAND ${CMAKE_COMMAND} -E env
        CFE_DIR=${MISSION_SOURCE_DIR}/cfe
        OSAL_DIR=${MISSION_SOURCE_DIR}/osal
        PSP_DIR=${MISSION_SOURCE_DIR}/psp
        BUILD_DIR=${MISSION_BINARY_DIR}
        CARGO_TARGET_DIR=${CMAKE_CURRENT_BINARY_DIR}/cargo_build
        cargo build --release --manifest-path=${CMAKE_CURRENT_SOURCE_DIR}/fsw/Cargo.toml
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/fsw
    COMMENT "Building sample_rust_app app with Cargo"
    # Re-run this command if the source code or project file changes.
    DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/fsw/Cargo.toml
            ${CMAKE_CURRENT_SOURCE_DIR}/fsw/src/lib.rs
    VERBATIM
)

# 5. Create a target that represents the result of the Cargo build.
add_custom_target(sample_rust_app_lib ALL DEPENDS ${SAMPLE_RUST_APP_OUTPUT_LIB})

# 6. Make the main "sample_rust_app" target (from add_cfe_app) depend on the Cargo build.
add_dependencies(sample_rust_app sample_rust_app_lib)

# 7. Copy the Cargo-built library directly to the target location
add_custom_command(TARGET sample_rust_app POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy
        ${SAMPLE_RUST_APP_OUTPUT_LIB}
        $<TARGET_FILE:sample_rust_app>
    COMMENT "Copying Cargo-built library to target"
)
