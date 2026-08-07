# A carga do rádio — as quatro famílias numa linha do tempo só

Medição feita em 05/08/2026, sobre as duas capturas de referência do acervo (Lime Rock e
Okayama, ~17 min cada) somadas ao modelo de desgaste do Loop.

Quatro famílias abrem o canal sem ser chamadas — spotter, quebra na grade, peça do nosso carro
e volta mais rápida. Cada uma foi medida antes de existir, e cada uma foi medida **sozinha**. A
pergunta que faltava é a única que o jogador faz: **cabe num par de ouvidos?**

Duas famílias calmas separadas podem ser um rádio insuportável juntas, e o defeito não aparece
em taxa nenhuma. Aparece na **fila** — desde que ela existe (`src/lib/engenheiroVoz.js`), nada é
cortado no meio, e o que acontece com o excesso é atraso e descarte silencioso.

## Como rodar

```bash
node scripts/analise-spotter.mjs <captura.jsonl.gz> --linha-do-tempo=<saida.json>
cargo test --lib despeja_linha_do_tempo -- --ignored --nocapture
node scripts/analise-radio.mjs <saida.<captura>.<sessao>.json>
```

As três metades da entrada têm naturezas diferentes, e isso decide o que cada número vale:

| metade | fonte | natureza |
|---|---|---|
| spotter | captura real do iRacing | telemetria gravada — os instantes aconteceram |
| quebra + peça própria | `engenheiro::medicao_radio` | modelo de desgaste, 30 realizações |
| volta mais rápida | voltas da captura | tempos reais, observador espelhado |

A **duração** de cada fala não é estimada: sai do `.wav` que vai tocar, pelo cabeçalho RIFF. É o
que torna isto uma medição — a ocupação do canal é a soma de áudios que existem no disco, com as
pausas que `pausasDoRadio` calcula.

O que ela **não** modela, e conta a favor do rádio: o push-to-talk. Uma pergunta do piloto
esvazia a fila, então toda pergunta melhora estes números.

## O tamanho da grade decide tudo

A quebra é por carro por volta. Dobrar a grade dobra a conversa, e não há um número só:

| grade × prova | quebra (fundida) | peça própria | total do engenheiro |
|---|---|---|---|
| 24 carros × 18 voltas | 29,9 | 3,9 | **33,8 falas** |
| 12 carros × 18 voltas | 16,1 | 3,5 | **19,6 falas** |

A peça própria mal se mexe — ela é do nosso carro, e o nosso carro é um só.

As duas capturas longas do acervo têm **40 carros**, que é grade de campo cheio do iRacing e não
a do Loop. Elas continuam valendo pelo que medem bem (o spotter em tráfego pesado, e o
comportamento da fila sob pressão), mas o número de falas por minuto delas é o teto, não o normal.

## O resultado

| | Ledenon **12 carros** | Lime Rock 40 | Okayama 40 |
|---|---|---|---|
| falas por minuto (mediana / pior) | **0,5 / 3,7** | 5,5 / 9,0 | 8,5 / 11,6 |
| canal ocupado | **3,5% / 10%** | 20% / 28% | 27% / 35% |
| maior silêncio | 127 s / 225 s | 123 s / 321 s | 194 s / 315 s |
| atraso do anúncio (mediana / pior) | 0,0 s / 3,2 s | 0,0 s / 19,8 s | 0,0 s / 19,8 s |
| descartados | 0 | 0 / 3 | 0 / 2 |
| anúncios que chegaram a tocar | **100%** | **100%** | **100%** |

> A corrida de Ledenon é curta: **3 voltas, 341 s**, e é a única captura de corrida com 12 carros
> no acervo. Serve para a taxa por minuto e não serve para nada que dependa de distância de prova.

**Na grade de 12, o rádio é o oposto de tagarela.** Uma volta de Ledenon leva 97 s, então uma
prova de 18 voltas dura ~29 min — e as 19,6 falas do engenheiro dão **0,7 por minuto**. Somando o
spotter medido ali, algo perto de **1,2 falas por minuto: uma a cada 50 s**. Há orçamento de fala
sobrando, e é isso que autoriza gatilhos proativos novos.

**A fila aguenta.** Praticamente nenhum anúncio morre, e o atraso mediano é zero — o canal está
livre quando a notícia chega. O pior caso encosta nos 20 s da validade, o que quer dizer que
`VALIDADE_ANUNCIO_MS` está no lugar certo: ele corta exatamente a cauda que não valia mais.

**Com 40 carros ele fica falante.** Uma fala a cada 7 s na mediana de Okayama, com o canal
ocupado mais de um quarto da corrida. E a composição diz de onde vem:

| origem | mediana por piloto-corrida (Okayama) | |
|---|---|---|
| spotter lateral | 115 | 76% |
| quebra na grade | 30 | 20% |
| peça do nosso carro | 4 | 3% |
| spotter medido (fora/parado/trás) | 2 | 1% |
| volta mais rápida | **0** | 0% |

**Sem o lateral, são 2,0 falas por minuto** — nas duas pistas, com pior caso de 2,5. Esse é o
piso, e é confortável.

> ⚠ **O lateral é HIPÓTESE, não medida.** `CarLeftRight` está parado nas duas capturas (o humano
> passou as duas provas no box) e o canal é dele. O número acima vem da reconstrução geométrica a
> ±5 m descrita em [`spotter-obstaculo.md`](spotter-obstaculo.md). Ele domina o resultado, então
> **a próxima captura com o humano na pista muda esta página** — e é a única que pode fechá-la.

### Uma armadilha de método que mudou o resultado pela metade

A primeira rodada colapsava todas as quebras de uma volta no instante em que o **jogador** cruzou
a linha. Com isso a fila estourava em toda corrida: pilha de 6 em 100% dos pilotos, 21% dos
anúncios descartados, atraso mediano de 4 s.

Era artefato. Cada carro cruza a linha na sua vez, e o monitor avalia carro a carro — a rajada
não existe na corrida. Espalhando o evento dentro da volta pela posição do carro, o descarte cai
de 21% para 0,4%. **O número errado dizia para consertar a fila; o certo diz que ela está boa.**

## Dois defeitos que a medição encontrou — os dois consertados

### 1. O rádio de ritmo nunca falava ✅

Zero falas nas duas corridas, para os 40 pilotos. Não é calibração — é o gatilho.

O observador dispara em **troca de dono** da volta mais rápida. Medido nas três capturas:

| | Ledenon (3 voltas) | Lime Rock | Okayama |
|---|---|---|---|
| melhorias da volta mais rápida | 2 | 3 | 5 |
| **trocas de carro** | 1 | **0** | **0** |
| pilotos que chegam a menos de 0,9 s dela | — | 3 de 40 | 3 de 40 |

O padrão está nas duas provas longas: um carro crava a melhor volta nas primeiras passagens e
**melhora a própria marca** o resto do tempo. Ninguém toma dele. Como "melhorou" não é "trocou de
dono", o anúncio some depois do primeiro terço — e a aproximação, que precisa de 0,9 s, só
alcançaria 3 pilotos de 40.

Em Ledenon houve uma troca, e ela foi na abertura, quando todo mundo ainda estava melhorando. É a
mesma leitura: **a família só tem o que dizer nas voltas iniciais**, e cala justo quando a corrida
fica interessante.

As 14 peças da família estão gravadas e caladas. Os 2.101 tempos de volta **não** estão perdidos:
eles têm o outro consumidor, o push-to-talk (`"Volta em, um trinta e dois e quatro."`), que
responde quando o piloto pergunta.

**Consertado.** O gatilho passou a ser a **melhoria** da marca, com a mesma trava de intervalo —
`ritmo::Observador` guarda `ultima_melhor_s` e compara. Remedido depois da troca, em Okayama:
**2 falas por piloto-corrida**, contra zero. O teste que faltava está lá com o nome do que ele
mede (`o_dono_melhorando_a_propria_marca_e_noticia`); a família inteira estava muda porque todos
os testes falavam de troca de dono, que era a única coisa que o código sabia ver.

### 2. A quebra do nosso carro era anunciada em terceira pessoa ✅

1,3 vez por corrida, o rádio diz ao jogador:

> *"O piloto um da Racing Academy Red foi retirado da corrida com problemas no motor."*

O piloto um da Racing Academy Red **é ele**. O carro do jogador entra no `breakdown_log` como
qualquer outro (`tick_breakdown_player`), e `get_breakdown_feed` monta a fala para toda linha do
log sem separar a dele. Como o nome do jogador não sai de pool nenhum, a fala cai na forma pela
equipe — que é justamente a que soa como se fosse sobre outra pessoa.

**Consertado, e custou 108 gravações.** O carro do jogador saiu do feed da grade
(`get_breakdown_feed` filtra pelo número dele) e o desfecho passou a sair pelo canal que já é em
2ª pessoa, `get_player_warnings`, com a família nova
[`peca_propria::desfecho_frase`](../src-tauri/src/engenheiro/peca_propria.rs) — 12 peças × 3
severidades × 3 redações.

A alternativa barata era falar só o desfecho ("Acabou por hoje") em 9 peças, com o argumento de
que o aviso anterior já tinha dito qual peça era. Não vale: o aviso só sai se a peça cruzou a
janela de risco, e quebra sem aviso prévio existe — nesses casos o jogador ficaria sem saber o
que largou, justo no momento em que a corrida dele acabou.

A linha continua entrando no `breakdown_log` (é ela que manda o `!black`/`!dq` e alimenta o
resultado). O que mudou é quem fala dela.

## O fôlego é da TOMADA, não do texto

Descoberto ao auditar as nove redações novas: três saíram com 0,21 a 0,25 s de silêncio dentro da
gravação. Reescritas as três, a segunda rodada trouxe **outra** peça respirando — uma que tinha
saído limpa na primeira. A mesma frase, dois resultados.

O gerador só avisava disso no console, e um aviso sobre peça que já está no disco não conserta
nada: a execução seguinte a pula pelo "já existe". Agora ele **refaz** a tomada, como já fazia
com a peça muda, com uma isenção para os textos de duas frases (o conselho de poupar, onde o
respiro é o certo). As 108 peças novas saíram sem um aviso sequer.

E o aviso que sobrou virou diagnóstico: um fôlego que sobrevive a cinco tomadas não é azar, é o
texto.
