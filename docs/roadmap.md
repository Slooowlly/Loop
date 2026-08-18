# Roadmap — Loop

O que falta, como falta e por que falta. Levantado em 2026-07-27 lendo o código:
a lista de `generate_handler!` do [lib.rs](../src-tauri/src/lib.rs), os consumidores
de cada comando no frontend, e as telas realmente montadas. Reconferido em 11/08/2026.

Complementa o [backlog.md](backlog.md): lá está a lista priorizada com ids, aqui está
o raciocínio. Onde os dois divergirem, vale o que estiver conferido com data mais
recente, e a divergência é corrigida no mesmo dia em que aparecer.

---

## O diagnóstico em uma frase

**O Loop está organizado por momento da temporada, não por assunto.** Quase tudo que
"falta" existe — só que preso ao instante em que acontece, sem lugar permanente onde
o jogador possa voltar e consultar.

Isso não é desleixo, é consequência de como o app cresceu: cada feature nasceu junto
com o evento que a dispara. O mercado nasceu dentro da pré-temporada. A evolução do
piloto nasceu dentro do fim de temporada. O que ficou de fora foram exatamente as
visões que não pertencem a nenhum evento — e por isso nunca tiveram um gatilho para
serem escritas.

A correção estrutural do roadmap é: **promover assuntos a cidadãos de primeira classe**,
reusando a lógica que já existe.

**Aplicada em 11/08/2026, e o diagnóstico se confirmou por inteiro.** O assunto "eu" ganhou
lugar permanente — a aba **Carreira**, com cinco seções — e nenhuma das cinco precisou de
simulação nova. O único comando escrito foi `get_season_market_board`, que lista assentos
vazios; todo o resto saiu de payloads que já cruzavam a ponte e eram desenhados de esguelha,
ou nem isso. A previsão "quase tudo que falta existe" não era otimismo: era medida.

O assunto "o mundo ao longo do tempo" continua parcialmente órfão. Ele existe no recorte de
EQUIPE (`TeamHistoryDrawer`, `AtlasChampionsPanel`, `TeamRecordsTab`) e ganhou o recorte do
JOGADOR agora; o que não tem lugar é a história do mundo pela ótica dos PILOTOS de IA ao longo
das décadas. Isso não está no backlog e não é pedido de ninguém — fica registrado aqui só
para a próxima varredura não o descobrir como novidade.

---

## Correção de uma leitura anterior

A primeira varredura (que gerou o `backlog.md`) concluiu que "o mercado não tem tela".
**Isso está errado** e vale registrar para ninguém repetir o erro: a conclusão veio de
ver `MarketTab.jsx` como um placeholder de 9 linhas sem import.

O que existe de verdade:

| componente | linhas |
|---|---|
| `season/PreSeasonView.jsx` | 481 |
| `season/preseason/` (19 arquivos: contrato, ofertas, agentes livres, grid de equipes, transferências) | ~2100 |
| `season/PoachAuctionHost.jsx` + `PoachAuctionModal.jsx` | montado global no `App.jsx` |

São ~2700 linhas de UI de mercado funcionando. `MarketTab.jsx` era um esqueleto do
scaffold inicial (commit `f4e4449`, "Initial project baseline") que nunca foi tocado —
a feature cresceu em outro lugar e o stub ficou para trás mentindo sobre o estado do app.
Foi removido em `31dd713`.

**A lição de método:** um arquivo com nome promissor e 9 linhas é evidência mais fraca
que um `grep` pelos comandos do domínio. Neste repo, procure pela feature a partir do
comando Tauri, não a partir do nome do arquivo.

---

## Os buracos reais

### 1. ~~Backup e restauração~~ ✅ resolvido, conferido em 11/08/2026

**Estava errado neste documento até 11/08/2026.** A seção dizia que faltava "qualquer
interface. Nenhuma", e que os três comandos estavam registrados sem nenhum chamador no
frontend. A conclusão veio de um grep que não pegou o consumidor real.

O que existe: `src/components/ui/BackupsModal.jsx` consome `list_backups`,
`create_season_backup` e `restore_backup`, e é aberto por `src/pages/LoadSave.jsx`.
O fechamento está registrado em [divida-tecnica.md](divida-tecnica.md) e o item F-06
já saiu do [backlog.md](backlog.md).

**A lição de método:** este documento e o backlog ficaram três dias com verdades
diferentes sobre o mesmo item, porque a seção foi atualizada em um arquivo só. Ao
fechar um item, corrija os três lugares no mesmo commit: aqui, no backlog e na dívida
técnica.

---

### 2. ~~História das temporadas passadas~~ ✅ resolvido, 11/08/2026

**Era o maior buraco de produto.** O jogador não conseguia olhar para trás: nenhuma tela de
temporadas anteriores, de recordes, de títulos acumulados.

**Fechado pela aba Carreira** (`src/pages/tabs/carreira/`), seções História e Troféus:
a carreira em números, a escada de categorias percorrida, a curva de campeonato e a tabela
temporada por temporada, mais a prateleira de títulos com ano e equipe, as primeiras vezes e
o auge. F-03, F-04 e F-05 fecharam juntos, como esta seção previa.

**A aba foi apagada em 14/08/2026, e o buraco continua fechado.** Quem responde hoje é a
ficha do piloto (`DriverDetailModalV2`), aberta clicando no próprio nome na Home: a aba
Histórico dela serve a mesma trajetória, a curva de campeonato, os primeiros marcos, o auge,
a confiabilidade e os eventos especiais, e a aba Habilidade serve o dossiê. A aba Carreira
era uma segunda porta para o mesmo `get_driver_detail`.

**O que a execução ensinou, e que a análise não tinha visto:** o backend não precisou de uma
linha. A leitura era que a informação estava presa no `EndOfSeasonView` (uma cerimônia de
passagem que aparece uma vez e some) e que faltava expor `race_history`. Na verdade
`get_driver_detail` já servia trajetória, curva de campeonato com posição/grid/esperado,
títulos detalhados, marcos, auge, queda, confiabilidade e sábado — tudo pronto, para
QUALQUER piloto, inclusive o jogador. O buraco era só de lugar.

**Uma decisão de custo que vale registrar:** a posição do jogador nos recordes do MUNDO ficou
de fora. Ela existe em `get_driver_dossier_ranks`, e custa uma varredura de `race_results` e
do arquivo inteiro (~500ms num save maduro, medido antes de ser separado do payload da ficha
justamente por isso). A sala de troféus tem que abrir na hora, e "205º de 610 em vitórias" é
a pergunta seguinte, não esta — quem quer o ranking abre a lista global de pilotos.

---

### 3. ~~Mercado fora da janela~~ ✅ resolvido, 11/08/2026

**Era o buraco de continuidade:** o mercado só era navegável na pré-temporada. Durante o ano
o jogador recebia eventos pontuais — assédio, ofertas especiais, interesses — que chegavam,
exigiam resposta e desapareciam.

**Fechado pela seção Mercado da aba Carreira:** estado do contrato com aviso de último ano,
valor de mercado contra o salário atual (a distância entre os dois é o que diz se ele está
barato ou caro), quem está de olho em você, e as vagas abertas do mundo. Um comando novo,
`get_season_market_board`, que anota em cada assento vazio o veredito de elegibilidade pela
MESMA regra da proposta emergencial (licença da divisão mais faixa de tier).

**Correção de uma leitura desta seção.** Ela dizia: "`advance_transfer_window` está registrado
e nunca é chamado pelo frontend — indício de que a janela de transferências no meio do ano
existe no backend e não tem condução na UI". O indício era falso. O corpo do comando era
IDÊNTICO ao de `get_transfer_window_state`, com o único parâmetro que os distinguia
(`accepted_seat_id`) ignorado, e o próprio doc-comment em `transfer_market.rs` dizia isso:
"ficou legado: apenas devolve o estado atual das ofertas, sem avançar nada". Quem avança o
mercado é `advance_market_week` → `preseason::advance_week`.

Ligá-lo teria produzido um botão que não faz nada. O comando foi **removido** em 11/08/2026,
e saiu de `SEM_CONSUMIDOR_CONHECIDO` no guard.

**A lição de método:** comando registrado sem consumidor é indício de tela que falta OU de
código morto, e as duas hipóteses se separam lendo o CORPO — não a lista de registro. Este
ficou dois anos classificado como "feature futura, esperando o F-01" porque ninguém abriu a
função.

---

### 4. ~~Ficha do piloto~~ ✅ resolvido, 11/08/2026

**Era o buraco de identidade:** o jogador se enxergava com a lente de observador do mundo, o
mesmo `DriverDetailModal` usado para olhar qualquer piloto do grid.

**Fechado pela seção Meu piloto da aba Carreira**, e o que resolve o item não é a seção — é o
CABEÇALHO. Ele fica fora das pílulas, sempre visível, e carrega nome, título, licença, lesão,
momento, motivação e posição no campeonato. O modal continua existindo e continua servindo
para olhar qualquer um, inclusive o jogador; o que ele nunca foi é um LUGAR, porque abre por
cima de outra tela e some quando o jogador clica fora.

A seção abre pela habilidade MEDIDA (`get_player_dossier`), que é a única leitura do jogo que
existe para o jogador e não existe para a IA: atributos inferidos das corridas que ele
realmente correu, e não um número escrito no save.

---

### 5. ~~Espectadores e interesse de evento~~ ✅ resolvido, 11/08/2026

**Era o buraco de feedback:** o sistema modulava economia, narrativa e motivação, e o jogador
sentia o efeito sem ver a causa.

**Dois terços já estavam prontos quando esta seção foi conferida**, feitos depois que o
briefing F07 foi escrito (o arquivo `briefings/F07-espectadores-interesse.md` foi removido em
11/08/2026 junto com o fechamento do item — ver o [README dos briefings](briefings/README.md)):

- a repercussão pós-corrida (`EventRepercussionSummary` → `RepercussionSegment` e
  `RepercussionCard` no `RaceResultViewV2`), que era a fatia P1 do briefing e a mais valiosa:
  o confronto entre esperado e entregue, com o `delta_vs_expected` e o `headline_strength`;
- a presença pública da equipe (`presenca_publica` → `LineupStrip` no `MyTeamTabV2`), a fatia
  P2, com a linha que diz que ela multiplica o patrocínio.

**Faltava a exibição rica do interesse ESPERADO**, que o `DESIGN.md` §17.1 pedia junto da
outra: o público era um número no canto direito do card de clima, sem tier, sem escala e sem
relação com o jogador — dentro de um botão que abre a previsão do tempo. Virou exibição própria,
hoje `EventInterestBanner.jsx`, com o tier (`tier_label`, traduzido pelo backend), o público, o
porte da ocasião e a cota de plateia que a estrela do jogador puxa (`public_fame_share`), que é o
fio entre a fama dele e a bilheteria. Foi card na coluna de condições até 14/08/2026, quando virou
faixa sem moldura no vão central do cabeçalho da Sala de Estratégia.

**Segue fora de escopo, e continua verdade:** os três multiplicadores marcados como "uso
futuro" (`pressure_modifier`, `media_multiplier`, `motivation_multiplier`) são calculados em
`event_interest/calculator.rs` e não têm nenhum leitor. Ligá-los é design de equilíbrio, não
exposição de dado.

---

### 6. ~~Rivalidades~~ ✅ resolvido, 11/08/2026

Rivalidade tinha nascido como **adjetivo**: uma marcação que qualificava nove outras telas e
nunca precisou virar substantivo. Faltava a visão consolidada — quem são meus rivais, desde
quando, qual o placar.

**Fechado pela seção Rivais da aba Carreira**, de carona na tela que F-03 já montava, como
esta seção previa. Um card por rivalidade com placar de corrida e de sábado, gap médio, box
dividido, origem e nível; o Nemesis sobe ao topo, e o nome da rivalidade aparece quando há
capítulo registrado. Os rótulos de nível e de origem reusam as chaves `driverDetail.rivals.*`
de propósito: a faixa de intensidade sai de `rivalry::intensity_level` no Rust, e um segundo
jogo de rótulos no JS divergiria do primeiro na primeira recalibração.


### 7. Outras categorias — provavelmente já resolvido

**Status: verificar antes de agendar.** `GlobalDriversTab` e `GlobalTeamsTab` já dão a
visão do mundo inteiro, atravessando as 9 categorias, com ranking global
(`get_global_driver_rankings`) e histórico de equipes. O `OtherCategoriesTab` era outro
stub do scaffold.

Antes de tratar como tarefa, responda: o que uma aba de "outras categorias" mostraria
que as duas abas globais já não mostram? Se a resposta for "a classificação da
categoria vizinha", isso é um filtro de categoria no `StandingsTab`, não uma aba.

---

### 8. Integração com iRacing — ✅ decidido em 2026-07-27

Análise completa em [iracing-escopo.md](iracing-escopo.md); resumo em [DESIGN.md](DESIGN.md) §19.

**A decisão:** o Loop é uma **ferramenta de iRacing com uma carreira simulada dentro**.
Correr de verdade é o caminho principal.

**A correção de leitura:** nem o `DESIGN.md` (que dizia "removido, expansão futura") nem a
versão anterior desta seção (que dizia "exportar virou ler") estavam certos. `export/` foi
deletado, mas a exportação **mudou de casa** para `iracing_sdk/roster_gen.rs` e
`season_gen.rs`. A integração é um ciclo fechado: exporta roster+temporada → o jogador
corre → importa o resultado oficial + os sinais do monitor ao vivo. Eram 49 comandos (não ~15) e
16.910 linhas em 27/07; **recontado em 11/08/2026 são 57 comandos e 33.101 linhas de
`iracing_sdk/`**, mais 5.738 em `commands/iracing/`.

**O que sobrou de trabalho real** (reconferido em 11/08/2026, com duas correções):

Os dois painéis foram **aposentados em 18/08/2026**: `RosterGenPanel` (726 linhas) e
`PostRacePanel` (696) saíram do repositório com os próprios testes de contrato, fechando a decisão
que estava pendente desde 27/07. **Os 6 comandos que só eles chamavam** saíram junto, na mesma
data e por decisão do dono: `iracing_dump_session_yaml`, `iracing_preview_race_result`,
`iracing_apply_player_paint`, `iracing_player_custid`, `iracing_player_paint` e
`iracing_export_rain_test` — comando, registro e implementação. O núcleo compartilhado (pintura
automática, ponte de import) segue vivo. Recontado na §4 e §6.2 do
[iracing-escopo.md](iracing-escopo.md).

**Correção 1: a exportação não depende deles.** O ciclo que o jogador percorre hoje passa
por `src/components/race/nextrace/useIracingExport.js`, que chama os mesmos
`iracing_generate_roster` e `iracing_generate_season`. Os painéis são a bancada anterior,
com controles de diagnóstico que a tela do jogador não tem. O que está preso neles é o
diagnóstico, e não o caminho principal.

**Correção 2: a dificuldade adaptativa está LIGADA — e ligada não é o mesmo que executada.** A
leitura de que `iracing_process_race_result` estaria "implementada e nunca executada" ficou velha
pela metade. O **wiring** existe: ela é chamada de dentro do próprio Rust, em
`commands/iracing/importacao.rs:138`, amarrada à corrida entrar na carreira. O que não tem
consumidor é o COMANDO registrado, e não a lógica — o painel desligado deixou de ser o único
caminho. (O próprio `PostRacePanel`, enquanto existiu, documentava a escolha em comentário: processar ali duplicaria o
ajuste.)

A **execução** é outra pergunta, e ela continua aberta até uma corrida real. A prova é o par de
linhas `[import]` + `[adaptativo]` no `loop.log`, e o `[adaptativo]` agora é log explícito de
sucesso: sai sempre que o ajuste roda, terminando em `gravado` ou `sem mudança`. A receita completa
está na §6.1 do [iracing-escopo.md](iracing-escopo.md) e no §19.3 do [DESIGN.md](DESIGN.md).

O inventário dos comandos sem consumidor está em D-05 no [backlog.md](backlog.md) e congelado no
guard [`invoke-contra-generate-handler`](../scripts/tests/invoke-contra-generate-handler.test.mjs).
**O guard é a contagem oficial; nenhum número escrito aqui é.** Este parágrafo dizia 22, e a lista
mudou duas vezes só em 11/08/2026 — um comando removido (`advance_transfer_window`) e três que
ganharam tela (os de desfazer pintura e modo janela). Para contar hoje:

```bash
node --test scripts/tests/invoke-contra-generate-handler.test.mjs
```

Backlog derivado na §6 do `iracing-escopo.md`.

---

## Dívida técnica — o que muda o custo do resto

Detalhe por item em [backlog.md](backlog.md) (D-01 a D-10). O que importa para o roadmap:

**Deixou de bloquear em 11/08/2026:** os cinco briefings de acoplamento do lado Rust
([varredura-acoplamento/](varredura-acoplamento/), item D-09) foram reconferidos contra o
código atual e a dívida técnica deles fechou. Narrativa e hierarquia não são mais fundação
em disputa: **R3 e R5 resolvidos**, **R2 resolvido no Rust** (campo de mérito único em
`commands/race/merito.rs` e camada de sinais em `race_signals.rs`), **R1 e R4 com a parte
técnica corrigida** — código morto fora, `allow(dead_code)` fora, comentários falsos
apontando para quem realmente escreve.

O que sobrou dos cinco **não é dívida técnica e não bloqueia**: promover forma, lesão-arco e
marcos a beat do boletim (R1) e dar consequência nova à hierarquia interna (R4) são decisões
de design que valem discutir junto do produto; o eixo de tensão que não anda é calibração.
Um achado novo virou item próprio, **D-10** (`planned_events` write-only no plano de
pré-temporada), separado por tocar formato de save.

**Não bloqueia nada, só incomoda:** convocação legada (D-01) e tabela `races` legada
(D-02). Fazem barulho em qualquer varredura futura e escondem sinal.

**Fechados em 11/08/2026:** stores stub (D-03), `useTauri.js` morto (D-04) e a dívida
técnica do D-09. O registro está em [divida-tecnica.md](divida-tecnica.md).

**Cuidado com os briefings:** o README da varredura avisa que o método (grep excluindo
o próprio módulo) **gera falsos positivos** — re-export consumido por um irmão aparece
como "sem chamador". Cada briefing pede uma segunda análise antes de virar código. O
episódio do `MarketTab` neste documento é exatamente esse erro em outra forma.

---

## Ordem sugerida

**A fila de produto fechou em 11/08/2026**, e ela se cumpriu em UMA tela em vez de quatro.

| # | o quê | como saiu |
|---|---|---|
| 1 | ~~**Aba de História** (F-03 + F-04 + F-05)~~ ✅ | Virou as seções História, Troféus e Rivais da aba **Carreira**. Ver §2 e §6. |
| 2 | ~~**Ficha do piloto** (F-02)~~ ✅ | Virou a seção Meu piloto e, principalmente, o cabeçalho fixo da aba. Ver §4. |
| 3 | ~~**Mercado em temporada** (F-01)~~ ✅ | Virou a seção Mercado, com o comando novo `get_season_market_board`. O `advance_transfer_window` que esta tabela mandava conduzir era um no-op e foi removido. Ver §3. |
| 4 | **Etapa B do boletim + consequência da hierarquia** (o que sobrou do D-09) | **Único item de produto ainda aberto.** Deixou de ser dívida e virou design: quais beats de carreira entram no boletim com que peso, e o que a briga interna N1/N2 passa a causar. Discutir junto do produto, não como refactor. |
| 5 | ~~**Espectadores** (F-07)~~ ✅ | Dois terços já estavam prontos; faltava o card de interesse esperado. Ver §5. |
| — | ~~**Backup/restauração** (F-06)~~ ✅ 11/08/2026 | Já existia: `BackupsModal.jsx` aberto pelo `LoadSave.jsx`. Ver §1. |
| — | ~~**Decidir o escopo do iRacing** (F-10)~~ ✅ 27/07/2026 | Decidido: ferramenta de iRacing com carreira dentro. Ver [iracing-escopo.md](iracing-escopo.md) §6 para o backlog derivado. |

**A previsão de sequenciamento estava certa e o custo estava superestimado.** A tabela pedia
quatro entregas em ordem porque cada uma parecia uma tela. Três delas eram seções de uma tela
só, e o argumento que justificava juntar F-03+F-04+F-05 ("comem da mesma `race_history`")
valia igual para F-02 e F-01, que comem do mesmo `get_driver_detail(jogador)`. O sinal de que
os cinco eram um item só estava à vista desde a primeira varredura: todos eram sobre **eu**.

**Fora da fila:** F-08 (outras categorias) até alguém responder o que ele mostraria que as
abas globais já não mostram. F-09 (previsões) é sabor; estava bloqueado por F-01 e F-02 e
agora está livre — e o lugar dele é uma sexta seção da aba Carreira, não uma aba nova.

---

## Como manter este documento

Ele descreve **buracos e o porquê deles**, não tarefas. Item resolvido sai daqui; o
registro de execução é o `backlog.md`. Se a próxima varredura contradisser algo escrito
aqui, corrija no lugar e diga o que a evidência anterior tinha de errado — a seção
"Correção de uma leitura anterior" é o formato.
