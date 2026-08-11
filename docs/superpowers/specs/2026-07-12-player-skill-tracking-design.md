# Dossiê de Habilidade do Jogador — atributos reais rastreados da pista

**Data:** 2026-07-12
**Status:** ⚠️ **Retrato histórico, conferido em 11/08/2026.** Dizia "não implementado", e o
módulo `src-tauri/src/player_skill.rs` está no ar e cita este arquivo como referência de design.
Leia como a intenção original, e não como estado do app.
**Autor:** design conjunto (jogador + assistente)

## 1. Objetivo

Dar ao piloto-jogador os **mesmos 18 atributos** que a IA tem, mas **inferidos do
desempenho real na pista** (iRacing), e mostrados apenas como **camada visual de
progressão**. O mercado **NÃO** consulta esses valores para contratar — é só
feedback para o jogador ver a própria evolução.

## 2. A inversão fundamental

- **IA:** atributos são a **fonte da verdade** → alimentam o export/roster e a
  simulação; `evolution/growth.rs` os evolui um pouco por temporada.
- **Jogador:** **resultado real → infere atributo** (sentido inverso). NÃO reusar
  `growth.rs`. Construir um **estimador** que lê o histórico real de corridas e
  produz notas de exibição.

Como é camada puramente visual e desacoplada da simulação, o requisito "o mercado
não considera" sai de graça.

## 3. Princípio central do `skill`: posição no grid, nunca o número absoluto

**Problema:** a força efetiva de um mesmo skill varia por pista (há pistas onde a
IA precisa de skill 100+ para andar bem; a média fica ~80). Comparar a posição
final do jogador com o número **absoluto** da IA não funciona.

**Solução:** nós geramos o grid e sabemos o skill de cada IA daquela corrida
(`roster_gen.rs` exporta `driver_skill = attr(skill)`, praticamente 1:1). Então a
leitura correta é **relativa ao grid daquela corrida**:

> Chegou em P4, passou IAs de skill {82, 80, 78} e perdeu para {88, 85} →
> skill-da-corrida ≈ interpolado na **fronteira cruzada** (entre 80 e 85).

O efeito da pista **se cancela**: todas as IAs do grid rodaram a mesma pista, logo
a *ordenação* delas já embute a esquisitice da pista. Nunca se usa o número
absoluto do iRacing — só a ordem do nosso próprio grid. Agrega-se a fronteira ao
longo de N corridas; a confiança aperta com o tempo.

Mesmo mecanismo para `ritmo_classificacao`, usando o grid da **quali**.

### Fórmula (skill / ritmo_classificacao)
Por corrida, com a lista `(ai_skill, finish_pos)` do grid + `finish_pos` do jogador:
- `acima` = maior `ai_skill` que o jogador **perdeu** (ficou atrás)
- `abaixo` = menor `ai_skill` que o jogador **venceu** (ficou à frente)
- `skill_corrida` = média(`acima`, `abaixo`); se venceu todos → `max_grid + margem`;
  se perdeu de todos → `min_grid - margem`
- estimativa exibida = média móvel de `skill_corrida` sobre a janela de corridas,
  com confiança = f(nº de corridas com grid válido)

## 4. Dois grupos por disponibilidade de dado

### Grupo A — computável JÁ (resultado oficial + roster gerado)

Dados: `ResultPos` (posição, melhor volta, voltas lideradas, **incidents**,
reason_out) + grid da quali + mapa número→driver_id (já existe no import) +
`historico_circuitos` + sistema de fama.

| Atributo | Fórmula | Desbloqueio |
|---|---|---|
| `experiencia` | contagem de corridas completadas | imediato |
| `skill` | interpolação no grid da corrida (§3) | ~5 corridas c/ grid |
| `ritmo_classificacao` | interpolação no grid da quali (§3) | ~5 qualis |
| `aggression` | taxa de (incidentes + contatos + offtracks) por corrida | ~5 corridas |
| `racecraft` (básico) | posições ganhas (grid → chegada), média | ~5 corridas |
| `fator_chuva` | desempenho em molhado vs. IAs de `fator_chuva` alto no mesmo grid molhado (se foi tão bem quanto elas, é alto) | ~2–3 corridas de chuva OU temporadas |
| `adaptabilidade` | tendência dos resultados ao longo das temporadas (melhorou?) | ≥2 temporadas |
| `midia` | leitura direta do valor do sistema de fama (`fame.rs`) | imediato |

### Grupo B — precisa captura NOVA de telemetria ao vivo

O YAML de resultado só tem **melhor volta + última volta** (não a série
volta-a-volta), nem proximidade/contato, nem posição em instante T. Precisa o
monitor ao vivo amostrar e persistir.

| Atributo | Fórmula pretendida | Dado novo necessário |
|---|---|---|
| `consistencia` | variância dos tempos de volta | série de tempos por volta |
| `racecraft` (avançado) | tempo sem contato / tempo em briga | proximidade + contatos ao vivo |
| `habilidade_largada` | posições ganhas/perdidas nos **primeiros ~20 s** (não a volta toda) | posição em T+20s |

### Fora do escopo (decisão do jogador)
`smoothness` (exigiria capturar inputs de acelerador/freio — custoso demais),
`fitness`, `desenvolvimento`, `confianca` (ambíguo), `mentalidade`,
`gestao_pneus`, `defesa`, `carisma`.

## 5. Estimativa que ganha confiança + desbloqueio progressivo

- Cada atributo tem **threshold de desbloqueio** e uma **confiança crescente**.
- Antes do threshold: exibir "bloqueado" com a meta —
  *"Corra 5 corridas para revelar seu Ritmo de Classificação"*,
  *"Complete 2 temporadas para revelar sua Adaptabilidade"*.
- Depois: valor exibido "cinza/incerto" nas primeiras amostras, firmando conforme
  acumula corridas. Vira mecânica de progressão visível.

## 6. Pontos de calibração a resolver na implementação

1. **Base do skill do grid:** usar skill cru ou o efetivo pós-penalidade de pista?
   A penalidade de conhecimento depende do histórico/adaptabilidade de cada um, não
   é uniforme — decidir qual base representa melhor a "nota de catálogo" que o
   jogador vê.
2. **Margem** quando o jogador vence/perde todo o grid.
3. **Janela** da média móvel (recência vs. estabilidade).
4. **Amostra mínima de chuva** antes de revelar `fator_chuva`.

## 7. Faseamento sugerido

- **Fase 1 (Grupo A):** estimador backend lendo histórico real + os atributos
  A + UI de desbloqueio. Entrega valor imediato sem telemetria nova.
- **Fase 2 (Grupo B):** camada de captura volta-a-volta / posição-no-tempo no
  monitor ao vivo, destravando consistência, racecraft avançado e largada.
