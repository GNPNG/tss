cmake_minimum_required(VERSION 3.24)
project(luxcore_session_probe LANGUAGES CXX)

# Point to the exact LuxCore source release and its 2.11.2 sample bundle.
set(LUXCORE_SOURCE "" CACHE PATH "Pinned LuxCore source checkout")
set(LUXCORE_BUNDLE "" CACHE PATH "LuxCore sample bundle root")
set(BOOST_INCLUDE "" CACHE PATH "Boost include directory from LuxCoreDeps")
set(LUXCORE_VERSION "2.11.2")
set(LUXRAYS_ENABLE_OPENCL "ON")
set(LUXRAYS_ENABLE_CUDA "ON")
set(LUXRAYS_ENABLE_OPTIX "ON")
file(MAKE_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/generated/luxcore")
configure_file("${LUXCORE_SOURCE}/include/luxcore/cfg.h.in"
  "${CMAKE_CURRENT_BINARY_DIR}/generated/luxcore/cfg.h" @ONLY)

add_executable(luxcore_session_probe session_probe.cpp)
target_compile_features(luxcore_session_probe PRIVATE cxx_std_20)
target_compile_definitions(luxcore_session_probe PRIVATE LUXCORE_DLL)
target_include_directories(luxcore_session_probe PRIVATE
  "${CMAKE_CURRENT_BINARY_DIR}/generated" "${LUXCORE_SOURCE}/include" "${BOOST_INCLUDE}")
target_link_libraries(luxcore_session_probe PRIVATE "${LUXCORE_BUNDLE}/lib/luxcore.lib")

set(BOOST_SERIALIZATION_LIB "" CACHE FILEPATH "Boost.Serialization library from LuxCoreDeps")
add_executable(serialization_repro serialization_repro.cpp)
target_compile_features(serialization_repro PRIVATE cxx_std_20)
target_compile_definitions(serialization_repro PRIVATE BOOST_ALL_NO_LIB)
target_include_directories(serialization_repro PRIVATE "${BOOST_INCLUDE}")
target_link_libraries(serialization_repro PRIVATE "${BOOST_SERIALIZATION_LIB}")
