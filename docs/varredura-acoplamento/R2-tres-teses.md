# R2 — Três motores de tese derivando os mesmos sinais

**Área:** Rust + frontend · **Risco:** médio · **Conflita com:** R1 — não rode em paralelo

## O que foi encontrado

O projeto elege "qual foi a história" três vezes, em três lugares, de forma
independente.

| Motor | Arquivo | Voz | Eixo |
|---|---|---|---|
| Boletim do grid | [`src-tauri/src/narrative/tese.rs`](../../src-tauri/src/narrative/tese.rs) | revista, grid inteiro | `RaceThesis`: Caos, VitoriaImprovavel, PoleFrustrada, Remontada, Dominio, CorridaLimpa |
| Debrief pós-corrida | [`src-tauri/src/commands/ai_news/tese.rs`](../../src-tauri/src/commands/ai_news/tese.rs) | piloto do jogador | `select_post_race_thesis(&PostRaceSignals)` |
| Prévia pré-corrida | [`src/pages/tabs/nextRaceThesis.js`](../../src/pages/tabs/nextRaceThesis.js) | piloto do jogador, antes | — (frontend, JS) |

O comentário em `ai_news/tese.rs` referencia `nextRaceThesis.js` explicitamente:
"Mesmo princípio da prévia (nextRaceThesis.js)".

## O que **não** é o problema

As três vozes são diferentes **de propósito** e isso está documentado nos dois
arquivos Rust. `narrative/tese.rs` diz que ali "o eixo é a HISTÓRIA DA CORRIDA" e
"o piloto do leitor segue citado, nunca protagonista", em oposição explícita ao
debrief. **Não unifique as vozes.** Uma fusão ingênua destrói uma decisão de design.

## O que é o problema

Os **sinais brutos** de onde as três teses partem são os mesmos conceitos, derivados
três vezes:

- DNF mecânico vs. DNF por erro/contato
- remontada (ganho de posições)
- colapso (perda de posições)
- over/under performance vs. expectativa
- vitória / pódio improvável
- caos (contagem de DNFs)

`ai_news/tese.rs` parte de `race_eval::Assessment` (Acima/MuitoAcima/Abaixo/MuitoAbaixo)
+ `PostRaceSignals`. `narrative/tese.rs` parte de `RaceThesisSignals` construído em
`race_thesis_signals(result)` direto do `RaceResult`. O frontend parte do que o
backend já mandou serializado.

Três derivações do mesmo conceito é onde nasce a incoerência: o debrief dizer "você
foi bem acima do esperado" e o boletim da mesma corrida chamar de "dia de
administração".

## Armadilhas conhecidas

1. **Este briefing toca `narrative/`, igual ao R1.** Se R1 estiver em andamento,
   espere. Coordene.
2. `narrative/tese.rs` é deliberadamente **puro e testável** — o doc-comment de
   `RaceThesisSignals` diz "sem depender da estrutura inteira do `RaceResult`".
   Fazer ele importar `race_eval` pode quebrar essa pureza. Avalie.
3. `race_eval` é player-cêntrico (avalia o piloto do jogador contra a expectativa).
   O boletim do grid precisa do equivalente para **qualquer** piloto. Pode não ser
   uma reutilização direta, e sim uma generalização.
4. O frontend é JS e não pode importar do Rust — a coerência ali só vem via o que o
   backend serializa.

## O que eu quero da segunda análise

1. **Tabela de sinais.** Para cada um dos ~6 conceitos acima, mostre como cada um
   dos três motores o calcula (arquivo:linha, fórmula, limiares). Onde os limiares
   divergem, aponte.
2. **Existe incoerência observável hoje?** Construa um ou dois cenários concretos de
   corrida em que debrief e boletim classificariam a mesma corrida de formas
   contraditórias. Se não conseguir construir nenhum, diga isso — talvez o problema
   seja teórico e não valha o refactor.
3. **`race_eval` generaliza?** Ele é player-cêntrico. Dá para avaliar qualquer
   piloto do grid contra expectativa com o que já existe, ou falta dado (grid de
   largada esperado, força do carro)? Custo dessa generalização.
4. **Proponha a camada de sinais compartilhada** — um módulo que produz os fatos
   brutos, consumido pelos três seletores, preservando as três vozes. Diga o que
   entra nele e, importante, **o que deve ficar de fora** por ser específico de voz.
5. **O frontend entra?** `nextRaceThesis.js` é prévia (não tem resultado ainda), então
   pode ser que ele use um conjunto de sinais estruturalmente diferente e não pertença
   a esta unificação. Decida e justifique.
6. **Vale a pena?** Quero uma recomendação franca de fazer / não fazer / fazer só
   parcialmente, com o custo estimado. Este é o item da varredura em que mais suspeito
   que a duplicação seja aceitável.

Não aplique nada ainda — quero ler a análise antes.
