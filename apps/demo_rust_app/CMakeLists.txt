# Set the minimum required version of CMake.
cmake_minimum_required(VERSION 3.10)

# 1. Define a placeholder C source file. This is just to satisfy the
#    `add_cfe_app` function signature.
set(APP_SRC_FILES
  fsw/src/placeholder.c
)

# 2. Register the application with the cFS build system.
#    This creates the "rust" target with all the standard cFS compiler flags,
#    include paths, and default link libraries.
add_cfe_app(rust ${APP_SRC_FILES})

# 3. Define the path to where Cargo will place our compiled library.
#    We build in release mode, so the library will be in the `release` subdirectory.
set(RUST_OUTPUT_LIB ${CMAKE_CURRENT_BINARY_DIR}/cargo_build/release/librust.so)

# 4. Create a custom command to invoke Cargo.
add_custom_command(
    OUTPUT ${RUST_OUTPUT_LIB}
    COMMAND ${CMAKE_COMMAND} -E env
        # Pass necessary environment variables to the Rust build script (build.rs).
        CFE_DIR=$ENV{CFE_DIR}
        OSAL_DIR=$ENV{OSAL_DIR}
        PSP_DIR=$ENV{PSP_DIR}
        BUILD_DIR=${MISSION_BINARY_DIR}
        # Set the CARGO_TARGET_DIR to a location inside CMake's build directory.
        # This keeps all build artifacts in one place.
        CARGO_TARGET_DIR=${CMAKE_CURRENT_BINARY_DIR}/cargo_build
        # Invoke Cargo to build the library in release mode.
        cargo build --release --manifest-path=${CMAKE_CURRENT_SOURCE_DIR}/fsw/Cargo.toml
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/fsw
    COMMENT "Building Rust app with Cargo"
    # Re-run this command if the source code or project file changes.
    DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/fsw/Cargo.toml
            ${CMAKE_CURRENT_SOURCE_DIR}/fsw/src/lib.rs
    VERBATIM
)

# 5. Create a target that represents the result of the Cargo build.
add_custom_target(rust_lib ALL DEPENDS ${RUST_OUTPUT_LIB})

# 6. Make the main "rust" target (from add_cfe_app) depend on the Cargo build.
add_dependencies(rust rust_lib)

# 7. Copy the Cargo-built library directly to the target location
add_custom_command(TARGET rust POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy
        ${RUST_OUTPUT_LIB}
        $<TARGET_FILE:rust>
    COMMENT "Copying Cargo-built library to target"
)
