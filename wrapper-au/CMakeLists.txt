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
project(te2_au VERSION 1.0.0 LANGUAGES C CXX OBJC OBJCXX)

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
    BUNDLE_VERSION "1.0.0"
    # Music effect: audio effect that accepts MIDI (position select / gating).
    INSTRUMENT_TYPE "aumf"
    MANUFACTURER_NAME "naturarum"
    MANUFACTURER_CODE "Ntrm"
    SUBTYPE_CODE "Rdmr"
)
