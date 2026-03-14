# KIO worker for Google Drive (Dolphin integration)
# Requires KF6. Minimal C++ shim — all logic in daemon via D-Bus.

cmake_minimum_required(VERSION 3.16)
project(gdrivekio LANGUAGES CXX)

find_package(KF6KIO REQUIRED)

add_library(gdrivekio MODULE
    src/gdrivekio.cpp
)
target_link_libraries(gdrivekio KF6::KIO)
set_target_properties(gdrivekio PROPERTIES
    PREFIX ""
    OUTPUT_NAME "gdrive"
    INSTALL_RPATH "${KDE_INSTALL_RPATH}"
    SKIP_BUILD_RPATH FALSE
    BUILD_WITH_INSTALL_RPATH FALSE
)
install(TARGETS gdrivekio DESTINATION ${KDE_INSTALL_PLUGINDIR}/kf6/kio)
