# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## O que é

**Loop** — jogo desktop de carreira no automobilismo **construído em volta do iRacing**. Tauri v2 (Rust) + React 18 + Vite, com SQLite local. O jogador controla **um piloto** subindo uma pirâmide de 9 categorias, cercado por um mundo vivo de 200+ pilotos de IA e 60+ equipes que correm, evoluem, trocam de equipe e se aposentam sozinhos.

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
npm run test:structure     # node --test scripts/tests/*.test.mjs — guards estruturais/visuais
npm run test:all           # as duas de JS
cargo test --manifest-path src-tauri/Cargo.toml
```

Um teste só:

```bash
npx vitest run src/pages/tabs/MyTeamTab.test.jsx
npx vitest run -t "nome do caso"
node --test scripts/tests/window-controls-sizing.test.mjs
cargo test --manifest-path src-tauri/Cargo.toml nome_do_teste
```

**⚠️ `cargo build`/`cargo test` exigem `npm run build` antes.** `tauri::generate_context!` embute os assets de `dist/` em tempo de compilação — sem o build do frontend o crate Rust não compila.

### Outros

```bash
npm run i18n:audit         # cobertura de i18n (o mesmo checker do pre-commit)
node scripts/release.mjs --bump patch --notes "..."   # bump + build assinado + manifesto + upload
```

Não há ESLint/Prettier configurados. O Rust segue `cargo fmt`/clippy com `too_many_arguments` e `type_complexity` desligados no topo de [lib.rs](src-tauri/src/lib.rs).

## Arquitetura

### Fronteira Rust ↔ React

Toda a simulação vive em Rust; o React só desenha e dispara `invoke`. O padrão de camadas no backend é consistente e deve ser seguido:

- **`commands/career/`** — a lógica de verdade, exposta como funções `*_in_base_dir(base_dir, ...)` que recebem o diretório de save explicitamente. São puras em relação ao Tauri, o que as torna testáveis sem `AppHandle`. O `career.rs` é só o índice; a lógica mora nos irmãos por área (`standings`, `queries`, `season_flow`, `market_window`, `lifecycle`, `interests`, `briefing`, `save_state`, `vacancies`, `debug`), com os testes em `career/tests/`.
- **`commands/career_commands.rs`** — casca fina `#[tauri::command]` que resolve o `base_dir` a partir do `AppHandle` e delega para as funções acima.
- **`commands/career_types.rs`** — os DTOs serde que cruzam a ponte.
- **`lib.rs`** — `invoke_handler(tauri::generate_handler![...])`. **Um comando novo só existe depois de ser registrado nessa lista** (201 entradas em 11/08/2026). O guard `scripts/tests/invoke-contra-generate-handler.test.mjs` cobra que todo `invoke("...")` do frontend exista nessa lista, e congela o inventário dos que ainda não têm consumidor.

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

- **Estado**: Zustand. [`useCareerStore.js`](src/stores/useCareerStore.js) é o hub, mas hoje é só a **composição** dos slices de `src/stores/career/` (`careerSlice`, `raceSlice`, `marketSlice`, `seasonSlice`, `preRaceCacheSlice`) sobre o `initialState` de `career/state.js`. Todos recebem o mesmo par `(set, get)`, então compartilham um único estado e uma ação chama a de outro domínio via `get()`. Os `invoke` ficam nos slices — e, quando o dado é local de uma tela, em hooks `use*.js` dentro de `components/`. O outro store vivo é o `useAttentionStore`, trivial; `useUIStore` e `useNotificationStore` eram stubs vazios sem consumidor e foram removidos em 11/08/2026.
- **Navegação**: `pages/` são as telas (MainMenu, Dashboard, NewCareer, Settings) e `pages/tabs/` as abas dentro do Dashboard.
- **Componentes** em `components/` por domínio: `race`, `market` (dentro de tabs), `driver`, `season`, `standings`, `team`, `layout`, `ui`, `wizard`, `iracing`, `system`.
- Os `invoke` vêm direto de `@tauri-apps/api/core`, nos slices do store ou nos hooks de dados dos componentes. Não há camada de abstração da ponte: `src/hooks/useTauri.js` era um stub vazio e foi removido em 11/08/2026.

### Janelas

`tauri.conf.json` declara **três** webviews, todas servindo o mesmo `index.html`: a principal (sem decorações — os controles de janela são React + `commands/window.rs`), `overlay` e `engineer` (transparentes, always-on-top, para uso sobre o iRacing em corrida/VR).

## Convenções que quebram o build se ignoradas

### i18n é obrigatório e tem guard

Um hook de **pre-commit** ([.githooks/pre-commit](.githooks/pre-commit), ativado pelo `npm install`) bloqueia commits com strings de UI em português fora de `t()` em arquivos `.jsx` no stage. O mesmo checker roda em `src/i18n/i18nCoverage.test.js`.

Exceções intencionais:
- `{/* i18n-ignore */}` na linha ou na linha acima
- `// i18n-ignore-file` em qualquer ponto do arquivo
- `git commit --no-verify` para pular pontualmente

Frontend: i18next, um namespace por área (`src/i18n/locales/<lang>/common.json`); pt-BR é o locale-base, en-US o par. `localeParity.test.js` garante que as chaves dos dois batem.

Backend: `rust-i18n` lendo `src-tauri/locales/*.yml`. **O locale é global do processo** — testes Rust que trocam de idioma precisam de `#[serial]` (crate `serial_test`), senão contaminam testes que asseveram prosa em PT.

### Versão tem fonte única

`package.json` é a fonte; `tauri.conf.json` e `Cargo.toml` espelham. O `vite.config.js` injeta `__APP_VERSION__` (do package.json) e `__APP_BUILD__` (contagem de commits do git). Use `scripts/release.mjs` para bumpar — ele sincroniza os três, assina e publica.

### `.cargo/config.toml` é específico da máquina

`target-dir = "C:/cargo-target/iracer"` — aponta o target para fora do OneDrive. O CI sobrescreve com `CARGO_TARGET_DIR`. Não commite mudanças nesse arquivo achando que é config geral.

## Testes estruturais (`scripts/tests/`)

Além dos testes de comportamento, há uma suíte `node --test` que faz guards de **estrutura e consistência visual** lendo o código-fonte como texto: alinhamento de layout, paleta de cores de equipe, contratos dos controles de janela, acentuação de copy em português, sanidade de encoding. Ao mexer em layout ou em paleta, espere que essa suíte reclame — ela existe para pegar regressão visual sem screenshot.
