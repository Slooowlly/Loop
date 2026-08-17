# Spotter — obstáculo à frente

O que a captura de corrida precisa registrar para que o spotter possa um dia avisar
"carro parado à frente", e por quê. Escrito em 04/08/2026, depois de rodar
[`scripts/analise-frente.mjs`](../scripts/analise-frente.mjs) contra sete capturas reais.

## As perguntas

Uma captura só serve se, sozinha, responder isto para cada carro:

1. Quando saiu da pista?
2. Quanto tempo ficou fora?
3. Estava em asfalto, grama, areia ou brita?
4. Ficou parado?
5. Isso foi no grid, no box, na formação, em corrida ou depois da bandeirada?
6. Havia alguém se aproximando?
7. A que distância, e em quanto tempo chegaria?
8. Retomou, foi para o box, ou sumiu do mundo?

O gravador do formato **3** responde as oito. O 2 respondia três.

## O que já estava provado antes de mexer

Medido contra as capturas existentes, não suposto:

- **A distância por `ΔLapDistPct × TrackLength` está certa, inclusive no wrap.** 917 s de
  Ledenon, 9 cruzamentos da linha, **zero** saltos inexplicados na distância do carro à frente.
- **`CarIdxEstTime` é populado e coerente em sessão offline com IA** — 100% dos carros na
  mesma ordem de `lap_dist_pct`, em todas as capturas.
- **`CarIdxF2Time` não serve**: 100% zerado em duas capturas, 3% em outras, 34% na de julho.
- **A velocidade derivada de Δpct bate com a real**: erro mediano de 0,9 km/h a 60 Hz e
  2,0 km/h a 4 Hz, contra a `Speed` do próprio jogador.
- **`OffTrack` aparece em sessão offline com IA.** Pouco (27 amostras), mas aparece.

## O defeito que a análise encontrou

O detector ingênuo — *parado + `OnTrack` + não-`InPitStall` + não-`NotInWorld`* — marcou
**os onze carros do grid**, 80 amostras cada. Não era acidente nenhum: `SessionState` 1 e 2,
`pct` 0,9847, superfície `OnTrack`, 0 km/h. Carros legitimamente imóveis antes da largada.

A lista de exclusão protegia contra box e fora-do-mundo, que **não aparecem uma única vez
em todo o acervo**, e não protegia contra o grid, que aparece em **todas** as sessões. Daí
`SessionState` e `PaceMode` serem gravados: sem contexto de momento da prova, "parado na
pista" não quer dizer nada.

## O que o formato 3 acrescenta

### Por carro

| Campo | Responde |
|---|---|
| `track_surface_material` | pergunta 3 — asfalto, grama, areia, brita |
| `rpm` | motor vivo ou morto num carro imóvel: vai voltar ou vai ficar |
| `steer` | manobra anormal, recuperação |
| `session_flags` | bandeira **daquele** carro (amarela local, preta) |
| `pace_line`, `pace_row`, `pace_flags` | pergunta 5, do lado do carro |
| `best_lap_num`, `fast_repairs_used` | contexto barato |

`track_surface_material` é o mais importante da lista. `OffTrack` sozinho é ambíguo: pisar
a área asfaltada de uma escapada e enterrar o carro na brita dão o mesmo valor, e só um dos
dois vira obstáculo. Sem o material, ou o aviso dispara em todo limite de pista da corrida,
ou é calibrado tão alto que perde o acidente.

### Globais

`session_tick`, `session_laps_remain`, `session_laps_total`, `pace_mode`, `pits_open` —
pergunta 5. `precipitation`, `weather_declared_wet`, `track_temp_crew`, `skies`, `fog_level`,
`wind_dir` — o ritmo normal de um trecho depende do tempo, e sem isso um carro em ritmo de
chuva parece um carro com problema. `cam_*` e `replay_*` não entram em detector nenhum: são
para achar o instante no replay depois, porque "o carro 7 ficou a 2 km/h por 1,8 s" não
distingue batida de engarrafamento — mas o número do frame do replay distingue.

### Registros novos

- **`vars`** — o inventário do que o SDK publica nesta build: nome, tipo, quantidade,
  unidade e descrição. A leitura casa nomes num `match`, e um nome que não existe cai no
  `_ => {}` calado; sem o inventário, um canal ausente e um canal zerado são idênticos
  vistos de fora. Com ele, dá para saber o que se poderia ter lido sem reabrir o sim.
- **`session` repetido.** O YAML não é estático: é reescrito quando a sessão avança e,
  sobretudo, quando os resultados saem. Guardar só a primeira cópia é guardar a corrida sem
  o desfecho — sem `ReasonOutStr`, ninguém sabe quem abandonou e por quê. Agora regrava a
  cada mudança, com `n` crescente.
- **`cars[]` a 20 Hz** (era 4). Não porque 4 Hz errasse a velocidade — não erra —, mas
  porque a pergunta mudou de "está parado?" para "quando parou e por quanto tempo ficou".
  A 4 Hz cada fronteira de estado tem ±250 ms de incerteza, e é uma duração de fronteira
  que precisa ser calibrada.

## A corrida gravada

Lime Rock Park, 2369 m, 40 carros de IA, 17,1 min, jogador parado no box.
`race_1785885657.jsonl.gz`, 19 MB. É a primeira captura de corrida do acervo — as sete
anteriores eram todas `Open Qualify`.

O formato 3 se provou: **327 variáveis** no inventário do SDK, todos os canais críticos
presentes (`CarIdxTrackSurfaceMaterial`, `CarIdxSessionFlags`, `CarIdxPaceFlags` inclusive),
166 cópias do YAML, `cars[]` a 20 Hz cravados durante a corrida (deltas de 3 ticks; a média
de 15 Hz do arquivo inteiro é diluída pelas fases de menu e replay). Zero salto inexplicado
de distância em 17 minutos.

### O falso positivo que o `SessionState` não pega

No instante `t=74,6 s`, **os 40 carros** aparecem a 0 km/h, em asfalto, na pista — e com
`SessionState` já em **4 (Correndo)**. É a largada parada: o estado vira "corrida" antes de
o grid andar. Dura de 1,0 a 1,7 s por carro.

Isto derruba a conclusão anterior de que gravar `SessionState` resolvia o grid. Resolve o
grid *antes* da largada; não resolve a largada. A regra que resolve é outra, e é barata:

> **Só é obstáculo quem estava andando.** Um carro parado em corrida só conta se passou de
> 50 km/h em algum momento dos últimos 10 s.

Aplicada aos dados: 40 episódios → **0**. Não custa nada, porque um carro que parou na pista
por definição estava andando instantes antes; quem nunca andou é grid.

### Fora da pista bruto é limite de pista

Quatro episódios de `TrackSurface == 0` com mais de 1 s. Dois deles a **167 e 202 km/h** —
carro passando por cima da grama na saída da curva, o que acontece toda volta e não é
obstáculo nenhum. Os outros dois a **51 e 42 km/h**, com perda grande de ritmo: esses são
excursões de verdade.

O separador é a velocidade contra o próprio ritmo recente. Com o filtro *fora da pista **e**
perdeu mais de 40% do pico dos últimos 10 s*, sobram exatamente os dois reais.

### A janela de aviso, medida

Dos dois obstáculos reais:

| Obstáculo | Duração | Perseguidor mais próximo |
|---|---|---|
| `#4`, `t=922,6 s` | 9,2 s | 90 m — 2,3 s |
| `#12`, `t=802,8 s` | 4,2 s | 3 m — 0,1 s |

Sobre os 24 pares obstáculo↔perseguidor dentro de 500 m: distância p10 = 18 m, p50 = 289 m;
tempo até chegar p10 = 0,6 s, p50 = 5,6 s.

Daí sai a decisão que faltava, e ela **não** é "quanto mais cedo melhor":

- **400 m (≈12 s) é cedo demais.** Os obstáculos duraram 4,2 e 9,2 s. Avisar a 12 s é avisar
  de algo que já terá saído da frente quando o carro chegar — a pior espécie de falso
  positivo, porque o piloto freia por nada.
- **Os casos mais próximos não são avisáveis.** O `#12` apareceu com um carro a 3 m. Nenhum
  rádio ajuda ali; aceitar isso é parte do projeto.
- **A faixa útil é 100–200 m, ou 2 a 5 s.** Longa o bastante para servir, curta o bastante
  para o obstáculo ainda estar lá.

### Taxa-base

Com o detector final (fora da pista + perda de ritmo, ou parado + estava andando).
O denominador é **carro-quadro em corrida, presente em `cars[]`, na pista, fora do box** —
está dito porque a primeira medição não o registrou e ficou irreproduzível:

| Janela | Lime Rock | Okayama |
|---|---|---|
| 100 m | 0,127% | 0,468% |
| 150 m | 0,195% | 0,554% |
| 200 m | 0,234% | 0,616% |
| 300 m | 0,313% | 0,726% |

> Os valores publicados antes para Lime Rock (0,106% a 100 m, e proporcionalmente os
> outros) eram ~20% menores em todas as quatro janelas — razão constante, portanto
> diferença de denominador e não de eventos. A medição acima sai de
> `scripts/analise-spotter.mjs`, que imprime o denominador junto e é repetível.

Duas ocorrências em 17 minutos de corrida com 40 carros. É raro — e é por isso que vale.
Um aviso que toca uma ou duas vezes por corrida é informação; um que toca a cada volta é
ruído que o piloto aprende a ignorar.

## O detector implementado

[`spotter_frente.rs`](../src-tauri/src/iracing_sdk/spotter_frente.rs). Um episódio por
carro, um aviso por episódio.

**Abre** com: sessão em corrida **+** `TrackSurface == OffTrack` **+** velocidade abaixo
de 60% do pico dos últimos 10 s **+** pico acima de 50 km/h.
**Fala** quando: o carro está à frente, a ≤ 200 m, com 2 a 5 s de chegada, e o jogador
está na pista.
**Encerra** com: volta a superfície válida, entra no box, `NotInWorld`, é ultrapassado
pelo jogador, ou a sessão sai de corrida.

Duas decisões que os dados forçaram e que não estavam na especificação:

- **A velocidade não entra no encerramento.** A perda de ritmo é filtro de ENTRADA — é
  ela que separa a escapada do corte de grama. Depois de aberto, um carro que recupera
  ritmo ainda enterrado na grama continua sendo o mesmo obstáculo; fechar ali só faria o
  episódio piscar e reabrir.
- **Um episódio novo exige uma volta ao normal no meio.** Sem isso, todo encerramento que
  não vem da física reabre no tick seguinte: o jogador ultrapassa um carro na grama, o
  episódio fecha como `Ultrapassado`, o carro continua exatamente onde estava, e 16 ms
  depois nasce outro. Em 10 s de escapada são centenas de episódios idênticos.

### Reprodução sobre a corrida gravada

O detector, rodado sobre `race_1785885657`, encontra **3 episódios**: 11,80 s (`#5`),
4,18 s (`#13`) e 0,20 s (`#22`). Os dois cortes de grama a 167 e 202 km/h ficam de fora,
como projetado, e **zero** episódios de "parado" — a largada não passa.

Como o jogador ficou no box, nenhum aviso saiu. Para exercitar a via do rádio, cada um
dos 40 carros foi simulado como jogador:

| | |
|---|---|
| Avisos somados | 15 |
| Média por piloto por corrida | **0,38** |
| Pilotos que ouviram mais de uma vez | **0** |
| Faixa dos avisos | 90–200 m, 2,1–5,0 s |

Uma vez a cada duas ou três corridas, por piloto. É a frequência de algo que o piloto
ouve e leva a sério.

### Quantos avisos são inúteis, e o que não resolve

A primeira medição usou o critério errado: contava como "tarde" todo aviso cujo EPISÓDIO
já tinha fechado na chegada. Só que um episódio fecha quando o carro volta à pista — e um
carro voltando da grama, lento, na trajetória, é exatamente o perigo que o aviso
descreveu. Aquele número (33%) media fechamento de registro, não utilidade.

O critério certo é: **quando o piloto passou pelo ponto, ainda havia um problema ali?**
Ou seja, o carro ainda estava fora da pista, ou de volta à pista abaixo de 70% do próprio
ritmo. Guinchado e no pit road contam como não-problema — o carro saiu do caminho.

Sobre as **duas** corridas (Lime Rock e Okayama), 40 carros simulados como jogador em cada:

| Família | Avisos | Inúteis | |
|---|---|---|---|
| Fora da pista | 40 | 14 | **35%** |
| Parado | 54 | 8 | **15%** |

**O carro do jogador real conta como obstáculo.** Ele é um carro na pista se comportando
mal, e em Okayama isso é literal: rodou a 22 km/h por quase quatro minutos, o que é o
próprio cenário do acidente documentado adiante. Mas a assimetria precisa ficar à vista —
**16 dos 25 avisos "fora da pista" de Okayama são sobre esse único carro**, e 3 dos 54
"parado". Sem ele: 24 e 51.

> A tabela publicada antes trazia 51 avisos "parado" e 16% de inúteis. Esse par foi medido
> **sem** o carro do jogador, enquanto os 40 de "fora da pista" foram medidos **com** —
> duas regras diferentes na mesma tabela. Não existe critério uniforme que reproduza os
> dois números antigos ao mesmo tempo. O padrão agora é o fiel ao Rust (o detector só
> ignora o próprio jogador), e `scripts/analise-spotter.mjs --sem-jogador-real` dá a outra
> leitura quando ela for a pergunta certa.

**O piso de permanência não ajuda.** Testado a 0,5 s, 1,0 s e 1,5 s, ele derruba de 40
para 27 avisos e a taxa de inúteis fica igual ou pior (41%, 42%). Um teto de velocidade
do obstáculo no instante do aviso (100, 70, 50, 30 km/h) também não ajuda — a 50 km/h
piora para 71%.

Fica registrado que a recomendação anterior de um piso de 0,5 s **não se sustentou** com a
segunda corrida. A especificação aprovada estava certa em não tê-lo, e ainda bem que não
foi aplicado.

Os 35% da família "fora da pista" são o preço real dela: uma escapada dura o que quiser
durar, e nenhuma leitura do instante do aviso prevê isso. Os 15% da família "parado" são
outra coisa — um carro parado tende a continuar parado.

### A régua é normativa, e mora no arreio

A definição acima ("fora da pista, ou de volta à pista abaixo de **70% do próprio ritmo**;
guincho e box contam como não-problema e **ficam no denominador**") é a única válida para
comparar famílias entre si. A implementação de referência é
[`scripts/analise-spotter.mjs`](../scripts/analise-spotter.mjs) — se a prosa e o arreio
divergirem, o arreio ganha.

Isso está escrito porque já custou uma comparação errada: a família "lento" foi medida com
0,85 no lugar de 0,70 e com os avisos que nunca chegaram fora do denominador, o que deu 17%
e a fez parecer melhor que as duas existentes. Na régua desta seção ela dá **39%**.

### O que a régua NÃO arbitra

Ela funciona para perigo de **estado binário** — fora da pista, parado. O estado persiste
ou não persiste, e o instante do contato o mede bem.

Ela não funciona para perigo de **grandeza contínua**, e a família "lento" é a prova. Medido
ao longo de toda a aproximação: **zero** dos 67 avisos tem um alvo que nunca esteve abaixo
de 0,60 do ritmo do campo entre o aviso e a chegada — a razão mínima do alvo tem mediana de
0,29. Não existe caso de "não havia nada ali". O que existe é perigo que se dissolve na
própria chegada.

Consequência: como o gatilho e a régua vivem no mesmo eixo, escolher o ponto de avaliação é
escolher outro ponto da mesma reta, e a taxa varia monotonicamente com ele (8% a 0,95, 32% a
0,70, 38% a 0,50; e 17% a 5 m do contato contra 5% a 50 m). O critério alternativo — "houve
perigo durante a aproximação" — é degenerado do outro lado: reprova nada, 0%. Os dois
extremos são inúteis como discriminador.

O que arbitraria é se o piloto teve de levantar ou desviar, e **a captura não responde**:
não há entrada de piloto para carros de IA, e desvio de IA não se distingue de traçado.
Para uma família assim, o número que decide não é a utilidade medida — é se ela dispara em
corrida verde.

## A família `Parado` — calibrada, ainda sem áudio

Okayama trouxe os quatro primeiros casos positivos do acervo:

| Carro | t | Duração | Desfecho |
|---|---|---|---|
| `#14` | 918,8 s | **19,70 s** | retomou, voltou a 155 km/h |
| `#21` | 180,6 s | **7,98 s** | retomou sozinho |
| `#25` | 180,9 s | **4,65 s** | sumiu do array → guincho |
| `#20` | 181,1 s | **4,18 s** | sumiu do array → guincho |

A pergunta que o doc deixou aberta — *por quanto tempo precisa ficar abaixo de 5 km/h?* —
tem resposta: **nenhum piso é necessário**. As quatro paradas duraram entre 4,2 e 19,7 s,
e em 34 minutos de corrida não houve **um único** episódio curto de ruído. A regra "estava
andando" já elimina tudo que seria falso positivo; não sobra nada para um piso cortar.

Nos quatro casos o `rpm` fica cravado em **1000** — marcha lenta, motor vivo. A aposta do
doc de que o `rpm` separaria "vai voltar" de "vai ficar" continua de pé, mas segue sem
prova do outro lado: não há um caso de `rpm = 0` no acervo. E os dois que foram guinchados
tinham o motor ligado igual aos dois que voltaram, então até aqui o `rpm` não previu nada.

A família continua **sem áudio** — não porque falte calibração, mas porque não há
gravação. Pelos números acima ela é a mais confiável das duas.

O perseguidor é medido **entre carros**, não a partir do jogador: numa captura com o
jogador parado no box — que é como se grava sem gerar incidente — tudo medido a partir
dele seria zero.

## O guincho é a ausência, não o `NotInWorld`

A premissa de que um carro rebocado apareceria com `CarIdxTrackSurface == -1` está errada.
Em Okayama, com dois carros guinchados, **`-1` não aparece uma única vez na corrida
inteira**. O que acontece é isto:

> carro parado → **some do array `cars[]`** por ~145 s → reaparece com `on_pit_road = true`

O array encolhe de verdade — o índice deixa de existir, não vem zerado.

Isso é um furo em qualquer laço que só visite os carros presentes. O detector tinha esse
furo: um episódio aberto num carro guinchado nunca fechava, virava um obstáculo eterno, e
a duração registrada passava a contar o tempo de ausência. Para o `#25`, 150 s em vez de
4,7 — falso por trinta vezes, e a duração é justamente o número que este módulo existe
para medir. Corrigido com uma varredura de ausentes: o episódio fecha como
`SumiuDoMundo`, com a duração contada **até a última vez em que o carro foi visto**.

O jogador é um caso diferente e não serve de modelo: ele teleportou **dentro** de `cars[]`
(462 m em 0,067 s, `FORA` → `CAIXA`), e só ele tem `tow_time`. São dois mecanismos, e só
um é observável para a IA.

## Um obstáculo de volta 1 é inavisável

O rastro de perseguidores do `#21` parado, na primeira volta com o campo ainda em trem:

> `#22` a 5 m · `#10` a 1 m · `#3` a 2 m · `#7` a 1 m · `#4` a 1 m · `#5` a 1 m ·
> `#11` a 1 m · `#9` a 1 m · `#6` a 2 m · `#13` a 6 m — e o seguinte, `#38`, a **788 m**

O campo inteiro passou entre 1 e 7 metros de um carro parado atravessado. Não existe um
único carro na faixa útil de 100–200 m. Numa parada em trem de volta 1 o spotter tem
**zero** avisos possíveis: ou o carro já está em cima, ou está uma reta inteira atrás.

## O piso do "lento", de graça

A amarela de Okayama entregou a referência que faltava para o terceiro detector. A IA
obedece de verdade:

| | p10 | mediana | p90 |
|---|---|---|---|
| Verde | 75 | **111** | 161 km/h |
| Amarela | 56 | **78** | 122 km/h |

A mediana cai 30%. Abaixo de ~78 km/h em Okayama já é mais lento que ritmo de amarela —
um piso medido, não arbitrado, para quando o detector de "carro lento" for montado.

Duas observações que não são do spotter mas saíram daí: a primeira amarela durou 350 s de
uma prova de 1047 s (um terço), e a segunda subiu aos 920,4 s e **nunca limpou** — a
corrida terminou sob amarela.

## O comentário do engenheiro

A ideia de, depois da passagem, o rádio dizer "Silva ficou quatro segundos fora, a
diferença caiu" pertence ao **engenheiro**, não ao spotter: o spotter informa o perigo
imediato, o engenheiro interpreta o que aconteceu. O que este módulo entrega para isso é
o dado, e ele já está no [`Episodio`](../src-tauri/src/iracing_sdk/spotter_frente.rs):
duração, desfecho, posição do carro e do jogador no início e no fim, e o gap com sinal
nos dois instantes.

O gap nos dois instantes é o campo que importa, e é uma distinção que quase se perde:

> **Ficar 4 s fora da pista não é perder 4 s.**

Dá para passar quatro segundos com duas rodas na grama e perder meio segundo. A duração
descreve o evento; só a variação do gap descreve a consequência. Um rádio que disser
"perdeu quatro segundos" a partir da duração vai estar errado quase sempre — e errado
por muito: nos dados, uma escapada de 5 s a 100 km/h custa 139 m, que a 200 km/h valem
2,5 s. Metade do que a duração sugere.

A fala em si (redações, montagem por peças, regra de relevância) ainda não existe.

## O que ainda falta

- **Brita: existe, e são três amostras.** `#14`, 0,13 s, `brita6`. Okayama tem caixa de
  brita e o `CarIdxTrackSurfaceMaterial` a lê corretamente — o canal está **provado**. Mas
  três amostras de uma roçada não testam poder de discriminação nenhum. A lacuna saiu de
  "não sei se o canal funciona" para "funciona, falta um caso". Todo o resto fora da pista
  nas duas corridas foi grama.
- **O arrasto lento para o box não está em nenhuma gravação.** Nas duas corridas, ninguém
  entrou no pit road devagar por avaria: as únicas entradas tardias foram o jogador
  guinchado e uma parada normal a 98% do ritmo do campo. O carro atingido (`#14`) parou
  22 s e voltou a correr — `on_pit_road` nunca foi verdadeiro para ele. O comportamento é
  real (é o que o nosso `!black` produz), mas segue sem um caso medido.
- **`rpm = 0` não apareceu.** Os quatro carros parados tinham o motor em marcha lenta, e
  dois deles foram guinchados assim mesmo. Sem um caso de motor morto, a hipótese de que o
  `rpm` distingue "vai voltar" de "vai ficar" continua sem teste — e, pelos quatro casos
  que temos, ela não previu nada.
- **Carro lento**: o piso agora existe (mediana de amarela em Okayama), o detector não.

### Regressões contra a captura de Lime Rock

Okayama piorou em três medidas, e vale acompanhar se é a pista ou o gravador:

| | Lime Rock | Okayama |
|---|---|---|
| Saltos inexplicados de distância | 0 | **1** (20,7 m em 0,067 s, `idx 8`) |
| `est_time` na ordem de `lap_dist_pct` | 100% | **91,7%** |
| `f2_time` zerado | 58% | 23% |

A primeira gravação de Okayama (`race_1785887784`) está truncada em 243 s, sem nenhum
episódio em corrida — o app fechou antes. Não serve; descarte.

Nota de custo: 19 MB para 17 min ≈ 1,1 MB/min. Uma corrida de 40 min dá ~45 MB, e com
`MAX_CAPTURAS = 10` a pasta de depuração chega a meio giga.

## Ordem de confiança dos detectores

Medida, não opinada:

A ordem inverteu depois de Okayama. A família que parecia a mais sólida é a menos:

1. **Parado** — 54 avisos simulados em duas corridas, **15% inúteis**. Quatro casos reais,
   de 4,2 a 19,7 s. Um carro parado tende a continuar parado, e é isso que faz o aviso
   valer. **Ganhou áudio** (`carro_parado_frente`): a família nasceu só como observação
   porque Lime Rock não teve um caso, e a pergunta aberta era o piso de permanência.
   Okayama respondeu — quatro casos, nenhum curto, piso nenhum necessário.
2. **Fora da pista** — 40 avisos, **35% inúteis**. `CarIdxTrackSurface == 0` mais perda de
   ritmo, senão dispara em limite de pista a 200 km/h. O problema que sobra não é de
   detecção e sim de previsão: uma escapada dura o que quiser durar, e nenhum piso ou teto
   testado melhora isso.
3. **Trás** — `spotter_tras.rs`, estado sustentado com duas portas (ritmo e bandeira azul).
   **1 estado em 82 pilotos-corrida**, 22,3 s, 3 falas. É o acidente de Okayama, e o estado
   abre **11,95 s antes do impacto** — 6,1 s antes de a azul acender. A calibração repousa
   sobre um caso, o que é pouco; mas a ablação da supressão de amarela (1 → 14 estados) e o
   volume de 0,07 fala por piloto dizem que ele não é ruidoso. **Entra.**
4. **Lento** — `spotter_lento.rs`, construído e calibrado, **não fiado**. Na régua desta
   página dá **39% de inúteis** — pior que "fora da pista". Mas o que o desqualifica não é
   isso: em Lime Rock, 17 minutos limpos com 40 carros, ele **não falou uma vez**, e em
   Okayama **100% dos avisos saíram sob amarela** quando o carro do humano sai da conta.
   Uma família que só dispara sob amarela não pode ser resolvida com supressão de amarela —
   ou entra dizendo o que a própria bandeira já diz, ou não entra. Fica no repositório,
   compilado e testado, até haver corrida verde com caso.
5. **Incidente genérico** — não é medida, é o balde de baixa confiança dos acima. O SDK
   não expõe incidente de outro carro.

## O diário: medir o que NÃO saiu

Adicionado em 16/08/2026, junto de `scripts/spotter-tracker.mjs`.

Toda a régua acima saiu de **captura reprocessada**: rodar o detector de novo, fora do jogo, sobre
`race_*.jsonl.gz`. É o método certo para calibrar, e ele tem um ponto cego que só aparece na
corrida de verdade: o detector reprocessado não conhece a arbitragem do tique, a fila de
prioridade nem a camada de voz. Ele diz o que a regra faria; não diz o que o jogador ouviu.

`iracing_sdk/spotter_diario.rs` fecha essa distância gravando, **em corrida**, o que cada detector
recusou e por quê. O vocabulário de motivos hoje:

| Família | Motivos |
|---|---|
| todas | `perdeu_o_tique` (com quem ganhou) |
| `frente` | `jogador_fora_da_pista`, `sessao_nao_e_corrida`, `jogador_parado`, `ja_avisado`, `longe`, `cedo`, `tarde` |
| `tras` | `campo_sem_ritmo`, `sem_perseguidor`, `ritmo_ok`, `saida_de_box` |
| `boxe` | `perto`, `longe`, `sem_diferenca`, `sem_fechamento`, `cedo` |

Toda recusa que compara contra um limiar carrega a **folga**: quanto faltou. É essa coluna que
responde a pergunta de calibração sem rodar nada de novo, e é o que separa um detector saudável
(recusas a centenas de metros do corte) de um limiar mal posto (recusas a 2 m dele).

> **A folga é a MENOR do episódio, e a primeira versão errou isso.** Ela fechava a linha na
> transição do motivo e registrava a folga daquele instante. O motivo muda justamente quando um
> limiar é cruzado, então a folga saía perto de zero por construção, mesmo para um candidato que
> passou longe de disparar: na corrida de 17/08/2026 as recusas de `boxe/perto` saíram todas com
> folga de 2 a 20 cm, e nenhuma delas queria dizer nada. Hoje a unidade do arquivo é o episódio
> (mesmo candidato, mesmo motivo, do primeiro tique ao último), e o que ele carrega é o instante
> em que a recusa chegou mais perto de virar fala, com `durou_s` e `tiques` ao lado. Uma folga de
> 0,15 km/h que durou quatro segundos é notícia; a mesma folga num tique é ruído de amostragem.

As famílias `voltar`, `bandeira` e `clima` ficaram só com `perdeu_o_tique`. Os portões delas são
bits que a captura já grava (`session_flags`, `track_wetness`), então o tracker os responde da
captura sem instrumentação nova.

> Armadilha registrada na construção: a detecção de salto de sessão do diário mediu, na primeira
> versão, o intervalo entre duas NOTAS. As notas são esparsas por construção, então duas recusas
> legítimas a dez segundos de distância pareciam replay, o dedup era limpo e a mesma recusa entrava
> duas vezes no arquivo. O relógio do salto é o do tique, sempre.
