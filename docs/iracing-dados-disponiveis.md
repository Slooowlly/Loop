# O que o iRacing nos dá — mapa dos dados de pista

Este documento responde uma pergunta que aparece toda vez que alguém desenha uma feature em
cima da telemetria: **isso a gente tem?**

Ele é um mapa, não um tutorial. Está organizado em camadas — do que o SDK entrega cru até o
que o app calcula em cima — porque é assim que a dúvida costuma chegar: alguém quer saber se
dá para dizer "o pneu dele é quinze voltas mais velho", e a resposta depende de três camadas
diferentes, sendo que a primeira delas não tem o dado.

As duas seções mais importantes são as **últimas**: [o que não temos](#5-o-que-não-temos) e
[as armadilhas medidas](#6-armadilhas-medidas). Elas custaram captura real e cada uma já mordeu
pelo menos uma vez.

Complementos: [iracing-escopo.md](iracing-escopo.md) (por que o iRacing é o centro do jogo),
[spotter-obstaculo.md](spotter-obstaculo.md) (a dedução de obstáculo à frente),
[tts-poc-latencia.md](tts-poc-latencia.md) (a voz que consome tudo isto).

---

## 1. As duas torneiras

Tudo entra por **duas** fontes, lidas pelo mesmo amostrador de fundo em
[`race_monitor/amostrador.rs`](../src-tauri/src/iracing_sdk/race_monitor/amostrador.rs):

| fonte | o quê | cadência |
|---|---|---|
| **Memória compartilhada** | telemetria numérica (`IracingTelemetry` + array `cars[]`) | **60 Hz** conectado, 1 Hz ocioso |
| **YAML de sessão** | identidade: pista, pilotos, classes, números, **resultados oficiais** | a cada N ticks (muda raro) |

O 60 Hz não é capricho. O spotter lateral e a pontuação de batida precisam dele: um carro que
entra e sai do seu lado numa freada simplesmente não teria acontecido a 2 Hz, e o pico de um
impacto se perde entre dois quadros.

Nem todo consumidor precisa dessa resolução, e os que não precisam são estrangulados na
origem — o [`EstadoAgora`](#41-ao-vivo--estadoagora) recolhe a 4 Hz, porque a fala mais curta
do engenheiro leva meio segundo e clonar o vetor de carros sessenta vezes por segundo seria
pagar caro por uma resolução que ninguém ouve.

Em paralelo, o [`race_capture`](../src-tauri/src/iracing_sdk/race_capture.rs) **grava sempre**:
todo frame cru vai para um JSONL comprimido, ~0,8 MB por corrida, 23,7 µs por frame (0,14% de
um núcleo). É a fonte-verdade para rebalancear o app com dado real de pista, e é o que sai
junto quando o jogador manda o log pelo botão de diagnóstico. Uma captura que só liga quando
alguém lembra de ligar nunca pega o bug que aconteceu ontem no PC do jogador.

---

## 2. Camada crua — o que o SDK entrega

### 2.1 Escalares do jogador

De [`IracingTelemetry`](../src-tauri/src/iracing_sdk/tipos.rs). São ~60 campos; o SDK expõe
centenas de outros que não lemos.

| grupo | campos |
|---|---|
| **Pilotagem** | velocidade (m/s e km/h), RPM, marcha, acelerador, freio, embreagem, ângulo de volante |
| **Volta** | volta atual, `lap_dist_pct` (0–1), tempo da volta em curso, última volta completa |
| **Sessão** | `session_time`, `session_tick`, estado (1 GetInCar … 4 Racing … 6 CoolDown), número da sessão, tempo total, tempo restante, voltas restantes/totais, `PaceMode`, boxes abertos |
| **Posição do carro** | posição na sessão, superfície (−1 fora do mundo, 0 fora da pista, 1 no box, 2 chegando, 3 na pista), na garagem, no pit road |
| **Física de impacto** | aceleração lateral / longitudinal / **vertical**, taxa de guinada, rolagem, arfagem |
| **Disciplina** | `session_flags` (bitfield), pontos de incidente (jogador / piloto / equipe), tempo de reboque |
| **Dano** | segundos de reparo **obrigatório** e **opcional** — mas **só durante o serviço na caixa**: medido (2026-08-10), um carro destruído rodando/rebocado/no grid lê 0.0 nos dois canais, com meatball na tela. Não servem para detectar dano ao vivo; a severidade da batida (G + velocidade perdida) é quem mede isso |
| **Combustível** | litros no tanque |
| **Clima** | `TrackWetness` (0–7), precipitação **agora**, declarada molhada, temperatura do ar / da pista / medida pela equipe, umidade relativa, vento (velocidade e direção), céu, neblina |
| **Vizinhança** | `CarLeftRight` (0 Off, 1 Livre, 2 Esq, 3 Dir, 4 três largos, 5 duas esq, 6 duas dir) |
| **Replay** | número do frame, sessão e tempo do replay, câmera |

Duas observações que economizam tempo depois:

**`session_time` não é monotônico de verdade.** Ele volta para perto de zero quando a corrida
reinicia, e salta quando alguém mexe no replay. O `session_tick` é o relógio imune a isso — é
por ele que se prova que um frame não foi pulado.

**Os campos de replay não alimentam detector nenhum, e é de propósito.** Eles são o par
olho-a-olho do dado: uma captura que diz *"o carro 7 ficou a 2 km/h por 1,8 s"* não distingue
batida de engarrafamento. Com o número do frame do replay, distingue — basta olhar. Custam
alguns bytes por frame.

### 2.2 Por carro

De [`CarSnapshot`](../src-tauri/src/iracing_sdk/tipos.rs), até 64 entradas, lidas das
variáveis de array `CarIdx*`. **Só os carros presentes no mundo entram.**

| grupo | campos |
|---|---|
| **Onde está** | `lap_dist_pct`, volta, voltas completas, **`est_time`** (segundos desde a linha até o ponto onde ele está) |
| **Classificação** | posição geral, posição na classe |
| **Estado** | superfície, **material da superfície** (asfalto, concreto, grama, terra, areia, brita), no pit road, marcha, RPM, volante |
| **Ritmo** | última volta, melhor volta, em que volta fez a melhor |
| **Disputa** | `f2_time` (gap ao líder — **não usar**, ver §6), `tire_compound` |
| **Disciplina** | bandeiras daquele carro, reparos rápidos usados |
| **Formação** | fila, linha, flags de pace |

O **material da superfície** é o canal que separa situações que o `track_surface` mistura.
"Fora da pista" sozinho é ambíguo: pisar a zebra numa área asfaltada e enterrar o carro na
brita dão o mesmo `OffTrack`, e só um dos dois vira um obstáculo parado no caminho de quem vem
atrás. Sem o material, ou o aviso dispara em todo limite de pista da corrida, ou é calibrado
tão alto que perde o acidente.

O **`est_time`** é o campo mais importante desta tabela e o menos óbvio. Ele é a base de todo
gap na pista — ver §6 para o motivo de o campo que *parece* ser o gap não servir.

### 2.3 YAML de sessão

| parser | entrega |
|---|---|
| `parse_driver_names` | **nome de cada piloto** por `car_idx` |
| `parse_car_numbers` | número do carro — a ponte para o `driver_id` da carreira |
| `parse_driver_classes` | é IA? é pace car? qual classe? |
| `parse_class_names` | nome curto de cada classe |
| `parse_track_id`, `parse_track_length_m` | pista e **comprimento** (sem ele, `lap_dist_pct` não vira metro nenhum) |
| `parse_subsession_id` | identidade única do evento |
| `parse_player_car_name`, `parse_car_redline` | o carro do jogador |
| `parse_qualy_session_num`, `parse_race_session_num` | qual sessão é qual |
| [`session_results.rs`](../src-tauri/src/iracing_sdk/session_results.rs) | **os resultados oficiais** |

Os resultados oficiais merecem destaque: **o iRacing roda a própria corrida e a classificação
até o fim, independente do que o jogador faça.** Se ele bate na volta 2 e sai, o resultado
final ainda existe no YAML — com posição de cada carro e motivo de abandono. É o que permite a
carreira continuar coerente depois de um DNF.

---

## 3. Camada acumulada — o que o app guarda ao longo da corrida

`RaceHistory`, montado ao vivo em
[`race_monitor/historico.rs`](../src-tauri/src/iracing_sdk/race_monitor/historico.rs). Isto
não vem do SDK: é o app assistindo à corrida e anotando.

| o quê | detalhe |
|---|---|
| **Race trace** | snapshot de todos os carros a cada virada de volta do líder **e a cada troca de posição** — a ultrapassagem aparece no ponto exato em que aconteceu, não só na virada |
| **Voltas do jogador** | tempo + **combustível restante ao completar**. A diferença entre voltas dá o consumo |
| **Voltas de todos os carros** | base do ritmo comparado e da adaptação de dificuldade |
| **Parciais por setor** | pista dividida em 3 por `lap_dist_pct` |
| **A batalha** | quem estava à frente e atrás, com os gaps, amostrado a ~1 Hz a corrida toda |
| **Paradas de box** | de **todos** os carros, com o **tempo parado na caixa** e se a pista estava molhada naquele instante |
| **Voltas de amarela** | as faixas do gráfico |
| **Incidentes do jogador** | volta fracionária + pontos (1 saída, 2 rodada, 4 contato) + se saiu da pista |
| **Voltas de quali** | capturadas à parte, sem contaminar o histórico da corrida |
| **Tentativas** | cada restart abre uma tentativa, com batidas pontuadas, evidências e desfecho |

Mais o anel de **amostras de gap** a 4 Hz, com janela de 60 s, que sustenta a tendência
("ele está vindo" vs "está indo embora").

O **tempo parado na caixa** é o campo que mais rende por byte guardado. Ele é a única porta de
entrada para estratégia de pneu — ver §4.1.

---

## 4. Camada calculada

### 4.1 Ao vivo — `EstadoAgora`

De [`race_monitor/estado_agora.rs`](../src-tauri/src/iracing_sdk/race_monitor/estado_agora.rs).
A regra da camada é: **aqui se calcula, lá fora só se redige.**

| calculado | como |
|---|---|
| **Gap ao vizinho** | diferença de `est_time`, **fechando o círculo da volta** — quem está à frente pode já ter cruzado a linha |
| **Tendência do gap** | inclinação sobre a janela de 60 s, convertida para segundos **por volta** |
| **Voltas para alcançar** | gap ÷ diferença de ritmo. −1 quando a distância aumenta, porque "nunca" não é um número |
| **Voltas restantes** | do sim em prova por voltas; **estimadas** por tempo ÷ ritmo em prova por tempo, com a marca de estimativa viajando junto |
| **Consumo e autonomia** | do histórico de tanque por volta |
| **Saldo de combustível** | autonomia − voltas restantes. É a única forma útil do dado: "dá para 14 voltas" não responde nada sem saber quantas faltam |
| **Idade do pneu** | voltas desde a última parada com **mais de 20 s parado na caixa** |
| **Composto** | índice → seco/chuva |
| **Bandeira legível** | bitfield → rótulo, pela **mais grave** (uma preta com uma amarela é uma preta) |

A **idade do pneu** é o melhor exemplo de camada calculada compensando ausência de canal. O
SDK não expõe desgaste. O que ele expõe é quanto tempo cada carro ficou parado no box, e
trocar pneu é um bloco fixo de ~21,5 s **por cima** do abastecimento — medido, não estimado.
Acima de 20 s a parada trocou pneu; abaixo, só abasteceu. O limiar fica entre o maior
abastecimento sozinho observado (~19 s) e o serviço mínimo de pneu (~21 s).

### 4.2 Situacional — os quatro spotters

| módulo | deduz |
|---|---|
| [`spotter.rs`](../src-tauri/src/iracing_sdk/spotter.rs) | vizinhança lateral, com **histerese** |
| [`spotter_frente.rs`](../src-tauri/src/iracing_sdk/spotter_frente.rs) | obstáculo à frente — carro fora da pista ou parado |
| [`spotter_lento.rs`](../src-tauri/src/iracing_sdk/spotter_lento.rs) | carro muito mais devagar que o resto, ainda andando |
| [`spotter_tras.rs`](../src-tauri/src/iracing_sdk/spotter_tras.rs) | carro chegando por trás |

**Só o lateral vem pronto do SDK**, e mesmo ele não vem utilizável: o `CarLeftRight` é cru e
nervoso, e na borda da zona de detecção pisca de quadro em quadro. A 60 Hz isso viraria um
spotter gago dizendo "esquerda, livre, esquerda, livre". Todo o valor daquele módulo está em
confirmar antes de falar.

Os outros três são inferência sobre posição, superfície e velocidade — o SDK não diz "há um
carro fora da pista à sua frente".

### 4.3 Pós-corrida — `telemetry_analysis`

De [`telemetry_analysis/`](../src-tauri/src/iracing_sdk/telemetry_analysis.rs): ritmo limpo
(voltas dentro de 4% da melhor), consistência, **você vs. a média do campo**, rival mais
disputado, fluxo de posições na pista, erro mais caro, melhor momento, estratégia de pneu de
todos os carros, setor fraco, volta teórica, consumo.

**Tudo isso é lógica pura sobre `RaceHistory`.** Foi escrito para o painel pós-corrida, mas
nada nele exige que a corrida tenha acabado — o `get_history()` devolve o histórico ao vivo a
qualquer momento. Quem quiser esses números durante a prova já pode.

---

## 5. O que **não** temos

Esta seção importa tanto quanto a lista de cima, e é a que evita desenhar uma feature em cima
de um canal que não existe.

| ausente | consequência |
|---|---|
| **Desgaste de pneu** | não há canal. Só dá para inferir por queda de ritmo ao longo do stint |
| **Temperatura e pressão por roda** | idem — nada disso está no frame |
| **Dano por peça** | só os **segundos de reparo** agregados. Qual peça quebrou é modelo nosso, não leitura |
| **Amarela local (setorizada)** | medido em Okayama: numa corrida de 17 min com duas amarelas e 41 carros, o `CarIdxSessionFlags` só assume quatro valores, nenhum deles de amarela. A amarela existe **apenas** no bitfield global |
| **Delta ao vivo** | não há canal de "quanto acima do seu melhor". Teria de ser cronometrado por setor do nosso lado |
| **Combustível dos outros carros** | só o do jogador |

---

## 6. Armadilhas medidas

Todas custaram captura real e todas já morderam.

### `f2_time` não é proximidade

O `CarIdxF2Time` parece ser o gap e não é utilizável como tal. Em corrida de IA ele ora vem 0,
ora vem populado mas **congelado** — só reescrito na passagem pela linha, misturando diferença
de voltas com distância de pista.

Numa captura real, dois carros a **40 s** um do outro tinham `f2_time` a **0,165 s** de
distância. Um zero denunciaria o defeito na primeira olhada; um número plausível não.

**Use `est_time`**, e feche o círculo da volta: quem está à frente pode já ter cruzado a linha,
e aí o `est_time` dele é *menor* que o seu. Sem fechar o círculo, um adversário a três décimos
aparece como uma volta inteira menos três décimos. A função `gap_circular` no `estado_agora.rs`
é a implementação de referência.

### `SessionLapsRemainEx` é sentinelado

Em prova por **tempo** o sim manda **32767** — o sentinela de "ilimitado". Ler isso como voltas
restantes anuncia "faltam trinta e dois mil setecentas e sessenta e sete voltas".

Em prova por tempo, o total de voltas é **sempre estimativa nossa** (tempo restante ÷ ritmo), e
quem for comunicar isso ao jogador tem de dizer "por volta de".

### `position == 0` é sentinela, não posição

Vale a **classificação inteira** até o piloto marcar o primeiro tempo válido — o iRacing só
atribui posição em quali depois que existe volta cronometrada.

E vale **durante a corrida** também, para carro na garagem ou fora do mundo. Medido: com a
busca de vizinho da frente feita por `position == me.position - 1`, **29% dos frames com
vizinho** apontavam para um carro de `position` 0 — o jogador em P1 fazia a conta cair em zero.
Exigir `position >= 1` no candidato conserta.

Para ordem em classificação, derive de `best_lap_time` em vez de confiar em `position`.

### O jogador some de `cars[]`

Sempre que ele não está na pista — garagem, box, depois da bandeirada — **nenhum elemento do
array tem `is_player`**. Na captura de Ledenon foram os frames 3085–5400 (ida ao box na quali)
e do 53168 até o fim.

Tratar a ausência como estado válido, não como erro.

### `tire_compound` vem 0 em carro mono-composto

Medido no MX-5 Cup: 0 para todos os carros em todos os 53 mil frames. O campo **é** lido
corretamente; o SDK é que não popula onde não há escolha a informar. O default do Loop é −1,
então um 0 significa "o iRacing respondeu 0".

**Não está medido em série com pneu de chuva**, que é exatamente onde o campo valeria — e onde
a distinção seco/chuva é o que permite dizer "o da frente ainda está de seco, vai ter que
parar". Fica em aberto.

### A moderação da TTS bloqueia texto inocente

Fora do SDK, mas na mesma cadeia: `Livre.` — palavra portuguesa comum, em contexto de
automobilismo — foi recusada como conteúdo sensível, e de forma **não determinística**: 5
bloqueios e 4 sucessos no mesmo texto. Qualquer gerador de falas em lote precisa de repetição
automática, não de uma lista de palavras proibidas. Detalhes em
[tts-poc-latencia.md](tts-poc-latencia.md).
