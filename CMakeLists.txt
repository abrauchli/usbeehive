cmake_minimum_required(VERSION 3.20)
project(whatcable-linux VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

find_package(Qt6 REQUIRED COMPONENTS Core)
find_package(PkgConfig REQUIRED)
pkg_check_modules(UDEV REQUIRED libudev)

add_subdirectory(src/core)
add_subdirectory(src/cli)
