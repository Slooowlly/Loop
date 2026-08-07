# Frente A — carro chegando por trás

> **Leia `docs/spotter-frentes/CONTRATO.md` primeiro.** Ele tem a regra de propriedade, os
> comandos, e tudo que já foi medido. Este documento só cobre o que é seu.

Você cria **`src-tauri/src/iracing_sdk/spotter_tras.rs`** e adiciona a linha
`pub mod spotter_tras;` no `mod.rs`. Nada mais.

## Por que esta frente existe

O spotter do Loop cobre o **lado** (`CarLeftRight`, um canal pronto do SDK) e a **frente**
(`spotter_frente.rs`, deduzido). Atrás não há nada. E foi exatamente ali que aconteceu o
único acidente que temos gravado com telemetria completa.

O que a captura de Okayama mostra, medido:

O jogador saiu do box em `t=858 s` e rodou de 861 a 903 a **17–53 km/h em marcha 1**,
enquanto a mediana do campo era **133 km/h**. Ficou abaixo de 60% do campo por 238
segundos seguidos. O `#37`, a 113 km/h — ritmo perfeitamente normal, 84% do campo —, veio
por trás e o acertou em `t=905,45 s`. Os incidentes do jogador foram de 0 a 4.

Aviso não faltava:

| Antes da batida | `#37` atrás | Fechando | Chegaria em |
|---|---|---|---|
| −18 s | 578 m | 28 m/s | 20,5 s |
| −10 s | 275 m | 45 m/s | 6,1 s |
| **−3,7 s** | **91 m** | **19 m/s** | **4,8 s** |
| −1 s | 24 m | 28 m/s | 0,9 s |

Ele entrou na mesma janela de 2–5 s que calibramos para a frente **3,7 segundos antes do
impacto, a 91 metros**. E a **bandeira azul** disparou duas vezes: em `t=899,6` e
`t=903,5` — 5,9 s e 2,0 s antes. Ninguém leu nenhum dos dois sinais.

## A armadilha, e ela é o projeto inteiro

Nos mesmos 57 segundos em que o jogador rodava lento, **17 carros diferentes** entraram na
janela de 2–5 s por trás dele:

```
t=893.5  #36 a 199 m atrás, fechando 45 m/s → chega em 4.5 s
t=901.7  #37 a  91 m atrás, fechando 19 m/s → chega em 4.8 s
t=903.8  #33 a  99 m atrás, fechando 20 m/s → chega em 4.9 s
t=904.8  #32 a 101 m atrás, fechando 21 m/s → chega em 4.9 s
...      (mais treze)
```

**Um aviso por carro seria uma metralhadora — pior que silêncio.** Espelhar o detector da
frente é a solução errada, e é a primeira que vem à cabeça.

O que a situação pede é um **estado sustentado**, não uma sequência de chamadas: "estão
vindo por trás, deixa passar", entrando uma vez, lembrando de vez em quando enquanto dura,
e liberando quando passa. É a mesma forma do spotter lateral — entrada, permanência,
liberação — e não a do de obstáculo.

A bandeira azul (`session_flags & 0x20`) já resume os 17 num sinal só. É literalmente
para isso que ela existe.

## O que decidir com dados, não por gosto

1. **Quando entrar.** A azul sozinha basta? Ela disparou a 5,9 s e a 2,0 s do impacto, mas
   também piscou e apagou (`899,6` liga, `901,7` desliga, `903,5` liga, `911,0` desliga) —
   vai precisar de histerese como o resto do sistema. E ela só existe quando o sim decide
   mostrá-la; um carro muito mais rápido chegando sem azul (mesma volta, disputa de
   posição) é outro caso, e talvez outro aviso.
2. **Separar "estão te passando" de "disputa normal".** Um carro 2 s atrás numa briga de
   posição não é notícia — quando ele chegar ao seu lado, o spotter lateral fala. O que é
   notícia é quem chega **muito mais rápido**. A taxa de fechamento é o candidato óbvio, e
   você tem duas corridas para achar o corte.
3. **Uma ou duas famílias?** "Carro chegando" e "deixa passar / azul" podem ser a mesma
   coisa ou coisas diferentes. Decida e justifique.

## O que medir e reportar

Sobre as **duas** capturas, simulando cada carro da IA como jogador:

- Quantas vezes o detector entraria em estado, por piloto por corrida.
- Quanto tempo cada estado dura.
- Quantas falas sairiam no total (entrada + permanências).
- E o caso-teste: **rodando com o jogador real (`idx 0`) na captura de Okayama, o seu
  detector teria avisado antes de `t=905,45`? Com quanto tempo de sobra?** É a pergunta
  que justifica a frente existir.

Cuidado com o falso positivo simétrico ao do grid: no pit road, na formação e antes da
largada há carros "chegando por trás" o tempo todo. Use a regra do CONTRATO.

## Entregável

O módulo, os testes, as chaves de fala propostas com redação, e os números acima.

Sobre a redação — o tom do pacote é curto e descreve a situação agora, não o
acontecimento. Compare: `"Carro fora da pista logo à frente."` (bom) contra `"Um carro
saiu."` (narra o passado). E **texto de UI e de fala nunca começa em minúscula** — é um
vício recorrente neste projeto.
