# Dívida técnica — Loop

Registro do que **já foi resolvido**. As pendências abertas vivem em
[backlog.md](backlog.md), com id estável (`D-xx`), tamanho e motivo.

A divisão existe para que os dois documentos não briguem: aqui é o que fechou e como; lá é o
que falta. Até 11/08/2026 este arquivo terminava dizendo "nenhuma pendência registrada" — o
que se lia como "não há dívida", enquanto o backlog listava de D-01 a D-09 em aberto. A
seção final foi trocada por um ponteiro para o backlog.

Item que fecha sai do backlog e entra aqui, com a data e o que provou o fechamento.

---

## Itens resolvidos

### DB-001 a DB-004 — Schema de `teams` normalizado

**Resolvido na migration v28.**

Foram removidas as colunas legadas da tabela `teams`:
`reliability`, `prestige`, `temp_pontos`, `temp_vitorias`, `carreira_vitorias`.

As queries passaram a usar os nomes atuais do domínio:
`confiabilidade`, `reputacao`, `stats_pontos`, `stats_vitorias`, `historico_vitorias`.

A leitura de `Team` também deixou de usar placeholder permissivo. Campos obrigatórios agora
são lidos de forma explícita, fazendo a consulta falhar caso o schema esteja desalinhado, em
vez de carregar valores padrão em silêncio.

---

### F-06 — Backup e restauração de save

**Resolvido antes de 11/08/2026; o registro é que estava faltando.**

`src/components/ui/BackupsModal.jsx` consome `list_backups`, `create_season_backup` e
`restore_backup`, e é aberto por `src/pages/LoadSave.jsx`. O backlog ainda descrevia os três
comandos como "registrados e nunca chamados".

---

### D-03 e D-04 — Stores stub e `hooks/useTauri.js`

**Resolvidos em 11/08/2026.** `useUIStore.js` e `useNotificationStore.js` eram stores Zustand
vazios sem nenhum consumidor; `useTauri.js` era um stub com `// TODO`. Os três foram
removidos. Os `invoke` seguem vindo direto de `@tauri-apps/api/core`, por decisão.

---

### Árvore V1 da tela de resultado de corrida

**Removida em 11/08/2026.** `src/components/race/RaceResultView.jsx` mais o subdiretório
`race/raceresult/` inteiro (15 arquivos, 1.681 linhas com o teste) não tinham nenhum
importador vivo: o `Dashboard` usa `RaceResultViewV2` direto, e a única referência externa
era um caso de teste de `src/utils/weather.test.js` sobre um helper daquela árvore.

Agravante que motivou a remoção em vez do adiamento: os 13 arquivos carregavam
`i18n-ignore-file`, então eram código morto que ainda escapava do guard de i18n — cada
varredura de tradução passava por eles sem cobrar nada.

**As outras duas árvores V1 ficaram, e de propósito.** `DriverDetailModal` (v1) e
`TeamHistoryDrawer` (v1) estão vivos atrás de um seletor de versão explícito
(`driver/index.js` e `team/history/index.js`, com `VERSION = 2` e o comentário "voltar para o
v1 é reverter esta linha"). Isso é uma alavanca de rollback declarada, e não código
esquecido. Cortar exige decidir que o rollback não é mais necessário — decisão de produto,
não de limpeza.

---

### Mapa de categoria para logo, quintuplicado

**Unificado em 11/08/2026** em `src/utils/categoryLogos.js`.

O dicionário categoria → arquivo de brasão vivia copiado em cinco lugares (torre do overlay,
atlas de equipes, pré-temporada, convocação e calendário), cada cópia conhecendo um
subconjunto diferente de categorias. Categoria nova entrava em duas telas e nascia sem
brasão nas outras três, sem erro nenhum: o `?? null` devolve nada e o selo simplesmente não
desenha.

Guard: [`categoria-logo-fonte-unica`](../scripts/tests/categoria-logo-fonte-unica.test.mjs)
cobra que as 9 categorias da escada do Rust tenham arte nas duas variantes (recortada e com
moldura) e que nenhum arquivo do frontend volte a remontar o caminho na mão.

---

### Vãos do auditor de i18n

**Fechados em 11/08/2026.** O `scripts/i18nAudit.mjs` enxergava duas das cinco formas de
escrever copy num `.jsx`. As três que faltavam deixaram passar 8 strings de português vivas
em produção, todas em telas que já estavam traduzidas no resto:

- nó de texto sozinho na linha (o que o Prettier produz para qualquer rótulo longo);
- atributo de UI escrito como template literal (`aria-label={\`Ver títulos de ${nome}\`}`);
- prosa colada a uma expressão (`<span>Criado: {x}</span>`).

O padrão dos vãos é o mesmo: eles pegavam justamente a copy LONGA e a copy COM NÚMERO, que
são as duas mais comuns.

Junto disso, o guard de acentuação
([`portuguese-copy-accents`](../scripts/tests/portuguese-copy-accents.test.mjs)) foi trocado:
ele listava 11 arquivos `.jsx` com fragmentos proibidos escritos na mão, e a copy já tinha
migrado para o locale do i18next, onde ninguém olhava. Passou a varrer
`src/i18n/locales/pt-BR/common.json` inteiro. A troca encontrou 29 strings sem acento que o
jogador lia na tela ("Indice", "Vitorias", "Podios", "Lesoes", "Campeoes", "Titulos",
"Classificacao", entre outras) — todas corrigidas.

---

### Lógica pura presa dentro dos dois arquivos-deus do v2

**Extraída em 11/08/2026**, seguindo o caminho que `atlasV2Geometry.js` e `gridMetrics.js` já
tinham aberto:

- `DriverDetailModalV2.jsx` (5.505 linhas) → `driver/v2/driverDetailV2Logic.js` + teste
  espelho com 60 casos;
- `TeamHistoryDrawerV2.jsx` (4.525 linhas) → `team/v2/teamHistoryV2Logic.js` + teste espelho
  com 27 casos;
- `OverlayPositionPanel.jsx` (682 linhas) → `overlay/overlayPose.js` + teste espelho.

O corte é sempre o mesmo: sai o que decide CONTEÚDO (ordem, corte, cor derivada de dado),
fica o que decide DESENHO (JSX, geometria de SVG, estado de realce). Os arquivos continuam
grandes — o objetivo aqui não foi encolher número, foi tirar a regra de negócio de dentro do
render, onde ela só podia ser testada montando o modal inteiro.

---

### D-09 — Varredura de acoplamento do lado Rust (R1 a R5)

**A dívida técnica dos cinco briefings fechou em 11/08/2026.** Os cinco foram reconferidos
afirmação por afirmação contra o código, na mesma sessão, para não ter duas frentes editando
`narrative/`. A classificação de cada afirmação está no bloco "Situação em 11/08/2026" de
cada arquivo em [varredura-acoplamento/](varredura-acoplamento/); o quadro geral está no
[README](varredura-acoplamento/README.md).

O que veio de código:

- **R1** — `#![allow(dead_code)]` fora de `narrative/mod.rs`, e o que ele escondia corrigido:
  três reexports `pub use` que não exportavam nada, `Beat.driver_id` e `Beat.team_name`
  (dez escritas, zero leituras), e a struct `RaceContext` — o único chamador lia só `facts`,
  então `build_race_context` passou a devolver a `String`. Os textos de `StoryError::Server`
  e `::Network` eram construídos e descartados por todo callsite: um 5xx (os dois provedores
  caídos) e uma queda de rede chegavam como o mesmo erro mudo, sem rastro no `loop.log`.
  Passaram a ser registrados.
- **R4** — `#![allow(dead_code)]` fora de `hierarchy/{orders,transition}.rs`. `DuelResult`
  perdeu `team_id`/`n1_id`/`n2_id`, calculados a cada duelo e lidos por ninguém. O par
  `decide_hierarchy_transition` / `resolve_transition_values`, mais o enum e as duas structs
  que só ele usava, saiu: nunca teve chamador de produção, só os próprios testes, e a
  preservação parcial de tensão entre temporadas que ele descrevia nunca foi ligada. Dois
  comentários que afirmavam que as equipes chegam alinhadas "pelo `UpdateHierarchy` do
  mercado" foram corrigidos para apontar `market::pipeline::consolidacao`, que é quem
  escreve. `TENSAO_EQUILIBRIO_TAXA_N2` ganhou o teste que a prende aos deltas.
- **Sujeira trivial** — a árvore vazia `src-tauri/src/src-tauri/` foi removida.

**R2, R3 e R5 já estavam resolvidos** e o registro é que faltava: campo de mérito único em
`commands/race/merito.rs` e camada de sinais em `race_signals.rs` (R2); o enum de tier de
equipe removido de `public_presence/team.rs` e o comentário obsoleto de `market/visibility.rs`
corrigido (R3); as três funções legadas de simulação já não existem, e `run_market` fica com
`cfg_attr` e contrato escritos (R5).

**Nenhum valor de calibração foi tocado**, e nenhuma consequência de gameplay foi inventada.

---

### Rodada de higiene de 11/08/2026 (segunda passada)

Rodada só de dívida concreta: nenhuma feature, nenhuma calibração, nenhum offset.

**Imports e reexports mortos.** Removidos 15, todos conferidos por `grep` antes: `HashSet` em
`career_team_dossier.rs`, `OptionalExtension` em `career_detail/recordes.rs`, o reexport
`part_com_seu` em `overlay/avisos.rs` (a função só é usada dentro de `engenheiro/peca_propria.rs`),
os 3 reexports de fachada de `economia/mod.rs` (apontavam para a "troca da seção 3.9" e nunca
ganharam consumidor; os itens continuam públicos nos submódulos), `FuelSummary` e
`SectorAnalysis` em `telemetry_analysis.rs`, `category_finance_scale` em `market/preseason.rs`,
`WindowResult` em `market/transfer_window.rs`, `rand::Rng` e `rain_intensity_for` em
`simulation/race/pontuacao.rs`, `IncidentSeverity` e `IncidentType` em
`simulation/race/resultados.rs`, mais 4 do lado de teste.

**Um falso positivo do compilador, e vale registrar o padrão.** `OptionalExtension` em
`db/migrations.rs` aparecia como morto no `cargo check --lib` e NÃO estava: é usado em
`migrations.rs:785`, dentro de `mod tests`. Removê-lo quebrou o build. Voltou sob `#[cfg(test)]`.
Mesma natureza: `CURRENT_VERSION` também é sinalizado "never used" pelo build da lib e só é
consumido pelos testes. **Antes de apagar um item que o `--lib` diz morto, confira se o
consumidor é `cfg(test)`.**

**Fachadas glob NÃO foram tocadas, de propósito.** Sobram 21 warnings de `unused import` em
blocos do tipo `pub use submodulo::*;` (`simulation/race.rs`, `iracing_sdk/behavior.rs`,
`calendar/mod.rs`, `rivalry/`, `commands/overlay.rs` e outros). São o padrão de fachada do
módulo — `calendar/full_season.rs` chega a documentá-lo ("manter idênticos todos os caminhos
públicos do módulo"). Apagar linha a linha muda a superfície pública sem ganho de runtime.
O mesmo vale para `use super::*` em `commands/iracing/spotter.rs`, que é o padrão em 12 dos
15 irmãos do diretório.

**Comandos órfãos.** Ver D-05 no [backlog.md](backlog.md): 201 → 198 registrados, 24 → 21 órfãos.

**D-07 — venues ausentes: falso positivo, era comentário velho.** O cabeçalho de
`calendar/generator.rs` dizia que os pools incluíam "tracks ainda ausentes do DB (marcadas com
`// TODO`)" e mandava filtrar com `get_track(id).is_some()` "ao wiring no gerador real". As duas
afirmações venceram: cruzando todo id de pool com `constants::tracks::dados` (107 pistas),
**zero ids ficam fora do catálogo**; não há mais nenhuma marcação `// TODO` de pista; o filtro
`is_some()` já está aplicado (linhas 471, 502 e 528); e o gerador já está ligado
(`calendar/geracao.rs` chama `resolve_thematic_pool` nas duas rotas). Só o comentário foi
corrigido — nenhum dado de catálogo mudou.

**D-08 — TODO de migration do `models/driver.rs`: a afirmação era verdadeira, faltava a prova.**
Os 19 campos de `DriverAttributes` têm coluna na tabela `drivers`. O TODO saiu e no lugar ficou
o ponteiro para o teste que sustenta a frase:
`db::queries::drivers::tests::todo_atributo_do_piloto_tem_coluna_e_volta_igual_do_banco`. Ele
monta um piloto com um valor DISTINTO por atributo, grava e relê contra o schema real
(`migrations::run_all`), e não contra o DDL de bancada do `setup_test_db` — que é cópia à mão e
pode divergir da baseline sem ninguém perceber. Valor distinto por campo é o que faz uma troca
de duas colunas na `INSERT` ou na `driver_from_row` quebrar o teste.

---

### Achado novo: `race_weekend_readings` nunca é escrita em produção

`db/queries/races.rs` tem o par `set_race_weekend_readings` / `get_race_weekend_reading`. O
getter é lido em produção (`commands/ai_news/fatos.rs:809`). **O setter não tem nenhum chamador
fora dos testes.** O doc-comment do getter diz "se voltar `None`, quem chama deve calcular e
gravar via `set_race_weekend_readings`" — esse ponto não existe no código.

Consequência: a leitura anunciada do fim de semana só existe nas linhas que a migração v57
(`migrate_v57_leitura_do_fim_de_semana`) semeou. Corrida nova nunca grava, e o getter devolve
`None` para sempre.

Não corrigido nesta rodada: ligar o setter é escrever feature, e a rodada era de higiene.
Precisa de decisão sobre QUEM grava (a Sala de Estratégia, no preparo do fim de semana).

---

### `advance_transfer_window` — comando no-op removido

**Removido em 11/08/2026, junto com o fechamento do F-01.**

Era o item mais antigo de `SEM_CONSUMIDOR_CONHECIDO` no guard
[`invoke-contra-generate-handler`](../scripts/tests/invoke-contra-generate-handler.test.mjs),
classificado ali como "feature futura, esperando o F-01". O F-01 chegou, e a tela que o item
pedia — a seção Mercado da aba Carreira, apagada em 14/08/2026 junto com a aba — recusou
ligá-lo.

O motivo é que ele nunca avançou nada. `advance_transfer_window_in_base_dir` tinha o corpo
IDÊNTICO ao de `get_transfer_window_state_in_base_dir`, e o único parâmetro que os distinguia
(`accepted_seat_id`) era recebido como `_accepted_seat_id` e ignorado. O próprio doc-comment
dizia: "ficou legado: apenas devolve o estado atual das ofertas, sem avançar nada". Quem avança
o mercado é `advance_market_week` → `preseason::advance_week`, que a pré-temporada já conduz.

**A lição de método:** comando registrado sem consumidor é indício de tela que falta OU de
código morto, e as duas hipóteses se separam lendo o CORPO da função, não a lista do
`generate_handler!`. Este passou dois anos na primeira classificação porque ninguém abriu o
arquivo. O `roadmap.md` §3 chegou a escrever que a existência dele era "indício de que a janela
de transferências no meio do ano existe no backend e não tem condução na UI" — o indício era
falso, e a correção está registrada lá.

---

### Vistoria independente da série V (11/08/2026)

A vistoria inteira está em [vistoria-independente-2026-08-11.md](vistoria-independente-2026-08-11.md),
com o achado original, a evidência e o critério de aceite de cada id. **Aquele documento é
histórico**: ele descreve a árvore como ela estava no dia, e não deve ser reescrito quando um item
fecha. O que fechou está aqui.

Fechado e conferido contra a árvore em 11/08/2026:

| Id | O que fechou | Onde provar |
|---|---|---|
| V1.1 | Categoria → carro vira fonte única e passa a recusar. Acabou o `else → mx5` que exportava GT3, GT4, LMP2, Production e Endurance como Mazda MX-5 | `commands/iracing/exportavel.rs`, teste `o_catalogo_inteiro_e_mapeado_ou_recusado_explicitamente` |
| V1.2 | O aiseason deixa de ler `duracao_corrida_min` da categoria. A sentinela `0` morre na cascata e a divergência entre etapas é recusada em vez de achatada | `exportavel::race_length_da_temporada`, `commands/iracing/temporada.rs` |
| V1.3 / V1.4 / V8.2 | Os guards pararam de dar falso verde: o de flags varre a árvore inteira em vez de três arquivos escolhidos à mão, e o de comandos deixou de contar literal em teste e `invoke` de módulo órfão como consumidor vivo | `constants/flags_experimentais.rs`, `scripts/tests/invoke-contra-generate-handler.test.mjs` |
| V2.1 | Save de schema mais NOVO que o binário é recusado com mensagem, no lugar de abrir e operar sobre um schema desconhecido | `db/migrations.rs::verificar_compatibilidade_do_schema` |
| V2.2 | `preseason_plan.json` sai da transação: escreve em staging e só publica depois do commit, com `Drop` limpando o staging em qualquer saída de erro | `market/preseason/plano.rs::PreSeasonPlanStaging`, `evolution/pipeline/orquestracao.rs` |
| V2.3 | Restauração passa a ser atômica (`substituir_preservando_anterior`) e confere a compatibilidade de schema do backup antes de tocar no banco vivo | `commands/save/restore.rs` |
| V4.1 | `incident_catalog` guarda chave, não prosa. Migração v65, com a prosa de id fora dos 54 preservada em `incident_catalog_texto_legado` | `db/migrations/incident_catalog_chaves.rs`, teste `troca_de_locale_muda_o_texto_do_mesmo_save` |
| V4.2 (mecânico) | As três strings PT cruas do módulo de notícias viraram chave de `rust-i18n`. O "desfalque confirmado" em minúscula foi junto | `commands/race/noticias/manchetes.rs` |
| V4.3 | O auditor de i18n passou a varrer `.js`, com o passivo congelado em `scripts/i18nBaseline.mjs` e baseline órfã falhando o auditor | `scripts/i18nAudit.mjs`, `scripts/tests/i18n-audit-staged.test.mjs` |
| V4.4 | O carrossel de carregamento perdeu a segunda fonte de verdade: a contagem sai do próprio locale e o teste assevera as chaves de pt-BR | `src/pages/NewCareer.jsx`, `src/utils/constants.test.js` |
| V5.1 | As três flags de regra que escapavam (`IRACER_GATE_SHARE`, `IRACER_SALARY_SHARE`, `IRACER_TRACK_RIVALRY`) estão no inventário, e env de ambiente precisa de motivo escrito na allowlist | `constants/flags_experimentais.rs` |
| V6.2 | `.catch(() => {})` cru proibido no caminho de corrida e nos stores; no lugar dele `bestEffort`, que registra no `loop.log` | `src/utils/bestEffort.js`, `scripts/tests/catch-vazio-no-caminho-de-corrida.test.mjs` |
| V6.3 | CSP explícita no `tauri.conf.json` (era `null`) e o corpo da caixa de entrada deixou de ser HTML concatenado | `scripts/tests/csp-e-sink-html.test.mjs`, `src/components/news/MagazineMailbox.jsx` |
| V7.1 / V7.3 | Política de target-dir única, em `scripts/lib/cargo-target.mjs`, e o comando documentado no CLAUDE.md deixou de criar um terceiro target dentro do repositório | `CLAUDE.md`, `scripts/release.mjs`, `scripts/tests/release-target-unico.test.mjs` |
| V7.4 | A verificação final do release confere a estrutura do manifesto antes de ler a URL da plataforma, em vez de estourar `TypeError` depois do upload | `scripts/release.mjs`, `scripts/tests/release-manifesto-plataforma.test.mjs` |
| V8.1 | Os fake timers saíram de `RosterGenPanel.test.jsx`. Três execuções seguidas da suíte completa em 11/08/2026, sem falha | `src/components/iracing/RosterGenPanel.test.jsx` |
| V3.3 | Passou a existir teto de não-regressão rápido e NÃO ignorado para as duas propriedades centrais da simulação, enquanto a calibração do V3.1 segue aberta | `rookie_nao_piora_alem_do_teto_atual`, `gt3_nao_piora_alem_do_teto_atual` |
| V9.1 (parcial) | As bancadas soltas da raiz estão gitignoradas e o dump que regravava `__curva_preview.html` a cada suíte virou opt-in por `LOOP_PREVIEW_CURVA=1`, escrevendo no temp do sistema | `.gitignore` |

O que a série V deixou em aberto, e por quê, está no [backlog.md](backlog.md).

---

## Pendências abertas

Estão em [backlog.md](backlog.md): D-01 (fases legadas da convocação — reenquadrado, a feature
de convocação está viva), D-02 (tabela `races` órfã — medida, exige `DROP TABLE` em migração),
D-05 (comandos sem consumidor — o motivo de cada um congelado no guard, **que é a contagem**; este
arquivo dizia 21 e a lista se moveu duas vezes depois disso), D-06 (TODO de
design) e D-10 (`planned_events` write-only no plano de pré-temporada, achado do D-09/R4 e
separado por tocar formato de save). **D-07 e D-08 fecharam em 11/08/2026** (ver acima).

Aberto e fora de higiene: `race_weekend_readings` sem escritor de produção (acima).

**Corrigido em 18/08/2026:** este parágrafo dizia que `overlay_window_set_interactive` sem
consumidor deixava o overlay "preso em click-through". O código diz o contrário. Quem alterna o
clique-atravessa em produção é o VIGIA DE CURSOR de
[overlay_window.rs:155](../src-tauri/src/commands/overlay_window.rs), que chama
`set_ignore_cursor_events(!inside)` a cada entrada e saída do mouse na caixa da janela, e o
comentário do próprio comando (linha 220 em diante) já registrava isso. O comando é a alavanca
MANUAL paralela ao vigia, e um `interactive` forçado por ali vale até o próximo tique. O que
sobra é o comando sem consumidor, que é o D-05 e vive no inventário congelado do guard: o
overlay funciona.

Do D-09 sobrou o que não é dívida técnica: promover forma, lesão-arco e marcos a beat do
boletim (design), dar consequência nova à hierarquia interna (design) e o frontend recalcular
`dismal` em JS (opcional). **O eixo de tensão saiu da lista em 11/08/2026**: medido pelo harness
`calibracao_do_eixo_de_tensao`, o equilíbrio era 0,420 contra os 0,227–0,325 que o mundo produz, e
baixar `TENSAO_DELTA_N1_VENCE` de 2,0 para 0,5 trouxe para 0,308. Continua morto, mas por
aritmética e não por calibração, o **gatilho de inversão**: ele exige tensão ≥ 90 e o teto de uma
temporada perfeita do N2 é 67, com a virada zerando tudo — ligar isso é decisão de produto (limiar
de Crise em `models/team.rs` ou preservar a tensão em `hierarchy/transition.rs`).
