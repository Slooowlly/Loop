# POC de voz do engenheiro — da geração ao vivo ao pacote pré-gravado

Este documento começou como uma pergunta de latência: **o TTS do Google é rápido o
bastante para o engenheiro falar ao vivo?** A resposta foi sim. E foi irrelevante, porque
a audição encontrou um obstáculo maior que latência, e a solução para esse obstáculo tirou
a geração ao vivo do caminho crítico.

O documento está em duas partes. A **Parte 1** é a POC de latência, preservada porque as
medições continuam valendo e porque é a evidência de por que o caminho mudou. A **Parte
2** é o desenho que ficou de pé: um pacote de falas pré-gravadas, montadas por colagem.

## Onde isto parou

| | |
|---|---|
| **Fornecedor** | Cloud Text-to-Speech (não o Gemini TTS) |
| **Voz** | `pt-BR-Chirp3-HD-Algenib` |
| **Autenticação** | OAuth2 via ADC + header `x-goog-user-project`. Chave de API é **recusada**. |
| **Modo** | Pré-gravado. Sem streaming, sem chamada em tempo de jogo. |
| **Tempo de volta** | Peça **fundida** (`"um trinta e dois e quatro."`), ~1200 arquivos |
| **Codec** | Opus 24 kbps, mono 24 kHz — ~7 MB o pacote inteiro |
| **Ordem em execução** | decodificar → colar → **filtrar**. Nessa ordem, obrigatoriamente. |
| **Geração ao vivo** | Descartada para o pacote — **readmitida para o push-to-talk**. Ver Parte 3. |
| **Transcrição** | ElevenLabs `scribe_v2`, 735 ms de mediana. Ver Parte 4. |
| **Redação** | `gemini-2.5-flash` com `thinkingBudget: 0`, 827 ms. Ver Parte 5. |
| **Orçamento do PTT** | ~0,9 s no caminho gravado, ~2,9 s no caminho do modelo |

---

# Parte 1 — a POC de latência (Gemini TTS)

## O que a documentação dizia (consultada em 04/08/2026)

O `gemini-3.1-flash-tts-preview` é o **único** TTS do Gemini com streaming; a família 2.5
só devolve o áudio inteiro no fim. A chamada é a **Interactions API**
(`POST /v1beta/interactions`), não o `generateContent` clássico, e a resposta em
streaming é SSE com o áudio em base64 num delta de `type: "audio"`.

O áudio sai em PCM linear **16 bits, 24 kHz, mono, little-endian**, às vezes com
cabeçalho RIFF no primeiro bloco. A cobrança é por **25 tokens por segundo de áudio**,
o que dá ~US$ 1,80 por hora de fala gerada.

> O extrator de áudio, tanto no Rust quanto no script headless, **varre a árvore JSON**
> atrás do base64 em vez de indexar `delta.data` direto. A API é preview e o nome do
> campo já mudou entre revisões da documentação; varrer custa microssegundos e evita que
> a POC pare de funcionar por causa de uma renomeação.

Duas propriedades incômodas, que vieram a pesar na decisão final: o idioma é **inferido
do texto** (não há `language_code`, e pt-BR não é selecionável), e **não existe parâmetro
`seed`** em nenhum modelo TTS do Gemini.

Fontes: [geração de fala](https://ai.google.dev/gemini-api/docs/speech-generation),
[Interactions API](https://ai.google.dev/gemini-api/docs/interactions/speech-generation),
[preços](https://ai.google.dev/gemini-api/docs/pricing),
[limites](https://ai.google.dev/gemini-api/docs/rate-limits),
[termos](https://ai.google.dev/gemini-api/terms).

## Os números

Camada paga, 30 gerações, `--rpm 10`. **30 de 30 com sucesso, zero cortes na reprodução.**

Tempo até o **primeiro bloco de áudio**:

| Conjunto | n | melhor | mediana | P90 | P95 | pior |
|---|---|---|---|---|---|---|
| Curta e urgente | 10 | 728 ms | 908 ms | 943 ms | 1292 ms | 1292 ms |
| Informativa média | 10 | 763 ms | 1007 ms | 1226 ms | 1620 ms | 1620 ms |
| Narrativa de sabor | 10 | 896 ms | 941 ms | 1098 ms | 1178 ms | 1178 ms |
| **TOTAL** | **30** | **728 ms** | **935 ms** | **1178 ms** | **1292 ms** | **1620 ms** |

Do primeiro bloco ao primeiro som somam-se três termos medidos — encher o pré-buffer
(~49 ms, porque o stream entrega a 2,39× o tempo real), folga de agendamento (20 ms) e
latência de saída do dispositivo (40 ms) — dando **~109 ms**. Total: mediana de
**1.052 ms**, P95 de **1.428 ms**. Isso classifica como **"muito viável"** no critério
que a POC tinha definido de antemão.

### O streaming era a arquitetura inteira

O mesmo modelo, o mesmo texto, `stream: false`:

| Categoria | Com streaming | Sem streaming | Fator |
|---|---|---|---|
| Curta (≈40 car.) | 908 ms | 3.104 ms | 3,4× |
| Média (≈111 car.) | 1.007 ms | 7.131 ms | 7,1× |
| Narrativa (≈141 car.) | 941 ms | 10.051 ms | 10,7× |

Sem streaming a espera **escala com o tamanho do texto**. Com streaming ela é **plana**:
a narrativa longa começa a chegar no mesmo tempo que o alerta seco. Essa era a descoberta
que sustentava o desenho — e é a mesma descoberta que deixou de importar quando a fala
passou a ser pré-gravada, porque sem tempo real o streaming não compra nada.

## Por que a latência deixou de importar

A audição, uma geração por vez, encontrou o que nenhuma métrica de tempo capturaria:
**a voz não é estável entre chamadas.** Mesma voz nomeada, mesmo texto, mesma direção,
mesmos parâmetros — e a saída sai como outra pessoa. Não há como prender uma tomada: o
`generation_config` do TTS aceita apenas `speech_config.voice`, e não existe `seed`.

Para o Loop isso é mais grave que latência. Um engenheiro que muda de pessoa entre uma
fala e outra quebra a ilusão de um jeito que nenhum tempo de resposta quebra. Um segundo
de espera é aceitável num rádio de equipe; um interlocutor diferente a cada dez falas
destrói exatamente a relação que a fala dinâmica existia para construir.

Daí a virada: **se a identidade só se garante escolhendo a tomada a dedo, então as falas
têm de ser pré-gravadas — e se são pré-gravadas, streaming e latência saem da conta.**

## Dois achados da audição que sobreviveram à virada

**A moderação recusa texto inocente, e de forma não determinística.** `Livre.` — palavra
portuguesa comum, em contexto de automobilismo — foi bloqueada como *"sensitive words
that violate the Prohibited Use policy"*. Medido no mesmo texto: **5 bloqueios e 4
sucessos**. A primeira leitura desta POC dizia que a recusa era estável por texto; é
falso, e a correção importa: um gerador de biblioteca precisa de **repetição automática**
quando o bloqueio aparece, não de uma lista de palavras proibidas.

**Pontuação vale mais que a voz escolhida, e é decisão por frase.** `...` no fim deixa a
fala morrer aberta, como quem vai emendar a próxima informação; o ponto final fecha com
queda de entonação. Falas de manutenção (*"Segura aí…"*, *"Ainda aí…"*) pedem a cauda
aberta; falas conclusivas (*"Pista livre."*) pedem o ponto. **Não é regra global** — é
propriedade de cada linha, e a lista de falas precisa carregar a sua. Aplicar reticências
como transformação automática foi um erro desta POC, corrigido.

---

# Parte 2 — o pacote pré-gravado (Cloud TTS)

## O outro Google

A Cloud Text-to-Speech é um produto diferente do Gemini TTS, com armadilhas próprias:

- **Recusa chave de API** — `401 "API keys are not supported by this API"`. Exige OAuth2;
  aqui vem do Application Default Credentials do `gcloud`.
- **Exige projeto de cota** no header `x-goog-user-project`. Sem ele, `403` apontando
  para o projeto do próprio SDK, o que é um erro difícil de ler.
- **pt-BR é explícito** (`languageCode`), não inferido do texto. Cada voz declara gênero.
  São os dois buracos que custaram caro no Gemini.
- **Sem direção por prompt** nas famílias convencionais; o controle é SSML.

### Consistência — o critério que decide

Provado por comparação de SHA, três gerações do mesmo texto:

| Família | Resultado |
|---|---|
| `pt-BR-Neural2-B` (convencional) | **bytes idênticos** nas 3 |
| `pt-BR-Chirp3-HD-Algenib` (generativa) | 3 hashes distintos, durações 1,24 / 1,36 / 1,68 s |

Chirp 3 **não** é byte-determinística — é generativa, como o Gemini TTS. A diferença é
que a deriva ficou **abaixo do limiar perceptivo**: numa audição de um pacote inteiro, as
falas soaram como a mesma pessoa. Isso é o que basta para um pacote pré-gravado, onde
cada tomada é ouvida antes de entrar. Não bastaria para geração ao vivo.

`Algenib` foi escolhida por audição, contra as demais vozes Chirp 3 em pt-BR.

### Preço e cota

| | |
|---|---|
| Chirp 3 HD | **US$ 30 / 1M caracteres**, com **1 milhão de caracteres grátis por mês** |
| Cota | **200 requisições por minuto** por projeto, **sem teto diário** |
| Limite por chamada | 5.000 bytes — não aumentável |

O pacote embutido (spotter + engenheiro sem nome próprio, ~30 mil caracteres) cabe
**inteiro no gratuito**. Geração por carreira com o nome do jogador sairia em ~44 mil
caracteres, o que dá ~22 carreiras por mês de graça e US$ 1,32 cada uma depois disso.

## Colagem — a ideia que torna o pacote viável

Se uma frase pode ser montada de pedaços gravados em separado, a biblioteca deixa de ser
"uma gravação por nome × por frase" e vira "os nomes **mais** as frases". É a diferença
entre milhares de arquivos e dezenas.

Testado com `"Daniel,"` + `"boxes nesta volta, boxes."` contra a mesma frase gerada de uma
tacada só. **Funciona** — a emenda não se ouve. A troca de nome é livre.

[`juntar.mjs`](../scripts/tts-poc/juntar.mjs) resolve os dois problemas da colagem ingênua
e alarma sobre o terceiro:

1. **Silêncio de borda.** Todo TTS embrulha a fala em silêncio. Concatenar cru soma as
   duas bordas e abre ~1 s de buraco no meio da frase.
2. **Clique na emenda.** Cortar rente ao zero cria um degrau na onda. Rampa de 5 ms nas
   pontas de cada peça.
3. **Buraco interno** — quando o modelo parte a peça em dois fôlegos. Não é problema de
   colagem, é de material, e o único conserto é regerar. O script grita.

O rádio é aplicado **depois** da junção, no sinal inteiro. Filtrar cada peça e só então
colar deixaria o compressor com histórico diferente em cada uma, e a emenda ganharia um
salto de volume.

## Tempo de volta — três desenhos, um vencedor

Três decomposições do mesmo `1:32.418`, cada uma comparada com a frase gerada inteira:

| desenho | peças | montado | referência |
|---|---|---|---|
| **fundido** | `"Volta em,"` + `"um trinta e dois e quatro."` | 2,32 s | 2,72 s |
| décimo | + `"um trinta e dois,"` + `"quatro."` | 2,69 s | 2,72 s |
| dígito a dígito | + `"um,"` + `"trinta e dois,"` + `"quatro,"` + `"um,"` + `"oito."` | 4,02 s | 3,08 s |
| qualitativo | + `"um trinta e dois,"` + `"baixo."` | 2,37 s | 1,84 s |

**O fundido ganhou por uma margem larga** — veredito de audição, não de número. O dígito a
dígito soa robótico e ainda custa ~1 s a mais, que num rádio de corrida é muito. O
qualitativo (`baixo` / `alto` / `cravado`) foi gerado, ouvido e **rejeitado**: parecia boa
ideia no papel, porque resolveria o `1:32.000` sem o esquisito `"zero"` no fim, mas não
convenceu na audição.

Repare que o fundido sai **mais curto que a referência**: a peça inteira já vem com a
prosódia certa e sobra uma única emenda para pagar.

### O inventário

| desenho | arquivos |
|---|---|
| **fundido** | **~1201** — 1 lead + 120 combinações minuto-segundo × 10 décimos |
| décimo | 131 |
| qualitativo | 124 |

Os 120 assumem minutos 1 e 2. As nove categorias do Loop têm voltas abaixo de um minuto
e acima de dois, o que soma mais uma faixa em qualquer um dos desenhos — não muda a ordem
de grandeza.

Os 1201 do fundido parecem caros e não são: gerar tudo custa ~30 mil caracteres, dentro do
1 milhão grátis, e é feito uma vez. O custo real é disco, e o Opus resolve.

## Opus — quanto o codec custa

Ida e volta pelo libopus, imitando o que o produto fará: cada peça guardada em Opus,
decodificada no carregamento, colada, e **só então** filtrada. Comprimir o áudio já
filtrado esconderia o pior do teste, porque a cadeia de rádio levanta os trechos baixos,
que é exatamente onde o artefato de codec mora.

| taxa | 2 peças (3,5 s) | pacote de 1201 |
|---|---|---|
| WAV | 163 KB | ~115 MB |
| 48 kbps | 19,5 KB | ~14 MB |
| 32 kbps | 13,1 KB | ~9,4 MB |
| **24 kbps** | **9,9 KB** | **~7,0 MB** |
| 16 kbps | 6,8 KB | ~4,7 MB |

**Nenhuma diferença audível entre 16 e 48 kbps.** Há um motivo técnico: o passa-baixas do
rádio corta em 3,2 kHz, e é acima de 3,2 kHz que o Opus faz a maior parte da sujeira em
taxa baixa. O filtro joga fora o artefato antes de ele chegar ao ouvido.

A escolha ficou em **24 kbps mesmo assim**, não em 16. A diferença no pacote é 2,3 MB —
irrelevante dos dois lados — e os 24 compram margem para o dia em que a cadeia de rádio
for retocada. Subir o corte de agudo de 3,2 para 5 kHz faria o 16 aparecer, e o pacote
inteiro teria de ser regerado. Economizar 2 MB não paga esse risco.

## A cadeia de rádio

Mesma receita em dois lugares: [`ttsRadio.js`](../src/dev/tts/ttsRadio.js) (Web Audio, no
app) e [`filtro-radio.mjs`](../scripts/tts-poc/filtro-radio.mjs) (Node, para audição fora
do app). **Mudou num, muda no outro.**

```
passa-altas 300 Hz → passa-baixas 3200 Hz → presença 1800 Hz +6 dB → saturação (tanh, 2×)
→ ganho 1,9 → compressor (−24 dB, joelho 6, razão 8:1, ataque 3 ms, release 120 ms)
→ recuperação 3,5× → limitador (limiar 0,7, teto 0,97)
```

A ordem não é estética; cada posição foi decidida por medição:

- **O ganho vem ANTES do compressor.** Com ele na saída, o pico media 1,298 — clipe
  garantido. Empurrando antes, o compressor é quem limita e o pico fica em 0,985.
- **A recuperação de nível vem DEPOIS.** Sem ela, com limiar em −24 dB e razão 8:1, a fala
  fica achatada contra o limiar e a cadeia sai três a quatro vezes mais baixa que a voz
  limpa — RMS medido caindo de 0,14 para 0,04.
- **O limitador é o último.** O compressor tem ataque de 3 ms e o transiente escapa por
  baixo dele: com a recuperação, os picos chegavam a 1,70. O limitador mexe só nos poucos
  milissegundos que passam do teto.

O custo em latência é **10,13 ms**, medido por impulso em `OfflineAudioContext` — vem do
look-ahead do compressor e do sobreamostrado 2× da saturação; os filtros são de custo
zero. Para o pacote pré-gravado isso é irrelevante; ficou registrado porque era a
pergunta da POC original.

> A versão Node reproduz o **som**, não a amostra. O `DynamicsCompressor` do Chromium tem
> curva de joelho e look-ahead próprios, e o sobreamostrado 2× do `WaveShaper` usa filtros
> que a especificação não define. Para decisão de timbre isso basta; se a diferença um dia
> importar, a referência é o app.

## Armadilha do material: o silêncio do Chirp 3 não é zero

O aparador de silêncio do `juntar.mjs` nasceu errado e os números denunciaram:
`"Volta em"` tinha 1,64 s de arquivo e o corte tirou 0,00 s.

A causa é que o silêncio do Chirp 3 é um **chiado baixo cujos picos isolados passam de
−45 dBFS**. Bastava uma amostra de ruído para o trecho inteiro contar como fala. O corte
por pico não separa chiado de voz; energia média numa janela curta separa.

A versão correta usa janela de 10 ms e limiar **relativo ao pico da própria peça** (−32 dB,
com piso absoluto em −50 dB). Relativo, e não absoluto, porque os níveis variam muito entre
gerações: `"um,"` saiu 10 dB mais baixo que `"um trinta e dois,"`. Depois da correção o
corte encontrou os 0,70 s que estavam lá.

---

## Como rodar

### Cloud TTS (o caminho atual)

Uma vez, na máquina:

```bash
gcloud auth application-default login
```

```bash
gcloud services enable texttospeech.googleapis.com
```

Uma fala, já com o rádio (é o padrão — o som do produto é o filtrado; a voz limpa é
material intermediário, e ouvir o intermediário para decidir leva a decidir errado):

```bash
node scripts/tts-poc/uma-fala-cloud.mjs --rotulo boxes --texto "Boxes nesta volta, boxes."
```

Prova de consistência, comparando os hashes de N gerações:

```bash
node scripts/tts-poc/uma-fala-cloud.mjs --voz pt-BR-Chirp3-HD-Algenib --repetir 3
```

Colar peças (o rádio é aplicado depois da junção):

```bash
node scripts/tts-poc/juntar.mjs lead.wav corpo.wav --pausa 60 --saida frase.wav
```

Reprocessar o filtro num `.wav` já gerado, sem gastar chamada:

```bash
node scripts/tts-poc/filtro-radio.mjs docs/tts-poc/audicao-cloud/algum.wav
```

Flags que importam: `--sem-radio` (ouvir a voz crua), `--velocidade`, `--tom`,
`--ssml`, `--projeto`. No `juntar.mjs`: `--pausa` em ms e `--sem-aparar`.

### Gemini TTS (o caminho da Parte 1, mantido)

`GEMINI_API_KEY` no ambiente, ou o arquivo `<app_data_dir>/gemini_tts_key.txt`. A chave
**nunca** chega ao frontend — o painel só é informado da origem dela.

```bash
node scripts/tts-poc/bateria.mjs --rpm 10
```

O painel interativo vive em `/dev/tts` sob `npm run tauri dev`. Precisa ser o shell do
Tauri: `npm run dev` sozinho não tem o `invoke`, e o painel avisa isso. Nada disso entra
no bundle do jogador — a rota está atrás de `import.meta.env.DEV`, que vira a constante
`false` no build de produção e leva o import dinâmico junto.

## O que foi construído

| Arquivo | Papel |
|---|---|
| [`scripts/spotter-pack.mjs`](../scripts/spotter-pack.mjs) | **O primeiro pacote de produção.** Gera as 7 falas do spotter, nomeadas pela chave do evento. |
| [`scripts/tts-poc/uma-fala-cloud.mjs`](../scripts/tts-poc/uma-fala-cloud.mjs) | Cloud TTS: uma geração, com rádio por padrão e prova de determinismo por SHA. |
| [`scripts/tts-poc/juntar.mjs`](../scripts/tts-poc/juntar.mjs) | Colagem de peças: rampa anti-clique e alarme de buraco interno. |
| [`scripts/tts-poc/filtro-radio.mjs`](../scripts/tts-poc/filtro-radio.mjs) | A cadeia de rádio em Node — módulo e CLI — mais o aparador de silêncio. |
| [`src/lib/spotterVoice.js`](../src/lib/spotterVoice.js) | Reprodução do pacote no app: decodifica uma vez, e a fala mais nova corta a anterior. |
| [`scripts/tts-poc/uma-fala.mjs`](../scripts/tts-poc/uma-fala.mjs) | Gemini TTS: uma fala, para audição de voz e de direção. |
| [`scripts/tts-poc/bateria.mjs`](../scripts/tts-poc/bateria.mjs) | Gemini TTS: a bateria de 30 sem 30 cliques. |
| [`src-tauri/src/commands/tts_poc.rs`](../src-tauri/src/commands/tts_poc.rs) | POST, leitura do SSE e repasse de cada bloco base64 por evento Tauri. |
| [`src/dev/tts/ttsPcm.js`](../src/dev/tts/ttsPcm.js) | base64 → PCM → Float32. Come o cabeçalho RIFF, carrega o byte órfão entre blocos. |
| [`src/dev/tts/ttsPlayer.js`](../src/dev/tts/ttsPlayer.js) | Reprodução em streaming sobre Web Audio, com a instrumentação. |
| [`src/dev/tts/ttsRadio.js`](../src/dev/tts/ttsRadio.js) | A cadeia de rádio como grafo persistente. |
| [`src/dev/tts/ttsRunner.js`](../src/dev/tts/ttsRunner.js) | Uma geração de ponta a ponta, com corte por SLA e reserva local. |
| [`src/dev/tts/ttsMetrics.js`](../src/dev/tts/ttsMetrics.js) | Percentis, faixas e o veredito de latência. |
| [`src/dev/tts/TtsPocPage.jsx`](../src/dev/tts/TtsPocPage.jsx) | O painel, em `/dev/tts` (só no build de desenvolvimento). |

Áudio em `docs/tts-poc/audio/` (bateria), `docs/tts-poc/audicao/` (Gemini) e
`docs/tts-poc/audicao-cloud/` (Cloud). Logs em `docs/tts-poc/bateria.jsonl` e
`docs/tts-poc/sem-streaming.jsonl`.

## O primeiro pacote em produção — o spotter

Sete falas, geradas por [`scripts/spotter-pack.mjs`](../scripts/spotter-pack.mjs) em
`src/assets/spotter/<chave>.wav`, onde a chave é a do evento que
[`iracing_sdk::spotter`](../src-tauri/src/iracing_sdk/spotter.rs) emite. Acrescentar uma
fala é acrescentar um arquivo — não há tabela de-para no meio.

O WAV é o **master**, e fica fora do git: o app carrega `<chave>.opus`, produzido por
[`scripts/audio-para-opus.mjs`](../scripts/audio-para-opus.mjs). O acervo do engenheiro
cresceu para 3.943 peças — 328 MB em PCM contra 29 MB em Opus a 32 kbps — e WAV não
delta-comprime, então cada regravação somaria o arquivo inteiro ao histórico. As ferramentas
que medem **forma de onda** (`engenheiro-auditar.mjs` e as audições em `scripts/tts-poc/`)
continuam lendo o WAV, porque pico e silêncio interno se medem no PCM da tomada, não na
saída do codec — quem não tem os masters no disco não roda essas.

| Chave | Fala | Duração |
|---|---|---|
| `esquerda` | "Esquerda." | 0,73 s |
| `direita` | "Direita." | 0,73 s |
| `tres_largos` | "Três largos." | 0,95 s |
| `duas_esquerda` | "Dois à esquerda." | 1,15 s |
| `duas_direita` | "Dois à direita." | 1,09 s |
| `livre` | "Livre." | 0,51 s |
| `teste` | "Spotter na escuta." | 1,29 s |

303 KB no total, em WAV 24 kHz. Não vale converter para Opus neste tamanho: sete arquivos
custam menos que o decodificador que precisariam.

Três decisões da POC mudaram de valor aqui:

- **A cadeia de rádio é gravada no arquivo**, não aplicada na hora. A regra "filtrar
  depois de juntar" existia por causa da emenda entre peças; estas falas nunca são
  coladas, então a razão não se aplica e o app fica com um `decodeAudioData` e nada mais.
- **Aparar o silêncio deixou de ser estética e virou latência.** Meio segundo de silêncio
  de cabeça é meio segundo de atraso num aviso que só vale enquanto o carro ainda está do
  lado. O aparador saiu do `juntar.mjs` para o `filtro-radio.mjs` justamente para os dois
  clientes usarem a mesma calibragem.
- **A fala mais nova corta a anterior.** Enfileirar produziria "esquerda" quando o carro
  já está à direita. Isto é o que permitiu tirar a trava de cadência do detector, que
  descartava anúncios em vez de adiá-los.

O `teste` é o único que não descreve o mundo: é o spotter se apresentando quando o piloto
senta no carro. Existe porque o silêncio é ambíguo — um spotter mudo pode estar certo ou
estar morto —, e descobrir qual dos dois no meio de uma disputa a três é tarde.

## O próximo passo

**O gerador de biblioteca.** Ler uma lista de falas que carregue a pontuação de cada uma,
gerar tudo, escrever os `.wav` nomeados por chave, repetir automaticamente quando a
moderação bloquear, e empacotar em Opus. Aí as frases viram conteúdo e não código.

O inventário real já foi contado: o bloco `breakdown` de
[`src-tauri/locales/pt-BR.yml`](../src-tauri/locales/pt-BR.yml) tem **148 strings — 54
estáticas** (22 caracteres em média) **e 94 com placeholders** (50 caracteres em média).
As 94 são o problema interessante: não dá para pré-gravar como estão, e é exatamente para
elas que a colagem existe.

Em aberto: **Chirp 3 Instant Custom Voice** (clonagem, US$ 60 / 1M, 30 RPM, sem camada
gratuita). Se fixar uma voz de referência em pt-BR, a deriva de timbre acaba e a curadoria
das tomadas some junto. É o único obstáculo técnico que sobrou.

## Riscos

1. **A deriva de timbre não sumiu, só ficou tolerável.** Chirp 3 é generativa e não tem
   `seed`. O pacote pré-gravado contorna porque cada tomada é ouvida antes de entrar —
   mas isso é curadoria manual, e ela não escala sozinha.
2. **A moderação bloqueia texto inocente, de forma não determinística.** O gerador de
   biblioteca precisa de repetição automática; sem isso, uma frase some do pacote sem
   ninguém perceber.
3. **A pontuação é decisão por frase.** A lista de falas precisa carregar a sua, e nenhuma
   transformação automática substitui isso.
4. **Buraco interno na peça** quando o modelo parte a fala em dois fôlegos. O `juntar.mjs`
   detecta, mas o conserto é regerar — e um pipeline em lote precisa tratar isso sozinho.
5. **A prosódia de peça isolada é de frase isolada.** A colagem passou na audição, mas
   peças gravadas fora de contexto têm entoação de começo e de fim. Onde incomodar, o
   caminho é gerar a peça já dentro da frase completa e descartar o resto.
6. **~1200 arquivos são um artefato de build**, não fonte. Precisa de um passo
   reproduzível, senão regerar o pacote vira trabalho manual.
7. **Duas cópias da cadeia de rádio** (Web Audio e Node) que podem divergir em silêncio.
   Não há teste amarrando as duas.
8. **`gcloud` como dependência de desenvolvimento.** O token vem do ADC; num CI sem
   `gcloud` isso quebra e precisaria de conta de serviço.
9. **Nenhuma cláusula encontrada proibindo redistribuir a voz gerada dentro de um
   produto** — mas isso foi lido, não confirmado com jurídico. Antes de publicar, vale
   confirmar.

---

# Parte 3 — o push-to-talk, ou a geração ao vivo de volta

A Parte 1 descartou a geração ao vivo por causa da deriva de timbre. O push-to-talk a
traz de volta, porque não há alternativa: o piloto faz uma pergunta que ninguém previu, e
uma resposta a uma pergunta imprevista não pode estar pré-gravada.

O desenho é um sanduíche. O piloto segura o botão e fala; ao soltar, uma frase de espera
**pré-gravada** entra na hora (`"Ok, deixa eu ver aqui…"`), e é dentro dela que o
encanamento inteiro roda — Scribe transcreve, Gemini redige, Cloud TTS sintetiza. A
resposta sai quando fica pronta.

Isso põe a fala curada e a fala crua **no mesmo ouvido, separadas por segundos** — o pior
arranjo possível para a deriva que a Parte 1 mediu. Era a pergunta que podia matar o
desenho antes de ele existir, e por isso foi a primeira a ser medida.

## O veredito de timbre

[`audicao-ptt.mjs`](../scripts/tts-poc/audicao-ptt.mjs) gera três frases de espera e dez
respostas, cada uma em sua própria chamada, apara o silêncio, cola cada resposta atrás de
uma espera e filtra o par inteiro. A pausa da colagem é de **700 ms** e não os ~4 s reais:
o teste é de timbre, e quanto mais perto as duas falas, mais dura a comparação.

**Passou.** Audição dos dez pares: mesma pessoa, e a emenda não se ouve. O controle — a
mesma frase três vezes — confirmou de novo que a Chirp 3 não é determinística (três hashes
distintos, durações de 2,08 / 1,92 / 2,36 s, **23% de espalhamento**), e de novo isso ficou
abaixo do limiar perceptivo. A conclusão da Parte 1 (*"não bastaria para geração ao vivo"*)
era prudente e está **corrigida pela medição**: basta.

## A latência que ninguém tinha medido

A Parte 1 mediu o Gemini TTS. A Cloud TTS nunca teve latência medida — só consistência.
17 chamadas:

| n | melhor | mediana | P90 | pior |
|---|---|---|---|---|
| 17 | 576 ms | **1.104 ms** | 1.504 ms | 1.698 ms |

E o achado que muda uma decisão de produto — **não escala com o texto**:

| tamanho | n | média |
|---|---|---|
| até 40 caracteres | 10 | 976 ms |
| 70+ caracteres | 3 | 1.318 ms |
| | | **fator 1,35×** |

O Gemini TTS sem streaming fazia **10,7×** nesse mesmo eixo (3,1 s no curto, 10,1 s na
narrativa) — foi o que tornou o streaming a arquitetura inteira da Parte 1. A Cloud TTS é
praticamente plana. A consequência prática: **limitar o tamanho da resposta é decisão de
estilo do engenheiro, não requisito de latência.** O Gemini pode escrever o que for natural
para um rádio de equipe, e a síntese custa o mesmo.

A TTS é, portanto, a perna **mais barata** das três. O orçamento do PTT se decide em Scribe
e Gemini, que ainda não foram medidos.

## O defeito que aparece 1 vez em 10

O `buracoInterno` acusou 6 dos 10. Medindo o tamanho de cada vão, só um é defeito:

| | vão interno |
|---|---|
| `"Você ganhou duas posições na largada. Sexto agora."` | **1,23 s** |
| duas respostas de duas orações | 0,46 / 0,51 s |
| três respostas | 0,23–0,31 s |
| quatro respostas | — |

Os de 0,2–0,3 s são pausa de frase, prosódia correta: o detector foi calibrado para peça de
colagem — um fôlego só — e uma resposta de duas orações está fora dessa calibragem. Os
1,23 s são o defeito real da Parte 2, o modelo partindo a fala em dois fôlegos.

A diferença é que **no pacote pré-gravado o conserto é regerar, e ao vivo não há tempo**. A
resposta sai quebrada e pronto. A saída é colapsar silêncio interno acima de ~350 ms na hora
de tocar, com a mesma lógica de envelope do `filtro-radio.mjs` — conserta o defeito e ainda
encurta a resposta. Vale para todo silêncio, não só o defeituoso: o silêncio de cabeça da
Chirp 3 é de ~0,7 s, e ao vivo cada um desses milissegundos é latência pura.

## As frases

Todas as esperas terminam em reticências **de propósito**, aplicando a regra de pontuação da
Parte 1: a cauda aberta faz a fala morrer como quem vai emendar a informação a seguir — que
é literalmente o que estas frases fazem. A desistência fecha com ponto, porque é conclusiva.

| papel | fala |
|---|---|
| espera | `"Ok, deixa eu ver aqui…"` · `"Peraí, tô checando…"` · `"Deixa eu olhar…"` |
| desistência | `"Não consegui ver isso agora."` |

A desistência existe porque silêncio depois de um "deixa eu ver" é pior que uma negativa.

## As chaves ficam no proxy

O app **não embarca chave de provedor nenhuma** e não vai passar a embarcar. O que existe
hoje é o `APP_SECRET` de [`narrative/client.rs`](../src-tauri/src/narrative/client.rs) —
porta de entrada do nosso Cloud Run, embutida por design — e o caminho de POC do
`tts_poc.rs`, que lê `GEMINI_API_KEY` do ambiente e vive atrás de `import.meta.env.DEV`.

O PTT segue a mesma regra: **as três chamadas saem do servidor**, atrás de um endpoint novo
que recebe o áudio e os fatos e devolve o áudio. Isso herda de graça o cooldown por
`install_id` e o teto de gasto que os outros endpoints já têm — e herda também o
`spawn_warmup()` como obrigação, porque o Cloud Run faz scale-to-zero e a primeira chamada
depois de ocioso paga 20–40 s de cold start. Sem aquecer quando o piloto senta no carro, o
primeiro PTT da sessão é meio minuto de silêncio depois do "deixa eu ver".

## Riscos da Parte 3

1. **1 em 10 sai com buraco interno e ao vivo não dá para regerar.** O colapso de silêncio
   na reprodução é mitigação, não conserto — uma fala partida em dois fôlegos continua com
   a prosódia errada, só que mais curta.
2. **A moderação bloqueia texto inocente de forma não determinística** (Parte 1: `Livre.`
   deu 5 bloqueios em 9). Ao vivo isso é uma resposta que simplesmente não sai. Precisa cair
   na frase de desistência, não no silêncio.
3. **O cold start do Cloud Run é maior que o PTT inteiro.** O aquecimento vira requisito,
   não otimização.
4. **Scribe e Gemini não foram medidos.** Todo o orçamento de tempo está apoiado em duas
   estimativas.
5. **A deriva passou nesta audição, com esta voz e estas frases.** Não é uma garantia
   permanente: a Chirp 3 é generativa e o modelo do lado de lá pode mudar sem aviso.

---

# Parte 4 — o Scribe, e o desenho que mudou de forma

A Parte 3 terminou com um orçamento de três quintos chutado. Antes de medir o que faltava, o
desenho mudou — e a mudança reordenou o que precisava ser medido.

## A ideia que tirou o modelo do caminho comum

A maioria das perguntas de rádio tem **forma fixa e valor variável**: "faltam N voltas", "o
carro da frente está a X". Mandar isso a um modelo é pagar dois serviços e dois segundos para
redigir uma frase que já se sabe de cor.

Então o push-to-talk passou a ter dois caminhos:

```text
áudio ─► Scribe ─► classificar ─┬─► renderiza  ─► toca peças gravadas   (rápido, grátis)
                                └─► não renderiza ─► Gemini + TTS        (o caminho lento)
```

O que decide **não é a intenção, é se a fala renderiza**: o renderizador tenta montar a
resposta com as peças do acervo e devolve nada se faltar **uma**. Tudo-ou-nada, porque meia
fala gravada emendada com meia gerada traria de volta a emenda e a deriva de timbre juntas.

Isso inverteu a prioridade da medição. O Scribe deixou de ser uma etapa entre cinco e virou
**a única coisa no caminho da maioria das perguntas**.

## Os números

[`scripts/scribe-poc/medir.mjs`](../scripts/scribe-poc/medir.mjs), modelo `scribe_v2`, 15
perguntas × 2 versões (limpa e passada pela cadeia de rádio), `language_code=por`.

| n | melhor | mediana | P90 | pior |
|---|---|---|---|---|
| 30 | 475 ms | **735 ms** | 1.210 ms | 1.371 ms |

E, como a Cloud TTS, **não escala com o tamanho**:

| duração do clipe | n | média |
|---|---|---|
| até 2,2 s | 18 | 843 ms |
| 3 s ou mais | 6 | 899 ms |
| | | **fator 1,07×** |

Perguntar mais devagar não custa mais. As duas pernas medidas do encanamento são planas.

## Transcrição: 44 de 45 palavras-chave intactas

| versão | exatas | WER |
|---|---|---|
| limpa | 15/15 | 0,0% |
| com rádio | 14/15 | 1,1% |

O único erro foi a cadeia de rádio comendo uma conjunção: *"pro carro da frente **e** dá pra
alcançar"* virou *"pro carro da frente dá pra alcançar"*.

**E a intenção sobreviveu assim mesmo** — 30 de 30 classificadas certo, incluindo as quatro
perguntas ambíguas que já derrubaram a tabela de termos. É a vantagem estrutural de casar
termo em vez de comparar frase: uma conjunção perdida não muda de assunto.

O laço é fechado por um teste de verdade e não por inspeção. O `medir.mjs` grava o que o
Scribe ouviu em `docs/scribe-poc/transcricoes.json`, e
`engenheiro::tests::intencao_sobrevive_a_transcricao` lê o arquivo e passa cada transcrição
pelo classificador **de produção**. Reimplementar a classificação no script em JS criaria uma
segunda verdade, e a divergência apareceria como roteamento errado em corrida — não como
teste vermelho. O teste pula sozinho quando o arquivo não existe, porque a medição depende de
chave e de cota.

## O que estes números NÃO dizem

O áudio foi **gerado pela mesma Cloud TTS do engenheiro**: voz de estúdio, sem motor, sem
headset, sem ruído de cockpit. Qualquer taxa de acerto aqui é um **teto**, não uma previsão.

O que o eixo de acurácia testa de fato é o **nosso lado**: se o classificador errasse com
áudio limpo, o problema estaria na tabela de termos e nenhum microfone melhor consertaria.
Ele não errou. Isso elimina uma hipótese; não confirma a outra.

A versão passada pela cadeia de rádio é a aproximação disponível de degradação — corta em
3,2 kHz, satura e comprime. Não é ruído de motor, e a diferença de 0% para 1,1% de WER entre
as duas versões é o único sinal que temos de sensibilidade a áudio sujo. **Medir com voz real
e o sim rodando continua pendente.**

## Dois padrões da API que trabalham contra nós

O `tag_audio_events` vem **ligado** e marca risadas, passos e afins — processamento puro sem
uso numa pergunta de três segundos. O `timestamps_granularity` vem em `word`, e não usamos
carimbo de tempo em lugar nenhum. Os dois são desligados aqui; `--padroes` mede com os padrões
deles, para saber quanto custam.

Existe também um `pcm_s16le_16` anunciado como de menor latência, ainda não comparado contra o
WAV usado aqui.

## O orçamento, agora com quatro quintos medidos

| etapa | mediana | P90 | |
|---|---|---|---|
| Scribe | **735 ms** | 1.210 ms | medido |
| Cloud TTS | **1.104 ms** | 1.504 ms | medido |
| Gemini (saída curta) | ~1.000 ms | ? | **estimado** |
| Ida e volta ao nosso proxy | ~200 ms | ? | **estimado** |

Somando: o **caminho gravado** fica por volta de **0,9 s** — abaixo da mais curta das frases
de espera (1,36 s) e bem abaixo da mais longa (2,36 s). O **caminho do modelo** fica em torno
de **3 s**, dentro da faixa em que uma pausa soa natural.

Consequência de desenho: **a frase de espera precisa ser condicional**. Dispará-la ao soltar o
botão faria o caminho rápido esperar 2,36 s de "deixa eu ver" com a resposta já pronta na mão
— mais lento que o caminho lento. O certo é armar um temporizador de ~700 ms: se a resposta
chegou antes, nenhuma espera toca.

## Riscos da Parte 4

1. **Acurácia medida em áudio sintético.** O número real, com motor e headset, é pior e não
   se sabe quanto.
2. **O classificador virou carregador de peso.** Antes ele só escolhia o contexto do modelo;
   agora dispara uma fala pronta, e um erro dele é uma resposta errada dita com convicção.
3. **O `scribe_v2` em lote não faz streaming.** Existe um **Scribe v2 Realtime** por
   WebSocket, ~150 ms anunciados: transmitindo enquanto o piloto segura o botão, a
   transcrição ficaria pronta quando ele solta. Não medido, e custa manter um socket aberto
   no proxy.
4. **O cold start do Cloud Run continua maior que o orçamento inteiro** (20–40 s). O
   aquecimento é requisito, não otimização.
5. **Gemini segue sem medição** — é o último quinto, e só aparece no caminho da minoria.

---

# Parte 5 — o Gemini, e quatro defeitos que só a medição encontraria

O último quinto do orçamento. O Gemini só atende o que o acervo não cobre — ritmo, dano no
carro, pergunta aberta e os casos que o renderizador recusa —, então é a perna menos crítica.
Mediu-se mesmo assim, e o que ela devolveu de mais valioso não foi o tempo.

## O material vem do código

Os dossiês são gerados pelo teste `dumpa_dossies_para_medicao`, com as **mesmas funções que a
produção usa**. Escrever os fatos à mão no script mediria um prompt que a produção nunca vai
emitir, e envelheceria no primeiro campo novo do `EstadoAgora`. Só entram os casos que o
renderizador **recusa** — medir com um caso que o acervo cobre seria medir uma chamada que
nunca acontece.

## Os números

[`scripts/gemini-poc/medir.mjs`](../scripts/gemini-poc/medir.mjs), ~660 tokens de entrada,
~20 de saída:

| modelo | mediana | P90 | |
|---|---|---|---|
| `gemini-2.5-flash-lite` | **715 ms** | 956 ms | raciocínio desligado por padrão |
| `gemini-2.5-flash` + `thinkingBudget: 0` | **827 ms** | 979 ms | **a escolha** |
| `gemini-2.5-flash` (padrão) | 4.048 ms | 4.387 ms | ~765 tokens de raciocínio por frase |

O `flash` com raciocínio custa **cinco vezes** o mesmo modelo sem ele, para produzir uma
frase. Desligado, custa praticamente o mesmo que o `flash-lite` e responde melhor — lidera com
o urgente e ainda dá o contexto de corrida. É a escolha.

### A armadilha que custou uma rodada

**O `maxOutputTokens` do Gemini 2.5 é compartilhado com os tokens de raciocínio.** Com 200, o
`flash` gastava ~189 pensando e sobravam sete para responder; as respostas saíam cortadas no
meio — *"Sua última volta foi de um"*, *"Temos bande"*. O sintoma não parece um teto de
tokens: parece o modelo sendo burro.

## Os quatro defeitos no dossiê

Nenhum apareceria em teste unitário, porque todos são sobre **como um leitor entende o texto**.

**1. O formato telegráfico virou outra coisa na boca do modelo.** A linha `Silva (#12), 0,8 s`
produziu *"a oitocentos metros de Silva, o décimo segundo"* — segundos viraram metros e o
número do carro virou posição. O número saiu do dossiê (existe para a ponte com o `driver_id`,
não para ser falado) e as unidades foram escritas por extenso.

**2. O mesmo vizinho aparecia duas vezes**, na forma curta do núcleo e na longa do bloco, porque
a deduplicação era por igualdade exata. Como a curta é prefixo literal da longa, descartar toda
linha que seja prefixo de outra resolve sem que nenhum bloco precise saber do outro.

**3. O dossiê dizia "você é o último na pista" a um carro em quinto de vinte e quatro.** Vizinho
ausente não é ausência de vizinho: o carro de trás some do `cars[]` quando está no box ou fora
do mundo. É uma mentira que o piloto desmente pelo retrovisor — e que contamina a confiança em
tudo o mais que o engenheiro disser depois.

**4. O fato urgente estava na linha 11 de 19 e o modelo não o mencionava.** Num estado com
combustível para seis voltas e oito por correr, a resposta a "como estamos?" falava de posição
e gap. Um leitor dá peso às primeiras linhas; o urgente passou a subir — falta de combustível,
preta, DQ, reparo obrigatório e descasamento de pneu vêm antes do núcleo.

## O conserto que não foi trocar de modelo

Depois dos quatro, sobrou um erro que reaparecia: `a 0,8 segundos` virava *"um e oito"* num
modelo e *"a oitocentos metros"* no outro. Os dois soam perfeitamente verdadeiros ditos em voz
alta, e o piloto não tem como conferir a 200 por hora.

A saída não foi um modelo melhor — foi **tirar a conversão da mão dele**. O gap vai ao dossiê
já falado (`a um e dois de você`), pela [`fala::gap_falado`](../src-tauri/src/engenheiro/fala.rs),
que é a **mesma função que nomeia as 300 peças de gap do acervo**. O modelo copia em vez de
converter.

O efeito colateral vale mais que o conserto: o engenheiro passa a falar o mesmo idioma nos dois
caminhos. A costura entre a peça gravada e a fala gerada é exatamente onde a ilusão se quebra,
e agora as duas dizem o gap com as mesmas palavras.

Ficam como próximos candidatos à mesma conversão os **segundos de reparo** (`18,5 s de box` sai
como "dezoito e cinco"; um engenheiro diria "dezoito e meio") e o **tempo de volta**.

## A varredura de alucinação e o seu ponto cego

O script conta os números da resposta que **não** aparecem no dossiê. Não prova ausência de
alucinação — pega a categoria mais perigosa dela, o número inventado.

E tem um ponto cego conhecido: **números por extenso escapam.** Pedimos ao modelo que escreva
assim, então "um e oito" passou pelas duas medições em que ele estava errado, com o placar
marcando 0/15. A varredura é um alarme barato, não uma prova — e o único jeito de fechar esse
buraco é o que a seção acima descreve: não deixar o modelo escolher a forma do número.

## O orçamento fechado

| etapa | mediana | |
|---|---|---|
| Scribe | 735 ms | medido |
| Gemini (`flash`, sem raciocínio) | 827 ms | medido |
| Cloud TTS | 1.104 ms | medido |
| Ida e volta ao nosso proxy | ~200 ms | estimado |

**Caminho gravado ≈ 0,9 s. Caminho do modelo ≈ 2,9 s.** O primeiro cabe antes da frase de
espera mais curta (1,36 s); o segundo deixa meio segundo de pausa depois da mais longa
(2,36 s), que é o tempo que um engenheiro real leva mesmo.

## Riscos da Parte 5

1. **A qualidade foi julgada por leitura, não por métrica.** Cinco casos, dois modelos, três
   repetições. É audição, não estatística.
2. **Nenhum dos dois modelos é confiável nos dois eixos ao mesmo tempo.** Antes dos consertos,
   o `flash-lite` acertava a urgência e errava o número; o `flash` fazia o inverso. A lição é
   estrutural: o que der para tornar determinístico, torne — a escolha de modelo é a última
   alavanca, não a primeira.
3. **A instrução do engenheiro vive no script de medição**, não no servidor. Quando o endpoint
   existir, ela precisa ir junto — e as duas cópias vão poder divergir.
4. **`thinkingBudget: 0` é um campo que a documentação atual não descreve** (ela fala em
   `thinking_level`). Funciona hoje; pode deixar de funcionar sem aviso, e o sintoma seria a
   latência quintuplicar em silêncio.
