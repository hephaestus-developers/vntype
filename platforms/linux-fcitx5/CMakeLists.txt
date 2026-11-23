cmake_minimum_required(VERSION 3.16)
project(vntype-fcitx5 VERSION 0.1.0 LANGUAGES CXX)

include(GNUInstallDirs)
include(CTest)
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)
set(CMAKE_CXX_VISIBILITY_PRESET hidden)
set(CMAKE_VISIBILITY_INLINES_HIDDEN ON)

option(VNTYPE_FCITX5_TEST_ONLY "Chỉ biên dịch bộ kiểm thử C ABI (không cần header Fcitx5)" OFF)

if(NOT VNTYPE_STATIC_LIB)
    set(VNTYPE_STATIC_LIB "${CMAKE_CURRENT_SOURCE_DIR}/../../target/release/libvntype_ffi.a")
endif()

if(NOT VNTYPE_FCITX5_TEST_ONLY)
    find_package(Fcitx5Core REQUIRED)
    find_package(Fcitx5Utils REQUIRED)

    add_library(vntype MODULE
        src/engine.cpp
        src/ic_state.cpp)
    target_compile_features(vntype PRIVATE cxx_std_20)
    target_compile_options(vntype PRIVATE -Wall -Wextra -Wpedantic -fvisibility=hidden)
    target_include_directories(vntype PRIVATE
        "${CMAKE_CURRENT_SOURCE_DIR}/src"
        "${CMAKE_CURRENT_SOURCE_DIR}/../../crates/vntype-ffi/include")
    target_link_libraries(vntype PRIVATE
        Fcitx5::Core
        Fcitx5::Utils
        "${VNTYPE_STATIC_LIB}")

    if(UNIX AND NOT APPLE)
        # Ẩn toàn bộ biểu tượng của libvntype_ffi.a để không rò rỉ ra tiến trình fcitx5
        target_link_options(vntype PRIVATE "-Wl,--exclude-libs,ALL")
        target_link_libraries(vntype PRIVATE dl pthread m)
    endif()

    install(TARGETS vntype LIBRARY DESTINATION "${CMAKE_INSTALL_LIBDIR}/fcitx5")
    install(FILES vntype-addon.conf.in DESTINATION "${CMAKE_INSTALL_DATADIR}/fcitx5/addon" RENAME vntype.conf)
    install(FILES vntype.conf.in DESTINATION "${CMAKE_INSTALL_DATADIR}/fcitx5/inputmethod" RENAME vntype.conf)
endif()

if(BUILD_TESTING)
    add_executable(vntype-fcitx5-test
        tests/test_fcitx5_adapter.cpp)
    target_compile_features(vntype-fcitx5-test PRIVATE cxx_std_20)
    target_compile_options(vntype-fcitx5-test PRIVATE -Wall -Wextra -Wpedantic -UNDEBUG)
    target_include_directories(vntype-fcitx5-test PRIVATE
        "${CMAKE_CURRENT_SOURCE_DIR}/../../crates/vntype-ffi/include")
    target_link_libraries(vntype-fcitx5-test PRIVATE "${VNTYPE_STATIC_LIB}")
    if(UNIX AND NOT APPLE)
        target_link_libraries(vntype-fcitx5-test PRIVATE dl pthread m)
    endif()
    add_test(NAME vntype-fcitx5 COMMAND vntype-fcitx5-test)
endif()
