# R3 — `public_presence` duplica a escada de tiers de `market/visibility` (e joga o próprio tier fora)

**Área:** Rust · **Risco:** baixo-médio · **Conflita com:** nada

## O que foi encontrado

Duas escadas de tier idênticas, copiadas à mão, com um comentário assumindo a cópia.
E a mais nova das duas descarta o próprio resultado.

### As duas escadas

[`market/visibility.rs:73`](../../src-tauri/src/market/visibility.rs):

```rust
pub enum MarketVisibilityTier {
    Baixa,     // 0..=25
    Relevante, // 26..=59
    Alta,      // 60..=84
    Elite,     // 85..=100
}
```

[`public_presence/team.rs:5`](../../src-tauri/src/public_presence/team.rs) —
mesmas 4 variantes, mesmos 4 limiares. A **primeira linha do arquivo** diz:

> `// Sem dependência de market::visibility — thresholds próprios, paralelos por semântica.`

e o doc de `derive_team_public_presence` repete: "Thresholds espelham
MarketVisibilityTier: Baixa ≤25, Relevante 26–59, Alta 60–84, Elite ≥85."

Ou seja: a duplicação foi uma decisão consciente. A pergunta é se ela ainda se
sustenta.

### O tier calculado é descartado

`derive_team_public_presence` devolve `TeamPublicPresence { raw_score, tier }`. Os
**três** callsites leem só `.raw_score`:

- [`commands/career/lifecycle.rs:271`](../../src-tauri/src/commands/career/lifecycle.rs)
- [`commands/career/lifecycle.rs:277`](../../src-tauri/src/commands/career/lifecycle.rs)
- [`commands/race/persistencia.rs:473`](../../src-tauri/src/commands/race/persistencia.rs)

O `raw_score` vira multiplicador de patrocínio em
[`commands/race/financas.rs:41`](../../src-tauri/src/commands/race/financas.rs)
(`lineup_public_presence * round_operating_base * FAME_SPONSORSHIP_COEFF`) e acabou.
O `tier` — o enum inteiro, com sua escada e seus testes — nunca é lido em produção.

### Assimetria de maturidade

Do lado do mercado, a escada equivalente tem uma **camada de política** completa por
cima dela: `market/team_ai.rs` (prioridade de seleção, bônus por tier,
`marketability_bias` contínuo para desempate fino), `market/driver_ai.rs` (percepção
de desejabilidade), `market/renewal.rs` (premium salarial por tier). Do lado da
equipe, nada.

### Bônus: doc desatualizado que vai enganar quem ler

`market/visibility.rs`, logo acima de `MarketVisibilityProfile`, lista "Pontos
futuros de integração (**não ativados**)" e cita `team_ai::candidate_score`,
`team_ai::generate_team_proposals`, `driver_ai::evaluate_proposal`,
`renewal::calculate_renewal_salary`. **Os quatro já estão ativados** — confirmei os
callsites em `team_ai.rs:57-142,171`, `driver_ai.rs:89-126`, `renewal.rs:98-301`.
Esse comentário deve ser corrigido de qualquer forma, independentemente do resto.

## Por que importa

Duas escadas com os mesmos números mantidas à mão divergem no primeiro
rebalanceamento — e o sintoma será sutil (um piloto Elite numa equipe que o
patrocínio trata como Alta). Mais interessante que a duplicação, porém, é a pergunta
de design: a presença pública da equipe **deveria** ter uma camada de política como a
do piloto tem? Hoje ela é um número que multiplica patrocínio e nada mais.

## Armadilhas conhecidas

1. **`raw_score` de equipe e `midia` de piloto não são a mesma grandeza.** O da
   equipe é `top_media * 0.7 + second_media * 0.3` — uma média ponderada de duas
   mídias. Aplicar os mesmos limiares nas duas pode ser semanticamente errado, e a
   separação pode existir justamente por isso. Investigue antes de fundir.
2. Fundir os enums acopla `public_presence` a `market`, que é um módulo grande. Pode
   ser que o certo seja extrair a escada para um terceiro lugar neutro
   (`common/`? `constants/`?) e os dois passarem a depender dela.
3. Mexer em limiar muda economia de jogo. Qualquer alteração numérica precisa de
   justificativa de balanceamento, não só de arquitetura.

## O que eu quero da segunda análise

1. **A separação ainda se justifica?** O comentário "paralelos por semântica" foi
   escrito num contexto. Vá no `git log` do arquivo, entenda a intenção original, e
   diga se ela continua válida.
2. **Se fundir: onde mora a escada?** `market/visibility` continua dono e
   `public_presence` importa, ou os dois passam a depender de um módulo neutro?
   Considere o custo de acoplamento nas duas direções.
3. **Se não fundir: como travar a sincronia?** Um teste que assere que os dois
   conjuntos de limiares são iguais custa 10 linhas e resolve o modo de falha real.
   Proponha-o.
4. **O `tier` de equipe deveria ser usado?** Esta é a pergunta de design que mais me
   interessa. Liste onde ele faria diferença — patrocínio escalonado por tier em vez
   de linear? Apelo da equipe no mercado (equipe Elite atrai piloto melhor)? Notícia?
   Diga o que é barato e o que é caro, e o que muda no equilíbrio do jogo.
5. **Corrija o comentário obsoleto** em `market/visibility.rs`. Confirme os quatro
   callsites que citei antes de reescrever — quero o texto novo refletindo o que
   está ligado hoje. Este item pode ir sozinho num commit, sem depender do resto.

Não aplique nada além do item 5 — quero ler a análise antes.
