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

### 2. História das temporadas passadas — o maior buraco de produto

**O que falta:** o jogador não consegue olhar para trás. Não existe tela de temporadas
anteriores, de recordes, de títulos acumulados, de campeões do passado.

**Como falta:** o backend está pronto e é rico — `db/queries/race_history/` responde
recordes por pista (`pistas.rs`), títulos por categoria (`recordes.rs`), e o `world/`
já arquiva a temporada inteira. `get_global_team_history` e `get_team_history_dossier`
existem e são consumidos, mas **só no recorte de uma equipe** (`TeamHistoryDrawer`,
`GlobalTeamsTab`). Não há a visão do mundo ao longo do tempo, nem a do próprio jogador.

O `EndOfSeasonView.jsx` (665 linhas) mostra o balanço do ano — promoções, licenças,
evolução de atributos — mas é uma **cerimônia de passagem**: aparece uma vez, na virada,
e nunca mais. A informação é gerada, exibida por um instante e some.

**Por que falta:** é o caso mais puro do diagnóstico. Olhar para o passado não é um
evento — é algo que o jogador quer fazer numa terça-feira qualquer, por curiosidade.
O app nunca teve esse gatilho.

**Por que importa mais do que parece:** um simulador de carreira sem memória consultável
é um jogo sem consequência sentida. O mundo do Loop tem 200+ pilotos que sobem, caem e
se aposentam — e nada disso é visitável depois que rola a tela.

**Como fazer:** uma aba de História no Dashboard cobrindo temporadas passadas, sala de
troféus e recordes. As três coisas comem da mesma `race_history`; separar em três telas
seria triplicar navegação para uma fonte só.

---

### 3. Mercado fora da janela — o buraco de continuidade

**O que falta:** visibilidade do mercado **durante** a temporada.

**Como falta:** o mercado só é navegável na pré-temporada, via `PreSeasonView`. Durante
o ano, o jogador recebe eventos pontuais — assédio (`PoachAuctionHost`, global),
ofertas especiais (`get_player_special_offers`), interesses (`get_player_interests`) —
que chegam, exigem resposta e desaparecem. Não há onde consultar, no meio da temporada:
quanto tempo resta do meu contrato, quem está de olho em mim, que vagas abriram.

`advance_transfer_window` está registrado e nunca é chamado pelo frontend — indício de
que a janela de transferências no meio do ano existe no backend e não tem condução na UI.

**Por que falta:** o mercado foi construído como um capítulo do calendário. Enquanto o
jogo só perguntasse "o que você faz nesta pré-temporada?", isso bastava.

**Como fazer:** não é uma aba nova do zero. É um painel de estado contratual permanente
que reusa os mesmos comandos do `marketSlice`, mais a condução de
`advance_transfer_window`. Barato perto do valor.

---

### 4. Ficha do piloto — o buraco de identidade

**O que falta:** uma tela do seu próprio piloto.

**Como falta:** os dados existem e são exibidos, mas sempre de esguelha.
`PlayerSkillSection.jsx` mostra atributos dentro do modal de detalhe do piloto;
o `EndOfSeasonView` mostra a evolução na virada do ano. `get_driver_detail` serve o
`DriverDetailModal`, que é a mesma ferramenta usada para olhar **qualquer** piloto do grid.

Ou seja: o jogador enxerga a si mesmo com a lente de observador do mundo, não com a de
protagonista. Motivação, licença, forma recente, lesões, progressão de carreira e
histórico de contratos estão espalhados ou invisíveis.

**Por que falta:** o `DriverDetailModal` resolveu o problema "ver um piloto" bem demais.
Serviu para o jogador também, e a falta nunca doeu o suficiente para virar tarefa.

**Por que importa:** o Loop é sobre controlar **um** piloto. A ausência de uma ficha
própria é a distância mais curta entre "simulador que roda sozinho" e "jogo com dono".

---

### 5. Espectadores e interesse de evento — o buraco de feedback

**O que falta:** a UI que o próprio `DESIGN.md` §17.1 já registra como pendente.

**Como falta:** `event_interest/` e `public_presence/` calculam interesse por evento e
presença pública por equipe/piloto. No frontend, a palavra aparece em três lugares
(`EngineerBriefingPanel`, `OfferCardRich`, `RivalMarker`) — sempre como tempero, nunca
como leitura própria.

**Por que falta:** é um sistema que modula outros sistemas (economia, ofertas, narrativa)
sem ter um momento próprio. Sente-se o efeito sem ver a causa.

**Por que é médio e não alto:** o jogador não sente falta do que não sabe que existe.
Vale depois dos quatro primeiros da ordem sugerida, e sobe de prioridade se a economia
começar a parecer arbitrária ao jogador.

---

### 6. Rivalidades — quase pronto, sem sala própria

**O que falta:** menos do que o backlog dizia.

**Como falta:** rivalidade já aparece em nove componentes — `RivalMarker`, marcação no
calendário (`DayCellV2`), seções do detalhe do piloto, análise de corrida, movimento
semanal da pré-temporada, e um painel dedicado no lado iRacing
(`RivalryPerceptionPanel`). O que não existe é a visão consolidada: quem são meus
rivais, desde quando, qual o placar.

**Por que falta:** rivalidade nasceu como **adjetivo** — uma marcação que qualifica
outras telas. Nunca precisou virar substantivo.

**Custo real:** baixo, e cai mais ainda se entrar de carona na aba de História (item 2),
que já vai montar a navegação e a leitura de histórico.

---

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
corre → importa o resultado oficial + os sinais do monitor ao vivo. São 49 comandos (não
~15) e 16.910 linhas.

**O que sobrou de trabalho real** (reconferido em 11/08/2026, com duas correções):

Os dois painéis continuam órfãos: `RosterGenPanel` (726 linhas) e `PostRacePanel` (696)
não são importados por nenhum arquivo de `src/`. Segue valendo a decisão pendente — ligar
ou apagar.

**Correção 1: a exportação não depende deles.** O ciclo que o jogador percorre hoje passa
por `src/components/race/nextrace/useIracingExport.js`, que chama os mesmos
`iracing_generate_roster` e `iracing_generate_season`. Os painéis são a bancada anterior,
com controles de diagnóstico que a tela do jogador não tem. O que está preso neles é o
diagnóstico, e não o caminho principal.

**Correção 2: a dificuldade adaptativa RODA.** A leitura de que
`iracing_process_race_result` estaria "implementada e nunca executada" ficou velha: hoje ela
é chamada de dentro do próprio Rust, em `commands/iracing/importacao.rs:127`, amarrada à
corrida entrar na carreira. O que não tem consumidor é o COMANDO registrado, e não a lógica
— o painel desligado deixou de ser o único caminho. (O próprio `PostRacePanel` documenta a
escolha em comentário: processar ali duplicaria o ajuste.)

O inventário completo dos 22 comandos sem consumidor, com a natureza de cada um, está em
D-05 no [backlog.md](backlog.md) e congelado no guard
[`invoke-contra-generate-handler`](../scripts/tests/invoke-contra-generate-handler.test.mjs).
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

| # | o quê | por que agora |
|---|---|---|
| 1 | **Aba de História** (F-03 + F-04 + F-05) | Maior ganho de produto por linha escrita. Três itens do backlog numa tela só, todos comendo da mesma `race_history`. Devolve consequência ao mundo simulado. |
| 2 | **Ficha do piloto** (F-02) | Transforma o app em jogo com protagonista. Depende de decidir o que é "meu piloto" contra "um piloto". |
| 3 | **Mercado em temporada** (F-01 revisado) | Extensão do que já funciona, sem construção nova. Inclui conduzir `advance_transfer_window`. |
| 4 | **Etapa B do boletim + consequência da hierarquia** (o que sobrou do D-09) | Deixou de ser dívida e virou design: quais beats de carreira entram no boletim com que peso, e o que a briga interna N1/N2 passa a causar. Discutir junto do produto, não como refactor. |
| 5 | **Espectadores** (F-07) | Depois que houver telas onde a informação caiba. |
| — | ~~**Backup/restauração** (F-06)~~ ✅ 11/08/2026 | Já existe: `BackupsModal.jsx` aberto pelo `LoadSave.jsx`. Ver §1. |
| — | ~~**Decidir o escopo do iRacing** (F-10)~~ ✅ 27/07/2026 | Decidido: ferramenta de iRacing com carreira dentro. Ver [iracing-escopo.md](iracing-escopo.md) §6 para o backlog derivado. |

**Fora da fila:** F-08 (outras categorias) até alguém responder o que ele mostraria que
as abas globais já não mostram. F-09 (previsões) é sabor, e só entra depois dos quatro
primeiros.

---

## Como manter este documento

Ele descreve **buracos e o porquê deles**, não tarefas. Item resolvido sai daqui; o
registro de execução é o `backlog.md`. Se a próxima varredura contradisser algo escrito
aqui, corrija no lugar e diga o que a evidência anterior tinha de errado — a seção
"Correção de uma leitura anterior" é o formato.
