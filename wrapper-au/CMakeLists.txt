# AUv2 wrapper for the RE-DEEMER CLAP, via free-audio/clap-wrapper.
#
# The produced .component dynamically loads the CLAP of the same name from
# the standard CLAP locations, so install "RE-DEEMER.clap" into
# ~/Library/Audio/Plug-Ins/CLAP first.
#
# Build:
#   cmake -B build -DCMAKE_BUILD_TYPE=Release
#   cmake --build build
#   cp -r "build/RE-DEEMER.component" ~/Library/Audio/Plug-Ins/Components/

cmake_minimum_required(VERSION 3.21)
project(te2_au VERSION 1.0.2 LANGUAGES C CXX OBJC OBJCXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_OSX_ARCHITECTURES "arm64;x86_64" CACHE STRING "" FORCE)

set(CLAP_WRAPPER_DOWNLOAD_DEPENDENCIES TRUE CACHE BOOL "" FORCE)
set(CLAP_WRAPPER_BUILD_AUV2 ON CACHE BOOL "" FORCE)

add_subdirectory(clap-wrapper)

add_library(te2_as_auv2 MODULE)
target_add_auv2_wrapper(
    TARGET te2_as_auv2
    OUTPUT_NAME "RE-DEEMER"
    BUNDLE_IDENTIFIER "com.naturarum.re-deemer.auv2"
    BUNDLE_VERSION "1.0.2"
    # Music effect: audio effect that accepts MIDI (position select / gating).
    INSTRUMENT_TYPE "aumf"
    MANUFACTURER_NAME "naturarum"
    MANUFACTURER_CODE "Ntrm"
    SUBTYPE_CODE "Rdmr"
)

# clap-wrapper's build helper writes the real Info.plist (with the
# AudioComponents registration block) on the FIRST build only; on incremental
# rebuilds CMake regenerates its default bundle plist and the component stops
# registering with the system entirely (auval: "didn't find the component").
# This shipped broken once — force the generated plist onto the bundle on
# every build, and the matching BNDL package type with it.
#
# The helper also mis-parses 3-segment versions (drops the patch digit:
# "1.0.1" -> 65536), so the AudioComponents version integer is recomputed
# here from the project version (hosts use it to spot updated components).
math(EXPR AU_COMPONENT_VERSION
    "${PROJECT_VERSION_MAJOR} * 65536 + ${PROJECT_VERSION_MINOR} * 256 + ${PROJECT_VERSION_PATCH}")
add_custom_command(TARGET te2_as_auv2 POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy
        "${CMAKE_BINARY_DIR}/te2_as_auv2-build-helper-output/auv2_Info.plist"
        "$<TARGET_BUNDLE_DIR:te2_as_auv2>/Contents/Info.plist"
    COMMAND plutil -replace AudioComponents.0.version
        -integer ${AU_COMPONENT_VERSION}
        "$<TARGET_BUNDLE_DIR:te2_as_auv2>/Contents/Info.plist"
    COMMAND /bin/sh -c "printf 'BNDL????' > '$<TARGET_BUNDLE_DIR:te2_as_auv2>/Contents/PkgInfo'"
    COMMENT "Restoring AUv2 AudioComponents Info.plist (version ${AU_COMPONENT_VERSION})"
    VERBATIM
)
