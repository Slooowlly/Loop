// ─────────────────────────────────────────────────────────────────────────────
//  iRacer VR Overlay — OpenXR API Layer (DOIS painéis independentes)
// ─────────────────────────────────────────────────────────────────────────────
//
//  A layer carrega dentro do iRacing (OpenXR/VDXR) e, no `xrEndFrame`, ANEXA quads
//  de composição com os nossos painéis — sem ler memória do jogo, sem tocar input:
//  só desenha pixels (mesma categoria de RaceLab/OpenKneeboard).
//
//  Agora são DOIS painéis, cada um 100% independente (swapchain + config de pose +
//  recentro próprios), lidos de mapeamentos de memória compartilhada separados:
//    • TORRE  → Local\iRacerOverlayFrame  (retrato, cockpit-locked por padrão)
//    • RÁDIO  → Local\iRacerEngineerFrame (banner, cockpit-locked por padrão)
//
//  Cada painel:
//    • redesenha o CONTEÚDO a no máx ~10 Hz (gate por tempo); todo frame só reanexa
//      o quad pronto (custo desprezível);
//    • tem sua config (trava cockpit/cabeça, X/Y/Z, giro, inclinação, escala,
//      visível, recentro) escrita pelo app na SHM e lida aqui todo frame;
//    • pode ser ajustado por teclado dentro do VR (segure Ctrl direito). Ctrl+T
//      alterna QUAL painel o teclado controla (torre ↔ rádio).
//
//  Alvo gráfico: Direct3D 11 (o iRacing usa DX11).
// ─────────────────────────────────────────────────────────────────────────────

#define _CRT_SECURE_NO_WARNINGS
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
#include <cwchar>  // wcsrchr — nome do .exe hospedeiro
#include <cwctype> // towlower — comparação sem caixa
#include <mutex>
#include <string>
#include <vector>

// ─── Log em arquivo (%TEMP%\iracer_overlay_layer.log) ─────────────────────────
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

// ─── Sinal de "o iRacing está em VR" (lido pelo app) ─────────────────────────
//
// O app não tem como saber, pela telemetria, se o sim foi aberto em VR ou em monitor —
// o irsdk não publica nada de HMD. Mas ESTA layer é a resposta: uma API layer do OpenXR
// só é carregada quando a aplicação cria uma instância XR. Se este código está rodando,
// o processo hospedeiro está em VR.
//
// Como avisar: um EVENTO NOMEADO. Enquanto a instância existe a gente mantém um handle
// aberto em `Local\iRacerVrActive`; o app faz `OpenEventW` e a mera existência do objeto
// é a resposta. Preferido a uma palavra de heartbeat na memória compartilhada porque o
// tempo de vida é do SO: se o iRacing fechar ou travar, o último handle morre com o
// processo e o sinal apaga sozinho — sem estado velho grudado, sem timestamp pra
// comparar, sem bump da versão do cabeçalho da SHM.
//
// FILTRO DE PROCESSO, e não é detalhe: layers implícitas carregam em QUALQUER app
// OpenXR. Sem checar quem nos hospeda, abrir um menu de VR qualquer acenderia o sinal e
// o app acharia que a corrida começou. Só acendemos dentro do executável do iRacing.
static const wchar_t* const IRACER_VR_EVENT = L"Local\\iRacerVrActive";
static HANDLE g_vrActiveEvent = nullptr;

// O processo hospedeiro é o iRacing? (compara o nome do .exe, sem caminho e sem caixa —
// o nome declarado pelo app no XrInstanceCreateInfo é escolha dele, o executável não.)
static bool HostIsIracing() {
    wchar_t path[MAX_PATH];
    DWORD n = GetModuleFileNameW(nullptr, path, MAX_PATH);
    if (n == 0 || n >= MAX_PATH) {
        return false;
    }
    const wchar_t* exe = std::wcsrchr(path, L'\\');
    exe = exe ? exe + 1 : path;

    std::wstring lower(exe);
    for (wchar_t& c : lower) {
        c = towlower(c);
    }
    // Cobre iRacingSim64DX11.exe e qualquer variante futura do mesmo prefixo.
    return lower.rfind(L"iracingsim", 0) == 0;
}

static void SignalVrActive() {
    if (g_vrActiveEvent) {
        return; // já aceso
    }
    if (!HostIsIracing()) {
        LogLine("Sinal de VR NAO aceso: processo hospedeiro nao e o iRacing.");
        return;
    }
    // Manual-reset, não sinalizado: ninguém espera nele. O que importa é o objeto
    // EXISTIR — o estado sinalizado/não é irrelevante pro nosso uso.
    g_vrActiveEvent = CreateEventW(nullptr, TRUE, FALSE, IRACER_VR_EVENT);
    if (g_vrActiveEvent) {
        LogLine("Sinal de VR aceso (%ls) — o app pode detectar o modo VR.", IRACER_VR_EVENT);
    } else {
        LogLine("AVISO: CreateEventW falhou (%lu) — o app nao vera o modo VR.", GetLastError());
    }
}

static void ClearVrActive() {
    if (!g_vrActiveEvent) {
        return;
    }
    CloseHandle(g_vrActiveEvent);
    g_vrActiveEvent = nullptr;
    LogLine("Sinal de VR apagado.");
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

// ─── Estado GLOBAL da sessão (compartilhado pelos dois painéis) ───────────────
static XrInstance   g_instance   = XR_NULL_HANDLE;
static XrSession    g_session    = XR_NULL_HANDLE;
static XrSpace      g_viewSpace  = XR_NULL_HANDLE;  // VIEW  = grudado na cabeça
static XrSpace      g_localSpace = XR_NULL_HANDLE;  // LOCAL = fixo no cockpit/mundo

static ID3D11Device*        g_device  = nullptr;    // device do PRÓPRIO iRacing
static ID3D11DeviceContext* g_context = nullptr;

static const float kPi = 3.14159265f;
// Gate de 10 Hz: só redesenha o conteúdo a cada 100 ms.
static const XrTime kRenderPeriodNs = 100'000'000;
// O escritor manda ~10 Hz. Se o contador `frame` não andar por 1 s, o app parou (fechou,
// travou, ou o gate de VR desligou): paramos de anexar o quad em vez de deixar a última
// imagem congelada dentro do headset.
static const XrTime kFrameStaleNs = 1'000'000'000;
// Mapeamento inexistente é o caso NORMAL enquanto o app não subiu. Sem freio, cada frame
// (90 Hz × 2 painéis × 4 consultas) viraria um par de OpenFileMappingW — syscall à toa no
// caminho mais quente que existe. Uma tentativa por segundo por painel basta.
static const XrTime kShmRetryNs = 1'000'000'000;

// ─── Formatos de cor que sabemos escrever ─────────────────────────────────────
//
// Os pixels da SHM são RGBA (R no primeiro byte — é o que o `getImageData` do canvas
// entrega). Runtimes que só oferecem B8G8R8A8 esperam os canais na outra ordem, então
// nesse caso a gente troca R↔B antes de subir. Fora destes quatro não há o que adivinhar:
// aceitar um `formats[0]` qualquer (que pode ser 64-bit float, 10-bit ou típeless) faria o
// `UpdateSubresource` interpretar o buffer com o stride errado e pintar lixo no headset.
struct ColorFormat {
    int64_t dxgi;
    bool    bgra;  // canais em B,G,R,A → precisa da troca R↔B
};
static const ColorFormat kKnownFormats[] = {
    {DXGI_FORMAT_R8G8B8A8_UNORM_SRGB, false},
    {DXGI_FORMAT_B8G8R8A8_UNORM_SRGB, true},
    {DXGI_FORMAT_R8G8B8A8_UNORM,      false},
    {DXGI_FORMAT_B8G8R8A8_UNORM,      true},
};
// Preenchido por PickColorFormat: o runtime escolhido quer B,G,R,A?
static bool g_formatBgra = false;

// ─── Desistência da composição (o único fallback que a spec permite) ──────────
//
// A spec do OpenXR trata o xrEndFrame como a submissão ÚNICA daquele frame: xrBeginFrame e
// xrEndFrame se alternam, um par por frame, e chamar xrEndFrame duas vezes sem um
// xrBeginFrame no meio é XR_ERROR_CALL_ORDER_INVALID — que está entre os `errorcodes`
// declarados para xrEndFrame no registro do próprio SDK que a gente pina
// (specification/registry/xr.xml). Ou seja: "tentou com os nossos quads, falhou, tenta de
// novo sem eles" NÃO existe. Repetir a chamada trocaria um frame perdido por um erro de
// protocolo, e num runtime menos tolerante isso derruba a sessão do jogador.
//
// O que dá pra fazer é desistir a partir do PRÓXIMO frame. Se a submissão com os nossos
// quads falhar com um erro de CAMADA algumas vezes seguidas, a layer se desliga e volta a
// repassar o frameEndInfo original intacto pelo resto da sessão. O jogador perde o overlay
// e mantém o jogo, que é a troca certa.
static bool           g_compositionOff = false;
static uint32_t       g_layerErrStreak = 0;
static const uint32_t kLayerErrLimit   = 3;

// Erros que apontam pras CAMADAS submetidas — os únicos que os nossos quads podem causar.
// A lista é o recorte dos `errorcodes` de xrEndFrame no xr.xml que dependem do conteúdo de
// `layers`. Ficam de fora, de propósito: SESSION_NOT_RUNNING, CALL_ORDER_INVALID,
// TIME_INVALID e INSTANCE/SESSION_LOST (o jogo teria do mesmo jeito sem nós) e
// ENVIRONMENT_BLEND_MODE_UNSUPPORTED (campo do frameEndInfo que a gente nunca toca).
static bool ErroDeCamada(XrResult r) {
    switch (r) {
        case XR_ERROR_LAYER_INVALID:
        case XR_ERROR_LAYER_LIMIT_EXCEEDED:
        case XR_ERROR_SWAPCHAIN_RECT_INVALID:
        case XR_ERROR_POSE_INVALID:
            return true;
        default:
            return false;
    }
}

// Config de pose lida da SHM (posição/trava/escala). Se o app ainda não escreveu,
// usamos o padrão de cada painel.
struct OverlayConfig {
    int32_t lockMode;   // IRACER_LOCK_HEAD | IRACER_LOCK_WORLD
    float   posX, posY, posZ;
    float   yawDeg;
    float   pitchDeg;
    float   scale;
    bool    visible;
};

static XrPosef IdentityPose() {
    XrPosef p{};
    p.orientation = XrQuaternionf{0.0f, 0.0f, 0.0f, 1.0f};
    p.position    = XrVector3f{0.0f, 0.0f, 0.0f};
    return p;
}

// ─── Um PAINEL: tudo que é próprio de cada quad (torre OU rádio) ──────────────
struct Panel {
    // Fixos (definidos na criação da instância):
    const wchar_t* shmName;
    uint32_t       width, height;   // pixels — devem casar com o escritor
    uint64_t       shmSize;
    float          quadW, quadH;    // tamanho base no mundo, em metros
    OverlayConfig  defaultCfg;

    // Runtime (default-inicializados):
    XrSwapchain swapchain   = XR_NULL_HANDLE;
    XrSpace     anchorSpace = XR_NULL_HANDLE;  // LOCAL reancorado no recentro (próprio)
    std::vector<ID3D11RenderTargetView*> rtvs;
    std::vector<ID3D11Texture2D*>        textures;
    HANDLE shm         = nullptr;
    void*  shmPtr      = nullptr;
    bool   shmWritable = false;
    bool   shmLogged   = false;
    bool   shmVersionLogged = false;  // já logou um mismatch de versão? (evita spam a ~10 Hz)
    XrTime lastNullLog = 0;           // quando logamos o último "sem frame" (garganta de 10 s)
    bool   rendered    = false;
    XrTime lastRender  = 0;
    uint32_t lastRecenterSeq = 0;
    bool     recenterSeqInit = false;
    bool     prevRecenterKey = false;
    // Vigia do contador `frame` do escritor (detecção de app parado).
    uint32_t lastFrameCounter = 0;
    bool     frameCounterInit = false;
    XrTime   lastFrameAdvance = 0;
    // Buffer da troca R↔B, só alocado em runtime BGRA.
    std::vector<uint8_t> swizzled;
    // Freio da abertura da SHM: relógio do frame corrente + memo da tentativa.
    XrTime frameTime          = 0;
    bool   openTriedThisFrame = false;
    XrTime lastOpenTry        = 0;

    // Marca o começo do frame: um único relógio para todas as consultas e a zerada do memo
    // de abertura. Chamado uma vez por painel no topo do xrEndFrame.
    void BeginFrame(XrTime now) {
        frameTime          = now;
        openTriedThisFrame = false;
    }

    // Abre o mapeamento sob demanda (o app pode subir depois do iRacing). Tenta RW
    // (p/ o teclado escrever a pose); se não der, cai pra RO (overlay ainda desenha).
    //
    // Quatro chamadores diferentes perguntam por frame (pixels, config, recentro, teclado).
    // Enquanto o mapeamento NÃO existe, a primeira tentativa do frame decide por todos
    // (`openTriedThisFrame`) e só volta a tentar depois de `kShmRetryNs`.
    bool EnsureShmOpen() {
        if (shmPtr) {
            return true;
        }
        if (openTriedThisFrame) {
            return false;  // já respondida neste frame
        }
        openTriedThisFrame = true;
        if (lastOpenTry != 0 && (frameTime - lastOpenTry) < kShmRetryNs) {
            return false;  // ainda no intervalo do freio
        }
        lastOpenTry = frameTime;

        shm = OpenFileMappingW(FILE_MAP_WRITE, FALSE, shmName);
        shmWritable = (shm != nullptr);
        if (!shm) {
            shm = OpenFileMappingW(FILE_MAP_READ, FALSE, shmName);
        }
        if (!shm) {
            return false;
        }
        shmPtr = MapViewOfFile(shm, shmWritable ? FILE_MAP_WRITE : FILE_MAP_READ,
                               0, 0, static_cast<SIZE_T>(shmSize));
        if (!shmPtr) {
            CloseHandle(shm);
            shm = nullptr;
            shmWritable = false;
            return false;
        }
        LogLine("SHM aberta [%ls] (%s)", shmName, shmWritable ? "RW" : "RO");
        return true;
    }

    // Valida o prefixo FIXO do cabeçalho (magic + version). Um mismatch de versão =
    // DLL e app compilados contra layouts diferentes; rejeita (em vez de ler offsets de
    // pose errados em silêncio) e loga UMA vez o motivo, orientando o rebuild conjunto.
    bool HeaderValid(const IracerFrameHeader* hdr) {
        if (hdr->magic != IRACER_SHM_MAGIC) {
            return false;
        }
        if (hdr->version != IRACER_SHM_VERSION) {
            if (!shmVersionLogged) {
                LogLine("SHM [%ls] REJEITADA: versão %u != esperada %u — recompile a DLL e o app JUNTOS",
                        shmName, hdr->version, IRACER_SHM_VERSION);
                shmVersionLogged = true;
            }
            return false;
        }
        return true;
    }

    // Sem frame válido o painel simplesmente não aparece — e um painel que some sem dizer
    // por quê dá pra ficar uma sessão inteira sem saber se o problema era SHM fechada,
    // cabeçalho de outro build, resolução trocada ou app parado. A cada 10 s a gente loga o
    // motivo com os números.
    //
    // A garganta é por TEMPO, não por contagem de chamadas: enquanto o painel não tem imagem
    // válida a consulta acontece a cada frame (90 Hz e não os 10 Hz do gate de conteúdo), e
    // um contador calibrado no gate viraria uma linha por segundo pela sessão inteira.
    static const XrTime kNullLogEveryNs = 10'000'000'000;

    void LogNullReason(const char* motivo, const IracerFrameHeader* hdr) {
        if (lastNullLog != 0 && (frameTime - lastNullLog) < kNullLogEveryNs) {
            return;  // já logamos há pouco; não vira spam
        }
        lastNullLog = frameTime;
        if (hdr) {
            LogLine("Sem frame [%ls]: %s (magic=0x%08X ver=%u %ux%u frame=%u) — esperado "
                    "(magic=0x%08X ver=%u %ux%u)",
                    shmName, motivo, hdr->magic, hdr->version, hdr->width, hdr->height,
                    hdr->frame, IRACER_SHM_MAGIC, IRACER_SHM_VERSION, width, height);
        } else {
            LogLine("Sem frame [%ls]: %s — o app está aberto e escrevendo?", shmName, motivo);
        }
    }

    // Pixels VÁLIDOS E ATUAIS, ou nullptr. "Atual" é o contador `frame` do escritor: ele
    // sobe a cada escrita, então parar de subir é a única evidência que temos de que o app
    // do outro lado morreu — o mapeamento continua aberto e a última imagem continua lá,
    // intacta e mentindo. Com o quad congelado o jogador veria posições de dez voltas atrás
    // como se fossem de agora, que é pior do que não ver overlay nenhum.
    const uint8_t* TryGetFramePixels(XrTime now) {
        if (!EnsureShmOpen()) {
            LogNullReason("SHM não abriu", nullptr);
            return nullptr;
        }
        const IracerFrameHeader* hdr = static_cast<const IracerFrameHeader*>(shmPtr);
        if (!HeaderValid(hdr)) {
            LogNullReason("cabeçalho inválido", hdr);
            return nullptr;
        }
        if (hdr->width != width || hdr->height != height) {
            LogNullReason("resolução diferente da do painel", hdr);
            return nullptr;
        }

        const uint32_t contador = hdr->frame;
        if (!frameCounterInit) {
            // O escritor incrementa DEPOIS de copiar os pixels, então `frame == 0` é
            // mapeamento recém-criado e ainda zerado: não há imagem, só a página limpa.
            if (contador == 0) {
                LogNullReason("nenhum frame escrito ainda", hdr);
                return nullptr;
            }
            frameCounterInit = true;
            lastFrameCounter = contador;
            lastFrameAdvance = now;
        } else if (contador != lastFrameCounter) {
            lastFrameCounter = contador;
            lastFrameAdvance = now;
        } else if ((now - lastFrameAdvance) >= kFrameStaleNs) {
            LogNullReason("contador de frame parado (app fechou?)", hdr);
            return nullptr;
        }

        if (!shmLogged) {
            LogLine("Primeiro frame [%ls] (frame=%u)", shmName, hdr->frame);
            shmLogged = true;
        }
        lastNullLog = 0;  // voltou a fluir: o próximo problema loga na hora
        return static_cast<const uint8_t*>(shmPtr) + sizeof(IracerFrameHeader);
    }

    OverlayConfig GetConfig() {
        if (!EnsureShmOpen()) {
            return defaultCfg;
        }
        const IracerFrameHeader* hdr = static_cast<const IracerFrameHeader*>(shmPtr);
        if (!HeaderValid(hdr)) {
            return defaultCfg;
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

    // Header p/ o teclado escrever (nullptr se a SHM não abriu RW).
    IracerFrameHeader* HeaderRW() {
        if (!EnsureShmOpen() || !shmWritable) {
            return nullptr;
        }
        IracerFrameHeader* hdr = static_cast<IracerFrameHeader*>(shmPtr);
        return HeaderValid(hdr) ? hdr : nullptr;
    }

    // Devolve o painel ao estado "sem swapchain". Usado no teardown e em QUALQUER erro do
    // Setup: um painel meio montado (swapchain viva, RTV faltando, textura indefinida) é
    // pior que painel nenhum, porque o xrEndFrame passa a anexar um quad cujo conteúdo
    // ninguém escreveu.
    void DestroySwapchainResources() {
        for (auto* rtv : rtvs) {
            if (rtv) rtv->Release();
        }
        rtvs.clear();
        textures.clear();
        if (swapchain != XR_NULL_HANDLE && g_next_xrDestroySwapchain) {
            g_next_xrDestroySwapchain(swapchain);
        }
        swapchain = XR_NULL_HANDLE;
        rendered  = false;
    }

    // Monta swapchain + RTVs deste painel (device do iRacing). Formato vem escolhido.
    void Setup(XrSession session, int64_t format) {
        XrSwapchainCreateInfo sc{XR_TYPE_SWAPCHAIN_CREATE_INFO};
        sc.usageFlags  = XR_SWAPCHAIN_USAGE_COLOR_ATTACHMENT_BIT | XR_SWAPCHAIN_USAGE_SAMPLED_BIT;
        sc.format      = format;
        sc.sampleCount = 1;
        sc.width       = width;
        sc.height      = height;
        sc.faceCount   = 1;
        sc.arraySize   = 1;
        sc.mipCount    = 1;
        if (g_next_xrCreateSwapchain(session, &sc, &swapchain) != XR_SUCCESS) {
            LogLine("ERRO: xrCreateSwapchain falhou [%ls]", shmName);
            swapchain = XR_NULL_HANDLE;
            return;
        }

        uint32_t imgCount = 0;
        if (g_next_xrEnumerateSwapchainImages(swapchain, 0, &imgCount, nullptr) != XR_SUCCESS ||
            imgCount == 0) {
            LogLine("ERRO: contagem de imagens do swapchain falhou [%ls]", shmName);
            DestroySwapchainResources();
            return;
        }
        std::vector<XrSwapchainImageD3D11KHR> images(imgCount, {XR_TYPE_SWAPCHAIN_IMAGE_D3D11_KHR});
        if (g_next_xrEnumerateSwapchainImages(
                swapchain, imgCount, &imgCount,
                reinterpret_cast<XrSwapchainImageBaseHeader*>(images.data())) != XR_SUCCESS) {
            LogLine("ERRO: xrEnumerateSwapchainImages falhou [%ls]", shmName);
            DestroySwapchainResources();
            return;
        }

        // Muitos runtimes (VDXR) criam a textura do swapchain como TYPELESS; um DESC
        // explícito com o formato concreto evita E_INVALIDARG no CreateRenderTargetView.
        D3D11_RENDER_TARGET_VIEW_DESC rtvDesc{};
        rtvDesc.Format             = static_cast<DXGI_FORMAT>(format);
        rtvDesc.ViewDimension      = D3D11_RTV_DIMENSION_TEXTURE2D;
        rtvDesc.Texture2D.MipSlice = 0;

        rtvs.assign(imgCount, nullptr);
        textures.assign(imgCount, nullptr);
        for (uint32_t i = 0; i < imgCount; ++i) {
            if (images[i].texture == nullptr) {
                LogLine("ERRO: imagem %u do swapchain veio nula [%ls]", i, shmName);
                DestroySwapchainResources();
                return;
            }
            textures[i] = images[i].texture;
            HRESULT hr = g_device->CreateRenderTargetView(images[i].texture, &rtvDesc, &rtvs[i]);
            if (FAILED(hr)) {
                LogLine("ERRO: CreateRenderTargetView[%u] hr=0x%08lX [%ls] — painel desativado",
                        i, (unsigned long)hr, shmName);
                DestroySwapchainResources();
                return;
            }
        }
        LogLine("Painel pronto [%ls]: %ux%u, %u imagens", shmName, width, height, imgCount);
    }

    // Sobe os pixels da SHM na textura. Devolve false quando não deu pra subir — e aí o
    // painel NÃO tem imagem válida, então o quad não pode ser anexado.
    //
    // Não existe mais o cinza "aguardando": ele era um retângulo opaco plantado na frente
    // do jogador dizendo apenas que nós estávamos vivos. Sem pixels, o certo é sumir.
    bool RenderContent(uint32_t imageIndex, const uint8_t* pixels) {
        if (pixels == nullptr || imageIndex >= textures.size() || textures[imageIndex] == nullptr) {
            return false;
        }
        const uint8_t* fonte = pixels;
        if (g_formatBgra) {
            // RGBA → BGRA, uma palavra por vez. Em little-endian o pixel RGBA lido como
            // uint32 é 0xAABBGGRR; o BGRA que o runtime quer é 0xAARRGGBB. Só os bytes de
            // R e B trocam de lugar; A e G ficam onde estão.
            const size_t bytes = static_cast<size_t>(width) * height * 4;
            if (swizzled.size() != bytes) {
                swizzled.resize(bytes);
            }
            const uint32_t* s = reinterpret_cast<const uint32_t*>(pixels);
            uint32_t*       d = reinterpret_cast<uint32_t*>(swizzled.data());
            const size_t    n = bytes / 4;
            for (size_t i = 0; i < n; ++i) {
                const uint32_t v = s[i];
                d[i] = (v & 0xFF00FF00u) | ((v & 0x00FF0000u) >> 16) | ((v & 0x000000FFu) << 16);
            }
            fonte = swizzled.data();
        }
        g_context->UpdateSubresource(textures[imageIndex], 0, nullptr, fonte, width * 4, 0);
        return true;
    }

    // Adquire uma imagem do swapchain e sobe os pixels. `rendered` só vira true depois de
    // pixels VÁLIDOS terem ido pra textura.
    bool AcquireAndRender(XrTime now, const uint8_t* pixels) {
        if (pixels == nullptr) {
            return false;
        }
        uint32_t index = 0;
        XrSwapchainImageAcquireInfo acq{XR_TYPE_SWAPCHAIN_IMAGE_ACQUIRE_INFO};
        if (g_next_xrAcquireSwapchainImage(swapchain, &acq, &index) != XR_SUCCESS) {
            return false;
        }
        XrSwapchainImageWaitInfo wait{XR_TYPE_SWAPCHAIN_IMAGE_WAIT_INFO};
        wait.timeout = XR_INFINITE_DURATION;
        if (g_next_xrWaitSwapchainImage(swapchain, &wait) != XR_SUCCESS) {
            // Sem o wait a imagem não foi adquirida do ponto de vista da spec; nada a
            // liberar e nada a desenhar.
            return false;
        }
        const bool ok = RenderContent(index, pixels);
        // A imagem foi adquirida E esperada: a spec exige devolvê-la mesmo se não pintamos.
        XrSwapchainImageReleaseInfo rel{XR_TYPE_SWAPCHAIN_IMAGE_RELEASE_INFO};
        g_next_xrReleaseSwapchainImage(swapchain, &rel);
        if (!ok) {
            return false;
        }
        if (!rendered) {
            LogLine("Primeiro render enviado [%ls] (displayTime=%lld)", shmName, (long long)now);
        }
        rendered   = true;
        lastRender = now;
        return true;
    }

    // Monta o quad de composição a partir da config (espaço/pose/giro/pitch/escala).
    void BuildQuad(XrCompositionLayerQuad& quad, const OverlayConfig& cfg) {
        XrSpace space = g_viewSpace;
        if (cfg.lockMode == IRACER_LOCK_WORLD) {
            if (anchorSpace != XR_NULL_HANDLE) {
                space = anchorSpace;
            } else if (g_localSpace != XR_NULL_HANDLE) {
                space = g_localSpace;
            }
        }
        // Orientação = yaw ⊗ pitch (produto de Hamilton, forma fechada).
        const float halfYaw   = cfg.yawDeg   * (kPi / 180.0f) * 0.5f;
        const float halfPitch = cfg.pitchDeg * (kPi / 180.0f) * 0.5f;
        const float cy = std::cos(halfYaw),   sy = std::sin(halfYaw);
        const float cp = std::cos(halfPitch), sp = std::sin(halfPitch);
        const XrQuaternionf orient{cy * sp, sy * cp, -sy * sp, cy * cp};

        quad = XrCompositionLayerQuad{XR_TYPE_COMPOSITION_LAYER_QUAD};
        quad.layerFlags               = XR_COMPOSITION_LAYER_BLEND_TEXTURE_SOURCE_ALPHA_BIT;
        quad.space                    = space;
        quad.eyeVisibility            = XR_EYE_VISIBILITY_BOTH;
        quad.subImage.swapchain       = swapchain;
        quad.subImage.imageRect       = XrRect2Di{{0, 0},
                                                  {static_cast<int32_t>(width), static_cast<int32_t>(height)}};
        quad.subImage.imageArrayIndex = 0;
        quad.pose.orientation         = orient;
        quad.pose.position            = XrVector3f{cfg.posX, cfg.posY, cfg.posZ};
        quad.size                     = XrExtent2Df{quadW * cfg.scale, quadH * cfg.scale};
    }

    // Reancora o world-lock DESTE painel na cabeça atual (achata orientação p/ só-yaw).
    void DoRecenter(XrTime now) {
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
            return;
        }
        const XrQuaternionf q = loc.pose.orientation;
        const float yaw = std::atan2(2.0f * (q.x * q.z + q.w * q.y),
                                     1.0f - 2.0f * (q.x * q.x + q.y * q.y));
        const float hy = yaw * 0.5f;

        XrReferenceSpaceCreateInfo info{XR_TYPE_REFERENCE_SPACE_CREATE_INFO};
        info.referenceSpaceType = XR_REFERENCE_SPACE_TYPE_LOCAL;
        info.poseInReferenceSpace.position    = loc.pose.position;
        info.poseInReferenceSpace.orientation = XrQuaternionf{0.0f, std::sin(hy), 0.0f, std::cos(hy)};

        XrSpace newAnchor = XR_NULL_HANDLE;
        if (g_next_xrCreateReferenceSpace(g_session, &info, &newAnchor) != XR_SUCCESS) {
            LogLine("Recenter [%ls]: xrCreateReferenceSpace falhou", shmName);
            return;
        }
        if (anchorSpace != XR_NULL_HANDLE && g_next_xrDestroySpace) {
            g_next_xrDestroySpace(anchorSpace);
        }
        anchorSpace = newAnchor;
        LogLine("Recenter [%ls]: reancorado (yaw=%.1f)", shmName, yaw * (180.0f / kPi));
    }

    // Detecta pedidos de recentro deste painel: bump do app (recenterSeq) e tecla (VK).
    void CheckRecenter(XrTime now) {
        if (!EnsureShmOpen()) {
            return;
        }
        const IracerFrameHeader* hdr = static_cast<const IracerFrameHeader*>(shmPtr);
        if (!HeaderValid(hdr)) {
            return;
        }
        bool trigger = false;

        const uint32_t seq = hdr->recenterSeq;
        if (!recenterSeqInit) {
            lastRecenterSeq = seq;
            recenterSeqInit = true;
        } else if (seq != lastRecenterSeq) {
            lastRecenterSeq = seq;
            trigger = true;
        }

        const uint32_t vk = hdr->recenterKey;
        if (vk != 0 && vk < 256) {
            const bool downNow = (GetAsyncKeyState(static_cast<int>(vk)) & 0x8000) != 0;
            if (downNow && !prevRecenterKey) {
                trigger = true;
            }
            prevRecenterKey = downNow;
        } else {
            prevRecenterKey = false;
        }

        if (trigger) {
            DoRecenter(now);
        }
    }

    void Teardown() {
        if (shmPtr) {
            UnmapViewOfFile(shmPtr);
            shmPtr = nullptr;
        }
        if (shm) {
            CloseHandle(shm);
            shm = nullptr;
        }
        shmLogged   = false;
        shmVersionLogged = false;
        shmWritable = false;
        lastNullLog = 0;
        DestroySwapchainResources();
        if (anchorSpace != XR_NULL_HANDLE && g_next_xrDestroySpace) {
            g_next_xrDestroySpace(anchorSpace);
        }
        anchorSpace      = XR_NULL_HANDLE;
        lastRender       = 0;
        recenterSeqInit  = false;
        prevRecenterKey  = false;
        frameCounterInit = false;
        lastFrameCounter = 0;
        lastFrameAdvance = 0;
        swizzled.clear();
        swizzled.shrink_to_fit();
        frameTime          = 0;
        openTriedThisFrame = false;
        lastOpenTry        = 0;
    }
};

// Os dois painéis. Aggregate-init dos campos FIXOS; o resto usa os defaults acima.
//
// Os `defaultCfg` valem só enquanto o app não escreveu a config na SHM — e mesmo assim
// precisam ser os MESMOS defaults do Rust (`def_*` em commands/vr_overlay.rs) e do JS
// (`factory` em src/overlay/overlayPose.js). Divergir faz o painel nascer num lugar e
// pular pra outro assim que o app conecta. O guard
// scripts/tests/vr-overlay-contrato-dimensoes.test.mjs cobra a igualdade nos três lados.
static Panel g_tower = {
    IRACER_SHM_NAME, IRACER_OVERLAY_W, IRACER_OVERLAY_H, IRACER_SHM_SIZE,
    0.45f, 0.90f,
    {IRACER_LOCK_WORLD, 0.0f, 0.61f, -1.29f, 0.0f, 30.0f, 1.7f, true},
};
static Panel g_engineer = {
    IRACER_ENGINEER_SHM_NAME, IRACER_ENGINEER_W, IRACER_ENGINEER_H, IRACER_ENGINEER_SHM_SIZE,
    0.90f, 0.225f,  // banner 4:1
    {IRACER_LOCK_WORLD, 0.0f, 0.22f, -0.73f, 0.0f, 10.0f, 0.26f, true},
};
static Panel* const g_panels[2] = {&g_tower, &g_engineer};

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
//
// Só os quatro formatos 32-bit que sabemos preencher (ver kKnownFormats). Se nenhum deles
// estiver na lista do runtime, devolvemos 0 e o overlay não sobe: pegar `formats[0]` às
// cegas era escolher um layout de pixel desconhecido e mandar o buffer RGBA nele — o
// UpdateSubresource aceita, e o resultado é lixo colorido no headset sem uma linha de erro.
static int64_t PickColorFormat(XrSession session) {
    g_formatBgra = false;
    uint32_t count = 0;
    if (g_next_xrEnumerateSwapchainFormats(session, 0, &count, nullptr) != XR_SUCCESS || count == 0) {
        return 0;
    }
    std::vector<int64_t> formats(count);
    if (g_next_xrEnumerateSwapchainFormats(session, count, &count, formats.data()) != XR_SUCCESS) {
        return 0;
    }
    for (const ColorFormat& want : kKnownFormats) {
        for (int64_t have : formats) {
            if (have == want.dxgi) {
                g_formatBgra = want.bgra;
                return want.dxgi;
            }
        }
    }
    LogLine("ERRO: o runtime não oferece nenhum formato 32-bit conhecido (%u ofertados; "
            "o primeiro é %lld) — overlay desativado",
            count, formats.empty() ? 0LL : (long long)formats[0]);
    return 0;
}

// ─── Monta os espaços de referência (compartilhados) + os dois painéis ────────
static void SetupOverlay(XrSession session) {
    g_session        = session;
    g_compositionOff = false;
    g_layerErrStreak = 0;

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
        LogLine("AVISO: xrCreateReferenceSpace(LOCAL) falhou — world-lock cai pro VIEW");
        g_localSpace = XR_NULL_HANDLE;
    }

    int64_t format = PickColorFormat(session);
    if (format == 0) {
        LogLine("ERRO: nenhum formato de swapchain disponível");
        return;
    }
    LogLine("Formato de cor escolhido: %lld (%s)", (long long)format,
            g_formatBgra ? "BGRA — converte R<->B" : "RGBA — direto");

    for (Panel* p : g_panels) {
        p->Setup(session, format);
    }
}

// ─── Ajuste por TECLADO (modo: segure Ctrl direito). Ctrl+T alterna o alvo ────
static XrTime g_lastKbd = 0;
static int    g_kbdTarget = 0;      // 0 = torre, 1 = rádio
static bool   g_prevL = false;      // toggle de trava
static bool   g_prevH = false;      // toggle de visível
static bool   g_prevC = false;      // recentro (Ctrl dir + C)
static bool   g_prevT = false;      // alterna alvo (Ctrl dir + T)

static float ClampF(float v, float lo, float hi) {
    return v < lo ? lo : (v > hi ? hi : v);
}

static void ProcessKeyboard(XrTime now) {
    if (g_lastKbd != 0 && (now - g_lastKbd) < 20'000'000) {  // ~50 Hz
        return;
    }
    g_lastKbd = now;

    auto down = [](int vk) { return (GetAsyncKeyState(vk) & 0x8000) != 0; };

    // Fora do modo de ajuste (sem Ctrl direito): zera as bordas e sai.
    if (!down(VK_RCONTROL)) {
        g_prevL = g_prevH = g_prevC = g_prevT = false;
        return;
    }

    // Ctrl dir + T: alterna QUAL painel o teclado controla (torre ↔ rádio).
    const bool tKey = down('T');
    if (tKey && !g_prevT) {
        g_kbdTarget = (g_kbdTarget == 0) ? 1 : 0;
        LogLine("Teclado VR: alvo = %s", g_kbdTarget == 0 ? "TORRE" : "RADIO");
    }
    g_prevT = tKey;

    Panel* p = g_panels[g_kbdTarget];
    IracerFrameHeader* hdr = p->HeaderRW();
    if (!hdr) {
        return;  // painel-alvo sem SHM gravável ainda
    }

    // Recentro por teclado (Ctrl dir + C): reancora o painel-alvo agora.
    const bool cKey = down('C');
    if (cKey && !g_prevC) {
        p->DoRecenter(now);
    }
    g_prevC = cKey;

    const float posStep   = 0.005f;
    const float yawStep   = 0.5f;
    const float pitchStep = 0.5f;
    const float scaleStep = 0.01f;
    bool changed = false;

    if (down(VK_LEFT))  { hdr->posX -= posStep; changed = true; }
    if (down(VK_RIGHT)) { hdr->posX += posStep; changed = true; }
    if (down(VK_UP))    { hdr->posY += posStep; changed = true; }
    if (down(VK_DOWN))  { hdr->posY -= posStep; changed = true; }
    if (down(VK_PRIOR)) { hdr->posZ += posStep; changed = true; }  // PageUp   = mais perto
    if (down(VK_NEXT))  { hdr->posZ -= posStep; changed = true; }  // PageDown = mais longe
    if (down(VK_ADD)      || down(VK_OEM_PLUS))  { hdr->scale += scaleStep; changed = true; }
    if (down(VK_SUBTRACT) || down(VK_OEM_MINUS)) { hdr->scale -= scaleStep; changed = true; }
    if (down(VK_OEM_COMMA))  { hdr->yawDeg -= yawStep; changed = true; }  // ','
    if (down(VK_OEM_PERIOD)) { hdr->yawDeg += yawStep; changed = true; }  // '.'
    if (down(VK_HOME)) { hdr->pitchDeg += pitchStep; changed = true; }
    if (down(VK_END))  { hdr->pitchDeg -= pitchStep; changed = true; }

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

    hdr->posX     = ClampF(hdr->posX, -1.5f, 1.5f);
    hdr->posY     = ClampF(hdr->posY, -1.2f, 1.2f);
    hdr->posZ     = ClampF(hdr->posZ, -2.5f, -0.3f);
    hdr->yawDeg   = ClampF(hdr->yawDeg, -45.0f, 45.0f);
    hdr->pitchDeg = ClampF(hdr->pitchDeg, -45.0f, 45.0f);
    hdr->scale    = ClampF(hdr->scale, 0.2f, 2.0f);
    hdr->configEpoch++;
}

// ─── Interceptação: xrEndFrame (onde anexamos os quads) ───────────────────────
static XrResult XRAPI_CALL Layer_xrEndFrame(XrSession session, const XrFrameEndInfo* frameEndInfo) {
    if (frameEndInfo == nullptr || g_next_xrEndFrame == nullptr || g_viewSpace == XR_NULL_HANDLE ||
        g_compositionOff) {
        return g_next_xrEndFrame ? g_next_xrEndFrame(session, frameEndInfo)
                                 : XR_ERROR_FUNCTION_UNSUPPORTED;
    }

    const XrTime now = frameEndInfo->displayTime;

    // Um relógio só para o frame inteiro, e o memo da abertura da SHM zerado.
    for (Panel* p : g_panels) {
        p->BeginFrame(now);
    }
    // Teclado (Ctrl direito) escreve a pose do painel-alvo de volta na SHM.
    ProcessKeyboard(now);
    // Recentro por painel (botão do app OU tecla configurável).
    for (Panel* p : g_panels) {
        p->CheckRecenter(now);
    }

    // Recopia as camadas do jogo; vamos ACRESCENTAR os nossos quads no fim.
    std::vector<const XrCompositionLayerBaseHeader*> layers(
        frameEndInfo->layers, frameEndInfo->layers + frameEndInfo->layerCount);

    // Storage dos quads — precisa sobreviver até a chamada de xrEndFrame (mesmo escopo).
    XrCompositionLayerQuad quads[2];
    int quadCount = 0;

    for (Panel* p : g_panels) {
        if (p->swapchain == XR_NULL_HANDLE) {
            continue;
        }
        const OverlayConfig cfg = p->GetConfig();
        if (!cfg.visible) {
            continue;
        }
        // Enquanto o painel não tem imagem válida, reconsulta todo frame: é barato (leitura
        // de cabeçalho, com o freio da abertura da SHM por trás) e é o que faz o quad voltar
        // assim que o app reconecta.
        const bool timeToRender = !p->rendered || (now - p->lastRender) >= kRenderPeriodNs;
        if (timeToRender) {
            const uint8_t* pixels = p->TryGetFramePixels(now);
            if (pixels == nullptr) {
                // Sem SHM, cabeçalho de outro build, resolução trocada ou contador parado:
                // o painel perde a validade e o quad SAI da composição neste frame mesmo.
                p->rendered = false;
            } else {
                p->AcquireAndRender(now, pixels);
            }
        }
        if (!p->rendered) {
            continue;  // sem pixels válidos: não anexa nada
        }
        p->BuildQuad(quads[quadCount], cfg);
        layers.push_back(reinterpret_cast<const XrCompositionLayerBaseHeader*>(&quads[quadCount]));
        quadCount++;
    }

    if (quadCount == 0) {
        return g_next_xrEndFrame(session, frameEndInfo);  // nada nosso a anexar
    }

    XrFrameEndInfo mutated = *frameEndInfo;
    mutated.layers     = layers.data();
    mutated.layerCount = static_cast<uint32_t>(layers.size());
    const XrResult res = g_next_xrEndFrame(session, &mutated);

    // `res >= 0` e não `== XR_SUCCESS`: em XrResult o negativo é erro e o resto é sucesso, e
    // xrEndFrame também devolve XR_SESSION_LOSS_PENDING, que é sucesso e não tem nada a ver
    // com as nossas camadas.
    if (res >= 0) {
        g_layerErrStreak = 0;
    } else if (ErroDeCamada(res)) {
        if (++g_layerErrStreak >= kLayerErrLimit) {
            g_compositionOff = true;
            LogLine("Composição DESLIGADA: xrEndFrame recusou as nossas camadas %u vezes "
                    "seguidas (último erro %d). O jogo segue; o overlay some até a próxima "
                    "sessão.",
                    g_layerErrStreak, (int)res);
        } else {
            LogLine("AVISO: xrEndFrame devolveu erro de camada (%d), %u/%u", (int)res,
                    g_layerErrStreak, kLayerErrLimit);
        }
    }
    return res;
}

// ─── Interceptação: xrCreateSession (pegamos o device D3D11 aqui) ─────────────
static XrResult XRAPI_CALL Layer_xrCreateSession(XrInstance instance,
                                                 const XrSessionCreateInfo* createInfo,
                                                 XrSession* session) {
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
        LogLine("Sessão criada; binding D3D11 encontrado. Montando overlays...");
        SetupOverlay(*session);
    } else {
        LogLine("Sessão criada, mas SEM binding D3D11 (D3D12/Vulkan?). Overlay desativado.");
    }
    return res;
}

// ─── Interceptação: limpeza ───────────────────────────────────────────────────
static void TeardownOverlay() {
    for (Panel* p : g_panels) {
        p->Teardown();
    }
    if (g_viewSpace != XR_NULL_HANDLE && g_next_xrDestroySpace) {
        g_next_xrDestroySpace(g_viewSpace);
    }
    if (g_localSpace != XR_NULL_HANDLE && g_next_xrDestroySpace) {
        g_next_xrDestroySpace(g_localSpace);
    }
    if (g_context) {
        g_context->Release();
        g_context = nullptr;
    }
    g_viewSpace  = XR_NULL_HANDLE;
    g_localSpace = XR_NULL_HANDLE;
    g_session    = XR_NULL_HANDLE;
    g_device     = nullptr;
    // A desistência da composição é POR SESSÃO: a próxima começa limpa (o motivo pode ter
    // sido a contagem de camadas daquele frame, não um defeito permanente).
    g_compositionOff = false;
    g_layerErrStreak = 0;
    g_formatBgra     = false;
}

static XrResult XRAPI_CALL Layer_xrDestroySession(XrSession session) {
    LogLine("xrDestroySession: limpando overlays");
    TeardownOverlay();
    return g_next_xrDestroySession(session);
}

static XrResult XRAPI_CALL Layer_xrDestroyInstance(XrInstance instance) {
    LogLine("xrDestroyInstance");
    XrResult res = g_next_xrDestroyInstance(instance);
    g_instance = XR_NULL_HANDLE;
    // Saiu do VR de forma limpa (o jogador fechou o sim pelo menu): apaga o sinal já.
    // Se o processo morrer de outro jeito, o SO fecha o handle e o efeito é o mesmo.
    ClearVrActive();
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

    XrApiLayerCreateInfo forwarded = *apiLayerInfo;
    forwarded.nextInfo = apiLayerInfo->nextInfo->next;

    XrResult res = nextCreate(info, &forwarded, instance);
    if (res == XR_SUCCESS) {
        g_instance = *instance;
        LoadNextFunctions(*instance);
        // A instância existir JÁ significa VR — antes de sessão, de swapchain, de
        // qualquer frame. É o ponto mais cedo em que a resposta é confiável, e é o que
        // deixa o app decidir o pipeline antes da corrida começar.
        SignalVrActive();
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

    apiLayerRequest->layerInterfaceVersion  = XR_CURRENT_LOADER_API_LAYER_VERSION;
    apiLayerRequest->layerApiVersion        = XR_CURRENT_API_VERSION;
    apiLayerRequest->getInstanceProcAddr    = Layer_xrGetInstanceProcAddr;
    apiLayerRequest->createApiLayerInstance = Layer_xrCreateApiLayerInstance;

    LogLine("xrNegotiateLoaderApiLayerInterface: layer iRacer (2 painéis) carregada.");
    return XR_SUCCESS;
}
