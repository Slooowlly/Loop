# Spec de Tradução / i18n (PT-BR ⇄ EN-US)

> Documento de design. Mapa único da execução — a tradução vai acontecer em várias
> sessões, então este arquivo é a fonte de verdade do que fazer, em que ordem, e o
> que **não** tocar.

## Decisões travadas

| Decisão | Escolha | Consequência |
|---|---|---|
| Estratégia | **Bilíngue** (PT/EN com chave, trocável em runtime) | Extrair todo texto pra um sistema de i18n; ligar o seletor que já existe |
| Servidor de IA (`iracer-news`, Cloud Run) | **Controlado pelo dev** | Dá pra traduzir os "fatos" e garantir o prompt honrando `lang` |
| Saves existentes | **Só saves novos** (sem migração de banco) | Conteúdo gerado nasce no idioma ativo; saves antigos ficam como estão |
| Limite do bilíngue | **Opção A — pragmático** | UI + conteúdo *novo* seguem o seletor; texto já **persistido no banco** fica congelado no idioma em que nasceu |

## Idiomas-alvo & roadmap

A arquitetura barateia idiomas adicionais: o **conteúdo de IA localiza sozinho** pelo
campo `lang` (o servidor traduz), então o custo real por idioma é só a **UI estática +
prosa determinística** (arquivos de locale). Contrapartida: cada idioma com **gênero/
plural** (como o PT) reintroduz a mesma máquina gramatical, e cada idioma mantido pelo
dev vira custo de manutenção contínuo (toda frase nova = N traduções).

| Prioridade | Idioma | Racional |
|---|---|---|
| **1 — obrigatório** | `en-US` (Inglês) | Plano atual. iRacing é majoritariamente anglófono (EUA/UK/AU). Destrava o mercado global |
| **2 — alto ROI** | `es` (Espanhol) | Espanha + LatAm. Mais barato: língua irmã do PT, reaproveita a máquina de gênero/plural quase direto |
| **2 — alto ROI** | `de` (Alemão) | Maior mercado de sim racing não-anglófono; base iRacing/hardware forte. Gramática mais cara (casos) |
| **3 — opcional** | `fr`, `it` | Comunidades sólidas, ROI menor que ES/DE. Latinas → custo de infra baixo. "Se a base pedir" |
| **Casa** | `pt-BR` (Português) | Baseline. Permanece |

**Escopo agora:** só **EN** (Fases 0–6 abaixo). ES/DE são os **próximos** — não
traduzir agora, mas **não fechar portas**. Isso impõe **2 exigências na Fase 0**:

1. A máquina de **gênero/plural/ordinal** deve ser **genérica por locale**, nunca
   hardcoded em `"pt"`. Assim ES/IT/FR entram só adicionando o arquivo de locale.
2. Confirmar que o servidor `iracer-news` aceita `lang` **arbitrário** (`es`, `de`,
   `fr`…), não só `pt`/`en`.

> Além de ~3–4 idiomas mantidos pelo dev, o caminho recomendado é deixar a estrutura
> de chaves pronta e **abrir tradução pra comunidade** (locales são só JSON/YAML).

## A espinha: 1 configuração → 3 sistemas

`config.language` (`"pt-BR"` \| `"en-US"`) é a **única fonte de verdade**. Ela aciona:

```
config.language
        │
        ├──► Frontend:  react-i18next  (locale ativo → t("chave"))
        ├──► Backend:   rust-i18n      (locale por request → t!("chave"))
        └──► IA:        campo `lang`    (já enviado → servidor responde no idioma)
```

- **PT vira o locale-base.** Não jogamos o português fora; ele passa a ser `pt-BR`
  na tabela. Isso deixa a extração *segura*: durante a migração, nada muda na tela
  porque o PT continua sendo servido.
- Ligar o seletor de `src/pages/Settings.jsx` (hoje morto — grava `config.language`
  mas ninguém lê) passa a trocar os três de uma vez.

### Mecanismos propostos (confirmar antes da Fase 0)

- **Front:** `react-i18next` + `locales/{pt,en}/*.json`. Plural nativo do i18next.
  Ordinais (`1º` vs `1st`) via helper próprio por idioma.
- **Back:** crate `rust-i18n` (`t!("key")`, locale por thread, arquivos YAML/JSON).
  O locale é setado no início de cada comando Tauri, lido de `config.language`.

## O limite do bilíngue (Opção A)

Texto gerado que **já foi gravado no banco** fica congelado no idioma em que nasceu
(notícias antigas, notas do "Mundo do Grid", inbox, cenários de quebra). Trocar o
idioma no menu **não** retraduz o que já existe. Aceito de propósito:

- UI e **conteúdo novo** seguem o seletor.
- Histórico persistido permanece no idioma de origem.
- Combina com "saves novos": o conteúdo passa a nascer no idioma ativo.

## Convenções

### Nomes de chave
- Namespaces por área: `common`, `nav`, `settings`, `race`, `market`, `team`,
  `driver`, `news`, `overlay`, `season`, `finance`.
- Padrão `namespace:contexto.item` — ex.: `nav:tab.news`, `settings:autosave.label`,
  `race:result.dnf`.
- **Mesma chave nos dois locales.** O `pt-BR` recebe o texto atual (baseline).

### Plural / ordinal / gênero
- **Plural:** usar plural nativo do i18next (`_one`/`_other`) no front e o mecanismo
  do rust-i18n no back. Aposentar os helpers manuais PT (`plural(n,"vez","vezes")`).
- **Ordinal:** helper `ordinal(n, lang)` → `12º` (pt) / `12th` (en). Centralizar num
  util; hoje está espalhado como `` `${n}º` `` inline.
- **Gênero:** o PT carrega concordância de gênero (o/a) que o inglês não tem. Nas
  chaves EN a variante some; nas PT mantém. Onde hoje há lógica de gênero inline,
  resolver via chaves separadas em vez de concatenação.

### Formatação sensível a locale
- **Moeda:** já é USD/`en-US` em todo lugar (`Intl.NumberFormat("en-US", USD)`).
  **Não mexer** — há comentário no código marcando USD como fonte única.
- **Datas:** hoje `toLocaleDateString("pt-BR")` / `Intl.DateTimeFormat("pt-BR")` e
  `formatCompactDate` hardcoda `DD/MM/YYYY`. Passar a usar o locale ativo.
  - `src/utils/formatters.js:6,13-18,54`
  - `src/components/season/PreSeasonView.jsx:1398`
- **Agrupamento de número:** `toLocaleString("pt-BR")` → locale ativo.
  - `src/pages/NewCareer.jsx:582`, `src/pages/tabs/NextRaceTab.jsx:2171`,
    `src/pages/tabs/GlobalDriversTab.jsx:1463`
- **Ordenação:** `localeCompare(…, "pt-BR")` → locale ativo (afeta ordem de listas).
  - `EndOfSeasonView.jsx:140`, `GlobalDriversTab.jsx:1054-1355`, `MyTeamTab.jsx:1315,1328`,
    `StandingsTab.jsx:1217`
- **Tempo relativo:** frases manuais em PT (`"Hoje"`, `"Ontem"`, `` `Há ${d} dias` ``).
  - `src/pages/MainMenu.jsx:55-60`, `src/utils/formatters.js:20-41`

## ❌ NÃO traduzir (lista de exclusão)

| Item | Onde | Motivo |
|---|---|---|
| Nomes de pista | `src-tauri/src/constants/tracks.rs` | Nomes próprios reais, já em inglês |
| Nomes de time | `src-tauri/src/constants/teams.rs` | Nomes próprios inventados, já em inglês |
| Nomes de piloto | `src-tauri/src/generators/names.rs` | Nomes reais por nacionalidade |
| Nomes de categoria/série | `src-tauri/src/constants/categories.rs` (`nome`) | Nomes próprios de série |
| **Tokens de enum** | `src-tauri/src/models/enums.rs` (`Ativo`, `Numero1`, `PreTemporada`…) | **Chaves de serialização no banco — renomear quebra saves.** Traduzir só na camada de label |
| Ids internos de peça/classe | `categories.rs`, `car/parts.rs` (ids) | Identificadores |

> **Regra de ouro dos enums:** os `as_str()`/`Display` dos enums em `models/enums.rs`
> e `news/mod.rs` retornam tokens de banco, **não** texto de tela. A tradução deles
> vive nos mapas de label do frontend (`utils/formatters.js`, `utils/constants.js`,
> `StandingsTab.jsx`, `GlobalDriversTab.jsx`, `EndOfSeasonView.jsx`).

> **Já bilíngue:** `src-tauri/src/generators/nationality.rs` já tem `nome_pt` **e**
> `nome_en` (+ formas femininas). Só selecionar `nome_en` quando o locale for EN.

---

## Plano em fases

### Fase 0 — Fundação ✅ CONCLUÍDA
- [x] Front: `react-i18next` + `i18next` instalados; init em `src/i18n/index.js`;
  locales em `src/i18n/locales/{pt-BR,en-US}/common.json`.
- [x] Front: `config.language` → i18next ligado no store (`useCareerStore.loadLanguage`
  e `setLanguage` chamam `applyLanguage`); boot aplica em `main.jsx`; seletor de
  `Settings.jsx` já vivo. Prova-de-conceito: "Geral/Idioma" via `t()`.
- [x] Front: helpers `ordinal`/`formatDate`/`formatCompactDate`/`formatNumber`/
  `localeCompare` em `src/i18n/format.js`, **genéricos por locale**. Plural/gênero
  ficam no i18next nativo (Intl.PluralRules + `context`), sem hardcode em `"pt"`.
- [x] Back: `rust-i18n = "3"` no Cargo; `i18n!("locales", fallback="pt-BR")` em
  `lib.rs`; locales em `src-tauri/locales/{pt-BR,en-US}.yml` (formato arquivo-por-locale,
  **sem `_version`**). Locale setado do config no boot (`setup`) e na troca (`update_config`).
- [x] Convenção de chave definida (ver acima); `pt-BR` semeado como baseline.
- [x] Testes: `src/i18n/i18n.test.js` (4 casos, verde) + `i18n_smoke` em `lib.rs`
  (`t!()` troca PT↔EN, verde).
- **Meta atingida:** nada muda na tela (PT = baseline); encanamento dos 3 sistemas de pé.

> **Refinamento vs. plano:** o locale do backend é **global** (1 usuário/1 idioma),
> setado no boot + em `update_config` — **não** é preciso setar "em cada comando".
>
> **Pendência p/ Fase 4 (manual, servidor externo):** confirmar que o `iracer-news`
> aceita os valores de `lang` enviados (`pt-BR`/`en-US` hoje; `es`/`de` no futuro).
> O app já envia `config.language` cru em `narrative/client.rs` — o mapeamento/normalização
> é server-side.

### Fase 1 — UI estática (frontend) — *volume alto, risco baixo*
Ordem: mapas centrais → chrome → aba por aba.
- [x] **Mapas de label — `src/utils/formatters.js`** ✅ (difficulty, license, season/
  preseason phase, attribute, countdown c/ plural, seasonYear, sufixo de salário).
  Funções agora leem `i18n.t()` (rodam no render, sem tocar consumidores). `App.jsx`
  ganhou `useTranslation()` pra re-render ao vivo na troca. Teste: `formatters.i18n.test.js`.
  Ficam p/ Fase 5: `formatDate`/`formatCompactDate`/`formatDateTime` (locale de data).
  Ficam como está: moeda (USD), `categoryLabel` (nome próprio), mapas de bandeira.
- [ ] **Mapas de label — `src/utils/constants.js`**: ⚠️ são **constantes avaliadas no
  import** (congelam no idioma do boot). Traduzir direito = converter em getters OU
  guardar `id` e resolver `t()` no consumidor (o wizard `NewCareer.jsx`). Fazer junto
  com o wizard. Alvos: `WIZARD_STEPS`, `DIFFICULTIES` (name/desc), `STARTING_CATEGORIES`
  (só `description`), `TEAM_PREVIEWS` (`country`). `NATIONALITIES` (gênero) e
  `LOADING_MESSAGES` (76 frases de prosa) → tratar como bloco à parte.
- [x] **Chrome/nav** ✅ (blocos 2+3). Casca da carreira: `TabNavigation.jsx`,
  `Header.jsx`, `PauseMenu.jsx`, `LeaveToMenuModal.jsx`. Shell do menu: `MainMenu.jsx`
  (+ `relativeTime`), `WindowControlsDrawer.jsx`, e o **shipping** do `Settings.jsx`
  (Voltar, autosave, seção Corrida, bandeira amarela + status, salvar, LoadingOverlay).
  Namespaces: `nav`, `pause`, `leaveModal`, `seasonBanner`, `raceBanner`, `weather`,
  `menu`, `closeApp`, `windowControls`, `settings.*`. Componentes usam `useTranslation`;
  helpers de módulo usam `i18n.t`. **i18n inicializado no `src/test/setup.js`** (pra
  testes que mockam o store). Testes: `navChrome.i18n.test.jsx` (troca ao vivo),
  `localeParity.test.js` (paridade PT/EN + sem vazios). Suíte: 331/331.
  ⚠️ **Adiado de propósito:** (a) `Settings.jsx` ferramentas de **debug/dev** (teste de
  chat, armar quebra, demo de rádio, gravador, detalhes técnicos) — fora do comercial;
  (b) Header `TRACK_COUNTRY` (nomes de país PT → **bloco de países**) e `BANNER_MONTHS_PT`/
  `formatBannerDate` (→ **Fase 5**, datas).
- [ ] **Modais:** `DriverDetailModal(.jsx/Sections.jsx)`, `PoachAuctionModal.jsx`,
  `IracingTutorialModal.jsx` (`LeaveToMenuModal.jsx` ✅ feito na casca).
- [~] **Abas** (menores/mais limpas primeiro):
  - [x] `CalendarTab.jsx` ✅ — namespace `calendar` (fases, legenda, detalhes, loading,
    erro) + `weather.dry`. Meses/dias-da-semana agora vêm de **Intl** via novos helpers
    `monthLongLabels`/`weekdayNarrowLabels` em `i18n/format.js` (locale-genéricos, servem
    à Fase 5). Consts `MONTH_NAMES`/`WEEKDAY_LABELS` removidas. Teste existente verde (PT).
  - [x] `StandingsTab.jsx` (Home) ✅ — namespace `standings` (loading/erro, headers de
    tabela, badges de piloto, séries/tiers de navegação, dividers de zona, standings de
    equipes, empty-states do bloco especial). SERIES/tiers (Mazda/Rookie/Championship…)
    ficam como nome próprio. Teste existente verde (19, PT).
  - [x] `GlobalDriversTab.jsx` ✅ — namespace `globalDrivers` (headers ordenáveis,
    filtros+opções, badges, modais de títulos/campeões, prosa de pódio/fama/rank com
    plural+direção, loading/empty). ⚠️ PT preservado **verbatim** (fonte tem acentuação
    inconsistente: "Titulos"/"Índice", "Campeoes"/"Campeões") p/ não quebrar testes.
    Valores-sentinela de filtro (Todos/Todas/Ativo…) ficam como dado. Teste verde (23, PT).
  - [x] `GlobalTeamsTab.jsx` ✅ — namespace `globalTeams` (timeline/gráfico: erros,
    zoom, scrubber inicio/fim/janela visível, familyWindow, campeão vigente). Aba com
    pouco texto (é chart). PT verbatim. Teste verde (30, PT).
  - [x] `Dashboard.jsx` ✅ — namespace `dashboard` (só o modal de conserto do carro;
    o resto é roteamento de abas). `repair_message` vem do backend → Fase 3. Suíte verde.
  - **Todas as abas "limpas" FEITAS.** Faltam só os **gigantes com prosa/IA** (deixar
    p/ Fase 2/4, misturam texto gerado): `NextRaceTab.jsx` (207), `MyTeamTab.jsx` (152),
    `NewsMagazineTab.jsx` (59), `PreSeasonView.jsx` (162), `RaceResultView(V2).jsx`,
    `EndOfSeasonView.jsx`, `ConvocationView.jsx`. Store `useCareerStore.js` (mensagens).
    Modais restantes: `DriverDetailModal`, `PoachAuctionModal`, `IracingTutorialModal`.
    Overlay (canvas): `towerCanvas.js` etc.
- [ ] **Overlay** (texto desenhado em canvas — não-DOM, cuidado): `overlay/towerCanvas.js`
  (107), `towerRows.js`, `EngineerRadio.jsx`, `OverlayPositionPanel.jsx`,
  `overlayMockData.js`.

### Fase 2 — Prosa gerada (frontend) — *alto, risco médio*
Geradores de sentença com gramática PT. **Padrão CRAVADO** (via inboxMessages): frase
inteira→chave i18next; código mantém a lógica de ramificação; plural via `_one/_other`+
`{{count}}` (nativo, genérico por locale); ordinal via `ordinal()` de format.js; gênero
via `context`; HTML embutido no valor (opção A). **Auditar display vs. fatos-de-IA
(Fase 4) ANTES de traduzir cada arquivo.**
- [x] `pages/tabs/inboxMessages.js` ✅ — 100% display (comentário confirma "nada de IA").
  Namespace `inbox` (attr/fama/h2h/fav/interest). Concordância `dessa/dessas` dissolvida
  em plural. Teste `inboxMessages.i18n.test.js` (5, PT+EN, plural/ordinal/HTML) verde.
- [x] `utils/postRaceLanding.js` ✅ AUDITADO — **lógica pura, ZERO prosa** (só decide aba
  pós-corrida + localStorage). Nada a traduzir.
- [x] `utils/driverMentions.jsx` ✅ AUDITADO — **lógica pura** (realça nomes via regex).
  Nada a traduzir.
- [x] `pages/tabs/nextRaceThesis.js` ✅ — AUDITADO: o `statement` é o EIXO que serve
  **display fallback E fatos-de-IA** (comentário confirma "fonte só") → traduzir cobre
  Fase 2+4. Namespace `thesis` (12 teses + fragmentos + títulos + stageAppendix). Atalho
  "(s)" mantido literal (a IA reescreve). `THESIS_TITLES` const removida → título via
  `i18n.t` em selectThesis. Testes de estrutura preservados + `nextRaceThesis.i18n.test.js`
  (PT+EN). ⚠️ armadilha de teste: sem `championshipUnderway` cai em `debut`, não `baseline`.
- [x] `pages/tabs/nextRaceEditorial.js` ✅ — AUDITADO: **100% display** (fallback ao
  leitor, não alimenta IA). Namespace `editorial`: 12 teses × 4 campos × 2 variantes =
  96 chaves `_0/_1` + ctx (fallbacks) + form (forma recente). Estrutura preservada via
  helper `V(key)`→[fn0,fn1] (o teste exige array de 2). Seed determinístico intacto.
  Testes existentes (12) + `nextRaceEditorial.i18n.test.js` verdes.
- [x] `pages/tabs/nextRaceBriefing.js` ✅ — AUDITADO: banco de 50 frases de expectativa
  (5 pos × 5 perfis × 2), **sem interpolação**; lógica de seleção opera só no `id`.
  Namespace `briefing.expectation.<id>` (50 chaves flat). `phrase(id)` vira **getter**
  (`get text(){ return i18n.t(...) }`) → não congela. Banco GERADO em ~5 linhas (ids
  seguem `bucket-profile-variant`), 266 ln de literais viraram geração. PT verbatim
  (fonte com acentos inconsistentes). Testes existentes (5) + i18n verdes. **Trio fechado.**
- [ ] `pages/tabs/newsHelpers.js`, `NewsMagazineTab.jsx` — fecha a Fase 2.

### Fase 3 — Texto determinístico (Rust) — *médio, risco médio*
> **Padrão Rust CRAVADO** (via driver_tags): chave dinâmica `rust_i18n::t!(&format!("ns.{}.{}", a, b))`
> **funciona**; campos `&'static str` de texto viram `String` (i18n é runtime); YAMLs em
> `src-tauri/locales/{pt-BR,en-US}.yml` (chaves numéricas entre aspas). ⚠️ **Display-como-chave
> de lógica** aparece — comparações tipo `tag_text == "Alien"` viram `tag.level == TagLevel::X`.
> ⚠️ **Verificar = compilar** (~2min, `CARGO_TARGET_DIR` fora do OneDrive). ⚠️ **17 testes
> pré-existentes FALHAM no HEAD** (rivalry/team, team_rivalries, migrations, calendar, weather —
> feature de rivalidade WIP; provado por stash) — ignorar nas rodadas de Fase 3.
> **✅ Locale default de teste RESOLVIDO** (no race_eval): rust-i18n tem locale GLOBAL de
> processo (não thread-local). O default do processo é `"en"` (não carregado) → cai no
> `fallback = "pt-BR"`, então prosa = PT sem setup. O ÚNICO disruptor é o `i18n_smoke`
> (troca pra en-US). **Padrão cravado:** todo teste que assevera prosa i18n → `#[serial]`
> (crate `serial_test`, dev-dep) + helper `baseline_pt()` no topo; o `i18n_smoke` também é
> `#[serial]`. Assim nunca correm juntos. **Interpolação rust-i18n:** placeholder `%{nome}`
> no YAML; no `t!` é `nome = valor` (com `=`, não `=>`); chave dinâmica precisa `let` (temp).
- [x] `models/driver_tags.rs` ✅ — 17 atributos × 5 níveis = 85 tags. `phrase` gerado por
  chave `driver_tags.<attr>.<idx>`; struct `tag_text: String`; `TAGGED_ATTRS` valida atributo.
  2 testes de display-como-chave corrigidos p/ `.level`. Compilou (1759 ok).
- [x] `race_eval.rs` ✅ — `Assessment::label()` (5 labels), `build_headline` (6 templates:
  dnf/recovery/solid/dentro/below_lost/below), `build_team_read` (4: dnf/above/limit/below).
  Bloco `race_eval:` nos 2 YAMLs. `label()` virou `String`. Testes marcados `#[serial]`+
  `baseline_pt()`; guard de interpolação (P14/P8, sem `%{`). 5 testes ok. **Padrão de teste
  serial cravado aqui** (ver bloco acima).
- [x] `commands/world_footer.rs` ✅ — ~30 templates (record_broken/watch, team_state,
  teammate, star) + tags (MERCADO→`tag_label`) traduzidos. Helpers novos: `metric_noun`
  (singular/plural via `.one`/`.other`), `metric_noun_id`, `tag_label`, `ord_label` (ordinal
  locale-aware: PT gendered `º`/`ª`, EN sufixo `st/nd/rd/th`, regra 11–13). `tone`/`kind`
  ficam como tokens (CSS/máquina). **Mata o hack `localizedAiError`** no front: NewsMagazineTab
  agora renderiza as notas direto (removido o gate `!isPortuguese`, e o state morto
  `worldNotesAi`). Guard de 2 locales (nouns/ord/interp) + 4 ai_tests + front i18n 6 verdes.
- [x] `market/pipeline.rs` ✅ — leilão de assédio: `bid_label` (abertura/lance N) +
  4 notas de `PlayerPoachOutcome` (expired/stayed/unavailable/signed). `market/preseason.rs`
  ✅ — 6 spots de `MarketEvent` (deal/window_closed/departure/contract_ended). Namespace
  `market:` nos 2 YAMLs. `event_type`/`movement_kind`/`category_label` ficam como tokens.
  Guard de 2 locales + 59 testes de mercado verdes. Sem `#[serial]` nos testes de mercado
  (não asseveram prosa); só o guard novo é `#[serial]`.
- [x] **Guard de paridade dos YAML Rust** ✅ — `i18n_parity` em lib.rs (dev-dep `serde_yaml`):
  achata pt-BR/en-US e exige mesmo conjunto de chaves + nenhum valor vazio. Protege todo
  bloco futuro (chave só num idioma quebra o teste na hora).
- [~] Notícias `titulo`/`texto` — PARCIAL:
  - [x] `db/queries/rivalry_episodes.rs` ✅ — `rivalry_label` (5 variantes). Namespace `rivalry:`.
  - [x] `commands/career_detail.rs` ✅ — DISPLAY da ficha do piloto (bem mais denso que os
    trechos que a spec apontava): personalidade (12×name+desc, namespace `career.personality`),
    marcos (`career.milestone`, com plural título one/other), níveis fama/carisma/técnico +
    stardom + entrega + veredito da temporada (namespace `driver_read`). `tom`/`tendencia`/
    status/tipo-de-marco = tokens. ⚠️ personality `tipo` é DISPLAY, desacoplado do token de
    serialização em `enums.rs` (NÃO tocar enums.rs). 2 testes de veredito viraram `#[serial]`+
    pt-BR. 15 testes verdes. **DEFER:** `injury_name_pool` (fica em `simulation/injuries.rs`,
    é sub-sistema de lesão à parte — não é career_detail).
  - [x] `commands/career.rs` ✅ COMPLETO — GRANDE (10.550 linhas), 5 clusters de DISPLAY
    todos traduzidos (namespace `team_dossier` + `career.{message,group}`). ⚠️ a spec
    apontava ~6809 mas AQUILO É TESTE. Clusters: (1) dossiê da equipe management/ownership/
    state; (2) records + first_milestone; (3) heritage + **perfil de competitividade**
    (era display-como-chave-de-lógica: `real_team_profile` re-matchado em `real_identity_summary`
    → refatorado p/ CHAVE estável + `team_profile_label` p/ display; front só renderiza,
    não compara); (4) highlights + streaks + fallbacks (rival/símbolo); (5) misc (criação/
    deleção/proposta 4 variantes/no_driver/group labels). 1 teste dossiê `#[serial]`+pt-BR.
    92 testes career + parity verdes. Sweep de prosa limpo.
  - [x] `commands/race.rs` ✅ COMPLETO — auditoria feita: SÓ o display (namespaces `race`
    + `maintenance`), os builders de fato-de-IA (`rivalry_arc_facts`, `performance_context_facts`,
    `telemetry_context_facts`) ficam pra Fase 4. Traduzido: fatura de manutenção (8 labels;
    `damage_split` refatorado p/ retornar só `(key, prop)` + `maintenance_label`), notícias
    de vitória (3 variantes) + campeão + "resumo de outras categorias", motivo de aposentadoria
    (persistido → Opção A). ⚠️ os `titulo`/`texto` em ~6809 de career.rs eram TESTE (não confundir).
    28 testes race + parity verdes. Sweep de prosa limpo.
  - [x] `simulation/injuries.rs` `injury_name_pool` ✅ — pools viraram arrays de CHAVES
    estáveis (ordem preservada p/ o hash de fallback); display via `injury_display_name`
    (namespace `injury`, 19 nomes); nome persiste no save (Opção A). 2 testes → `#[serial]`+
    pt-BR, 1 teste virou key-based (locale-indep). `fallback_injury_display_name` (career_detail)
    resolve a chave. 23 testes verdes.

### Fase 4 — Fatos da IA + servidor — *médio, dependência externa*
- [ ] Traduzir os builders de "fatos" (usar o mesmo `rust-i18n` → saem no idioma ativo):
  - `narrative/mod.rs` — `build_race_context` (393-471), `build_beats` (104-258),
    `select_race_thesis` (339-390).
  - `commands/ai_news.rs` — `select_post_race_thesis` (697-783),
    `build_post_race_facts` (790-1241), `telemetry_facts` (480-664),
    `weather_pt` (348-355), `build_recent_arc_facts` (141-198).
  - `commands/race.rs` — `context_facts`/`injury_facts` (~2944-4091).
  - Front: `pages/tabs/NextRaceTab.jsx` `aiFacts` (1774-1789).
- [ ] Servidor `iracer-news`: confirmar/ajustar prompt pra honrar `lang` e escrever
  em inglês. Ref: `docs/world-notes-endpoint.md` (persona em 47-70; deploy em 72).
  Endpoints em `src-tauri/src/narrative/client.rs` (`/race-story`, `/pre-race`,
  `/post-race`, `/world-notes`).
- [ ] Deploy do servidor.

### Fase 5 — Formatação + seeds
- [ ] Datas / números / ordenação por locale ativo (ver "Formatação" acima).
- [x] Cenários de quebra/DNF ✅ — RESOLVIDO POR RUNTIME (não por seed-freeze): os 54
  cenários do `seed_incident_catalog` viram `breakdown.<id>.{dnf,warn,part}` (148 chaves ×2
  idiomas), resolvidos no render (`simulation/catalog.rs`) por id com FALLBACK ao texto
  semeado (rust-i18n devolve a própria chave quando ausente). **Sem mudança de migração** —
  o seed PT vira só rede de segurança. `{driver}` continua substituído no render (não é
  interpolação). Toggle ao vivo (não congela). Guard de 2 locales + catalog/parity verdes.
  (YAML gerado via script pra garantir PT byte-accurate; EN traduzido à mão, 100% coberto.)
- [ ] Datas / números / ordenação por locale ativo (ver "Formatação" acima).
- [ ] Resto Fase 5: `car/parts.rs` `display_name` (121-141: Motor/Câmbio/Freios/Suspensão) +
  países com emoji (`tracks.rs pais`, `teams.rs pais_sede`) + tiers PT em `categories.rs`
  (`Amador`, `Especial`, `Super Pro`). Só saves novos.

### Fase 6 — QA em inglês
- [ ] Jogar uma carreira inteira em EN; caçar sobras.
- [ ] Métrica de cobertura: grep de caracteres acentuados nos arquivos de UI (fora
  de comentários/SQL/erros) deve tender a zero nas telas.

---

## Notas de volume (baseline da varredura)

- **Front:** ~90-100 arquivos JS/JSX com texto PT de tela. Concentração em
  `src/pages/tabs/`, `components/season/`, `/race/`, `/iracing/`, `overlay/`.
- **Back:** ~20-30 arquivos `.rs` com prosa/label de verdade (dos 168 com acento, o
  resto é comentário/SQL/erro). Concentração em `commands/`, `narrative/`, `race_eval.rs`,
  `models/driver_tags.rs`.
- **IA:** prompt de persona é **externo** (servidor Cloud Run, fora do repo); o repo
  controla os "fatos" e o campo `lang` (já plumbado em todas as chamadas).
