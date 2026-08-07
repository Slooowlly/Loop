# Briefing — as frases do pacote de voz

Documento auto-contido: escrito para ser entregue a quem vai **escrever as frases**, sem
precisar conhecer o projeto. Quem já conhece pode pular para [O que eu preciso](#o-que-eu-preciso).

---

## 1. O contexto, em um minuto

**Loop** é um jogo de carreira no automobilismo construído em volta do **iRacing**. O
jogador controla um piloto e corre as etapas **de verdade** dentro do iRacing; o Loop lê a
telemetria ao vivo e devolve a corrida para a carreira.

Enquanto o jogador dirige — de capacete, muitas vezes de VR, sem poder ler nada na tela —
duas vozes falam com ele. Elas já existem como **texto**; o que se está montando agora é o
**áudio pré-gravado** delas.

### As duas vozes

| | **SPOTTER** | **ENGENHEIRO** |
|---|---|---|
| Onde está | Numa torre, olhando a pista | Nos boxes, olhando os dados |
| Do que fala | O que está acontecendo AGORA, ao redor do carro | O carro, a estratégia, os outros pilotos |
| Urgência | Máxima — a informação vale por 2 segundos | Baixa — dá para ouvir com calma |
| Tamanho | 1 a 3 palavras | Uma frase |
| Fala de você? | Raramente. Fala da PISTA | Sim, e fala dos outros pilotos pelo sobrenome |
| Exemplo real | "Esquerda." | "Silva sente o motor perdendo fôlego." |

**A mesma voz grava as duas.** Uma voz masculina, sintética (Google Cloud TTS,
`pt-BR-Chirp3-HD-Algenib`), com um filtro de rádio aplicado depois — soa como transmissão
de rádio de equipe, não como assistente de celular.

---

## 2. As regras que funcionam

Nada aqui é opinião. Cada regra saiu de material gerado e ouvido, e várias custaram uma
tentativa fracassada antes.

### 2.1. Curto ganha, e ganha muito

O aviso do spotter disputa atenção com uma freada. Cada sílaba a mais é atraso, e o aviso
só vale enquanto a situação existe.

- ✅ `"Esquerda."` — 0,73 s
- ✅ `"Livre."` — 0,51 s
- ❌ `"Tem um carro à sua esquerda"` — chega depois de a curva ter acabado

Para o engenheiro a pressa não existe, e frases inteiras são bem-vindas.

### 2.2. A pontuação é o controle de entoação

Não há como dirigir a atuação da voz por instrução — só pela escrita. Medido:

| Escrita | O que a voz faz |
|---|---|
| `Silva.` | Entoação de FIM de frase; alonga a última sílaba |
| `Silva,` | Entoação de CONTINUAÇÃO — pede o que vem depois |
| `está fora — problemas na suspensão` | O travessão vira **0,38 s de silêncio** no meio da fala |
| `Volta em` | O modelo partiu em dois fôlegos, com 0,34 s de buraco |
| `Volta em,` | Um fôlego só |

**Regras práticas:**
- Peça que **começa** uma frase montada termina em **vírgula**.
- Peça que fecha, ou fala inteira, termina em **ponto**.
- **Nunca use travessão (—).** Use vírgula. Se a ideia exige uma pausa, escreva duas frases.
- Evite reticências, parênteses e ponto e vírgula — todos viram pausa imprevisível.

### 2.3. Colar peças funciona — em fronteira de oração

Provado: `[Sobrenome,] + [trecho]` é indistinguível da frase gravada de uma vez.

```
"Silva," + "sente o motor perdendo fôlego."  →  "Silva sente o motor perdendo fôlego."
```

É isso que torna a biblioteca viável: **355 sobrenomes + 105 trechos** em vez de 355 × 105
gravações.

**O que NÃO foi provado, e por isso não se usa:** emenda no meio de uma palavra ou de um
nome próprio (`"Neil" + "Cooper"`), e montagem de número por dígito.

### 2.4. Número por extenso, numa peça só

Testado e decidido para tempo de volta: `"um trinta e dois e quatro."` gravado **inteiro**
venceu a montagem por dígitos por larga margem — o veredito foi "um abismo de melhor". A
versão por dígitos soa robótica e ainda fica ~1 s mais longa.

Também testada e **rejeitada**: a versão qualitativa (`"um trinta e dois baixo"`,
`"um trinta e um alto"`). Não reabrir.

Vale para qualquer número: escreva por extenso, numa peça só.

### 2.5. Três variações por situação

A voz é generativa e não tem semente — a mesma entrada gera áudio um pouco diferente a
cada vez, então não dá para contar com repetição idêntica. E, mais importante: ouvir a
mesma frase pela quinta vez numa corrida é irritante.

O código já segue essa convenção: **3 redações distintas por situação**, escolhidas por
rodízio. Exemplo real, para "motor com problema leve":

```
"sente o motor perdendo fôlego"
"relata o motor engasgando"
"avisa que o motor não está redondo"
```

Não são sinônimos preguiçosos — cada uma tem um ângulo (sintoma, comportamento, juízo).
É o que se espera de todas as famílias marcadas com "3 variações" abaixo.

O rodízio é automático: os arquivos `chave.wav`, `chave_2.wav` e `chave_3.wav` são a
mesma fala, e quem toca alterna entre eles. Escrever a quarta variação de alguma coisa é
só escrever mais uma linha — nada no código precisa saber quantas existem.

### 2.6. Gênero: a voz é masculina, o jogador é desconhecido

A voz é `MALE`. Então:

- ✅ O falante pode se referir a si no masculino — mas evite, não acrescenta nada.
- ❌ **Nunca use adjetivo ou particípio que concorde com o JOGADOR.** Não sabemos o gênero
  dele, e não há como saber.

| Não | Sim | O que era o problema |
|---|---|---|
| "Você está sozinho na pista" | "Pista livre atrás" | `sozinho` |
| "Fique tranquilo" | "Sem pressa" | `tranquilo` |
| "Você foi muito rápido" | "Boa volta" | `rápido` |
| "Está liberado" | "Pode ir" | `liberado` |

Substantivo e advérbio não têm esse problema — é sempre possível reescrever.

O código já resolve isso hoje escrevendo na 1ª pessoa do engenheiro sem adjetivos:
`"Estou ouvindo algo estranho no seu motor"`.

### 2.7. Maiúscula inicial, sempre

As frases também aparecem como texto no overlay. Toda string que o jogador lê começa em
maiúscula. Exceção: os **trechos de cauda**, que por definição vêm depois de um nome e
seguem em minúscula (`"sente o motor perdendo fôlego"`).

### 2.8. Termo em inglês se escreve pelo SOM, não pela grafia

O automobilismo é cheio de termo em inglês que o piloto brasileiro usa e reconhece —
*three wide*, *box*, *undercut*. Escrevê-los como se escreve em inglês **não funciona**:
a voz é pt-BR e tropeça no `th`, nos ditongos e nas consoantes finais.

Medido com "three wide", o aviso de quando há um carro de cada lado:

| Escrita | Resultado |
|---|---|
| `Three wide.` | ❌ ruim |
| `Three wide, cuidado.` | ❌ ruim |
| `Thri uaid, cuidado.` | ❌ ruim |
| `Tri uáide, cuidado.` | ✅ ótimo |
| `Thríuaid, cuidado.` | ✅ ótimo |

A regra: **escreva como um brasileiro leria em voz alta**, com acento gráfico onde a
tônica cair. É o mesmo remédio que os sobrenomes estrangeiros vão precisar (Prioridade 3).

E vale ter sempre uma variação em português puro no rodízio — no caso acima,
`"Carro dos dois lados."`. Se um dia a voz mudar e o truque fonético parar de colar,
ainda sobra um aviso inteligível.

### 2.9. Nada de gíria de época, nem de nome de pista

O pacote é gravado uma vez e vale para sempre. Gíria envelhece, e nome de pista
multiplicaria a biblioteca por 40.

---

## 3. As condições — o que o jogo sabe

Esta é a parte que mais derruba boas ideias. **Só dá para falar do que a telemetria
entrega.** Tudo abaixo foi verificado no código.

### 3.1. O que o jogo SABE (60 vezes por segundo)

**Ao redor do carro**
- Quem está do lado: nada, um à esquerda, um à direita, um de cada lado, dois à esquerda, dois à direita
- Onde cada carro está na volta (fração e tempo estimado desde a linha) → distância para quem está à frente e atrás
- Quem está no pit lane; quem está fora da pista ou na brita

**A corrida**
- Posição do jogador, na geral e na classe; posição de todos os outros
- Bandeiras da sessão; estado da sessão (aquecimento, volta de apresentação, corrida, bandeirada)
- Volta atual e voltas completas
- Tempo de sessão restante
- Última volta e melhor volta de **cada** carro

**O carro do jogador (só o dele)**
- Combustível, contador de incidentes, segundos de reparo pendentes
- Velocidade, marcha, RPM, acelerador, freio, embreagem, ângulo de volante
- Acelerações e taxas de guinada/rolagem/arfagem — dá para detectar batida e rodada

**O ambiente**
- Molhado da pista, em 8 níveis (de seco a encharcado)
- Temperatura do ar e da pista, vento, umidade

### 3.2. O que o jogo NÃO SABE

Frases que dependam de qualquer coisa desta lista são impossíveis. Não há contorno.

- ❌ **Pneu.** Composto, desgaste, temperatura, pressão — de ninguém, nem do jogador. O
  canal existe no SDK e vem **sempre zerado**. Nada de "seus pneus estão acabando".
- ❌ **Dano dos outros carros.** Só o do jogador.
- ❌ **Combustível dos outros.**
- ❌ **Intenção da IA.** Não dá para saber que "ele vai parar na próxima".
- ❌ **Voltas restantes em prova por TEMPO.** O simulador manda um valor sentinela
  (32767) em vez do número. Só dá para falar em voltas restantes quando a prova é POR
  VOLTAS; em prova por tempo, fale em minutos.
- ❌ **Posição durante a classificação.** Vem zerada — não dá para anunciar posição na quali.
- ❌ **Rádio da direção de prova**, penalidades por texto, avisos oficiais.

### 3.3. Duas armadilhas de redação

1. **O jogador some da telemetria quando sai da pista.** Frases que dependem de "onde você
   está" precisam funcionar sem esse dado por alguns instantes.
2. **"Livre" é a frase mais perigosa do pacote.** Se sair cedo, o jogador fecha a porta em
   cima de quem ainda estava lá. A redação tem que ser inequívoca — nada de "acho que deu",
   "parece livre".

---

## 4. O que eu preciso

Ordem por prioridade. Os números são quantas frases cada família precisa.

### JÁ PRONTO — serve de exemplo do padrão

**Spotter, proximidade (20 gravações).** Já gravado e testado em pista. Vale estudar a
estrutura: toda família de situação contínua vai precisar das mesmas **três fases**.

**Fase 1 — ENTRADA.** Alguém chegou. É a fala mais prioritária do sistema.

```
esquerda         "Esquerda."
direita          "Direita."
duas_esquerda    "Dois à esquerda."
duas_direita     "Dois à direita."
tres_largos      "Tri uáide, cuidado."        ← um de cada lado. Ver a regra 2.8:
tres_largos_2    "Thríuaid, cuidado."            "three wide" escrito pelo som
tres_largos_3    "Carro dos dois lados."
```

**Fase 2 — PERMANÊNCIA.** Ele continua ali. Repete a cada 3 s, com a espera crescendo
1 s a cada repetição até parar em 6 s — presente sem virar metrônomo. É onde as 3
variações mais importam, porque é a única fala que se ouve quatro vezes seguidas.

```
ainda_esquerda   "Ainda à esquerda."
ainda_esquerda_2 "Continua à esquerda."
ainda_esquerda_3 "Ele segue à esquerda."
ainda_direita    "Ainda à direita."
ainda_direita_2  "Continua à direita."
ainda_direita_3  "Ele segue à direita."
ainda_ai         "Ainda aí."                  ← os DOIS lados: não há lado a nomear
ainda_ai_2       "Segura, segura."
ainda_ai_3       "Mantém a posição."
```

**Fase 3 — LIBERAÇÃO.** Abriu. **Sem variação, de propósito:** é a fala mais perigosa do
pacote — entendida errado, o piloto fecha a porta em cima de alguém — e três redações
são três chances de confundir.

```
livre_esquerda   "Livre à esquerda."          ← abriu um lado, o outro segue ocupado
livre_direita    "Livre à direita."
livre            "Livre."                     ← não há mais ninguém, dos dois lados
teste            "Spotter na escuta."         ← ao sentar no carro, antes da largada
```

Três coisas para copiar daqui:

- **Toda situação que DURA precisa das três fases.** Só a entrada não basta: anunciar
  uma vez e calar deixa o piloto sem saber se o outro saiu ou se o spotter parou de
  olhar.
- **A liberação nomeia O LADO.** Um `"Livre."` solto depois de três largos é ambíguo
  justamente no instante em que o piloto decide o movimento.
- **`"Segura, segura."`** A ideia original era `"Segura…"`. Reticências viram pausa
  imprevisível (regra 2.2); dobrar a palavra dá a mesma insistência sem pontuação. E
  `"Segura, à esquerda."` teve de ser reescrito como `"Ele segue à esquerda."` — aquela
  vírgula abriu 0,27 s de buraco no meio da fala.

---

### PRIORIDADE 1 — Rádio do engenheiro (105 peças)

Já existe redação para todas, escrita **para ser lida**. Preciso de uma **reescrita para o
ouvido**: mesma quantidade de espaços, mesma informação, frases que soem em voz alta.

**1.1. Quebra de peça — 72 trechos de CAUDA** (vêm depois de `"Sobrenome,"`, começam em minúscula)

11 peças × 2 gravidades × 3 variações, mais 2 genéricos × 3.

Peças: motor, câmbio, freios, suspensão, arrefecimento, asa dianteira, asa traseira,
laterais, assoalho, chassi, parte elétrica.
Gravidades: **leve** (perdeu ritmo, ainda anda) e **grave** (pode não terminar).
Genéricos: um para "problema grave não identificado", outro para "problema qualquer".

Exemplo do que existe hoje (motor / grave):
```
"está com o motor em pane"
"perdeu potência e o motor pode não aguentar"
"relata o motor no limite, situação séria"
```

**1.2. Abandono — 3 aberturas + 12 peças**

A frase monta assim: `[Sobrenome,] + [abertura] + [peça]`.

Aberturas de hoje (com o travessão que precisa sair):
```
"está fora — problemas {peça}"
"abandona a corrida com problemas {peça}"
"foi retirado da corrida — problemas {peça}"
```

As 12 peças com preposição: `"no motor"`, `"no câmbio"`, `"nos freios"`, `"na suspensão"`,
`"no arrefecimento"`, `"na asa dianteira"`, `"na asa traseira"`, `"nas laterais"`,
`"no assoalho"`, `"no chassi"`, `"na parte elétrica"`, `"no carro"`.

Preciso das 3 aberturas reescritas de modo que a peça encaixe no fim, sem travessão.

**1.3. Aviso pessoal — 3 aberturas + 12 peças + 3 fechos**

Único caso em que o engenheiro fala **com** o jogador (2ª pessoa), então não usa nome.
Monta assim: `[abertura] + [peça "no seu…"] + [fecho]`.

Hoje:
```
abertura: "Estou ouvindo algo estranho" / "Não gostei de um barulho" / "Tem algo esquisito acontecendo"
peça:     "no seu motor" / "no seu câmbio" / "nos seus freios" / … (12, espelhando 1.2)
fecho:    "pode dar problema a qualquer momento" / "fica de olho, pode falhar a qualquer hora" / "risco de pane a qualquer momento"
```

---

### PRIORIDADE 2 — Spotter, o resto do repertório (~150 falas)

Nada disto existe. Todas são **falas inteiras**, curtas, com **3 variações** cada, salvo
onde eu disser o contrário.

| Família | Falas | Quando dispara | Observações |
|---|---|---|---|
| **Bandeiras** | 8 × 3 | mudança de bandeira | verde (largada), verde (relargada), amarela no setor, amarela geral, vermelha, azul (mais rápido chegando), branca (última volta), quadriculada |
| **Posição** | 30 × 1 | mudou de posição | P1 a P30. **Preciso que você decida a forma** e entregue as 30: "Primeiro" / "Posição doze" / outra coisa. Sem variações — é um número |
| **Voltas restantes** | 31 × 1 | virou a volta | "Última volta" + de 2 a 30 voltas. Peça inteira, número por extenso (regra 2.4) |
| **Tempo restante** | 7 × 3 | cruzou a marca | 30, 20, 15, 10, 5, 2 e 1 minuto |
| **Distância** | ver abaixo | continuamente | o grupo que precisa de decisão |
| **Ritmo relativo** | 6 × 3 | derivado da distância | quem vem atrás colando / abriu / vem forte; quem está à frente abrindo / caindo nas suas mãos |
| **Incidente à frente** | 6 × 3 | carro fora da pista adiante | acidente à frente, carro parado, carro devagar, detrito, cuidado na curva seguinte |
| **Box** | 4 × 3 | carro entrando/saindo do pit | entrada livre, carro saindo do box, cuidado na saída, atenção ao limite de velocidade |
| **Largada** | 4 × 3 | estado da sessão | volta de apresentação, formação, verde vem aí, largou |
| **Chuva** | 8 × 3 | mudou o nível de molhado | pista molhando, chuva começando, pista encharcada, pista secando, e as travessias entre elas |

**O grupo da distância — decisão sua.** A frase monta `[prefixo] + [valor]`:

- Prefixos: `"Atrás a,"` e `"À frente a,"` (2 peças de cabeça — reescreva se soar mal)
- Valores: `"meio segundo."`, `"um segundo."`, `"um e meio."`, `"dois segundos."` …

Duas resoluções possíveis:
- **0,5 s até 10 s → 21 valores.** Recomendo esta. É a resolução que um aviso falado
  aguenta, pela mesma razão que o milésimo foi rejeitado no tempo de volta.
- **0,1 s até 10 s → 99 valores.** Detalhe demais para o ouvido, provavelmente.

---

### PRIORIDADE 3 — Os sobrenomes (355)

Trabalho diferente: **não é escrever, é corrigir a grafia para que a voz leia certo.**

O jogo tem 355 sobrenomes de 23 nacionalidades, e eles estão guardados de um jeito que a
voz não lê:

| Guardado assim | Deveria soar como | Problema |
|---|---|---|
| `vanDijk` | van Dijk | palavras coladas, sem espaço |
| `deJong` | de Jong | idem |
| `DeSmet` | De Smet | idem |
| `Hamalainen` | Hämäläinen | acentos removidos |
| `Muller` | Müller | idem |
| `Gagne` | Gagné | idem |

Preciso, para cada um dos 355, da **grafia falável** — o texto que eu mando para a voz,
que pode ser diferente do nome escrito. Se a pronúncia correta não sair com a grafia
original, reescreva foneticamente em português (`Wisniewski` → `Vichnievski`, se for o caso).

Eu entrego a lista completa quando começarmos esta parte.

---

### SEM TRABALHO CRIATIVO

**Tempo de volta (1.201 peças).** Padrão já fechado: `"um trinta e dois e quatro."`, uma
peça por combinação de minuto, segundo e décimo. É geração mecânica, não precisa de
redação.

---

## 5. O formato da resposta

YAML, uma entrada por fala. Um arquivo por família está ótimo.

```yaml
familia: spotter_bandeiras
tipo: inteira          # inteira | cabeca | cauda
falas:
  - chave: bandeira_amarela_setor
    gatilho: "bandeira amarela num setor à frente do jogador"
    variacoes:
      - "Amarela no setor três."
      - "Amarela à frente."
      - "Bandeira amarela, cuidado."
```

- `chave` — minúscula, sem acento, com `_`. Vira o nome do arquivo de áudio.
- `tipo` — `inteira` (frase completa), `cabeca` (termina em vírgula, vem antes de outra
  peça), `cauda` (começa em minúscula, vem depois de outra peça).
- `gatilho` — uma linha dizendo quando a fala sai. É o que me permite conferir se o dado
  existe (seção 3) antes de gravar.
- `variacoes` — 3, salvo onde a tabela pedir 1.

---

## 6. Antes de escrever, confira

- [ ] Cabe em 3 palavras, se for do spotter?
- [ ] Tem travessão, reticências ou parênteses? Tire.
- [ ] Tem termo em inglês? Escreva pelo som (`Tri uáide`, não `Three wide`).
- [ ] A situação DURA? Então precisa também de um lembrete, com 3 variações.
- [ ] Peça de cabeça termina em vírgula? Fala inteira termina em ponto?
- [ ] Tem adjetivo concordando com o jogador (`pronto`, `sozinho`, `tranquilo`)? Reescreva.
- [ ] Depende de pneu, de dano dos outros, de combustível dos outros, ou de voltas
      restantes em prova por tempo? Não dá — corte.
- [ ] As 3 variações dizem a mesma coisa por ângulos diferentes, ou são sinônimo trocado?
- [ ] Número escrito por extenso?
