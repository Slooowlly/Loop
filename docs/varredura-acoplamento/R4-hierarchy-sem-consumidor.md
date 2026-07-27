# R4 — `hierarchy`: estado rico, dois consumidores, nenhuma consequência de longo prazo

**Área:** Rust · **Risco:** médio-alto (mexe em mercado = equilíbrio de jogo) · **Conflita com:** nada

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
