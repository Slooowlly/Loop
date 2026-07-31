# iRacer VR Overlay — OpenXR API Layer

> **Distribuição (o caminho normal).** A layer NÃO é mais instalada à mão pelo jogador:
> `scripts/build-vr-layer.mjs` a compila pra `src-tauri/resources/` em cada build (via
> `beforeBuildCommand`), o bundle do Tauri a leva no instalador, e o app registra
> manifesto + chave de registro a cada boot ([vr_layer.rs](../src-tauri/src/commands/vr_layer.rs)).
> O hook do NSIS ([hooks.nsh](../src-tauri/installer/hooks.nsh)) limpa o registro ao
> desinstalar. A DLL é artefato e não vai pro git.
>
> Os scripts `install.ps1`/`uninstall.ps1` daqui continuam úteis só pra **desenvolvimento**
> — registrar um build local sem passar pelo instalador.
>
> A layer também é o **detector de VR** do app: enquanto o iRacing tem uma instância
> OpenXR viva, ela mantém aberto o evento nomeado `Local\iRacerVrActive`, e é assim que o
> Loop sabe que a corrida é de headset (o irsdk não publica nada de HMD). O sinal só
> acende dentro do executável do iRacing — layers implícitas carregam em qualquer app
> OpenXR, e sem esse filtro um menu de VR qualquer acenderia o sinal.

## Histórico — Spike 1

Prova de conceito: uma **OpenXR API layer** em C++ que injeta um **painel de teste
(retângulo magenta)** grudado na sua visão dentro do iRacing rodando em **OpenXR /
VDXR (Virtual Desktop + Pico 4)**.

**Objetivo:** responder só uma pergunta — *"a layer carrega no iRacing e desenha no
óculos?"* Se o retângulo aparecer, o caminho pro overlay real (torre de tempos) está
aberto. Aqui **não** tem dado do jogo, nem torre; é o menor teste possível.

Isto é a **mesma técnica** do RaceLab/OpenKneeboard: não injeta "hack", não lê a
memória do jogo, não toca input — só acrescenta uma camada de composição (um quad)
no `xrEndFrame`. É display-only.

## Desempenho

- O **conteúdo** do painel só é redesenhado a **~10 Hz** (gate por tempo).
- Todo frame a gente só **anexa a referência** do quad já pronto → 1 quad a mais pro
  compositor, custo desprezível. Fora do tick de 10 Hz, **zero** trabalho de GPU nosso.

## Pré-requisitos

- **Visual Studio 2022** (com "Desktop development with C++") ou Build Tools equivalente.
- **CMake ≥ 3.20** e **git** no PATH.
- Conexão de internet no primeiro build (o CMake baixa os headers do OpenXR-SDK).

## 1) Compilar

Num **Developer PowerShell for VS 2022**, dentro de `vr-overlay\`:

```powershell
cmake -S . -B build -G "Visual Studio 17 2022" -A x64
cmake --build build --config Release
```

Saída: `build\Release\iracer_overlay_layer.dll`.

> Se a tag do OpenXR falhar no download, edite `GIT_TAG` no `CMakeLists.txt` para uma
> release existente (ex.: `release-1.1.36`).

### Alternativa SEM CMake (compilar direto com `cl`)

Se não tiver CMake, dá pra compilar só com o compilador do Visual Studio. Primeiro
pegue os headers do OpenXR uma vez:

```powershell
git clone --depth 1 --branch release-1.1.43 https://github.com/KhronosGroup/OpenXR-SDK.git
```

Depois, num **Developer PowerShell for VS 2022** (dentro de `vr-overlay\src\`, com a
pasta `OpenXR-SDK` ao lado):

```powershell
cl /nologo /std:c++17 /LD /EHsc /W4 /I"OpenXR-SDK\include" `
   overlay_layer.cpp /link d3d11.lib dxgi.lib user32.lib /OUT:iracer_overlay_layer.dll
```

Isso gera `iracer_overlay_layer.dll` na mesma pasta. Aponte o `install.ps1` pra ele:
`.\..\scripts\install.ps1 -DllPath "$PWD\iracer_overlay_layer.dll"`.

## 2) Registrar a layer

```powershell
.\scripts\install.ps1
```

Isso gera o manifesto JSON ao lado do `.dll` e registra em
`HKCU\Software\Khronos\OpenXR\1\ApiLayers\Implicit` (por-usuário, **sem admin**).

## 3) Testar

1. Abra o **iRacing** e confirme o runtime de VR em **OpenXR** (o seletor do print).
2. Entre numa sessão de teste/prática em VR.
3. Você deve ver um **retângulo magenta** flutuando ~1 m à frente, um pouco à direita
   e abaixo, acompanhando sua cabeça.

Deu certo? 🎉 Caminho aberto. Não apareceu? Abra o log — ele diz exatamente até onde foi:

```
%TEMP%\iracer_overlay_layer.log
```

Sequência esperada no log: `layer carregada pelo loader` → `Instância OpenXR criada`
→ `binding D3D11 encontrado` → `Overlay pronto` → `Primeiro render do painel enviado`.
Onde parar = onde está o problema.

## Ajustar a posição do painel

Duas formas (as duas persistem e ficam em sincronia):

- **No app:** painel **⚙ posição VR** (canto da tela) — toggle Cockpit/Cabeça + sliders.
- **Por teclado, dentro do VR:** segure **Ctrl direito** (modo de ajuste) e use:

  | Tecla | Ação |
  |---|---|
  | ← / → | mover horizontal |
  | ↑ / ↓ | mover em altura |
  | PageUp / PageDown | aproximar / afastar |
  | `+` / `-` | aumentar / diminuir tamanho |
  | `,` / `.` | girar (yaw) |
  | Home / End | inclinar (pitch): topo tomba pra frente / recosta |
  | `L` | alterna Cockpit ↔ Cabeça |
  | `H` | mostra / esconde o painel |
  | `C` | **recentraliza** o overlay na sua frente (ver abaixo) |

  Segurar move suave (~50 Hz). Os valores respeitam os mesmos limites dos sliders.

## Recentralizar (mesma posição sempre)

O world-lock (Cockpit) prende o painel a um ponto fixo — mas esse ponto depende de
onde você estava ao entrar. **Recentralizar** reancora o overlay na **sua cabeça
agora** (posição + direção horizontal, sem herdar pitch/roll), então ele reaparece
sempre no mesmo lugar relativo ao seu corpo. Três formas:

- **Botão no app:** `⟳ Recentralizar overlay` no painel ⚙.
- **Tecla no VR:** defina uma tecla no painel (campo "Tecla no VR"). Dica: use a
  **mesma tecla do "Center VR" do iRacing** (Options → Controls) — aí um aperte só
  recentra o cockpit E o overlay juntos, na mesma posição pra sempre.
- **Atalho fixo:** **Ctrl direito + C** dentro do VR (sempre disponível).

> Por que não pegamos o botão do iRacing automaticamente: o SDK não expõe os
> atalhos, e o "Center VR" do iRacing não move o espaço LOCAL do OpenXR (ele desloca
> as câmeras internamente). Por isso o recentro é NOSSO — mesma abordagem do
> OpenKneeboard/RaceLab.

> **Definir padrão:** posicionou do jeito ideal? No painel **⚙ posição VR** do app,
> clique **"Definir posição atual como padrão"**. Aí essa vira a pose que o overlay
> assume ao abrir (e o destino do botão "Padrão") — em qualquer óculos.

> **Nitidez:** o painel é desenhado em **1024×2048** (supersampling 2× do layout
> lógico 512×1024). O tamanho físico do quad não muda; só cai mais pixel na mesma
> área, deixando o texto nítido (menos "borrão" que na resolução antiga).

> **Cockpit-lock (padrão):** o painel fica fixo no assento. A origem é o ponto de
> **recentragem** (long-press no Virtual Desktop) — recentre olhando pra frente
> ANTES de posicionar, aí o "à frente" cai dentro do cockpit.

## Ligar/desligar sem desinstalar

Setar a variável de ambiente **`IRACER_OVERLAY_DISABLE=1`** desliga a layer (o loader
respeita isso). Remover a variável religa.

## Desinstalar

```powershell
.\scripts\uninstall.ps1
```

## Próximos passos (depois que o quad aparecer)

1. **Textura compartilhada** — trocar o "pinta de magenta" por uma textura D3D11
   compartilhada que o app Tauri preenche com a torre de tempos (a ~10 Hz).
2. **Dados ao vivo** — ligar o `race_monitor` / SDK.
3. **Posição + toggle** — world-lock (grudar no painel do carro) vs. head-lock,
   tamanho/distância, e um atalho pra mostrar/esconder.
