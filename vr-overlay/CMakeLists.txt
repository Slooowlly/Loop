cmake_minimum_required(VERSION 3.20)
project(iracer_vr_overlay LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Puxa só os HEADERS do OpenXR-SDK (não linkamos o loader: uma API layer é
# CARREGADA pelo loader do jogo, não o contém).
#
# O pin é o SHA DO COMMIT, não a tag. Tag no git é um ponteiro móvel: quem publica pode
# reapontar `release-1.1.43` para outro commit a qualquer momento, e o build passaria a
# compilar contra headers diferentes sem uma linha de diff aqui. O SHA é o conteúdo.
# Para subir de versão: pegue o commit da release nova em
# github.com/KhronosGroup/OpenXR-SDK/releases e troque os DOIS pins (aqui e em
# scripts/build-vr-layer.mjs, que o guard cobra que sejam iguais).
#
# GIT_SHALLOW fica DESLIGADO de propósito: com SHA, um clone raso precisaria que o commit
# fosse a ponta de um ref, e ele não é. O FetchContent faz o init+fetch do SHA sozinho.
include(FetchContent)
FetchContent_Declare(
  OpenXR
  GIT_REPOSITORY https://github.com/KhronosGroup/OpenXR-SDK.git
  GIT_TAG        781f2eab3698d653c804ecbd11e0aed47eaad1c6 # release-1.1.43
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
  # /utf-8 = fonte E execução em UTF-8. Sem ele o cl lê o .cpp na codepage do sistema, e
  # todo acento das mensagens de log vira mojibake no %TEMP%\iracer_overlay_layer.log —
  # justo o arquivo que a gente usa pra descobrir por que o overlay não apareceu.
  target_compile_options(iracer_overlay_layer PRIVATE /W4 /utf-8)
endif()
