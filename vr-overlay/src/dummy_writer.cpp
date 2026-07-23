// Escritor de TESTE do Spike 2a. NÃO faz parte do produto — só prova a ponte de
// memória compartilhada, no lugar do app real.
//
// Cria o bloco compartilhado e fica preenchendo um gradiente que ANDA (pra dar
// pra ver, no óculos, que está atualizando ao vivo ~10 Hz). Rode ANTES ou DEPOIS
// do iRacing — a layer pega quando existir. Ctrl+C pra parar.

#include <windows.h>
#include "shared_frame.h"

#include <cstdint>
#include <cstdio>

int main() {
    HANDLE h = CreateFileMappingW(
        INVALID_HANDLE_VALUE, nullptr, PAGE_READWRITE,
        static_cast<DWORD>(IRACER_SHM_SIZE >> 32),
        static_cast<DWORD>(IRACER_SHM_SIZE & 0xFFFFFFFF),
        IRACER_SHM_NAME);
    if (!h) {
        std::printf("ERRO: CreateFileMapping falhou (%lu)\n", GetLastError());
        return 1;
    }

    auto* base = static_cast<uint8_t*>(
        MapViewOfFile(h, FILE_MAP_WRITE, 0, 0, static_cast<SIZE_T>(IRACER_SHM_SIZE)));
    if (!base) {
        std::printf("ERRO: MapViewOfFile falhou (%lu)\n", GetLastError());
        return 1;
    }

    auto* hdr = reinterpret_cast<IracerFrameHeader*>(base);
    hdr->magic   = IRACER_SHM_MAGIC;
    hdr->version = IRACER_SHM_VERSION;  // senão a layer rejeita o mapeamento (guarda de layout)
    hdr->width   = IRACER_OVERLAY_W;
    hdr->height  = IRACER_OVERLAY_H;
    hdr->frame   = 0;

    uint8_t* px = base + sizeof(IracerFrameHeader);

    std::printf("Escritor de teste rodando (%ux%u @ ~10 Hz). Ctrl+C pra parar.\n",
                IRACER_OVERLAY_W, IRACER_OVERLAY_H);

    uint32_t t = 0;
    for (;;) {
        for (uint32_t y = 0; y < IRACER_OVERLAY_H; ++y) {
            for (uint32_t x = 0; x < IRACER_OVERLAY_W; ++x) {
                uint8_t* q = px + (static_cast<size_t>(y) * IRACER_OVERLAY_W + x) * 4;
                q[0] = static_cast<uint8_t>((x + t) & 0xFF);          // R
                q[1] = static_cast<uint8_t>((y + t) & 0xFF);          // G
                q[2] = static_cast<uint8_t>((x + y + t * 2) & 0xFF);  // B
                q[3] = 255;                                           // A (opaco)
            }
        }
        hdr->frame = ++t;
        Sleep(100);  // ~10 Hz
    }
}
