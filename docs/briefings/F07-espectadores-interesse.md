# F-07 — Espectadores e interesse de evento: efeito sem causa visível

**Área:** Rust (exposição) + Frontend (leitura) · **Risco:** baixo · **Tamanho:** M
**Depende de:** nada, mas rende mais depois que existirem telas onde a informação caiba

Briefing autocontido. Contexto em [roadmap.md](../roadmap.md) §5. É a pendência que o
próprio [`DESIGN.md`](../DESIGN.md) §17.1 já registra.

---

## O que foi encontrado

O sistema **modula a economia, a narrativa e a motivação do jogador — e é quase
invisível.** O jogador sente o efeito (o patrocínio rendeu mais, a notícia veio mais
forte) sem nunca ver a causa.

Não é um sistema não-exposto: é um sistema **parcialmente** exposto, e a parte exposta é
a menos interessante.

### O que já chega na UI

`EventInterestSummary` viaja no payload da próxima corrida
([`career_types/corrida.rs:23`](../../src-tauri/src/commands/career_types/corrida.rs#L23))
com três campos: `display_value`, `tier`, `tier_label`.

O frontend lê isso em dois lugares, ambos como **tempero textual**:
- [`components/race/raceEventContext.js:22,42`](../../src/components/race/raceEventContext.js) — usa `tier_label` para escolher frase
- [`pages/tabs/nextRaceContext.js:97,176`](../../src/pages/tabs/nextRaceContext.js) — estimativa de público e rótulo de interesse

Ou seja: antes da corrida, o jogador vê uma frase sobre a expectativa. É tudo.

### O que não chega

**1. A repercussão realizada.** `RealizedEventInterest` é o cálculo *pós-corrida* e é o
mais rico do módulo:

```rust
expected_display_value / expected_tier   // o que se esperava
final_display_value / final_tier         // o que de fato aconteceu
delta_vs_expected                        // ← a história inteira mora aqui
media_delta_modifier                     // efeito na mídia do piloto
motivation_delta_modifier                // efeito na motivação
news_importance_bias                     // peso na cobertura
headline_strength                        // Normal | Forte | Principal
```

O `delta_vs_expected` é literalmente "a corrida entregou mais ou menos do que prometia" —
a pergunta que todo fim de semana de automobilismo faz. **Nada disso é exposto ao
frontend.** Não há DTO, não há comando, não há tela.

**2. A presença pública de equipe.** [`public_presence/team.rs`](../../src-tauri/src/public_presence/team.rs)
deriva o perfil público da equipe do lineup:

```
presença = mídia_do_piloto_mais_midiático * 0.7 + mídia_do_segundo * 0.3
```

Consumida em `commands/race/financas.rs` como **multiplicador linear de patrocínio** — ou
seja, mexe direto no dinheiro do jogador. Verificado por `grep`: não existe nenhum campo
de presença pública nos DTOs de `career_types/`. O jogador não tem como saber que
contratar um companheiro midiático aumenta a receita.

**3. Os três multiplicadores de `ExpectedEventInterest`** (`pressure_modifier`,
`media_multiplier`, `motivation_multiplier`) estão marcados no código como "uso futuro" e
ficam de fora do `EventInterestSummary` por decisão explícita documentada no DTO.
Confirme se continuam sem uso — se algum já estiver ligado, o comentário mente.

---

## Uma armadilha de nomenclatura que vai te morder

**Existem três conceitos diferentes com nomes parecidos. Não os misture:**

| conceito | onde | o que é |
|---|---|---|
| `public_presence` | `public_presence/team.rs` | audiência/perfil público da **equipe**, contínuo, sem tier, multiplicador de patrocínio |
| `DriverCareerPresenceBlock.presenca` | `career_types/piloto.rs:288` | **tempo de carreira** do piloto (temporadas disputadas, anos desempregado). Nada a ver com público. Já exibido em `HistoricoCarreira.jsx` |
| `market::visibility::MarketVisibilityTier` | `market/visibility.rs` | classifica o atributo `midia` de um piloto **individual**, com limiares 25/85 |

O comentário no topo de `public_presence/team.rs` é explícito: as grandezas **não são
comensuráveis** — uma é média ponderada de duas mídias sem sistema de labels, a outra tem
tiers vindos de tags visuais. O briefing **R3** da varredura de acoplamento trata
exatamente da suspeita de duplicação entre `public_presence` e `market/visibility`;
**leia o R3 antes de unificar qualquer coisa aqui**, e considere resolver o R3 primeiro.

---

## O que construir

Em duas fatias de valor decrescente. A primeira sozinha já paga o item.

### P1 — Repercussão pós-corrida (o que vale de verdade)

Expor `RealizedEventInterest` e mostrar, na tela de resultado, o confronto entre o
esperado e o realizado. A leitura que interessa é o `delta_vs_expected` — "esta corrida
entregou mais do que prometia" — mais o `headline_strength` explicando por que aquela
notícia veio com o peso que veio.

Precisa de: um DTO público novo (siga o padrão do `EventInterestSummary`, que expõe só o
que tem uso real na UI — não serialize a struct inteira), o campo no payload de resultado
de corrida, e a leitura no frontend.

### P2 — Presença pública da equipe

Mostrar a presença pública no `MyTeamTab`, junto das finanças, deixando visível que ela
multiplica o patrocínio. Isso transforma a escolha de companheiro de equipe numa decisão
informada em vez de um efeito invisível.

Precisa de: campo no DTO de equipe + leitura. Cuidado com a nomenclatura acima ao nomear
a chave — não use `presenca` cru, que já significa outra coisa no `piloto.rs`.

### Fora de escopo

Ligar os três multiplicadores "de uso futuro". Isso é design de equilíbrio, não exposição
de dado. Se você achar que devem ser ligados, escreva a proposta e pare.

---

## Armadilhas

1. **Não invente número novo.** Todo valor mostrado deve vir do backend. Estimativa de
   público calculada no frontend (`estimateAudience` em `nextRaceContext.js:97`) já é uma
   concessão existente — não amplie o padrão, e considere anotá-la como dívida.
2. **`#![allow(dead_code)]` no topo de `event_interest/models.rs`.** Se você expuser
   campos hoje mortos, parte do allow deixa de se justificar. Verifique o que quebra ao
   removê-lo.
3. **i18n obrigatório**, incluindo os labels de tier (`Baixo`…`EventoPrincipal`) e de
   `HeadlineStrength`. Backend usa `rust-i18n` com locale **global do processo** — teste
   Rust que troca idioma precisa de `#[serial]`. Use a skill `nova-string`.
4. **O `tier_label` já vem pronto do backend.** Não reimplemente a tradução no frontend;
   veja como o `EventInterestSummary` já resolve isso antes de criar caminho paralelo.

## Verificação

`npm run build` → `cargo test --manifest-path src-tauri/Cargo.toml` (nessa ordem,
`generate_context!` embute o `dist/`), mais `npm run test:ui` e `npm run test:structure`.
Comando novo precisa entrar no `generate_handler![...]` do `lib.rs` — **um comando só
existe depois de registrado lá**. Use a skill `novo-comando`.
