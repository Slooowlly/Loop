# Rivalidade pessoal do jogador — o Nemesis vivido (design)

**Data:** 2026-07-18
**Status:** design travado com o usuário; NÃO implementado.
**Escopo desta spec:** a rivalidade que o JOGADOR vive (piloto↔piloto centrada nele).
Rivalidade time-vs-time e a cadeia "piloto larga time → rival → título → crise" ficam
para specs separadas (ver backlog em `project_team_rivalry_backlog`).

---

## 1. Diagnóstico — por que hoje a rivalidade não é "vivida"

Existe um motor de rivalidade **maduro** (`rivalry/mod.rs`, `models/rivalry.rs`,
`db/queries/rivalry_episodes.rs`): dois eixos (`historical_intensity` memória +
`recent_activity` calor → `perceived = 0.4·h + 0.6·r`), 4 origens (Colisão,
Companheiros, Campeonato, Pista), ciclo de vida Viva/Adormecida/Extinta, níveis
nomeados com manchete ao cruzar patamar, e um **log de episódios** (a "memória da
novela") que a IA usa pra recapitular o arco.

**Dois problemas:**

1. **Dois "rivais" desconectados.** O que chega no jogador NÃO é o motor: é
   `build_primary_rival_summary` (`commands/career.rs:5594`), um proxy **posicional**
   (quem está ±1 no campeonato) que **troca toda semana** e tem `rivalry_label: None`
   cravado. A peça boa (o motor + o label) não é a que o briefing usa.

2. **A rivalidade é DESCRITIVA, não CONSEQUENTE.** Ela narra o passado, mas: o rival
   não age, o jogador não age, nada está em jogo além da corrida normal, e nunca há
   clímax — só decai até `Extinta` e some. Nunca houve um "acerto de contas".

**Decisão do usuário:** a rivalidade DEVE mexer no resultado da corrida (mecânica via
pressão + comportamento), não ficar só narrativa.

---

## 2. Ideia-âncora — a rivalidade é a 3ª fonte de pressão

`simulation/pressure.rs` já é um modelo **intensidade × direção** com fontes que se
**somam** via `combine()`: pressão de TÍTULO + pressão de CASA CHEIA. A Pressão de
Duelo entra como a **terceira fonte**, na mesma forma, reusando `headroom_pace_mult`,
`combine` e o clamp de `error_mult [0.5, 2.0]`. No export, o sinal `nemesis`
(`iracing_sdk/behavior.rs:673`) — hoje um booleano solto — passa a ser **alimentado
pelo motor** (intensidade percebida), em vez de detectado do traço.

---

## 3. Pressão de Duelo (mecânica — Fase 1)

Fonte nova `rivalry_pressure_for(...)` em `pressure.rs`, somada no `combine` junto de
título + casa cheia. Aplica-se ao JOGADOR **e** ao Nemesis (simétrica, coerente com o
sistema de pressão já ser simétrico).

### Intensidade
```
rivalry_intensity = (perceived/100) × duel_gate × stance_mult
```
- `perceived`: puxado do motor de 2 eixos para o par (jogador, Nemesis). Frio → ~0;
  "intensa" (80–100) → swing cheio.
- `duel_gate` (0..1): só acende quando os dois vão **brigar de verdade** nesta corrida
  — MESMA categoria e pace esperado próximo (mesma faixa do grid). Traduz o "lado a
  lado" para a sim probabilística sem tick-a-tick. Nemesis 10 posições à frente, ou em
  outra categoria → gate ~0 (duelo frio hoje; o arco continua vivo como meta narrativa).
- `stance_mult`: 1.0 na Fase 1 (sem escolha do jogador ainda). Fica ligado na Fase 2.

### Direção — o tempero que a faz sentir como ódio, não como campeonato
O sinal nemesis é descrito como *"ADVERSO — emoção que a cabeça fria controla"*. Então
a Pressão de Duelo usa um **neutro MAIS ALTO** que o do título:
- Proposta: `RIVALRY_NEUTRAL = 0.62` (vs `NEUTRAL = 0.55` do título; oposto filosófico
  do `EVENT_NEUTRAL = 0.42` da casa cheia, onde palco = oportunidade).
- Efeito: contra o inimigo, **mais gente tende ao erro/afobação**; só o genuinamente
  frio (mentalidade alta + experiência) converte em clutch. Reusa `pressure_resilience`.

### Constantes a calibrar (via Monte Carlo)
- `RIVALRY_NEUTRAL` (≈0.62)
- peso/escala da intensidade da rivalidade (garantir swing comparável, não dominante)
- limiares do `duel_gate` (o que conta como "mesma faixa de pace")

### Flag
`IRACER_RIVALRY_PRESSURE` liga a camada mecânica separada do resto.

---

## 4. O Arco (Fase 1) — começo, escalada, clímax, desfecho

### Seleção do Nemesis (substitui o proxy posicional)
- Nemesis = par de maior `perceived` **envolvendo o jogador**, com `perceived ≥ 40`
  (abaixo: SEM Nemesis — no início de carreira ninguém te odeia; ele emerge).
- **Histerese:** o Nemesis atual só perde o posto se outro superar por margem (≈+15) —
  não troca de inimigo toda semana.
- **Atravessa categorias** (decisão do usuário): não filtra por categoria. Rival que
  subiu antes de você continua "o cara a alcançar" (meta narrativa/arco); a Pressão de
  Duelo é que fica dormente até se reencontrarem (via `duel_gate`).
- `build_primary_rival_summary` passa a preencher `rivalry_label` de verdade e a apontar
  para este Nemesis (não mais o vizinho no campeonato).

### Origem nomeada (o `rivalry_label`, hoje sempre `None`)
Derivada do 1º capítulo do log de episódios / tipo da rivalidade:
- Colisão → *"A Revanche de {pista}"*
- Mercado (vaga/troca) → *"A Vaga que Ele Te Tirou"* (gancho pra agência de saída do
  `project_team_rivalry_backlog`)
- Subiram juntos → *"Os Rookies de S{n}"*
- Rival em categoria acima → *"Ele Chegou Lá Primeiro"*

### Clímax — "O Acerto de Contas"
Quando Nemesis intenso (`perceived ≥ 80`) E se encontram em **decisão de título ou
última etapa**: o jogo marca o evento. A Pressão de Duelo e a de título **somam**
(o `combine` já faz), o briefing ganha moldura própria, e o resultado rende
fama/legado desproporcional (liga em `project_stardom_economy`). É o payoff que o
decaimento atual nunca entrega.

### Desfecho & legado
Ao esfriar (ou aposentadoria/dominância de um), em vez de virar `Extinta` em silêncio,
a rivalidade **fecha num registro permanente de carreira**: *"duelo com X encerrado
12–8 em 3 temporadas, decidido em Interlagos/S7"*. Vai pro Livro de Recordes
(`project_records_book_ui`) e pra tela de Rivais (`src/pages/history/Rivalries.jsx`).

---

## 5. Fase 2 (depois de calibrar a base)

- **Postura pré-corrida** (agência do jogador): *Provocar na imprensa* / *Respeito frio*
  / *Prometer revanche* (só após perder pra ele). Mexe em 3 alavancas:

  | Postura | Sua intensidade | Neutro DELE | Fama em jogo |
  |---|---|---|---|
  | Provocar | ↑ (≈×1.3) | ↑ (mais erro) | ↑↑ |
  | Respeito frio | ↓ (≈×0.8) | neutro | ↑ |
  | Prometer revanche | ↑ na próxima | — | ↑ |

  Faca de dois gumes: provocar esquenta os dois; jogador frágil de cabeça também erra
  mais. É a escolha que torna a rivalidade *sua*.
- **Efeito no export:** escala o sinal `nemesis` (`behavior.rs`) por
  `(perceived/100) × stance`, e a provocação empurra o neutro do RIVAL — usando a
  máquina de comportamento que já roda (`project_export_behavior`).
- **UI:** tela de Rivais mostrando origem, linha do tempo de capítulos, retrospecto
  H2H, fase e o nome; rivais aposentados como "clássicos históricos".

---

## 6. O que reusa (quase nada é máquina nova)

| Peça | Onde já existe |
|---|---|
| Seleção do Nemesis | motor de 2 eixos `rivalry/mod.rs` |
| Linha do tempo do arco | `db/queries/rivalry_episodes.rs` |
| Pressão de Duelo | 3ª fonte em `simulation/pressure.rs` (`combine`, `headroom`) |
| Efeito no export | sinal `nemesis` `iracing_sdk/behavior.rs:673` |
| Recompensa (fama) | `project_stardom_economy` |
| Recap do arco | briefing/debrief da IA |
| Manchetes | `NewsType::Rivalidade` já existe |
| Legado | `project_records_book_ui`, `Rivalries.jsx` |

Trabalho real da Fase 1: conectar motor → briefing (matar o proxy posicional),
marcar/nomear a origem, somar a Pressão de Duelo, e fazer a rivalidade **terminar em
clímax + legado** em vez de decair no vácuo.

---

## 7. Riscos / atenções

- **Swing combinado:** título + casa cheia + duelo, tudo no talo na última etapa, pode
  inflar demais. `combine` + clamp seguram, mas o MC precisa confirmar que o craque não
  desaba sozinho no pior caso. Métrica nova sugerida em `sim_stats`: distribuição do
  `rivalry_pace_delta` e **% de corridas com Nemesis ativo por tier** — rivalidade tem
  que ser RARA e QUENTE, não ruído constante.
- **Estabilidade da grade:** esta spec NÃO mexe nos freios de crise de equipe (piso de
  reputação etc.) — é rivalidade de piloto, não de time. A cadeia da crise fica no
  backlog de time.
