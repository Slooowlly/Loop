# Backlog — Loop

Lista única do que fazer no app. Levantada em 2026-07-27 varrendo o código contra
[DESIGN.md](DESIGN.md) §23 e as telas realmente montadas no `Dashboard`.

**O porquê de cada item está em [roadmap.md](roadmap.md)** — aqui fica a lista com ids
e status; lá fica o raciocínio. Itens marcados ⚠️ foram revisados depois da segunda
varredura, mais funda, que corrigiu erros desta primeira.

**Como usar:** cada item tem um id estável (`F-xx`, `D-xx`, `P-xx`), um tamanho
(P / M / G) e uma linha de por quê. Item feito sai daqui e, se for dívida técnica,
vira registro em [divida-tecnica.md](divida-tecnica.md). Item novo entra no fim da
seção — não renumere os antigos.

**Estado hoje:** 158 comandos Tauri registrados, 131 chamados pelo frontend.
7 abas ativas no Dashboard (`next-race`, `standings`, `global-drivers`,
`global-teams`, `news`, `my-team`, `calendar`).

---

## Features — o que falta pro jogo ficar inteiro

O padrão dominante: **o backend está pronto e não tem tela.** Quase todo item aqui é
trabalho de frontend em cima de simulação que já roda.

| id | item | tam | por quê |
|---|---|---|---|
| F-01 | **Mercado durante a temporada** | M | ⚠️ **Revisado** — a redação original ("o mercado não tem tela") estava errada: existem ~2700 linhas de UI de mercado em `PreSeasonView` + `season/preseason/` + `PoachAuctionHost`. O buraco real é o mercado **fora da janela de pré-temporada**: não há onde consultar estado de contrato, quem está de olho em você e vagas abertas no meio do ano. Inclui conduzir `advance_transfer_window`, registrado e nunca chamado. Ver [roadmap.md](roadmap.md) §3. |
| F-02 | **Tela do meu piloto** (`MyProfileTab`) | M | Atributos, evolução por idade, motivação, licença, lesões — tudo simulado, nada exibido. O jogador controla um piloto e não tem ficha dele. |
| F-03 | **Arquivo histórico / temporadas passadas** | M | `world/` arquiva a temporada e `db/queries/race_history/` já responde recordes, títulos e vitórias por pista. `Archive.jsx` e `SeasonsHistory.jsx` são placeholders sem rota. |
| F-04 | **Sala de troféus** (`TrophyRoom`) | P | Consome o mesmo `race_history` do F-03. Barato depois que F-03 existir. |
| F-05 | **Rivalidades — visão consolidada** | P | ⚠️ **Revisado** — rivalidade já aparece em 9 componentes (marcação no calendário, detalhe do piloto, análise de corrida, `RivalryPerceptionPanel`). Falta só a sala própria: quem são meus rivais, desde quando, qual o placar. Barato se entrar junto de F-03. |
| F-06 | **Backup e restauração de save** | P | `create_season_backup`, `list_backups`, `restore_backup` registrados e **nunca chamados**. É segurança de dados do jogador por um punhado de linhas de UI. |
| F-07 | **UI de espectadores / interesse de evento** | M | Backend completo (§17.1 do DESIGN); a UI é básica. Pendência já reconhecida no doc. |
| F-08 | **Outras categorias** | ? | ⚠️ **Provavelmente já resolvido.** `GlobalDriversTab` e `GlobalTeamsTab` já atravessam as 9 categorias. Antes de agendar, responder: o que essa aba mostraria que as globais não mostram? Se for "a classificação da categoria vizinha", é um filtro no `StandingsTab`, não uma aba. |
| F-09 | **Previsões / palpites** (`PredictionTab`) | M | Placeholder. Só vale depois de F-01 e F-02 — é camada de sabor, não de fundação. |
| F-10 | **Integração real com iRacing** | G | `AppConfig` guarda o caminho, mas `export/` e `commands/export.rs` foram removidos. Expansão futura assumida — decidir se entra ou se sai do escopo de vez. |

**Ordem revisada** (o raciocínio completo está em [roadmap.md](roadmap.md)):
F-06 → F-03+F-04+F-05 juntos → F-02 → F-01 → F-07.
F-06 primeiro porque é o único item cuja ausência destrói dados do jogador.
F-03/F-04/F-05 numa aba de História só: os três comem da mesma `race_history`.

---

## Dívida técnica

| id | item | tam | por quê |
|---|---|---|---|
| D-01 | **Código legado de convocação** | M | `convocation/`, `simulate_special_block` e as fases `BlocoRegular`/`JanelaConvocacao`/`BlocoEspecial`/`PosEspecial` só existem por saves antigos. Confirmar que nenhum save ativo usa e remover. |
| D-02 | **Tabela `races` legada** | M | Coexiste com `calendar` fazendo o papel de corridas. Duas fontes de verdade pro mesmo conceito. |
| D-03 | **Stores stub** | P | `useUIStore.js` e `useNotificationStore.js` são `// TODO: Implementar`. Ou implementa ou apaga. |
| D-04 | **`hooks/useTauri.js` morto** | P | `// TODO: Implementar`; os `invoke` vêm direto de `@tauri-apps/api/core`. Apagar o arquivo e a intenção junto. |
| D-05 | **Comandos registrados sem consumidor** | P | Fora dos casos de F-06 e das macros de iRacing, sobram `advance_transfer_window`, `get_driver`, `get_race_results_by_category` e `create_career` sem nenhuma chamada no frontend. Auditar: cada um é feature faltando (vira `F-xx`) ou código morto (some). |
| D-06 | **`constants/tracks/consultas.rs`** | P | `TODO(design final)`: trocar a consulta por "pistas que o jogador realmente possui". Decisão de design pendente, não bugfix. |
| D-07 | **Tracks ausentes do DB** | P | `calendar/generator.rs` lista venues marcados `// TODO` que ainda não existem no banco. |
| D-08 | **`models/driver.rs`** | P | `TODO(migration)` afirmando que o schema já cobre os campos do Módulo 10 — verificar e apagar o comentário, ou agir. |
| D-09 | **Varredura de acoplamento — lado Rust (R1–R5)** | G | Cinco briefings autocontidos em [varredura-acoplamento/](varredura-acoplamento/): `narrative/` cego com a Etapa B nunca ligada (R1), três motores de tese concorrentes (R2), `public_presence` vs `market/visibility` duplicando tiers (R3), `hierarchy/` com estado rico e sem consumidor (R4), caminhos paralelos vivos só pelos testes (R5). **R1 e R2 tocam os mesmos arquivos — não rodar em paralelo.** Cada briefing pede uma segunda análise antes de virar código: a varredura original foi rasa por design e conta com falsos positivos. |

O lado frontend da varredura (**F1–F4** — helpers de pista/clima, `formatLap` e paleta de gráficos,
`getReadableTeamColor`, `IN_TAURI`) foi resolvido no commit `2c85f44`.

---

## Pontas soltas

| id | item | tam | por quê |
|---|---|---|---|
| ~~P-01~~ | ~~9 arquivos de tela órfãos~~ | — | **Feito em 2026-07-27.** Removidos os 9 placeholders (`MarketTab`, `DriversTab`, `MyProfileTab`, `PredictionTab`, `OtherCategoriesTab`, `TrophyRoom`, `Archive`, `Rivalries`, `SeasonsHistory`), mais o componente `AppPlaceholder` e o guard `app-placeholder-visual-alignment.test.mjs` que só existiam para eles. `src/pages/history/` deixou de existir. As telas voltam pelo caminho dos `F-xx`, escritas de verdade. |
| ~~P-02~~ | ~~Sem rota `/history`~~ | — | **Reclassificado.** Não é ponta solta: sem tela de história para rotear, a rota não tem o que apontar. Vira parte do escopo de F-03/F-04/F-05. |
| ~~P-03~~ | ~~Árvore de trabalho suja~~ | — | **Feito em 2026-07-27** (commit `2c85f44`): 161 arquivos, cinco frentes, com as três suítes verdes antes de commitar. |
| P-04 | **Chaves i18n órfãs** | P | Removidas as telas do P-01, as chaves `marketTab.*`, `driversTab.*` e afins ficaram sem consumidor em `pt-BR/common.json` e `en-US/common.json`. Deixadas de propósito — voltam a ser usadas nos `F-xx`. Se algum `F-xx` for cancelado, limpar as dele. |

---

## Fora do backlog de propósito

- `divida-tecnica.md` segue como **registro do que foi resolvido** (DB-001..004) — não duplicar aqui.
- Os cuidados de manutenção do §23 do DESIGN (semântica de categoria especial, `glass-strong`,
  `BEGIN IMMEDIATE`, `season_week = week_of_year + 4`) são regras permanentes, não tarefas.
