# R4 — `hierarchy`: estado rico, dois consumidores, nenhuma consequência de longo prazo

**Área:** Rust · **Risco:** médio-alto (mexe em mercado = equilíbrio de jogo) · **Conflita com:** nada

---

## Situação em 11/08/2026 — PARCIAL. A premissa envelheceu; o que sobrou de técnico foi corrigido.

O briefing dizia "estado rico sem consumidor". Hoje há consumidor em quatro frentes, e o
que restou de problema é código morto, não falta de consequência.

### Mapa de consumo, rastreado de verdade

| Dado | Quem lê hoje |
|---|---|
| `hierarquia_n1_id` / `n2_id` | Frontend, em `components/team/myteam/teamMetrics.js` — a dupla da garagem sai daí, e os slots `piloto_1_id`/`piloto_2_id` são só fallback de save antigo. |
| `hierarquia_status`, `hierarquia_tensao`, `hierarquia_inversoes_temporada` | Frontend, no mesmo `garageClimate` (`MyTeamTab` v2 / `LineupStrip`), serializados por `commands/career/queries.rs` em `career_types/equipe.rs`. |
| `hierarquia_tensao` | `commands/world_footer/equipes.rs`: `tensao > 55` entra no `bad_mood` que colore o rodapé do mundo. |
| Inversão N1/N2 | **Chega ao mercado.** `sync_contract_roles_after_inversao` (em `hierarchy/orders.rs`) reescreve `contract.papel`, e é por `contract.papel` que `market::renewal::should_renew_contract` aplica os gates de N2. A afirmação "o mercado não lê nada disso" **caiu**. |
| Moral | `rivalry::team::{apply_derby_morale, process_driver_rivalry_bleed}`, como o briefing já dizia. |
| Placar do duelo | `rivalry::gatilhos::process_teammate_season_rivalry` lê o placar cru. |
| Motivação | `apply_inversao_driver_effects`: +15 no promovido, −10 no rebaixado, no momento da inversão. |

**Classificação das perguntas do briefing:**

- "O jogador enxerga isso?" → **sim**, item 2 respondido. **Falso positivo** o pressuposto de
  que não.
- "Qual ligação vale mais / desenhe a de menor risco / risco de loop" → **DESIGN pendente**.
  Nada foi ligado, nada foi inventado.
- O eixo de tensão hoje **não anda**: o ponto de equilíbrio dos deltas é 40% de vitórias do
  N2 e o mundo simulado entrega 22–33%, então a tensão da equipe média tende a zero e o
  gatilho de inversão (que exige Crise, tensão ≥ 90) quase nunca dispara. Isso é
  **CALIBRAÇÃO**, e não foi tocado. O que existe agora é `TENSAO_EQUILIBRIO_TAXA_N2`
  documentando o ponto, com um teste que a prende aos deltas — mexeu no delta sem mexer na
  constante, o teste quebra.

### O que era técnico e foi corrigido

- **`#![allow(dead_code)]`** saiu de `orders.rs` e de `transition.rs`.
- **`DuelResult.team_id`, `.n1_id`, `.n2_id`**: preenchidos a cada duelo de cada equipe de
  cada corrida, lidos por ninguém (quem recebe o duelo já está com a equipe em mãos).
  Removidos.
- **O par `decide_hierarchy_transition` / `resolve_transition_values`**, mais o enum
  `HierarchyTransition` e as structs `PrevHierarchyState` / `NewSeasonSetup`: nenhum
  chamador de produção, só os próprios testes. Removidos, com o motivo registrado no
  cabeçalho do módulo. **A regra `PartialPreserve` que eles descreviam nunca foi ligada** —
  hoje a transição entre temporadas é sempre reset, escrito por
  `market::pipeline::consolidacao`.
- **Comentários falsos**: `transition.rs` afirmava, em dois pontos, que as equipes chegam
  alinhadas "porque passaram pelo `UpdateHierarchy` do mercado". Não passam (ver abaixo).
  Os dois textos passaram a apontar a consolidação, que é quem escreve de fato.

### Achado confirmado e NÃO corrigido, por risco de colisão

`PendingAction::UpdateHierarchy` é **estado calculado que ninguém lê**, e a cadeia inteira é
write-only: `refresh_planned_hierarchy_for_team` (em `commands/career/market_window.rs`)
recalcula N1/N2 por skill, empurra o evento em `plan.planned_events`, e **nada no crate
executa `planned_events`** — só há inserção, filtro e remoção. A conta que ele faz é a mesma
que `market::pipeline::consolidacao` já faz e persiste.

Não removido nesta passada por dois motivos:

1. `planned_events` é serializado em `preseason_plan.json` e `load_preseason_plan` **falha
   duro** em variante desconhecida. Tirar a variante quebra save em andamento.
2. O problema é do plano de pré-temporada inteiro, não de `hierarchy/`. Cortar só a metade
   da hierarquia deixa o resto do vetor igualmente órfão.

Fica registrado como item próprio para quem for mexer no formato do plano.

## O que foi encontrado

`hierarchy/` modela a política interna da equipe: quem é N1, quem é N2, o duelo entre
os dois, a tensão acumulada, o gatilho de inversão. É um dos módulos mais detalhados
do domínio e um dos menos consultados.

### Superfície do módulo

[`hierarchy/orders.rs`](../../src-tauri/src/hierarchy/orders.rs):

| Linha | Função |
|---|---|
| 46 | `is_duel_valid` |
| 56 | `read_team_duel` |
| 88 | `apply_duel_counters` |
| 113 | `n2_win_rate` |
| 138 | `update_tensao` |
| 168 | `update_status` |
| 183 | `is_em_reavaliacao` |
| 214 | `has_inversao_trigger` |
| 238 | `apply_inversao` |
| 257 | `apply_inversao_driver_effects` |
| 272 | `process_hierarchy_for_category` ← **único ponto de entrada usado de fora** |

[`hierarchy/transition.rs`](../../src-tauri/src/hierarchy/transition.rs): `decide_hierarchy_transition`,
`resolve_transition_values`, `validate_and_normalize_team_hierarchies` (148).

### Consumidores externos: dois

- [`commands/race/persistencia.rs:195`](../../src-tauri/src/commands/race/persistencia.rs)
  → `process_hierarchy_for_category` (por corrida)
- [`commands/career/market_window.rs:362`](../../src-tauri/src/commands/career/market_window.rs)
  → `validate_and_normalize_team_hierarchies` (normalização, não consequência)

Efetivamente: **um** consumidor de lógica, um de sanidade.

### Onde a treta interna morre

O único destino do conflito N1/N2 que consegui rastrear é a moral: em
`commands/race/persistencia.rs`, junto do bloco de hierarquia, roda
`rivalry::team::apply_derby_morale` e `rivalry::team::process_driver_rivalry_bleed`.

O que **não** acontece:

- **Mercado.** `market/` não lê `tensao`, `status`, `is_em_reavaliacao` nem
  `n2_win_rate`. Um N2 que vence o duelo interno a temporada inteira não pede saída,
  não fica mais caro, não vira alvo de assédio por isso. `market/renewal.rs` decide
  renovação por vínculo (`bond`) e foco — não por posição na hierarquia.
- **Narrativa.** `narrative/` não vê hierarquia nenhuma (ver briefing R1). "Briga
  interna na equipe X" é uma das manchetes mais naturais do automobilismo e o motor
  já tem o dado.
- **Evolução.** `evolution/motivation.rs` ajusta motivação no fim da temporada.
  Um N2 preso em segundo plano apesar de ganhar os duelos deveria desmotivar; um
  promovido a N1 deveria motivar. Confirme se `apply_inversao_driver_effects` já
  cobre parte disso — o nome sugere que sim, mas só no momento da inversão.

## Por que importa

O sistema produz estado a cada corrida (tensão sobe, duelos são contados, status
muda) e esse estado quase não realimenta o mundo. É simulação que roda sem
consequência — custo de CPU e de complexidade sem retorno de jogabilidade.

## Armadilhas conhecidas

1. **Mercado é equilíbrio de jogo.** Ligar hierarquia ao mercado muda a economia de
   contratos. Não é um refactor, é design. Trate como tal.
2. **`apply_inversao_driver_effects` já mexe em atributos** dos dois pilotos.
   Adicionar efeito de motivação por cima pode dobrar o efeito. Leia antes.
3. **Ordem no pós-corrida.** `process_hierarchy_for_category` roda dentro de
   `commands/race/persistencia.rs`, num ponto específico em relação a moral,
   rivalidade e arquivamento. Qualquer consumidor novo precisa rodar depois, e a
   ordem lá é comentada linha a linha — respeite os comentários.
4. Modelo fechado de pilotos: o `evolution/pipeline` tem um comentário explicando
   que rookies **não** são pré-gerados porque isso criava órfãos. Se a ligação com o
   mercado fizer piloto pedir saída, cuidado para não criar agente livre eterno.

## O que eu quero da segunda análise

1. **Confirme o mapa de consumo.** Rastreie de verdade quem lê `tensao`, `status`,
   `duelos_total`, `duelos_n2_vencidos` — inclusive via query de DB e via serialização
   para o frontend. Talvez o frontend exiba e eu não tenha visto. Se exibe, onde?
2. **O jogador enxerga isso?** Se a hierarquia aparece na UI (MyTeamTab? DriverCard?),
   o sistema já tem valor como informação e a conversa muda. Verifique.
3. **Qual ligação vale mais?** Ranqueie: mercado (pedido de saída / prêmio salarial
   / alvo de assédio), narrativa (manchete de briga interna), evolução (motivação),
   moral (já parcialmente ligado). Critério: história gerada por unidade de risco de
   equilíbrio.
4. **Desenhe a ligação de menor risco em detalhe** — a que você ranqueou em 1º —
   com os números propostos, o ponto exato do pipeline onde entra, e como testar.
5. **Existe risco de loop?** Hierarquia → moral → simulação → resultado → hierarquia
   já é um ciclo. Adicionar mercado e motivação fecha mais ciclos. Analise se algum
   deles é de realimentação positiva descontrolada (o N1 vence, fica mais forte,
   vence mais).
6. **Recomendação franca**, incluindo "não ligar nada e simplificar o módulo" se for
   o caso. Se o sistema não tem consequência e ninguém o vê, cortá-lo é uma resposta
   legítima.

Não aplique nada ainda — quero ler a análise antes.
