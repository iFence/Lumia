cmake_minimum_required(VERSION 3.20)
project(lumia_raw_bridge VERSION 1.0.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

find_path(LIBRAW_INCLUDE_DIR NAMES libraw/libraw.h REQUIRED)
find_library(LIBRAW_LIBRARY NAMES raw raw_r libraw REQUIRED)

add_library(lumia_raw_bridge SHARED src/lumia_raw_bridge.cpp)
target_include_directories(lumia_raw_bridge
  PRIVATE
    ${LIBRAW_INCLUDE_DIR}
    ${CMAKE_CURRENT_SOURCE_DIR}/include
)
target_link_libraries(lumia_raw_bridge PRIVATE ${LIBRAW_LIBRARY})
if(WIN32)
  set_target_properties(lumia_raw_bridge PROPERTIES PREFIX "")
endif()


if(MSVC)
  target_compile_options(lumia_raw_bridge PRIVATE /W4 /permissive- /EHsc)
else()
  target_compile_options(lumia_raw_bridge PRIVATE -Wall -Wextra -Wpedantic)
  set_target_properties(lumia_raw_bridge PROPERTIES
    BUILD_RPATH "$ORIGIN"
    INSTALL_RPATH "$ORIGIN"
  )
  if(APPLE)
    set_target_properties(lumia_raw_bridge PROPERTIES
      BUILD_RPATH "@loader_path"
      INSTALL_RPATH "@loader_path"
    )
  endif()
endif()
