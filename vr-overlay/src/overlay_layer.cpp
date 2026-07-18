// ─────────────────────────────────────────────────────────────────────────────
//  iRacer VR Overlay — Spike 1: OpenXR API Layer (prova de conceito)
// ─────────────────────────────────────────────────────────────────────────────
//
//  OBJETIVO DESTE SPIKE
//  --------------------
//  Provar UMA coisa só: "uma OpenXR API layer nossa carrega dentro do iRacing
//  (rodando em OpenXR / VDXR no Pico 4) e consegue desenhar um painel na visão?"
//
//  Por isso aqui NÃO tem torre de tempos, nem textura compartilhada, nem dado do
//  jogo. A layer só injeta um RETÂNGULO SÓLIDO (magenta) grudado na sua cabeça,
//  ~1 m à frente, um pouco à direita e abaixo. Se ele aparecer no óculos, o
//  caminho está aberto e o resto (textura -> torre -> posição/toggle) é
//  engenharia previsível.
//
//  COMO A INJEÇÃO FUNCIONA (sem "hack")
//  ------------------------------------
//  O próprio LOADER do OpenXR carrega API layers registradas no registro do
//  Windows. O iRacing chama o loader; o loader nos carrega ANTES do runtime.
//  Interceptamos a "cadeia" de funções do OpenXR e, no `xrEndFrame` (quando o
//  jogo entrega o frame pronto pro compositor), ACRESCENTAMOS uma camada de
//  composição (um quad) com o nosso painel. Não lemos memória do jogo, não
//  escrevemos nada, não tocamos em input — só desenhamos pixels. É a mesma
//  categoria do RaceLab/OpenKneeboard.
//
//  DESEMPENHO (requisito: 10 Hz + quase sem custo em VR)
//  -----------------------------------------------------
//  • O CONTEÚDO do painel só é redesenhado a no máximo ~10 Hz (gate por tempo).
//  • Todo frame (90/120 Hz) só ANEXA a referência do quad já pronto — 1 quad a
//    mais pro compositor mesclar, custo desprezível.
//  • Fora do tick de 10 Hz, ZERO trabalho de GPU nosso. É isso que mantém o
//    impacto no frame do VR perto de nada.
//
//  Alvo gráfico: Direct3D 11 (o iRacing usa DX11).
// ─────────────────────────────────────────────────────────────────────────────

#define _CRT_SECURE_NO_WARNINGS  // usamos fopen/vfprintf no log; ok pra um sidecar
#define XR_USE_PLATFORM_WIN32
#define XR_USE_GRAPHICS_API_D3D11

#include <windows.h>
#include <d3d11.h>

#include <openxr/openxr.h>
#include <openxr/openxr_platform.h>
#include <openxr/openxr_loader_negotiation.h>

#include "shared_frame.h"

#include <cstring>
#include <cstdio>
#include <cmath>
#include <mutex>
#include <string>
#include <vector>

// ─── Log em arquivo (essencial: não dá pra depurar com breakpoint dentro do VR) ─
// Grava em %TEMP%\iracer_overlay_layer.log. É a nossa janela pro que aconteceu.
static std::mutex g_logMutex;

static void LogLine(const char* fmt, ...) {
    std::lock_guard<std::mutex> lock(g_logMutex);

    char path[MAX_PATH];
    DWORD n = GetEnvironmentVariableA("TEMP", path, MAX_PATH);
    if (n == 0 || n >= MAX_PATH) {
        return;
    }
    std::string file = std::string(path) + "\\iracer_overlay_layer.log";

    FILE* f = std::fopen(file.c_str(), "a");
    if (!f) {
        return;
    }

    SYSTEMTIME st;
    GetLocalTime(&st);
    std::fprintf(f, "[%02d:%02d:%02d.%03d] ", st.wHour, st.wMinute, st.wSecond, st.wMilliseconds);

    va_list args;
    va_start(args, fmt);
    std::vfprintf(f, fmt, args);
    va_end(args);

    std::fprintf(f, "\n");
    std::fclose(f);
}

// ─── Ponteiros da "próxima" camada (o runtime real, abaixo de nós) ────────────
static PFN_xrGetInstanceProcAddr        g_nextGIPA = nullptr;
static PFN_xrCreateSession              g_next_xrCreateSession = nullptr;
static PFN_xrEndFrame                   g_next_xrEndFrame = nullptr;
static PFN_xrDestroySession             g_next_xrDestroySession = nullptr;
static PFN_xrDestroyInstance            g_next_xrDestroyInstance = nullptr;
static PFN_xrCreateReferenceSpace       g_next_xrCreateReferenceSpace = nullptr;
static PFN_xrDestroySpace               g_next_xrDestroySpace = nullptr;
static PFN_xrLocateSpace                g_next_xrLocateSpace = nullptr;
static PFN_xrCreateSwapchain            g_next_xrCreateSwapchain = nullptr;
static PFN_xrDestroySwapchain           g_next_xrDestroySwapchain = nullptr;
static PFN_xrEnumerateSwapchainFormats  g_next_xrEnumerateSwapchainFormats = nullptr;
static PFN_xrEnumerateSwapchainImages   g_next_xrEnumerateSwapchainImages = nullptr;
static PFN_xrAcquireSwapchainImage      g_next_xrAcquireSwapchainImage = nullptr;
static PFN_xrWaitSwapchainImage         g_next_xrWaitSwapchainImage = nullptr;
static PFN_xrReleaseSwapchainImage      g_next_xrReleaseSwapchainImage = nullptr;

// ─── Estado do overlay ────────────────────────────────────────────────────────
static XrInstance   g_instance   = XR_NULL_HANDLE;
static XrSession    g_session    = XR_NULL_HANDLE;
static XrSpace      g_viewSpace   = XR_NULL_HANDLE;  // VIEW  = grudado na cabeça
static XrSpace      g_localSpace  = XR_NULL_HANDLE;  // LOCAL = fixo no cockpit/mundo
static XrSpace      g_anchorSpace = XR_NULL_HANDLE;  // LOCAL reancorado no recentro
static XrSwapchain  g_swapchain   = XR_NULL_HANDLE;

static ID3D11Device*        g_device  = nullptr;     // device do PRÓPRIO iRacing
static ID3D11DeviceContext* g_context = nullptr;
static std::vector<ID3D11RenderTargetView*> g_rtvs;
static std::vector<ID3D11Texture2D*>        g_textures;  // texturas do swapchain (raw, do runtime)

// Ponte com o app: memória compartilhada com o frame RGBA da torre.
static HANDLE g_shm         = nullptr;
static void*  g_shmPtr      = nullptr;
static bool   g_shmLogged   = false;
static bool   g_shmWritable = false;  // conseguimos abrir p/ escrita? (ajuste por teclado)

// Pixels do painel: vêm do contrato compartilhado (retrato, como a torre). Assim
// o swapchain, a validação do frame e o stride de cópia usam UMA fonte só.
static const int32_t kWidth  = static_cast<int32_t>(IRACER_OVERLAY_W);
static const int32_t kHeight = static_cast<int32_t>(IRACER_OVERLAY_H);
static const float   kQuadW  = 0.45f;  // tamanho no mundo, em metros
static const float   kQuadH  = 0.90f;

// Config do painel lida da memória compartilhada (posição/trava/escala). Se o app
// ainda não escreveu, usamos estes padrões — cockpit-locked, à frente/direita.
struct OverlayConfig {
    int32_t lockMode;   // IRACER_LOCK_HEAD | IRACER_LOCK_WORLD
    float   posX, posY, posZ;
    float   yawDeg;
    float   pitchDeg;
    float   scale;
    bool    visible;
};
static const OverlayConfig kDefaultConfig{
    IRACER_LOCK_WORLD, 0.30f, -0.10f, -1.0f, -12.0f, 8.0f, 1.0f, true};

// Gate de 10 Hz: só redesenha o conteúdo a cada 100 ms.
static const XrTime  kRenderPeriodNs = 100'000'000;  // 100 ms em nanossegundos
static bool          g_rendered  = false;
static XrTime        g_lastRender = 0;

static XrPosef IdentityPose() {
    XrPosef p{};
    p.orientation = XrQuaternionf{0.0f, 0.0f, 0.0f, 1.0f};
    p.position    = XrVector3f{0.0f, 0.0f, 0.0f};
    return p;
}

// ─── Carrega os ponteiros da próxima camada que vamos precisar ────────────────
#define LOAD_NEXT(inst, name)                                                        \
    do {                                                                             \
        if (g_nextGIPA((inst), #name,                                                \
                       reinterpret_cast<PFN_xrVoidFunction*>(&g_next_##name))        \
            != XR_SUCCESS) {                                                         \
            LogLine("AVISO: falha ao resolver %s", #name);                           \
        }                                                                            \
    } while (0)

static void LoadNextFunctions(XrInstance instance) {
    LOAD_NEXT(instance, xrCreateSession);
    LOAD_NEXT(instance, xrEndFrame);
    LOAD_NEXT(instance, xrDestroySession);
    LOAD_NEXT(instance, xrDestroyInstance);
    LOAD_NEXT(instance, xrCreateReferenceSpace);
    LOAD_NEXT(instance, xrDestroySpace);
    LOAD_NEXT(instance, xrLocateSpace);
    LOAD_NEXT(instance, xrCreateSwapchain);
    LOAD_NEXT(instance, xrDestroySwapchain);
    LOAD_NEXT(instance, xrEnumerateSwapchainFormats);
    LOAD_NEXT(instance, xrEnumerateSwapchainImages);
    LOAD_NEXT(instance, xrAcquireSwapchainImage);
    LOAD_NEXT(instance, xrWaitSwapchainImage);
    LOAD_NEXT(instance, xrReleaseSwapchainImage);
}

// ─── Escolhe um formato de cor que o runtime suporte ──────────────────────────
static int64_t PickColorFormat(XrSession session) {
    uint32_t count = 0;
    if (g_next_xrEnumerateSwapchainFormats(session, 0, &count, nullptr) != XR_SUCCESS || count == 0) {
        return 0;
    }
    std::vector<int64_t> formats(count);
    if (g_next_xrEnumerateSwapchainFormats(session, count, &count, formats.data()) != XR_SUCCESS) {
        return 0;
    }

    const int64_t preferred[] = {
        DXGI_FORMAT_R8G8B8A8_UNORM_SRGB,
        DXGI_FORMAT_B8G8R8A8_UNORM_SRGB,
        DXGI_FORMAT_R8G8B8A8_UNORM,
        DXGI_FORMAT_B8G8R8A8_UNORM,
    };
    for (int64_t want : preferred) {
        for (int64_t have : formats) {
            if (have == want) {
                return want;
            }
        }
    }
    return formats[0];  // fallback: aceita o primeiro que o runtime oferecer
}

// ─── Monta swapchain + espaço de visão logo após a sessão nascer ──────────────
static void SetupOverlay(XrSession session) {
    g_session = session;

    // Dois espaços de referência:
    //   • VIEW  → o quad segue a cabeça (head-locked).
    //   • LOCAL → o quad fica fixo no assento/cockpit (world-locked). É o padrão;
    //     a origem do LOCAL é onde você estava ao recentrar (long-press no VD),
    //     então "à frente" cai naturalmente dentro do cockpit.
    // Qual dos dois entra na cena é escolhido por frame pela config (lockMode).
    XrReferenceSpaceCreateInfo viewInfo{XR_TYPE_REFERENCE_SPACE_CREATE_INFO};
    viewInfo.referenceSpaceType   = XR_REFERENCE_SPACE_TYPE_VIEW;
    viewInfo.poseInReferenceSpace = IdentityPose();
    if (g_next_xrCreateReferenceSpace(session, &viewInfo, &g_viewSpace) != XR_SUCCESS) {
        LogLine("ERRO: xrCreateReferenceSpace(VIEW) falhou");
        return;
    }

    XrReferenceSpaceCreateInfo localInfo{XR_TYPE_REFERENCE_SPACE_CREATE_INFO};
    localInfo.referenceSpaceType   = XR_REFERENCE_SPACE_TYPE_LOCAL;
    localInfo.poseInReferenceSpace = IdentityPose();
    if (g_next_xrCreateReferenceSpace(session, &localInfo, &g_localSpace) != XR_SUCCESS) {
        // Não é fatal: sem LOCAL, o world-lock cai pro VIEW (ainda funciona).
        LogLine("AVISO: xrCreateReferenceSpace(LOCAL) falhou — world-lock indisponível");
        g_localSpace = XR_NULL_HANDLE;
    }

    int64_t format = PickColorFormat(session);
    if (format == 0) {
        LogLine("ERRO: nenhum formato de swapchain disponível");
        return;
    }
    LogLine("Formato de cor escolhido: %lld", (long long)format);

    XrSwapchainCreateInfo sc{XR_TYPE_SWAPCHAIN_CREATE_INFO};
    sc.usageFlags  = XR_SWAPCHAIN_USAGE_COLOR_ATTACHMENT_BIT | XR_SWAPCHAIN_USAGE_SAMPLED_BIT;
    sc.format      = format;
    sc.sampleCount = 1;
    sc.width       = kWidth;
    sc.height      = kHeight;
    sc.faceCount   = 1;
    sc.arraySize   = 1;
    sc.mipCount    = 1;
    if (g_next_xrCreateSwapchain(session, &sc, &g_swapchain) != XR_SUCCESS) {
        LogLine("ERRO: xrCreateSwapchain falhou");
        return;
    }

    uint32_t imgCount = 0;
    g_next_xrEnumerateSwapchainImages(g_swapchain, 0, &imgCount, nullptr);
    std::vector<XrSwapchainImageD3D11KHR> images(imgCount, {XR_TYPE_SWAPCHAIN_IMAGE_D3D11_KHR});
    if (g_next_xrEnumerateSwapchainImages(
            g_swapchain, imgCount, &imgCount,
            reinterpret_cast<XrSwapchainImageBaseHeader*>(images.data())) != XR_SUCCESS) {
        LogLine("ERRO: xrEnumerateSwapchainImages falhou");
        return;
    }

    // Uma render-target-view por imagem do swapchain (do device do iRacing).
    // IMPORTANTE: muitos runtimes (o VDXR do Virtual Desktop entre eles) criam a
    // textura do swapchain como TYPELESS. Aí CreateRenderTargetView(..., nullptr)
    // falha com E_INVALIDARG (0x80070057) — não dá pra inferir o formato de uma
    // textura typeless. A correção é passar um DESC EXPLÍCITO com o formato
    // concreto que pedimos.
    D3D11_RENDER_TARGET_VIEW_DESC rtvDesc{};
    rtvDesc.Format             = static_cast<DXGI_FORMAT>(format);
    rtvDesc.ViewDimension      = D3D11_RTV_DIMENSION_TEXTURE2D;
    rtvDesc.Texture2D.MipSlice = 0;

    g_rtvs.assign(imgCount, nullptr);
    g_textures.assign(imgCount, nullptr);
    for (uint32_t i = 0; i < imgCount; ++i) {
        g_textures[i] = images[i].texture;  // raw: pertence ao runtime, válido enquanto o swapchain viver
        HRESULT hr = g_device->CreateRenderTargetView(images[i].texture, &rtvDesc, &g_rtvs[i]);
        if (FAILED(hr)) {
            LogLine("ERRO: CreateRenderTargetView[%u] hr=0x%08lX", i, (unsigned long)hr);
        }
    }

    LogLine("Overlay pronto: swapchain %ux%u, %u imagens", kWidth, kHeight, imgCount);
}

// ─── Ponte: abre o mapeamento sob demanda (o app pode subir depois do iRacing) ─
// Tenta abrir p/ ESCRITA (pra o ajuste por teclado escrever a pose de volta). Se
// não der, cai pra somente leitura — o overlay ainda funciona, só sem teclado.
static bool EnsureShmOpen() {
    if (g_shmPtr) {
        return true;
    }
    g_shm = OpenFileMappingW(FILE_MAP_WRITE, FALSE, IRACER_SHM_NAME);
    g_shmWritable = (g_shm != nullptr);
    if (!g_shm) {
        g_shm = OpenFileMappingW(FILE_MAP_READ, FALSE, IRACER_SHM_NAME);
    }
    if (!g_shm) {
        return false;  // ninguém escrevendo ainda
    }
    g_shmPtr = MapViewOfFile(g_shm, g_shmWritable ? FILE_MAP_WRITE : FILE_MAP_READ,
                             0, 0, static_cast<SIZE_T>(IRACER_SHM_SIZE));
    if (!g_shmPtr) {
        CloseHandle(g_shm);
        g_shm = nullptr;
        g_shmWritable = false;
        return false;
    }
    LogLine("Memoria compartilhada aberta (escritor presente, %s).",
            g_shmWritable ? "RW" : "somente leitura");
    return true;
}

// Pega o frame RGBA atual (ou nullptr se não há escritor/cabeçalho inválido).
static const uint8_t* TryGetFramePixels() {
    if (!EnsureShmOpen()) {
        return nullptr;
    }
    const IracerFrameHeader* hdr = static_cast<const IracerFrameHeader*>(g_shmPtr);
    if (hdr->magic != IRACER_SHM_MAGIC ||
        hdr->width != IRACER_OVERLAY_W || hdr->height != IRACER_OVERLAY_H) {
        return nullptr;  // cabeçalho inválido/dimensão diferente: ignora
    }

    if (!g_shmLogged) {
        LogLine("Primeiro frame lido da memoria (frame=%u)", hdr->frame);
        g_shmLogged = true;
    }
    return static_cast<const uint8_t*>(g_shmPtr) + sizeof(IracerFrameHeader);
}

// Lê a config de pose escrita pelo app. Sem escritor/cabeçalho válido → padrões.
static OverlayConfig GetConfig() {
    if (!EnsureShmOpen()) {
        return kDefaultConfig;
    }
    const IracerFrameHeader* hdr = static_cast<const IracerFrameHeader*>(g_shmPtr);
    if (hdr->magic != IRACER_SHM_MAGIC) {
        return kDefaultConfig;
    }
    OverlayConfig c;
    c.lockMode = hdr->lockMode;
    c.posX     = hdr->posX;
    c.posY     = hdr->posY;
    c.posZ     = hdr->posZ;
    c.yawDeg   = hdr->yawDeg;
    c.pitchDeg = hdr->pitchDeg;
    c.scale    = (hdr->scale > 0.01f) ? hdr->scale : 1.0f;
    c.visible  = hdr->visible != 0;
    return c;
}

// ─── Desenha o conteúdo do painel ─────────────────────────────────────────────
// Com escritor: sobe os pixels RGBA direto na textura (UpdateSubresource —
// funciona em textura DEFAULT, sem amarrar estado do contexto do iRacing).
// Sem escritor: pinta um cinza "aguardando" pra dar sinal de vida.
static void RenderPanel(uint32_t imageIndex) {
    const uint8_t* pixels = TryGetFramePixels();

    if (pixels && imageIndex < g_textures.size() && g_textures[imageIndex]) {
        g_context->UpdateSubresource(g_textures[imageIndex], 0, nullptr,
                                     pixels, IRACER_OVERLAY_W * 4, 0);
        return;
    }

    if (imageIndex < g_rtvs.size() && g_rtvs[imageIndex]) {
        const float waiting[4] = {0.10f, 0.10f, 0.12f, 1.0f};  // cinza escuro
        g_context->ClearRenderTargetView(g_rtvs[imageIndex], waiting);
    }
}

// ─── Recentro (reancora o world-lock na cabeça atual) ─────────────────────────
// Captura a pose da cabeça AGORA (posição + direção horizontal), "achata" a
// orientação pra só-yaw (sem pitch/roll, senão o painel tombaria com o olhar) e
// cria um LOCAL deslocado pra lá. Com isso o overlay reaparece SEMPRE no mesmo
// ponto relativo ao corpo — o "mesma posição pra todos" que o iRacing não dá.
static void DoRecenter(XrTime now) {
    if (g_session == XR_NULL_HANDLE || g_localSpace == XR_NULL_HANDLE ||
        g_viewSpace == XR_NULL_HANDLE || g_next_xrLocateSpace == nullptr ||
        g_next_xrCreateReferenceSpace == nullptr) {
        return;
    }
    XrSpaceLocation loc{XR_TYPE_SPACE_LOCATION};
    if (g_next_xrLocateSpace(g_viewSpace, g_localSpace, now, &loc) != XR_SUCCESS) {
        return;
    }
    const XrSpaceLocationFlags need =
        XR_SPACE_LOCATION_ORIENTATION_VALID_BIT | XR_SPACE_LOCATION_POSITION_VALID_BIT;
    if ((loc.locationFlags & need) != need) {
        return;  // pose ainda não confiável (tracking perdido) — ignora o pedido
    }

    // Yaw da cabeça a partir do quaternião (fórmula padrão de heading):
    //   yaw = atan2( 2(xz + wy),  1 - 2(x² + y²) )
    const XrQuaternionf q = loc.pose.orientation;
    const float yaw = std::atan2(2.0f * (q.x * q.z + q.w * q.y),
                                 1.0f - 2.0f * (q.x * q.x + q.y * q.y));
    const float hy = yaw * 0.5f;

    XrReferenceSpaceCreateInfo info{XR_TYPE_REFERENCE_SPACE_CREATE_INFO};
    info.referenceSpaceType = XR_REFERENCE_SPACE_TYPE_LOCAL;
    info.poseInReferenceSpace.position    = loc.pose.position;  // mantém x/y/z da cabeça
    info.poseInReferenceSpace.orientation = XrQuaternionf{0.0f, std::sin(hy), 0.0f, std::cos(hy)};

    XrSpace newAnchor = XR_NULL_HANDLE;
    if (g_next_xrCreateReferenceSpace(g_session, &info, &newAnchor) != XR_SUCCESS) {
        LogLine("Recenter: xrCreateReferenceSpace falhou");
        return;
    }
    if (g_anchorSpace != XR_NULL_HANDLE && g_next_xrDestroySpace) {
        g_next_xrDestroySpace(g_anchorSpace);
    }
    g_anchorSpace = newAnchor;
    LogLine("Recenter: overlay reancorado (yaw=%.1f graus)", yaw * (180.0f / 3.14159265f));
}

// Detecta pedidos de recentro: (a) o app bump o `recenterSeq` (botão do painel) e
// (b) a tecla configurável `recenterKey` (borda de subida). Roda em leitura, então
// funciona mesmo com a SHM só-leitura. Chamado todo frame.
static uint32_t g_lastRecenterSeq  = 0;
static bool     g_recenterSeqInit  = false;
static bool     g_prevRecenterKey  = false;

static void CheckRecenter(XrTime now) {
    if (!EnsureShmOpen()) {
        return;
    }
    const IracerFrameHeader* hdr = static_cast<const IracerFrameHeader*>(g_shmPtr);
    if (hdr->magic != IRACER_SHM_MAGIC) {
        return;
    }
    bool trigger = false;

    // (a) Botão do app: seq mudou desde a última vez.
    const uint32_t seq = hdr->recenterSeq;
    if (!g_recenterSeqInit) {
        g_lastRecenterSeq = seq;
        g_recenterSeqInit = true;
    } else if (seq != g_lastRecenterSeq) {
        g_lastRecenterSeq = seq;
        trigger = true;
    }

    // (b) Tecla configurável (VK). 0 = desligada. Dispara na borda de subida.
    const uint32_t vk = hdr->recenterKey;
    if (vk != 0 && vk < 256) {
        const bool downNow = (GetAsyncKeyState(static_cast<int>(vk)) & 0x8000) != 0;
        if (downNow && !g_prevRecenterKey) {
            trigger = true;
        }
        g_prevRecenterKey = downNow;
    } else {
        g_prevRecenterKey = false;
    }

    if (trigger) {
        DoRecenter(now);
    }
}

// ─── Ajuste por TECLADO (modo: segure Ctrl direito) ───────────────────────────
// Escreve a pose de volta na memória compartilhada; o app lê e persiste. Só roda
// se conseguimos abrir a SHM p/ escrita. Gate por tempo (~50 Hz) pra o "segurar"
// mover suave, e não a 90+ fps.
static XrTime g_lastKbd = 0;
static bool   g_prevL   = false;  // borda de subida do toggle de trava
static bool   g_prevH   = false;  // borda de subida do toggle de visível
static bool   g_prevC   = false;  // borda de subida do recentro (Ctrl dir + C)

static float ClampF(float v, float lo, float hi) {
    return v < lo ? lo : (v > hi ? hi : v);
}

static void ProcessKeyboard(XrTime now) {
    if (!g_shmWritable) {
        return;
    }
    if (g_lastKbd != 0 && (now - g_lastKbd) < 20'000'000) {  // 20 ms = ~50 Hz
        return;
    }
    g_lastKbd = now;

    if (!EnsureShmOpen()) {
        return;
    }
    IracerFrameHeader* hdr = static_cast<IracerFrameHeader*>(g_shmPtr);
    if (hdr->magic != IRACER_SHM_MAGIC) {
        return;
    }

    auto down = [](int vk) { return (GetAsyncKeyState(vk) & 0x8000) != 0; };

    // Fora do modo de ajuste (sem Ctrl direito): não faz nada, só zera as bordas.
    if (!down(VK_RCONTROL)) {
        g_prevL = false;
        g_prevH = false;
        g_prevC = false;
        return;
    }

    // Recentro por teclado (Ctrl dir + C): reancora agora. Sempre disponível, mesmo
    // sem tecla configurada. Não mexe na pose — só recria o espaço-âncora.
    const bool cKey = down('C');
    if (cKey && !g_prevC) {
        DoRecenter(now);
    }
    g_prevC = cKey;

    const float posStep   = 0.005f;  // ~0,25 m/s segurando
    const float yawStep   = 0.5f;    // graus/tick
    const float pitchStep = 0.5f;    // graus/tick
    const float scaleStep = 0.01f;
    bool changed = false;

    if (down(VK_LEFT))  { hdr->posX -= posStep; changed = true; }
    if (down(VK_RIGHT)) { hdr->posX += posStep; changed = true; }
    if (down(VK_UP))    { hdr->posY += posStep; changed = true; }
    if (down(VK_DOWN))  { hdr->posY -= posStep; changed = true; }
    if (down(VK_PRIOR)) { hdr->posZ += posStep; changed = true; }  // PageUp   = mais perto
    if (down(VK_NEXT))  { hdr->posZ -= posStep; changed = true; }  // PageDown = mais longe
    if (down(VK_ADD)    || down(VK_OEM_PLUS))  { hdr->scale  += scaleStep; changed = true; }
    if (down(VK_SUBTRACT) || down(VK_OEM_MINUS)) { hdr->scale  -= scaleStep; changed = true; }
    if (down(VK_OEM_COMMA))  { hdr->yawDeg -= yawStep; changed = true; }  // ','
    if (down(VK_OEM_PERIOD)) { hdr->yawDeg += yawStep; changed = true; }  // '.'
    if (down(VK_HOME)) { hdr->pitchDeg += pitchStep; changed = true; }  // topo tomba na sua direção
    if (down(VK_END))  { hdr->pitchDeg -= pitchStep; changed = true; }  // topo recosta

    // Toggles por borda de subida (não repetem enquanto segura).
    const bool l = down('L');
    if (l && !g_prevL) {
        hdr->lockMode = (hdr->lockMode == IRACER_LOCK_HEAD) ? IRACER_LOCK_WORLD : IRACER_LOCK_HEAD;
        changed = true;
    }
    g_prevL = l;
    const bool h = down('H');
    if (h && !g_prevH) {
        hdr->visible = hdr->visible ? 0u : 1u;
        changed = true;
    }
    g_prevH = h;

    if (!changed) {
        return;
    }

    // Clamps iguais aos ranges do painel do app (mantém os dois lados coerentes).
    hdr->posX   = ClampF(hdr->posX, -1.5f, 1.5f);
    hdr->posY   = ClampF(hdr->posY, -1.2f, 1.2f);
    hdr->posZ     = ClampF(hdr->posZ, -2.5f, -0.3f);
    hdr->yawDeg   = ClampF(hdr->yawDeg, -45.0f, 45.0f);
    hdr->pitchDeg = ClampF(hdr->pitchDeg, -45.0f, 45.0f);
    hdr->scale    = ClampF(hdr->scale, 0.5f, 2.0f);
    hdr->configEpoch++;
}

// ─── Interceptação: xrEndFrame (onde anexamos o quad) ─────────────────────────
static XrResult XRAPI_CALL Layer_xrEndFrame(XrSession session, const XrFrameEndInfo* frameEndInfo) {
    // Sem overlay montado? Repassa o frame intocado.
    if (g_swapchain == XR_NULL_HANDLE || g_viewSpace == XR_NULL_HANDLE ||
        frameEndInfo == nullptr || g_next_xrEndFrame == nullptr) {
        return g_next_xrEndFrame ? g_next_xrEndFrame(session, frameEndInfo)
                                 : XR_ERROR_FUNCTION_UNSUPPORTED;
    }

    // Ajuste por teclado (modo Ctrl direito): escreve a pose de volta na SHM.
    ProcessKeyboard(frameEndInfo->displayTime);

    // Pedidos de recentro (botão do app OU tecla configurável).
    CheckRecenter(frameEndInfo->displayTime);

    // Config de pose (trava/posição/escala/visível) — o app escreve, lemos aqui.
    const OverlayConfig cfg = GetConfig();
    if (!cfg.visible) {
        return g_next_xrEndFrame(session, frameEndInfo);  // painel oculto
    }

    // GATE DE 10 Hz: só redesenha o conteúdo se passou o período. Nos ~9 frames
    // seguintes, nada de GPU nosso — só reaproveitamos a última imagem.
    const XrTime now = frameEndInfo->displayTime;
    const bool timeToRender = !g_rendered || (now - g_lastRender) >= kRenderPeriodNs;

    if (timeToRender) {
        uint32_t index = 0;
        XrSwapchainImageAcquireInfo acq{XR_TYPE_SWAPCHAIN_IMAGE_ACQUIRE_INFO};
        if (g_next_xrAcquireSwapchainImage(g_swapchain, &acq, &index) == XR_SUCCESS) {
            XrSwapchainImageWaitInfo wait{XR_TYPE_SWAPCHAIN_IMAGE_WAIT_INFO};
            wait.timeout = XR_INFINITE_DURATION;
            if (g_next_xrWaitSwapchainImage(g_swapchain, &wait) == XR_SUCCESS) {
                RenderPanel(index);
                XrSwapchainImageReleaseInfo rel{XR_TYPE_SWAPCHAIN_IMAGE_RELEASE_INFO};
                g_next_xrReleaseSwapchainImage(g_swapchain, &rel);
                if (!g_rendered) {
                    LogLine("Primeiro render do painel enviado (displayTime=%lld)", (long long)now);
                }
                g_rendered   = true;
                g_lastRender = now;
            }
        }
    }

    // Só entra na cena depois que existe ao menos uma imagem já liberada.
    if (!g_rendered) {
        return g_next_xrEndFrame(session, frameEndInfo);
    }

    // Espaço: world-lock usa o ÂNCORA do recentro se já houve um; senão o LOCAL
    // cru; head-lock usa VIEW. Sem LOCAL (runtime não deu) o world-lock cai pro VIEW.
    XrSpace space = g_viewSpace;
    if (cfg.lockMode == IRACER_LOCK_WORLD) {
        if (g_anchorSpace != XR_NULL_HANDLE) {
            space = g_anchorSpace;
        } else if (g_localSpace != XR_NULL_HANDLE) {
            space = g_localSpace;
        }
    }

    // Orientação = yaw (em torno de Y, aponta o painel na sua direção) COMPOSTO
    // com pitch (em torno de X, inclina o topo pra frente/pra trás). Produto de
    // Hamilton qYaw ⊗ qPitch, forma fechada:
    //   qYaw   = (0, sy, 0, cy)   qPitch = (sp, 0, 0, cp)
    //   q = ( cy*sp,  sy*cp,  -sy*sp,  cy*cp )
    const float halfYaw   = cfg.yawDeg   * (3.14159265f / 180.0f) * 0.5f;
    const float halfPitch = cfg.pitchDeg * (3.14159265f / 180.0f) * 0.5f;
    const float cy = std::cos(halfYaw),   sy = std::sin(halfYaw);
    const float cp = std::cos(halfPitch), sp = std::sin(halfPitch);
    const XrQuaternionf orient{cy * sp, sy * cp, -sy * sp, cy * cp};

    // Monta o quad a partir da config (posição/giro/inclinação/escala).
    XrCompositionLayerQuad quad{XR_TYPE_COMPOSITION_LAYER_QUAD};
    quad.layerFlags               = XR_COMPOSITION_LAYER_BLEND_TEXTURE_SOURCE_ALPHA_BIT;
    quad.space                    = space;
    quad.eyeVisibility            = XR_EYE_VISIBILITY_BOTH;
    quad.subImage.swapchain       = g_swapchain;
    quad.subImage.imageRect       = XrRect2Di{{0, 0}, {kWidth, kHeight}};
    quad.subImage.imageArrayIndex = 0;
    quad.pose.orientation         = orient;
    quad.pose.position            = XrVector3f{cfg.posX, cfg.posY, cfg.posZ};
    quad.size                     = XrExtent2Df{kQuadW * cfg.scale, kQuadH * cfg.scale};

    // Recopia a lista de camadas do jogo e ACRESCENTA a nossa no fim.
    std::vector<const XrCompositionLayerBaseHeader*> layers(
        frameEndInfo->layers, frameEndInfo->layers + frameEndInfo->layerCount);
    layers.push_back(reinterpret_cast<const XrCompositionLayerBaseHeader*>(&quad));

    XrFrameEndInfo mutated = *frameEndInfo;
    mutated.layers     = layers.data();
    mutated.layerCount = static_cast<uint32_t>(layers.size());

    return g_next_xrEndFrame(session, &mutated);
}

// ─── Interceptação: xrCreateSession (pegamos o device D3D11 aqui) ─────────────
static XrResult XRAPI_CALL Layer_xrCreateSession(XrInstance instance,
                                                 const XrSessionCreateInfo* createInfo,
                                                 XrSession* session) {
    // Procura o binding gráfico D3D11 na cadeia `next` do createInfo.
    const XrGraphicsBindingD3D11KHR* d3d11 = nullptr;
    for (const XrBaseInStructure* p = reinterpret_cast<const XrBaseInStructure*>(createInfo->next);
         p != nullptr; p = p->next) {
        if (p->type == XR_TYPE_GRAPHICS_BINDING_D3D11_KHR) {
            d3d11 = reinterpret_cast<const XrGraphicsBindingD3D11KHR*>(p);
            break;
        }
    }

    XrResult res = g_next_xrCreateSession(instance, createInfo, session);
    if (res != XR_SUCCESS) {
        return res;
    }

    if (d3d11 && d3d11->device) {
        g_device = d3d11->device;
        g_device->GetImmediateContext(&g_context);
        LogLine("Sessão criada; binding D3D11 encontrado. Montando overlay...");
        SetupOverlay(*session);
    } else {
        LogLine("Sessão criada, mas SEM binding D3D11 (jogo em D3D12/Vulkan?). "
                "Overlay desativado nesta sessão.");
    }
    return res;
}

// ─── Interceptação: limpeza ───────────────────────────────────────────────────
static void TeardownOverlay() {
    if (g_shmPtr) {
        UnmapViewOfFile(g_shmPtr);
        g_shmPtr = nullptr;
    }
    if (g_shm) {
        CloseHandle(g_shm);
        g_shm = nullptr;
    }
    g_shmLogged = false;

    for (auto* rtv : g_rtvs) {
        if (rtv) rtv->Release();
    }
    g_rtvs.clear();
    g_textures.clear();
    if (g_swapchain != XR_NULL_HANDLE && g_next_xrDestroySwapchain) {
        g_next_xrDestroySwapchain(g_swapchain);
    }
    if (g_viewSpace != XR_NULL_HANDLE && g_next_xrDestroySpace) {
        g_next_xrDestroySpace(g_viewSpace);
    }
    if (g_localSpace != XR_NULL_HANDLE && g_next_xrDestroySpace) {
        g_next_xrDestroySpace(g_localSpace);
    }
    if (g_anchorSpace != XR_NULL_HANDLE && g_next_xrDestroySpace) {
        g_next_xrDestroySpace(g_anchorSpace);
    }
    if (g_context) {
        g_context->Release();
        g_context = nullptr;
    }
    g_swapchain   = XR_NULL_HANDLE;
    g_viewSpace   = XR_NULL_HANDLE;
    g_localSpace  = XR_NULL_HANDLE;
    g_anchorSpace = XR_NULL_HANDLE;
    g_session     = XR_NULL_HANDLE;
    g_device      = nullptr;
    g_rendered    = false;
    g_lastRender  = 0;
    g_recenterSeqInit = false;
    g_prevRecenterKey = false;
}

static XrResult XRAPI_CALL Layer_xrDestroySession(XrSession session) {
    LogLine("xrDestroySession: limpando overlay");
    TeardownOverlay();
    return g_next_xrDestroySession(session);
}

static XrResult XRAPI_CALL Layer_xrDestroyInstance(XrInstance instance) {
    LogLine("xrDestroyInstance");
    XrResult res = g_next_xrDestroyInstance(instance);
    g_instance = XR_NULL_HANDLE;
    return res;
}

// ─── Nosso xrGetInstanceProcAddr: devolve os hooks, delega o resto ────────────
static XrResult XRAPI_CALL Layer_xrGetInstanceProcAddr(XrInstance instance, const char* name,
                                                       PFN_xrVoidFunction* function) {
    if (std::strcmp(name, "xrEndFrame") == 0) {
        *function = reinterpret_cast<PFN_xrVoidFunction>(Layer_xrEndFrame);
        return XR_SUCCESS;
    }
    if (std::strcmp(name, "xrCreateSession") == 0) {
        *function = reinterpret_cast<PFN_xrVoidFunction>(Layer_xrCreateSession);
        return XR_SUCCESS;
    }
    if (std::strcmp(name, "xrDestroySession") == 0) {
        *function = reinterpret_cast<PFN_xrVoidFunction>(Layer_xrDestroySession);
        return XR_SUCCESS;
    }
    if (std::strcmp(name, "xrDestroyInstance") == 0) {
        *function = reinterpret_cast<PFN_xrVoidFunction>(Layer_xrDestroyInstance);
        return XR_SUCCESS;
    }
    return g_nextGIPA(instance, name, function);
}

// ─── Criação da instância: capturamos a "próxima" camada e avançamos a cadeia ─
static XrResult XRAPI_CALL Layer_xrCreateApiLayerInstance(const XrInstanceCreateInfo* info,
                                                          const XrApiLayerCreateInfo* apiLayerInfo,
                                                          XrInstance* instance) {
    if (!apiLayerInfo || !apiLayerInfo->nextInfo) {
        return XR_ERROR_INITIALIZATION_FAILED;
    }

    g_nextGIPA = apiLayerInfo->nextInfo->nextGetInstanceProcAddr;
    PFN_xrCreateApiLayerInstance nextCreate = apiLayerInfo->nextInfo->nextCreateApiLayerInstance;

    // Avança a cadeia: a PRÓXIMA camada recebe o `nextInfo` dela, não o nosso.
    XrApiLayerCreateInfo forwarded = *apiLayerInfo;
    forwarded.nextInfo = apiLayerInfo->nextInfo->next;

    XrResult res = nextCreate(info, &forwarded, instance);
    if (res == XR_SUCCESS) {
        g_instance = *instance;
        LoadNextFunctions(*instance);
        LogLine("Instância OpenXR criada; layer iRacer conectada à cadeia.");
    } else {
        LogLine("ERRO: nextCreateApiLayerInstance falhou (%d)", (int)res);
    }
    return res;
}

// ─── Ponto de entrada que o LOADER do OpenXR chama ao descobrir a layer ───────
extern "C" __declspec(dllexport) XrResult XRAPI_CALL
xrNegotiateLoaderApiLayerInterface(const XrNegotiateLoaderInfo* loaderInfo,
                                   const char* apiLayerName,
                                   XrNegotiateApiLayerRequest* apiLayerRequest) {
    (void)apiLayerName;

    if (!loaderInfo || !apiLayerRequest ||
        loaderInfo->structType != XR_LOADER_INTERFACE_STRUCT_LOADER_INFO ||
        apiLayerRequest->structType != XR_LOADER_INTERFACE_STRUCT_API_LAYER_REQUEST) {
        return XR_ERROR_INITIALIZATION_FAILED;
    }

    apiLayerRequest->layerInterfaceVersion = XR_CURRENT_LOADER_API_LAYER_VERSION;
    apiLayerRequest->layerApiVersion       = XR_CURRENT_API_VERSION;
    apiLayerRequest->getInstanceProcAddr   = Layer_xrGetInstanceProcAddr;
    apiLayerRequest->createApiLayerInstance = Layer_xrCreateApiLayerInstance;

    LogLine("xrNegotiateLoaderApiLayerInterface: layer iRacer carregada pelo loader.");
    return XR_SUCCESS;
}
