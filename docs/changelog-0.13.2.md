# Loop 0.13.2 — changelog técnico

Base: `f62c8ec` (0.13.1) → `8eb18c2`.
**139 commits · 806 arquivos · +169.740 / −118.000 linhas.**

| tipo | commits |
|---|---|
| refactor | 123 |
| feat | 6 |
| fix | 5 |
| docs | 3 |
| test | 1 |
| chore | 1 |

---

## 1. Simulação de corrida — reforma da raiz (`a905ca2`)

Sintoma reportado: cinco corridas seguidas com exatamente a mesma ordem de
chegada, do P1 ao P12.

**Diagnóstico.** O motor acumulava *pontos* de ritmo ao longo de 5 segmentos e
ordenava no fim. O componente determinístico era idêntico em todo segmento, então
a vantagem do piloto melhor era multiplicada por 5 enquanto o ruído, sorteado de
novo a cada segmento, se cancelava parcialmente e crescia só com √5 — sinal sobre
ruído aumentava por construção. E sem gap entre carros não existia ar sujo, trem,
undercut nem safety car: a única alavanca sobre o resultado era aumentar o dado.

- **Camadas de variância** (`simulation/forma.rs`): afinidade piloto × pista por
  hash determinístico (sem coluna, permanente por construção), forma do momento
  em AR(1) persistida (**migração v54**) e acerto de fim de semana por
  equipe/evento. Entram como elos da esteira de modificadores de
  `commands/race/simulacao.rs`.
- **Moeda em tempo** (`race/motor.rs`, `pontuacao.rs`, `resultados.rs`):
  `cumulative_score` virou tempo acumulado. A refatoração foi provada
  **bit-exata** contra o modelo de pontos reimplementado consumindo o mesmo rng
  (0 divergências, 0.000000 ms) e a prova ficou como guarda permanente em
  `ModoDeRuido::Legado`. Depois disso o ruído passou a escalar com a distância da
  prova e a ter memória entre trechos.
- **Atrito de posição** (`race/trafego.rs`): ar sujo, trem de carros e
  ultrapassagem como *tentativa* em vez de ordenação. Consome o
  `overtaking_difficulty_multiplier` e a defesa, que eram calculados e nunca
  lidos por ninguém.
- **Estratégia e safety car** (`race/estrategia.rs`): parada como IA de equipe,
  determinística para não perturbar a equivalência, e safety car com
  relançamento — sem o relançamento ele não trocava vencedor nenhum.
- **Classificação como disciplina própria** (`qualifying.rs`): melhor de N
  tentativas com compensação analítica de amplitude, trim de quali do carro
  derivado do `car_shape`, e o evento de volta perdida.
- **`rain_sensitivity` ligado** (`math.rs`) pela curva validada
  `rain_skill_penalty`, não pelo multiplicador antigo, para não cobrar chuva duas
  vezes e preservar a paridade com o export do iRacing.
- **Harness de calibração** (`simulation/calibracao/`): gerador de grid alinhado
  com `driver_generation`, decomposição de variância, métricas de processo,
  varredura de sensibilidade por par knob × saída, máquina de busca com portão de
  decomposição, e ingestor de resultado real do iRacing.

**Dois bugs achados por medição:**

1. `run_pending` comparava a versão com `CURRENT_VERSION`, então a primeira
   migração depois do colapso da baseline recusaria **todo** save existente.
   Introduzida `BASELINE_VERSION` — o teste que existia passava alegremente.
2. O clamp em `cumulative_score` fazia reparo cedo em carro do fundo do grid
   custar **menos** que os segundos parados no box: quanto mais atrás, mais
   barato quebrar. Inversão de incentivo invisível na moeda de pontos e
   impossível na de tempo.

> ⚠️ **As constantes novas ainda não foram calibradas** — todas marcadas
> PROVISÓRIAS em blocos únicos, e o portão de decomposição de variância é
> obrigatório no ponto final, porque as métricas de resultado não distinguem
> campeonato disputado de campeonato sorteado. Mesmo assim o sintoma central caiu
> de **0,976 → 0,83** (alvo 0,20–0,55) e o desvio de posição na temporada subiu
> de **0,71 → 2,19**, sem uma constante calibrada.

Complementos em `2c85f44`: `category_adaptation.rs` e `race/merito.rs` (campo de
mérito do grid). Em `8eb18c2`: esteira de calibração, prévia e assinatura em
`simulation/calibracao`, mais o harness de medição nos testes de corrida.

---

## 2. Dossiê de equipe v2 (`a7f8261`)

Atrás do seletor em `components/team/history/index.js`. Voltar para o v1 é
reverter uma linha; nenhum arquivo do v1 desenha diferente por causa do v2.

**Records**
- Grid assimétrico: as três contagens numa linha, as duas taxas noutra, com média
  do grupo, barra de posição e rank por extenso em cada card.
- Faixa "top 5 por corrida": uma coluna por temporada, altura = fatia das corridas
  terminadas no top 5, repartida em 1º/2º/3º/4º-5º. Escala **por corrida** porque
  somar pontos mistura calendários e categorias incomparáveis. Janela de 15
  temporadas, anunciada ao lado do título.
- Três estados na coluna: trilho vazio (correu, sem top 5), ícone de categoria na
  cor da escada (correu **fora** do recorte comparável) e "×" (não disputou).
  Antes o terceiro engolia o segundo e o gráfico afirmava que a equipe tinha
  sumido do mundo num ano em que ela correu outro campeonato.
- Tira de categoria sob as colunas: uma célula por ano, alinhada por construção.
- Galeria de títulos: régua de todas as temporadas com os anos de título marcados
  (anel dourado na dobradinha), cabeçalho por categoria com o resumo do reinado e
  tabela densa de ano/pts/V/campeão de pilotos. A versão em cards repetia a
  categoria e a mesma frase seis vezes e escondia o que era a história: seis
  títulos seguidos. Título único colapsa numa linha.

**Esportivo**
- A tabela temporada a temporada saiu: era a faixa de Records em números. Das
  suas colunas, POS virou a curva de campeonato e PTS caiu pelo mesmo motivo que
  saiu do gráfico (incomparável entre calendários).
- Forma recente: as últimas 10 corridas, o único bloco que fala do presente.
  Marca a troca de categoria no meio da fita, senão a queda após uma promoção se
  lê como perda de forma.
- Curva de campeonato: eixo invertido, faixa do pódio, tira de categoria embaixo.
  A linha **quebra** em temporada sem posição conhecida em vez de ligar por cima
  do buraco.
- Assinatura de resultados: todas as corridas repartidas em
  1º/2º-3º/4º-5º/6º-10º/fora. Records dá a taxa de pódio; isto dá a forma dela.
- Marcos e linha do tempo eram dois blocos contando a primeira vitória cada um
  com sua frase; viraram uma cronologia só, fundida por `kind` e não por prosa.

**Backend**
- `rank_for_titles` passa a rankear contra todas as equipes que correram no
  grupo. Antes comparava só contra as campeãs, e o dossiê mostrava denominadores
  diferentes lado a lado ("10º de 10" junto de "14º de 19").
- Colocações exclusivas por corrida via `best_position` (`MIN(posicao_final)`):
  uma dobradinha 1º-3º contava duas colocações na mesma corrida.
- Novos campos: `category_id` nas temporadas e nos títulos, pontos/vitórias e
  campeão de pilotos do ano do título, forma recente, distribuição por faixa,
  intervalo de anos do mundo e temporadas fora do recorte.
- "Melhor temporada registrada: N pts" saiu da linha do tempo: Records já tem um
  card com o mesmo nome medindo outra coisa, e os dois liam como contradição.

### 2b. Curva de campeonato — segunda passada (`36a47f0`)

Era quatro pontos ligados por um fio dentro de um retângulo vazio, com o pódio
como bloco verde chapado e a tira de categoria virando uma barra contínua que
competia com a própria curva.

- Área preenchida sob a linha, em gradiente da cor da equipe, quebrando junto com
  a linha nas temporadas sem posição fechada.
- Chip de posição sobre cada ponto: entre P4 e P6 não havia como ler P5. Abaixo
  de 52 px por coluna sobram só os títulos e a última temporada fechada, senão a
  etiqueta vira tarja.
- Badge do pódio com troféu na linha do título, **fora** do desenho: dentro dele
  disputava espaço com a curva justamente nas temporadas boas.
- Brilho na linha, halo nos pontos e recorte na cor do card, para o marcador não
  se dissolver no traço quando a equipe repete a posição.
- Eixo Y desenhado, troféu marcando P1 e guias verticais por temporada.
- Coluna da temporada em andamento marcada, sem inventar ponto.

---

## 3. iRacing, overlay e VR (`d4c55e8`, `2c85f44`, `8eb18c2`)

- **Camada de VR**: `commands/vr_layer.rs` detecta o runtime ativo por evento
  nomeado e lê o caminho de instalação do registro; `scripts/build-vr-layer.mjs`
  compila a API layer do OpenXR como artefato (fora do git) e o
  `installer/hooks.nsh` registra/limpa a camada na instalação.
- **Overlay**: painel de posição novo, animação de linhas da torre, escrita VR,
  `ordem.rs` nos comandos de overlay, ajustes nos writers de VR e no
  `overlay_layer.cpp`.
- **Diagnóstico do iRacing**: módulo novo em três camadas — `diagnostico.rs` no
  domínio, `iracing_sdk/imp/diagnostico.rs` na leitura e
  `commands/iracing/diagnostico.rs` na ponte — com `IracingDiagnosticoPanel` e
  `IracingSemDadosAviso` no frontend.
- **Parser de sessão** corrigido para lista YAML; gate `in_race_session` no
  snapshot de grid; importação mais robusta.

---

## 4. Atlas de equipes v2 (`8eb18c2`)

Nova aba de equipes globais (`pages/tabs/atlas`) com chrome, rankings, galeria de
campeões, gráfico e normalização de logos. `TeamRecordsTab` e o dossiê ganham
pilotos e campeões como módulos próprios.

---

## 5. Outros sistemas

- **Finanças**: fatura de operação extraída para `finance/operacao.rs`; dossiê
  financeiro no MyTeam.
- **Narrativa**: `race_signals.rs` unifica os limiares de remontada, colapso e
  DNF — antes cada consumidor tinha o seu.
- **Corrida (UI)**: `RaceCoursePanel` e `WeekendReadingPanel` no fim de semana.
- **Saves**: modal de backups em `LoadSave`.

---

## 6. Correções

| commit | o que era |
|---|---|
| `ea4366d` | `get_previous_champions_in_base_dir` devolvia sempre vazio — o `if` sobre `season.numero > 1` era decorativo e o caminho de baixo retornava `driver_champion_id: None` mesmo assim. Ficavam mudos: o selo de campeão reinante na classificação, a prévia de temporada (sempre "trono vago") e os campeões de construtores no multiclasse. A query que resolve já existia e nunca tinha sido ligada. **No mesmo commit**: `refresh_planned_hierarchy_for_team` tinha dois braços de `match` para `UpdateHierarchy` devolvendo `Some(event.week)`, um com guarda `current == team_id` e outro sem — o catch-all tornava a guarda inócua e o `max` pegava a semana de qualquer equipe. Estava assim desde o commit inicial do repositório. |
| `ef98208` | Calendário de endurance: a regra é "final forte **mais** ao menos uma âncora de miolo", mas o Passo 2 rodava sob `if !has_strong_in_narrative` — tratava o miolo como plano B. Como `strong_last` quase sempre entrega pista forte na final, o passo era pulado e a temporada ficava com um único evento forte (1 em cada 20 seeds). Agora conta-se quantos fortes já foram reservados e completa-se até o mínimo de dois. |
| `01aa81d` | Fixtures de migração e de rivalidade ficam fiéis ao schema. |
| `b48bb01` | Suíte estrutural volta a ser sinal — 29/29 verdes. |
| `2bb68ac` | Guard do modo campeão volta a varrer os slices do store. |

---

## 7. Refatoração — 123 commits

O grosso do release. Nenhuma mudança de comportamento; o objetivo foi quebrar
monolitos em submódulos por assunto, com `mod.rs` virando índice.

**Banco**: `1f9b597` colapsa as **53 migrações numa baseline única**
(`migrations.rs`: −5.169 / +641 linhas), precedido por `43d165a`, que capturou o
schema-ouro gerado pelas 53 antes do colapso.

**Backend (Rust)** — módulos fatiados: `career.rs` (→ `queries`, `standings`,
`briefing`, `save_state`, `lifecycle`, `interests`, `market_window`,
`season_flow`, `vacancies`, `debug`), `iracing.rs` (→ fachada + 14 irmãos),
`race_monitor.rs` (→ 13 irmãos), `race.rs` de comandos, `market/pipeline.rs`,
`convocation/pipeline.rs`, `simulation/{race,incidents,profile}.rs`,
`narrative/mod.rs`, `calendar/`, `rivalry/`, `constants/{teams,tracks}.rs`,
`models/enums.rs`, `career_types.rs`, `iracing_sdk/mod.rs`, `save.rs`,
`preseason.rs`, `car_maintenance.rs`, `sim_stats.rs`, `ai_news.rs`,
`global_driver_rankings.rs`, `season_preview.rs`, `world_footer.rs`,
`race_history`, `breakdown.rs`, `historical_draft.rs`, `career_detail.rs`,
`telemetry_analysis.rs`, `behavior.rs`, `weather.rs`, `result_bridge.rs`,
`full_season.rs`, `generators/{names,world}.rs`. Blocos `#[cfg(test)]` inline
migrados para `<modulo>/tests/`.

**Frontend (React)** — `useCareerStore` virou composição de 5 slices
(`careerSlice`, `raceSlice`, `marketSlice`, `seasonSlice`, `preRaceCacheSlice`).
Telas fatiadas: `PreSeasonView`, `NextRaceTab`, `MyTeamTab`, `StandingsTab`,
`GlobalDriversTab`, `GlobalTeamsTab`, `CalendarTabRedesign`, `NewsMagazineTab`,
`ConvocationView`, `RaceResultView`, `DriverDetailModal`, overlay.

**Dedup de helpers** (`2c85f44`, varredura F1–F4): `src/lib/tauri.js` vira fonte
única da detecção de Tauri (antes repetida em **11 arquivos**), `chartTheme.js`
centraliza `formatLap` e a paleta dos gráficos, `teamColors.js` absorve
`getReadableTeamColor` (antes triplicado), e `weather`/`trackCountry`/
`trackBanners` saem de `calendarShared`. Cada um com teste novo, mais o guard
estrutural `tauri-detection-single-source`.

**Limpeza**: `102bec7` remove o seletor de versão do calendário; `31dd713` remove
telas placeholder órfãs.

---

## 8. Documentação e ferramentas

- `3519af7` — `CLAUDE.md`, o guia do repositório.
- `docs/varredura-acoplamento/` — nove briefings; `docs/backlog.md`;
  `docs/roadmap.md` com o porquê de cada buraco; `docs/iracing-escopo.md`;
  `docs/varredura-bugs-2026-07.md`; briefings de F-06, F-07, F-10 e D-09.
- `.claude/skills/` — seis skills do repositório.

---

## Suítes na data do build

1865–1871 testes cargo · 428 vitest · 31 estruturais.
