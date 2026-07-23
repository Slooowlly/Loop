// Contrato da MEMÓRIA COMPARTILHADA entre o app (escritor) e a API layer (leitor).
//
// Layout do bloco mapeado:
//   [ IracerFrameHeader (64 bytes) ][ W*H*4 bytes RGBA ]
//
// Pixels em RGBA (R primeiro), casando com o formato R8G8B8A8_UNORM_SRGB do
// swapchain — que também é a ordem do getImageData de um <canvas>, então o app
// pode cuspir os bytes do canvas direto aqui, sem reordenar.
//
// Protocolo mínimo: o escritor preenche os pixels e incrementa `frame`. O leitor,
// a ~10 Hz, sobe o conteúdo atual pra textura. Sem lock: um frame "rasgado"
// (metade novo/metade velho) num overlay que muda devagar é invisível na prática.
//
// POSE/CONFIG: o app também escreve um bloco de configuração (trava + posição +
// giro + escala + visível) logo depois do frame/contador. Quem escreve o frame e
// quem escreve a config tocam BYTES DISJUNTOS — nunca se atropelam. A layer lê a
// config a cada frame (custo desprezível) e monta o quad a partir dela.
#pragma once
#include <cstdint>

#define IRACER_SHM_NAME L"Local\\iRacerOverlayFrame"

static const uint32_t IRACER_SHM_MAGIC = 0x52414352;  // 'RACR'
// Versão do LAYOUT do cabeçalho. BUMP a cada mudança de campo/ordem/tamanho de
// `IracerFrameHeader`. O leitor rejeita um mapeamento cujo `version` não bate com o
// seu — assim uma DLL velha contra um app novo (ou vice-versa) FALHA ALTO em vez de
// ler offsets de pose errados em silêncio. `magic`+`version` são o prefixo FIXO do
// cabeçalho (words 0 e 1) e NUNCA mudam de posição; campos novos entram DEPOIS deles.
static const uint32_t IRACER_SHM_VERSION = 2;  // v2: + pitchDeg + recenterSeq/Key (60→64 bytes)
// Resolução do painel em pixels. 1024×2048 = supersampling 2× do layout lógico
// (512×1024) — o app desenha em 2× e manda esse buffer, deixando o texto nítido
// no VR (menos "borrão" de ampliação, mais perto do RaceLab). O tamanho FÍSICO do
// quad em metros NÃO muda; só cabe mais pixel na mesma área.
static const uint32_t IRACER_OVERLAY_W = 1024;
static const uint32_t IRACER_OVERLAY_H = 2048;

// ── 2º painel: RÁDIO DA EQUIPE (quad independente da torre) ──
// Mesmo cabeçalho (IracerFrameHeader) e mesmo magic; muda só o NOME do mapeamento e a
// resolução (banner largo e baixo, não retrato). O layer abre os dois mapeamentos e
// desenha dois quads com config separada. 2× do lógico ~512×128.
#define IRACER_ENGINEER_SHM_NAME L"Local\\iRacerEngineerFrame"
static const uint32_t IRACER_ENGINEER_W = 1024;
static const uint32_t IRACER_ENGINEER_H = 256;

// Modos de trava do painel no espaço VR.
static const int32_t IRACER_LOCK_HEAD  = 0;  // preso à cabeça (VIEW): segue o olhar
static const int32_t IRACER_LOCK_WORLD = 1;  // preso ao cockpit (LOCAL): fica fixo

#pragma pack(push, 4)
struct IracerFrameHeader {
    uint32_t magic;   // IRACER_SHM_MAGIC        (word 0 — prefixo fixo)
    uint32_t version; // IRACER_SHM_VERSION      (word 1 — prefixo fixo; guarda o layout)
    uint32_t width;   // deve bater com IRACER_OVERLAY_W
    uint32_t height;  // deve bater com IRACER_OVERLAY_H
    uint32_t frame;   // contador que o escritor incrementa a cada atualização
    // ── Config (escrita pelo comando set_pose do app; lida pela layer todo frame) ──
    uint32_t configEpoch;  // bump a cada mudança (só diagnóstico/log)
    int32_t  lockMode;     // IRACER_LOCK_HEAD | IRACER_LOCK_WORLD
    float    posX;         // metros: +direita
    float    posY;         // metros: +cima
    float    posZ;         // metros: -frente (negativo = à sua frente)
    float    yawDeg;       // giro em torno de Y (angular o painel na sua direção)
    float    pitchDeg;     // inclinação em torno de X (+ = topo tomba na sua direção)
    float    scale;        // multiplica o tamanho base do quad (1.0 = padrão)
    uint32_t visible;      // 0 = não desenha; 1 = desenha
    // ── Recentro (reancora o world-lock na cabeça atual) ──
    uint32_t recenterSeq;  // app INCREMENTA p/ pedir recentro; a layer detecta a mudança
    uint32_t recenterKey;  // Virtual-Key da tecla de recentro dentro do VR (0 = desligada)
};
#pragma pack(pop)

// Guarda de ABI: se um campo/padding mudar o tamanho sem bumpar a versão/atualizar o
// escritor Rust (que usa offsets de palavra fixos), isto FALHA A COMPILAÇÃO. 16 words × 4B.
static_assert(sizeof(IracerFrameHeader) == 64, "IracerFrameHeader deve ter 64 bytes (bump IRACER_SHM_VERSION + offsets em vr_overlay.rs)");

static const uint64_t IRACER_SHM_SIZE =
    sizeof(IracerFrameHeader) +
    static_cast<uint64_t>(IRACER_OVERLAY_W) * IRACER_OVERLAY_H * 4;

static const uint64_t IRACER_ENGINEER_SHM_SIZE =
    sizeof(IracerFrameHeader) +
    static_cast<uint64_t>(IRACER_ENGINEER_W) * IRACER_ENGINEER_H * 4;
