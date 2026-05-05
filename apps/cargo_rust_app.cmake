# Unified Cargo workspace build for LeoDOS Rust cFS apps.
#
# Each Rust app's CMakeLists.txt should include this file and call
# cargo_rust_app(<app_name> [LIB <libname>] [PACKAGE <cargo_pkg>]).
# All registered apps build together in one `cargo build` invocation,
# letting cargo share fingerprints and compile shared dependencies once.

get_property(_cargo_init_done GLOBAL PROPERTY _CARGO_RUST_APP_INITIALIZED SET)
if(NOT _cargo_init_done)
    # Global property is per-cmake-run (unlike CACHE INTERNAL, which persists
    # across reconfigures even though add_custom_target does not).
    set_property(GLOBAL PROPERTY _CARGO_RUST_APP_INITIALIZED TRUE)

    # Project root contains the cargo workspace manifest. This file lives in
    # ${root}/apps/, so the parent of CMAKE_CURRENT_LIST_DIR is the root.
    get_filename_component(LEODOS_PROJECT_ROOT
        "${CMAKE_CURRENT_LIST_DIR}/.." ABSOLUTE)

    set(CARGO_TARGET_DIR "${MISSION_BINARY_DIR}/cargo_target"
        CACHE INTERNAL "Shared cargo target directory for all Rust cFS apps")
    set(CARGO_WORKSPACE_MANIFEST "${LEODOS_PROJECT_ROOT}/Cargo.toml"
        CACHE INTERNAL "Workspace root manifest")
    set(LEODOS_PROJECT_ROOT "${LEODOS_PROJECT_ROOT}"
        CACHE INTERNAL "Project root (contains Cargo.toml)")

    define_property(GLOBAL PROPERTY CARGO_RUST_PACKAGES
        BRIEF_DOCS "Cargo packages for cFS Rust apps"
        FULL_DOCS "List of -p arguments to pass to the unified cargo build")
    define_property(GLOBAL PROPERTY CARGO_RUST_OUTPUTS
        BRIEF_DOCS "Expected .so output paths"
        FULL_DOCS "Cargo target dir paths to declare as the workspace target's outputs")
    define_property(GLOBAL PROPERTY CARGO_RUST_DEPS
        BRIEF_DOCS "Source files the workspace build depends on"
        FULL_DOCS "Cargo.toml + lib.rs files of every registered app")

    cmake_language(DEFER DIRECTORY "${CMAKE_SOURCE_DIR}"
        CALL _cargo_workspace_finalize)
endif()

function(cargo_rust_app APP_NAME)
    cmake_parse_arguments(CRA "" "LIB;PACKAGE" "" ${ARGN})
    if(NOT CRA_LIB)
        set(CRA_LIB ${APP_NAME})
    endif()
    if(NOT CRA_PACKAGE)
        set(CRA_PACKAGE ${APP_NAME})
    endif()

    set(OUTPUT_LIB "${CARGO_TARGET_DIR}/release/lib${CRA_LIB}.so")

    set_property(GLOBAL APPEND PROPERTY CARGO_RUST_PACKAGES "${CRA_PACKAGE}")
    set_property(GLOBAL APPEND PROPERTY CARGO_RUST_OUTPUTS "${OUTPUT_LIB}")
    set_property(GLOBAL APPEND PROPERTY CARGO_RUST_DEPS
        "${CMAKE_CURRENT_SOURCE_DIR}/fsw/Cargo.toml"
        "${CMAKE_CURRENT_SOURCE_DIR}/fsw/src/lib.rs")

    file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/placeholder.c"
        "typedef int placeholder;\n")
    add_cfe_app(${APP_NAME} "${CMAKE_CURRENT_BINARY_DIR}/placeholder.c")

    add_dependencies(${APP_NAME} cargo_workspace_build)

    # Trigger the cFE app's link step (and therefore the POST_BUILD copy
    # below) whenever cargo's output changes. Without this the cFE target
    # appears up-to-date — its only source is placeholder.c which never
    # changes — and a fresh cdylib in CARGO_TARGET_DIR is silently ignored.
    set_property(TARGET ${APP_NAME} APPEND PROPERTY LINK_DEPENDS "${OUTPUT_LIB}")

    add_custom_command(TARGET ${APP_NAME} POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy "${OUTPUT_LIB}" $<TARGET_FILE:${APP_NAME}>
        COMMENT "Copying ${CRA_LIB} from shared cargo target dir")
endfunction()

function(_cargo_workspace_finalize)
    get_property(_packages GLOBAL PROPERTY CARGO_RUST_PACKAGES)
    get_property(_outputs  GLOBAL PROPERTY CARGO_RUST_OUTPUTS)
    get_property(_deps     GLOBAL PROPERTY CARGO_RUST_DEPS)

    if(NOT _packages)
        return()
    endif()

    set(_p_args "")
    foreach(pkg IN LISTS _packages)
        list(APPEND _p_args -p ${pkg})
    endforeach()

    add_custom_command(
        OUTPUT ${_outputs}
        COMMAND ${CMAKE_COMMAND} -E env
            CFE_DIR=${MISSION_SOURCE_DIR}/cfe
            OSAL_DIR=${MISSION_SOURCE_DIR}/osal
            PSP_DIR=${MISSION_SOURCE_DIR}/psp
            BUILD_DIR=${MISSION_BINARY_DIR}
            HWLIB_DIR=/cFS/libs/nos3/fsw/apps/hwlib
            NOS3_COMPONENTS_DIR=/cFS/libs/nos3/components
            CARGO_TARGET_DIR=${CARGO_TARGET_DIR}
            cargo build --release
                --manifest-path=${CARGO_WORKSPACE_MANIFEST}
                ${_p_args}
        WORKING_DIRECTORY "${MISSION_SOURCE_DIR}"
        COMMENT "Building Rust cFS apps via unified cargo workspace"
        DEPENDS ${CARGO_WORKSPACE_MANIFEST} ${_deps}
        VERBATIM)

    add_custom_target(cargo_workspace_build ALL DEPENDS ${_outputs})
endfunction()
