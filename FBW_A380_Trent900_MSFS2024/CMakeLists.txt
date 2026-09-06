cmake_minimum_required(VERSION 3.15)
project(trent900_wasm CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Source files
set(SOURCES
    trent900_core.cpp
    trent900_wasm.cpp
)

# Standalone C++ static library
add_library(trent900_core_lib STATIC ${SOURCES})
target_include_directories(trent900_core_lib PUBLIC ${CMAKE_CURRENT_SOURCE_DIR})

# Standalone Test Runner Executable
add_executable(test_trent900_wasm test_trent900_wasm.cpp ${SOURCES})
target_include_directories(test_trent900_wasm PRIVATE ${CMAKE_CURRENT_SOURCE_DIR})

# WASM Target compilation flag guide
# For MSFS WASM:
# clang++ --target=wasm32-wasi -O3 -shared -o trent900_engine.wasm trent900_core.cpp trent900_wasm.cpp
