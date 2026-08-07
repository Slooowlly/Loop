# Pacote de voz — o que precisa ser gerado

Levantamento feito em 04/08/2026, lendo o código, não a memória. Decide **o que**
gravar antes de gravar — porque a conta que importa não é o preço (é irrisório, ver o
fim), é o tempo de audição: cada tomada precisa ser ouvida por alguém antes de virar
produto, e é isso que não escala.

Pré-requisitos já fechados em [`tts-poc-latencia.md`](tts-poc-latencia.md): voz
`pt-BR-Chirp3-HD-Algenib`, cadeia de rádio, colagem de peças, tempo de volta fundido.
O primeiro pacote (spotter, 20 gravações) já está em produção.

## A primeira separação: o que é FALADO

Nem todo texto em português do Loop é candidato a virar áudio. A divisa é o tempo:

- **Falado** — chega ao jogador enquanto ele dirige, de capacete, sem poder ler.
- **Escrito** — ele lê quando quiser, na tela do app.

Isso derruba um alvo que parecia óbvio. O bloco `breakdown:` de
[`src-tauri/locales/pt-BR.yml`](../src-tauri/locales/pt-BR.yml) tem 148 strings (50
`dnf` + 44 `warn` + 54 `part`, sendo 40/44/41 distintas) e a POC o apontava como
inventário. Mas quem o consome é [`simulation/catalog.rs`](../src-tauri/src/simulation/catalog.rs):
é o texto das corridas **simuladas**, as que o jogador não correu. Ninguém está de
capacete quando isso aparece. **Fora do pacote de voz.**

O rádio ao vivo tem outra fonte, e é essa que conta:
[`commands/overlay/radio.rs`](../src-tauri/src/commands/overlay/radio.rs) e
[`avisos.rs`](../src-tauri/src/commands/overlay/avisos.rs).

## O inventário

| # | Família | Fonte | Peças | Estado |
|---|---|---|---|---|
| 1 | Spotter — proximidade (entrada, permanência, liberação) | `iracing_sdk/spotter.rs` | **20** | ✅ gerado |
| 2 | Spotter — repertório restante | seis módulos `spotter_*.rs` | **31** | ✅ gerado |
| 3 | Push-to-talk do engenheiro | `engenheiro/fala.rs` | **480** | ✅ gerado |
| 4 | Quebra na grade (nomeia o piloto) | `engenheiro/quebra.rs` | **579** | ✅ gerado |
| 5 | O nosso carro (peça em risco + desfecho + poupar) | `engenheiro/peca_propria.rs` | **147** | ✅ gerado |
| 6 | Tempo de volta (fundido, 30,0 s a 4:00,0) | `engenheiro/tempo_volta.rs` | **2101** | ✅ gerado |
| 7 | Rádio de ritmo (volta mais rápida, aproximação) | `engenheiro/ritmo.rs` | **14** | ✅ gerado |
| | | | **3.388 peças, 257 MB** | |

O item 5 cresceu de 39 para 147 depois da medição do rádio inteiro
([`radio-carga.md`](radio-carga.md)): a quebra do carro do JOGADOR saía pelo rádio da grade, em
3ª pessoa e sobre ele mesmo, 1,3 vez por corrida. O desfecho em 2ª pessoa custou 108 gravações —
12 peças × 3 severidades × 3 redações.

**O acervo está fechado para tudo o que tem forma fixa.** O que sobra é, por desenho, o
caminho do modelo: `Intencao::Geral` (a pergunta aberta) e `Intencao::Carro` (mistura segundos
de reparo com peças avariadas e não tem forma). Não são buracos — são a divisão de trabalho.

O item 4 absorveu o antigo "nomes de piloto": a decisão foi (b), só os sobrenomes. Os 419
primeiros nomes continuam sem gravação e **sem consumidor** — nada no app os pede.

### A faixa do tempo de volta foi corrigida por medição

O desenho original eram 1.201 peças cobrindo 0:00,0 a 1:59,9. A faixa estava errada: medida na
tabela de tempos base (618 entradas em `simulation/profile/lap_times.rs`), a volta vai de
30,0 s a 703,0 s, e **18,3% delas passam de dois minutos**.

Partir o minuto para caber (`"um,"` + `"trinta e dois e quatro."`, 611 peças em vez de 6.900)
foi medido e **reprovado de ouvido**: a peça `"um,"` sozinha sai com 0,40 s para uma sílaba,
com acento pleno, porque é assim que o modelo lê um monossílabo isolado. A montagem ficou 22%
mais longa no caso comum e **91% no pior**. Voltou a ser fundido, com a faixa
**30,0 s a 4:00,0** — 97,4% do calendário. Os 2,6% de fora são Nordschleife, Spa e Le Mans de
enduro.

> **O item 2 encolheu de ~205 para 31, e isso é o resultado principal desta etapa.** O
> desenho original listava dez grupos por dedução de "o que o SDK permite". Medidos contra
> três corridas gravadas, a maioria não se sustentou: ou o volume era ruído (retardatário
> daria 8 avisos por piloto, com um ouvindo 73 vezes), ou a família não disparava em
> corrida verde (`lento`), ou o dado não existe no canal esperado (amarela por setor). O
> que ficou são sete famílias, todas com taxa medida antes de existir gravação. Ver
> `docs/spotter-obstaculo.md`.

### 1. Spotter — proximidade ✅

Três fases: ENTRADA (`esquerda`, `direita`, `duas_esquerda`, `duas_direita`,
`tres_largos` ×3), PERMANÊNCIA (`ainda_esquerda` ×3, `ainda_direita` ×3, `ainda_ai` ×3)
e LIBERAÇÃO (`livre_esquerda`, `livre_direita`, `livre`), mais o `teste`. Em
`src/assets/spotter/`, 1 MB. Provado em pista: o
`CarLeftRight` **é** preenchido em sessão offline com IA.

Duas lições que valem para todas as famílias seguintes:

- **Termo em inglês precisa ser escrito pelo SOM.** `"Three wide"` saiu ruim nas três
  tentativas literais; `"Tri uáide, cuidado."` saiu ótimo. A voz é pt-BR e lê como tal.
- **Situação que DURA precisa das três fases.** Anunciar a entrada e calar deixa o piloto
  sem saber se o outro saiu ou se o spotter parou de olhar; e a liberação precisa nomear
  O LADO, senão é ambígua justamente quando ele decide o movimento.
- **Toda chegada anunciada precisa de uma saída anunciada.** Parece óbvio e não é: a
  primeira versão tinha um mínimo de duração que só conseguia calar a SAÍDA (a entrada
  confirma em 0,12 s e já tinha falado), deixando o piloto a segurar linha por um carro
  que já fora embora. Qualquer família com estado — bandeira, incidente à frente, chuva —
  tem a mesma armadilha: o filtro anti-ruído precisa valer para as duas pontas ou para
  nenhuma.

### 2. Spotter — o repertório restante ✅

O desenho original desta seção listava dez grupos e ~205 peças, deduzidos de "o que o SDK
permite". A etapa seguinte mediu cada um contra três corridas gravadas, e o resultado foi
**31 peças em sete famílias** — com duas recusas e uma prateleira.

| Família | Módulo | Falas | Taxa medida (por piloto/corrida) |
|---|---|---|---|
| Obstáculo à frente — fora da pista | `spotter_frente.rs` | 3 | 0,38 a 0,63 |
| Obstáculo à frente — parado | `spotter_frente.rs` | 3 | 0 a 1,35 |
| Carro saindo dos boxes | `spotter_boxe.rs` | 3 | 0,46 a 0,56 |
| Tráfego por trás (estado) | `spotter_tras.rs` | 7 | 0,10 |
| Retorno à pista | `spotter_voltar.rs` | 4 | 0,15 |
| Bandeiras + relargada | `spotter_bandeira.rs` | 8 | ~0,2 |
| Clima | `spotter_clima.rs` | 3 | ~0,01 |

**O que NÃO entrou, e por quê** — vale mais que a lista acima, porque é o que impede
alguém de refazer o trabalho:

- **Retardatário à frente** — recusado. Daria **8,02 avisos por piloto**, com um piloto
  ouvindo **73 vezes**, contra 1,24 de todas as famílias somadas. E o problema não é
  calibração: estar uma volta atrás é fato de classificação, não evento de perigo.
- **Carro lento à frente** (`spotter_lento.rs`) — construído, calibrado e **engavetado**.
  Zero avisos em 17 min de corrida limpa; 100% dos avisos sob amarela. O módulo fica no
  repositório até haver corrida verde com caso.
- **Bandeira preta** — retirada. No Loop ela é o nosso mecanismo de dano (`!black #N` do
  `car::breakdown`), não punição do sim.
- **Amarela por setor** — impossível. Medido: `CarIdxSessionFlags` só carrega preta e
  reparo; a amarela existe apenas no bitfield global.
- **Posição, voltas restantes, tempo restante, distância para o carro** — não são spotter.
  São número montado em tempo real, e pertencem ao rádio do engenheiro (item 3).

A decisão sobre a **resolução da distância** (0,1 s = 101 peças contra 0,5 s = 21) segue
aberta e migra para o item 3, que é onde ela agora vive.

### 3, 4 e 5. O acervo do engenheiro — 1.101 peças ✅

Feito, e diferente do previsto aqui em dois pontos que valem registro.

**A conta era 105 e virou 1.101.** O desenho original tratava o rádio de quebra como 105
trechos colados a um nome. Ele é isso, mas o nome custa: 355 sobrenomes e 102 equipes, que
sozinhos são 83% do acervo. O item 4 (`engenheiro/quebra.rs`) reúne os dois — os trechos e os
nomes que eles pedem —, porque separá-los na contagem escondia o que domina o custo.

**A decisão dos nomes foi (b), só o sobrenome.** Os 419 primeiros nomes não foram gravados e
não têm consumidor. O que fechou a escolha não foi a contagem e sim a emenda: sobrenome é uma
peça única, nome completo teria colagem DENTRO do nome próprio — o único lugar onde a POC não
validou emenda.

**A forma pela equipe sobreviveu mesmo com os 355 gravados**, por dois motivos que só
apareceram construindo. Um é de registro: não nomear o 19º é o que diz ao jogador, sem dizer,
que ele não precisa se importar. O outro é técnico: o nome do JOGADOR não sai de pool nenhum,
então sem essa forma a fala morreria em vez de degradar.

| Família | Módulo | Peças |
|---|---|---|
| Push-to-talk (posição, gap, voltas, pneu, combustível, bandeira, pista) | `fala.rs` | 480 |
| Quebra: sobrenomes | `nomes.rs` | 355 |
| Quebra: equipes (nome falado, sem o sufixo genérico) | `nomes.rs` | 102 |
| Quebra: trechos por peça × severidade | `quebra.rs` | 108 |
| Quebra: enquadramento (vínculo, aposto, coda, atrito) | `quebra.rs` | 14 |
| O nosso carro: peça em risco | `peca_propria.rs` | 36 |
| O nosso carro: conselho de poupar | `peca_propria.rs` | 3 |
| Emprestadas do spotter (fora do catálogo, para não gerar duas tomadas) | — | 3 |

Duas medições saíram daqui e mudaram o produto:

- **Travessão parte a fala.** As 13 peças com travessão saíram com 0,35 s de silêncio DENTRO
  da gravação, contra 0,01 s das 106 de oração corrida. Trinta e cinco vezes. Reescritas.
- **~33 falas de quebra por corrida** (grid de 24 em 18 voltas, 300 corridas simuladas em
  `car::breakdown::medicao`), das quais ~4 são abandonos. É o teto de tagarelice do rádio, e
  está num teste nomeado para quem quiser apertar.

### 6. Tempo de volta — 1201 peças

Já desenhado e decidido: peça **fundida**, `"um trinta e dois e quatro."` num arquivo
só. 2 minutos × 60 segundos × 10 décimos + a peça de abertura. Testado contra a
decomposição por dígitos e contra a versão qualitativa; as duas foram rejeitadas de
ouvido.

É a família mais numerosa e a mais mecânica — nenhuma variação de redação, nenhum
julgamento por peça. Boa candidata a ser a primeira do gerador em lote, justamente
porque erra pouco.

## O que isso custa

| | Caracteres | US$ |
|---|---|---|
| Spotter completo (1 + 2) | ~2.500 | — |
| Rádio do engenheiro (3) | ~3.150 | — |
| Nomes, opção (b) | ~2.100 | — |
| Tempo de volta (5) | ~30.000 | — |
| **Total** | **~38.000** | **US$ 0,00** |

A camada gratuita da Cloud TTS é de 1M de caracteres por mês. **O pacote inteiro cabe
em 4% de um único mês grátis** — e caberia cinco vezes se tudo fosse regerado do zero.
Preço não é restrição aqui e não deve entrar em nenhuma decisão de escopo.

As restrições reais são outras três:

1. **Tempo de parede.** ~1.900 chamadas a ~1,2 s cada = **~40 minutos** em série. O
   limite é 200 RPM, então paralelizar em 4 põe isso em ~10 minutos. O gerador precisa
   ser retomável — perder 40 minutos por um 429 no arquivo 1.100 seria absurdo.
2. **Tamanho em disco.** ~1.900 arquivos × ~0,8 s em WAV 24 kHz ≈ **73 MB**. Aqui o
   Opus 24 kbps que a POC mediu vale a pena de verdade: cai para ~4,5 MB. (Nas 7 falas
   do spotter não valia — o decodificador custaria mais que os arquivos.)
3. **Audição.** É o gargalo verdadeiro. 1.900 tomadas a 1 s cada são 30 minutos só de
   reprodução, sem contar julgar. Precisa de triagem automática antes: duração fora da
   faixa, silêncio interno, pico anômalo — os alarmes que o
   [`spotter-pack.mjs`](../scripts/spotter-pack.mjs) já tem, e que precisam sair para
   um relatório em vez de linhas no console.

## Ordem sugerida

1. ~~**Spotter, repertório restante.**~~ ✅ Feito, e virou 31 em vez de ~205 — a medição
   cortou mais do que o desenho previa. A fala foi mesmo a parte barata; o caro foi provar
   quais famílias merecem existir.
2. **Rádio do engenheiro (105) + sobrenomes (355).** Liga uma feature que já existe e
   hoje é só texto na tela. Volume ainda humano de ouvir.
3. **Tempo de volta (1201).** Por último, porque é o único que exige o gerador em lote
   com retomada e triagem automática — e porque não adianta ter 1201 tempos de volta se
   o engenheiro ainda não abre a boca.

## O que ainda está em aberto

- **Nome completo ou sobrenome** (seção 4). Muda 774 para 355 e elimina uma emenda ruim.
- **Resolução da distância** (seção 2). 0,1 s são 101 peças; 0,5 s são 21.
- **Chirp 3 Instant Custom Voice** (US$ 60/1M, 30 RPM, sem camada gratuita). Com 1.900
  tomadas geradas ao longo de dias, a deriva de timbre deixa de ser um detalhe. É o
  único obstáculo técnico que a POC deixou sem resposta.
