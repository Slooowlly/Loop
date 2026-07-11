# Propostas formais + Poaching mid-contrato com leilão — Design

Data: 2026-07-11
Status: aprovado (decisões travadas). **Fases A e B COMPLETAS** (mérito forma+pedigree,
assentos segurados, dedup ofertas×propostas, expiração 3 semanas via `market_proposals.
semana_limite`, teto por prestígio, prazo no card). IMPORTANTE: o card "Proposta recebida"
NÃO existia na UI (respondToProposal no store nunca era chamado) — foi CONSTRUÍDO agora em
PreSeasonView (seção "Propostas recebidas" com aceitar/recusar + prazo). Fases C-E pendentes.

## Contexto

O mercado do jogador tem **dois surfaces** hoje:

- **"Suas ofertas"** (`market::pipeline::player_market_offers`, via `get_transfer_window_state`): vagas
  vivas que a carteira do jogador alcança, recalculadas toda semana. É o que o jogo usa de verdade.
- **"Proposta recebida" (card)** (`market::pipeline::generate_player_proposals` → `persist_player_proposal`
  → `get_player_proposals` → cards em `PreSeasonView.jsx` + `respond_to_proposal`): propostas nominais
  persistidas.

O card está **morto no jogo real**: `generate_player_proposals` só roda dentro de `run_market` com
`resolve_market=true` (`pipeline.rs:298`), e a produção usa `run_market_prepasses` (`resolve_market=false`,
`preseason.rs:299`), que pula esse bloco. `run_market` (resolve=true) só é chamado em testes. Toda a
plumbing (modelo `MarketProposal`, persistência, cards no front, `respond_to_proposal`, evento de feed
`PlayerProposalReceived`) existe e está adormecida.

Este design **revive o card como uma feature de mérito** (equipes cortejam o jogador) e adiciona um
sistema econômico novo de **poaching mid-contrato com multa e leilão de retenção**.

## Feature 1 — Valor do piloto = forma + pedigree

O `market` hoje usa só `visibility` (forma) e `team_prestige` (da equipe). O **índice do ranking mundial**
(`commands::global_driver_rankings::compute_historical_index`, `global_driver_rankings.rs:1198`) é um
`f64 ≥ 0` calculado ao vivo, puro mérito de CARREIRA (títulos≫vitórias + diversidade + eficiência +
coroas de slam), ilimitado, e hoje **só display** (`GlobalDriversTab.jsx`).

Blend nos dois eixos:

- **Forma**: `visibility` + skill + resultado recente ("está quente agora?").
- **Pedigree**: `compute_historical_index`, normalizado por **percentil entre os ativos** (porque é
  ilimitado) → `boost_pedigree ∈ ~[0, +0.4]`.

`atratividade = valor_de_forma × (1 + boost_pedigree)`.

Efeito: veterano condecorado segue cortejado após temporada morna (peso do nome); rookie em ascensão é
cortejado pela forma. O índice também alimenta o **teto de propostas por prestígio** (percentil → teto 1..5).

## Feature 2 — Propostas formais (card "Proposta recebida" vivo)

Reaproveita o leilão de dois lados (`apply_weekly_market`): roda a avaliação **incluindo o jogador**;
quando uma equipe o *escolheria* (bate os agentes livres da IA pelo assento dela), em vez de assinar na
hora ela **manda proposta formal e segura o assento** — que é o que `generate_player_proposals` já faz.

### Três origens
- **Mesmo tier**: limiar padrão do leilão.
- **Promoção (tier +1)** — *trava dura*: só se top-3/campeão da temporada anterior E a equipe de cima o
  avalia acima da IA dela. Raro e merecido.
- **Lealdade**: equipe atual/anterior com bônus pra recontratar.

### Volume por prestígio (teto simultâneo)
Baseado no índice do ranking (percentil) + fecho da última temporada:

| Status | Teto |
|---|---|
| Campeão / estrela multi-título | 5 |
| Pódio / brigou por título (top-3) | 4 |
| Metade de cima | 3 |
| Fundo | 2 |
| Rookie / sem temporada completa / voltando de hiato | 1 |

Promoção conta dentro do teto.

### Expiração — 3 semanas, avisada no card
Cada proposta guarda `semana_limite = semana_criada + 3`; o card mostra "expira em N semanas". Varredura
semanal expira e **libera o assento** pra IA.

### Conteúdo do card
Papel (N1/N2), salário com **prêmio de corte** (acima do piso passivo, escalado pelo interesse), duração
(1 ano padrão; 2-3 anos pra alvo cobiçado titular), categoria, e **pitch** (campo novo de texto) → vira
notícia via `PlayerProposalReceived`.

### Ciclo de vida (assentos segurados, sem deslocar)
1. Semana avança → gera propostas (respeita o teto, sem duplicar assento) → une os assentos ao conjunto
   `reserved` que a escada já poupa.
2. Aceita uma → assina (`respond_to_proposal`); as outras se retiram, assentos voltam pra IA.
3. Recusa uma → assento liberado.
4. Espera → cada proposta vive até seu limite de 3 semanas.
5. Janela fecha sem aceite → piso reservado coloca em assento vazio (sem dispensar ninguém — já
   implementado em `ensure_player_seated`).

Convivência: assento com proposta formal aparece como card (pitch + prazo), não repetido em "Suas ofertas".
Ordem: Propostas formais → Suas ofertas → piso invisível.

## Feature 3 — Poaching mid-contrato com multa e leilão

Sistema econômico **novo** (nada existe hoje: mercado só preenche assento vazio, pula contratado, libera só
na expiração, e não há transferência de dinheiro equipe→equipe).

### Escopo: GRADE TODA (mundo vivo)
IA assedia IA também, multa fluindo entre elas, vagas em cascata. O jogador tanto recebe assédio quanto vê
o mundo se mexer.

### Alvo
Contratado com **≥1 ano restante** depois da próxima temporada (mid-contrato de verdade).

### Comprador
Equipe que (a) valoriza o piloto acima da melhor opção de agente livre por margem real, e (b) consegue
pagar a multa (caixa/dívida).

### Multa (dinâmica)
`multa = salario_anual × anos_restantes × k × (1 + fator_pedigree)`. Mais anos + mais pedigree → multa
maior (protege contratos longos e estrelas; poaching raro e caro). `k` = constante de calibração.

### Leilão de retenção (equipe vs equipe; piloto passivo no lance)
Quando o comprador B assedia o piloto X (da equipe A):
1. **A não briga** → transferência limpa: B paga a multa a A; então X decide (ver "palavra final").
2. **A briga** → **leilão**: B e A sobem o salário de X em rodadas; cada uma só cobre dentro do teto dela
   (`calculate_salary_ceiling` + caixa/dívida). Uma recua ao passar do teto.
   - **B vence** (A recua) → X vai pra B no salário final; B paga a multa a A.
   - **A vence** (B recua) → X **fica em A** com salário inflado (prêmio por ser disputado; sem multa).

O piloto **não participa do lance**. Pro jogador: **tela real ao vivo** vendo A e B subirem seu salário
rodada a rodada. Pra IA: roda nos bastidores.

### Palavra final do JOGADOR (decisão travada)
O leilão decide QUAL equipe conquistou o direito (e o salário). Se B venceu, **você ainda escolhe**: ir pra
B ou ficar em A. Se A venceu, você fica (com o salário inflado). Preserva sua agência; a IA resolve sozinha
(gate simples de `evaluate_proposal`).

### Dinheiro e cascata
- Multa só se houver **movimento** (B vence e X vai). Peça nova: `transfer_between_teams(perde, compra,
  multa)` — debita a compradora (podendo endividar), credita a que perde, re-roda estado financeiro
  (`refresh_team_financial_state`). É a primeira transferência equipe→equipe do jogo.
- A que perde abre vaga → cascata (assedia outro ou pega agente livre).

### Decisão do piloto (estender `evaluate_proposal`)
Limiar de "quebrar contrato": mid-deal exige upgrade maior (carro/tier/papel/salário). Personalidade:
**Leal** quase não sai, **Mercenário** sai por dinheiro, **Ambicioso** por promoção/N1.

## Decisões travadas (resumo)
- Valor do piloto = forma × (1 + pedigree do índice do ranking, normalizado por percentil).
- Propostas formais por mérito (venceu o leilão pelo assento).
- Origens: mesmo tier, promoção (top-3/campeão, trava dura), lealdade.
- Volume escala com prestígio (teto 1..5).
- Expiração 3 semanas, avisada no card; libera o assento.
- Poaching GRADE TODA; multa DINÂMICA (salário × anos × pedigree).
- Vendedor pode brigar → LEILÃO de salário entre equipes (piloto passivo no lance).
- Jogador dá a PALAVRA FINAL após o leilão; IA resolve sozinha.

## Roadmap faseado (ordem: propostas formais primeiro)
- **Fase A — núcleo das propostas**: gerar propostas de mesmo-tier por mérito (forma+pedigree) no loop
  semanal vivo (`advance_week`), segurar assentos, card com dados reais. Revive o card.
- **Fase B — ciclo de vida**: expiração 3 semanas + prazo no card + liberação do assento; teto por
  prestígio.
- **Fase C — promoção**: propostas de tier+1 com trava dura.
- **Fase D — poaching base**: `transfer_between_teams`, multa dinâmica, contratados como alvo, decisão de
  quebra de contrato (transferência limpa, sem leilão ainda).
- **Fase E — leilão + tela**: leilão de retenção A vs B, palavra final do jogador, tela ao vivo do leilão.

## Pontos de código (aterrado no mapeamento)
- Índice: `commands/global_driver_rankings.rs:1198` (`compute_historical_index`), display em
  `GlobalDriversTab.jsx`. Não usado no mercado hoje.
- Contrato: `models/contract.rs:19` (campos `salario_anual`, `duracao_anos` 1-3, `temporada_fim`); coluna
  **`clausulas` morta** em `migrations.rs:3095` (disponível pra reuso).
- Mercado só preenche vazio: `find_vacancies` (`pipeline.rs:613`), `find_available_drivers` pula
  contratado (`pipeline.rs:718`), expira em `expire_ending_contracts` (`contracts.rs:208`).
- Assinatura sem dinheiro entre times: `sign_driver_to_team` (`pipeline.rs:770`).
- Finanças de equipe única: `finance/cashflow.rs::apply_round_cashflow`; salário debitado em
  `commands/race.rs:1849`. NÃO existe transferência time→time.
- Decisão do piloto: `market/driver_ai.rs:19` (`evaluate_proposal`, personalidades) e
  `market/renewal.rs:39` (`should_renew_contract`, wired em `pipeline.rs:234`).
- Propostas do jogador (adormecido): `generate_player_proposals` (`pipeline.rs:1465`),
  `persist_player_proposal` (`pipeline.rs:1697`), gate `if resolve_market` (`pipeline.rs:298`).
- Assentos reservados (já implementado): `player_reserved_seats` (`pipeline.rs:2160`), poupados em
  `advance_week` (`preseason.rs:554`); garantia de porta sem dispensa em `ensure_player_seated`
  (`pipeline.rs:1344`).
