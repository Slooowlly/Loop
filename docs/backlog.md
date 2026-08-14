# Backlog — Loop

Lista única do que fazer no app. Levantada em 2026-07-27 varrendo o código contra o capítulo de
estado e pendências do [DESIGN.md](DESIGN.md) — hoje a **§27**, e era a §23 no retrato de junho — e
as telas realmente montadas no `Dashboard`.

**O porquê de cada item está em [roadmap.md](roadmap.md)** — aqui fica a lista com ids
e status; lá fica o raciocínio. Itens marcados ⚠️ foram revisados depois da segunda
varredura, mais funda, que corrigiu erros desta primeira.

**Como usar:** cada item tem um id estável (`F-xx`, `D-xx`, `P-xx`), um tamanho
(P / M / G) e uma linha de por quê. Item feito sai daqui e, se for dívida técnica,
vira registro em [divida-tecnica.md](divida-tecnica.md). Item novo entra no fim da
seção — não renumere os antigos.

**Estado hoje: pergunte ao guard, não a esta linha.** O
[`scripts/tests/invoke-contra-generate-handler.test.mjs`](../scripts/tests/invoke-contra-generate-handler.test.mjs)
cruza os `invoke("...")` de `src/` com o `generate_handler!` de `lib.rs`, cobra que todo
invoke exista do outro lado e congela a lista dos órfãos com o motivo de cada um em
`SEM_CONSUMIDOR_CONHECIDO`. **A lista do guard é o número oficial**, e nenhuma contagem em prosa é:
esta linha já disse 198/177/21 e envelheceu em menos de um dia — a lista de órfãos mudou duas vezes
só em 11/08/2026 (um comando removido, três que ganharam tela). Para contar hoje:

```bash
node --test scripts/tests/invoke-contra-generate-handler.test.mjs
```

O Dashboard tem 5 abas na barra (`standings`, `news`, `carreira`, `my-team`, `calendar`) mais
4 vistas alcançadas por navegação interna (`next-race`, `global-drivers`, `global-teams`,
`team-records`). A aba `carreira` entrou em 11/08/2026 e é a lente do protagonista: ela
fechou F-01, F-02, F-03, F-04 e F-05 de uma vez, em cinco seções sobre o mesmo payload.

---

## Features — o que falta pro jogo ficar inteiro

O padrão dominante: **o backend está pronto e não tem tela.** Quase todo item aqui é
trabalho de frontend em cima de simulação que já roda.

| id | item | tam | por quê |
|---|---|---|---|
| ~~F-01~~ | **Mercado durante a temporada** | M | **FEITO em 11/08/2026.** Seção Mercado da aba Carreira (`src/pages/tabs/carreira/MercadoSection.jsx`): estado do contrato com aviso de último ano, valor de mercado contra o salário atual, "quem está de olho em você" (`get_inbox_messages().team_interest`) e as vagas abertas do mundo com o veredito de elegibilidade — comando novo `get_season_market_board`. ⚠️ A parte do item que pedia "conduzir `advance_transfer_window`" foi **recusada com evidência**: o comando era um no-op (corpo idêntico ao de `get_transfer_window_state`, `accepted_seat_id` ignorado) e foi REMOVIDO em vez de religado. Quem avança o mercado é `advance_market_week`. |
| ~~F-02~~ | **Tela do meu piloto** | M | **FEITO em 11/08/2026.** Seção Meu piloto da aba Carreira, com cabeçalho fixo do protagonista (nome, título, licença, lesão, momento, motivação, posição no campeonato) fora das pílulas. Abre pela habilidade MEDIDA (`get_player_dossier`, que só existe para o jogador), seguida de arco de carreira, personalidade, estrelato e leitura técnica. O `DriverDetailModal` continua servindo para olhar qualquer piloto; o que ele não era é um LUGAR. |
| ~~F-03~~ | **Arquivo histórico / temporadas passadas** | M | **FEITO em 11/08/2026.** Seção História da aba Carreira: a carreira em números, a escada de categorias percorrida, a curva de campeonato (reusa `CurvaDeCampeonato` da ficha v2 — desenhar um segundo gráfico daria duas leituras do mesmo passado) e a tabela temporada por temporada, com a em curso marcada como parcial. Tudo de `trajetoria.curva_campeonato` / `categorias_timeline`, sem comando novo. |
| ~~F-04~~ | **Sala de troféus** | P | **FEITO em 11/08/2026.** Seção Troféus da aba Carreira: prateleira de títulos com ano e equipe, o acervo da carreira, as primeiras vezes, o auge e os eventos especiais. Entrou junto de F-03 como o roadmap previa. ⚠️ A posição do jogador nos recordes do MUNDO ficou de fora de propósito: ela existe (`get_driver_dossier_ranks`) e custa ~500ms de varredura de `race_results`, e a sala tem que abrir na hora. |
| ~~F-05~~ | **Rivalidades — visão consolidada** | P | **FEITO em 11/08/2026.** Seção Rivais da aba Carreira: um card por rivalidade com placar de corrida e de sábado, gap médio, box dividido, origem e nível — reusando as chaves `driverDetail.rivals.*` em vez de criar um segundo jogo de rótulos. O Nemesis sobe ao topo (`playerInterests`), e o nome da rivalidade aparece quando há capítulo registrado. |
| ~~F-06~~ | **Backup e restauração de save** | P | **FEITO.** Conferido em 11/08/2026: `src/components/ui/BackupsModal.jsx` consome `list_backups`, `create_season_backup` e `restore_backup`, e é aberto por `src/pages/LoadSave.jsx`. O fechamento não tinha sido registrado aqui. |
| ~~F-07~~ | **UI de espectadores / interesse de evento** | M | **FEITO em 11/08/2026, e dois terços já estavam prontos.** O retorno pós-corrida da repercussão (`EventRepercussionSummary` → `RepercussionSegment`/`RepercussionCard` no `RaceResultViewV2`) e a presença pública da equipe (`presenca_publica` → `LineupStrip` no `MyTeamTabV2`) tinham sido feitos depois que o briefing foi escrito. Faltava a **exibição rica do interesse esperado**: o público era um número solto no canto do card de clima. Virou card próprio (`EventInterestCard.jsx`) com tier, público, porte da ocasião e a cota de plateia que a estrela do jogador puxa. Os três multiplicadores "de uso futuro" (`pressure_modifier`, `media_multiplier`, `motivation_multiplier`) continuam sem consumidor — ligá-los é design de equilíbrio, fora do escopo do item. |
| F-08 | **Outras categorias** | ? | ⚠️ **Provavelmente já resolvido.** `GlobalDriversTab` e `GlobalTeamsTab` já atravessam as 9 categorias. Antes de agendar, responder: o que essa aba mostraria que as globais não mostram? Se for "a classificação da categoria vizinha", é um filtro no `StandingsTab`, não uma aba. |
| F-09 | **Previsões / palpites** | M | Camada de sabor. **Desbloqueado em 11/08/2026**: dependia de F-01 e F-02, e os dois fecharam. ⚠️ O placeholder `PredictionTab.jsx` foi **removido** em 27/07/2026 (ver P-01); se voltar, o lugar natural é uma sexta seção da aba Carreira, não uma aba nova. |
| ~~F-10~~ | **Integração real com iRacing** | G | **DECIDIDO em 27/07/2026.** A redação original estava errada em dois pontos: `export/` e `commands/export.rs` foram deletados, e a exportação **mudou de casa** para `iracing_sdk/roster_gen.rs` e `season_gen.rs`. O Loop é uma ferramenta de iRacing com uma carreira simulada dentro, e correr de verdade é o caminho principal. Levantamento completo em [iracing-escopo.md](iracing-escopo.md), retrato em [DESIGN.md](DESIGN.md) §19. O backlog derivado está na §6 do `iracing-escopo.md`. |

**A fila de features fechou em 11/08/2026.** A ordem revisada era
F-03+F-04+F-05 → F-02 → F-01 → F-07, e ela se cumpriu em uma tela só: os cinco primeiros
viraram as cinco seções da aba Carreira, porque o argumento que juntava F-03/F-04/F-05 (comem
da mesma `race_history`) vale igual para F-02 e F-01, que comem do mesmo
`get_driver_detail(jogador)`. Cinco abas custariam cinco buscas do mesmo payload e cinco
cabeçalhos discordando entre si.

O que sobra na seção: **F-08**, que continua fora da fila até alguém responder o que ele
mostraria que as abas globais já não mostram, e **F-09**, que é sabor e agora está
desbloqueado (dependia de F-01 e F-02).

---

## Dívida técnica

| id | item | tam | por quê |
|---|---|---|---|
| D-01 | **Fases legadas da convocação** | M | ⚠️ **Reenquadrado em 11/08/2026 — o texto anterior estava errado.** `convocation/` NÃO é legado: os 10 comandos do bloco especial estão registrados e o `seasonSlice.js` chama todos. O que é legado são as QUATRO FASES `BlocoRegular`/`JanelaConvocacao`/`BlocoEspecial`/`PosEspecial`, e o próprio código já diz isso em `models/enums/temporada.rs` (`is_legacy()`). No modelo 9D a temporada vai `PreTemporada → Temporada → Encerramento`, e `advance_to_convocation_window` exige `BlocoRegular`, que o fluxo 9D nunca grava — `tests_9d.rs::assert_no_legacy_phases` cobra exatamente isso em 4 pontos. **Não removido, e o motivo é o que falta apurar:** a migração da virada 9D foi colapsada na baseline v53, então não dá para provar pelo código de hoje se ela reescreveu o `fase` dos saves em voo. Um save v53+ que atravessou a virada pode carregar fase legada, e a rampa se auto-cura numa virada de temporada. Decidir exige saber se ainda existe save assim. |
| D-02 | **Tabela `races` órfã** | M | ⚠️ **Medido em 11/08/2026: não são duas fontes de verdade — `calendar` é a única, e `races` está VAZIA.** `race_results.race_id` tem `FOREIGN KEY REFERENCES calendar(id)`, e a produção grava `race_entry.id` (`commands/race/persistencia.rs:193`). Os únicos `INSERT INTO races` do repositório são fixture de teste (`db/queries/races.rs`, `world/integrity.rs`, `world/team_archive.rs`), e o único `SELECT` é um JOIN em teste. Nenhum código de produção escreve nem lê a tabela. **Eliminar exige `DROP TABLE races` na próxima migração livre** (o v64 já foi usado pela normalização da meta-linguagem dos textos de IA, então hoje é a v65), que apaga linhas de saves antigos que ninguém consegue provar que não existem — decisão do dono, não improviso de limpeza. Nota de nome: `db/queries/races.rs` não fala com a tabela `races`; ele guarda as queries de `race_results`, `race_safety_cars` e `race_weekend_readings`, todas chaveadas por `calendar.id`. |
| ~~D-03~~ | **Stores stub** | P | RESOLVIDO em 11/08/2026: `useUIStore.js` e `useNotificationStore.js` eram Zustand vazios sem nenhum consumidor. Removidos. |
| ~~D-04~~ | **`hooks/useTauri.js` morto** | P | RESOLVIDO em 11/08/2026: arquivo removido. Os `invoke` seguem vindo direto de `@tauri-apps/api/core`, por decisão. |
| D-05 | **Comandos registrados sem consumidor** | P | ⚠️ **Auditado em 11/08/2026 e o inventário virou guard — que é a fonte, não este texto.** A primeira varredura falou em 4; a auditoria achou 24; daí em diante a lista só se move por decisão consciente, e cada movimento quebra o teste. A lista com o motivo de cada um vive em `SEM_CONSUMIDOR_CONHECIDO`. **Não repetimos o total aqui de propósito:** ele já foi escrito como 4, 24, 21 e 20 neste arquivo. Rode `node --test scripts/tests/invoke-contra-generate-handler.test.mjs`. **Removidos até agora (registro + implementação):** `toggle_maximize_window` e `get_window_maximized` (os controles de janela trabalham com TELA CHEIA — `WindowControlsDrawer.jsx` só chama minimize, start_drag, toggle_fullscreen, get_fullscreen e close), `get_driver` (assinatura antiga `career_number: u32`; quem lê um piloto é `get_driver_detail`) e `advance_transfer_window` (no-op; ver a linha do F-01 e [divida-tecnica.md](divida-tecnica.md)). **Ganharam tela e saíram da lista:** `iracing_desfazer_pinturas`, `iracing_modo_janela_status` e `iracing_modo_janela_restaurar`, pelo `IracingDesfazerPanel.jsx` nas Configurações. **Rótulos corrigidos:** `get_race_reading` e `ptt_gatilho_atual` NÃO são API interna — `get_race_reading_in_base_dir` só é chamado pela própria casca, e nada em Rust ou JS chama `ptt_gatilho_atual` fora dos testes de `ptt.rs`. Os que ficam se dividem em: **API interna** (só `iracing_process_race_result`, em `iracing/importacao.rs:138`); **feature futura**; **diagnóstico de bancada**; **reservado** (`ptt_gatilho_atual`, o lado de leitura do estático de `ptt.rs`); e **consumidor removido acidentalmente** (`overlay_window_set_interactive` — o botão "Mover" que o chamava não existe mais, e sem ele o overlay fica preso em click-through). A leitura pela ótica do iRacing está na §3 do [iracing-escopo.md](iracing-escopo.md). |
| D-06 | **`constants/tracks/consultas.rs`** | P | `TODO(design final)`: trocar a consulta por "pistas que o jogador realmente possui". Decisão de design pendente, não bugfix. |
| ~~D-07~~ | **Tracks ausentes do DB** | P | **RESOLVIDO em 11/08/2026 — era comentário velho, não bug de dados.** Cruzando todo id de pool de `calendar/generator.rs` com as 107 pistas de `constants::tracks::dados`, ZERO ficam fora do catálogo, e não sobrou nenhuma marcação `// TODO` de pista. O cabeçalho ainda mandava "filtrar com `get_track(id).is_some()` ao wiring no gerador real": o gerador já está ligado (`geracao.rs` chama `resolve_thematic_pool` nas duas rotas) e o filtro já está aplicado (linhas 471, 502, 528). Só o comentário foi corrigido. |
| ~~D-08~~ | **`models/driver.rs`** | P | **RESOLVIDO em 11/08/2026 — a afirmação era verdadeira, faltava a prova.** Os 19 campos de `DriverAttributes` têm coluna em `drivers`. O `TODO(migration)` saiu e no lugar ficou o ponteiro para `db::queries::drivers::tests::todo_atributo_do_piloto_tem_coluna_e_volta_igual_do_banco`, que grava um piloto com valor distinto por atributo e relê contra o schema real das migrações (não contra o DDL de bancada do `setup_test_db`). Campo novo sem coluna derruba esse teste. |
| D-09 | **Varredura de acoplamento — lado Rust (R1–R5)** | P | ⚠️ **Revisado em 11/08/2026, briefing por briefing, na mesma sessão.** A dívida TÉCNICA fechou: **R3 e R5 resolvidos**, **R2 resolvido no Rust**, **R1 e R4 com a parte técnica corrigida**. O que sobrou nos cinco não é conserto: promover forma/lesão/marcos a beat do boletim (R1) e dar consequência nova à hierarquia (R4) são **design**; o eixo de tensão parado é **calibração**; o frontend recalcular `dismal` em JS (R2) é **opcional**. Cada arquivo em [varredura-acoplamento/](varredura-acoplamento/) abre com a classificação afirmação por afirmação, e o [README](varredura-acoplamento/README.md) tem o quadro. **Um achado ficou aberto de propósito:** `PendingAction::UpdateHierarchy` e todo o vetor `planned_events` são write-only, e removê-los mexe no formato de `preseason_plan.json` — ver D-10. |
| D-10 | **`planned_events` do plano de pré-temporada é write-only** | M | Achado do D-09/R4, separado por tocar formato de save. `commands/career/market_window.rs` insere, filtra e remove eventos em `plan.planned_events`, e **nada no crate os executa**: quem escreve N1/N2 da temporada nova é `market::pipeline::consolidacao`, que refaz a mesma conta por skill. `refresh_planned_hierarchy_for_team` roda queries a cada aceite do jogador para produzir um evento que ninguém lê. Remover a variante `UpdateHierarchy` quebra save em andamento, porque `load_preseason_plan` falha duro em variante desconhecida — precisa de um passo de compatibilidade, ou de decidir que o plano inteiro sai. |
| D-11 | **O piso do cheque especial é absoluto num módulo que virou relativo** | M | Achado em 14/08/2026, investigando a queda do CI no `aaa0d5d`. [`finance/cashflow.rs:301`](../src-tauri/src/finance/cashflow.rs) converte em dívida tudo que passa de `-100_000.0` de caixa e trava o caixa nesse valor. É um literal cru, sem nome e sem comentário, e é o único absoluto que sobrou: o vizinho `debt_amortization_reserve` já é por categoria, e o socorro inteiro passou a meses de operação no B50. **Na rookie esses 100 mil valem 5,7 meses de operação; num degrau caro valem uma fração de mês** — a mesma equipe endividada é tratada como situação diferente conforme o degrau, e é justamente a classe de defeito que os comentários do próprio `events.rs` descrevem ter caçado duas vezes. **A consequência medida:** ele fecha a janela do socorro por baixo. O gate de necessidade pede caixa abaixo de `-2` meses e o teto pede dívida abaixo de `4` meses, e o cheque especial converte o rombo em dívida antes do gate ser avaliado. Na rookie a janela em que o socorro sai é de `-170 mil` a `-35 mil` de caixa pós-rodada; noutro degrau ela tem outra largura, sem que nada diga isso. Trocar por `PISO_CHEQUE_ESPECIAL_MESES` é uma linha, e é **calibração de economia**: muda quanta dívida o mundo cria em cada degrau, então pede o harness de `economia` antes e depois. Fica como pendência por decisão de 14/08/2026. |

O lado frontend da varredura (**F1–F4** — helpers de pista/clima, `formatLap` e paleta de gráficos,
`getReadableTeamColor`, `IN_TAURI`) foi resolvido no commit `2c85f44`.

---

## Vistoria da série V: o que ficou esperando decisão

A vistoria de 11/08/2026 está em
[vistoria-independente-2026-08-11.md](vistoria-independente-2026-08-11.md) e o que fechou está em
[divida-tecnica.md](divida-tecnica.md). Sobrou o que **não é conserto mecânico**: cada linha abaixo
espera uma resposta, não trabalho. Enquanto a resposta não vem, o comportamento de hoje é o que
está descrito, e ele é intencional.

| id | pergunta em aberto | por que não é mecânico |
|---|---|---|
| V-D1 | **GT4, GT3, LMP2, Production Challenger e Endurance: recusar para sempre, mapear para um carro pago, ou marcar a categoria como "só simulada"?** Hoje o export recusa com motivo. | Cada saída tem custo diferente. Carro pago exige saber o que o jogador possui e quebra a promessa de conteúdo grátis; "só simulada" precisa aparecer na UI da categoria, não só no botão; Production e Endurance ainda esbarram no grid de mais de uma classe, que o aiseason não representa. |
| V-D2 | **Existe cenário em que o resultado precisa ser construído da sessão ao vivo em vez do JSON do aiseason?** Se não existe, `iracing_sdk/result_bridge/sessao.rs` sai, com os testes junto. | São 269 linhas com testes próprios, o que as mantém verdes para sempre e faz o módulo parecer vivo. Apagar é fácil; saber se o cenário "aiseason não saiu" existe na prática depende de uso real do produto. |
| V-D3 | **`race_weekend_readings` (migrações v55 e v57): quem grava?** Hoje ninguém, e `get_race_weekend_reading` devolve `None` para toda corrida nova, para sempre. | Ou a Sala de Estratégia ganha a escrita (feature), ou o trio getter/setter/comando sai e a tabela vira `DROP` numa migração futura, o que apaga linha de save antigo. As duas saídas são trabalho de produto. |
| V-D4 | **`RosterGenPanel` e `PostRacePanel` ganham lugar na UI ou são apagados?** São 1.423 linhas e seis comandos de ponte pendurados na resposta. Os outros órfãos do V6.1 (`RaceCharts.jsx`, `RaceCoursePanel.jsx`, `GaragePanelV2.jsx`, `DebugDataSwitch.jsx`) esperam a mesma decisão, painel a painel. | São a bancada de diagnóstico do iRacing. Ligar significa dar tela, rota e copy traduzida; apagar significa perder a bancada. Nenhuma das duas é limpeza. |
| V-D5 | **Notícia antiga deve acompanhar a troca de idioma?** As três strings PT cruas viraram chave, e isso fechou. O que sobra é que `news` guarda prosa renderizada, então notícia gravada fica no idioma em que nasceu. | "Sim" implica `news` guardar chave mais parâmetros, o que é mudança de formato de save e migração. "Não" precisa ser escrito em algum lugar, porque hoje é acidente e não decisão. |
| V-D6 | **O padrão `cargo fmt` e clippy vale como obrigatório?** Em 11/08/2026 `cargo fmt --check` acusa **um** arquivo (`commands/overlay/radio.rs`), contra os 119 da vistoria, e `cargo clippy --all-targets` sai sem erro e com 428 avisos. O CI não roda nenhum dos dois. | Ligar `-D warnings` de uma vez transforma 428 avisos em build vermelho. Ligar só `cargo fmt --check` é barato, e ainda assim reformata em bloco e colide com qualquer frente de Rust aberta. Se o padrão não vale, a afirmação sai do CLAUDE.md. |
| V-D7 | **Qual é o alvo da calibração V3.1?** A pole vence 76% a 85% das provas contra alvo declarado de 15% a 55%, e o Spearman grid × chegada dá 0,91 a 0,96 contra 0,40 a 0,88. | Antes de perseguir o número é preciso decidir se `Alvos::entrada()` e `Alvos::topo()` descrevem o jogo desejado: a diferença entre "queremos 0,75" e "queremos 0,88" muda inteiramente o tamanho do conserto, e sem alvo a varredura de knobs não tem critério de parada. Enquanto isso, os tetos de não-regressão impedem que piore. |
| V-D8 | **O que fazer com os 181 itens `never used` do `cargo build --lib`?** A contagem não mudou (201 avisos, 21 imports não usados). | O número mede "API pública alcançada apenas por teste", não código morto: cinco de sete conferidos à mão têm chamador em `#[cfg(test)]`. Separar os dois conjuntos é leitura item a item, e as duas builds emitem conjuntos de aviso textualmente disjuntos. Só os imports são mecânicos. |
| V-D9 | **Vale um job de CI semanal rodando `cargo test -- --ignored`?** São 60 ignorados hoje, a maioria harness de medição legítimo. | Três deles são asserções que falham por causa do V3.1 e devem continuar vermelhas até a calibração fechar, então o job nasce vermelho por projeto. Isso precisa ser combinado antes, ou o job vira ruído que ninguém lê. |
| V-D10 | **Limpeza destrutiva do disco e da raiz.** Fora do git e sem rastro: `src-tauri/target` com **91,5 GB**, `C:/cargo-target/iracer` com **283,2 GB**, `C:/dev/loop-target` com **12,5 GB**, mais `%TEMP%/`, `rh_fail.log` e cinco páginas de preview soltas na raiz. O drive C: tinha 94,5 GB livres em 11/08/2026. | Apagar diretório de build é irreversível e joga fora cache que custa recompilação. Os 283 GB do target configurado são o que o desenvolvimento usa todo dia. Nada disso vaza para o commit, então é decisão de máquina e não de repositório. |

---

## Pontas soltas

| id | item | tam | por quê |
|---|---|---|---|
| ~~P-01~~ | ~~9 arquivos de tela órfãos~~ | — | **Feito em 2026-07-27.** Removidos os 9 placeholders (`MarketTab`, `DriversTab`, `MyProfileTab`, `PredictionTab`, `OtherCategoriesTab`, `TrophyRoom`, `Archive`, `Rivalries`, `SeasonsHistory`), mais o componente `AppPlaceholder` e o guard `app-placeholder-visual-alignment.test.mjs` que só existiam para eles. `src/pages/history/` deixou de existir. As telas voltam pelo caminho dos `F-xx`, escritas de verdade. |
| ~~P-02~~ | ~~Sem rota `/history`~~ | — | **Resolvido em 11/08/2026, e sem rota.** A história do jogador é a seção História da aba Carreira, dentro do `Dashboard`. Não ganhou rota própria de propósito: as outras oito vistas do Dashboard também não têm, e criar `/history` faria dela a única tela de conteúdo alcançável sem passar pela carreira carregada. |
| ~~P-03~~ | ~~Árvore de trabalho suja~~ | — | **Feito em 2026-07-27** (commit `2c85f44`): 161 arquivos, cinco frentes, com as três suítes verdes antes de commitar. |
| P-05 | **Corrida multiclasse de verdade** | G | Hoje as categorias que compartilham pista só têm o conflito de calendário prevenido por `has_calendar_conflict()` em `constants/categories.rs`: elas correm em entradas separadas, sem interação nenhuma. Fazer a coisa inteira pede grid combinado (intercalado por performance relativa entre classes), resultado por classe além do geral, tráfego e bandeiras cruzando as classes, e o calendário marcando quais rodadas são multiclasse. Registrado aqui em 12/08/2026 ao remover `src-tauri/src/calendar/multiclass.rs`, que era um arquivo só de comentário, nunca declarado em `calendar/mod.rs` e portanto nunca compilado — a nota estava fora do alcance de qualquer leitor e de qualquer guard. |
| P-04 | **Chaves i18n órfãs** | P | Removidas as telas do P-01, as chaves `marketTab.*`, `driversTab.*` e afins ficaram sem consumidor em `pt-BR/common.json` e `en-US/common.json`. Deixadas de propósito — voltavam a ser usadas nos `F-xx`. ⚠️ **A aposta não se pagou**: os `F-xx` fecharam em 11/08/2026 sob o namespace novo `carreiraTab.*`, com prosa escrita para as telas de verdade, e nenhuma chave antiga foi reaproveitada. Agora é limpeza pura, e o único item que ainda a bloqueia é F-09. |

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
- Os cuidados de manutenção do DESIGN são regras permanentes, não tarefas. Os quatro que a §27
  lista hoje: não colapsar as duas semânticas de categoria especial (§5.1), `duracao_corrida_min = 0`
  como sentinela de endurance (§7.4), procurar a feature pelo comando Tauri e não pelo nome do
  arquivo, e `npm run build` antes de `cargo build`/`cargo test`. Os outros dois que esta linha
  citava vivem em capítulos próprios: `BEGIN IMMEDIATE` na §23.3 (concorrência) e a armadilha do
  `glass-strong` na §25.5 (design system). `season_week = week_of_year + 4` é definição, não
  cuidado, e está na §7.3.
