cmake_minimum_required(VERSION 3.20)
project(zspar VERSION 1.0.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

add_library(zspar INTERFACE)
target_include_directories(zspar INTERFACE
    $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>
    $<INSTALL_INTERFACE:include>
)

enable_testing()

add_executable(test_zspar tests/test_zspar.cpp)
target_link_libraries(test_zspar PRIVATE zspar)
add_test(NAME test_zspar COMMAND test_zspar)

add_executable(exhaustive_rs tests/exhaustive_rs.cpp)
target_link_libraries(exhaustive_rs PRIVATE zspar)
add_test(NAME exhaustive_rs COMMAND exhaustive_rs)
