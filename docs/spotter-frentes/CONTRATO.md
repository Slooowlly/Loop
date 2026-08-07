# Frentes do spotter — o contrato comum

Três frentes trabalham em paralelo para fechar as lacunas do spotter. **Leia isto antes
do seu briefing.** O que faz o paralelismo funcionar é a regra de propriedade abaixo; sem
ela as três colidem nos mesmos quatro arquivos.

## A regra de propriedade

> **Cada frente é dona de exatamente UM arquivo novo. Não edita nenhum arquivo existente.**

| Frente | Arquivo que você cria | Assunto |
|---|---|---|
| A | `src-tauri/src/iracing_sdk/spotter_tras.rs` | carro chegando por trás, bandeira azul |
| B | `src-tauri/src/iracing_sdk/spotter_lento.rs` | carro lento à frente, relativo ao campo |
| C | `scripts/analise-spotter.mjs` | arreio de validação sobre capturas reais |

**Não toque nestes**, mesmo que pareça óbvio — eles são integrados por quem coordena, e
uma edição sua aqui vira conflito garantido:

- `src/lib/spotterVoice.js` (tabela `PRIORIDADE`)
- `scripts/spotter-pack.mjs` (redações das falas)
- `src-tauri/src/iracing_sdk/spotter.rs` (fiação e fila de eventos)
- `src-tauri/src/iracing_sdk/mod.rs` (declaração do módulo)
- `scripts/tests/spotter-chaves-contrato.test.mjs` (guard de chaves)
- `docs/spotter-obstaculo.md`

Frentes A e B: seu módulo **não** precisa estar declarado no `mod.rs` para você
trabalhar. Compile e teste com `cargo test --lib` apontando direto para o arquivo? Não dá
— o Rust exige a declaração. Então **adicione a linha `pub mod spotter_<seu>;` no
`mod.rs`, e só ela**. É a única exceção, é uma linha, e o merge dela é trivial.

Nenhuma frente registra comando em `lib.rs`. Nada disto é exposto ao front nesta etapa.

## O que entregar

1. O arquivo, completo e testado.
2. **A lista de chaves de fala que o seu detector emite**, com a redação que você propõe
   para cada uma e as variações (`chave`, `chave_2`, `chave_3`). Não crie os `.wav` nem
   edite o gerador — só proponha o texto.
3. **Os números medidos**: quantas vezes o seu detector dispararia por piloto por corrida,
   nas duas capturas reais. Sem isso a entrega não está pronta.

## O projeto

**Loop** — jogo de carreira no automobilismo construído em volta do iRacing. Tauri v2
(Rust) + React 18. Ver `CLAUDE.md` na raiz.

**O código, os comentários e os testes são em português.** Mantenha o padrão. Comentários
explicam *por quê*, não *o quê* — leia `spotter_frente.rs` e imite o tom.

### Comandos

```bash
npm run build      # OBRIGATÓRIO antes de qualquer cargo — o generate_context! embute dist/
```

Cada frente usa um `CARGO_TARGET_DIR` próprio, senão três `cargo` simultâneos dão
`LNK1104: cannot open file`:

```bash
# frente A
CARGO_TARGET_DIR=C:/cargo-target/loop-a cargo test --manifest-path src-tauri/Cargo.toml --lib spotter_tras
# frente B
CARGO_TARGET_DIR=C:/cargo-target/loop-b cargo test --manifest-path src-tauri/Cargo.toml --lib spotter_lento
```

## O que já foi medido — não redescubra

`docs/spotter-obstaculo.md` é o registro. Os fatos que mais importam:

- **Não existe `CarIdxSpeed`.** A velocidade dos outros carros sai de
  `Δlap_dist_pct × comprimento_da_pista / Δt`. Janela de 0,25 s. Erra 2 km/h na mediana.
- **Distância envolvida**: `d = pct_alvo − pct_jogador; if (d<0) d += 1; metros = d × comprimento`.
  Para distância com sinal (à frente positiva, atrás negativa), normalize para meia volta.
- **`SessionState == 4` (Correndo) não protege da largada parada.** No verde os 40 carros
  estão a 0 km/h, em asfalto, na pista, com o estado já em corrida. A regra que resolve é
  *"só conta quem estava andando"*: o carro precisa ter passado de 50 km/h nos últimos 10 s.
- **Um carro guinchado some do array `cars[]`** por dezenas de segundos e reaparece com
  `on_pit_road`. `track_surface == -1` nunca chega até você — nosso próprio leitor filtra.
  Qualquer laço que só visite carros presentes deixa estado aberto para sempre.
- **`CarIdxSessionFlags` só serve para preta e reparo.** Nenhum bit de amarela chega ao
  canal por carro; a amarela existe só no `session_flags` global.
- **`CarIdxF2Time` é inservível** (23–58% zerado). `CarIdxEstTime` é populado e coerente.
- **A faixa útil de aviso é 100–200 m, ou 2 a 5 segundos.** Avisar a 12 s é avisar de algo
  que já terá saído da frente. Os casos mais próximos (obstáculo a 3 m) não são avisáveis,
  e aceitar isso é parte do projeto.

## A doutrina que não se negocia

**Nunca descarte um aviso para preservar cadência — no máximo adie.** Este projeto já
cometeu esse erro uma vez: havia um intervalo mínimo entre falas que *descartava* o
evento, e o "três largos" sumia 120 ms depois do "esquerda", deixando o piloto sem saber
do carro que apareceu do outro lado. Se o seu detector decide falar e o momento não é bom,
ele mantém o aviso pendente e tenta de novo no tick seguinte. Quem arbitra quem
interrompe quem é a camada de voz, não o detector.

Corolário prático: **só marque um aviso como dado quando ele virou fala de verdade.** Veja
`ObservadorFrente::confirmar_aviso` — o detector devolve a chave, e um método separado
confirma. Imite.

## O módulo a imitar

`src-tauri/src/iracing_sdk/spotter_frente.rs` é o irmão do que você vai escrever. Copie a
estrutura:

- Uma máquina **pura**: recebe uma `Amostra*` com os dados do tick e devolve no máximo uma
  chave por chamada. Sem `AppHandle`, sem I/O, sem relógio de parede — o tempo vem do
  `session_time`. É isso que torna a coisa testável.
- Uma fachada no fim do arquivo com o singleton (`OnceLock<Mutex<...>>`) e as funções
  livres. Não a fie no amostrador — quem faz isso é a integração.
- Um harness de teste tipo `Cena`, que roda o mundo a 60 Hz com carros que **de fato
  andam**. Um carro congelado num `pct` fixo é, por definição do detector, um obstáculo
  parado legítimo — a primeira versão dos testes do `spotter_frente` caiu nessa e media a
  coisa errada.
- Trate o salto de `session_time` (> 5 s ou para trás): é replay, rebobinada ou troca de
  sessão. Zere a máquina.

## As capturas reais

```
%APPDATA%\com.loop.app\debug\race_captures\
  race_1785885657.jsonl.gz    Lime Rock, 2369 m, 40 carros, 17,1 min
  race_1785889561.jsonl.gz    Okayama Short, 1929 m, 41 carros, 17,3 min
```

`.jsonl` comprimido, uma linha por registro. Leia assim — as capturas são truncadas
quando o app fecha, e um `createGunzip()` em pipe estoura com `Z_BUF_ERROR`:

```js
zlib.gunzipSync(fs.readFileSync(p), { finishFlush: zlib.constants.Z_SYNC_FLUSH })
```

Registros: `{kind:"header"}`, `{kind:"session", yaml}` (repetido a cada mudança),
`{kind:"vars"}` (inventário do SDK), `{kind:"frame", tele:{...}}`. O `cars[]` vem a 20 Hz
em corrida; nos outros quadros ele não existe. `TrackLength` sai do YAML (`2.37 km`).

**Ambas as capturas têm o jogador parado no box quase o tempo todo.** Para medir o que o
seu detector faria, simule **cada carro da IA como jogador** — é assim que se obtém uma
amostra decente a partir de duas corridas.

## Como saber se o seu detector presta

A métrica é: **quando o piloto chegou ao ponto, ainda havia um problema ali?** Não vale
"o episódio ainda estava aberto" — um episódio fecha quando o carro volta à pista, e um
carro voltando da grama, lento, na trajetória, é exatamente o perigo anunciado. Medir
fechamento de registro em vez de utilidade foi um erro cometido aqui e custou uma
recomendação errada.

Referência do que já existe, sobre as duas corridas:

| Família | Avisos | Inúteis |
|---|---|---|
| Fora da pista | 40 | 35% |
| Parado | 51 | 16% |

E a taxa-base: **0,38 avisos por piloto por corrida** na família "fora da pista", com
nenhum piloto ouvindo duas vezes. Um aviso que toca uma ou duas vezes por corrida é
informação; um que toca toda volta é ruído que o piloto aprende a ignorar. Se o seu
detector disparar muito mais que isso, ele está errado mesmo que cada disparo esteja certo.
