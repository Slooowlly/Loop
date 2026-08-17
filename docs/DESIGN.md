# Loop, Documento de Design Completo

> **Loop** é um jogo desktop de carreira no automobilismo construído em volta do **iRacing**.
> Este documento descreve tudo o que existe no programa hoje: identidade de produto, arquitetura,
> modelo de domínio, mecânicas, banco de dados, ponte IPC e frontend. É um retrato do código
> atual. Quando este texto divergir do código, o **código é a fonte de verdade**.

Data do retrato: 2026-08-11 · Versão do app: **0.14.0** · Schema do banco: **v64** (baseline v53)
· Categorias: **9** · Comandos IPC registrados: **ver o guard** (§24 — o total em prosa envelhece no
primeiro comando novo, e envelheceu duas vezes aqui)

**Histórico deste arquivo:** a versão anterior era o retrato de 2026-06-15 e descrevia o Loop como
"simulador offline de carreira". Essa identidade foi aposentada por decisão do dono do projeto em
2026-07-27 (§19). O cabeçalho velho seguia contradizendo o próprio capítulo de iRacing do mesmo
arquivo, e o corpo apontava schema v34 com o banco já na casa dos v60. Esta revisão refaz o documento
inteiro contra o código.

---

## 1. Visão geral

O jogador controla **um único piloto** ao longo de uma carreira que sobe por uma pirâmide de 9
categorias (Mazda Rookie até Endurance). Ao redor desse piloto o Loop mantém um **mundo vivo**:
200+ pilotos de IA e 60+ equipes que correm, evoluem, são contratados, promovidos e rebaixados,
lesionam-se, criam rivalidades e se aposentam sozinhos.

**O caminho principal é correr a etapa de verdade.** O Loop exporta o grid e o calendário como AI
roster e AI season do iRacing, o jogador corre a prova dentro do simulador, e o resultado oficial
volta para a carreira. A simulação interna preenche o que ele não corre: as outras 8 categorias,
as etapas puladas e o mundo inteiro entre uma corrida e outra.

**"Offline" vale para os dados.** Não há servidor de jogo, nenhuma conta e nenhum login: tudo vive
num SQLite no disco do jogador. O propósito do app é online no sentido que importa, que é correr
no iRacing. Os únicos serviços externos são opcionais e de enriquecimento: o proxy de notícias por
IA e o endpoint de telemetria de produto, ambos com opt-out.

**Windows é requisito de produto, e não detalhe de implementação.** O SDK do iRacing e a winapi
são Windows-only. Fora do Windows a integração compila como stub inerte por construção, e o CI
roda em `windows-latest`.

Pilares de design:

- **Mundo persistente e coerente.** Cada temporada deixa histórico arquivado (pilotos, equipes,
  campeões, resultados, finanças).
- **Simulação probabilística por atributos.** A UI não mostra número cru: a leitura é por tags e
  qualitativos, e o motor usa 18 atributos numéricos por baixo.
- **Carreira de longo prazo.** Promoção e rebaixamento com tamanho fixo de categoria, licenças,
  evolução e declínio por idade, mercado entre temporadas e um bloco especial sazonal.
- **Companhia dentro do carro.** Engenheiro de pista e spotter falam com o jogador durante a
  corrida real, com voz, lendo a telemetria ao vivo.

O código, os comentários e a UI são em **português**.

---

## 2. Stack tecnológica e arquitetura

| Camada | Tecnologia |
|---|---|
| Shell desktop | **Tauri v2** (plugins: shell, dialog, fs, updater) |
| Backend e motor | **Rust** (edição 2021), `rusqlite` |
| Persistência | **SQLite** (arquivo local), migrações versionadas |
| Frontend | **React 18** + **Vite 5** |
| Estado (UI) | **Zustand** (slices) |
| Roteamento | **react-router-dom v6** |
| Gráficos | **Recharts** + canvas próprio nos overlays |
| Estilo | **Tailwind CSS 3** + CSS custom (glass system) |
| Fonte | Space Grotesk Variable |
| i18n | **i18next** (frontend) + **rust-i18n** (backend) |
| Testes | **Vitest** (UI), `node --test` (guards estruturais), `cargo test` (Rust) |

**Modelo de execução.** O frontend é burro no sentido de regra de jogo: quase toda decisão de
domínio acontece no Rust. A comunicação é via comandos IPC do Tauri (`invoke`). O Rust abre a
conexão SQLite, roda a lógica em transação e devolve DTO serializado (serde) ao React.

```
┌──────────────────────────────────────────────────────────┐
│  React (Vite): páginas, abas, overlays, slices Zustand     │
│      │  @tauri-apps/api  invoke(cmd, args)                  │
└──────┼───────────────────────────────────────────────────┘
       ▼  IPC (comandos registrados em lib.rs — total no guard, §24)
┌──────────────────────────────────────────────────────────┐
│  Rust: commands/ → módulos de domínio                      │
│  simulation · car · evolution · market · promotion         │
│  economia · finance · engenheiro · narrative · news         │
│  iracing_sdk (leitura do sim, export, import, spotter)     │
│      │  db/queries/*  (rusqlite)                            │
└──────┼───────────────────────────────────────────────────┘
       ▼                        ▼
   SQLite (save por carreira)   iRacing (memória compartilhada,
   + config.json + backups      airosters/, aiseasons/, app.ini)
```

**Amostrador de fundo.** No `setup` do `lib.rs` sobem `race_capture::init` e
`race_monitor::start_watching()`: um amostrador a ~60 Hz que consome `read_telemetry` e
`read_session` por dentro do crate, sem passar pela ponte IPC, e alimenta o monitor unificado de
corrida (tentativas de ultrapassagem, batidas, DNF, quebra de peça, incidentes, estilo de
pilotagem).

**Conferir o que foi gravado.** `scripts/captura-auditar.mjs` (`npm run captura:auditar`) audita a
captura em quatro frentes: estrutura (cabeçalho, inventário, YAML, bloco `history`, trailer do
gzip), continuidade (taxa efetiva, buracos, saltos), canais mortos e o derivado contra o cru.

O modo de falha que ele existe para pegar está em `canais.rs`: a leitura casa cada canal por NOME
num `match`, e nome que não existe cai no `_ => {}` calado. Foi assim que `PitRepairNeeded` (o
nome real é `PitRepairLeft`) deixou o dano do carro sumir. **De fora, um canal ausente e um canal
que vale zero são a mesma coisa**, e o auditor separa os dois cruzando três listas: os canais que
o Rust cura, os que o sim publica (o bloco `vars`), e os campos que de fato variaram.

> **A regressão que ele achou, em 17/08/2026.** `record_frame` pulava o quadro sempre que
> `session_time` não fosse MAIOR que o último gravado. Reiniciar a sessão devolve o relógio a
> zero, e a guarda passou a recusar todo quadro seguinte até o relógio novo ultrapassar o
> velho: **594 segundos de sim sem um único quadro** numa corrida em Oschersleben, mais 161 s e
> 614 s em outras duas capturas do acervo. O arquivo resultante parece contínuo, porque o filtro
> só admite valor crescente, e o buraco só aparece cruzando com o `session_tick` — que é o
> relógio monotônico do sim e não reinicia. Hoje a decisão mora em `decidir()`, uma função pura
> com os três casos (gravar, pular, relógio novo) e a regressão coberta por teste.

Duas régua medidas em 16/08/2026 sobre as onze capturas do disco, e ambas contra a intuição:

- **A taxa efetiva não é 60 Hz e não pode ser.** O amostrador dorme `SAMPLER_PERIOD_MS = 16` e o
  relógio do Windows tem resolução de ~15,6 ms. A banda real é **53 a 58 Hz**, e um limiar posto
  em 60 reprovaria toda captura saudável.
- **O `cars[]` tem teto aritmético abaixo do alvo.** Ele é escrito no primeiro quadro que cruza
  1/20 s desde o último, e a 56 Hz esse é o terceiro quadro: 18,7 Hz, caindo a 14 quando o jitter
  empurra para o quarto. O teto é `taxa / ceil(taxa / alvo)`, nunca os 20 nominais.

O julgamento de "campo constante a corrida inteira" é por GRUPO (ambiente, sessão, sentinela,
pilotagem, progresso), cada um com a condição sob a qual a constância vira alarme. A primeira
versão, sem grupos, apontou 19 falsos numa única sessão, e um auditor que sempre grita treina
quem o lê a ignorá-lo. O guard `scripts/tests/auditor-de-captura-le-o-rust.test.mjs` cobre a
parte frágil: as duas extrações por regex do Rust falham devolvendo VAZIO, e o relatório seguiria
saindo bonito sem ter olhado nada.

**Janelas.** `tauri.conf.json` declara três webviews servindo o mesmo `index.html`: a principal
(sem decoração, com controles de janela em React sobre `commands/window.rs`), `overlay` e
`engineer`, as duas transparentes e always-on-top, para uso sobre o iRacing em corrida e em VR.

---

## 3. Estrutura de diretórios

### 3.1 Backend (`src-tauri/src/`)

| Módulo | Responsabilidade |
|---|---|
| `commands/` | Camada IPC, funções `#[tauri::command]` expostas ao React |
| `config/` | `AppConfig` (config.json: idioma, autosave, janela, caminho do iRacing, consentimentos) |
| `constants/` | Dados estáticos: categorias, pistas, carros, pontuação, equipes, linha do tempo histórica, faixas de skill |
| `models/` | Entidades de domínio: driver, team, contract, season, license, injury, rivalry, enums, temporal, tags |
| `db/` | `connection`, `migrations` (baseline v53 + incrementais até v64), `queries/` (um módulo por área) |
| `generators/` | Geração de mundo: nomes, nacionalidades, ids, pilotos, `world` (bootstrap) |
| `calendar/` | Geração de calendário (janelas mensais, multiclasse, slots temáticos, temporada cheia e parcial) |
| `simulation/` | Motor de corrida: `qualifying`, `race/` (motor, tráfego, estratégia, danos, pontuação), `incidents`, `forma`, `profile`, `track_profile`, `calibracao`, `batch` |
| `car/` | Carro físico: peças, desgaste, quebra (`breakdown`), custo de conserto, batida, estilo de pilotagem, ponte com a simulação |
| `evolution/` | Crescimento e declínio por idade, experiência, motivação, lesões, aposentadoria, licenças, transição de temporada |
| `market/` | Mercado entre temporadas: proposta, renovação, assédio (poaching), IA de piloto e de equipe, janela de transferências |
| `convocation/` | Bloco especial sazonal: elegibilidade, cotas, pontuação, ofertas ao jogador, janela diária |
| `promotion/` | Promoção e rebaixamento de equipes na escada fechada de categorias |
| `hierarchy/` | Hierarquia N1/N2 dentro da equipe e as transições entre elas |
| `rivalry/` | Modelo dual de rivalidade (intensidade histórica e atividade recente) |
| `finance/` | Caixa da equipe: salário, prêmio, fluxo, moral, reputação, foco, resgate, estratégia, estado |
| `economia/` | Modelo econômico novo, escrito ao lado do `finance/`: âncora, receita, fatura de etapa e de temporada, desenvolvimento, evento |
| `event_interest/` | Interesse esperado e realizado do evento (sistema de espectadores) e impacto público |
| `public_presence/` | Presença pública e atração de equipe e de piloto |
| `fame` | Fama do piloto, derivada de mídia e resultado |
| `player_skill`, `race_eval`, `race_signals` | Leitura da performance do jogador e sinais extraídos da corrida |
| `sim_stats/` | Instrumentação da própria simulação: métricas, snapshots, ciclo, experimentos de calibragem |
| `news/` | Geração de notícia determinística pós-corrida e de mercado |
| `narrative/` | Camada narrativa sobre a notícia: tese, beats, contexto, incidentes, consulta e cliente HTTP do proxy de IA |
| `engenheiro/` | Engenheiro de pista: fatos, intenção, momento, fala, quebra, ritmo, tempo de volta, vizinhança, memória, tratamento, catálogo de peças de áudio |
| `iracing_sdk/` | Integração com o iRacing (§19): leitura de telemetria e sessão, monitor de corrida, geração de roster e temporada, pintura, race control, spotter, clima, percepção de rivalidade, dificuldade adaptativa, ponte de resultado |
| `telemetry/` | Telemetria de produto (anônima, opt-out): fila, entrega, uso |
| `volante`, `radio_registro`, `diagnostico` | Botão do volante para PTT, registro do rádio, log rotativo de diagnóstico |
| `world/` | Arquivamento de temporada e integridade do mundo |
| `common/` | Utilitários (tempo e afins) |

### 3.2 Frontend (`src/`)

| Pasta | Conteúdo |
|---|---|
| `pages/` | `MainMenu`, `NewCareer`, `LoadSave`, `Settings`, `Dashboard` |
| `pages/tabs/` | Abas do Dashboard: `NextRaceTab`, `StandingsTab`, `CalendarTabRedesign`, `NewsMagazineTab`, `GlobalDriversTab`, `TeamRecordsTab`. Equipe e atlas existem **só** na V2, em `tabs/myteam/MyTeamTabV2` e `tabs/atlas/GlobalTeamsTabV2` (o `index.js` de cada pasta é o que o `Dashboard` importa como `MyTeamTab` e `GlobalTeamsTab`) |
| `components/calendar/` | Calendário: célula do dia, mini mês, linha de evento, tooltip de bilhete |
| `components/driver/` | Ficha e dossiê de piloto em `v2/` (o v1 saiu em 11/08/2026; `index.js` é só o reexport que mantém o nome `DriverDetailModal`), mais `detalhes/`, mini card, ranking global, marcador de rival |
| `components/team/` | Equipe: histórico e atlas em `v2/` (o v1 saiu em 11/08/2026; `history/index.js` é só o reexport de `TeamHistoryDrawer`), logo, mini card, grade mundial, finanças, `myteam/` |
| `components/race/` | Fim de semana: briefing do engenheiro, resultado V2, gráficos (traçado, ritmo, volta, clima), cockpit de telemetria, risco de quebra |
| `components/season/` | Pré-temporada (`preseason/`), convocação (`convocacao/`), fim de temporada, leilão de assédio, overlay de campeão |
| `components/standings/` | Tabelas de classificação, escada de categorias, navegador de série, selo de troféu |
| `components/news/` | Revista: capa, matéria de etapa, encarte de pré-temporada, notas do mundo, caixa postal |
| `components/iracing/` | Overlay de conectado, diagnóstico, tutorial, painel de compostos, percepção de rivalidade, PTT do engenheiro |
| `components/system/` | Atualizador, changelog, portão de consentimento de telemetria |
| `components/layout/`, `ui/`, `wizard/` | Casca (header, navegação, controles de janela, menu de pausa), design system e wizard de nova carreira |
| `overlay/` | App dos overlays: torre de canvas, rádio do engenheiro, spotter, escritores de VR, painel de posição |
| `lib/` | Voz do engenheiro, fila de carga de áudio, filtro de rádio, microfone, PTT, registro de rádio, updater |
| `stores/` | `useCareerStore` (composição) sobre `stores/career/` (`careerSlice`, `raceSlice`, `marketSlice`, `seasonSlice`, `blocoEspecialSlice`, `preRaceCacheSlice`) e `useAttentionStore` |
| `hooks/` | `useCareerDraft`, `useConfiguracaoDoApp`, `useExitToMenu`, `useFerramentasDeDebug`, `useIracingFocoAutomatico`, `useLoading`, `useRaceControl`, `useSaves`, `useSpotterNativo`, `useTempoDeTela` |
| `i18n/` | i18next, um namespace por área, `locales/<lang>/common.json` |
| `utils/`, `styles/`, `assets/`, `dev/`, `test/` | Formatadores, cores de categoria, logos, design system, arte e apoio de teste |

Não existem mais `pages/history/`, `components/charts/`, `useUIStore`, `useNotificationStore` nem
`hooks/useTauri.js`. Os três últimos eram stub sem consumidor e foram removidos em 11/08/2026, com
o registro em [divida-tecnica.md](divida-tecnica.md). Os `invoke` vêm direto de
`@tauri-apps/api/core`, nos slices ou nos hooks de dados dos componentes, por decisão: não há
camada de abstração da ponte.

---

## 4. Modelo de domínio

### 4.1 Piloto (`drivers`)

Entidade central. Pode ser o jogador (`is_jogador = 1`) ou IA.

- Identidade: `id`, `nome`, `idade`, `nacionalidade`, `genero`, `ano_inicio_carreira`.
- Estado: `status` (`Ativo`, `Lesionado`, `Aposentado`, `Suspenso`), `categoria_atual`,
  `categoria_especial_ativa` (separada da regular, usada no bloco especial).
- Personalidade: primária (`Ambicioso`, `Consolidador`, `Mercenario`, `Leal`) e secundária
  (`CabecaQuente`, `SangueFrio`, `Apostador`, `Calculista`, `Showman`, `TeamPlayer`, `Solitario`,
  `Estudioso`).
- **18 atributos** (0 a 100), detalhados em §6, mais `potencial`.
- `forma` (REAL, migração v54): o estado do AR(1) de `simulation::forma`, a forma do momento.
  0.0 é forma neutra.
- Stats da temporada (`temp_*`) e de carreira (`carreira_*`).
- Rastreio dinâmico: `motivacao`, `historico_circuitos` (JSON), `ultimos_resultados` (JSON),
  `temporadas_na_categoria`, `corridas_na_categoria`, `temporadas_motivacao_baixa`.

### 4.2 Equipe (`teams`)

- Identidade e visual: `nome`, `nome_curto`, `cor_primaria`, `cor_secundaria`, `pais_sede`,
  `ano_fundacao`, `marca`, `classe`.
- Categoria atual e `categoria_anterior` (para detectar promoção e rebaixamento).
- Performance: `car_performance`, `confiabilidade`, `budget`, `reputacao`, `facilities`,
  `engineering`, `morale`, `aerodinamica`, `motor`, `chassi`.
- Estratégia: `car_build_profile`, `pit_strategy_risk`, `pit_crew_quality`, `season_strategy`.
- Hierarquia interna N1/N2: `hierarquia_status`, `hierarquia_tensao`, `hierarquia_duelos_total`,
  `hierarquia_inversoes_temporada`.
- Finanças: `cash_balance`, `debt_balance`, `financial_state`, `last_round_income`,
  `last_round_expenses`, `last_round_net`, `parachute_payment_remaining`.
- Stats de temporada (`stats_*`) e de história (`historico_*`), mais `carreira_titulos`.

> Os nomes legados `reliability`, `prestige`, `temp_pontos`, `temp_vitorias` e `carreira_vitorias`
> saíram de `teams` na normalização registrada como DB-001 a DB-004 em
> [divida-tecnica.md](divida-tecnica.md). Os nomes de domínio são `confiabilidade`, `reputacao`,
> `stats_pontos`, `stats_vitorias` e `historico_vitorias`.

### 4.3 Contrato (`contracts`)

Liga piloto e equipe. `papel` (`Numero1` ou `Numero2`), `salario` e `salario_anual`,
`duracao_anos`, `temporada_inicio` e `temporada_fim`, `categoria`, `classe` (preenchida só em
contrato especial multiclasse), `tipo` (`Regular` ou `Especial`), `status` (`Ativo`, `Expirado`,
`Rescindido`, `Pendente`) e `clausulas`.

**Invariante:** índice único garante no máximo **um contrato ativo por (piloto, tipo)**.

### 4.4 Temporada (`seasons`)

`numero`, `ano`, `status` (`EmAndamento` ou `Finalizada`), `rodada_atual` e **`fase`**, o coração
do loop macro (§7.1). **Invariante:** índice único garante uma única temporada `EmAndamento`.

### 4.5 As outras entidades

O schema tem 54 tabelas. Agrupadas por assunto:

- **Corrida:** `calendar` (o evento agendado, que é a corrida de verdade), `race_results`,
  `standings`, `race_breakdowns` (quebra de peça por corrida), `race_safety_cars`,
  `race_weekend_readings` (a leitura do fim de semana), `player_race_telemetry`,
  `track_lap_records`, `track_dnf_history`, `incident_catalog`, `races` (legada).
- **Piloto e equipe:** `licenses`, `injuries`, `driver_favorites`, `driver_team_bond`,
  `player_nemesis`, `rivalries`, `rivalry_episodes`, `team_rivalries`, `team_car`, `team_focus`,
  `team_strategic_plan`, `team_finance_history`, `team_collapse_streak`, `team_rescue_counters`,
  `team_promotion_history`, `team_ownership_events`.
- **Mercado:** `market`, `market_proposals`, `transfer_window`, `player_special_offers`, mais as
  quatro tabelas `special_window_*` e `special_team_entries` do bloco especial.
- **História:** `driver_season_archive`, `team_season_archive`, `history_seasons`,
  `history_general`, `retired`, `category_scalar_records`, `record_milestones`.
- **Conteúdo por IA:** `ai_pre_race_briefing`, `ai_post_race_debrief`, `ai_race_story`,
  `ai_world_notes`, e `news` para o feed determinístico.
- **Infra:** `meta` (contadores e configuração da carreira) e `config`.

---

## 5. Sistema de categorias e progressão

9 categorias fixas (`constants/categories.rs`), organizadas por tier:

| id | Nome | Tier | Nível | Grid | Corridas | Duração | Licença | Multiclasse |
|---|---|---|---|---|---|---|---|---|
| `mazda_rookie` | Mazda MX-5 Rookie Cup | 0 | Rookie | 6×2 = 12 | 5 | 15 min | (nenhuma) | não |
| `toyota_rookie` | Toyota GR86 Rookie Cup | 0 | Rookie | 6×2 = 12 | 5 | 15 min | (nenhuma) | não |
| `mazda_amador` | Mazda MX-5 Championship | 1 | Amador | 10×2 = 20 | 8 | 25 min | 0 | não |
| `toyota_amador` | Toyota GR86 Cup | 1 | Amador | 10×2 = 20 | 8 | 25 min | 0 | não |
| `bmw_m2` | BMW M2 CS Racing | 2 | Pro | 10×2 = 20 | 8 | 25 min | 1 | não |
| `production_challenger` | Production Car Challenger | 2 | **Especial** | 18×2 = 36 | 10 | 30 min | 1 | **sim** |
| `gt4` | GT4 Series | 3 | Super Pro | 10×2 = 20 | 10 | 30 min | 2 | não |
| `gt3` | GT3 Championship | 4 | Master | 14×2 = 28 | 14 | 50 min | 3 | não |
| `endurance` | Endurance Championship | 6 | **Especial** | 18×2 = 36 | 6 | (variável) | 4 | **sim** |

**Classes multiclasse:**

- **Production Challenger:** `mazda` (×1.00), `toyota` (×1.00), `bmw` (×1.05), 6 equipes cada.
- **Endurance:** `gt4` (×0.85), `gt3` (×1.00), `lmp2` (×1.30), 6 equipes cada.
- **LMP2** é uma classe de referência dentro da Endurance (tier 5, Elite), sem existência como
  categoria autônoma do grid principal. `get_category_config("lmp2")` devolve config sintética.

**Conflitos de calendário** (não podem coexistir no tempo do jogador):
`mazda_rookie ↔ toyota_rookie` e `mazda_amador ↔ toyota_amador`.

**Grafo de progressão** (`get_target_categories` e `get_feeder_categories`):

```
mazda_rookie  → mazda_amador ┐
toyota_rookie → toyota_amador ┤→ bmw_m2 ┐
                              └─────────┴→ gt4 → gt3 → endurance
        production_challenger ───────────→ gt4
```

### 5.1 As duas semânticas das categorias especiais (armadilha conhecida)

`production_challenger` e `endurance` exigem **dois predicados opostos** conforme o contexto. Eles
são inconfundíveis entre si:

- **Sentido fase e mercado** (`uses_regular_contracts` e `uses_regular_teams` = `config.is_some()`,
  **true** para as especiais): fase especial roda mercado de contrato regular e cria contrato por
  classe (a vaga de endurance recebe contrato regular com `categoria=gt3`). Usado em
  `market/pipeline`, `world/integrity` e `generators/world`.
- **Sentido validação-folha** (`is_especial` = **true**, logo a categoria é **excluída**): um
  contrato ou piloto rotulado com a meta-categoria literal `endurance` ou `production_challenger`
  é inválido. Pilotos são contratados no nível da classe (gt3, gt4, mazda), jamais na meta. Usado
  nos reparos de consistência de `commands/career.rs`, `career_detail.rs` e
  `global_driver_rankings.rs`.

> Nunca colapse `is_especial(x)` em `!uses_regular_contracts(x)`. Os dois têm valores diferentes
> para a mesma categoria e servem a perguntas diferentes.

---

## 6. Atributos do piloto

18 atributos (`models/driver_attributes.rs::DriverAttributeKey`), todos de 0 a 100:

| Atributo | Papel principal |
|---|---|
| `skill` | Ritmo bruto e talento geral |
| `consistencia` | Reduz variância de resultado (menos erro) |
| `racecraft` | Ultrapassagem e briga roda a roda |
| `defesa` | Defender posição |
| `ritmo_classificacao` | Performance na classificação |
| `gestao_pneus` | Reduz degradação de pneu (até −50%) |
| `habilidade_largada` | Peso alto no segmento de largada |
| `adaptabilidade` | Pista difícil e caráter de pista |
| `fator_chuva` | Performance no molhado |
| `fitness` | Reduz degradação física (segmentos finais) |
| `experiencia` | Maturidade, modula incidente |
| `desenvolvimento` | Potencial e curva de crescimento |
| `aggression` | Aumenta incidente e risco |
| `smoothness` | Reduz degradação de pneu (até −20%) |
| `midia` | Presença pública e repercussão |
| `carisma` | Atração de patrocínio e vínculo com a equipe |
| `mentalidade` | Peso nos segmentos Late e Finish |
| `confianca` | Peso no segmento Finish, sobe e desce com resultado |

A coluna `potencial` fica em `drivers` fora dessa lista: ela é o teto de crescimento, lido pela
evolução, sem entrar no cálculo da corrida.

A UI **não mostra número cru**. A leitura é por tag e badge (`DriverTags`, `PersonalityBadge`) e
por radar. Os números vivem só no motor.

---

## 7. Calendário e sistema temporal

### 7.1 Fases da temporada (`SeasonPhase`), modelo 9D

O macroestado de uma temporada percorre, em ordem:

```
PreTemporada (sw 1 a 9) → Temporada (sw 10 a 51) → Encerramento → (advance_season) → PreTemporada
```

- **PreTemporada:** mercado entre temporadas, 9 semanas (`MARKET_DURATION_WEEKS = 9`), de dezembro
  a fevereiro (sw 1 a 9). O jogador recebe e responde proposta, a IA assina contrato. Termina em
  `finalize_preseason`.
- **Temporada:** as 9 categorias correm em paralelo ao longo de 42 semanas (sw 10 a 51, de
  fevereiro a novembro). O calendário tem exatamente **74 entradas** geradas por
  `build_full_season_calendar`. Sem corrida pendente, `advance_season` leva a Encerramento.
- **Encerramento:** `run_end_of_season` roda o pipeline completo: classificação final, licenças,
  arquivo de piloto e de equipe, evolução, rookies, promoção e rebaixamento, criação da próxima
  temporada e inicialização da PreTemporada.

**Fases legadas** preservadas no enum para save antigo, nunca emitidas em save novo:
`BlocoRegular`, `JanelaConvocacao`, `BlocoEspecial`, `PosEspecial`. Com o colapso das migrações na
baseline v53 (§23.1), save anterior a v53 é recusado, então na prática essas fases só sobrevivem
como código.

**Eixo `season_week` (sw):** `sw = week_of_year + 4`. A janela de mercado é sw 1 a 9 e a janela de
corridas é sw 10 a 51.

### 7.2 Calendário da temporada (`build_full_season_calendar`)

74 entradas por temporada, todas com `season_phase = Temporada` e `season_week` entre 10 e 51:

| Categoria | Rodadas | woy início | woy fim | sw início | sw fim |
|---|---|---|---|---|---|
| mazda_rookie | 5 | 6 | 46 | 10 | 50 |
| toyota_rookie | 5 | 7 | 47 | 11 | 51 |
| mazda_amador | 8 | 6 | 46 | 10 | 50 |
| toyota_amador | 8 | 7 | 47 | 11 | 51 |
| bmw_m2 | 8 | 6 | 47 | 10 | 51 |
| production_challenger | 10 | 6 | 47 | 10 | 51 |
| gt4 | 10 | 6 | 47 | 10 | 51 |
| gt3 | 14 | 6 | 47 | 10 | 51 |
| endurance | 6 | 6 | 45 | 10 | 49 |

Invariantes garantidos por `tests_9d.rs`: exatamente 74 entradas, todas `Pendente`, zero LMP2 no
calendário (LMP2 é classe dentro de endurance), production e endurance sem rodada duplicada, e uma
única temporada `EmAndamento`.

`calendar/full_season/parcial.rs` gera o calendário parcial quando a temporada entra em voo já
começada.

### 7.3 Unidade temporal interna: `week_of_year`

Toda ordenação e lógica temporal usa **semana do ano** (1 a 52). As datas visíveis (`display_date`)
são derivadas da semana, só para UI, notícia e narrativa.

`SeasonTemporalSummary` (`models/temporal.rs`) é o DTO que a UI consome: fase atual,
`effective_week`, data de exibição, próximo evento do jogador, dias até esse evento e número de
corridas pendentes na fase.

### 7.4 `CalendarEntry`

Cada evento carrega categoria, rodada, pista (`track_id`, `track_name`, `track_config`), clima,
temperatura, umidade, vento, voltas, `duracao_corrida_min`, `duracao_classificacao_min`, `status`
(`Pendente` ou `Concluida`), `week_of_year`, `season_week`, `season_phase`, `display_date` e
`thematic_slot`.

> Armadilha registrada: `duracao_corrida_min = 0` é a **sentinela de endurance**, e não uma
> duração ausente. Quatro pontos de leitura da config testam o gate de enduro a partir desse
> campo. Ver [divida-tecnica.md](divida-tecnica.md) e o backlog antes de mexer.

### 7.5 Slots temáticos (`ThematicSlot`)

Papel narrativo fixo e imutável atribuído na geração do calendário, independente do resultado e da
importância calculada. Grupo regular: `AberturaDaTemporada`, `RodadaRegular`, `VisitanteRegional`,
`MidpointPrestigio`, `TensaoPreFinal`, `FinalDaTemporada`. Grupo especial: `AberturaEspecial`,
`RodadaEspecial`, `FinalEspecial`. Fallback explícito: `NaoClassificado`.

Pistas vêm de `constants/tracks.rs`, com pistas fixas e variáveis por categoria, chance de chuva
por pista e duração de classificação. Algumas categorias usam apenas pista gratuita do iRacing.

---

## 8. Motor de simulação de corrida

Pipeline (`simulation/engine.rs::run_full_race_with_breakdowns`):

```
simulate_qualifying → simulate_race_with_breakdowns (5 trechos) → determine_fastest_lap → assign_points
```

### 8.1 A moeda da corrida é TEMPO

Esta é a reforma mais importante do motor, e o ponto em que o retrato anterior deste documento
ficou velho. O motor acumulava **pontos** e ordenava no fim. Sem gap entre carros não existe ar
sujo, trem de carros, undercut nem safety car, e a única alavanca sobre o resultado era mexer no
dado.

Hoje `RaceState` acumula **milissegundos**. O score composto de `race/pontuacao.rs` continua sendo
o cérebro, e produz um **ritmo de volta** em vez de um saldo de pontos. As constantes da tradução
estão em `constants/scoring.rs` e vieram do modelo antigo por equivalência exata:

| constante | valor | papel |
|---|---|---|
| `MS_POR_PONTO_DE_RITMO_POR_VOLTA` | `RACE_SCORE_TO_LAP_MS × 5` = 150 | quanto vale um ponto de ritmo, em ms por volta |
| `RITMO_DE_REFERENCIA` | 100.0 | carro de referência teórico, cancela no resultado publicado |
| `ATRASO_LARGADA_MS_POR_POSICAO_POR_VOLTA` | `2 × 30` = 60 | custo de cada posição de grid, em ms por volta |
| `RITMO_PERDIDO_POR_POSICAO_EM_INCIDENTE` | 2.0 | ritmo perdido por posição num incidente |
| `QUALI_SCORE_TO_LAP_MS` | 50.0 | converte gap de score de classificação em ms |

O ×5 em `MS_POR_PONTO_DE_RITMO_POR_VOLTA` é fidelidade, e não calibragem nova: o modelo antigo
somava o score de cada um dos 5 trechos a um total único, como se o ritmo daquele trecho valesse a
corrida inteira. Com o tempo de um trecho valendo `ritmo × voltas_do_trecho`, o ×5 devolve a mesma
magnitude.

### 8.2 Classificação

`quali_score` único por piloto (escala aproximada de 55 a 85, com peso de `ritmo_classificacao`,
skill, carro e clima), convertido em ms por `QUALI_SCORE_TO_LAP_MS`. Define o grid.

### 8.3 A corrida em 5 trechos

Trechos: **Start, Early, Mid, Late, Finish**. Cada piloto tem um `RaceState` com desgaste de pneu,
condição física, tempo acumulado, posição, incidentes e dano latente.

Pesos por trecho no `segment_score`:

| Trecho | skill | largada | racecraft | carro | pneus | fitness | mentalidade | confiança |
|---|---|---|---|---|---|---|---|---|
| Start | .20 | **.35** | .25 | .20 | | | | |
| Early | **.35** | | .20 | .30 | .15 | | | |
| Mid | **.35** | | | .30 | .20 | .15 | | |
| Late | .25 | | | .20 | **.25** | .20 | .10 | |
| Finish | .25 | | .25 | .20 | | | .10 | **.20** |

Modificadores aplicados ao score do trecho:

- Penalidade de pneu: `(1 − tire_wear) × 0.15`.
- Penalidade de fadiga (só Late e Finish): `(1 − physical) × 0.10`.
- Clima: multiplicador por `fator_chuva` e sensibilidade do contexto.
- Pista difícil: bônus de `adaptabilidade` e `consistencia`.
- Caráter de pista (`Flowing`, `Technical`, `Tight`, `Roval`): pequenos vieses em skill, carro e
  adaptabilidade.
- Amplitude de ritmo: comprime ou abre o campo em torno de 60 (endurance fecha, rookie abre), com
  relançamento quando o campo fica achatado demais.
- Inexperiência: penalidade se `corridas_na_categoria < 10`.
- **Forma do momento** (`simulation/forma.rs`): um AR(1) persistido na coluna `drivers.forma`, com
  afinidade de pista e acerto de fim de semana saindo de hash determinístico.
- Variância: `(100 − consistencia) / 100 × 5`, escalada pelo perfil, com caos extra na largada
  (densidade do pelotão vezes `start_chaos`) e correlação de ruído entre trechos de 0.5.

### 8.4 Tráfego, estratégia e danos

O que só passou a existir depois que a moeda virou tempo (`simulation/race/`):

- **`trafego/`**: ar sujo (`perda_por_ar_sujo` dentro de `JANELA_AR_SUJO_MS`), tentativa de
  ultrapassagem (`tentar_ultrapassagem` dentro de `JANELA_DE_ATAQUE_MS`), custo da tentativa
  falha para atacante e defensor, `GAP_MINIMO_ENTRE_CARROS_MS`, e margem de ataque modulada por
  rivalidade do par.
- **`estrategia/`**: plano de paradas (`planejar_paradas`), `CUSTO_DE_PARADA_MS`, safety car
  (`traz_bandeira_amarela`) e o desconto do custo de box sob safety car
  (`FRACAO_DO_CUSTO_SOB_SAFETY_CAR`).
- **`danos.rs`**: dano latente pós-contato, detalhado em §10.

O contato de disputa registra incidente e pode avariar o carro. Ele **não** soma posição perdida:
o custo da tentativa falha já foi cobrado em tempo, e somar posição cobraria o mesmo evento duas
vezes.

### 8.5 Resultado

- Finishers ordenados por tempo acumulado. DNFs ordenados por trecho de abandono (mais tarde fica
  melhor classificado entre os DNFs).
- `gap_to_winner_ms ≥ 0`, ancorado no vencedor.
- Voltas no DNF estimadas pela fração do trecho (Start 10% até Finish 90%).
- Campos narrativos agregados: `total_incidents`, `total_dnfs`, `main_incident_count`,
  `notable_incident_pilot_ids`, `most_positions_gained_id`.
- `race_weekend_readings` guarda a leitura do fim de semana e `player_race_telemetry` a
  telemetria do jogador, para o painel "o curso da corrida" e o debrief por IA.

O motor é **determinístico por seed** (`StdRng`), o que torna o teste reproduzível. `sim_stats/`
existe para instrumentar a própria simulação: métricas, snapshots, ciclo e experimentos de
calibragem, com harness de medição em `simulation/calibracao/`.

---

## 9. Sistema de pontuação

`constants/scoring.rs`:

- **Padrão** (P1 a P10): `25, 18, 15, 12, 10, 8, 6, 4, 2, 1`.
- **Endurance** (P1 a P10): `35, 28, 23, 19, 16, 13, 10, 7, 4, 2`.
- **Volta mais rápida:** +1, só para quem termina no top 10 sem DNF.
- Bônus overall (1º +5, 2º +3, 3º +1) para uso multiclasse e agregado.
- DNF vale 0.

**Dificuldade** (faixa de skill da IA gerada): Fácil 20 a 60, Médio 30 a 80, Difícil 50 a 90,
Lendário 70 a 100.

**Clima**, penalidade base e multiplicador de dificuldade: Dry (0.00, 1.00), Damp (0.06, 1.15),
Wet (0.12, 1.35), HeavyRain (0.18, 1.60). Quando chove, a intensidade sai de Damp 40%, Wet 40% e
HeavyRain 20%.

> Medição registrada: a chuva chega ao jogador em torno de 10% das corridas, e nas corridas só de
> IA o `AI_WET_BIAS` de 2.0 dobra a frequência. Medir taxa de chuva exige separar os dois
> universos.

---

## 10. Incidentes, quebras, danos e lesões

### 10.1 Incidentes (`simulation/incidents/` e `incident_catalog`)

Com `incidents_enabled`, cada trecho processa rolls de incidente por piloto, influenciados por
`aggression`, `consistencia`, `experiencia`, clima, decisão de campeonato, densidade do pelotão e
confiabilidade do carro. Um incidente pode custar posição ou virar DNF.

O catálogo (`incident_catalog`, semeado pela baseline) é parametrizado por classe de veículo,
formato (sprint ou endurance), fonte e tipo de gatilho, com peso separado por formato.

O texto da narrativa **não** fica no banco. Desde a migração v65 a tabela guarda chave de i18n
(`dnf_key`, `non_dnf_key`, `description_key`, todas na forma `breakdown.<id>.{dnf|warn|part}`) e a
frase sai de `locales/*.yml` no locale ativo, na hora de apresentar. O que já foi gravado num
incidente antigo continua no idioma da corrida — o save guarda prosa renderizada, e retraduzir o
histórico é decisão em aberto, registrada no topo de `simulation/catalog.rs`.

### 10.2 Dano latente pós-colisão (`PendingDamage`)

Colisão gera dano latente que se manifesta em trecho posterior: a cada trecho há uma
`manifest_chance` que cresce 0.15 se não manifestar, e ao manifestar há 70% de chance de virar DNF
se o dano for `dnf_capable`. Modela falha mecânica tardia originada de toque anterior.

O contato de disputa entra no mesmo mecanismo com `is_dnf_capable: false`: um encostão custa
posição dentro da corrida e desgaste depois dela (`car::crash::apply_contact_wear`, aplicado na
manutenção pós-corrida), e não tira o carro sozinho.

### 10.3 Quebra de peça (`car/`)

Módulo próprio, e a fonte única do desfecho mecânico. `car/parts.rs` descreve as peças,
`car/wear.rs` o desgaste acumulado, `car/breakdown.rs` a quebra e `car/cost.rs` o custo de
conserto. O desfecho é **pré-rolado** e entra na corrida antes de `build_race_results`, então
posição, gap e pontos já saem coerentes com o tempo perdido no box, sem remendo depois.

Passar `mechanicals` para o motor também **desliga** a pane genérica do catálogo de incidentes,
para a corrida ter uma fonte só de falha mecânica.

`race_breakdowns` persiste a quebra por corrida, e `get_breakdown_forecast` e
`get_grid_breakdown_risk` levam a previsão ao briefing pré-corrida.

> Sobrecusto de enduro registrado como não calibrado: o modelo é linear e sem teto, chegando a
> 3,8x a 12,2x nas durações reais e a desgaste 5x além do fim de vida da peça. Ver
> [divida-tecnica.md](divida-tecnica.md).

### 10.4 Lesões (`injuries`, `evolution/injury.rs`)

Tipos `Leve`, `Moderada`, `Grave` e `Critica`. A lesão carrega `modifier`, `skill_penalty`,
`races_total` e `races_remaining`, e fica ativa por N corridas. **Máximo de uma lesão ativa por
piloto**, garantido por índice único. Enquanto ativa muda o `status` do piloto para `Lesionado` e
afeta a performance.

### 10.5 Histórico de DNF por pista (`track_dnf_history`)

Registra DNF por piloto e pista, com motivo e com quem houve colisão. É a base narrativa da
redenção na volta àquela pista.

---

## 11. Evolução de pilotos (`evolution/`)

Roda na transição de temporada. Submódulos:

- **growth:** crescimento de atributo, modulado por idade, `desenvolvimento`, `potencial` e
  resultado.
- **decline:** declínio por idade avançada.
- **experience:** ganho de `experiencia` por corrida disputada.
- **motivation:** `motivacao` sobe e desce contra a expectativa. Motivação baixa sustentada
  (`temporadas_motivacao_baixa`) tem consequência no mercado e na aposentadoria.
- **injury:** recuperação e expiração de lesão.
- **licenses:** concessão de licença ao cumprir o requisito da categoria.
- **retirement:** aposentadoria por idade, declínio ou motivação. O piloto vai para `retired`.
- **rookies:** entrada de piloto novo para repor vaga.
- **standings:** consolidação da classificação.
- **season_transition:** o orquestrador. Cria a temporada nova e arquiva o snapshot completo de
  cada piloto (`driver_season_archive`) depois do crescimento e antes da promoção.

A ordem importa: crescimento de atributo **antes** do arquivamento, e arquivamento **antes** da
promoção e do rebaixamento, para capturar atributo final e categoria original.

---

## 12. Mercado de transferências (`market/`)

Roda na pré-temporada. Componentes:

- **team_ai:** cada equipe decide quem quer contratar (necessidade por papel N1 e N2, orçamento,
  reputação, encaixe na categoria).
- **driver_ai:** cada piloto decide aceitar ou recusar, com motivo tipado (`RefusalReason`:
  `SalarioBaixo`, `EquipeFraca`, `CategoriaErrada`, `BloqueioHierarquico`, `PreferenciaPessoal`).
- **evaluation:** valoração do piloto (desempenho recente, idade, atributos).
- **proposals:** emissão e resposta de `market_proposals`.
- **renewal:** renovação de contrato existente.
- **preseason** (`preseason.rs` + `preseason/`): orquestra a janela semana a semana — estado,
  plano, eventos, expectativa, sincronização.
- **pipeline** (`pipeline.rs` + `pipeline/`): as etapas com banco — vagas, contratação,
  consolidação de N1/N2, assédio, slam, e `janela.rs`, que é o wiring do leilão.
- **transfer_window** (`transfer_window.rs` + `transfer_window/`): o **motor puro** do leilão
  semanal de dois lados (ofertas → respostas → resultados → rollover), sem banco. Quem lê vaga e
  piloto do banco e aplica as assinaturas é o `pipeline::janela`.
- **poaching:** assédio a piloto sob contrato, com leilão que chega ao jogador via
  `PoachAuctionHost` montado global no `App.jsx`.
- **bond:** vínculo de longo prazo por par (piloto, equipe), 0–100, que cresce a cada temporada
  juntos e decai devagar quando separados. Nesta fase só ACUMULA e expõe o selo qualitativo de 6
  níveis; as consequências (renovação leal, segurar-vs-vender) não estão ligadas.
- **slam_ambition:** decide se um piloto de elite persegue um slam de prestígio e que categoria
  ele quer na próxima temporada. Lógica pura; quem leva a preferência ao mercado é o chamador.
- **visibility:** o que o jogador enxerga do mercado.
- **sync:** os contratos regulares ativos mandam; `piloto_1_id`/`piloto_2_id` da equipe e o
  `categoria_atual` do piloto obedecem. O que não couber nessa regra é rescindido aqui.
- **car_maintenance** (`car_maintenance.rs` + `car_maintenance/`): o cérebro de manutenção do
  carro por corrida (trocar / esticar / degradar, e quando subir de nível), dentro do caixa e
  olhando o calendário à frente. Substituiu o `car_build_strategy` de perfil discreto, que **não
  existe mais no crate** — ver §10.3.
- **pit_strategy:** risco de estratégia de box por equipe (`pit_strategy_risk`), derivado do plano
  financeiro e do teto por categoria. É ele que alimenta o `strategyRiskiness` do roster do
  iRacing.

O jogador recebe proposta (`get_player_proposals` e `respond_to_proposal`), acompanha interesse
(`get_player_interests`) e avança a janela semana a semana (`advance_market_week`) até
`finalize_preseason`. O estado da janela de transferências (tabela `transfer_window`) é **de
leitura** para a UI, por `get_transfer_window_state`; não há comando de condução. O
`advance_transfer_window` que existia era um no-op — corpo idêntico ao do getter, parâmetro
ignorado — e foi removido em 11/08/2026 (o registro está em
[divida-tecnica.md](divida-tecnica.md)).

---

## 13. Convocação e bloco especial (`convocation/`)

As categorias especiais (Production e Endurance) não têm elenco fixo de temporada inteira: o grid é
montado convocando piloto das categorias feeder.

| Categoria especial | Classe | Feeder |
|---|---|---|
| production_challenger | mazda | mazda_amador |
| production_challenger | toyota | toyota_amador |
| production_challenger | bmw | bmw_m2 |
| endurance | gt4 | gt4 |
| endurance | gt3 | gt3 |
| endurance | lmp2 | (classe de referência) |

Pipeline: **eligibility** coleta candidato elegível por fonte e licença, **quotas** calcula a cota
por classe e equipe, **scoring** pontua para preencher a vaga, **player_offers** gera a oferta
especial ao jogador (`player_special_offers`), e **special_window** é a máquina de estado diária da
janela (tabelas `special_window_*`).

Contrato especial tem `tipo = Especial` com `classe` preenchida, e expira no `PosEspecial`.

> **O módulo `convocation/` está vivo**, e o texto que o chamava de legado estava errado: os 10
> comandos do bloco especial estão registrados e o `seasonSlice.js` chama todos. O que é legado
> são as quatro **fases** `BlocoRegular`, `JanelaConvocacao`, `BlocoEspecial` e `PosEspecial`, e o
> próprio código diz isso em `models/enums/temporada.rs::is_legacy()`. No modelo 9D a temporada vai
> de `PreTemporada` a `Temporada` a `Encerramento`, e `advance_to_convocation_window` exige
> `BlocoRegular`, que o fluxo 9D nunca grava (`tests_9d.rs::assert_no_legacy_phases` cobra isso em
> quatro pontos). O item D-01 do [backlog.md](backlog.md) tem a apuração completa.

---

## 14. Promoção e rebaixamento (`promotion/`)

Mantém o tamanho fixo de cada categoria movendo **equipes** entre tiers no fim da temporada, e não
piloto individual.

- `PromotionResult` = `movements` (`TeamMovement`: `Promocao` ou `Rebaixamento`) + `pilot_effects`
  + `attribute_deltas` + `errors`.
- Efeitos sobre piloto (`PilotEffectType`): `MovesWithTeam`, `FreedNoLicense` (liberado por não ter
  licença para a categoria nova) e `FreedPlayerStays` (o jogador permanece por escolha).
- `TeamAttributeDelta` ajusta `car_performance`, `budget`, `facilities`, `engineering`, `morale` e
  reputação ao promover ou rebaixar.

Organizado em `block1`, `block2`, `block3`, mais `standings` (apuração), `effects` e `pipeline`.
`team_promotion_history` guarda o registro.

---

## 15. Hierarquia de equipe e rivalidades

### 15.1 Hierarquia interna N1/N2 (`hierarchy/`)

Cada equipe tem um piloto Número 1 e um Número 2. O sistema rastreia `hierarquia_tensao`, duelos
totais, duelos vencidos pelo N2, sequências e inversões na temporada. O núcleo do eixo (os deltas de
cada rodada) vive isolado em `hierarchy/tensao.rs`, sem dependência do resto do crate, justamente
para poder ser medido; o pipeline pós-corrida e a inversão ficam em `hierarchy/orders.rs`, e a
invariante de fim de pré-temporada em `hierarchy/transition.rs`.

> Medição registrada (11/08/2026, harness `calibracao_do_eixo_de_tensao`): o equilíbrio do eixo — a
> taxa de vitórias do N2 em que a tensão para de cair — **não** era os 0,40 da conta ingênua, porque
> ela ignora os bônus de sequência, que são assimétricos. O real era **0,420**, acima de toda a faixa
> que o mundo produz (o N2 leva de 0,227 a 0,325 dos duelos), então nenhuma dupla montável tinha
> deriva positiva. Baixando `TENSAO_DELTA_N1_VENCE` de 2,0 para 0,5, o equilíbrio caiu para **0,308**
> e o eixo passou a se mover: numa temporada de 14 corridas, 56% das duplas 50/50 saem de "estável".
>
> O que **continua** morto é o gatilho de inversão, e não por calibração: ele exige status Crise
> (tensão ≥ 90) e o teto de uma temporada perfeita do N2 é `14×3 + 10 + 15 = 67`, com a virada
> zerando tudo. Ligar a inversão exige decisão de produto — baixar o limiar de Crise ou deixar a
> tensão atravessar a virada.
>
> **Bloqueio a montante, medido no banco do harness (11/08/2026):** no mundo do rascunho
> histórico, as 102 equipes ativas estavam com `hierarquia_n1_id`/`n2_id` apontando para pilotos
> sem contrato ativo na própria equipe. Com a dupla defasada o duelo sai inválido e a tensão só
> decai — só 12 equipes tinham algum duelo registrado. Quem realinha é
> `validate_and_normalize_team_hierarchies`, e o único chamador de produção é a pré-temporada
> **jogável** (`commands/career/market_window.rs`); o caminho histórico não passa por lá. Antes
> disso, nenhum harness de mundo consegue medir este eixo.

### 15.2 Rivalidades (`rivalry/`)

Modelo **dual**: `historical_intensity` (calor acumulado de longo prazo) e `recent_activity`
(atividade recente, que decai). `temporada_update` sustenta a decisão de decaimento. O par de
pilotos é único por índice. Rivalidade nasce de briga roda a roda, colisão e disputa de título, e
alimenta notícia, calendário, análise de corrida e a UI de histórico. `rivalry_episodes` guarda o
episódio e `player_nemesis` o rival principal do jogador.

Do lado do iRacing existe a **percepção de rivalidade de pista**, que alimenta o ledger do grid
inteiro, inclusive IA contra IA (§19).

---

## 16. Finanças e economia

Duas camadas convivem hoje, e a distinção importa.

### 16.1 `finance/`, o caixa da equipe

`economy` (parâmetros do mundo), `salary`, `prize`, `cashflow`, `events`, `planning`, `state`,
`morale`, `reputation`, `focus`, `rescue` e `strategy`. O `budget` da equipe condiciona o que ela
paga no mercado e o quanto investe em performance. `team_finance_history`, `team_focus`,
`team_strategic_plan`, `team_collapse_streak`, `team_rescue_counters` e `team_ownership_events`
persistem esse estado.

### 16.2 `economia/`, o modelo novo

Escrito do zero ao lado do anterior, com o desenho completo em
[economia-redesign.md](economia-redesign.md). Peças: `ancora` (a âncora de escala), `receita`
(`ParametrosDeReceita`), `evento::fatura_da_etapa` (o custo da etapa concreta),
`temporada::fatura_de_temporada` (a manutenção estrutural, sem a folha de pilotos),
`desenvolvimento` (melhoria de estrutura) e `fatura`, que é a fatura visível ao jogador.

O consumidor é `commands/race/despesa.rs`, que monta a despesa da etapa, e `get_stage_invoice`, que
leva a fatura à tela. A decisão de reescrever veio depois que o harness
`commands::race::tests::medicao_financeira` passou a reproduzir um save real com 34 de 36 células
de validação dentro de 10%.

---

## 17. Interesse de evento, espectadores, presença e fama

### 17.1 Interesse de evento (`event_interest/`)

Ciclo fechado no backend:

- **Esperado** (`calculate_expected_event_interest`), pré-corrida, a partir de reputação da
  categoria, slot temático, disputa de título e estrelas no grid. Aparece no `NextRaceTab` como
  `EventInterestSummary`.
- **Realizado** (`RealizedEventInterest`), pós-corrida, gera delta de mídia e de motivação por
  piloto e pode elevar uma notícia a destaque (`public_impact`).

A UI dos dois lados do ciclo, fechada em 11/08/2026 (F-07):

- **antes da largada** — `components/race/EventInterestBanner.jsx`, no cabeçalho da Sala de
  Estratégia: tier (`tier_label`, já traduzido pelo backend), público (`display_value`), porte da
  ocasião e a cota de plateia que a estrela do jogador puxa (`public_fame_share`). Antes disso o
  público era um número solto no canto do card de clima, sem tier e sem escala. Nasceu como card na
  coluna de condições e virou faixa sem moldura no vão central do cabeçalho em 14/08/2026: ele é
  identificação da etapa, pelo mesmo critério que o nome da pista e a data, e como card empurrava
  risco de quebra e narrativa para baixo da dobra.
- **depois da bandeirada** — `RepercussionSegment` e `RepercussionCard` no `RaceResultViewV2`, sobre
  o `EventRepercussionSummary` que viaja em `event_repercussion` nos dois caminhos de resultado
  (simulação e importação do iRacing): esperado contra entregue, o delta e o `headline_strength`.

> Segue sem consumidor, e é decisão de equilíbrio e não de exposição: os três multiplicadores de
> `ExpectedEventInterest` (`pressure_modifier`, `media_multiplier`, `motivation_multiplier`) são
> calculados em `calculator.rs` e ninguém os lê.

### 17.2 Presença pública (`public_presence/`) e fama (`fame`)

Repercussão pública de equipe e de piloto, ligada ao atributo `midia`, com `atracao` e `medicao`.
O módulo `fame` deriva a fama do piloto a partir de mídia e resultado.

---

## 18. Notícias e narrativa

### 18.1 Notícia determinística (`news/`)

Feed gerado após cada corrida e em evento de mercado. Tipos (`NewsType`): `Corrida`,
`Contratacao`, `Lesao`, `Aposentadoria`, `Promocao`, `Rivalidade`, `Titulo`, `Incidente`. Dedup por
`chave_dedup` com índice único. A narrativa é contextual: usa histórico recente do piloto, como
sequência de vitória, rebaixamento e redenção em pista de DNF anterior.

`world_footer` gera as notas do mundo (`get_world_footer`), também determinísticas.

### 18.2 Narrativa por IA (`narrative/` e `commands/ai_news/`)

Camada opcional que reescreve o material determinístico com voz de revista. `narrative/tese.rs`
escolhe a tese, `beats.rs` a estrutura, `contexto.rs` e `consulta.rs` montam os fatos, e
`client.rs` fala com o proxy HTTP.

Comandos: `enrich_race_news_ai`, `pre_race_briefing_ai`, `post_race_debrief_ai`,
`enrich_world_footer_ai` e `enrich_season_preview_ai`. O resultado é persistido em
`ai_pre_race_briefing`, `ai_post_race_debrief`, `ai_race_story` e `ai_world_notes`.

O proxy usa dois provedores (DeepSeek fora do pico, Gemini no pico e como fallback). Quando o
endpoint não responde, o app cai no texto determinístico e nada quebra. As referências a piloto
dentro do texto de IA usam a marcação descrita em [ai-mention-tags.md](ai-mention-tags.md), para o
apelido continuar clicável.

### 18.3 Aba de Notícias

Revista (`NewsMagazineTab` e `components/news/`): capa, matéria da etapa, encarte de pré-temporada,
notas do mundo e caixa postal.

**Não há seletor de escopo.** A revista é sempre a da **categoria atual do jogador** — o recorte sai
de `playerTeam.categoria`, e nada na tela o troca. A navegação é por **edição**: as corridas com
`status = "Concluida"` do calendário daquela categoria, da mais recente para a mais antiga, com o
par de setas (`goEdition`). Enquanto nenhuma etapa foi concluída, a revista abre no encarte de
pré-temporada; se nem categoria houver, cai na capa fechada.

Cada painel tem seu `invoke`, todos nos hooks de [`useMagazineData.js`](../src/components/news/useMagazineData.js)
e todos caindo no vazio em qualquer falha, sem quebrar a página:

| painel | comandos |
|---|---|
| Edições e pista de abertura | `get_calendar_for_category` |
| Construtores e pilotos | `get_teams_standings`, `get_drivers_by_category` |
| Matéria da etapa (boletim de IA) | `player_race_news_id` → `enrich_race_news_ai` |
| Encarte de pré-temporada | `enrich_season_preview_ai` |
| Notas do mundo | `get_world_footer`, depois `enrich_world_footer_ai` por cima |
| Caixa postal | `get_inbox_messages` (no `MagazineMailbox`) |

O `get_news` — o feed determinístico da §18.1 — **não passa pela revista**. Quem o consome é o
mercado: `marketSlice.js` e `stores/career/helpers.js` o pedem com `limit: 400` e o passam por
`buildWeeksFromNews` para montar a linha do tempo semana a semana da pré-temporada.

---

## 19. Integração com iRacing

**A decisão de produto, tomada em 2026-07-27:** o Loop é uma **ferramenta de iRacing com uma
carreira simulada dentro**. Correr de verdade é o caminho principal, e a simulação preenche o que o
jogador não corre. O levantamento completo está em [iracing-escopo.md](iracing-escopo.md), e o que
a telemetria entrega de fato em
[iracing-dados-disponiveis.md](iracing-dados-disponiveis.md).

A integração é um **ciclo fechado**:

```
carreira → exporta AI roster + AI season para Documentos/iRacing/
        → o jogador corre a etapa no iRacing
        → resultado oficial (JSON do aiseason) + sinais do monitor ao vivo (~60 Hz)
        → importa para a carreira (iracing_auto_import_if_ready)
        → resultado, telemetria, quebras, rivalidade de pista (inclusive IA contra IA)
```

> Lição de leitura registrada aqui porque já enganou duas varreduras: os módulos `export/` e
> `commands/export.rs` foram deletados, e a exportação **mudou de casa** para
> `iracing_sdk/roster_gen.rs` e `season_gen.rs`. "Módulo deletado" não significa "feature
> removida".

### 19.1 Antes da corrida

`src/components/race/nextrace/useIracingExport.js` faz, num botão só: `iracing_generate_roster`,
`iracing_generate_season`, `iracing_install_yellow_macro` e `iracing_modo_janela_aplicar` (os dois
últimos best-effort, aproveitando que o sim está fechado e a escrita nos `.ini` cola). Depois
oferece `iracing_focus_window` e, com o sim fechado, `iracing_launch_ui`.

O roster carrega a identidade da carreira: os atributos do piloto viram `driverSkill`,
`driverAggression`, `driverOptimism` e `driverSmoothness`, a equipe vira `pitCrewSkill` e
`strategyRiskiness`, e cor de carro, macacão e capacete saem da paleta da equipe. A temporada
carrega o calendário com clima por evento (keyframes dinâmicos, versão 3).

`iracing_auto_paint_player` pinta o carro do jogador na cor da equipe e vincula o `custid` ao save.
Roda sem perguntar porque o `car_<custid>.tga` é local e a pintura anterior fica preservada em
`.tga.loop-bak`. O interruptor é `auto_paint_car`, em Configurações.

Briefing: `get_breakdown_forecast`, `get_grid_breakdown_risk` e `get_weekend_modifiers`.

### 19.2 Durante a corrida

O amostrador de fundo alimenta o monitor unificado. `IracingConnectedOverlay` fica montado no
`MainLayout`, com `iracing_connected` a 1 Hz, `iracing_get_race_feedback` e `iracing_car_colors`.

Race control automático: `iracing_set_auto_yellow` e `iracing_auto_yellow_enabled` ligam o disparo
de bandeira amarela pelo próprio monitor, via macro no `app.ini`. `iracing_throw_yellow` é a versão
manual. O chat de texto livre é `iracing_send_chat_text`.

### 19.3 Depois da corrida

`Dashboard` chama `iracing_focus_self_if_closed` em laço: quando o sim fecha, a janela do Loop
volta para frente sozinha. No mesmo ritmo o `raceSlice` chama `iracing_auto_import_if_ready`, que:

- lê o **resultado oficial** do JSON do aiseason, sem reconstruir ao vivo;
- confere que a pista bate com a que foi exportada;
- sobrepõe o que o iRacing não sabe e o monitor sabe: batida do jogador (severidade vira custo de
  conserto), DNF real, direção do impacto, estilo de pilotagem, quebra de peça e quem bateu em
  quem;
- persiste na carreira e devolve resultado e resumo para a tela abrir sozinha;
- aplica a percepção de rivalidade de pista, alimentando o ledger do grid inteiro;
- chama `iracing_process_race_result` best-effort, que é a **dificuldade adaptativa**: atualiza o
  perfil do jogador por `custid`, global e por pista, depois de cada corrida limpa. O
  `ai_sweet_spot` lê esse perfil para ancorar a curva de skill da IA na geração seguinte.

**Ligado não é o mesmo que executado, e a diferença é auditável.** A chamada existe em
[`importacao.rs:138`](../src-tauri/src/commands/iracing/importacao.rs) e depende do auto-import
fechar; o `Err` é engolido para nunca desfazer o import. O que prova que o ciclo rodou é o par de
linhas no `loop.log` (`%APPDATA%\com.loop.app\logs\`):

```
[import]      Corrida importada: <race_id> (pista <track_id>)
[adaptativo]  Pista <id> · classe <c>: N IA de M carros · carro sim|não ·
              ritmo vs frente +0,42%/volta · <veredito> ·
              global 0+1=1 · pista 0+1=1 · gravado
```

A linha do adaptativo sai **sempre** que ele roda, mesmo sem mexer a agulha — termina em `gravado`
ou `sem mudança`, e essa distinção é o ponto: arquivo de perfil ausente é ambíguo entre "nunca
rodou" e "rodou e ficou no escudo". Falha vira `[adaptativo] Sem ajuste: <motivo>`, e a causa mais
comum é o monitor sem histórico vivo (app reaberto entre correr e importar). **Sem o par de linhas
num log de corrida real, o perfil por `custid` continua zerado e o `ai_sweet_spot` ancora em
nada** — é medição, não código.

### 19.4 Manutenção e diagnóstico

`iracing_apply_market_paint` reaplica a pintura quando o piloto troca de equipe. Em Configurações:
`IracingDiagnosticoPanel` (`iracing_diagnostico`, `iracing_log_ler`, `iracing_log_revelar`,
`iracing_log_enviar`), `RivalryPerceptionPanel` (salvar, listar e carregar corrida, mais
`iracing_perceive_rivalries`) e os armadores de quebra de teste.

O log rotativo vive em `%APPDATA%\com.loop.app\logs\loop.log` (`diagnostico.rs`) e o contrato do
envio está em [log-endpoint.md](log-endpoint.md).

### 19.5 Ressalva de plataforma

Tudo acima é Windows. Fora do Windows o `imp/stub.rs` compila no lugar da winapi e a integração é
inerte por construção. "Não funciona no Linux" é o desenho, e não um bug.

### 19.6 Dívida conhecida da área

`RosterGenPanel` (726 linhas) e `PostRacePanel` (696) continuam sem nenhum importador em `src/`.
São a bancada de diagnóstico anterior, e o caminho principal do jogador não passa por eles.
Segue pendente a decisão de ligar ou apagar. O inventário dos comandos sem consumidor está em D-05
no [backlog.md](backlog.md) e congelado pelo guard
[`invoke-contra-generate-handler`](../scripts/tests/invoke-contra-generate-handler.test.mjs).

---

## 20. Engenheiro de pista e spotter

Duas vozes que falam com o jogador durante a corrida real. É a área mais nova do app e a que mais
cresceu desde o retrato anterior.

### 20.1 Engenheiro (`engenheiro/` e `src/lib/`)

O backend decide **o que** falar: `fatos.rs` reúne o que aconteceu, `intencao.rs` escolhe a
intenção, `momento.rs` o instante, `fala.rs` monta a frase, e `tratamento.rs` resolve como chamar o
piloto. Áreas próprias para `quebra/` (peça quebrada), `ritmo/`, `tempo_volta/`,
`volta_referencia/`, `classificacao/`, `campeonato/`, `vizinhanca.rs` e `memoria.rs`.

O frontend toca: `src/lib/engenheiroVoz.js` mantém a fila, `filaDeCarga.js` a carga do áudio,
`filtroRadio.js` o filtro de rádio, `volumeRadio.js` o volume e `pausasDoRadio.js` as pausas.

A voz é um **pacote pré-gravado montado por colagem**, e não geração ao vivo. O caminho e o porquê
estão em [tts-poc-latencia.md](tts-poc-latencia.md), o inventário de peças em
[pack-de-voz-inventario.md](pack-de-voz-inventario.md) e o briefing das frases em
[pack-de-voz-briefing.md](pack-de-voz-briefing.md). O acervo está catalogado em
[engenheiro-catalogo.md](engenheiro-catalogo.md) e no JSON irmão.

**Push to talk:** o jogador fala com o engenheiro. `commands/ptt.rs` captura o gatilho,
`ptt_voz.rs` transcreve e responde, `volante.rs` lê o botão do volante, e
`engenheiro_responder` monta a resposta.

> **`SessionState = Racing` não quer dizer corrida.** Ele vale igual em treino livre e
> classificação, e quem responde "que sessão é esta" é o `SessionNum` cruzado com o
> `SessionInfo:Sessions` do YAML. Até 17/08/2026 `EstadoAgora::em_corrida` era só o estado, e o
> efeito estava no rádio: no fim de uma classificatória em Oschersleben, com o jogador sem ter
> marcado tempo, o engenheiro abriu com "Novato, que corrida". Hoje `em_corrida` cruza os dois e
> `EstadoAgora::tipo_sessao` devolve `corrida`, `classificacao` ou `treino`, que vira a primeira
> linha do dossiê enviado ao modelo. A linha é **afirmativa** de propósito: "não é corrida"
> deixava o modelo escolher entre treino e classificatória, e ele errava metade das vezes.
>
> Fica em aberto a peça gravada `tv_melhor_e_do` ("A volta mais rápida **da corrida** é do"),
> que o canal de ritmo toca também na classificatória. Corrigir o texto exige regravar o áudio.
>
> **`PaceMode` não diz se o campo está em formação.** Nas sessões com IA ele vale 4
> (NotPacing) a classificação inteira e fica preso em 1 a 3 a corrida inteira — medido em
> Okayama e Oschersleben (17/08/2026). A leitura antiga (`em_formacao = pace_mode > 0`) deixava
> `em_formacao` verdadeiro quase sempre, e ele é o portão de `em_corrida_de_verdade`: **~820
> peças gravadas do acervo de resposta (21% do total) nunca tocavam**, toda resposta de PTT
> caía no modelo, o briefing de "antes da largada" saía dentro da classificação e o dossiê
> dizia "volta de formação" no meio da prova. Hoje a formação é uma TRAVA no monitor
> (`verde_da_tentativa`): sessão de corrida antes do primeiro `live_is_green` da tentativa,
> rearmada no reinício e no salto de replay. A amarela do meio da prova não a ressuscita, por
> memória e por teste.

### 20.2 Spotter (`iracing_sdk/spotter*.rs`)

Um módulo por assunto: `spotter_frente`, `spotter_tras`, `spotter_lento`, `spotter_bandeira`,
`spotter_boxe`, `spotter_clima` e `spotter_voltar`, com `spotter_control.rs` no comando.
`iracing_spotter_status`, `iracing_spotter_set`, `iracing_spotter_restore` e
`iracing_spotter_vizinhanca` são a ponte.

Duas regras de escrita registradas: a fala do spotter **nunca elide o referente** (dizer "carro
fora" soa como "você está fora", e o sujeito implícito é sempre o piloto), e a carga somada das
quatro famílias que abrem o canal sozinhas foi medida em [radio-carga.md](radio-carga.md). O que
falta na captura para o aviso de obstáculo à frente está em
[spotter-obstaculo.md](spotter-obstaculo.md).

#### O diário: por que ele ficou calado

`iracing_sdk/spotter_diario.rs` grava a **recusa**. Os detectores decidem a 60 Hz e só o SIM
deixava rastro: uma família que passa minutos perdendo a arbitragem do tique para o lateral
produzia o mesmo arquivo que produziria se estivesse morta.

O que entra: o candidato que a regra descartou, com o motivo e a **folga** que faltou para o
limiar (`longe`, `cedo`, `tarde` no `spotter_frente`; `ritmo_ok`, `sem_perseguidor`,
`campo_sem_ritmo`, `saida_de_box` no `spotter_tras`), os portões de sessão, e quem perdeu o tique
nas sete famílias. O que fica de fora é o tique comum, em que nada foi visto: a corrida inteira
já está em `race_capture.rs`.

Três decisões que sustentam o custo:

- **Dedup por transição**, com piso de 0,5 s por par (família, alvo). A mesma recusa se repete
  milhares de vezes enquanto a geometria não muda.
- **Nada de I/O no tique.** `nota()` enfileira; quem escreve é `escoar()`, uma vez por amostra e
  fora de todo lock de observador.
- **O relógio do salto é o do tique**, jamais o intervalo entre notas. As notas são esparsas por
  construção, e medir o salto nelas fazia duas recusas legítimas distantes parecerem replay, o
  que limpava o dedup e contava duas vezes o que aconteceu uma.

As linhas vão para o mesmo `logs/radio/*.jsonl`, no canal `spotter_diario`, porque o carimbo de
sessão já vem pronto dali. `radio-timeline.mjs` esconde esse canal por padrão: aquela leitura é o
que o jogador ouviu.

#### O tracker

`scripts/spotter-tracker.mjs` é a ferramenta de quem termina uma corrida teste. Ele junta o
registro do rádio com a captura de corrida por `(sn, t)` e imprime, por fala, o estado do mundo
naquele instante (velocidade, volta, superfície, `CarLeftRight` cru, vizinho da frente e de trás
em metros), depois as recusas agrupadas por família e motivo com a distribuição da folga, e o
resumo da sessão.

A captura é lida em fluxo, com `Z_SYNC_FLUSH`: o `.gz` da corrida que está rodando agora está
sempre truncado, porque o fluxo só é finalizado no `stop()`, e sem isso o `zlib` joga fora
justamente a corrida que se quer olhar.

---

## 21. Overlays e VR

Três webviews servindo o mesmo `index.html` (§2). O `overlay/` do frontend desenha em canvas
próprio, e não em DOM, para aguentar ficar sobre o sim.

- **Torre de classificação:** `TowerCanvasView.jsx` com `towerCanvas.js`, `towerRows.js`,
  `towerAnimation.js` e `towerThemes.js`.
- **Rádio do engenheiro:** `EngineerRadio.jsx` e `radioCanvas.js`.
- **Feeds:** `useOverlayData.js`, `useBreakdownFeed.js`, `useSpotter.js`, `useOverlayFlags.js`.
- **Posição e VR:** `OverlayPositionPanel.jsx` com a lógica pura em `overlayPose.js`, mais
  `OverlayVrWriter.jsx` e `EngineerVrWriter.jsx`.

Comandos: `overlay_window_*` e `engineer_window_*` (mostrar, esconder, interatividade, hover,
demo), `vr_overlay_*` e `vr_engineer_*` (escrever frame, pose, recentrar, tecla de recentrar), e
`get_overlay_data`, `get_breakdown_feed`, `get_pace_feed`, `get_player_warnings`.

> Armadilha registrada: o preview do overlay não redesenha com HMR. A torre de classificação fica
> estática até recarregar.

---

## 22. Telemetria de produto

`telemetry/` (fila, entrega, uso) e `commands/telemetria.rs`. Anônima e opt-out, com a chave sendo
o `install_id` (UUID da máquina, sem vínculo com e-mail ou conta). Nome de piloto, nome de equipe e
conteúdo de save **nunca** saem do app. O portão de consentimento é `TelemetryConsentGate.jsx`.

O contrato do que existe está em [telemetry-endpoint.md](telemetry-endpoint.md), e o que vem
depois em [telemetry-roadmap-design.md](telemetry-roadmap-design.md), com a fase 1 implementada e o
resto ainda como desenho.

---

## 23. Persistência (SQLite)

### 23.1 Migrações: a baseline v53

Esta é a mudança estrutural que o retrato anterior não registrava. As migrações incrementais de v1
a v52 foram **colapsadas numa baseline única**. Hoje `db/migrations.rs` tem:

```rust
const BASELINE_VERSION: u32 = 53;
const CURRENT_VERSION:  u32 = 64;

const MIGRATIONS: &[(u32, fn(&Connection) -> Result<(), DbError>)] = &[
    (53, migrate_baseline),
    (54, migrate_v54_forma_do_piloto),
    ...
    (64, migrate_v64_normaliza_meta_linguagem_dos_textos_de_ia),
];
```

- `migrate_baseline` roda o `BASELINE_DDL` inteiro (`db/migrations/baseline.rs`, **43 tabelas** —
  este número já foi escrito como 54 aqui; conte com `grep -c 'CREATE TABLE'` no arquivo, e some as 5
  que as incrementais criam), todo o DDL em `IF NOT EXISTS`, então reaplicar é inofensivo, mais o
  seed de `meta` e do `incident_catalog`.
- `run_all` aplica tudo num banco novo, e `run_pending` só o que falta num save existente.
- **Save entre v1 e v52 é recusado com erro explícito.** A baseline não faz backfill de dado, então
  aplicá-la ali carimbaria v53 num banco que continua na forma velha. O arquivo do jogador fica
  intocado e a mensagem manda usar backup mais recente ou começar carreira nova.

Migrações sobre a baseline:

| versão | o que faz |
|---|---|
| v54 | `drivers.forma`, o estado do AR(1) de `simulation::forma` |
| v55 | a leitura da corrida sobrevive ao save: escalar vira coluna (ar sujo, ultrapassagens, maior sequência preso, estratégia) e vetor vira JSON (posição e gap por trecho, paradas) |
| v56 | faixa anunciada |
| v57 | leitura do fim de semana |
| v58 | semeia o carro das categorias especiais |
| v59 | reconcilia títulos de carreira |
| v60 | índice de resultados por piloto |
| v61 | ledger com linhas físicas |
| v62 | tabelas de query sob as migrações |
| v63 | índice de resultados por equipe |
| v64 | normaliza a meta-linguagem dos textos de IA já gravados (o que o filtro de render fazia byte a byte passa a ser feito uma vez, no save; idempotente) |

> **Regra que não muda:** o array `MIGRATIONS` é a única fonte de verdade da ordem. Adicionar uma
> migração é uma linha nesse array mais o bump do `CURRENT_VERSION`. Nunca edite migração já
> lançada: crie a próxima.

A divisão escalar contra JSON da v55 é decisão de consulta, e não de estética. Escalar é o que se
quer agregar (`SUM`, `AVG`, `COUNT(DISTINCT)`), e JSON dentro de coluna mataria isso. Vetor nunca é
predicado: é lido inteiro, de uma corrida só, para desenhar um gráfico.

### 23.2 As tabelas

54 tabelas, agrupadas em §4.5. Os mapas visuais estão em
[database-network-diagram.mmd](database-network-diagram.mmd) (a rede completa),
[database-core-flow.mmd](database-core-flow.mmd) (o laço principal da carreira) e
[database-modules-flow.mmd](database-modules-flow.mmd) (os sistemas de apoio).

> Nota sobre `races`, medida em 11/08/2026: ela está **vazia**, e `calendar` é a fonte única.
> `race_results.race_id` tem `FOREIGN KEY REFERENCES calendar(id)`, e a produção grava
> `race_entry.id` (`commands/race/persistencia.rs:193`). Os únicos `INSERT INTO races` do
> repositório são fixture de teste, e o único `SELECT` é um JOIN em teste. Nenhum código de
> produção escreve nem lê a tabela. Eliminá-la exige `DROP TABLE` numa migração nova, o que é
> decisão do dono. É o item D-02 do [backlog.md](backlog.md).
>
> Armadilha de nome: `db/queries/races.rs` **não** fala com a tabela `races`. Ele guarda as
> queries de `race_results`, `race_safety_cars` e `race_weekend_readings`, todas chaveadas por
> `calendar.id`.

### 23.3 Concorrência (lição registrada)

O "database is locked" ao simular vinha de comando concorrente com transação `DEFERRED`
(BUSY_SNAPSHOT). A correção foi `BEGIN IMMEDIATE`, guard de idempotência dentro da transação e
guard de reentrância no store do frontend.

### 23.4 Saves e backups

`commands/save.rs`: `flush_save`, `create_season_backup`, `list_backups` e `restore_backup`. A UI é
`src/components/ui/BackupsModal.jsx`, aberta pelo `src/pages/LoadSave.jsx`. `AppConfig` controla o
autosave. Cada carreira tem seu próprio arquivo SQLite.

---

## 24. API IPC (comandos Tauri)

Um comando novo só existe depois de entrar no `lib.rs::invoke_handler`. O guard
[`invoke-contra-generate-handler`](../scripts/tests/invoke-contra-generate-handler.test.mjs) cobra
que todo `invoke("...")` do frontend exista ali, e congela em `SEM_CONSUMIDOR_CONHECIDO` o
inventário dos que ainda não têm consumidor, com o motivo de cada um.

**O guard é a contagem oficial — este documento não repete o total.** Todo número em prosa envelhece
no primeiro comando novo, e já envelheceu duas vezes aqui. Para contar hoje:

```bash
node --test scripts/tests/invoke-contra-generate-handler.test.mjs
```

Agrupados por área:

| área | módulo | exemplos |
|---|---|---|
| Config e janela | `config`, `window` | `get_config`, `update_config`, `minimize_window`, `toggle_fullscreen_window` |
| Carreira | `career_commands` | `create_career`, `create_historical_career_draft`, `load_career`, `list_saves`, `delete_career` |
| Loop de temporada | `career_commands`, `calendar` | `advance_season`, `skip_all_pending_races`, `advance_market_week`, `finalize_preseason`, `get_temporal_summary` |
| Mercado | `career_commands` | `get_player_proposals`, `respond_to_proposal`, `get_player_interests`, `get_transfer_window_state`, `get_season_market_board`, `get_player_poach_offer` |
| Consultas e dossiês | `career_commands` | `get_drivers_by_category`, `get_driver_detail`, `get_player_dossier`, `get_global_driver_rankings`, `get_team_history_dossier`, `get_team_finance_report`, `get_race_reading` |
| Corrida | `race` | `simulate_race_weekend`, `simulate_special_block`, `get_saved_race_screen`, `get_stage_invoice`, `get_race_breakdowns`, `get_weekend_modifiers` |
| Convocação | `convocation` | `advance_to_convocation_window`, `run_convocation_window`, `iniciar_bloco_especial`, `run_pos_especial` |
| Notícias e IA | `ai_news`, `world_footer`, `season_preview` | `enrich_race_news_ai`, `pre_race_briefing_ai`, `post_race_debrief_ai`, `get_world_footer`, `enrich_season_preview_ai` |
| iRacing | `iracing` | `iracing_generate_roster`, `iracing_generate_season`, `iracing_auto_import_if_ready`, `iracing_connected`, `iracing_spotter_*`, `iracing_diagnostico` |
| Engenheiro e PTT | `engenheiro`, `ptt`, `ptt_voz`, `volante` | `engenheiro_responder`, `engenheiro_catalogo`, `ptt_transcrever`, `volante_dispositivos` |
| Overlay e VR | `overlay`, `overlay_window`, `vr_overlay` | `get_overlay_data`, `overlay_window_show`, `vr_overlay_set_pose`, `vr_engineer_recenter` |
| Save | `save` | `flush_save`, `create_season_backup`, `list_backups`, `restore_backup` |
| Telemetria e diagnóstico | `telemetria`, `debug_capture` | `telemetria_tela`, `race_capture_start`, `race_capture_stop` |
| POC de TTS | `tts_poc` | `tts_poc_falar`, `tts_poc_log_ler`. Vive fora do jogo: nenhuma tela de carreira invoca |

---

## 25. Frontend

### 25.1 Fluxo de telas

```
MainMenu
  ├─ NewCareer (wizard: piloto → histórico → categoria → equipe → confirmação)
  ├─ LoadSave (lista de saves + BackupsModal)
  ├─ Settings (idioma, autosave, iRacing, engenheiro, overlay, diagnóstico)
  └─ Dashboard (MainLayout: Header + TabNavigation + PauseMenu)
```

Em paralelo, as janelas `overlay` e `engineer` sobem sobre o iRacing.

### 25.2 Abas do Dashboard (`pages/tabs/`)

`NextRaceTab` (briefing, exportação para o iRacing, simular fim de semana), `StandingsTab`,
`CalendarTabRedesign`, `NewsMagazineTab`, `GlobalDriversTab`,
`GlobalTeamsTab` (com a V2 em `tabs/atlas/`), `MyTeamTab` (com a V2 em `tabs/myteam/`) e
`TeamRecordsTab`.

Quatro delas estão na barra (`standings`, `news`, `my-team`, `calendar`, em
`layout/TabNavigation.jsx`); as outras são alcançadas por navegação interna.

**A aba Carreira existiu entre 11/08 e 14/08/2026** e foi apagada com a pasta
`pages/tabs/carreira/` inteira (commit 4892aa8, para quem precisar do código). Ela nasceu como
a lente do protagonista, porque o jogador se enxergava pelo mesmo `DriverDetailModal` que serve
para olhar qualquer piloto de IA, e reunia cinco seções sobre UM payload: `get_driver_detail`
do piloto do jogador.

A ficha do piloto é que respondeu esse buraco. Ela lê o MESMO payload e cresceu até cobrir
quatro das cinco seções: a aba Habilidade é o dossiê medido (F-02), a aba Histórico serve a
trajetória e a curva de campeonato (F-03) mais os primeiros marcos, o auge, a confiabilidade e
os eventos especiais que a sala de troféus listava (F-04), e Rivais (F-05) e Mercado são as
mesmas seções. Duas portas para a mesma resposta é o que a aba cobrava. A porta que ficou é
clicar no próprio nome na tabela da Home.

A quinta seção era o F-01, e dela sobraram duas coisas sem equivalente na ficha: as vagas
abertas do mundo com o veredito de elegibilidade (`get_season_market_board`) e o "quem está de
olho em você" (`get_inbox_messages().team_interest`, que na Home passa como mensagem e aqui
fica como estado). As duas mudaram de casa no mesmo dia, para
`components/driver/v2/MercadoDoJogador.jsx`, montado no fim da aba Mercado da ficha quando
`detail.is_jogador` — são os únicos blocos dela que buscam dado do MUNDO em vez de ler o
`get_driver_detail`, e por isso são também os únicos que uma ficha de piloto de IA não mostra.

### 25.3 Estado (Zustand)

`useCareerStore.js` é o hub, e hoje é só a **composição** dos slices de `src/stores/career/`
(`careerSlice`, `raceSlice`, `marketSlice`, `seasonSlice`, `blocoEspecialSlice`,
`preRaceCacheSlice`) sobre o `initialState` de `career/state.js`. Todos recebem o mesmo par `(set, get)`, então compartilham um
estado único e uma ação chama a de outro domínio via `get()`. Os `invoke` ficam nos slices e, quando
o dado é local de uma tela, em hooks `use*.js` dentro de `components/`.

O outro store vivo é `useAttentionStore`, trivial.

### 25.4 O seletor de versão V1/V2, que não existe mais

**Os três seletores foram removidos em 11/08/2026, e com eles as árvores V1.** Esta seção descrevia
`DriverDetailModal` e `TeamHistoryDrawer` com as duas árvores vivas atrás de um `VERSION = 2` e o
comentário "voltar para o v1 é reverter esta linha" — a alavanca de rollback declarada. O rollback
deixou de ser necessário e a decisão foi tomada.

O que sobrou nos dois `index.js` (`driver/index.js` e `team/history/index.js`) é **só o reexport**
que preserva os nomes antigos: `DriverDetailModalV2` sai como `DriverDetailModal` e
`TeamHistoryDrawerV2` como `TeamHistoryDrawer`, para os consumidores não precisarem mudar de import.
O comentário de cada arquivo registra que houve seletor ali. A normalização de payload que morava
dentro do V1 do dossiê de equipe ficou em `team/teamHistoryDossier.js`.

A árvore V1 da tela de resultado de corrida (`race/RaceResultView.jsx` mais `race/raceresult/`, 15
arquivos e 1.681 linhas) foi removida no mesmo dia, e essa não tinha nem seletor — não tinha
importador nenhum, e escapava do guard de i18n por carregar `i18n-ignore-file`. Tudo registrado em
[divida-tecnica.md](divida-tecnica.md).

### 25.5 Design system (`index.css`), valores aprovados

**Hierarquia de vidro**, três níveis:

- `.glass-light` para sub-elemento (`rgba(10,15,28,0.18)`, blur 8px).
- `.glass` para card interno (`rgba(10,15,28,0.25)`, blur 12px). É a base padrão dos painéis.
- `.glass-strong` e `.entry-panel` para painel externo e launcher (`rgba(255,255,255,0.08)`,
  blur 20px).
- Prop `darkBg` no `GlassCard` (`#07111fa6`, blur 12px), para card dentro de `glass-strong`.

> Armadilha: `GlassCard` sempre aplica `.glass`. Passar `glass-strong` no `className` deixa as duas
> ativas e o `.glass-strong` vence, o que clareia o painel. Nunca passe `glass-strong` em painel
> interno.

**Fundo do app** (`.app-shell`): gradiente escuro com radiais azuladas, jamais preto sólido.

**Paleta** (tokens Tailwind): `text-primary #e6edf3`, `text-secondary #7d8590`,
`text-muted #484f58`, `accent-primary #58a6ff`, `status-green #3fb950`, `status-red #f85149`,
`status-yellow #d29922`, `podium-gold #ffd700`, `podium-silver #c0c0c0`, `podium-bronze #cd7f32`.

**Tipografia:** Space Grotesk Variable, base 10px, rótulo de seção em `11px uppercase
tracking-[0.22em] text-accent-primary`, transição `.transition-glass`.

**Tabelas e listas:** linha com `border-b border-white/5`, hover `hover:bg-white/5`, linha do
jogador `bg-accent-primary/8`, selecionada com
`shadow-[inset_3px_0_0_0_rgba(88,166,255,1)]`.

> Armadilha registrada: opacidade do Tailwind com `currentColor` falha aberta. Grade de SVG em
> `text-white/N` vira branco total. Use `rgba` literal.

### 25.6 i18n

Obrigatório e com guard. Um hook de pre-commit ([.githooks/pre-commit](../.githooks/pre-commit),
ativado pelo `npm install`) bloqueia commit com string de UI em português fora de `t()` em arquivo
`.jsx` no stage. O mesmo checker roda em `src/i18n/i18nCoverage.test.js`.

Exceções intencionais: `{/* i18n-ignore */}` na linha ou na de cima, `// i18n-ignore-file` em
qualquer ponto do arquivo, e `git commit --no-verify` para pular pontualmente.

Frontend: i18next com um namespace por área (`src/i18n/locales/<lang>/common.json`). pt-BR é o
locale base e en-US o par, com `localeParity.test.js` garantindo que as chaves batem. Backend:
`rust-i18n` lendo `src-tauri/locales/*.yml`.

> O locale do backend é **global do processo**. Teste Rust que troca de idioma precisa de
> `#[serial]` (crate `serial_test`), senão contamina teste que asseveram prosa em português.

Duas regras de copy: texto de UI **nunca** em minúscula (capitalize toda string que o jogador lê, o
tom discreto vem do estilo), e acentuação plena, cobrada pelo guard
[`portuguese-copy-accents`](../scripts/tests/portuguese-copy-accents.test.mjs) sobre o
`pt-BR/common.json` inteiro.

---

## 26. Loop de jogo, ponta a ponta

```
1. Nova carreira    → wizard escolhe piloto, categoria e equipe (a dificuldade saiu do
                      wizard em 16/08/2026: o mundo histórico nasce sempre em "medio")
                    → generators/world cria 200+ pilotos, 60+ equipes e contratos
                    → build_full_season_calendar gera 74 entradas (sw 10 a 51)
                    → temporada em fase Temporada, pronta para correr

2. Temporada        → para a etapa DELE: exporta roster e temporada, o jogador corre no
                      iRacing, e iracing_auto_import_if_ready traz o resultado oficial
                    → para o resto do mundo: simulate_race_weekend resolve as outras
                      categorias e as etapas puladas
                    → resultado, classificação, notícia, rivalidade, interesse de evento,
                      desgaste e quebra de peça, fatura da etapa

3. advance_season   → Temporada sem pendente vira Encerramento
                    → run_end_of_season: classificação final, licença, arquivo de piloto e
                      de equipe, evolução, rookies, promoção e rebaixamento
                    → cria a temporada seguinte (74 entradas) e inicializa a pré-temporada

4. PreTemporada     → advance_market_week (até 9 semanas, sw 1 a 9)
                    → o jogador responde proposta e enfrenta assédio
                    → finalize_preseason devolve a fase Temporada e volta ao passo 2
```

Cada passo persiste no SQLite, com autosave e backup por temporada protegendo o progresso.

---

## 27. Estado atual e pendências

**Operacional:** simulação com tráfego e estratégia, quebra de peça, pontuação, evolução, mercado
com assédio, promoção, finanças e economia nova, notícia determinística e por IA, interesse de
evento de ponta a ponta (backend e UI), arquivo histórico, integração completa com o iRacing,
engenheiro de pista com voz e PTT, spotter, overlays e VR, telemetria de produto, backup e
restauração, e a **ficha do piloto** — a lente do protagonista, com dossiê de habilidade,
temporadas passadas, títulos e marcos, rivais e situação de contrato (§25.2).

**Pendências conhecidas**, com o raciocínio em [roadmap.md](roadmap.md) e a lista com id em
[backlog.md](backlog.md):

- **Etapa B do boletim e consequência da hierarquia interna** (o que sobrou do D-09): deixou de ser
  dívida técnica e virou decisão de design, a ser discutida junto do produto.
- **Dois painéis de iRacing órfãos** (§19.6): ligar ou apagar.
- **Fases legadas da convocação** (D-01) e **tabela `races` órfã e vazia** (D-02). Os dois foram
  reenquadrados em 11/08/2026, e nos dois o que falta é decisão do dono, e não limpeza.
- **Varredura de acoplamento do lado Rust** (D-09, briefings R1 a R5 em
  [varredura-acoplamento/](varredura-acoplamento/)). R1 e R2 tocam os mesmos arquivos e não devem
  rodar em paralelo.

A vistoria de reconhecimento de 10/08/2026 sobre os 14 subsistemas está em
[vistoria-2026-08.md](vistoria-2026-08.md), com 179 pontos de revisão classificados por
prioridade.

**Cuidados ao manter**, registrados como aprendizado:

- Não colapse as duas semânticas de categoria especial (§5.1).
- `duracao_corrida_min = 0` é sentinela de endurance (§7.4).
- Procure a feature pelo comando Tauri, e não pelo nome do arquivo. Arquivo com nome promissor e 9
  linhas é evidência mais fraca que um `grep` pelos comandos do domínio.
- `cargo build` e `cargo test` exigem `npm run build` antes: `tauri::generate_context!` embute os
  assets de `dist/` em tempo de compilação.

---

## 28. Documentação correlata

| documento | assunto | estado |
|---|---|---|
| [iracing-escopo.md](iracing-escopo.md) | o levantamento que gerou a decisão de produto de 27/07 | vigente |
| [iracing-dados-disponiveis.md](iracing-dados-disponiveis.md) | o que a telemetria entrega, o que não entrega e as armadilhas medidas | vigente |
| [roadmap.md](roadmap.md) | os buracos abertos e o porquê de cada um | vigente |
| [backlog.md](backlog.md) | a lista priorizada com id estável | vigente |
| [divida-tecnica.md](divida-tecnica.md) | o que já fechou, com data e prova | vigente |
| [vistoria-2026-08.md](vistoria-2026-08.md) | vistoria de 14 subsistemas, 179 pontos | vigente |
| [economia-redesign.md](economia-redesign.md) | inventário, diagnóstico e redesign da economia | proposta parcialmente implantada |
| [i18n-translation-spec.md](i18n-translation-spec.md) | especificação de tradução | vigente |
| [telemetry-endpoint.md](telemetry-endpoint.md), [log-endpoint.md](log-endpoint.md), [world-notes-endpoint.md](world-notes-endpoint.md), [season-preview-endpoint.md](season-preview-endpoint.md) | contratos de servidor | vigente |
| [telemetry-roadmap-design.md](telemetry-roadmap-design.md) | o que medir em seguida | fase 1 feita, resto é desenho |
| [season-preview-design.md](season-preview-design.md) | a aba "O Que Esperar" | design travado, implementação pendente |
| [fase3-fim-de-semana-atribuivel.md](fase3-fim-de-semana-atribuivel.md) | o fim de semana atribuível | desenho, nada no código |
| [tts-poc-latencia.md](tts-poc-latencia.md), [pack-de-voz-briefing.md](pack-de-voz-briefing.md), [pack-de-voz-inventario.md](pack-de-voz-inventario.md), [engenheiro-catalogo.md](engenheiro-catalogo.md) | a voz do engenheiro | vigente |
| [spotter-obstaculo.md](spotter-obstaculo.md), [radio-carga.md](radio-carga.md) | o spotter e a carga do canal | vigente |
| [ai-mention-tags.md](ai-mention-tags.md) | marcação de menção de piloto no texto de IA | vigente |
| [varredura-bugs-2026-07.md](varredura-bugs-2026-07.md) | os 6 achados de julho, com veredito | ✅ fechada em 11/08/2026 |
| [database-network-diagram.mmd](database-network-diagram.mmd), [database-core-flow.mmd](database-core-flow.mmd), [database-modules-flow.mmd](database-modules-flow.mmd), [database-flow-improvement-notes.md](database-flow-improvement-notes.md) | mapas do banco | vigente, regerados em 11/08/2026 contra o v63 (o v64 não mexe em schema, só normaliza texto gravado) |

---

## Como manter este documento

É um **retrato do código**, e não um roadmap. Quem mexer no schema, na lista de comandos, na
estrutura de diretórios ou na identidade do produto atualiza o capítulo correspondente e a linha
de data do cabeçalho no mesmo commit.

Quando uma leitura anterior estiver errada, corrija no lugar e diga o que a evidência velha tinha
de errado. O bloco "Histórico deste arquivo" no topo e a lição da §19 são o formato.
