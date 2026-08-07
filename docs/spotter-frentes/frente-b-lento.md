# Frente B — carro lento à frente

> **Leia `docs/spotter-frentes/CONTRATO.md` primeiro.** Ele tem a regra de propriedade, os
> comandos, e tudo que já foi medido. Este documento só cobre o que é seu.

Você cria **`src-tauri/src/iracing_sdk/spotter_lento.rs`** e adiciona a linha
`pub mod spotter_lento;` no `mod.rs`. Nada mais.

## Por que esta frente existe

O spotter já sabe dizer que há um carro **fora da pista** e que há um carro **parado**. Não
sabe dizer que há um carro **andando, na pista, muito mais devagar que o resto** — que é o
caso mais comum dos três e o que mais causa fechada de trajetória em curva cega.

É também o mais difícil dos três, e por um motivo específico: os outros dois são leituras
de estado (`TrackSurface == 0`, velocidade ≈ 0). Este é um limiar num contínuo.

## Por que o limiar não pode ser absoluto

Medido em Okayama, com a IA obedecendo a bandeira amarela de verdade:

| | p10 | mediana | p90 |
|---|---|---|---|
| **Verde** | 75 | **111** | 161 km/h |
| **Amarela** | 56 | **78** | 122 km/h |

A mediana cai 30% sob amarela. Um limiar absoluto calibrado no verde não dispara nunca sob
amarela; calibrado na amarela, dispara em todo mundo no verde. **A referência tem de ser o
campo naquele instante**, e essa normalização também resolve de graça: pista molhada,
categorias diferentes, trecho lento do traçado, primeira volta.

A referência que funcionou nas análises é a **mediana da velocidade dos carros na pista**
(`track_surface == 3`), fora do pit road, com velocidade derivada válida. A mediana é
robusta a um outlier — importante, porque o outlier pode ser o próprio jogador.

Isso não é hipotético: na captura de Okayama o jogador rodou a 22 km/h contra um campo de
133 por quase quatro minutos. **Exclua o jogador do cálculo da referência.**

## O que decidir com dados, não por gosto

1. **O corte.** Que fração da mediana do campo separa "mais lento" de "muito mais lento"?
   O detector de obstáculo usa 60% do pico do *próprio carro*; aqui a referência é outra e
   o número tem de sair das duas capturas, não por analogia.
2. **Permanência mínima.** Uma freada forte numa curva lenta derruba a razão por um
   instante. Diferente do detector de obstáculo — onde o piso de permanência foi testado e
   **não ajudou** —, aqui ele provavelmente é necessário, porque o sinal é ruidoso por
   natureza. Meça, não presuma: o resultado contrário já aconteceu uma vez neste projeto.
3. **A janela de aviso.** Comece pela mesma que foi calibrada para a frente — 2 a 5 s de
   chegada, no máximo 200 m — e verifique se serve. Um carro lento não some da frente como
   uma escapada some, então a janela pode ser mais generosa. Prove.
4. **A sobreposição com as outras famílias.** Um carro parado é, tecnicamente, um carro
   muito lento. Um carro voltando da grama também. Se o `spotter_lento` disparar por cima
   deles, o piloto ouve duas coisas sobre o mesmo carro. Decida como se excluem — e note
   que você **não pode** consultar o estado do `spotter_frente` (é de outra frente); então
   ou o critério é interno e observável (superfície, velocidade absoluta), ou você deixa a
   sobreposição documentada para a integração resolver na tabela de prioridade.

## Os falsos positivos que já sabemos existir

Todos já morderam o detector irmão. Confira que o seu não cai neles:

- **A largada parada.** No verde os 40 carros estão a 0 km/h com `SessionState` já em
  Correndo. Use a regra do CONTRATO: só conta quem passou de 50 km/h nos últimos 10 s.
- **Pit road e entrada de box.** Carros lá são lentos por projeto.
- **Formação e volta de apresentação.** `SessionState` 3, e `PaceMode` / `CarIdxPaceFlags`
  existem se precisar.
- **Carros que sumiram do array.** Um guinchado deixa estado aberto para sempre se o seu
  laço só visitar quem está presente.
- **O trem de volta 1.** Medido: numa parada em trem, o campo inteiro passou entre 1 e 7
  metros do carro parado, e o perseguinte seguinte estava a 788 m. Não há ninguém na faixa
  de 100–200 m. Seu detector não vai ter o que fazer ali, e isso é aceitável — só não
  invente aviso.

## O que medir e reportar

Sobre as **duas** capturas, simulando cada carro da IA como jogador:

- Avisos por piloto por corrida. Referência: a família "fora da pista" dá **0,38**, e
  nenhum piloto ouve duas vezes. Se o seu der 5, está errado mesmo que cada um esteja certo.
- Taxa de inúteis pela métrica do CONTRATO ("ainda havia problema na chegada?").
  Referência: 35% para "fora da pista", 16% para "parado".
- Quantos dos seus avisos são sobre carros que as outras duas famílias já pegariam
  (superfície fora da pista, ou abaixo de 5 km/h). É o número que diz se a família se
  justifica sozinha.

## Entregável

O módulo, os testes, as chaves de fala propostas com redação, e os números acima.

Sobre a redação — o tom do pacote é curto e descreve a situação agora, não o
acontecimento. E **texto de UI e de fala nunca começa em minúscula**; é um vício recorrente
neste projeto. Vale considerar se "lento" merece grau ("Carro lento à frente" contra
"Carro muito lento à frente"), mas cada grau é mais uma decisão de limiar que precisa sair
dos dados.
