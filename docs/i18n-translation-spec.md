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
  - [ ] Faltam: `GlobalDriversTab.jsx`, `GlobalTeamsTab.jsx`, `Dashboard.jsx`, e os
    gigantes com prosa/IA (deixar por último, entram na Fase 2/4): `NextRaceTab.jsx` (207),
    `MyTeamTab.jsx` (152), `NewsMagazineTab.jsx` (59), `PreSeasonView.jsx` (162),
    `RaceResultView(V2).jsx`. Store `useCareerStore.js`.
- [ ] **Overlay** (texto desenhado em canvas — não-DOM, cuidado): `overlay/towerCanvas.js`
  (107), `towerRows.js`, `EngineerRadio.jsx`, `OverlayPositionPanel.jsx`,
  `overlayMockData.js`.

### Fase 2 — Prosa gerada (frontend) — *alto, risco médio*
Geradores de sentença com gramática PT (plural/ordinal/gênero). Reescrever puxando de
chaves com interpolação + plural por idioma.
- [ ] `pages/tabs/nextRaceBriefing.js` (458 ln)
- [ ] `pages/tabs/nextRaceEditorial.js` (320 ln)
- [ ] `pages/tabs/nextRaceThesis.js` (254 ln)
- [ ] `pages/tabs/inboxMessages.js` (156 ln, com `<b>`/`<p>` embutido)
- [ ] `utils/driverMentions.jsx` (106 ln)
- [ ] `pages/tabs/newsHelpers.js`, `NewsMagazineTab.jsx`, `utils/postRaceLanding.js`

### Fase 3 — Texto determinístico (Rust) — *médio, risco médio*
- [ ] `race_eval.rs` — `Assessment::label()` (73-79), `build_headline` (216-255),
  `build_team_read` (260-274).
- [ ] `models/driver_tags.rs` — banco de frases `[&str;5]` por atributo×nível (43-162).
- [ ] `commands/world_footer.rs` — ~30 templates (`record_broken_notes:233-345`,
  `team_state_note:91-126`, `star_of_category_note:493-503`) + plurais de substantivo.
- [ ] `market/pipeline.rs` (notas `note:` ~3375-3491), `market/preseason.rs`
  (`headline:` ~687,933).
- [ ] Notícias `titulo`/`texto`: `commands/race.rs` (~3030), `commands/career.rs`
  (~6809-6864, ~4868), `commands/career_detail.rs` (289-349, 1947-1963),
  `db/queries/rivalry_episodes.rs` (103-112).

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
- [ ] Cenários de quebra/DNF: **traduzir o código que semeia** (`db/migrations.rs`
  ~2560-2900, ~39 cenários) + `car/parts.rs` `display_name` (121-141: Motor/Câmbio/
  Freios/Suspensão) + países com emoji (`tracks.rs pais`, `teams.rs pais_sede`) +
  tiers PT em `categories.rs` (`Amador`, `Especial`, `Super Pro`). Só saves novos.

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
