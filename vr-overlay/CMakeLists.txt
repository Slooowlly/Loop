cmake_minimum_required(VERSION 3.20)
project(iracer_vr_overlay LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Puxa só os HEADERS do OpenXR-SDK (não linkamos o loader: uma API layer é
# CARREGADA pelo loader do jogo, não o contém). Se a tag falhar, é só trocar por
# uma release existente em github.com/KhronosGroup/OpenXR-SDK/releases.
include(FetchContent)
FetchContent_Declare(
  OpenXR
  GIT_REPOSITORY https://github.com/KhronosGroup/OpenXR-SDK.git
  GIT_TAG        release-1.1.43
  GIT_SHALLOW    ON
)
FetchContent_MakeAvailable(OpenXR)

add_library(iracer_overlay_layer SHARED src/overlay_layer.cpp)

target_link_libraries(iracer_overlay_layer PRIVATE
  OpenXR::headers
  d3d11
  dxgi
  user32   # GetAsyncKeyState (ajuste de posição por teclado)
)

# Nome fixo do .dll (o manifesto/registro aponta pra ele).
set_target_properties(iracer_overlay_layer PROPERTIES
  OUTPUT_NAME "iracer_overlay_layer"
)

if (MSVC)
  target_compile_options(iracer_overlay_layer PRIVATE /W4)
endif()
