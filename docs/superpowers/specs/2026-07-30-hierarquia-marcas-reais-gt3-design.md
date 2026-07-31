# Hierarquia de força no GT3 — marcas reais no topo, fictícias como azarão

> Status: **núcleo implementado.** Motivação: no atlas histórico de uma carreira de 26
> temporadas, a Solaris (equipe fictícia) ganhou **20 dos 21** campeonatos do GT3
> Endurance, e nenhuma marca real venceu o GT3 sprint desde **2006**.

Evidência primária: `saves/career_012/career.db` (temporada 27, ano jogável 2026).
Todos os números abaixo foram lidos desse banco, não estimados.

---

## 0. O que foi implementado — e o que ficou superado neste documento

A premissa original deste design estava **errada num ponto decisivo**: ele tratava a
coluna legada `car_performance` (domínio −5..16) como a medida do carro e propunha
contê-la com tetos e pisos (D1/D2 abaixo). Não é mais essa a medida. O carro do jogo é o
**Sistema de Nível do Carro**: 11 peças com nível 1–10 em `team_car`, teto por categoria,
lidas por `Team::effective_car_performance` sempre que existem. A coluna só sobrevive
como fallback de quem não tem carro.

A correção, então, não foi domar o número velho — foi **fazer todo mundo correr no
sistema novo**:

| # | Mudança | Onde |
|---|---|---|
| 1 | Bloco especial (Endurance, Production Challenger) passa a rodar o cérebro de manutenção de carro. Era o `return` que descartava finanças **e** carro junto — a raiz de C7. | [`race/persistencia.rs`](../../../src-tauri/src/commands/race/persistencia.rs) |
| 2 | Teto de nível passa a ser da **classe**, não da categoria (`endurance:gt3` → 7, `endurance:gt4` → 6, `endurance:lmp2` → 8). | [`car/cost.rs`](../../../src-tauri/src/car/cost.rs) |
| 3 | **Hierarquia real × fictícia vira teto de nível**: privateer sem `marca` constrói um nível abaixo da fábrica nas arenas GT3. Substitui a penalidade morta de `car_dev_gain_factor`. | [`models/team.rs`](../../../src-tauri/src/models/team.rs) (`Team::car_ceiling`) |
| 4 | O offseason **não escreve mais** em `car_performance`. Quem constrói carro é o cérebro de manutenção, corrida a corrida, com caixa real. Fim do runaway sem teto. | [`finance/cashflow.rs`](../../../src-tauri/src/finance/cashflow.rs) |
| 5 | Removida a banda histórica por marca, que era o teto que prendia as 11 marcas reais enquanto as fictícias cresciam livres. | [`historical_draft.rs`](../../../src-tauri/src/commands/historical_draft.rs) |
| 6 | Draft histórico semeia carro na criação, como a carreira clássica. Antes o backstory inteiro largava com o grid no mesmo carro neutro. | [`historical_draft.rs`](../../../src-tauri/src/commands/historical_draft.rs) |
| 7 | Qualidade do seed medida por `(categoria, classe)`, não por categoria. | [`car_maintenance/semeadura.rs`](../../../src-tauri/src/market/car_maintenance/semeadura.rs) |
| 8 | `career_start_year` passa a gravar o ano JOGÁVEL (2026), não o início do backstory (2000). | [`historical_draft.rs`](../../../src-tauri/src/commands/historical_draft.rs) |
| 9 | **Guarda**: auditoria de mundo reprova qualquer equipe ativa sem carro em `team_car`. É a regra que teria pego isto anos atrás. | [`world/integrity.rs`](../../../src-tauri/src/world/integrity.rs) |
| 10 | **Migração v58**: saves existentes ganham carro derivado do `car_performance` atual, medido dentro da classe — preserva a ordem do grid em vez de achatá-la. | [`db/migrations.rs`](../../../src-tauri/src/db/migrations.rs) |

**Superado por isso:** D1 (teto suave sobre a coluna) e D2 (piso de fábrica na coluna) —
ambos operavam sobre um número que o sim não lê mais. A intenção dos dois sobrevive no
item 3, expressa em nível de peça, que é a leitura que o jogador enxerga.

**Continua valendo:** D4 (reseed da classe GT3 do Endurance com 4 fábricas + 2 privateers),
D5 (programa de fábrica como payoff do azarão), D6 (nomes de equipe no GT3) e a medição M1.

O diagnóstico abaixo fica como está — é o registro de como se chegou aqui. Onde ele fala
em "clampar a coluna", leia o item 3 desta seção.

---

## 1. O sintoma

**GT3 Endurance — títulos de construtores, 2005–2025 (21 temporadas):**

| Equipe | Natureza | Títulos |
|---|---|---|
| Solaris | fictícia | **20** (2006–2025, ininterruptos) |
| Peregrine | fictícia | 1 (2005) |
| *todas as marcas reais* | — | **0** |

**GT3 sprint — títulos de construtores, 2000–2025 (26 temporadas):**

| Equipe | Natureza | Títulos | Último |
|---|---|---|---|
| McLaren | real | 4 | **2005** |
| Mercedes-AMG | real | 2 | **2006** |
| Porsche | real | 1 | **2002** |
| Obsidian, Kitsune, Arclight, Blackwell, Stratos | fictícias | 3 cada | 2025 |
| Valkyrie, Helion | fictícias | 2 cada | 2023 |
| Ferrari, Lamborghini, BMW, Audi, Aston, Chevrolet, Ford, Acura | reais | **0** | — |

As marcas reais ganharam 7 títulos, **todos nos 7 primeiros anos**. Depois de 2006 são
19 temporadas seguidas de domínio fictício. Em 2020 o pódio do campeonato de GT3 foi
Arclight / Helion / Valkyrie — e a Valkyrie nasce como a **pior equipe do grid**
(`car_performance_base: 0.0`).

---

## 2. Diagnóstico

### C1 — A banda histórica é um teto que só existe para as marcas reais

[`constants/historical_timeline.rs`](../../../src-tauri/src/constants/historical_timeline.rs)
define `historical_team_performance_band`, que devolve `Some((min, max))` **apenas** para
`categoria == "gt3"` e **apenas** para nomes de marca conhecidos. `apply_historical_performance_band`
faz `clamp(min, max)`, e o draft histórico chama `stabilize_historical_performance_bands`
**ao fim de toda temporada** ([`historical_draft.rs:791,807`](../../../src-tauri/src/commands/historical_draft.rs)).

Do outro lado, [`finance/cashflow.rs:322`](../../../src-tauri/src/finance/cashflow.rs) faz:

```rust
// Sem teto superior (Pilar B): só piso em −5; os retornos decrescentes regulam o topo.
team.car_performance = (team.car_performance + impact.car_performance_delta).max(-5.0);
```

Não há teto. Os retornos decrescentes (`/(1 + perf/14)`) **desaceleram** o ganho, não o
param — a integral diverge. Resultado depois de 26 offseasons:

| Equipe | Natureza | `car_performance` hoje | Banda |
|---|---|---|---|
| Peregrine | fictícia | **61,22** | — |
| Stratos | fictícia | **57,62** | — |
| Blackwell | fictícia | **57,31** | — |
| Kitsune | fictícia | **56,64** | — |
| Mercedes-AMG | real | **16,00** | 14,8–**16,0** |
| Porsche | real | **16,00** | 14,4–**16,0** |
| Ferrari | real | **16,00** | 14,3–**16,0** |
| Lamborghini | real | **15,80** | 13,9–**15,8** |
| BMW | real | **13,20** | 10,5–**13,2** |
| Audi | real | **12,30** | 9,0–**12,3** |
| Aston Martin | real | **12,00** | 8,8–**12,0** |
| Chevrolet | real | **10,50** | 7,0–**10,5** |
| Ford Mustang | real | **10,00** | 6,5–**10,0** |
| Acura | real | **8,50** | 4,5–**8,5** |

**As onze marcas reais estão, sem exceção, cravadas no topo da própria banda.** Todas
querem crescer, todas são empurradas de volta todo ano. As fictícias, sem banda, chegaram
a **~4× o máximo do domínio** (`CAR_PERF_MAX = 16`).

A banda foi escrita para *proteger* as marcas reais. Ela é hoje a única coisa que as
impede de competir.

### C2 — A penalidade de azarão existe, está bem desenhada e **nunca liga**

`cashflow.rs` já tem exatamente o mecanismo que você quer:

```rust
const FICTIONAL_CAR_DEV_FACTOR: f64 = 0.6;   // fictícia ganha 60% do que a fábrica ganha
pub(crate) const PENALTY_FADE_YEARS: f64 = 5.0;
```

O fade é ancorado na meta `career_start_year`. Só que
[`historical_draft.rs:177–185`](../../../src-tauri/src/commands/historical_draft.rs)
grava essa meta com `HISTORY_START_YEAR` (**2000**), não com `PLAYABLE_START_YEAR` (2026).
Confirmado no save: `career_start_year = '2000'`.

Consequência: a penalidade some em **2005**, e no ano jogável `career_year = 26` → fator
**1,0**. O único freio das equipes fictícias está desligado durante 21 dos 26 anos de
história e durante **100% da carreira jogável**.

### C3 — A classe GT3 do Endurance não tem uma única marca real no seed

O seed de `categoria: "endurance", classe: Some("gt3")` em
[`constants/teams/dados.rs`](../../../src-tauri/src/constants/teams/dados.rs) é:
Solaris, Peregrine, Arclight, Blackwell, Stratos, Helion — **seis equipes, `marca: None` em todas**.

O campeonato mais importante de GT3 do jogo é, por construção, um campeonato onde nenhuma
marca real está inscrita. Uma marca real só chega lá por promoção (`gt3 → endurance/gt3`),
e quando chega já está 20 anos atrás no desenvolvimento.

### C4 — A Solaris nasce como a equipe mais forte do jogo inteiro

```rust
TeamTemplate { nome: "Solaris", categoria: "endurance", classe: Some("gt3"),
    marca: None,
    car_performance_base: 16.0,   // = CAR_PERF_MAX, o máximo do domínio
    reputacao_base: 90.0,         // a MAIOR reputação das 66 equipes do jogo
    ... }
```

Mercedes-AMG nasce com 15,0 / 88. Ferrari com 14,0 / 85. Uma privateer fictícia é semeada
**acima de todas as fábricas** em carro e em reputação. Hoje a Solaris está com reputação
**98,0** — o teto absoluto (`CEIL` em [`finance/reputation.rs`](../../../src-tauri/src/finance/reputation.rs)).

### C5 — Reputação é um laço fechado sem contrapeso

`reputação → atrai os melhores pilotos → vitórias → reputação` ([`reputation.rs`](../../../src-tauri/src/finance/reputation.rs),
`advance_team_reputation`). O alvo do campeão num grid de 6 é 93 + `TITLE_KICKER`; a
inércia é 0,20/ano. Vinte títulos seguidos travam a Solaris em 98 permanentemente.

Isso *deveria* ser contrabalançado pelo `elite_score` em
[`finance/strategy.rs`](../../../src-tauri/src/finance/strategy.rs), que dá `+1000` a
quem tem `marca` e garante piso de caixa (Pilar D). Mas o Pilar D e a banda histórica
**trabalham um contra o outro**: a marca real recebe caixa de elite para desenvolver o
carro, desenvolve, e o `stabilize_historical_performance_bands` devolve o carro para 16,0
no fim da temporada. O dinheiro da elite é queimado todo ano.

### C6 — O jogador nunca teve como enxergar isso

`normalize_car_performance` mapeia −5..16 → 0..100 e satura em 100.
`Team::car_strength()` também. Logo:

- Solaris (79,65) → 403 → exibe **100**
- Mercedes-AMG (16,00) → 100 → exibe **100**

Na tela, os dois carros são idênticos. O desequilíbrio cresceu por 26 temporadas em cima
de um domínio que a UI não consegue representar, e nenhuma tela, nenhum teste e nenhuma
auditoria de mundo reclamou.

### C7 — O Endurance inteiro roda no sistema antigo, e é por isso que a Solaris é intocável

**Este é o mecanismo que fecha o caso.** Contagem de carros no save:

| Categoria | Equipes ativas | Equipes com carro em `team_car` |
|---|---|---|
| gt3 | 14 | **14** |
| gt4 | 10 | **10** |
| bmw_m2, mazda/toyota (amador e rookie) | 42 | **42** |
| **endurance** | **18** | **0** |
| **production_challenger** | **18** | **0** |

As duas categorias multiclasse — as do "bloco especial" — **não têm um único carro
persistido**. A causa está em
[`race/persistencia.rs:455`](../../../src-tauri/src/commands/race/persistencia.rs):

```rust
if runs_in_special_phase(race_category) {
    return Ok(());          // <- sai antes de TODO o bloco de finanças
}
```

`maintain_team_car_pits` — o cérebro que decide, compra, desgasta e persiste as peças —
vive dentro desse bloco (linha 556). Como `runs_in_special_phase` é verdadeiro para
`endurance` e `production_challenger`, essas categorias nunca criam carro e nunca rodam o
cérebro de manutenção.

Consequência direta, via `Team::effective_car_performance()` (usa as peças quando existem,
senão cai na coluna legada):

- **GT3 sprint** → tem peças → ritmo vem do Sistema de Nível do Carro, **teto de nível 7
  para todo mundo**. Os carros ficam comprimidos (níveis médios de 5,18 a 7,0 hoje).
- **GT3 Endurance** → não tem peças → ritmo vem **integralmente da coluna
  `car_performance`**, que é exatamente onde a Solaris está em **79,65**, sem teto, sem
  banda, contra um grid que ela mesma lidera desde 2006.

A Solaris é intocável porque corre num campeonato que ainda é decidido pelo número que
disparou. Não é um desequilíbrio de balanceamento — é uma categoria inteira rodando num
sistema que o resto do jogo já abandonou.

> **Escopo do que está provado.** O mecanismo acima explica o GT3 Endurance de forma
> conclusiva. Para o **GT3 sprint** (onde o carro é de peças e está comprimido no teto 7),
> a correlação é forte — as marcas reais param de vencer em 2006, mais ou menos quando as
> fictícias cruzam o 16 da coluna legada — mas o canal exato ainda não foi traçado. Ver
> §7, medição M1: é a primeira coisa a medir antes de mexer no sprint.

---

## 3. O grupo de controle: o GT4 já funciona

Você disse que no GT4 não se importa com a hierarquia. Vale olhar por quê ela está boa lá:

**GT4 — títulos, mesmas 26 temporadas:** Atlas (fictícia) 4, Rahal Letterman/BMW 3,
Stuttgart Racing Academy/Porsche 3, Grove Drive/Porsche 2, Formosa Corsa/Mercedes 2 —
distribuição saudável, maioria real, ninguém dominante.

Duas diferenças estruturais explicam tudo:

1. **O GT4 não tem banda.** `historical_team_performance_band` retorna `None` para
   `categoria != "gt3"`. Sem o clamp assimétrico, ninguém dispara e ninguém é preso.
2. **No GT4 a marca é um atributo da equipe, não o nome dela.** As equipes são
   *Rahal Letterman Racing (BMW)*, *Heart of Racing (Aston Martin)*, *Stuttgart Racing
   Academy (Porsche)* — nomes de equipe reais com `marca: Some(...)`. O GT3 é a única
   categoria do jogo onde a **marca é a própria equipe**, e foi essa anomalia que gerou a
   necessidade da banda-gambiarra.

**Conclusão de diagnóstico: a banda não é a proteção das marcas reais, é a doença.**
O GT4, sem nenhum mecanismo de proteção, produz exatamente o resultado que você quer.

---

## 4. Princípio de design

> **A fábrica é o teto por natureza; o azarão sobe até quase encostar, e só passa quando
> conquista o direito de virar programa de fábrica.**

Três regras que decorrem disso:

1. **Ninguém sai do domínio.** `car_performance` vive em `[−5, CAR_PERF_MAX]`, para todo
   mundo, sempre. Um valor de 79 é um bug, não um estado de jogo.
2. **A vantagem da marca real é um piso, não um teto.** A fábrica não pode cair abaixo do
   patamar de fábrica; ela pode subir até o topo.
3. **A privateer tem teto mais baixo, não ganho menor** (ou: além do ganho menor). O
   azarão chega a ~85% do topo — bate as fábricas fracas, incomoda as fortes, não vira
   dinastia. Para passar disso, precisa de um evento narrativo: **virar programa de fábrica**.

---

## 5. As correções

### D1 — Teto suave global, no lugar da banda (núcleo da correção)

Substituir o clamp por uma **assíntota**: o ganho de offseason decai conforme o carro se
aproxima do teto da equipe, e nunca o ultrapassa.

Em [`finance/cashflow.rs`](../../../src-tauri/src/finance/cashflow.rs):

```rust
/// Teto de car_performance por NATUREZA da equipe nas arenas GT3.
/// Fábrica chega ao máximo do domínio; privateer chega a ~85% dele.
fn car_perf_ceiling(team: &Team) -> f64 {
    const CAR_PERF_MAX: f64 = 16.0;
    let arena_gt3 = team.categoria == "gt3"
        || (team.categoria == "endurance" && team.classe.as_deref() == Some("gt3"));
    if team.marca.is_some() || !arena_gt3 {
        CAR_PERF_MAX
    } else {
        CAR_PERF_MAX * 0.85   // 13,6
    }
}
```

e no `apply_offseason_competitiveness_impact`:

```rust
let ceiling = car_perf_ceiling(team);
// Headroom: o ganho vai a zero quando o carro encosta no teto da equipe.
let headroom = (1.0 - team.car_performance.max(0.0) / ceiling).clamp(0.0, 1.0);
let gain = impact.car_performance_delta.max(0.0) * headroom;
let loss = impact.car_performance_delta.min(0.0);           // quedas aplicam cheias
team.car_performance = (team.car_performance + gain + loss).clamp(-5.0, ceiling);
```

O `clamp` final é rede de segurança; o `headroom` é o que dá a curva suave. Quedas
continuam integrais, para que uma elite desfinanciada perca terreno de verdade.

Fora das arenas GT3 nada muda — o GT4 continua exatamente como está.

### D2 — Piso de fábrica substitui a banda

`historical_team_performance_band` deixa de ser uma banda e vira
`marque_performance_floor(team) -> Option<f64>`, devolvendo **só o mínimo** atual de cada
marca (Mercedes 14,8; Porsche 14,4; Ferrari 14,3; Lamborghini 13,9; McLaren 13,8;
BMW 10,5; Audi 9,0; Aston 8,8; Chevrolet 7,0; Ford 6,5; Acura 4,5), e passa a valer
também para `endurance/gt3` — hoje uma marca promovida ao endurance **perde a proteção**
justamente onde mais precisa dela.

`stabilize_historical_performance_bands` vira `enforce_marque_floors` e só empurra para
cima. Note que os pisos de BMW/Audi/Aston/Chevrolet/Ford/Acura ficam **abaixo** do teto
privateer (13,6) — de propósito: o azarão bem gerido bate as fábricas fracas. Ele nunca
passa de Ferrari, Porsche, Mercedes, Lamborghini e McLaren sem um programa de fábrica.

### D3 — Ligar a penalidade de azarão

Uma linha, em [`historical_draft.rs:177`](../../../src-tauri/src/commands/historical_draft.rs):
passar `PLAYABLE_START_YEAR` (não `HISTORY_START_YEAR`) ao `sync_draft_meta_counters`,
para que `career_start_year = 2026`.

Efeito: durante toda a história `career_year` é negativo → `max(0)` → **penalidade cheia
(0,6)** nas 26 temporadas de backstory, que é exatamente onde ela precisa agir. E na
carreira, o azarão do jogador vê a penalidade esmaecer em 5 anos, como já está documentado
no código.

O `None => PENALTY_FADE_YEARS` (saves antigos ficam sem penalidade) permanece.

### D4 — Reseed da classe GT3 do Endurance: 4 fábricas + 2 privateers

Seguir o padrão que já funciona no GT4 — nome de equipe real, `marca` como atributo:

| Nome | Marca | `car_performance_base` | `reputacao_base` | Papel |
|---|---|---|---|---|
| AF Corse | Ferrari | 15,4 | 88 | fábrica de ponta |
| Manthey EMA | Porsche | 15,0 | 84 | fábrica de ponta |
| WRT | BMW | 14,4 | 80 | fábrica |
| Iron Lynx | Lamborghini | 13,8 | 76 | fábrica |
| **Solaris** | — | **13,2** | **70** | privateer histórica |
| **Arclight** | — | **12,6** | **66** | privateer / assento de azarão do jogador |

Peregrine, Blackwell, Stratos e Helion descem para o grid do GT3 sprint como privateers,
liberando os quatro assentos de fábrica. Nenhuma equipe fictícia é apagada — elas só
deixam de nascer mais fortes que as fábricas.

**Reduções obrigatórias no seed atual** (nenhuma fictícia acima do topo das fábricas):

| Equipe | Hoje (`perf` / `rep`) | Proposto |
|---|---|---|
| Solaris | 16,0 / 90 | 13,2 / 70 |
| Peregrine | 15,2 / 86 | 12,8 / 64 |
| Kitsune, Obsidian, Valkyrie (GT3 sprint) | 1,0–2,0 / 31–36 | mantidos |

### D5 — Programa de fábrica: o payoff do azarão

Sem isso, o teto de 13,6 vira uma parede frustrante para o jogador que escolheu a
privateer. Com isso, vira um arco.

Uma equipe fictícia da arena GT3 que atinja **(a)** um título de construtores **ou**
**(b)** reputação ≥ 85 recebe uma **proposta de programa de fábrica** de uma marca real
que não tenha equipe cliente na categoria. Aceitando:

- `team.marca = Some(marca)` → teto sobe para 16,0 e o `elite_score` ganha o `+1000`;
- a equipe mantém o nome fictício (vira *Solaris (Porsche)*, como *Grove Drive Racing (Porsche)*);
- gera notícia em [`news/`](../../../src-tauri/src/news) e entra no dossiê da equipe.

É o mesmo campo `marca` que o GT4 já usa — não exige schema novo além de um marcador de
evento.

### D6 — (Fase 2, opcional) Nomes de equipe no GT3 sprint

O GT3 é a única categoria onde a marca *é* a equipe. É essa anomalia que produziu a
banda. A correção estrutural é adotar o padrão do GT4 — *Akkodis ASP (Mercedes-AMG)*,
*AF Corse (Ferrari)*, *Manthey (Porsche)*, *Rowe (BMW)*… — mas é uma mudança de identidade
visual grande (paleta de cores por equipe tem guard estrutural em `scripts/tests/`).
**Não é pré-requisito** para D1–D5. Fica registrado como dívida.

### D7 — Levar o Sistema de Nível do Carro para o Endurance (C7) — **prioridade alta**

Sem isto, D1 e D2 corrigem a coluna legada e **o GT3 Endurance continua sendo o único
campeonato do jogo decidido por ela**. É a correção que ataca a Solaris diretamente.

O bloqueio é o `return Ok(())` de `runs_in_special_phase` em `persistencia.rs:455`, que
descarta finanças **e** manutenção de carro de uma vez só. Separar as duas coisas:

```rust
// A economia da rodada (contratos, bilheteria, patrocínio) segue fora do bloco especial —
// o calendário especial não é uma rodada de campeonato regular. O CARRO, não: ele é da
// equipe e se desgasta em qualquer corrida que ela dispute.
maintain_team_car_pits(...)?;              // roda sempre
if runs_in_special_phase(race_category) {
    return Ok(());
}
```

Duas pendências que vêm junto:

- **Semear os carros no draft histórico.** `seed_and_persist_team_cars` só é chamado em
  [`career/lifecycle.rs:62`](../../../src-tauri/src/commands/career/lifecycle.rs) (carreira
  clássica). No draft os carros nascem pelo fallback de `tick_corrida.rs:106`, com
  `seed_car(categoria, 0.5)` — **qualidade 0,5 para todo mundo**, ou seja, o grid começa a
  história com carros idênticos. Chamar o seed no draft.
- **Teto de nível por classe, não por categoria.** `category_ceiling` devolve 8 para
  `endurance` inteiro, então um GT4 do Endurance teria carro melhor (8) que um GT3 do
  sprint (7). Precisa ser `(categoria, classe)`.

> Bug adjacente, mesma raiz: `category_quality` em
> [`car_maintenance/semeadura.rs`](../../../src-tauri/src/market/car_maintenance/semeadura.rs)
> agrupa min/max apenas por `categoria`. No `endurance` isso mede o carro de GT4 contra o
> de LMP2 numa escala só. Corrigir junto, agrupando por `(categoria, classe)`.

> Bug adjacente encontrado em `car_maintenance/semeadura.rs`: `category_quality` agrupa
> min/max apenas por `categoria`. No `endurance` isso mistura GT4, GT3 e LMP2 numa escala
> só — uma equipe de GT4 do endurance é medida contra o carro de LMP2. Corrigir junto,
> agrupando por `(categoria, classe)`.

### D8 — Migração para saves existentes

Saves em campo têm equipes com `car_performance` até ~80. Uma migração
([`db/migrations.rs`](../../../src-tauri/src/db/migrations.rs), nunca editar migração já
lançada) deve **recomprimir cada `(categoria, classe)` de volta ao domínio preservando a
ordem**, e em seguida aplicar os pisos de marca de D2. O histórico de títulos já
arquivado não é reescrito — a dinastia da Solaris continua no atlas como o passado que foi,
mas a partir dali o mundo volta a ser disputável.

---

## 6. Guardas — o que teria pego isso

Nenhum teste falhou enquanto uma equipe chegava a 5× o máximo do domínio. Toda correção
aqui vem acompanhada da guarda que a trava:

| Guarda | Onde | Trava |
|---|---|---|
| `car_performance ∈ [−5, 16]` para toda equipe ativa | [`world/integrity.rs`](../../../src-tauri/src/world/integrity.rs), dentro de `audit_historical_world` — já roda ao fim do draft e já falha a criação | C1, C6 |
| `career_start_year == PLAYABLE_START_YEAR` após o draft | teste em `commands/historical_draft/tests/` | C2 |
| Nenhuma equipe `marca: None` nasce acima da menor fábrica da sua classe | teste em `constants/teams/tests.rs` (já existe suíte de seed lá) | C3, C4 |
| `apply_offseason_competitiveness_impact` nunca ultrapassa `car_perf_ceiling` | teste unitário em `finance/cashflow.rs` | D1 |
| Nenhuma equipe vence > 40% dos títulos de uma faixa em 26 temporadas | harness de calibração (Monte Carlo, N ≥ 20 drafts) | o sintoma em si |

---

## 7. Medição pendente antes de mexer no GT3 sprint

**M1 — qual número decide o campeonato de GT3 sprint hoje?** O GT3 sprint tem carros de
peças comprimidos no teto 7, então a coluna legada (Peregrine 61,22 × Ferrari 16,00)
*não* deveria estar decidindo as corridas de lá. Ainda assim as marcas reais não vencem
desde 2006. Antes de aplicar D1/D2 ao sprint, instrumentar um draft histórico e registrar,
por temporada: nível médio de peças por equipe, `car_performance` e posição final. Se os
níveis de peça divergirem ao longo da história, o canal é o dinheiro (patrocínio/mérito
alimentados pela coluna legada); se não divergirem, o canal são os pilotos (reputação).

A correção muda conforme a resposta — e as duas hipóteses já estão cobertas pelo desenho
(D1/D2 para o carro, §10 para pilotos). O que M1 evita é aplicar a correção no lugar errado.

---

## 8. Critérios de aceite

Medidos sobre **20 drafts históricos** com seeds diferentes, por faixa:

1. **GT3 sprint:** marcas reais somam **≥ 60%** dos títulos de 2007 em diante (hoje: 0%).
2. **GT3 Endurance:** pelo menos **3 marcas reais distintas** com título ao longo das
   21 temporadas (hoje: 0).
3. **Nenhuma equipe** — real ou fictícia — vence **mais de 40%** dos títulos de uma faixa
   (hoje: Solaris, 95%).
4. **Nenhuma equipe ativa** sai de `[−5, 16]` em `car_performance` ao fim do draft.
5. As privateers **continuam relevantes**: pelo menos uma equipe `marca: None` termina no
   pódio do campeonato em **≥ 30%** das temporadas. Se cair a zero, a correção passou do
   ponto e virou o problema oposto.
6. **GT4 inalterado**: a distribuição de títulos do GT4 antes e depois é
   estatisticamente indistinguível (nenhuma das mudanças toca a categoria).

---

## 9. Ordem de implementação

| # | Correção | Esforço | Risco | Pré-requisito |
|---|---|---|---|---|
| 1 | **D3** — `career_start_year` correto | 1 linha + teste | baixo | — |
| 2 | **D1** — teto suave global | ~40 linhas em `cashflow.rs` + testes | médio | — |
| 3 | **D2** — piso de fábrica no lugar da banda | reescrita de `historical_timeline.rs` | médio | D1 |
| 4 | **Guardas** — auditoria de domínio + testes | — | baixo | D1, D2 |
| 5 | **D7** — carro de peças no Endurance | médio | **alto** | D1, D2 |
| 6 | **D4** — reseed do GT3 do Endurance | dados + guard de paleta | médio | D1–D3 |
| 7 | **D8** — migração de saves | 1 migração | médio | D1, D2 |
| 8 | **M1** — medir o canal do GT3 sprint | instrumentação | baixo | — (pode rodar já) |
| 9 | **D5** — programa de fábrica | feature nova | médio | D4 |
| 10 | **D6** — renomear o grid do GT3 | grande | alto | — (dívida) |

Os itens 1–5 são o núcleo: 1–4 fecham o vazamento da coluna legada e **5 tira o GT3
Endurance do sistema antigo**, que é onde a Solaris vive. O item 5 é o de maior risco —
mexe no caminho de persistência de corrida, compartilhado com a carreira jogável — e
merece uma bateria completa (`cargo test` + as duas suítes de JS) antes de entrar.
**M1 (item 8) não depende de nada e pode ser medido antes de qualquer código de correção.**

---

## 10. Fora de escopo

- **GT4, LMP2, Production Challenger e as categorias de base.** Explicitamente inalterados.
  O LMP2 tem uma dinastia parecida (United Autosports, 17 títulos, reputação 98) e um grid
  100% fictício, mas ali não há marca real para disputar — é outro problema, com outro
  desenho.
- **Reescrever o histórico de saves existentes.** A dinastia da Solaris permanece no atlas.
- **Balanceamento de pilotos.** O laço reputação→piloto→vitória (C5) é atacado
  indiretamente, pelo carro. Se depois de D1–D4 a reputação ainda decidir sozinha, aí sim
  vale mexer em `market/`.
