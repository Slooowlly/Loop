# Frente C — arreio de validação

> **Leia `docs/spotter-frentes/CONTRATO.md` primeiro.** Ele tem a regra de propriedade, os
> comandos, e tudo que já foi medido. Este documento só cobre o que é seu.

Você cria **`scripts/analise-spotter.mjs`**. Nada mais. Nenhum arquivo Rust, nenhum
arquivo do front.

## Por que esta frente existe

Todo número que sustenta o spotter hoje — a janela de 2 a 5 segundos, o limiar de 40% de
perda de ritmo, a taxa de 0,38 avisos por piloto, a descoberta de que a largada parada
produz 40 falsos positivos — saiu de scripts descartáveis, escritos à mão, rodados uma vez
e perdidos. Nenhum deles está no repositório.

A consequência prática: **não dá para revalidar nada.** Quando as frentes A e B
entregarem, e quando chegar a terceira captura, a única forma de conferir é reescrever os
scripts. E o custo disso já apareceu — uma métrica mal escolhida (contar fechamento de
episódio em vez de utilidade real) produziu uma recomendação errada que só foi pega na
corrida seguinte.

Você transforma isso em ferramenta.

Existe um precedente parcial: `scripts/analise-frente.mjs`. Ele lê capturas e reporta
saúde de canais e episódios, mas **não** reproduz os detectores nem mede utilidade de
aviso. Leia-o, aproveite o que servir, e não o edite — ele é de outra frente.

## O que a ferramenta faz

```bash
node scripts/analise-spotter.mjs <captura.jsonl.gz | pasta>
```

Para cada captura:

**1. Reproduz os detectores.** Roda a lógica de detecção sobre os quadros e lista os
episódios: tipo, instante, duração, pico anterior, mínima, materiais pisados, desfecho.

**2. Simula cada carro da IA como jogador.** As duas capturas têm o jogador parado no box
quase o tempo todo, então a via do aviso nunca é exercitada por ele. Tratar cada um dos 40
carros como jogador hipotético é o que transforma duas corridas numa amostra utilizável —
foi assim que se chegou aos 15 avisos de Lime Rock e aos 0,38 por piloto.

**3. Mede utilidade, não fechamento.** Para cada aviso, olhe o instante da chegada
(`t + tempo_até_chegar`) e pergunte: **ainda havia problema ali?** A definição que se
provou correta:

```
ainda_problema =
    o carro está presente no array
    E não está no pit road
    E ( está fora da pista
        OU ( está na pista E abaixo de 70% do próprio pico recente ) )
```

Guinchado e no box contam como **não**-problema: o carro saiu do caminho. A primeira
versão desta métrica contava "o episódio já fechou" e estava errada — um episódio fecha
quando o carro volta à pista, e um carro voltando da grama, lento, na trajetória, é
exatamente o perigo que o aviso descreveu.

**4. Varre parâmetros.** Uma tabela de piso de permanência × teto de velocidade contra
avisos / úteis / inúteis. Foi essa varredura que mostrou que o piso de permanência **não
ajuda** na família "fora da pista" (0,5 s derruba de 40 para 27 avisos e a taxa piora), o
que contrariou a recomendação anterior. Deixe isso fácil de repetir.

**5. Taxa-base.** Percentual do tempo-carro em pista com obstáculo dentro de 100, 150, 200
e 300 m. Referência medida: 0,106% a 100 m em Lime Rock.

## A parte que evita o pior defeito

O arreio é um **espelho em JS** de detectores escritos em Rust. Espelhos derivam, e um
espelho derivado é pior que nenhum: ele valida algo que não é o que roda.

**Extraia as constantes do próprio fonte Rust, por leitura de texto.** É a técnica que a
suíte `scripts/tests/*.test.mjs` já usa — ela lê o código como texto para pegar regressão
visual. Aqui a aplicação é a mesma: leia `JANELA_VEL_S`, `PICO_JANELA_S`, `PICO_MIN_KMH`,
`FRACAO_RITMO`, `TTA_MIN_S`, `TTA_MAX_S`, `DIST_MAX_M`, `PARADO_KMH`, `AUSENCIA_MAX_S` de
`spotter_frente.rs` em vez de copiá-las. Se alguma sumir ou mudar de nome, **falhe alto**
em vez de usar um padrão silencioso.

Se conseguir fazer o mesmo para os módulos das frentes A e B quando existirem, melhor —
mas eles ainda não existem, então projete para que acrescentá-los seja acrescentar uma
entrada numa tabela, não reescrever o laço.

## Armadilhas de leitura das capturas

- **Descompacte inteiro, não em pipe.** As capturas são truncadas quando o app fecha e um
  `createGunzip()` em pipe estoura com `Z_BUF_ERROR: unexpected end of file`:
  ```js
  zlib.gunzipSync(fs.readFileSync(p), { finishFlush: zlib.constants.Z_SYNC_FLUSH })
  ```
- **`cars[]` não vem em todo quadro** — só a 20 Hz, e só em corrida. Quadros sem ele não
  são falha.
- **O array encolhe.** Um carro guinchado some dele por dezenas de segundos. Se você
  indexar por `cars.length` do primeiro quadro, vai ler o mundo errado: no início da
  sessão o array tem 1 carro. Use o máximo de `idx` visto em toda a captura. *(Este erro
  já foi cometido e escondeu 44 de 45 episódios.)*
- **`TrackLength`** sai do YAML (`{kind:"session"}`), no formato `2.37 km`.
- **Há uma captura truncada e inútil** (`race_1785887784`, 243 s, zero episódios em
  corrida). Não trave nela; reporte e siga.

## Verificação da sua própria entrega

O arreio está certo quando reproduz os números já publicados em
`docs/spotter-obstaculo.md`. Confira pelo menos estes:

| Sobre | Esperado |
|---|---|
| Lime Rock, episódios "fora da pista" | **3** (11,80 s / 4,18 s / 0,20 s) |
| Lime Rock, episódios "parado" | **0** |
| Okayama, episódios "parado" | **4** (19,70 / 7,98 / 4,65 / 4,18 s) |
| Okayama, `#25` e `#20` | desfecho "sumiu do array", **não** 150 s de duração |
| Duas corridas, avisos "fora da pista" | 40, com **35%** inúteis |
| Duas corridas, avisos "parado" | 51, com **16%** inúteis |

Se algum não bater, é o arreio que está errado — ou você achou um erro nos números
publicados, o que também é uma entrega válida, desde que venha com a evidência.

## Entregável

O script, e a saída dele rodando sobre as duas capturas. Sem chaves de fala nesta frente.

Uma nota de escopo: **não** transforme isto num teste do `npm run test:structure`. As
capturas moram em `%APPDATA%` e não estão no repositório; um teste que depende delas
quebra em qualquer máquina limpa e no CI. É uma ferramenta de análise, rodada à mão.
