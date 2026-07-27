---
name: guard-visual
description: Navega a suíte de guards estruturais do Loop (scripts/tests/*.test.mjs) — testes que leem o código-fonte como texto para pegar regressão visual sem screenshot. Use SEMPRE que npm run test:structure falhar com mensagem críptica sobre regex, bloco ou contagem, e ANTES de mexer em layout, espaçamento, paleta de cores de equipe, controles de janela, copy acentuada em português ou encoding de arquivo. Gatilhos típicos: "alinha esse painel", "muda a cor da equipe", "ajusta o header", "os botões de janela sumiram", "test:structure quebrou", "por que esse teste .mjs tá reclamando". Use também ao adicionar equipe ou categoria nova, porque a distribuição da paleta é verificada.
---

# Guards estruturais do Loop

`scripts/tests/*.test.mjs` (rodados por `npm run test:structure`) não executam o
app. Eles **leem o código-fonte como texto** — regex sobre JSX e sobre `.rs` — e
asseveram propriedades de estrutura e consistência visual.

Isso é deliberado: pega regressão de layout e de paleta sem screenshot, sem
snapshot binário e sem navegador. O preço é que a mensagem de falha aponta para
um regex que não casou, não para "o painel ficou torto". Entender o guard é o
caminho mais curto para resolver.

## O catálogo

| Arquivo | O que protege |
|---|---|
| `app-placeholder-visual-alignment` | alinhamento dos placeholders de tela |
| `dashboard-visual-alignment` | alinhamento e estrutura do Dashboard |
| `wizard-panel-color-balance` | equilíbrio de cor nos painéis do wizard |
| `team-palette-distribution` | distribuição de cores de equipe por faixa de categoria |
| `result-badge-fastest-lap` | badge de volta mais rápida no resultado |
| `season-champion-disabled` | estado desabilitado do campeão da temporada |
| `driver-detail-modal` | estrutura do modal de detalhe do piloto |
| `window-controls-contract` | contrato dos controles de janela (React ↔ `commands/window.rs`) |
| `window-controls-sizing` | dimensões dos controles de janela |
| `window-controls-hover-zone` | zona de hover dos controles |
| `window-controls-navigation` | navegação pelos controles |
| `window-fullscreen-config` | configuração de fullscreen |
| `portuguese-copy-accents` | acentuação correta na copy em português |
| `text-encoding-sanity` | sanidade de encoding dos arquivos |
| `career-command-structure` | camadas dos comandos de carreira |
| `career-commands-structure` | idem, do lado da casca `#[tauri::command]` |
| `career-detail-helpers-structure` | organização dos helpers de detalhe |

## Como resolver uma falha

**1. Abra o teste antes de mexer no código.** O guard é curto e diz explicitamente
o que espera — normalmente uma constante no topo (lista de categorias, valores de
px, nomes de classe) e um regex que extrai blocos do fonte. Ler os 40 primeiros
caminhos é mais rápido que adivinhar pela mensagem.

**2. Descubra qual arquivo ele lê.** Vários guards apontam para um arquivo
específico e explicam por quê. Exemplo real, em `team-palette-distribution`:

```js
// Os templates moram em `constants/teams/dados.rs` desde que `teams.rs` virou
// fachada; ler a fachada faria os testes passarem sem inspecionar nada.
const TEAM_DATA_FILE = "src-tauri/src/constants/teams/dados.rs";
```

Se você moveu ou renomeou esse arquivo, o guard não está errado — está
desatualizado, e atualizar o caminho é parte da sua mudança.

**3. Decida honestamente entre corrigir o código e atualizar o guard.**

- Regressão de verdade (alinhamento quebrou, contraste sumiu, controle de janela
  mudou de tamanho sem intenção) → **corrija o código.** É exatamente o caso para
  o qual o guard existe.
- Mudança intencional de design que o guard ainda descreve pelo estado antigo →
  **atualize o guard**, e explique no comentário por que o novo valor é o certo.
  O comentário é o que impede a próxima pessoa de reverter.

Afrouxar o regex até passar é a única saída errada: o guard continua verde e para
de proteger.

## Ao mexer em paleta ou categoria

Adicionar equipe ou categoria mexe na distribuição verificada por
`team-palette-distribution`, que separa categorias de base e de topo em listas
próprias. Categoria nova provavelmente precisa entrar numa dessas listas —
se não entrar, ela simplesmente não é verificada, o que passa despercebido.

## Fechamento

```bash
npm run test:structure
```

Um arquivo só, quando estiver iterando:

```bash
node --test scripts/tests/team-palette-distribution.test.mjs
```

Mudanças de layout costumam disparar mais de um guard — rode a suíte inteira
antes de considerar terminado, não só o que estava falhando.
