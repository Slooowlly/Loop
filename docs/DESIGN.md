# Loop — Documento de Design Completo

> Simulador offline de carreira no automobilismo (codinome interno **iRacerApp V1**, marca **Loop**).
> Este documento descreve **tudo o que existe no programa até o momento**: arquitetura, modelo de
> domínio, mecânicas de jogo, banco de dados, API e frontend. É um retrato do código atual, não um
> roadmap. Quando há divergência entre este texto e o código, o **código é a fonte de verdade**.

Data do retrato: 2026-06-15 · Schema do banco: **v34** · Categorias: **9** · **Fase 9D concluída**

---

## 1. Visão geral

O jogador controla **um único piloto** ao longo de uma carreira que sobe por uma pirâmide de 9
categorias (Mazda Rookie → Endurance). Ao redor desse piloto, o simulador mantém um **mundo vivo**:
200+ pilotos de IA e 60+ equipes que correm, evoluem, são contratados, promovidos/rebaixados,
lesionam-se, criam rivalidades e se aposentam — tudo automaticamente.

O jogo é **single-player, offline e local**. Toda a simulação roda em Rust; o estado persiste em
SQLite no disco do usuário; a interface é uma aplicação desktop (Tauri + React).

Pilares de design:

- **Mundo persistente e coerente** — cada temporada deixa histórico arquivado (pilotos, equipes,
  campeões, resultados).
- **Simulação probabilística por atributos** — sem números crus expostos na UI; a leitura é por
  tags/qualitativos, mas o motor usa 17 atributos numéricos.
- **Carreira de longo prazo** — promoção/rebaixamento com tamanhos fixos de categoria, licenças,
  evolução/declínio etário, mercado entre temporadas e um "bloco especial" sazonal.

---

## 2. Stack tecnológica & arquitetura

| Camada | Tecnologia |
|---|---|
| Shell desktop | **Tauri v2** (plugins: shell, dialog, fs) |
| Backend / motor | **Rust** (edição 2021), `rusqlite` |
| Persistência | **SQLite** (arquivo local), migrações versionadas |
| Frontend | **React 18** + **Vite 5** |
| Estado (UI) | **Zustand** |
| Roteamento | **react-router-dom v6** |
| Gráficos | **Recharts** |
| Estilo | **Tailwind CSS 3** + CSS custom (glass system) |
| Fonte | Space Grotesk Variable |
| Testes | **Vitest** + Testing Library (UI); `cargo test` (Rust) |

**Modelo de execução.** O frontend é "burro" no sentido de regras de jogo: quase toda decisão de
domínio acontece no Rust. A comunicação é via **comandos IPC do Tauri** (`invoke`). O Rust abre uma
conexão SQLite, roda a lógica em transações e devolve DTOs serializados (serde) ao React.

```
┌──────────────────────────────────────────────┐
│  React (Vite)  — páginas, abas, stores Zustand │
│      │  @tauri-apps/api  invoke(cmd, args)      │
└──────┼─────────────────────────────────────────┘
       ▼  IPC (comandos registrados em lib.rs)
┌──────────────────────────────────────────────┐
│  Rust  — commands/ → módulos de domínio        │
│  simulation · evolution · market · convocation │
│  promotion · finance · calendar · news · ...   │
│      │  db/queries/*  (rusqlite)                │
└──────┼─────────────────────────────────────────┘
       ▼
   SQLite (save por carreira) + config.json + backups
```

**Janela.** `lib.rs` gerencia o ciclo de vida da janela: restaura tamanho/maximização salvos em
`AppConfig`, faz *debounce* (500 ms) da persistência de resize e grava estado final no fechamento.

---

## 3. Estrutura de diretórios

### Backend (`src-tauri/src/`)

| Módulo | Responsabilidade |
|---|---|
| `commands/` | Camada IPC — funções `#[tauri::command]` expostas ao React |
| `config/` | `AppConfig` (config.json: idioma, autosave, janela, caminho iRacing) |
| `constants/` | Dados estáticos: categorias, pistas, carros, scoring, equipes, timeline histórica, ranges de skill |
| `models/` | Entidades de domínio: driver, team, contract, season, license, injury, rivalry, enums, temporal, tags |
| `db/` | `connection`, `migrations` (v1→v32), `queries/` (uma query module por agregado) |
| `generators/` | Geração de mundo: nomes, nacionalidades, IDs, pilotos, `world` (bootstrap) |
| `calendar/` | Geração de calendário (janelas mensais, multiclasse, slots temáticos) |
| `simulation/` | Motor de corrida: qualifying, race (5 segmentos), incidents, scoring, car_build, track_profile, batch |
| `evolution/` | Crescimento/declínio de atributos, experiência, motivação, lesões, aposentadoria, licenças, transição de temporada |
| `market/` | Mercado de transferências: AI de piloto e equipe, propostas, renovação, pré-temporada, avaliação, pit/car strategy |
| `convocation/` | Janela de convocação + bloco especial (Production / Endurance): elegibilidade, cotas, scoring, ofertas ao jogador |
| `promotion/` | Promoção/rebaixamento de equipes e efeitos sobre pilotos (blocos 1–3 + pipeline) |
| `hierarchy/` | Hierarquia N1/N2 dentro da equipe e transições |
| `rivalry/` | Modelo dual de rivalidade (intensidade histórica + atividade recente) |
| `finance/` | Economia, fluxo de caixa, salários, eventos financeiros, planejamento, estado |
| `event_interest/` | Interesse esperado/realizado do evento (sistema de espectadores) |
| `public_presence/` | Presença pública / repercussão de equipe |
| `news/` | Geração de notícias pós-corrida e de mercado |
| `world/` | Integridade do mundo + arquivo histórico de equipes |
| `common/` | Utilitários (tempo, etc.) |

### Frontend (`src/`)

| Pasta | Conteúdo |
|---|---|
| `pages/` | `SplashScreen`, `BootLogoScreen`, `MainMenu`, `NewCareer`, `LoadSave`, `Settings`, `Dashboard` |
| `pages/tabs/` | Abas do dashboard: NextRace, Standings, Calendar, News, Drivers, GlobalDrivers, GlobalTeams, Market, MyTeam, MyProfile, OtherCategories, Prediction |
| `pages/history/` | `Archive`, `Rivalries`, `SeasonsHistory`, `TrophyRoom` |
| `components/` | Por domínio: `driver/`, `team/`, `race/`, `season/`, `standings/`, `charts/`, `wizard/`, `layout/`, `ui/` |
| `stores/` | `useCareerStore`, `useUIStore`, `useNotificationStore` (Zustand) |
| `hooks/` | `useTauri`, `useLoading` |
| `utils/` | `formatters`, `categoryColors`, `colors`, `constants` |
| `index.css` | Design system (glass, paleta, tipografia) |

---

## 4. Modelo de domínio

### 4.1 Piloto (`drivers`)
Entidade central. Pode ser o **jogador** (`is_jogador = 1`) ou IA. Campos principais:

- Identidade: `id`, `nome`, `idade`, `nacionalidade`, `genero`, `ano_inicio_carreira`.
- Estado: `status` (`Ativo`/`Lesionado`/`Aposentado`/`Suspenso`), `categoria_atual`,
  `categoria_especial_ativa` (separada da regular — usada no bloco especial).
- **Personalidade**: primária (`Ambicioso`, `Consolidador`, `Mercenario`, `Leal`) +
  secundária (`CabecaQuente`, `SangueFrio`, `Apostador`, `Calculista`, `Showman`, `TeamPlayer`,
  `Solitario`, `Estudioso`).
- **17 atributos** (0–100) — ver §6.
- Stats da temporada (`temp_*`) e de carreira (`carreira_*`).
- Tracking dinâmico: `motivacao`, `historico_circuitos` (JSON), `ultimos_resultados` (JSON),
  `temporadas_na_categoria`, `corridas_na_categoria`, `temporadas_motivacao_baixa`.

### 4.2 Equipe (`teams`)
- Identidade/visual: `nome`, `nome_curto`, cores, `pais_sede`, `ano_fundacao`, `marca`, `classe`.
- Categoria atual + `categoria_anterior` (para detectar promoção/rebaixamento).
- **Performance**: `car_performance`, `reliability`, `budget`, `prestige`, `facilities`,
  `engineering`, `morale`, `aerodinamica`, `motor`, `chassi`.
- **Estratégia**: `car_build_profile` (perfil de setup), `pit_strategy_risk`, `pit_crew_quality`.
- **Hierarquia interna** (N1/N2): pilotos `hierarquia_n1_id`/`n2_id`, `hierarquia_tensao`,
  contadores de duelos, sequências e inversões.
- Stats de temporada e de carreira (vitórias, pódios, poles, pontos, títulos).

### 4.3 Contrato (`contracts`)
Liga piloto↔equipe. `papel` (`Numero1`/`Numero2`), `salario_anual`, `duracao_anos`,
`temporada_inicio`/`fim`, `categoria`, `classe` (preenchida só em contratos especiais multiclasse),
`tipo` (`Regular`/`Especial`), `status` (`Ativo`/`Expirado`/`Rescindido`/`Pendente`).
**Invariante:** índice único garante no máximo **um contrato ativo por (piloto, tipo)**.

### 4.4 Temporada (`seasons`)
`numero`, `ano`, `status` (`EmAndamento`/`Finalizada`), `rodada_atual`, e **`fase`** — o coração do
loop macro (ver §7.1). Invariante: índice único garante **uma única temporada EmAndamento**.

### 4.5 Outras entidades
- `calendar` — eventos agendados (ver §7).
- `race_results` — resultado por piloto por corrida (posições, pontos, DNF, incidentes, desgaste).
- `standings` — classificação por temporada/categoria.
- `licenses` — licenças por piloto (gate de progressão).
- `injuries` — lesões ativas (1 ativa por piloto, no máximo).
- `rivalries` — modelo dual (par único de pilotos).
- `market_proposals` / `player_special_offers` — ofertas regulares e especiais.
- `news` — feed gerado (dedup por `chave_dedup`).
- Arquivos históricos: `driver_season_archive`, `team_season_archive`, `history_seasons`,
  `retired`, `track_dnf_history`, `incident_catalog`.

---

## 5. Sistema de categorias e progressão

9 categorias fixas (`constants/categories.rs`), organizadas por **tier**:

| id | Nome curto | Tier | Nível | Equipes×Pilotos = Grid | Corridas | Duração | Licença | Multi-classe |
|---|---|---|---|---|---|---|---|---|
| `mazda_rookie` | Mazda Rookie | 0 | Rookie | 6×2 = 12 | 5 | 15 min | — | não |
| `toyota_rookie` | Toyota Rookie | 0 | Rookie | 6×2 = 12 | 5 | 15 min | — | não |
| `mazda_amador` | Mazda Championship | 1 | Amador | 10×2 = 20 | 8 | 25 min | 0 | não |
| `toyota_amador` | Toyota Cup | 1 | Amador | 10×2 = 20 | 8 | 25 min | 0 | não |
| `bmw_m2` | BMW M2 | 2 | Pro | 10×2 = 20 | 8 | 25 min | 1 | não |
| `production_challenger` | Production | 2 | **Especial** | 18×2 = 36 | 10 | 30 min | 1 | **sim** |
| `gt4` | GT4 Series | 3 | Super Pro | 10×2 = 20 | 10 | 30 min | 2 | não |
| `gt3` | GT3 Championship | 4 | Master | 14×2 = 28 | 14 | 50 min | 3 | não |
| `endurance` | Endurance | 6 | **Especial** | 18×2 = 36 | 6 | (variável) | 4 | **sim** |

**Classes multiclasse:**
- **Production Challenger**: `mazda` (×1.00), `toyota` (×1.00), `bmw` (×1.05) — 6 equipes cada.
- **Endurance**: `gt4` (×0.85), `gt3` (×1.00), `lmp2` (×1.30) — 6 equipes cada.
- **LMP2** é uma **classe de referência** dentro da Endurance (tier 5, "Elite"), não uma categoria
  autônoma do grid principal. `get_category_config("lmp2")` retorna uma config sintética.

**Conflitos de calendário** (não podem coexistir no tempo do jogador):
`mazda_rookie ↔ toyota_rookie`, `mazda_amador ↔ toyota_amador`.

**Grafo de progressão** (`get_target_categories` / `get_feeder_categories`):

```
mazda_rookie  → mazda_amador ┐
toyota_rookie → toyota_amador ┤→ bmw_m2 ┐
                              └─────────┴→ gt4 → gt3 → endurance
        production_challenger ───────────→ gt4
```

### 5.1 As duas semânticas das categorias especiais (armadilha conhecida)
`production_challenger` e `endurance` exigem **dois predicados opostos** conforme o contexto — eles
**não** são intercambiáveis:

- **Sentido fase/mercado** (`uses_regular_contracts` / `uses_regular_teams` = `config.is_some()`,
  **true** para especiais): fases especiais rodam mercado de contratos regulares e criam contratos
  por classe (ex.: vaga de endurance recebe contrato regular `categoria=gt3`). Usado em
  `market/pipeline`, `world/integrity`, `generators/world`.
- **Sentido validação-folha** (`is_especial` = **true**, logo a categoria é **excluída**): um
  contrato/piloto rotulado com a meta-categoria literal `endurance`/`production_challenger` é
  **inválido** — pilotos são contratados no nível da classe (gt3/gt4/mazda…), nunca na meta. Usado
  em reparos de consistência de `commands/career.rs`, `career_detail.rs`, `global_driver_rankings.rs`.

> Nunca colapsar `is_especial(x)` em `!uses_regular_contracts(x)` — não são equivalentes.

---

## 6. Atributos do piloto

17 atributos (`models/driver_attributes.rs`), todos 0–100:

| Atributo | Papel principal |
|---|---|
| `skill` | Ritmo bruto / talento geral |
| `consistencia` | Reduz variância de resultado (menos erros) |
| `racecraft` | Ultrapassagens, briga roda-a-roda |
| `defesa` | Defender posição |
| `ritmo_classificacao` | Performance no qualifying |
| `gestao_pneus` | Reduz degradação de pneu (até −50%) |
| `habilidade_largada` | Peso alto no segmento de largada |
| `adaptabilidade` | Pistas difíceis / caráter de pista |
| `fator_chuva` | Performance no molhado |
| `fitness` | Reduz degradação física (segmentos finais) |
| `experiencia` | Maturidade; modula incidentes |
| `desenvolvimento` | Potencial/curva de crescimento |
| `aggression` | Aumenta incidentes/risco |
| `smoothness` | Reduz degradação de pneu (até −20%) |
| `midia` | Presença pública / repercussão |
| `mentalidade` | Peso em segmentos Late/Finish |
| `confianca` | Peso no segmento Finish; sobe/desce com resultados |

A UI **não mostra números crus** — usa tags/badges (`DriverTags`, `PersonalityBadge`) e radar
(`DriverProfile` + Recharts). Os números só existem no motor.

---

## 7. Calendário e sistema temporal

### 7.1 Fases da temporada (`SeasonPhase`) — modelo 9D ✓

O macroestado de uma temporada percorre, em ordem, no **modelo 9D** (ativo desde v33/v34):

```
PreTemporada (sw 1–9) → Temporada (sw 10–51) → Encerramento → (advance_season) → PreTemporada
```

- **PreTemporada** — mercado entre temporadas; 9 semanas (MARKET_DURATION_WEEKS = 9);
  dec/jan/fev (sw 1–9). O jogador recebe/aceita propostas, a IA assina contratos.
  Termina em `finalize_preseason` → Temporada.
- **Temporada** — todas as 9 categorias correm em paralelo ao longo de 42 semanas (sw 10–51,
  fev–nov). O calendário tem exatamente **74 entradas** geradas por `build_full_season_calendar`.
  Quando não há corridas pendentes, `advance_season` transiciona para Encerramento.
- **Encerramento** — `run_end_of_season` executa o pipeline completo: standings, licenças,
  arquivos (driver/team), evolução, rookies, promoção/rebaixamento, criação da próxima
  temporada, inicialização de PreTemporada.

**Fases legadas** (preservadas no código para saves pré-v33; nunca emitidas em saves novos):
`BlocoRegular`, `JanelaConvocacao`, `BlocoEspecial`, `PosEspecial`.
Saves nessas fases são migrados por v33/v34 ou seguem o caminho legado até chegar em
`advance_season`, que então inicializa o modelo novo.

**Eixo `season_week` (sw)**:
`sw = week_of_year + 4`. A janela de mercado é sw 1–9 (woy −3 a 5, set-dez/jan-fev);
a janela de corridas é sw 10–51 (woy 6–47, fev–nov). Adicionado em v33 com backfill.

### 7.2 Calendário da Temporada (`build_full_season_calendar`)

74 entradas por temporada, todas `season_phase = Temporada` e `season_week` em 10–51:

| Categoria | Rodadas | woy inicio | woy fim | sw inicio | sw fim |
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

Invariantes garantidos por `tests_9d.rs`: exatamente 74 entradas, todas Pendente, zero LMP2
no calendário (LMP2 é *classe* dentro de endurance, não categoria), production/endurance sem
rodadas duplicadas, única temporada EmAndamento.

### 7.3 Unidade temporal interna: `week_of_year`
Toda ordenação e lógica temporal usa **semana do ano (1–52)**, não datas. As datas visíveis
(`display_date`) são **derivadas** da semana, apenas para UI/notícias/narrativa.

`SeasonTemporalSummary` (`models/temporal.rs`) é o DTO que a UI consome: fase atual,
`effective_week`, data de exibição, próximo evento do jogador, dias até o próximo evento e número de
corridas pendentes na fase.

### 7.3 `CalendarEntry`
Cada evento carrega: categoria, rodada, pista (`track_id`/`track_name`/`track_config`), clima,
temperatura, voltas, durações, `status` (`Pendente`/`Concluida`), `week_of_year`, `season_phase`,
`display_date` e **`thematic_slot`**.

### 7.4 Slots temáticos (`ThematicSlot`)
Papel narrativo **fixo e imutável** atribuído na geração do calendário (não é o resultado, nem a
importância calculada). Grupo regular: `AberturaDaTemporada`, `RodadaRegular`, `VisitanteRegional`,
`MidpointPrestigio`, `TensaoPreFinal`, `FinalDaTemporada`. Grupo especial: `AberturaEspecial`,
`RodadaEspecial`, `FinalEspecial`. Fallback explícito: `NaoClassificado` (saves pré-v12).

Pistas vêm de `constants/tracks.rs`, com pistas fixas + variáveis por categoria, chance de chuva por
pista e duração de classificação. Algumas categorias usam apenas "pistas gratuitas".

---

## 8. Motor de simulação de corrida

Pipeline (`simulation/engine.rs::run_full_race`):

```
simulate_qualifying → simulate_race (5 segmentos) → determine_fastest_lap → assign_points
```

### 8.1 Qualifying
Calcula um `quali_score` único por piloto (escala ~55–85, peso de `ritmo_classificacao`, skill,
carro, clima) e converte gaps de score em ms (`QUALI_SCORE_TO_LAP_MS = 50`). Define o grid.

### 8.2 Corrida em 5 segmentos
Segmentos: **Start → Early → Mid → Late → Finish**. Cada piloto tem um `RaceState`
(`tire_wear`, `physical_condition`, `cumulative_score`, posição, incidentes, dano latente).
Score inicial = posição de largada invertida ×2.

Para cada segmento, calcula-se `segment_score` com **pesos por segmento**:

| Segmento | skill | largada | racecraft | carro | pneus | fitness | mentalidade | confiança |
|---|---|---|---|---|---|---|---|---|
| Start | .20 | **.35** | .25 | .20 | — | — | — | — |
| Early | **.35** | — | .20 | .30 | .15 | — | — | — |
| Mid | **.35** | — | — | .30 | .20 | .15 | — | — |
| Late | .25 | — | — | .20 | **.25** | .20 | .10 | — |
| Finish | .25 | — | .25 | .20 | — | — | .10 | **.20** |

Modificadores aplicados ao score do segmento:
- **Penalidade de pneu**: `(1 − tire_wear) × 0.15`.
- **Penalidade de fadiga** (só Late/Finish): `(1 − physical) × 0.10`.
- **Clima**: multiplicador por `fator_chuva` × sensibilidade do contexto.
- **Pista difícil**: bônus de `adaptabilidade` e `consistencia`.
- **Caráter de pista** (`Flowing`/`Technical`/`Tight`/`Roval`): pequenos biases em skill/carro/adaptabilidade.
- **Spread de ritmo**: comprime/expande em torno de 60 (endurance fecha o campo, rookie abre).
- **Inexperiência**: penalidade se `corridas_na_categoria < 10`.
- **Variância**: `(100 − consistencia)/100 × 5`, escalada pelo perfil; caos extra na largada
  (densidade do pelotão × start_chaos).

Após cada segmento: ordena por `cumulative_score` (DNFs ao fim), aplica degradação de pneu
(`gestao_pneus` até −50%, `smoothness` até −20%, escalado pela duração) e degradação física
(`fitness` até −60%).

### 8.3 Resultado
- Finishers ordenados por score; DNFs ordenados por **segmento de abandono** (mais tarde = melhor
  classificado entre os DNFs).
- Tempo de volta derivado do gap de score (`RACE_SCORE_TO_LAP_MS = 30`); `gap_to_winner_ms ≥ 0`.
- Voltas no DNF estimadas pela fração do segmento (Start 10% … Finish 90%).
- Campos narrativos agregados: `total_incidents`, `total_dnfs`, `main_incident_count`,
  `notable_incident_pilot_ids`, `most_positions_gained_id`.

O motor é **determinístico por seed** (`StdRng`), o que torna os testes reproduzíveis (há suíte
extensa em `race.rs` validando "bom piloto tende a vencer", "pneu ruim machuca no fim", etc.).

---

## 9. Sistema de pontuação

`constants/scoring.rs`:

- **Padrão** (P1–P10): `25, 18, 15, 12, 10, 8, 6, 4, 2, 1`.
- **Endurance** (P1–P10): `35, 28, 23, 19, 16, 13, 10, 7, 4, 2`.
- **Volta mais rápida**: +1, apenas se o piloto terminar no **top 10** e não for DNF.
- Bônus "overall" definidos (1º=+5, 2º=+3, 3º=+1) para usos multiclasse/agregados.
- DNF = 0 pontos.

**Dificuldade** (range de skill da IA gerada): Fácil 20–60, Médio 30–80, Difícil 50–90,
Lendário 70–100.

**Clima** — penalidade base + multiplicador de dificuldade: Dry (0.00, 1.00), Damp (0.06, 1.15),
Wet (0.12, 1.35), HeavyRain (0.18, 1.60). Distribuição de intensidade de chuva quando chove:
Damp 40% / Wet 40% / HeavyRain 20%.

---

## 10. Incidentes, DNFs e lesões

### 10.1 Incidentes (`simulation/incidents.rs` + `incident_catalog`)
Quando `incidents_enabled`, cada segmento processa rolls de incidente por piloto, influenciados por
`aggression`, `consistencia`, `experiencia`, clima, "championship deciding", densidade do pelotão e
confiabilidade do carro. Um incidente pode causar **perda de posições** ou **DNF**.

O **catálogo de incidentes** (`incident_catalog`, semeado na migração v14) é parametrizado por
classe de veículo, formato (sprint/endurance), fonte e tipo de gatilho, com pesos separados para
sprint/endurance e templates de texto (DNF e não-DNF) para narrativa.

### 10.2 Dano latente pós-colisão (`PendingDamage`)
Colisões podem gerar **dano latente** que se manifesta em segmentos posteriores: a cada segmento há
uma `manifest_chance` (que cresce +0.15 se não manifestar); ao manifestar, há 70% de chance de virar
DNF se o dano for "dnf-capable". Modela falhas mecânicas tardias originadas de toques anteriores.

### 10.3 Lesões (`injuries`, `evolution/injury.rs`)
Tipos: `Leve`, `Moderada`, `Grave`, `Critica`. Uma lesão carrega `modifier`, `skill_penalty`,
`races_total`/`races_remaining` e fica ativa por N corridas. **Máximo de 1 lesão ativa por piloto.**
Afeta status do piloto (`Lesionado`) e performance enquanto ativa.

### 10.4 Histórico de DNF por pista (`track_dnf_history`)
Registra DNFs por piloto×pista (motivo, colisão com quem) — base para narrativa de "redenção".

---

## 11. Evolução de pilotos (`evolution/`)

Roda na transição de temporada. Submódulos:

- **growth** — crescimento de atributos (modulado por idade, `desenvolvimento`, resultados).
- **decline** — declínio por idade avançada.
- **experience** — ganho de `experiencia` por corridas disputadas.
- **motivation** — `motivacao` sobe/desce com resultados vs. expectativa; baixa motivação
  sustentada (`temporadas_motivacao_baixa`) tem consequências (mercado/aposentadoria).
- **injury** — recuperação/expiração de lesões.
- **licenses** — concessão de licença ao cumprir requisitos na categoria.
- **retirement** — aposentadoria (idade/declínio/motivação); piloto vai para `retired`.
- **rookies** — entrada de novos pilotos para repor vagas.
- **standings** — consolidação de classificação.
- **season_transition** — orquestrador: cria nova temporada, **arquiva snapshot completo de cada
  piloto** (`driver_season_archive`) após crescimento e antes da promoção, e dispara o resto.

Ordem importante: crescimento de atributos **antes** do arquivamento; arquivamento **antes** da
promoção/rebaixamento (para capturar atributos finais e categoria original).

---

## 12. Mercado de transferências (`market/`)

Roda na pré-temporada (entre temporadas). Componentes:

- **team_ai** — cada equipe decide quem quer contratar (necessidades por papel N1/N2, orçamento,
  prestígio, encaixe na categoria).
- **driver_ai** — cada piloto decide aceitar/recusar, com motivos tipados (`RefusalReason`:
  `SalarioBaixo`, `EquipeFraca`, `CategoriaErrada`, `BloqueioHierarquico`, `PreferenciaPessoal`).
- **evaluation** — valoração de piloto (desempenho recente, idade, atributos).
- **proposals** — emissão/resposta de `market_proposals` (equipe→piloto).
- **renewal** — renovação de contratos existentes.
- **preseason** — orquestra a janela: gera propostas, processa respostas da IA, preenche vagas.
- **visibility** — o que o jogador enxerga do mercado.
- **sync** — sincroniza contratos↔lineups das equipes↔`categoria_atual` dos pilotos.
- **car_build_strategy / pit_strategy** — escolha de perfil de setup e risco de pit por equipe.

O **jogador** recebe propostas (`get_player_proposals` / `respond_to_proposal`) e avança a janela
semana a semana (`advance_market_week`) até `finalize_preseason`.

---

## 13. Convocação e bloco especial (`convocation/`)

As categorias especiais (Production, Endurance) **não têm elenco fixo de temporada inteira** — seus
grids são montados na **JanelaConvocacao** convocando pilotos das categorias feeder.

Mapeamento de classes convocadas:

| Categoria especial | Classe | Feeder |
|---|---|---|
| production_challenger | mazda | mazda_amador |
| production_challenger | toyota | toyota_amador |
| production_challenger | bmw | bmw_m2 |
| endurance | gt4 | gt4 |
| endurance | gt3 | gt3 |
| endurance | lmp2 | (referência) |

Pipeline:
- **eligibility** — coleta candidatos elegíveis por fonte (`FonteConvocacao`) e licença.
- **quotas** — calcula cotas por classe/equipe.
- **scoring** — pontua candidatos para preencher vagas (`calcular_score`).
- **player_offers** — gera ofertas especiais ao jogador (`player_special_offers`); o jogador
  responde dia a dia na **janela especial**.
- **special_window** — máquina de estados diária da janela (estado, atribuições, pool de
  candidatos, log diário — tabelas `special_window_*`).
- **pipeline** — `advance_to_convocation_window`, `run_convocation_window`,
  `iniciar_bloco_especial`, `encerrar_bloco_especial`, `run_pos_especial`.

Contratos especiais são `tipo = Especial`, com `classe` preenchida, e expiram no `PosEspecial`.

---

## 14. Promoção / rebaixamento (`promotion/`)

Mantém os **tamanhos fixos** de cada categoria movendo **equipes** (não pilotos individuais) entre
tiers ao fim da temporada. Estruturas:

- `PromotionResult` = `movements` (TeamMovement: `Promocao`/`Rebaixamento`) + `pilot_effects` +
  `attribute_deltas` + `errors`.
- **Efeitos sobre pilotos** (`PilotEffectType`): `MovesWithTeam` (sobe/desce com a equipe),
  `FreedNoLicense` (liberado por não ter licença para a nova categoria), `FreedPlayerStays`
  (jogador permanece por escolha).
- **Deltas de atributo da equipe** (`TeamAttributeDelta`): ao promover/rebaixar, ajusta
  `car_performance`, `budget`, `facilities`, `engineering`, `morale` (multiplicador) e reputação.

Organizado em blocos (`block1/2/3`) + `standings` (apuração) + `effects` + `pipeline`.

---

## 15. Hierarquia de equipes & rivalidades

### 15.1 Hierarquia interna N1/N2 (`hierarchy/`)
Cada equipe tem um piloto **Número 1** e um **Número 2**. O sistema rastreia `hierarquia_tensao`,
duelos totais, duelos vencidos pelo N2, sequências (N1/N2) e inversões na temporada. Quando o N2
supera consistentemente o N1, a hierarquia pode **inverter** (`hierarchy/transition.rs`).

### 15.2 Rivalidades (`rivalry/`, `rivalries`)
Modelo **dual** (migração v8): `historical_intensity` (calor acumulado de longo prazo) +
`recent_activity` (atividade recente, que decai). `temporada_update` baseia a decisão de decaimento.
Par de pilotos é único (índice único `piloto1_id, piloto2_id`). Rivalidades nascem de brigas
roda-a-roda, colisões e disputas de título, e alimentam notícias e a UI de histórico.

---

## 16. Finanças (`finance/`)

- **economy** — parâmetros econômicos do mundo.
- **salary** — cálculo de salários (por categoria, papel, desempenho).
- **cashflow** — entradas/saídas (prêmios, salários, custos).
- **events** — eventos financeiros (patrocínio, bônus, multas).
- **planning** — planejamento orçamentário da equipe.
- **state** — estado financeiro consolidado.

O `budget` da equipe condiciona o que ela pode pagar no mercado e seu investimento em performance.

---

## 17. Interesse do evento, espectadores e presença pública

### 17.1 Event interest (`event_interest/`)
Sistema de **espectadores** com ciclo fechado no backend:
- **Esperado** (`calculate_expected_event_interest`) — pré-corrida, baseado em prestígio da
  categoria, slot temático, disputa de título, estrelas no grid, etc. Aparece em `NextRaceTab` como
  `EventInterestSummary` (display_value + tier_label).
- **Realizado** (`RealizedEventInterest`) — pós-corrida; gera deltas de mídia/motivação por piloto e
  pode **elevar** uma notícia a destaque (`public_impact`).

> **Pendência de UI**: o backend (3 blocos) está completo; a UI ainda mostra só o widget básico.
> Falta exibição rica do interesse esperado e feedback visual pós-corrida da repercussão.

### 17.2 Presença pública (`public_presence/`)
Repercussão pública da equipe/piloto, ligada ao atributo `midia`.

---

## 18. Notícias (`news/`)

Feed gerado automaticamente após cada corrida e em eventos de mercado. Tipos (`NewsType`):
`Corrida`, `Contratacao`, `Lesao`, `Aposentadoria`, `Promocao`, `Rivalidade`, `Titulo`, `Incidente`.
Dedup por `chave_dedup` (índice único). Narrativa é contextual: usa histórico recente do piloto
(sequências de vitória, rebaixamentos, redenção em pista de DNF anterior).

**Aba de Notícias** (decisões de design confirmadas):
- Hero card "Central de Notícias" — estética A (gradiente dark blue, accent azul), 4 slots de
  metadados: `Publicada em`, `Próxima etapa`, `Etapa (X de Y + pista)`, `Matérias (contagem)`.
- **Filtro padrão**: ao abrir, escopo = categoria atual do jogador, filtro = **última corrida
  disputada** dessa categoria. Ao trocar de escopo, o filtro vira a última corrida **daquela**
  categoria automaticamente. (`get_news_tab_bootstrap` / `handleScopeChange`.)

---

## 19. Persistência (SQLite)

### 19.1 Migrações
`db/migrations.rs` — schema versionado em `meta.schema_version`. **v1** cria o schema base (DDL
inline `MIGRATION_V1_DDL`); **v2→v32** evoluem incrementalmente (colunas, índices, rebuilds de
tabela preservando linhas, seeds). `run_all` aplica tudo num banco novo; `run_pending` aplica só o
que falta num save existente. Helpers: `ensure_column`, `rebuild_table_preserving_rows`,
`copy_legacy_rows`, `column_expr` (resiliência a saves legados).

Marcos das migrações:
- v5: rebuild de `race_results` (FK para `calendar`, colunas narrativas).
- v8/v17: modelo dual de rivalidade + dedup de pares.
- v9–v12: contratos especiais (`tipo`, `classe`), fase da temporada, `week_of_year`,
  `season_phase`, `thematic_slot`.
- v13/v14: contexto narrativo de corrida + `track_dnf_history` + `incident_catalog`.
- v15/v16: `driver_season_archive`, `categoria_anterior` da equipe.
- v18–v22: invariantes via índices únicos (1 contrato ativo/piloto/tipo; 1 temporada ativa;
  1 resultado/corrida/piloto; 1 lesão ativa/piloto; rebuild de `standings`).
- v23/v24: `car_build_profile`, `pit_strategy_risk`, `pit_crew_quality`.
- janela especial: `special_window_*`, `special_team_entries`; `team_season_archive`.
- **v33** (Fase 9D): adiciona coluna `season_week`, backfill `week_of_year + 4`, remapeia
  `ThematicSlot` de corridas finalizadas, converte `Finalizada → Encerramento`.
- **v34** (Fase 9D): converte saves `BlocoRegular → Temporada`; gera calendário parcial de
  `production_challenger` e `endurance` para semanas restantes (`from_sw` = última Concluída + 1).
  Saves em `BlocoEspecial/JanelaConvocacao/PosEspecial`: **intocados**.

**Condição de disparo do modelo novo**: save em `BlocoRegular` (v32) → run_pending aplica v33+v34
→ `fase = Temporada`, `season_week` preenchido, calendário especial gerado. Saves em fases
especiais legadas continuam o caminho antigo até `advance_season`, que então inicializa 9D.

### 19.2 Tabelas principais
`meta`, `config`, `drivers`, `teams`, `contracts`, `seasons`, `calendar`, `races`, `race_results`,
`standings`, `licenses`, `injuries`, `market`, `market_proposals`, `player_special_offers`, `news`,
`rivalries`, `retired`, `history_seasons`, `history_general`, `track_dnf_history`,
`incident_catalog`, `driver_season_archive`, `team_season_archive`, `special_window_state`,
`special_window_assignments`, `special_window_candidate_pool`, `special_window_daily_log`,
`special_team_entries`.

> Nota: a tabela `races` é legada — o sistema usa entradas de `calendar` como corridas;
> `race_results.race_id` referencia `calendar(id)`.

### 19.3 Concorrência (lição registrada)
"database is locked" ao simular era causado por comandos concorrentes + transação `DEFERRED`
(BUSY_SNAPSHOT). **Correção**: `BEGIN IMMEDIATE` + guard de idempotência dentro da transação +
guard de reentrância no store do frontend.

### 19.4 Saves & backups
`commands/save.rs`: `flush_save`, `create_season_backup`, `list_backups`, `restore_backup`.
`AppConfig` controla autosave. Cada carreira tem seu próprio arquivo SQLite.

---

## 20. API IPC (comandos Tauri)

Registrados em `lib.rs::invoke_handler`. Agrupados por domínio:

**Config / janela**: `get_config`, `update_config`, `minimize_window`, `toggle_maximize_window`,
`close_window`, `get_window_maximized`, `toggle_fullscreen_window`, `get_window_fullscreen`.

**Carreira (ciclo de vida)**: `create_career`, `create_historical_career_draft`, `get_career_draft`,
`discard_career_draft`, `finalize_career_draft`, `load_career`, `delete_career`, `list_saves`,
`set_career_resume_context`.

**Loop de temporada**: `advance_season`, `skip_all_pending_races`, `advance_market_week`,
`get_preseason_state`, `finalize_preseason`, `get_preseason_free_agents`, `get_temporal_summary`.

**Mercado (jogador)**: `get_player_proposals`, `respond_to_proposal`.

**Corrida**: `simulate_race_weekend`, `simulate_special_block`.

**Convocação / bloco especial**: `advance_to_convocation_window`, `run_convocation_window`,
`get_player_special_offers`, `get_special_window_state`, `accept_special_offer_for_day`,
`advance_special_window_day`, `respond_player_special_offer`, `iniciar_bloco_especial`,
`encerrar_bloco_especial`, `run_pos_especial`.

**Consultas / dossiês**: `get_drivers_by_category`, `get_teams_standings`,
`get_team_history_dossier`, `get_race_results_by_category`, `get_previous_champions`,
`get_calendar_for_category`, `get_driver`, `get_driver_detail`, `get_global_driver_rankings`,
`get_global_team_history`.

**Notícias**: `get_news`, `get_news_tab_bootstrap`, `get_news_tab_snapshot`,
`get_briefing_phrase_history`, `save_briefing_phrase_history`.

**Save**: `flush_save`, `create_season_backup`, `list_backups`, `restore_backup`.

---

## 21. Frontend

### 21.1 Fluxo de telas
```
SplashScreen / BootLogoScreen → MainMenu
  ├─ NewCareer (wizard: categoria → dificuldade → equipe → confirmação)
  ├─ LoadSave (lista de saves + backups)
  ├─ Settings (idioma, autosave, caminho iRacing)
  └─ Dashboard (MainLayout: Header + Sidebar + TabNavigation)
```

### 21.2 Abas do Dashboard (`pages/tabs/`)
`NextRaceTab` (briefing + simular fim de semana), `StandingsTab`, `CalendarTab`, `NewsTab`,
`DriversTab` (da categoria), `GlobalDriversTab` (ranking global), `GlobalTeamsTab`/histórico,
`MarketTab`, `MyTeamTab`, `MyProfileTab`, `OtherCategoriesTab`, `PredictionTab`.
Histórico (`pages/history/`): `Archive`, `Rivalries`, `SeasonsHistory`, `TrophyRoom`.

### 21.3 Stores (Zustand)
- **useCareerStore** — estado da carreira ativa, temporada, fase, dados carregados; orquestra
  chamadas IPC e guarda de reentrância de simulação.
- **useUIStore** — estado de UI (aba ativa, modais, drawers).
- **useNotificationStore** — toasts/notificações.

### 21.4 Design system (`index.css`) — valores aprovados
**Glass hierarchy** (3 níveis):
- `.glass-light` — sub-elementos (`rgba(10,15,28,0.18)`, blur 8px).
- `.glass` — cards internos (`rgba(10,15,28,0.25)`, blur 12px). **Base padrão** dos painéis.
- `.glass-strong` / `.entry-panel` — painéis externos/launcher (frosted, `rgba(255,255,255,0.08)`,
  blur 20px).
- Prop `darkBg` no `GlassCard` (`#07111fa6`, blur 12px) — para cards **dentro** de `glass-strong`.

> Armadilha: `GlassCard` sempre aplica `.glass`. Passar `glass-strong` no `className` deixa **ambas**
> ativas e o `.glass-strong` vence (fica claro). **Nunca** passar `glass-strong` em painéis internos.

**Fundo do app** (`.app-shell`) — nunca preto sólido; gradiente escuro com radiais azuladas.

**Paleta** (tokens Tailwind): `text-primary #e6edf3`, `text-secondary #7d8590`, `text-muted #484f58`,
`accent-primary #58a6ff`, `status-green #3fb950`, `status-red #f85149`, `status-yellow #d29922`,
`podium-gold #ffd700`, `podium-silver #c0c0c0`, `podium-bronze #cd7f32`.

**Tipografia**: Space Grotesk Variable; base `10px`; labels de seção `11px uppercase
tracking-[0.22em] text-accent-primary`; transições `.transition-glass` (cubic-bezier 0.4,0,0.2,1).

**Tabelas/listas**: linha `border-b border-white/5`, hover `hover:bg-white/5`, linha do jogador
`bg-accent-primary/8`, selecionada com `shadow-[inset_3px_0_0_0_rgba(88,166,255,1)]`.

---

## 22. Loop de jogo (end-to-end) — modelo 9D ✓

```
1. Nova carreira    → wizard escolhe categoria/dificuldade/equipe
                    → generators/world cria 200+ pilotos, 60+ equipes, contratos
                    → build_full_season_calendar gera 74 entradas (sw 10–51)
                    → temporada em fase Temporada, pronta para corridas

2. Temporada        → jogador simula fim de semana a fim de semana (simulate_race_weekend)
                    → resultados, standings, notícias, rivalidades, interesse de evento
                    → todas as 9 categorias correm em paralelo (sw 10–51, fev–nov)

3. advance_season   → Temporada (sem pendentes) → Encerramento
                    → run_end_of_season: standings finais, licenças, driver/team archives,
                      evolução, rookies, promoção/rebaixamento de equipes
                    → create_next_season_9d (74 entradas) + initialize_preseason

4. PreTemporada     → advance_market_week (até 9 semanas, sw 1–9, dez–fev)
                    → jogador responde propostas de equipe
                    → finalize_preseason → fase Temporada → volta ao passo 2
```

**Legado** (saves pré-v33 em BlocoEspecial/JanelaConvocacao/PosEspecial): seguem o caminho
antigo via `skip_all_pending_races_in_base_dir` + `advance_season`, que ao encerrar inicializa
o modelo 9D para a próxima temporada. Código legado preservado; nunca emitido em saves novos.

**Código legado a remover** (futura limpeza, sem urgência):
- `BlocoRegular`, `JanelaConvocacao`, `BlocoEspecial`, `PosEspecial` em `SeasonPhase`
- `convocation/` (pipeline da JanelaConvocacao)
- `special_window_*` tables e comandos associados
- `simulate_special_block` command

Cada passo persiste no SQLite; autosave/backup por temporada protegem o progresso.

---

## 23. Estado atual, pendências e dívida técnica

**Operacional**: simulação, pontuação, evolução, mercado, promoção, finanças, notícias,
interesse de evento (backend), arquivo histórico, e o grosso da UI (dashboard, abas, históricos).
**Fase 9D concluída** (2026-06-15): calendário unificado de 74 entradas, ciclo
PreTemporada/Temporada/Encerramento, migrações v33/v34, suite de testes de integração em
`src-tauri/src/tests_9d.rs` (1449 passed, 0 failed, 1 ignored).

**Pendências conhecidas**:
- **UI de espectadores** (event interest) — backend completo, UI ainda básica (ver §17.1).
- **Integração real com iRacing** — `AppConfig` guarda o caminho, mas export/watchdog foram
  **removidos** do código atual (módulos `export/` e `commands/export.rs` deletados; a integração é
  expansão futura).
- Tabela `races` legada coexiste com o uso de `calendar` como corridas.
- **Código legado de convocação** — `convocation/`, `simulate_special_block`, fases
  `BlocoRegular/JanelaConvocacao/BlocoEspecial/PosEspecial` preservados para saves antigos;
  remoção segura após confirmar que nenhum save ativo usa essas fases.

**Cuidados ao manter** (registrados como aprendizado):
- Não colapsar as duas semânticas de categoria especial (§5.1).
- Não passar `glass-strong` em `GlassCard` interno (§21.4).
- Transações de simulação devem usar `BEGIN IMMEDIATE` + guards de idempotência/reentrância (§19.3).
- `season_week = week_of_year + 4` é o eixo canônico; nunca usar `week_of_year` diretamente
  para ordenação de mercado ou calendário.

**Documentação correlata** em `docs/`: `database-core-flow.mmd`, `database-modules-flow.mmd`,
`database-network-diagram.mmd`, `database-flow-improvement-notes.md`, `divida-tecnica.md`, `mockups/`.

---

*Documento gerado a partir da leitura direta do código em 2026-06-11. Para detalhes de fórmulas
exatas, consulte os módulos citados — eles são a fonte de verdade.*
