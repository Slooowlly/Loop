# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## O que é

**Loop** — jogo desktop de carreira no automobilismo **construído em volta do iRacing**. Tauri v2 (Rust) + React 18 + Vite, com SQLite local. O jogador controla **um piloto** subindo uma pirâmide de 9 categorias, cercado por um mundo vivo de 204 pilotos de IA em 102 equipes que correm, evoluem, trocam de equipe e se aposentam sozinhos. Esses dois números saem da soma de `num_equipes` das 9 categorias em [constants/categories.rs](src-tauri/src/constants/categories.rs), com dois assentos por equipe; recontar é uma linha, e o texto dizia "60+ equipes" desde quando o mundo tinha 68.

O caminho principal é correr a etapa **de verdade**: o Loop exporta o grid e o calendário como AI roster/AI season do iRacing, o jogador corre, e o resultado oficial volta para a carreira. A simulação interna preenche o que ele não corre. "Offline" vale para os **dados** (nada de servidor, tudo em SQLite local), não para o propósito — ver [docs/iracing-escopo.md](docs/iracing-escopo.md).

Alvo real é **Windows** — não por acaso: o SDK do iRacing e winapi são Windows-only, e fora do Windows a integração compila como stub inerte. O CI roda em `windows-latest`.

**O código, os comentários e a UI são em português.** Mantenha esse padrão ao escrever código novo.

Docs de referência: [docs/DESIGN.md](docs/DESIGN.md) (retrato completo do domínio), [docs/iracing-dados-disponiveis.md](docs/iracing-dados-disponiveis.md) (o que a telemetria entrega, o que não entrega e as armadilhas medidas), [docs/i18n-translation-spec.md](docs/i18n-translation-spec.md), [docs/divida-tecnica.md](docs/divida-tecnica.md).

## Comandos

```bash
npm run tauri dev          # app completo (sobe o vite e o shell Tauri)
npm run dev                # só o frontend em http://localhost:1420
npm run build              # vite build -> dist/
```

### Testes — três suítes independentes

```bash
npm run test:ui            # vitest (src/**/*.test.{js,jsx}) — jsdom
npm run test:structure     # guards estruturais/visuais (scripts/rodar-guards.mjs)
npm run test:all           # as duas de JS
cd src-tauri && cargo test         # PowerShell: cd src-tauri; cargo test
```

Um teste só:

```bash
npx vitest run src/pages/tabs/myteam/MyTeamTabV2.test.jsx
npx vitest run -t "nome do caso"
node --test scripts/tests/window-controls-sizing.test.mjs
cd src-tauri && cargo test nome_do_teste
```

**⚠️ `cargo build`/`cargo test` exigem `npm run build` antes.** `tauri::generate_context!` embute os assets de `dist/` em tempo de compilação — sem o build do frontend o crate Rust não compila.

**⚠️ Sempre de dentro de `src-tauri/`, nunca `cargo test --manifest-path src-tauri/Cargo.toml` da raiz.** O cargo procura o `.cargo/config.toml` a partir do diretório atual: da raiz ele não enxerga o do crate, ignora o `target-dir` configurado e abre um `src-tauri/target` novo, recompilando tudo. Ver a política de target-dir abaixo.

### Outros

```bash
npm run i18n:audit         # cobertura de i18n (o mesmo checker do pre-commit)
node scripts/release.mjs --bump patch --notes "..."   # bump + build assinado + manifesto + upload
```

Não há ESLint/Prettier configurados. O Rust segue `cargo fmt`/clippy com `too_many_arguments` e `type_complexity` desligados no topo de [lib.rs](src-tauri/src/lib.rs).

## Arquitetura

### Fronteira Rust ↔ React

Toda a simulação vive em Rust; o React só desenha e dispara `invoke`. O padrão de camadas no backend é consistente e deve ser seguido:

- **`commands/career/`** — a lógica de verdade, exposta como funções `*_in_base_dir(base_dir, ...)` que recebem o diretório de save explicitamente. São puras em relação ao Tauri, o que as torna testáveis sem `AppHandle`. O `career.rs` é só o índice; a lógica mora nos irmãos por área (`standings`, `queries`, `season_flow`, `market_window`, `lifecycle`, `interests`, `briefing`, `champion`, `save_state`, `vacancies`, `errors`, `debug`), com os testes em `career/tests/`.
- **`commands/career_commands.rs`** — casca fina `#[tauri::command]` que resolve o `base_dir` a partir do `AppHandle` e delega para as funções acima.
- **`commands/career_types.rs`** — os DTOs serde que cruzam a ponte.
- **`lib.rs`** — `invoke_handler(tauri::generate_handler![...])`. **Um comando novo só existe depois de ser registrado nessa lista.** O guard `scripts/tests/invoke-contra-generate-handler.test.mjs` cobra que todo `invoke("...")` do frontend exista nessa lista, e congela o inventário dos que ainda não têm consumidor — **ele é a contagem oficial**, e nenhum número escrito aqui é: qualquer total em prosa envelhece no primeiro comando novo.

### Domínio Rust (`src-tauri/src/`)

Módulos de domínio, cada um com seu `pipeline.rs` quando há um processo de várias etapas:

| Módulo | Papel |
|---|---|
| `simulation/` | motor de corrida: `qualifying` → `race` → `incidents`/`injuries` → `scoring`. `engine.rs` orquestra o fim de semana. |
| `market/` | mercado entre temporadas: propostas, renovação, assédio (`poaching`), IA das equipes, janela de transferências |
| `evolution/` | crescimento/declínio por idade, licenças, motivação, geração de rookies |
| `promotion/` | promoção e rebaixamento na escada fechada de categorias |
| `convocation/` | "bloco especial" sazonal (convocações fora do calendário regular) |
| `world/` | arquivamento de temporada e integridade do mundo |
| `iracing_sdk/` | leitura de telemetria/sessão do iRacing real (Windows) |
| `narrative/`, `news/` | geração de notícias determinística; `commands/ai_news.rs` faz o enriquecimento por IA via HTTP |
| `db/` | `connection`, `migrations`, `queries/*` (uma query por área de domínio) |

### Banco de dados

SQLite local, migrações versionadas em [db/migrations.rs](src-tauri/src/db/migrations.rs). O array `MIGRATIONS` é a **única** fonte de verdade da ordem — adicionar uma migração é **uma linha nesse array + bump do `CURRENT_VERSION`**. Nunca edite uma migração já lançada; crie a próxima.

### Frontend (`src/`)

- **Estado**: Zustand. [`useCareerStore.js`](src/stores/useCareerStore.js) é o hub, mas hoje é só a **composição** dos slices de `src/stores/career/` (`careerSlice`, `raceSlice`, `marketSlice`, `seasonSlice`, `blocoEspecialSlice`, `preRaceCacheSlice`) sobre o `initialState` de `career/state.js`. Todos recebem o mesmo par `(set, get)`, então compartilham um único estado e uma ação chama a de outro domínio via `get()`. Os `invoke` ficam nos slices — e, quando o dado é local de uma tela, em hooks `use*.js` dentro de `components/`. O outro store vivo é o `useAttentionStore`, trivial; `useUIStore` e `useNotificationStore` eram stubs vazios sem consumidor e foram removidos em 11/08/2026.
- **Navegação**: `pages/` são as telas (MainMenu, Dashboard, NewCareer, LoadSave, Settings) e `pages/tabs/` as abas dentro do Dashboard.
- **Componentes** em `components/` por domínio: `race`, `market` (dentro de tabs), `driver`, `season`, `standings`, `team`, `layout`, `ui`, `wizard`, `iracing`, `system`.
- Os `invoke` vêm direto de `@tauri-apps/api/core`, nos slices do store ou nos hooks de dados dos componentes. Não há camada de abstração da ponte: `src/hooks/useTauri.js` era um stub vazio e foi removido em 11/08/2026.

### Janelas

`tauri.conf.json` declara **três** webviews, todas servindo o mesmo `index.html`: a principal (sem decorações — os controles de janela são React + `commands/window.rs`), `overlay` e `engineer` (transparentes, always-on-top, para uso sobre o iRacing em corrida/VR).

## Convenções que quebram o build se ignoradas

### i18n é obrigatório e tem guard

Um hook de **pre-commit** ([.githooks/pre-commit](.githooks/pre-commit), ativado pelo `npm install`) bloqueia commits com strings de UI em português fora de `t()` nos arquivos `.jsx` **e `.js`** no stage. O mesmo checker roda em `src/i18n/i18nCoverage.test.js`.

O `.js` entrou em 11/08/2026, com varredura própria (`varreduraJs`): mover uma string de um `.jsx` para um helper `.js` deixou de ser um jeito silencioso de sair do radar. O passivo que já existia está congelado em [scripts/i18nBaseline.mjs](scripts/i18nBaseline.mjs), arquivo por arquivo e frase por frase. Regras dele: entrada nova nunca se acrescenta ao baseline para liberar commit, e entrada que o auditor não encontra mais (string traduzida, arquivo apagado, texto reescrito) faz o auditor falhar pedindo a remoção da linha, para o baseline não apodrecer.

Exceções intencionais:
- `{/* i18n-ignore */}` na linha ou na linha acima
- `// i18n-ignore-file` em qualquer ponto do arquivo
- `git commit --no-verify` para pular pontualmente

Frontend: i18next, um namespace por área (`src/i18n/locales/<lang>/common.json`); pt-BR é o locale-base, en-US o par. `localeParity.test.js` garante que as chaves dos dois batem.

Backend: `rust-i18n` lendo `src-tauri/locales/*.yml`. **O locale é global do processo** — testes Rust que trocam de idioma precisam de `#[serial]` (crate `serial_test`), senão contaminam testes que asseveram prosa em PT.

### Categoria → carro do iRacing tem fonte única, e ela recusa

[commands/iracing/exportavel.rs](src-tauri/src/commands/iracing/exportavel.rs) é o único lugar que decide o que o export sabe fazer com uma categoria. Roster, temporada e pintura recebem a **categoria**, nunca uma `car_key`: quem traduz é o Rust, uma vez, ali. O frontend não adivinha carro.

O `match` é por identidade da categoria, nunca por substring, e o braço final **recusa com motivo** em vez de escolher um padrão. Hoje exportam `mazda_rookie`, `toyota_rookie`, `mazda_amador`, `toyota_amador` e `bmw_m2`; `gt4`, `gt3` e `lmp2` são recusadas por carro não decidido, e `production_challenger` e `endurance` por serem grid de mais de uma classe. Até 11/08/2026 todas elas caíam num `else → mx5` e o jogador exportava um grid de MX-5 sem que nada acusasse.

A duração segue a mesma regra: `race_length_da_temporada` reduz as durações **efetivas** das etapas ([calendar::duracao_efetiva](src-tauri/src/calendar/entry.rs)) ao único `race_length` que o aiseason aceita, e recusa quando elas divergem. A sentinela `0` de `duracao_corrida_min` morre na cascata e não chega ao arquivo.

O teste `o_catalogo_inteiro_e_mapeado_ou_recusado_explicitamente` percorre `constants::categories` inteiro: categoria nova entra por ali, ou ganhando carro, ou com o motivo da recusa escrito.

### CSP definida, e o corpo da caixa de entrada não é HTML

`tauri.conf.json` traz uma CSP explícita (era `null` até 11/08/2026). Recurso externo novo na webview (fonte, imagem, endpoint) exige mexer nela, e o guard [scripts/tests/csp-e-sink-html.test.mjs](scripts/tests/csp-e-sink-html.test.mjs) cobra as diretivas que não podem sumir, entre elas o `connect-src` com `ipc:` e `http://ipc.localhost`, sem os quais o `invoke` para e o app abre morto. Em `npm run tauri dev` a política não é aplicada, porque o HTML vem do Vite: ela só vale no bundle.

O mesmo guard proíbe `dangerouslySetInnerHTML` alimentado por string montada com dado do banco. HTTP do lado Rust (updater, proxy de notícias) fica fora do alcance da CSP.

### Falha engolida tem rastro

`.catch(() => {})` cru é proibido em `src/components/race/` e `src/stores/` ([guard](scripts/tests/catch-vazio-no-caminho-de-corrida.test.mjs)). No lugar dele, `bestEffort(promessa, rotulo)` de [src/utils/bestEffort.js](src/utils/bestEffort.js): engole igual para a UI e escreve uma linha no `loop.log` pelo comando `diagnostico_registrar`. O app é uma GUI sem console na máquina do jogador, então falha sem rastro chega ao suporte como "não funciona". Fora desse alcance (overlay a 60 Hz, Web Audio) o padrão continua válido, por decisão escrita no próprio guard.

### Versão tem fonte única

`package.json` é a fonte; `tauri.conf.json` e `Cargo.toml` espelham. O `vite.config.js` injeta `__APP_VERSION__` (do package.json) e `__APP_BUILD__` (contagem de commits do git). Use `scripts/release.mjs` para bumpar — ele sincroniza os três, assina e publica.

### Target-dir do cargo: uma política só

`src-tauri/.cargo/config.toml` traz `target-dir = "C:/cargo-target/iracer"`, que joga os artefatos para fora do OneDrive. Esse arquivo é **específico da máquina** — não commite mudanças nele achando que é config geral.

A regra vale para desenvolvimento, release e CI, e está implementada em [scripts/lib/cargo-target.mjs](scripts/lib/cargo-target.mjs):

1. `CARGO_TARGET_DIR` explícito no ambiente vence. É o que o CI usa, para o cache do Swatinem achar o target dentro do workspace.
2. Sem ele, vale o que o cargo resolve **de dentro de `src-tauri/`**, ou seja o `.cargo/config.toml` acima. Nenhum caminho de máquina fica escrito em script.

O `release.mjs` pergunta o caminho ao `cargo metadata` em vez de cravar um. Ele já cravou `C:/dev/loop-target`, e isso fazia o release recompilar do zero o que o desenvolvimento já tinha compilado.

Quando aparecer um `src-tauri/target` no repositório, alguém rodou o cargo da raiz com `--manifest-path`. Ele não é usado por nada e pode ser apagado à mão — em 11/08/2026 esse diretório acidental tinha **91,5 GB**. O `.gitignore` o cobre, então ele não vaza para o commit, só para o disco.

## Testes estruturais (`scripts/tests/`)

Além dos testes de comportamento, há uma suíte `node --test` que faz guards de **estrutura e consistência visual** lendo o código-fonte como texto: alinhamento de layout, paleta de cores de equipe, contratos dos controles de janela, acentuação de copy em português, sanidade de encoding. Ao mexer em layout ou em paleta, espere que essa suíte reclame — ela existe para pegar regressão visual sem screenshot.

Quem descobre os guards é [scripts/rodar-guards.mjs](scripts/rodar-guards.mjs), por `readdir` e com a lista explícita indo para o `node --test`. Nada de glob: `node --test "scripts/tests/*.test.mjs"` depende de quem expande o asterisco (o `cmd.exe` do `npm run` no Windows não expande, e o resolvedor do runner só existe do Node 21 em diante) e, quando não casa com nada, sai **verde com zero testes**. O runner também guarda um `PISO` com a contagem de guards; apagar guard exige baixá-lo no mesmo commit.

O repositório exige **Node >= 24** (`engines` do package.json), e o CI usa a mesma maior. O `package-lock.json` é escrito pelo npm 11 — com o npm 10 do Node 20 o `npm ci` recusa o lock.
