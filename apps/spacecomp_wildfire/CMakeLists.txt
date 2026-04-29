cmake_minimum_required(VERSION 3.19)

include(${CMAKE_CURRENT_LIST_DIR}/../cargo_rust_app.cmake)

cargo_rust_app(sc_wildfire LIB sc_wildfire PACKAGE spacecomp-wildfire)
