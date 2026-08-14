# Vistoria técnica independente do Loop

> **Documento histórico, congelado no dia.** Ele descreve a árvore como ela estava em
> 11/08/2026, antes das correções, e não é atualizado quando um achado fecha. Para saber o
> estado de hoje: o que fechou está em [divida-tecnica.md](divida-tecnica.md), seção "Vistoria
> independente da série V"; o que ficou esperando decisão está em [backlog.md](backlog.md),
> seção "Vistoria da série V", com id `V-Dx`. As medições abaixo (contagens de teste, de aviso,
> de arquivo fora do `fmt`, tamanhos de diretório) são do dia e não valem como número atual.

Levantada em 11/08/2026 contra o working tree como ele está (175 arquivos modificados e não
commitados). IDs novos da série `V`, sem reuso de nenhum documento anterior. Os documentos de
vistoria, dívida, roadmap e backlog foram lidos só no fim, e só para o item 9 (divergências de
documentação): nenhum achado abaixo partiu de uma lista pré-existente.

Baseline de execução desta vistoria:

| Suíte | Resultado |
|---|---|
| `cargo test` (src-tauri) | 3.361 passaram, 0 falharam, 57 ignorados, 385 s |
| `npm run test:structure` | 146 passaram, 0 falharam |
| `npm run test:ui` (vitest) | 1.982 passaram, 1 falhou, 1 pulado, em duas execuções seguidas, com caso diferente a cada vez |
| `cargo fmt --check` | falha: 119 arquivos divergentes |
| `cargo build --lib` | compila, 201 avisos |

---

## Índice por área

- [V1 — Ponte Rust ↔ Tauri ↔ React](#v1--ponte-rust--tauri--react)
- [V2 — Banco, migrações e persistência](#v2--banco-migrações-e-persistência)
- [V3 — Simulação e balanceamento](#v3--simulação-e-balanceamento)
- [V4 — Internacionalização](#v4--internacionalização)
- [V5 — Flags de ambiente e regra de jogo](#v5--flags-de-ambiente-e-regra-de-jogo)
- [V6 — Frontend](#v6--frontend)
- [V7 — Build, CI e release](#v7--build-ci-e-release)
- [V8 — Testes](#v8--testes)
- [V9 — Higiene do repositório e código morto](#v9--higiene-do-repositório-e-código-morto)
- [V10 — Documentação](#v10--documentação)
- [Fechamento](#fechamento) (resumo executivo, top problemas, mapa, ordem de ataque)

---

## V1 — Ponte Rust ↔ Tauri ↔ React

### V1.1 — `carKeyForCategory` devolve MX-5 para toda categoria acima do `bmw_m2`

**Classificação:** bug
**Gravidade:** alta
**Status:** confirmado

**Onde**
- [nextRaceHelpers.js:33](../src/components/race/nextrace/nextRaceHelpers.js) `carKeyForCategory`
- [useIracingExport.js:80](../src/components/race/nextrace/useIracingExport.js) monta o `carKey` e o passa a `iracing_generate_roster` e `iracing_generate_season`
- [pintura.rs:127](../src-tauri/src/commands/iracing/pintura.rs) `car_key_for_category`, o gêmeo em Rust
- [roster_gen.rs:64](../src-tauri/src/iracing_sdk/roster_gen.rs) `car_spec`, que só conhece `mx5`, `gr86` e `bmwm2`

**O que acontece hoje**

A função é uma cascata de `includes`: contém "toyota" ou "gr86" vira `gr86`, contém "bmw" ou "m2"
vira `bmwm2`, e **qualquer outra coisa** cai no `else` que devolve `mx5`. O catálogo de categorias
tem dez ids ([categories.rs](../src-tauri/src/constants/categories.rs)): `mazda_rookie`,
`toyota_rookie`, `mazda_amador`, `toyota_amador`, `bmw_m2`, `production_challenger`, `gt4`, `gt3`,
`lmp2` e `endurance`. Os cinco últimos, mais o `production_challenger`, caem todos no `else`.

Resultado: um jogador na GT3 aperta "exportar" e o Loop grava um AI roster e uma AI season de
**Mazda MX-5** na instalação do iRacing dele, sem erro nenhum. O backend não recusa porque recebe
uma `car_key` válida (`mx5`); ele só recusaria uma chave desconhecida
([roster.rs:454](../src-tauri/src/commands/iracing/roster.rs), `"Carro desconhecido: {car_key} (use
mx5, gr86 ou bmwm2)"`).

**Por que isso é um problema**

O ciclo exportar-correr-importar é o caminho principal do produto. Aqui ele produz um artefato
silenciosamente errado em 6 das 10 categorias, e escreve esse artefato dentro da pasta do iRacing do
jogador. O modo de falha é o pior possível: nada estoura, o arquivo existe, e a corrida acontece com
o carro errado.

**Evidência**

Leitura das duas funções (JS e Rust), da lista de `car_spec` e do catálogo de categorias. A cascata
não tem nenhum ramo de erro; o `else` é incondicional.

**Correção recomendada**

Uma fonte de verdade só, no Rust, com mapeamento explícito categoria → carro, e `Option`/`Result` no
retorno. O JS para de calcular a chave e passa só a categoria; o comando recusa a exportação da
categoria sem carro mapeado com mensagem que o jogador entenda. Enquanto não houver carro para GT3
e acima, recusar é o comportamento correto.

**Critério de aceite**

Exportar de `gt3`, `gt4`, `lmp2`, `production_challenger` ou `endurance` devolve `Err` com mensagem
nomeando a categoria, e existe um teste que percorre `constants::categories::ALL` cobrando que toda
categoria ou tenha carro mapeado ou seja recusada explicitamente.

**Dependências**

Decisão de produto sobre o que fazer com as categorias sem carro grátis equivalente (recusar,
mapear para um carro pago, ou marcar a categoria como "só simulada").

---

### V1.2 — O export da temporada usa a duração da CATEGORIA, e a de `endurance` é a sentinela 0

**Classificação:** bug
**Gravidade:** alta
**Status:** parcialmente confirmado (o defeito é certo; a alcançabilidade pela UI depende de V1.1)

**Onde**
- [temporada.rs:322](../src-tauri/src/commands/iracing/temporada.rs) `let race_end = cat.duracao_corrida_min as i64;`
- [temporada.rs:643](../src-tauri/src/commands/iracing/temporada.rs) `race_length_min: cat.duracao_corrida_min as i64`
- [categories.rs:262](../src-tauri/src/constants/categories.rs) `endurance` declara `duracao_corrida_min: 0`
- [entry.rs:71](../src-tauri/src/calendar/entry.rs) `duracao_efetiva`, a cascata que existe justamente para matar essa sentinela

**O que acontece hoje**

O projeto tem uma cascata declarada para resolver a sentinela: `duracao_efetiva(etapa, categoria)`
usa a duração da etapa, cai na constante da categoria, e por fim no `DURACAO_SPRINT_PADRAO`. O
doc-comment dela diz, com todas as letras, que é "a única cascata que sabe resolver a sentinela".

O gerador da AI season não a usa. Ele lê `cat.duracao_corrida_min` direto, duas vezes, e o valor
vale para a temporada inteira. Duas consequências:

1. Para `endurance`, `race_length_min` e o `time_offset` final da timeline de clima saem **zero**.
2. Para qualquer categoria, a duração por etapa que o calendário gravou
   ([montagem.rs:166](../src-tauri/src/calendar/montagem.rs), `resolve_race_duration`, que sorteia
   entre 120/180/240/360 min quando a constante é 0) é descartada: o export achata tudo num número
   só, vindo da constante.

**Por que isso é um problema**

Um AI season com `race_length_min: 0` é um artefato inválido entregue ao iRacing. E o achatamento
faz o calendário exportado divergir do calendário da carreira, que é a fonte que o import vai usar
para casar o resultado.

**Evidência**

Leitura das duas linhas e do catálogo. `grep` por `.duracao_corrida_min` mostra que todos os demais
consumidores de produção (despesa, fatura, importação, persistência, economia) passam pela etapa ou
pela âncora; estes dois são a exceção.

**Correção recomendada**

Trocar as duas leituras por `duracao_efetiva` da etapa correspondente, e mover `race_length_min`
para dentro do laço de eventos (ou provar que a AI season aceita duração por evento; se não aceitar,
recusar categorias com duração variável em vez de achatar).

**Critério de aceite**

Um teste que gera a AI season de uma categoria com etapas de duração diferente e cobra que cada
evento carregue a sua duração, mais um caso que prova que nenhuma saída contém `0` como duração.

**Dependências**

V1.1 (hoje `endurance` provavelmente não chega neste caminho pela UI, e isso mascara o item).

---

### V1.3 — `create_career` está registrado na ponte e não tem nenhum consumidor de produção

**Classificação:** dívida
**Gravidade:** média
**Status:** confirmado

**Onde**
- [lib.rs:500](../src-tauri/src/lib.rs) registra `commands::career_commands::create_career`
- [NewCareer.test.jsx:135](../src/pages/NewCareer.test.jsx) é a **única** ocorrência da string `"create_career"` em todo o `src/`, e é uma asserção **negativa**: `expect(mockInvoke).not.toHaveBeenCalledWith("create_career", ...)`

**O que acontece hoje**

O fluxo real de criação de carreira passa por `create_historical_career_draft` →
`update_career_draft_identity` → `finalize_career_draft`. O `create_career` sobrou registrado. Pior:
o guard [invoke-contra-generate-handler.test.mjs:172](../scripts/tests/invoke-contra-generate-handler.test.mjs)
tem um segundo laço que marca como "usado" **qualquer literal de string com 6+ caracteres que case
com um nome de comando registrado**. A asserção negativa do teste é exatamente esse literal. O guard
conta o comando como consumido e ele nunca entra em `SEM_CONSUMIDOR_CONHECIDO`.

**Por que isso é um problema**

O guard existe para congelar o inventário de comandos órfãos, e este é o caso em que ele mente com
mais precisão: o único vestígio do comando é um teste que afirma que ninguém o chama.

**Evidência**

Sonda cruzando o `generate_handler!` com todos os `invoke(...)` e todos os literais de string do
`src/`. `grep -rn "create_career"` em `src/` devolve uma linha só, a asserção negativa.

**Correção recomendada**

Decidir entre remover o comando (junto com a casca `#[tauri::command]`) ou registrá-lo em
`SEM_CONSUMIDOR_CONHECIDO` com o motivo. E apertar o guard: o segundo laço deveria ignorar arquivos
`*.test.*` e exigir que o literal esteja num contexto de invocação.

**Critério de aceite**

`create_career` some do `generate_handler!` ou aparece na lista congelada, e o guard passa a
detectar um comando cujo único vestígio é um teste.

---

### V1.4 — Seis comandos cujo único consumidor vive num componente órfão

**Classificação:** dívida
**Gravidade:** média
**Status:** confirmado

**Onde**

[RosterGenPanel.jsx](../src/components/iracing/RosterGenPanel.jsx), 726 linhas, **zero
importadores** em todo o `src/` fora do próprio teste. Os comandos que só ele chama:

```
iracing_apply_player_paint
iracing_dump_session_yaml
iracing_export_rain_test
iracing_player_custid
iracing_player_paint
iracing_preview_race_result
```

**O que acontece hoje**

O guard de inventário lê os `invoke(...)` do `src/` inteiro, sem distinguir módulo vivo de módulo
órfão. Os seis contam como consumidos. A lista congelada `SEM_CONSUMIDOR_CONHECIDO` tem 20 nomes; o
número real de comandos sem consumidor **vivo** é 26.

**Por que isso é um problema**

O inventário é a contagem oficial declarada no [CLAUDE.md](../CLAUDE.md) e no
[iracing-escopo.md](iracing-escopo.md). Ele subestima em 6 e não tem como perceber sozinho.

**Evidência**

Sonda de órfãos (varredura de importadores sobre os 332 módulos de `src/`) cruzada com a extração de
`invoke`. `grep -rn "RosterGenPanel" src/` só encontra o arquivo e o teste dele.

**Correção recomendada**

O guard passa a ignorar módulos sem importador vivo ao computar consumidores. Separadamente, decidir
o destino do painel (ligar ou apagar).

**Critério de aceite**

O guard, rodando contra a árvore atual, lista os seis comandos como órfãos, e alguém decide caso a
caso.

**Dependências**

V6.1 (o destino do painel é decisão de produto).

---

### V1.5 — Duas pontes paralelas de construção do resultado da corrida, uma delas viva só nos testes

**Classificação:** dívida
**Gravidade:** média
**Status:** confirmado

**Onde**
- [result_bridge/oficial.rs:19](../src-tauri/src/iracing_sdk/result_bridge/oficial.rs) `build_race_result_from_aiseason`, 447 linhas, chamada em produção por [resultado.rs:370](../src-tauri/src/commands/iracing/resultado.rs)
- [result_bridge/sessao.rs:23](../src-tauri/src/iracing_sdk/result_bridge/sessao.rs) `build_race_result_from_session`, 269 linhas, chamada **só** por `result_bridge/tests/mod.rs`

**O que acontece hoje**

Existem dois caminhos completos de construção do `RaceResult` a partir do iRacing: um a partir do
JSON oficial do aiseason, outro a partir da sessão ao vivo. Só o primeiro tem chamador de produção.
O segundo tem testes próprios, o que o faz parecer vivo em qualquer leitura superficial e o mantém
verde para sempre.

**Por que isso é um problema**

Quem for mexer no import precisa decidir, a cada mudança, se replica no caminho de sessão. Ele
carrega custo de manutenção sem entregar comportamento.

**Evidência**

`grep -rn "build_race_result_from_session"` devolve 4 ocorrências: a definição e três dentro de
`tests/`. O aviso `function build_race_result_from_session is never used` aparece no build da lib.

**Correção recomendada**

Ou ligar (se existe cenário em que o aiseason não sai e a sessão é a única fonte), ou remover o
módulo e os testes juntos.

**Critério de aceite**

Um dos dois: existe chamador de produção, ou o arquivo não existe mais.

**Dependências**

Decisão de produto sobre o cenário "resultado sem aiseason".

---

## V2 — Banco, migrações e persistência

### V2.1 — `run_pending` não barra um save de schema mais NOVO que o binário

**Classificação:** bug
**Gravidade:** média
**Status:** confirmado

**Onde**

[migrations.rs:71](../src-tauri/src/db/migrations.rs) `run_pending`

**O que acontece hoje**

A função tem uma guarda para o passado (`version < BASELINE_VERSION` recusa e explica), e nenhuma
para o futuro. Um save carimbado em v65 aberto por um binário com `CURRENT_VERSION = 64` passa pelo
laço sem executar nada e devolve `Ok(())`. O jogo abre e opera sobre um schema que não conhece.

**Por que isso é um problema**

O canal beta existe ([release.mjs](../scripts/release.mjs), `--channel beta`). Um jogador que testa
o beta, joga, e depois volta ao estável tem exatamente esse save. A falha vai aparecer como erro de
coluna inexistente no meio de uma consulta qualquer, longe da causa, ou pior, como leitura
silenciosamente errada de uma coluna que mudou de sentido.

**Evidência**

Leitura do laço: `for (target, migrate) in MIGRATIONS { if version < *target {...} }`. Com
`version = 65` nenhum ramo executa e não há verificação posterior.

**Correção recomendada**

Recusar `version > CURRENT_VERSION` com a mesma clareza da guarda de baixo, dizendo que o save veio
de uma versão mais nova e apontando o caminho (atualizar o app ou usar um backup).

**Critério de aceite**

Teste que carimba `user_version` acima de `CURRENT_VERSION`, chama `run_pending` e espera `Err`, com
o arquivo intocado.

---

### V2.2 — `preseason_plan.json` é gravado dentro da transação e a compensação só cobre a falha do commit

**Classificação:** bug
**Gravidade:** média
**Status:** confirmado

**Onde**
- [orquestracao.rs:188](../src-tauri/src/evolution/pipeline/orquestracao.rs) `initialize_preseason_phase(&tx, &new_season, save_path, ...)`
- [plano.rs:15](../src-tauri/src/market/preseason/plano.rs) `save_preseason_plan` faz `std::fs::write`
- [orquestracao.rs:209](../src-tauri/src/evolution/pipeline/orquestracao.rs) o `remove_file` de compensação, dentro do `map_err` do `tx.commit()`

**O que acontece hoje**

A virada de temporada roda inteira dentro de uma transação, e isso está certo. No meio dela,
`initialize_preseason_phase` grava um arquivo em disco. Depois dele ainda rodam dois passos que
podem falhar com `?`: `process_injury_recovery_without_seat` e `apply_offseason_motivation`. Se um
desses falhar, a transação sofre rollback (o `Drop` do `Transaction`) e o `preseason_plan.json`
**fica em disco**, porque a única limpeza está pendurada no erro do `commit`, que nunca chegou a ser
chamado.

**Por que isso é um problema**

O save volta para a temporada anterior com um plano de pré-temporada da temporada seguinte no disco
ao lado. `load_preseason_plan` não valida a qual temporada o plano pertence.

**Evidência**

Leitura do fluxo entre as linhas 188 e 211 de `orquestracao.rs`, mais `plano.rs` inteiro (a gravação
é `fs::write` direto, sem participação da transação).

**Correção recomendada**

Ou adiar a escrita do arquivo para depois do commit, ou embrulhar o trecho num guarda que apague o
arquivo em qualquer saída de erro (não só na do commit).

**Critério de aceite**

Teste que força a falha do último passo e cobra que `preseason_plan.json` não exista depois.

---

### V2.3 — `restore_backup` troca o banco sem atomicidade e sem checar compatibilidade

**Classificação:** dívida
**Gravidade:** média
**Status:** confirmado

**Onde**

[restore.rs:6](../src-tauri/src/commands/save/restore.rs) `restore_backup_internal`

**O que acontece hoje**

A sequência é: abrir o banco atual, fazer checkpoint, fechar, copiar `career.db` para
`career.db.bak`, apagar `-wal`/`-shm`, e então `std::fs::copy(backup, db_path)`. Se a última cópia
morrer no meio (disco cheio, antivírus, arquivo travado), `career.db` fica truncado e a única cópia
íntegra é o `.bak`, que a próxima restauração sobrescreve sem avisar.

O módulo irmão [comum.rs:50](../src-tauri/src/commands/save/comum.rs) tem exatamente o utilitário
para isso: `substituir_preservando_anterior`, com staging, `.old` e rollback. O caminho de backup o
usa; o de restauração não.

Além disso, nenhuma checagem de que o backup é migrável. Um backup de schema anterior à baseline é
copiado com sucesso e só falha no próximo `load_career`.

**Por que isso é um problema**

Restauração é o caminho que o jogador usa **porque** alguma coisa deu errado. É o pior lugar para
uma operação não atômica.

**Evidência**

Leitura das duas funções lado a lado. `substituir_preservando_anterior` é `pub(crate)` e está a um
`use` de distância.

**Correção recomendada**

Restaurar via staging + `substituir_preservando_anterior`. Antes de trocar, abrir o backup numa
conexão temporária e verificar a versão de schema contra `BASELINE_VERSION`/`CURRENT_VERSION`.

**Critério de aceite**

Teste com falha injetada na cópia final que prova que `career.db` continua íntegro, e teste que
recusa um backup de versão incompatível antes de tocar no banco vivo.

---

### V2.4 — A leitura do fim de semana (migrações v55/v57) nunca é escrita em produção

**Classificação:** dívida
**Gravidade:** média
**Status:** confirmado

**Onde**
- [races.rs:192](../src-tauri/src/db/queries/races.rs) `set_race_weekend_readings`: os únicos chamadores estão em `#[cfg(test)]`
- [races.rs:217](../src-tauri/src/db/queries/races.rs) `get_race_weekend_reading`: lido em produção por [fatos.rs:841](../src-tauri/src/commands/ai_news/fatos.rs)
- [lib.rs:548](../src-tauri/src/lib.rs) registra `get_race_reading`, que a lista congelada do guard já classifica como sem consumidor **dos dois lados**

**O que acontece hoje**

Duas migrações criaram a estrutura e semearam linhas. Nenhum caminho de produção grava. O getter
devolve `None` para toda corrida nova, para sempre, e o comando exposto na ponte não é chamado por
ninguém.

**Por que isso é um problema**

É uma feature com schema, migração, DTO, comando registrado e testes, e nenhum comportamento. Ela
consome espaço mental e superfície de ponte sem entregar nada, e o `None` silencioso faz o consumidor
degradar sem sinal.

**Evidência**

`grep -rn "set_race_weekend_readings"` devolve a definição, o doc-comment do getter e quatro linhas
dentro de `tests`. Nenhuma outra.

**Correção recomendada**

Decidir quem grava (a Sala de Estratégia, no preparo do fim de semana, é o candidato natural) ou
remover o trio getter/setter/comando e agendar o `DROP` numa migração futura.

**Dependências**

Decisão de produto.

---

## V3 — Simulação e balanceamento

### V3.1 — A corrida é decidida na classificação: a pole vence 76% a 85% das provas

**Classificação:** calibração
**Gravidade:** alta
**Status:** confirmado, com medição

**Onde**

[simulation/calibracao/](../src-tauri/src/simulation/calibracao/), harness `arena::medir`, alvos em
[alvos.rs](../src-tauri/src/simulation/calibracao/alvos.rs).

**O que acontece hoje**

Rodei o gerador de baseline do próprio projeto (`imprime_baseline`, 84 temporadas por cenário,
1.008 corridas cada, pelo caminho de produção `run_full_race_with_breakdowns`):

| Cenário | Spearman grid × chegada | Alvo | Vitórias do pole | Alvo |
|---|---|---|---|---|
| mazda_rookie sem incidentes | **0,92** | 0,40 a 0,75 | **78,9%** | 15% a 35% |
| mazda_rookie com incidentes | **0,91** | 0,40 a 0,75 | **76,4%** | 15% a 35% |
| gt3 sem incidentes | **0,96** | 0,60 a 0,88 | **85,4%** | 30% a 55% |
| gt3 com incidentes | **0,95** | 0,60 a 0,88 | **85,1%** | 30% a 55% |

No rookie, mais quatro métricas ficam fora: vencedores distintos 4,48 contra 5 a 10, trocas de
liderança 1,75 contra 2 a 7, margem do campeão 17,0% contra 2% a 15%, e P(melhor do grid fora do top
5) 14,6% contra 15% a 35%. O contexto impresso pelo próprio harness fecha o retrato: título decidido
a 90% a 93% da temporada e 21% a 33% das temporadas **sem nenhuma troca de liderança**.

**Por que isso é um problema**

A classificação está decidindo a corrida. Isso atravessa quase tudo: o campeonato fica sem disputa,
a narrativa não tem o que contar, a avaliação de carreira vira função da pole, e o mercado premia o
mesmo eixo duas vezes. É o número mais distante do alvo em toda a árvore, e é o próprio projeto que
declara o alvo.

**Evidência**

`cargo test --lib imprime_baseline -- --ignored --nocapture` e
`cargo test --lib rookie_distribui_como_corrida_de_verdade -- --ignored --nocapture` (falha com a
tabela acima). Nenhuma sonda minha: é o instrumento do repositório, no caminho de produção.

**Correção recomendada**

Investigar a cadeia qualificação → grid → primeiro trecho. A leitura da tabela sugere que o
diferencial determinístico do primeiro segmento (largada + ar sujo + custo de ultrapassagem)
está alto demais em relação ao ruído por trecho. O caminho é varredura de knob com o harness que já
existe (`varredura_do_trafego_mede_alavanca`, `imprime_varredura_de_knobs`), não escolha à mão.

**Critério de aceite**

`rookie_distribui_como_corrida_de_verdade` e `gt3_distribui_como_corrida_de_verdade` passam sem
`#[ignore]`, e o baseline congelado é reemitido.

**Dependências**

Decisão de produto sobre o alvo em si: os intervalos de `Alvos::entrada()` e `Alvos::topo()` também
são escolha humana, e ninguém verificou se eles descrevem o jogo que se quer.

---

### V3.2 — Os critérios de aceite da distribuição rodam com os incidentes DESLIGADOS

**Classificação:** dívida
**Gravidade:** média
**Status:** confirmado

**Onde**
- [arena.rs:176 e 191](../src-tauri/src/simulation/calibracao/arena.rs) `ConfigTemporada::rookie()` e `::gt3()` nascem com `incidentes: false`
- [tests/mod.rs:1688](../src-tauri/src/simulation/calibracao/tests/mod.rs) `campanha_pesada` só troca `pilotos` e `etapas`
- [tests/mod.rs:1756, 1772, 1790](../src-tauri/src/simulation/calibracao/tests/mod.rs) os três testes de aceite, nenhum chama `.com_incidentes(true)`

**O que acontece hoje**

Os três testes chamados "distribui como corrida de verdade" e "rookie é mais caótica que o topo"
rodam com `IncidentCatalog::empty()`. A linha de contexto que eles imprimem diz "0.00 abandonos por
etapa", e ela é literal: nenhuma corrida tem abandono.

**Por que isso é um problema**

Um critério de aceite chamado "como corrida de verdade" que desliga os abandonos mede outra coisa. O
efeito de correr com incidentes é pequeno (medi: o Spearman cai de 0,92 para 0,91), mas o valor de
um teste está em ninguém precisar refazer essa conta para saber que ele vale.

**Evidência**

Comparação das duas colunas do baseline (com e sem incidentes) mais a leitura dos construtores.

**Correção recomendada**

Os testes de aceite passam a rodar com incidentes ligados. O cenário sem incidentes continua
existindo como isolamento, com nome que diga isso.

**Critério de aceite**

Os três testes chamam `.com_incidentes(true)` e a taxa de abandono impressa é maior que zero.

---

### V3.3 — Três critérios de aceite ficam ignorados porque falham

**Classificação:** dívida
**Gravidade:** média
**Status:** confirmado

**Onde**

[tests/mod.rs:1755, 1771, 1789](../src-tauri/src/simulation/calibracao/tests/mod.rs), todos com
`#[ignore = "pesado (~N corridas) e FALHA hoje — é o critério de aceitação do conserto"]`

**O que acontece hoje**

A suíte verde de 3.361 testes não inclui os três testes que asseveram a propriedade central da
simulação. O motivo está escrito e é honesto, e ainda assim o efeito é que `cargo test` diz "ok"
sobre um motor que não atende ao próprio alvo.

**Por que isso é um problema**

O sinal verde é a coisa mais fácil de acreditar num projeto grande. Enquanto V3.1 estiver aberto,
alguém vai olhar a suíte e concluir que a simulação está calibrada.

**Correção recomendada**

Enquanto V3.1 não fechar, um teste rápido e não ignorado que trave o número **atual** como teto
(por exemplo: "vitórias do pole não pode piorar além de 86%"), para pelo menos impedir regressão.

**Critério de aceite**

Existe teste não ignorado que quebra se o determinismo aumentar.

---

## V4 — Internacionalização

### V4.1 — O catálogo de incidentes é texto de apresentação em português dentro do schema

**Classificação:** bug
**Gravidade:** média
**Status:** RESOLVIDO em 11/08/2026 pela migração v65

**Como ficou**

`incident_catalog` guarda `dnf_key`, `non_dnf_key` e `description_key`, todas na forma
`breakdown.<id>.{dnf|warn|part}`, derivadas de `(id, severity_context)` pela mesma função que o seed
usa ([seed_incidentes.rs](../src-tauri/src/db/migrations/seed_incidentes.rs), `chaves_de_texto`). A
migração renomeia as três colunas em vez de reconstruir a tabela, então índices e `dnf_catalog_id`
ficam intocados. Id fora dos 54 tem a prosa preservada em `incident_catalog_texto_legado`, reportada
no `loop.log` e ainda exibida como estava — preservar, sem inventar tradução. Peso, filtro e
incidência dos 54 eventos não mudaram.

O critério de aceite está coberto por `troca_de_locale_muda_o_texto_do_mesmo_save`
([catalog.rs](../src-tauri/src/simulation/catalog.rs)), e o caminho de save antigo por
`save_antigo_troca_prosa_por_chave`
([incident_catalog_chaves.rs](../src-tauri/src/db/migrations/incident_catalog_chaves.rs)).

O registro do problema original fica abaixo.

**Onde**
- [baseline.rs:354](../src-tauri/src/db/migrations/baseline.rs) tabela `incident_catalog` com `dnf_template`, `non_dnf_template` e `description_short`
- [seed_incidentes.rs](../src-tauri/src/db/migrations/seed_incidentes.rs) semeia 54 entradas, todas com prosa em PT

**O que acontece hoje**

Frases como `"{driver} abandona com problema no câmbio – sincronizador da 3ª marcha falhou"` entram
no banco pela migração. Elas viram a descrição do incidente na tela e o corpo da notícia
([manchetes.rs:183](../src-tauri/src/commands/race/noticias/manchetes.rs), `texto =
inc.description.clone()`). Trocar o idioma para en-US não muda nada: o texto está no save.

**Por que isso é um problema**

O projeto tem `rust-i18n` no backend, `localeParity` cobrando as chaves e um guard de pre-commit no
frontend, e 54 frases de UI atravessam tudo isso por dentro do schema. Não há caminho de tradução
sem mudar a tabela.

**Evidência**

Leitura do DDL e do seed. Nenhuma chamada a `t!()` no módulo de seed.

**Correção recomendada**

Guardar chave em vez de frase: `dnf_template_key` etc., com a prosa nos `locales/*.yml`. Migração de
conversão para os saves existentes.

**Critério de aceite**

Um teste que troca o locale para en-US e prova que a descrição de um incidente muda.

**Dependências**

Formato de save (migração).

---

### V4.2 — As notícias congelam o idioma no momento da gravação, e três títulos escapam do i18n

**Classificação:** bug
**Gravidade:** média
**Status:** confirmado

**Onde**
- [baseline.rs:789](../src-tauri/src/db/migrations/baseline.rs) tabela `news` com `titulo TEXT` e `texto TEXT`
- [manchetes.rs:178-183 e 214-217](../src-tauri/src/commands/race/noticias/manchetes.rs) três literais PT crus

**O que acontece hoje**

A maior parte do módulo de notícias usa `t!()` (45 chamadas em 7 arquivos), e o resultado é
renderizado e **gravado** no banco. Notícia antiga fica no idioma em que nasceu. E três strings nem
passam por `t!()`:

```rust
format!("{} abandona a corrida após incidente", driver_name)
format!("{} envolvido em incidente durante a prova", driver_name)
let titulo = "desfalque confirmado".to_string();
```

O terceiro também nasce em minúscula, contra a convenção de copy do projeto.

**Por que isso é um problema**

O jogador que troca de idioma no meio de uma carreira fica com um arquivo de notícias bilíngue, e
todas as notícias novas de incidente e lesão continuam em PT em qualquer idioma.

**Evidência**

`grep -c "t!("` por arquivo do módulo mais leitura das linhas citadas.

**Correção recomendada**

Curto prazo: as três strings viram chave. Médio prazo: decidir se `news` guarda chave + parâmetros
em vez de prosa renderizada, que é a única forma de a notícia antiga acompanhar a troca de idioma.

**Critério de aceite**

O auditor de i18n, estendido ao Rust (ou um teste de locale), não encontra literal PT no módulo de
notícias, e existe decisão registrada sobre o texto já persistido.

---

### V4.3 — O auditor de i18n varre só `.jsx`, e há texto de UI em `.js`

**Classificação:** dívida
**Gravidade:** média
**Status:** confirmado

**Onde**
- [i18nAudit.mjs:170](../scripts/i18nAudit.mjs) `listJsxFiles` filtra `p.endsWith(".jsx")`
- [.githooks/pre-commit](../.githooks/pre-commit) roda o mesmo auditor com `--staged`
- Exemplos vivos: [microfone.js:312-319](../src/lib/microfone.js), [pttEngenheiro.js:194-256](../src/lib/pttEngenheiro.js), [useIracingExport.js:79](../src/components/race/nextrace/useIracingExport.js)

**O que acontece hoje**

A limitação está declarada no cabeçalho do auditor, e por isso não é surpresa. O tamanho dela é que
não estava medido. Varri os 129 arquivos `.js` de `src/` com a mesma heurística de PT: 336
candidatos. Descontando comentários e chaves, sobram frases que o jogador lê, entre elas
`"Permissão de microfone negada. O Windows pode estar bloqueando o app em Privacidade → Microfone."`
e `"Transcrição vazia — nada foi entendido."`, mais o nome do roster exportado ao iRacing,
`` `Carreira ${player?.nome}` ``.

**Por que isso é um problema**

O hook de pre-commit dá a sensação de cobertura total. Mover uma string de um `.jsx` para um helper
`.js` a tira do radar sem nenhum aviso.

**Evidência**

Sonda no scratchpad com a mesma regex `PT` do auditor aplicada a literais de `.js`, mais leitura dos
arquivos citados.

**Correção recomendada**

Estender o auditor a `.js` com padrões próprios (retorno de função, literal atribuído a variável de
mensagem), começando com uma allowlist do passivo atual para não travar o commit de imediato.

**Critério de aceite**

O auditor roda sobre `.js`, o passivo atual está numa lista explícita, e uma string PT nova num `.js`
bloqueia o commit.

---

### V4.4 — `LOADING_MESSAGES`: fonte de verdade duplicada, e o teste assevera o texto que ninguém lê

**Classificação:** dívida
**Gravidade:** baixa
**Status:** confirmado

**Onde**
- [constants.js:88](../src/utils/constants.js) array de 75 frases em PT
- [NewCareer.jsx:695](../src/pages/NewCareer.jsx) renderiza `t("newCareer.loadingMessages.msg" + i)`, e usa o array **só pelo `.length`**
- [constants.test.js:21](../src/utils/constants.test.js) o caso "follows broad historical draft creation phases"

**O que acontece hoje**

O texto que o jogador vê está nos locales (75 chaves em pt-BR e 75 em en-US, conferido). O array é
usado apenas para calcular o módulo do índice. O teste `findIndex(/base.*2000/i)` e companhia
percorre o **array**, e o array não é o que a tela mostra. Reordenar ou reescrever as chaves de
locale não quebra nada.

**Por que isso é um problema**

Duas fontes de verdade para a mesma coisa, sem nada garantindo que o tamanho delas continue igual.
Se o locale ficar com menos chaves que o array, a tela mostra a chave crua
(`newCareer.loadingMessages.msg74`) em vez da frase.

**Evidência**

Contagem: 75 no array, 75 em cada locale. Leitura do render e do teste.

**Correção recomendada**

Substituir o array por uma constante de contagem derivada do próprio locale, e mover as asserções de
ordem narrativa para as chaves de pt-BR.

**Critério de aceite**

Existe teste que quebra se as chaves de locale e a contagem usada pelo carrossel divergirem.

---

## V5 — Flags de ambiente e regra de jogo

### V5.1 — Três flags de regra de jogo escapam do inventário, e o guard passa verde

**Classificação:** bug
**Gravidade:** alta
**Status:** confirmado

**Onde**
- [flags_experimentais.rs:52](../src-tauri/src/constants/flags_experimentais.rs) `INVENTARIO`, com 7 flags
- [flags_experimentais.rs:186](../src-tauri/src/constants/flags_experimentais.rs) `FONTES_VIGIADAS`, com **3 arquivos**: `market/pipeline/consolidacao.rs`, `promotion/pipeline.rs`, `promotion/effects.rs`
- Fora do inventário e fora da vigilância:
  - [cashflow.rs:130](../src-tauri/src/finance/cashflow.rs) `IRACER_GATE_SHARE` sobrescreve o coeficiente do bolo de bilheteria
  - [salary.rs:48](../src-tauri/src/finance/salary.rs) `IRACER_SALARY_SHARE` sobrescreve a fração da folha salarial
  - [importacao.rs:97](../src-tauri/src/commands/iracing/importacao.rs) `IRACER_TRACK_RIVALRY` liga ou desliga a aplicação das rivalidades percebidas na importação

**O que acontece hoje**

O módulo diz, no doc-comment, que é "o lugar ÚNICO onde estão declaradas" as flags de regra de jogo,
e o teste `nenhuma_flag_de_regra_escapa_do_inventario` existe para cobrar isso. Só que ele só olha
três arquivos, escolhidos à mão e embutidos por `include_str!`. As duas flags de economia e a de
rivalidade estão em arquivos que ninguém vigia. As três usam o prefixo `IRACER_` e as duas de
economia mudam número que entra no dinheiro das equipes.

Somando: uma máquina com `IRACER_GATE_SHARE=0.4` simula uma economia diferente da de outra máquina,
com o mesmo save, e nada na tela conta isso. É exatamente o risco que o módulo foi escrito para
eliminar.

**Por que isso é um problema**

O guard é a garantia declarada, e ele passa verde com três violações vivas. É o pior tipo de guard:
o que produz confiança sem cobertura.

**Evidência**

`grep -rn "env::var" src-tauri/src` cruzado com `INVENTARIO` e com `FONTES_VIGIADAS`. As três flags
citadas não aparecem no inventário e seus arquivos não estão na lista vigiada.

**Correção recomendada**

Trocar a lista `include_str!` por uma varredura de toda a árvore `src-tauri/src` (o teste já lê
texto; ler o diretório inteiro é a mesma técnica), com allowlist explícita para as envs que não são
regra de jogo (`USERPROFILE`, `LOOP_BASE_DIR`, `LOOP_BENCH_*`, `IRACER_MC_*`). Declarar as três
flags no inventário com dono, efeito e destino.

**Critério de aceite**

O teste, rodando contra a árvore atual, falha nas três, e passa depois de elas serem declaradas.
Adicionar uma env `IRACER_` nova em qualquer arquivo do crate quebra o teste.

---

## V6 — Frontend

### V6.1 — Sete módulos órfãos, cerca de 2.800 linhas, três deles vivos só pelos próprios testes

**Classificação:** dívida
**Gravidade:** média
**Status:** confirmado

**Onde**

| Linhas | Situação | Arquivo |
|---:|---|---|
| 727 | só teste | [RosterGenPanel.jsx](../src/components/iracing/RosterGenPanel.jsx) |
| 697 | só teste | [PostRacePanel.jsx](../src/components/iracing/PostRacePanel.jsx) |
| 558 | só teste | [driverDetailV2TestKit.jsx](../src/components/driver/v2/driverDetailV2TestKit.jsx) |
| 357 | **ninguém** | [RaceCharts.jsx](../src/components/race/RaceCharts.jsx) |
| 264 | só teste | [RaceCoursePanel.jsx](../src/components/race/RaceCoursePanel.jsx) |
| 116 | **ninguém** | [GaragePanelV2.jsx](../src/components/team/myteam/v2/GaragePanelV2.jsx) |
| 59 | **ninguém** | [DebugDataSwitch.jsx](../src/components/team/myteam/v2/DebugDataSwitch.jsx) |

`driverDetailV2TestKit.jsx` é kit de teste e está no lugar certo. Os outros seis não.

**O que acontece hoje**

`RaceCharts.jsx` não é citado em lugar nenhum, nem por teste. Os três "só teste" contribuem para a
suíte verde e para a métrica de cobertura sem entregar tela.

**Por que isso é um problema**

Além do custo de manutenção, eles distorcem dois guards: o de comandos sem consumidor (V1.4) e a
leitura de "o que está coberto por teste".

**Evidência**

Sonda de importadores sobre os 332 módulos fonte de `src/`, confirmada por `grep` nome a nome.

**Correção recomendada**

Decidir painel a painel: ligar ou apagar (com o teste junto). `RaceCharts.jsx` pode ir direto.

**Dependências**

Decisão de produto sobre `RosterGenPanel` e `PostRacePanel` (são a bancada de diagnóstico do
iRacing; ligar significa dar-lhes lugar na UI).

---

### V6.2 — 77 `catch(() => {})` no frontend, sem separação entre best-effort e falha real

**Classificação:** dívida
**Gravidade:** média
**Status:** confirmado

**Onde**

77 ocorrências em `src/`. Os casos que mais importam estão no caminho principal:
[useIracingExport.js:90 e 95](../src/components/race/nextrace/useIracingExport.js), que engolem a
falha de `iracing_install_yellow_macro` e `iracing_modo_janela_aplicar`.

**O que acontece hoje**

A maioria é best-effort legítimo e está comentada como tal. O problema é que o padrão é o mesmo nos
dois casos: quem lê não distingue "isto pode falhar e tudo bem" de "isto falhou e ninguém soube". No
caso da macro de bandeira e do modo janela, os dois são pré-requisitos de features que o jogador vai
usar depois (bandeira amarela automática e overlay), e a falha só aparece como "não funciona".

**Por que isso é um problema**

O modo de falha do produto passa a ser "a feature não acontece e não há rastro". O backend já tem um
canal para isso: `diagnostico::linha`, que a própria UI lê por `iracing_log_ler`.

**Correção recomendada**

Um helper único, `bestEffort(promise, rotulo)`, que engole para a UI e registra no log de
diagnóstico. Substituir os `catch(() => {})` do caminho de corrida por ele.

**Critério de aceite**

Guard estrutural que proíba `catch(() => {})` cru em `src/components/race/` e `src/stores/`.

---

### V6.3 — `dangerouslySetInnerHTML` com `escapeValue: false` e CSP nula

**Classificação:** dívida
**Gravidade:** baixa
**Status:** confirmado (risco latente, não explorável hoje)

**Onde**
- [MagazineMailbox.jsx:97](../src/components/news/MagazineMailbox.jsx) `dangerouslySetInnerHTML={{ __html: selected.body }}`
- [inboxMessages.js:18](../src/pages/tabs/inboxMessages.js) `const bold = (s) => \`<b>${s}</b>\``, com nomes vindos do banco
- [i18n/index.js:39](../src/i18n/index.js) `escapeValue: false`
- [tauri.conf.json](../src-tauri/tauri.conf.json) `"csp": null`

**O que acontece hoje**

O corpo da caixa de entrada é montado com HTML e injetado sem sanitização. Os nomes que entram
(`rival_name`, `driver_name`, `team_name`) vêm dos geradores do próprio jogo, e o favorito ao título
exclui o jogador explicitamente ([inbox.rs:142](../src-tauri/src/commands/inbox.rs)). Ou seja: hoje
nenhum texto controlado pelo jogador chega ali.

**Por que isso é um problema**

É um sink sem proteção esperando a primeira fonte controlável (nome de equipe customizado, apelido
do piloto, texto vindo do proxy de IA). Com `csp: null` num app Tauri, script injetado no webview
principal alcança a ponte de comandos.

**Correção recomendada**

Sanitizar no ponto de injeção, ou trocar `<b>` por `<Trans>`/componentes React e eliminar o
`dangerouslySetInnerHTML`. Independentemente disso, definir uma CSP.

**Critério de aceite**

Nenhum `dangerouslySetInnerHTML` recebendo string montada com dado do banco, e `csp` preenchida no
`tauri.conf.json`.

---

## V7 — Build, CI e release

### V7.1 — O comando de teste documentado no CLAUDE.md ignora o `.cargo/config.toml` e produziu 36 GB dentro do repositório

**Classificação:** bug
**Gravidade:** média
**Status:** confirmado, com medição

**Onde**
- [CLAUDE.md](../CLAUDE.md) documenta `cargo test --manifest-path src-tauri/Cargo.toml`
- [src-tauri/.cargo/config.toml](../src-tauri/.cargo/config.toml) aponta `target-dir = "C:/cargo-target/iracer"`
- [release.mjs:173](../scripts/release.mjs) explica esse mesmo problema em comentário e o resolve com `CARGO_TARGET_DIR` explícito
- [ci.yml](../.github/workflows/ci.yml) também sobrescreve `CARGO_TARGET_DIR`

**O que acontece hoje**

O cargo lê `.cargo/config.toml` a partir do diretório de trabalho, subindo. Rodando da raiz com
`--manifest-path`, o arquivo de `src-tauri/` não é lido, e o target volta para `src-tauri/target`.
Medi: **36 GB** existem hoje em `src-tauri/target`, em paralelo ao `C:/cargo-target/iracer` que a
config manda usar.

Os dois lugares que sabem do problema (o script de release e o CI) o contornam. O documento que
orienta o dia a dia não.

**Por que isso é um problema**

Além do disco duplicado, é o gatilho de LNK1104 entre sessões: dois processos cargo com target-dir
diferente competem pelo mesmo `.exe` de teste. Reproduzi isso durante esta vistoria.

**Correção recomendada**

Corrigir o CLAUDE.md para `cd src-tauri && cargo test`, ou documentar o `CARGO_TARGET_DIR` explícito
como o script de release faz. E resolver o 36 GB existente.

**Critério de aceite**

O comando documentado, executado da raiz, escreve em `C:/cargo-target/iracer`.

---

### V7.2 — `cargo fmt --check` falha em 119 arquivos e o CI não cobra

**Classificação:** dívida
**Gravidade:** baixa
**Status:** confirmado, com medição

**Onde**

[CLAUDE.md](../CLAUDE.md) afirma "O Rust segue `cargo fmt`/clippy". [ci.yml](../.github/workflows/ci.yml)
roda `npm run test:all`, `npm run build`, o build da layer C++ e `cargo test`. Nada de `fmt`, nada de
`clippy`.

**O que acontece hoje**

`cargo fmt --check` devolve 354 blocos de diff em 119 arquivos. `cargo build --lib` acumula 201
avisos, incluindo 20 imports não usados.

**Correção recomendada**

Decidir se o padrão vale. Se valer, rodar `cargo fmt` de uma vez e adicionar `cargo fmt --check` e
`cargo clippy -- -D warnings` (ou pelo menos `-D unused_imports`) ao CI. Se não valer, tirar a
afirmação do CLAUDE.md.

**Critério de aceite**

CI vermelho quando alguém commita Rust fora do formato.

---

### V7.3 — `release.mjs` traz um terceiro diretório de target, absoluto e específico da máquina

**Classificação:** dívida
**Gravidade:** baixa
**Status:** confirmado

**Onde**

[release.mjs:33](../scripts/release.mjs) `const TARGET_DIR = process.env.CARGO_TARGET_DIR || "C:/dev/loop-target";`

**O que acontece hoje**

O projeto tem três destinos de build: `C:/cargo-target/iracer` (config de dev), `src-tauri/target`
(o que o comando documentado cria, V7.1) e `C:/dev/loop-target` (o release). O caminho do release é
literal no script versionado.

**Por que isso é um problema**

Todo release recompila do zero, porque não reaproveita o cache de dev. Numa máquina sem `C:/dev`, o
script cria o diretório em silêncio.

**Correção recomendada**

Reaproveitar o mesmo target-dir do desenvolvimento, ou derivar o caminho de uma variável obrigatória
com mensagem de erro clara.

---

### V7.4 — Verificação final do release quebra com `TypeError` se o manifesto vier sem a plataforma

**Classificação:** dívida
**Gravidade:** baixa
**Status:** confirmado

**Onde**

[release.mjs:388](../scripts/release.mjs) `await fetch(live.platforms[TARGETS[channel]].url, ...)`

**O que acontece hoje**

O script confere `live.version` e vai direto ao `.url` da plataforma. Se a chave não existir, sai
`TypeError: Cannot read properties of undefined` com stack trace, na etapa 8, **depois** de o upload
já ter acontecido. É exatamente o momento em que o script precisa falar com clareza, porque o
rollback já acabou (o próprio comentário na linha 350 explica isso).

**Correção recomendada**

Validar `live.platforms?.[target]?.url` antes, com `die` explicando que o manifesto publicado não
tem a plataforma do canal.

---

## V8 — Testes

### V8.1 — `RosterGenPanel.test.jsx` é flaky sob carga

**Classificação:** bug
**Gravidade:** média
**Status:** confirmado, reproduzido

**Onde**

[RosterGenPanel.test.jsx:54](../src/components/iracing/RosterGenPanel.test.jsx)
`vi.useFakeTimers({ shouldAdvanceTime: true })` combinado com 20 `waitFor`.

**O que acontece hoje**

Duas execuções seguidas de `npm run test:ui`, dois casos **diferentes** do mesmo arquivo falhando:

```
1ª: "gera o roster com os quatro argumentos nomeados que o Rust espera"
2ª: "gera a temporada com o alvo de pista NULO quando o campo está vazio"
    AssertionError: o botão de temporada não disparou o comando: expected undefined to be truthy
```

Rodando o arquivo sozinho: 13 de 13 passam, em 1,36 s.

**Por que isso é um problema**

Uma suíte que falha em caso aleatório treina a equipe a reexecutar em vez de investigar. E este é o
arquivo que cobre o contrato de argumentos do export para o iRacing, ou seja, o guard mais barato de
perder a confiança.

**Evidência**

As duas execuções acima, mais a execução isolada.

**Correção recomendada**

Tirar os fake timers deste arquivo (ou isolá-los nos poucos casos que precisam deles) e transformar
os `waitFor` de "o botão disparou" em `findBy*`. Não aumentar timeout.

**Critério de aceite**

Dez execuções seguidas da suíte completa sem falha neste arquivo.

---

### V8.2 — Guards estruturais que passam verde com a condição real quebrada

**Classificação:** dívida
**Gravidade:** alta
**Status:** confirmado

Consolidação dos três casos provados nesta vistoria, porque o padrão é o mesmo e a correção é a
mesma família:

| Guard | Como ele passa verde | Item |
|---|---|---|
| `flags_experimentais::nenhuma_flag_de_regra_escapa_do_inventario` | vigia 3 arquivos escolhidos à mão; 3 flags vivem fora deles | V5.1 |
| `invoke-contra-generate-handler` (2º teste) | conta como "usado" qualquer literal de string, inclusive numa asserção negativa de teste | V1.3 |
| `invoke-contra-generate-handler` (2º teste) | conta `invoke` de módulo órfão como consumidor vivo | V1.4 |

O ponto comum: os três decidem cobertura a partir de uma **lista fixa** ou de uma **busca textual
sem contexto**. Os dois primeiros gritam quando a extração vem vazia (`assert.ok(nomes.length >=
150)`), o que é bom e não resolve este modo de falha: a extração não vem vazia, ela vem **errada
para mais**.

**Correção recomendada**

Regra geral para os guards do projeto: quando o guard depende de uma lista de arquivos, a lista tem
que ser derivada do diretório, não escrita à mão. Quando depende de busca textual, tem que excluir
teste e comentário.

**Critério de aceite**

Os três guards, sem nenhuma correção de produção, ficam vermelhos contra a árvore atual.

---

### V8.3 — 57 testes Rust ignorados, e o que isso deixa desprotegido

**Classificação:** dívida
**Gravidade:** média
**Status:** confirmado

**O que acontece hoje**

Dos 57, a grande maioria são harnesses de medição declarados como tal ("roda com `--ignored
--nocapture`"), e isso é uso legítimo. As exceções que importam:

- 3 são asserções que falham hoje (V3.3).
- `commands::race::fatura::primeiro_run_contra_um_save_real` depende de `LOOP_BASE_DIR` e de um save
  real: não roda em lugar nenhum automaticamente.
- `market::transfer_window::zzz_scale_validation`, `historical_draft::grid_skill_ladder_over_time`,
  `injured_orphans_are_reabsorbed_by_market_over_time` e
  `teammate_tension_climbs_and_finally_creates_rivalries` asseveram propriedades de mundo, não só
  imprimem, e ficam fora do CI por serem lentos.

**Correção recomendada**

Um job de CI semanal (`schedule`) que rode `cargo test -- --ignored` e reporte, separado do CI de
push. As três de V3.3 continuam vermelhas até o conserto, e é assim que deve ser.

---

### V8.4 — `constants.test.js` assevera o texto que a tela não usa

Ver V4.4. Registrado aqui porque a categoria é "teste que não testa o que diz testar".

---

## V9 — Higiene do repositório e código morto

### V9.1 — Lixo de bancada na raiz do repositório

**Classificação:** dívida
**Gravidade:** baixa
**Status:** confirmado

Na raiz, não rastreados: `%TEMP%/` (diretório de verdade, criado por um `mkdir "%TEMP%\..."` num
shell POSIX, com uma árvore `loop-esbuild-check/` dentro), `rh_fail.log` (157 KB, de 24/07). E, além
desses, quatro páginas de preview soltas: `__curva_preview.html`, `__fita-equipes-preview.html`,
`__mercado-card-preview.html`, `fita-mock.html`, `preview-curva-campeonato.html`.

Nada disso quebra nada. É ruído que qualquer pessoa nova vai tentar entender.

**Correção recomendada**

Apagar `%TEMP%/` e `rh_fail.log`; mover os previews para uma pasta `bancada/` ignorada, ou
gitignorar o padrão `__*.html`.

---

### V9.2 — 181 itens sem uso no build de produção da lib

**Classificação:** dívida
**Gravidade:** baixa
**Status:** parcialmente confirmado

**O que acontece hoje**

`cargo build --lib` reporta 181 avisos distintos de `never used` / `never read` / `never
constructed`: 85 funções, 35 constantes, 17 métodos, 12 structs, 6 campos, 2 variantes, 1 enum, 1
estático, mais 20 imports não usados.

**Ressalva importante, medida:** boa parte desses itens está viva nos testes. Conferi sete deles à
mão e cinco têm chamador em `#[cfg(test)]` (`calculate_gate_income`, `decide_maintenance`,
`maintain_team_car`, `compute_team_audience_appeal`, `financial_health_score`). Ou seja, o número
181 mede "API pública alcançada apenas por teste", não "código morto de verdade". Não consegui
separar limpo os dois conjuntos porque as duas builds emitem conjuntos textualmente disjuntos de
aviso.

O caso concreto de código morto de verdade que consegui provar por leitura é o V1.5
(`build_race_result_from_session`).

**Por que isso é um problema**

Um build com 201 avisos é um build cujos avisos ninguém lê. O próximo `unused_import` ou
`unreachable_pattern` de verdade vai entrar sem ser notado.

**Correção recomendada**

Começar pelos 20 imports não usados (correção mecânica, `cargo fix`), ligar `-D warnings` para essa
categoria no CI, e só então atacar o dead code com a pergunta certa por item: é API pública viva só
no teste, ou é código morto.

**Critério de aceite**

`cargo build --lib` sem avisos de import, e CI que impede a regressão.

---

## V10 — Documentação

### V10.1 — `CLAUDE.md`: comando de teste do Rust

Ver V7.1. É a única divergência de consequência prática que encontrei no CLAUDE.md.

### V10.2 — `CLAUDE.md`: a afirmação sobre `cargo fmt`/clippy

Ver V7.2. O padrão é declarado e não é praticado nem cobrado.

### V10.3 — O que conferi e está CORRETO

Registro aqui porque "documento obsoleto" é uma acusação que precisa de contraprova:

- `CLAUDE.md`, três webviews no `tauri.conf.json`: confere.
- `CLAUDE.md`, `useUIStore`/`useNotificationStore`/`hooks/useTauri.js` removidos: confere, não
  existem.
- `CLAUDE.md`, o array `MIGRATIONS` como fonte única da ordem: confere, e há guards estruturais
  cobrando (`schema-nasce-nas-migracoes`).
- `iracing-escopo.md` §2, "57 comandos registrados sob `commands::iracing::`": confere exatamente
  (contei 201 comandos no total, 57 deles de `iracing`).
- `divida-tecnica.md`: está atualizado. Os dois itens que ele lista como abertos
  (`race_weekend_readings` sem escritor e `overlay_window_set_interactive` sem consumidor) eu
  reencontrei de forma independente, e eles são reais (V2.4 e a lista congelada do guard).

### V10.4 — Dívida real que nenhum documento registra

Cruzando meus achados com os documentos: V1.1 (carro MX-5 para toda categoria alta), V1.2 (duração 0
no export), V2.1 (save de versão futura), V2.2 (`preseason_plan.json` fora da transação), V2.3
(restauração não atômica), V4.1 (catálogo de incidentes em PT no schema), V5.1 (flags fora do
inventário) e V7.1 (o 36 GB) não aparecem em `divida-tecnica.md`, `backlog.md` nem `roadmap.md`.

---

# Fechamento

## 1. Resumo executivo

**30 achados.** Foram atribuídos 34 IDs; quatro deles não são achado novo: V8.4, V10.1 e V10.2 são
referências cruzadas, e V10.3 é contraprova de documentação correta.

Por gravidade:

| Gravidade | Quantos | IDs |
|---|---:|---|
| Crítica | 0 | |
| Alta | 5 | V1.1, V1.2, V3.1, V5.1, V8.2 |
| Média | 17 | V1.3, V1.4, V1.5, V2.1, V2.2, V2.3, V2.4, V3.2, V3.3, V4.1, V4.2, V4.3, V6.1, V6.2, V7.1, V8.1, V8.3 |
| Baixa | 8 | V4.4, V6.3, V7.2, V7.3, V7.4, V9.1, V9.2, V10.4 |

Por tipo:

| Tipo | Quantos | IDs |
|---|---:|---|
| Bug | 9 | V1.1, V1.2, V2.1, V2.2, V4.1, V4.2, V5.1, V7.1, V8.1 |
| Dívida | 20 | os demais |
| Calibração | 1 | V3.1 |
| Design/produto | 0 achados isolados | 6 decisões humanas listadas no §7 |

**Nenhum achado crítico.** Não encontrei perda ou corrupção de dados em curso, nem falha que impeça
o release. O caminho de backup é bem construído (staging, `.old`, rollback, checkpoint do WAL que
recusa `busy`), a virada de temporada é transacional, e a ponte de memória compartilhada do VR tem
guarda de ABI nos três lados.

## 2. Top problemas

1. **V3.1 — a corrida é decidida na classificação.** Pole vence 76% a 85% contra alvo de 15% a 55%.
   É o número mais distante do alvo em toda a árvore, ele afeta campeonato, narrativa, avaliação e
   mercado ao mesmo tempo, e está medido pelo instrumento do próprio projeto.
2. **V1.1 — export de MX-5 para GT3.** Escreve artefato errado dentro da instalação do jogador, em 6
   das 10 categorias, sem nenhum erro. É o caminho principal do produto.
3. **V5.1 — três flags de regra de jogo fora do inventário, com guard verde.** Duas delas mexem no
   dinheiro das equipes. O módulo existe exatamente para impedir isso e não impede.
4. **V8.2 — três guards com falso verde.** Enquanto eles estiverem assim, qualquer conclusão tirada
   deles (inclusive as deste relatório) precisa ser reconferida à mão.
5. **V1.2 — duração 0 exportada para `endurance`.** Bug certo, alcançabilidade dependente de V1.1.

## 3. Mapa por área

| Área | Achados |
|---|---:|
| V1 Ponte Rust/Tauri/React | 5 |
| V2 Banco e persistência | 4 |
| V3 Simulação e balanceamento | 3 |
| V4 i18n | 4 |
| V5 Flags de ambiente | 1 |
| V6 Frontend | 3 |
| V7 Build, CI e release | 4 |
| V8 Testes | 4 (2 originais, 2 cruzados) |
| V9 Higiene e código morto | 2 |
| V10 Documentação | 4 (2 cruzados, 1 contraprova, 1 lacuna) |

## 4. Problemas transversais

- **Fallback silencioso como padrão de projeto.** V1.1 (`else` → mx5), V1.2 (sentinela 0 lida
  direto), V6.2 (77 `catch` vazios) e os `unwrap_or_default` sobre JSON de snapshot são a mesma
  decisão repetida: preferir seguir com um valor plausível a parar. Em quase todos os lugares isso
  está certo; nos quatro casos acima produz artefato errado ou perda de sinal.
- **Guard que decide cobertura por lista fixa ou busca textual.** V5.1, V1.3, V1.4. O projeto tem uma
  cultura forte de guard estrutural, e ela está batendo no limite da técnica: buscar texto sem
  entender contexto conta demais, e listar arquivo à mão conta de menos.
- **Texto de apresentação atravessando fronteira e sendo persistido.** V4.1 (schema), V4.2 (news),
  V4.3 (`.js` fora do auditor). O i18n do projeto é bem montado no eixo JSX↔locale e tem três buracos
  fora dele.
- **Três diretórios de build de Rust.** V7.1 e V7.3. Custa disco, tempo de release e provoca colisão
  de linker entre sessões.

## 5. Dívida arquitetônica

Refactors que não são bug e mudam a forma do sistema:

1. **Uma fonte de verdade para categoria → carro do iRacing**, no Rust, com retorno tipado. Hoje a
   regra está duplicada em JS e Rust e o formato é string livre (V1.1).
2. **A sentinela de duração morre no tipo.** `duracao_efetiva` já devolve `DuracaoDeProva`, que não
   consegue carregar zero. Falta fazer todos os caminhos passarem por ela, inclusive o export
   (V1.2).
3. **`news` guarda chave e parâmetros, não prosa renderizada.** É o que permite a notícia antiga
   acompanhar a troca de idioma (V4.2). Toca formato de save.
4. **`incident_catalog` guarda chave, não frase** (V4.1). Mesma família, mesma migração.
5. **Separar "API pública viva só no teste" de "código morto"** no crate, e ligar `-D warnings` por
   categoria (V9.2).

## 6. Calibrações pendentes

O que não dá para chamar de certo ou errado sem medir:

1. **Os alvos de `Alvos::entrada()` e `Alvos::topo()`.** Antes de perseguir V3.1 é preciso decidir se
   as faixas descrevem o jogo desejado. Um Spearman alvo de 0,60 a 0,88 na GT3 já é bem determinístico;
   0,95 é outro patamar, e a diferença entre "queremos 0,75" e "queremos 0,88" muda inteiramente o
   tamanho do conserto.
2. **Os knobs do primeiro trecho** (largada, ar sujo, custo de ultrapassagem). Existe varredura pronta
   (`imprime_varredura_de_knobs`, `varredura_do_trafego_mede_alavanca`); ninguém rodou contra o alvo.
3. **`IRACER_GATE_SHARE = 0.12` e `IRACER_SALARY_SHARE = 0.15`.** Ambos declarados como "calibrado por
   Monte Carlo" nos comentários, ambos sobrescrevíveis por ambiente e ambos fora do inventário. A
   procedência do número não está registrada em lugar nenhum que eu tenha encontrado.
4. **`IRACER_PROMO_DIMINISH_DECAY/WINDOW/FLOOR`** (0,55 / 3,0 / 0,15), declaradas no inventário como
   `Destino::Indefinido`, ou seja, o próprio código diz que o A/B não fechou.
5. **A taxa de abandono.** 0,63 por etapa no rookie e 0,30 na GT3, com o catálogo real. Não há alvo
   declarado para ela; sem alvo não dá para dizer se está alta ou baixa.

## 7. Decisões de produto necessárias

Perguntas que só o Carlos responde. Não escolhi nenhuma delas.

1. **Categorias sem carro grátis equivalente (GT4, GT3, LMP2, Production Challenger, Endurance): o
   Loop recusa a exportação, mapeia para um carro pago, ou marca a categoria como "só simulada"?**
   (V1.1)
2. **Existe cenário em que o resultado do iRacing precisa ser construído da sessão ao vivo em vez do
   JSON do aiseason?** Se não existe, `result_bridge/sessao.rs` sai. (V1.5)
3. **A leitura do fim de semana (migração v55) vai ser gravada por quem?** Se ninguém, o trio
   getter/setter/comando sai. (V2.4)
4. **`RosterGenPanel` e `PostRacePanel` ganham lugar na UI ou são apagados?** São 1.423 linhas e 6
   comandos de ponte pendurados nessa decisão. (V6.1, V1.4)
5. **Notícia antiga deve acompanhar a troca de idioma?** A resposta "sim" implica mudar o formato de
   save. A resposta "não" precisa ser escrita em algum lugar, porque hoje é acidente e não decisão.
   (V4.2)
6. **O padrão `cargo fmt`/clippy vale?** Se vale, entra no CI e alguém roda o `fmt` de uma vez. Se
   não vale, sai do CLAUDE.md. (V7.2)

## 8. Lacunas de testes

O que não está protegido:

- **O contrato categoria → carro do export** (V1.1). Nenhum teste percorre o catálogo de categorias.
- **A duração exportada** (V1.2). Nenhum teste prova que o AI season sai com duração não zero.
- **Save de schema futuro** (V2.1). Não há caso.
- **Atomicidade da restauração** (V2.3). O backup tem teste de falha injetada; a restauração não.
- **A propriedade central da simulação** (V3.3). Os três testes que a cobrem estão ignorados por
  falharem.
- **i18n em `.js`** (V4.3). 129 arquivos fora do auditor.
- **O texto que o carrossel de carregamento realmente mostra** (V4.4). O teste olha o array errado.
- **Toda env `IRACER_` fora dos 3 arquivos vigiados** (V5.1).
- **Módulos vivos apenas pelos próprios testes** (V6.1). Três painéis contribuem para o verde da
  suíte sem estarem no produto.

## 9. Divergências de documentação

Cobertas em V10. Em resumo: os documentos do projeto estão em bom estado, incomum para o tamanho da
árvore. As duas divergências reais são no CLAUDE.md (comando de cargo e a afirmação sobre fmt/clippy).
`divida-tecnica.md` está atualizado e não achei nele dívida documentada que já não exista. O que falta
é o inverso: oito dívidas reais que documento nenhum registra (V10.4).

## 10. Áreas não auditadas em profundidade

Digo explicitamente o que **não** cobri com a mesma profundidade, para o relatório não passar por
completo:

- **`iracing_sdk/spotter_*`** (cerca de 6.000 linhas em 8 arquivos). Li a estrutura e os guards
  estruturais que os cobrem, não a lógica de decisão de cada anúncio.
- **`engenheiro/`** e o pipeline de rádio/TTS. Li o contrato de peças e os guards; não segui o fluxo
  de fala.
- **`market/pipeline/consolidacao.rs`** (1.499 linhas) e `commands/career/champion.rs` (1.587). Abri
  para pontos específicos, não de ponta a ponta.
- **`vr-overlay/src/overlay_layer.cpp`**. Li o `shared_frame.h` e o lado Rust do contrato, e confiei
  nos 6 guards estruturais que cruzam os três lados. Não compilei a layer nem li o C++ inteiro.
- **`commands/overlay/torre.rs`** e o desenho dos overlays (1.187 linhas).
- **`generators/`, `hierarchy/`, `public_presence/`, `rivalry/`, `convocation/`**: passei por eles
  pelos guards e pelos testes, não por leitura completa.
- **O proxy de IA** (`narrative/client.rs` e o serviço remoto). Vi os endpoints e o gate `LOOP_SEM_IA`;
  não auditei o serviço, que está fora do repositório.
- **Os 80 scripts de `scripts/`**. Auditei em profundidade `release.mjs`, `make-update-manifest.mjs`,
  `i18nAudit.mjs` e `convert-track-images.mjs`; os demais só por varredura de operações destrutivas.
- **`installer/hooks.nsh`** e o empacotamento NSIS. Não exercitei o instalador.
- **Migrações v56 a v64 uma a uma.** Li a mecânica de versionamento e a v53/v54/v55; as demais só
  pelos guards de schema.

## 11. Ordem recomendada de ataque

Baseada em dependência técnica, risco e custo de descobrir tarde.

**Bloco 0, antes de qualquer conserto (meio dia).** Consertar os três guards de V8.2. Motivo: eles são
o instrumento com que todo o resto vai ser verificado, e hoje mentem para mais. Um guard corrigido
transforma V5.1, V1.3 e V1.4 em vermelho automático, e aí os itens se resolvem sozinhos como trabalho
mecânico.

**Bloco 1, o caminho principal quebrado (1 a 2 dias).** V1.1 e V1.2 juntos, porque V1.2 só é
alcançável depois de V1.1 e os dois moram no mesmo par de arquivos. Entrega: nenhuma exportação
silenciosamente errada.

**Bloco 2, integridade de save (1 a 2 dias).** V2.1, V2.2, V2.3. São independentes entre si e não
dependem de nada acima. Entrega: o save deixa de ter três caminhos de inconsistência conhecidos.

**Bloco 3, calibração (aberto).** V3.2 primeiro (mecânico, uma linha por teste), depois a decisão de
produto sobre os alvos (§7 item nenhum, §6 item 1), e só então V3.1. Não começar por V3.1: sem alvo
decidido, a varredura não tem critério de parada.

**Bloco 4, i18n (2 a 3 dias, mais migração).** V4.3 e V4.4 são baratos e independentes. V4.2 (as três
strings) é meia hora. V4.1 e a parte de formato de V4.2 dependem de decisão de produto e de migração,
e podem ficar para depois.

**Bloco 5, higiene (1 dia, a qualquer momento).** V7.1, V7.2, V7.3, V7.4, V9.1, V9.2, V6.1, V6.2,
V8.1. Nenhum depende de nada.

## 12. Frentes paralelizáveis

Grupos que não colidem em arquivo e podem ir para pessoas ou agentes diferentes:

| Frente | Itens | Arquivos que toca | Colisões |
|---|---|---|---|
| **A. Guards** | V8.2 (V5.1, V1.3, V1.4) | `constants/flags_experimentais.rs`, `scripts/tests/invoke-contra-generate-handler.test.mjs`, `finance/cashflow.rs`, `finance/salary.rs`, `commands/iracing/importacao.rs` | nenhuma |
| **B. Export iRacing** | V1.1, V1.2 | `nextRaceHelpers.js`, `useIracingExport.js`, `commands/iracing/{pintura,roster,temporada}.rs`, `iracing_sdk/roster_gen.rs` | toca `importacao.rs` de raspão, coordenar com A |
| **C. Persistência** | V2.1, V2.2, V2.3 | `db/migrations.rs`, `evolution/pipeline/orquestracao.rs`, `market/preseason/plano.rs`, `commands/save/restore.rs` | nenhuma |
| **D. i18n** | V4.2 (strings), V4.3, V4.4 | `scripts/i18nAudit.mjs`, `src/utils/constants.js`, `src/utils/constants.test.js`, `commands/race/noticias/manchetes.rs`, locales | nenhuma |
| **E. Build e CI** | V7.1, V7.2, V7.3, V7.4, V9.1 | `CLAUDE.md`, `.github/workflows/ci.yml`, `scripts/release.mjs`, raiz | V7.2 (`cargo fmt`) reformata 119 arquivos e colide com **todas** as outras frentes de Rust: rodar sozinho, numa janela em que ninguém mais está mexendo |
| **F. Front morto** | V6.1, V6.2, V8.1 | `src/components/{iracing,race,team}/...` | V6.1 remove os arquivos que a frente A usa como evidência: fazer A primeiro |
| **G. Calibração** | V3.1, V3.2, V3.3 | `simulation/**` | isolada do resto; nenhuma colisão com A a F |

Recomendação de sequenciamento entre frentes: **A → (B, C, D, G em paralelo) → F → E**, com **E/V7.2
(`cargo fmt`) reservado para uma janela exclusiva**.
