# Backlog — Loop

Lista única do que fazer no app. Levantada em 2026-07-27 varrendo o código contra
[DESIGN.md](DESIGN.md) §27 (era §23 no retrato de junho) e as telas realmente montadas no `Dashboard`.

**O porquê de cada item está em [roadmap.md](roadmap.md)** — aqui fica a lista com ids
e status; lá fica o raciocínio. Itens marcados ⚠️ foram revisados depois da segunda
varredura, mais funda, que corrigiu erros desta primeira.

**Como usar:** cada item tem um id estável (`F-xx`, `D-xx`, `P-xx`), um tamanho
(P / M / G) e uma linha de por quê. Item feito sai daqui e, se for dívida técnica,
vira registro em [divida-tecnica.md](divida-tecnica.md). Item novo entra no fim da
seção — não renumere os antigos.

**Estado hoje (recontado em 11/08/2026, depois da remoção dos 3 comandos mortos do D-05):**
198 comandos Tauri registrados, 177 chamados pelo frontend, 21 sem nenhum consumidor.
O número deixou de ser contagem manual: o guard
[`scripts/tests/invoke-contra-generate-handler.test.mjs`](../scripts/tests/invoke-contra-generate-handler.test.mjs)
cruza os `invoke("...")` de `src/` com o `generate_handler!` de `lib.rs`, cobra que todo
invoke exista do outro lado e congela a lista dos órfãos com o motivo de cada um em
`SEM_CONSUMIDOR_CONHECIDO`. **A lista do guard é o número oficial**, e não esta linha:
qualquer contagem escrita aqui envelhece no primeiro comando novo.

O Dashboard tem 4 abas na barra (`standings`, `news`, `my-team`, `calendar`) mais 4 vistas
alcançadas por navegação interna (`next-race`, `global-drivers`, `global-teams`,
`team-records`).

---

## Features — o que falta pro jogo ficar inteiro

O padrão dominante: **o backend está pronto e não tem tela.** Quase todo item aqui é
trabalho de frontend em cima de simulação que já roda.

| id | item | tam | por quê |
|---|---|---|---|
| F-01 | **Mercado durante a temporada** | M | ⚠️ **Revisado** — a redação original ("o mercado não tem tela") estava errada: existem ~2700 linhas de UI de mercado em `PreSeasonView` + `season/preseason/` + `PoachAuctionHost`. O buraco real é o mercado **fora da janela de pré-temporada**: não há onde consultar estado de contrato, quem está de olho em você e vagas abertas no meio do ano. Inclui conduzir `advance_transfer_window`, registrado e nunca chamado. Ver [roadmap.md](roadmap.md) §3. |
| F-02 | **Tela do meu piloto** | M | Atributos, evolução por idade, motivação, licença e lesão: tudo simulado e nada exibido como ficha própria. O jogador se enxerga pelo `DriverDetailModal`, que é a mesma lente usada para qualquer piloto do grid. ⚠️ O placeholder `MyProfileTab.jsx` foi **removido** em 27/07/2026 (ver P-01): a tela nasce do zero. |
| F-03 | **Arquivo histórico / temporadas passadas** | M | `world/` arquiva a temporada e `db/queries/race_history/` já responde recordes, títulos e vitórias por pista. ⚠️ Os placeholders `Archive.jsx` e `SeasonsHistory.jsx` foram **removidos** em 27/07/2026 junto com `src/pages/history/` (ver P-01). Não há nenhuma tela de história montada hoje. |
| F-04 | **Sala de troféus** | P | Consome o mesmo `race_history` do F-03, e fica barato depois que F-03 existir. ⚠️ O placeholder `TrophyRoom.jsx` foi **removido** em 27/07/2026 (ver P-01). |
| F-05 | **Rivalidades — visão consolidada** | P | ⚠️ **Revisado** — rivalidade já aparece em 9 componentes (marcação no calendário, detalhe do piloto, análise de corrida, `RivalryPerceptionPanel`). Falta só a sala própria: quem são meus rivais, desde quando, qual o placar. Barato se entrar junto de F-03. |
| ~~F-06~~ | **Backup e restauração de save** | P | **FEITO.** Conferido em 11/08/2026: `src/components/ui/BackupsModal.jsx` consome `list_backups`, `create_season_backup` e `restore_backup`, e é aberto por `src/pages/LoadSave.jsx`. O fechamento não tinha sido registrado aqui. |
| F-07 | **UI de espectadores / interesse de evento** | M | Backend completo (§17.1 do DESIGN); a UI é básica. Pendência já reconhecida no doc. |
| F-08 | **Outras categorias** | ? | ⚠️ **Provavelmente já resolvido.** `GlobalDriversTab` e `GlobalTeamsTab` já atravessam as 9 categorias. Antes de agendar, responder: o que essa aba mostraria que as globais não mostram? Se for "a classificação da categoria vizinha", é um filtro no `StandingsTab`, não uma aba. |
| F-09 | **Previsões / palpites** | M | Camada de sabor, e só vale depois de F-01 e F-02. ⚠️ O placeholder `PredictionTab.jsx` foi **removido** em 27/07/2026 (ver P-01). |
| ~~F-10~~ | **Integração real com iRacing** | G | **DECIDIDO em 27/07/2026.** A redação original estava errada em dois pontos: `export/` e `commands/export.rs` foram deletados, e a exportação **mudou de casa** para `iracing_sdk/roster_gen.rs` e `season_gen.rs`. O Loop é uma ferramenta de iRacing com uma carreira simulada dentro, e correr de verdade é o caminho principal. Levantamento completo em [iracing-escopo.md](iracing-escopo.md), retrato em [DESIGN.md](DESIGN.md) §19. O backlog derivado está na §6 do `iracing-escopo.md`. |

**Ordem revisada** (o raciocínio completo está em [roadmap.md](roadmap.md)):
F-03+F-04+F-05 juntos → F-02 → F-01 → F-07.
F-03, F-04 e F-05 numa aba de História só: os três comem da mesma `race_history`.
F-06 saiu da fila em 11/08/2026, já resolvido.

---

## Dívida técnica

| id | item | tam | por quê |
|---|---|---|---|
| D-01 | **Fases legadas da convocação** | M | ⚠️ **Reenquadrado em 11/08/2026 — o texto anterior estava errado.** `convocation/` NÃO é legado: os 10 comandos do bloco especial estão registrados e o `seasonSlice.js` chama todos. O que é legado são as QUATRO FASES `BlocoRegular`/`JanelaConvocacao`/`BlocoEspecial`/`PosEspecial`, e o próprio código já diz isso em `models/enums/temporada.rs` (`is_legacy()`). No modelo 9D a temporada vai `PreTemporada → Temporada → Encerramento`, e `advance_to_convocation_window` exige `BlocoRegular`, que o fluxo 9D nunca grava — `tests_9d.rs::assert_no_legacy_phases` cobra exatamente isso em 4 pontos. **Não removido, e o motivo é o que falta apurar:** a migração da virada 9D foi colapsada na baseline v53, então não dá para provar pelo código de hoje se ela reescreveu o `fase` dos saves em voo. Um save v53+ que atravessou a virada pode carregar fase legada, e a rampa se auto-cura numa virada de temporada. Decidir exige saber se ainda existe save assim. |
| D-02 | **Tabela `races` órfã** | M | ⚠️ **Medido em 11/08/2026: não são duas fontes de verdade — `calendar` é a única, e `races` está VAZIA.** `race_results.race_id` tem `FOREIGN KEY REFERENCES calendar(id)`, e a produção grava `race_entry.id` (`commands/race/persistencia.rs:193`). Os únicos `INSERT INTO races` do repositório são fixture de teste (`db/queries/races.rs`, `world/integrity.rs`, `world/team_archive.rs`), e o único `SELECT` é um JOIN em teste. Nenhum código de produção escreve nem lê a tabela. **Eliminar exige `DROP TABLE races` numa migração v64**, que apaga linhas de saves antigos que ninguém consegue provar que não existem — decisão do dono, não improviso de limpeza. Nota de nome: `db/queries/races.rs` não fala com a tabela `races`; ele guarda as queries de `race_results`, `race_safety_cars` e `race_weekend_readings`, todas chaveadas por `calendar.id`. |
| ~~D-03~~ | **Stores stub** | P | RESOLVIDO em 11/08/2026: `useUIStore.js` e `useNotificationStore.js` eram Zustand vazios sem nenhum consumidor. Removidos. |
| ~~D-04~~ | **`hooks/useTauri.js` morto** | P | RESOLVIDO em 11/08/2026: arquivo removido. Os `invoke` seguem vindo direto de `@tauri-apps/api/core`, por decisão. |
| D-05 | **Comandos registrados sem consumidor** | P | ⚠️ **Auditado em 11/08/2026, reconferido no mesmo dia, e o inventário virou guard.** A primeira varredura falou em 4; a auditoria achou 24; a segunda passada removeu 3 e corrigiu 2 rótulos, fechando em **21**. A lista com o motivo de cada um vive em `SEM_CONSUMIDOR_CONHECIDO` no guard de invoke — mudou, o teste quebra. **Removidos (registro + implementação):** `toggle_maximize_window` e `get_window_maximized` (os controles de janela trabalham com TELA CHEIA — `WindowControlsDrawer.jsx` só chama minimize, start_drag, toggle_fullscreen, get_fullscreen e close) e `get_driver` (assinatura antiga `career_number: u32`; quem lê um piloto é `get_driver_detail`). **Rótulos corrigidos:** `get_race_reading` e `ptt_gatilho_atual` NÃO são API interna — `get_race_reading_in_base_dir` só é chamado pela própria casca, e nada em Rust ou JS chama `ptt_gatilho_atual` fora dos testes de `ptt.rs`. Os 21 que ficam se dividem em: **API interna** (só `iracing_process_race_result`, em `iracing/importacao.rs:138`); **feature futura** (7); **diagnóstico de bancada** (11); **reservado** (`ptt_gatilho_atual`, o lado de leitura do estático de `ptt.rs`); e **consumidor removido acidentalmente** (`overlay_window_set_interactive` — o doc-comment cita um botão "Mover" que não existe mais, e sem ele o overlay fica preso em click-through). |
| D-06 | **`constants/tracks/consultas.rs`** | P | `TODO(design final)`: trocar a consulta por "pistas que o jogador realmente possui". Decisão de design pendente, não bugfix. |
| ~~D-07~~ | **Tracks ausentes do DB** | P | **RESOLVIDO em 11/08/2026 — era comentário velho, não bug de dados.** Cruzando todo id de pool de `calendar/generator.rs` com as 107 pistas de `constants::tracks::dados`, ZERO ficam fora do catálogo, e não sobrou nenhuma marcação `// TODO` de pista. O cabeçalho ainda mandava "filtrar com `get_track(id).is_some()` ao wiring no gerador real": o gerador já está ligado (`geracao.rs` chama `resolve_thematic_pool` nas duas rotas) e o filtro já está aplicado (linhas 471, 502, 528). Só o comentário foi corrigido. |
| ~~D-08~~ | **`models/driver.rs`** | P | **RESOLVIDO em 11/08/2026 — a afirmação era verdadeira, faltava a prova.** Os 19 campos de `DriverAttributes` têm coluna em `drivers`. O `TODO(migration)` saiu e no lugar ficou o ponteiro para `db::queries::drivers::tests::todo_atributo_do_piloto_tem_coluna_e_volta_igual_do_banco`, que grava um piloto com valor distinto por atributo e relê contra o schema real das migrações (não contra o DDL de bancada do `setup_test_db`). Campo novo sem coluna derruba esse teste. |
| D-09 | **Varredura de acoplamento — lado Rust (R1–R5)** | P | ⚠️ **Revisado em 11/08/2026, briefing por briefing, na mesma sessão.** A dívida TÉCNICA fechou: **R3 e R5 resolvidos**, **R2 resolvido no Rust**, **R1 e R4 com a parte técnica corrigida**. O que sobrou nos cinco não é conserto: promover forma/lesão/marcos a beat do boletim (R1) e dar consequência nova à hierarquia (R4) são **design**; o eixo de tensão parado é **calibração**; o frontend recalcular `dismal` em JS (R2) é **opcional**. Cada arquivo em [varredura-acoplamento/](varredura-acoplamento/) abre com a classificação afirmação por afirmação, e o [README](varredura-acoplamento/README.md) tem o quadro. **Um achado ficou aberto de propósito:** `PendingAction::UpdateHierarchy` e todo o vetor `planned_events` são write-only, e removê-los mexe no formato de `preseason_plan.json` — ver D-10. |
| D-10 | **`planned_events` do plano de pré-temporada é write-only** | M | Achado do D-09/R4, separado por tocar formato de save. `commands/career/market_window.rs` insere, filtra e remove eventos em `plan.planned_events`, e **nada no crate os executa**: quem escreve N1/N2 da temporada nova é `market::pipeline::consolidacao`, que refaz a mesma conta por skill. `refresh_planned_hierarchy_for_team` roda queries a cada aceite do jogador para produzir um evento que ninguém lê. Remover a variante `UpdateHierarchy` quebra save em andamento, porque `load_preseason_plan` falha duro em variante desconhecida — precisa de um passo de compatibilidade, ou de decidir que o plano inteiro sai. |

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

## Frontend — fechados em 11/08/2026

Não viraram `D-xx` porque nasceram e morreram no mesmo passe, a partir da
[vistoria de 10/08](vistoria-2026-08.md). Ficam registrados aqui para não serem redescobertos,
com o detalhe em [divida-tecnica.md](divida-tecnica.md).

| o que | onde |
|---|---|
| Árvore V1 da tela de resultado (15 arquivos, 1.681 linhas) removida — sem importador e com `i18n-ignore-file`, ou seja, código morto escapando do guard de i18n | `race/RaceResultView.jsx`, `race/raceresult/` |
| Mapa categoria → logo unificado (era 5 cópias divergentes) | `utils/categoryLogos.js` |
| Três vãos do auditor de i18n fechados; 8 strings PT vivas em produção corrigidas | `scripts/i18nAudit.mjs` |
| Guard de acentuação passou a varrer o locale inteiro em vez de 11 arquivos listados na mão; 29 strings sem acento corrigidas | `scripts/tests/portuguese-copy-accents.test.mjs` |
| Contrato VR_W/VR_H fechado entre JS, Rust e a layer C++ (era só comentário) | `scripts/tests/vr-overlay-contrato-dimensoes.test.mjs` |
| Nomes de `invoke` cruzados com o `generate_handler!` | `scripts/tests/invoke-contra-generate-handler.test.mjs` |
| `#[serial]` em teste que troca de idioma virou guard (a regra vivia só no CLAUDE.md) | `scripts/tests/locale-de-teste-serial.test.mjs` |
| Lógica pura extraída dos dois arquivos-deus do v2 e do painel de pose do overlay, com teste espelho | `driverDetailV2Logic.js`, `teamHistoryV2Logic.js`, `overlayPose.js` |
| Cobertura nova onde havia zero: `standings/`, `calendar/`, overlay (rádio e pose), painéis de iRacing | 11 arquivos de teste |
| Bancadas de layout soltas na raiz foram para o `.gitignore`; `testar-quali-destruida.cmd` foi para `scripts/` | `.gitignore`, `scripts/` |

---

## Fora do backlog de propósito

- `divida-tecnica.md` segue como **registro do que foi resolvido** — não duplicar aqui.
- Os cuidados de manutenção do §27 do DESIGN (semântica de categoria especial, `glass-strong`,
  `BEGIN IMMEDIATE`, `season_week = week_of_year + 4`) são regras permanentes, não tarefas.
