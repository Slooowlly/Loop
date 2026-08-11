# R2 — Três motores de tese derivando os mesmos sinais

**Área:** Rust + frontend · **Risco:** médio · **Conflita com:** R1 — não rode em paralelo

---

## Situação em 11/08/2026 — RESOLVIDO no Rust; o item de frontend virou opcional

A segunda análise ([R2-analise.md](R2-analise.md)) recomendou fazer parcialmente, em três
fatias. As duas de Rust estão em produção; a terceira é frontend e continua opcional.

| Fatia | Situação hoje |
|---|---|
| **P0** — campo de mérito com uma construção só | **Resolvido.** [`commands/race/merito.rs`](../../src-tauri/src/commands/race/merito.rs) é a fonte única, chamada por `fatos.rs:360` (boletim) e `importacao.rs:45` (debrief). O bug de unidade acabou: os dois usam `Team::car_strength()` em 0–100, e piloto ausente do banco entra com valor neutro em vez de encolher o grid. |
| **P1** — camada de sinais compartilhada | **Resolvido.** [`race_signals.rs`](../../src-tauri/src/race_signals.rs) tem um limiar por conceito e a classificação única do abandono (`dnf_kind`). Consumido por `narrative/{tese,beats}.rs`, `commands/ai_news/{tese,fatos}.rs` e `race_eval.rs`. |
| **P2** — frontend ler o `assessment` persistido | **Não feito, e classificado como opcional.** `nextRaceThesis.js` continua recalculando `dismal` em JS. É mudança do que o jogador lê na prévia, então é produto, não conserto. |

### O remapeamento pedido: evento → sinal → tese → notícia

```
evento real
  simulação  → IncidentResult / RaceDriverResult / RaceResult
  iRacing    → result_bridge → o mesmo RaceResult
        ↓
sinal estruturado
  race_signals::dnf_kind          (incidente > peça quebrada > texto, nessa ordem)
  race_signals::{remontada, remontada_epica, colapso, pole_frustrada,
                 overperf, underperf, vitoria_improvavel, caos}
  race_eval::evaluate  ← campo de mérito único de commands/race/merito.rs
        ↓
tese / debrief
  narrative/tese.rs            eixo do boletim  (voz de revista, grid inteiro)
  commands/ai_news/tese.rs     eixo do debrief  (1ª pessoa, piloto do jogador)
        ↓
notícia / narrativa
  narrative/contexto.rs → texto de fatos → narrative/client.rs → servidor
  commands/ai_news/comandos.rs → debrief e prévia
```

### Inconsistências procuradas de novo, uma a uma

- **DNF mecânico vs. batida.** Era o desencontro real (um lia `IncidentType`, o outro rodava
  regex no motivo). Fechado: os dois chamam `dnf_kind`, e o dano latente de colisão — que o
  motor registra como `Mechanical` — tem ramo próprio e dá a mesma resposta pelos dois
  caminhos. Há teste prendendo as duas pontas nos dois idiomas.
- **Remontada.** Tinha quatro limiares (>0, 4, 5, 8). Hoje são dois conceitos nomeados e
  distintos: `remontada` (≥4) e `remontada_epica` (≥8 e chegando em ≤6). O segundo não é um
  limiar concorrente, é a régua da manchete.
- **Over/under performance.** Mesmo `evaluate`, mesmo campo de mérito. Era aqui que morava a
  contradição "dia de somar" vs. "muito abaixo do esperado", e ela não existe mais.
- **`marcos.rs:285` (`gained >= 6`)** e **`fatos.rs:762` (`positions_gained >= 1`)**: parecem
  um quinto e um sexto limiar de remontada, e não são. O primeiro é o piso para uma
  recuperação virar **recorde de categoria**; o segundo é o piso para a telemetria render
  uma frase. Perguntas diferentes. **Falso positivo.**
- **`db/queries/injuries.rs`** tem listas próprias de radicais (`colis`, `batid`, `capot`…).
  É arqueologia de save legado para inferir **severidade de lesão**, não natureza do
  abandono, e o próprio arquivo explica. **Falso positivo.**
- **`narrative/contexto.rs`, `total_dnfs >= 2`** no cabeçalho, contra `caos()` na tese. São
  perguntas diferentes: uma decide se vale citar a contagem de abandonos, a outra decide se
  o caos é o eixo da matéria. Fica como está.

**Nenhuma duplicação técnica eliminável sobrou.** As três vozes continuam separadas, que é o
que o briefing original pediu para não destruir.

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
