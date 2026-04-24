# Briefing Form Realism and Persistence Design

## Objetivo

Corrigir dois problemas no briefing pre-corrida:

- a forma recente nao pode sumir ao reabrir a carreira
- a narrativa deve respeitar desempenho recente, diferenca de pontos e plausibilidade real de titulo/vitoria

## Decisoes

- O backend passa a tratar `race_results.json` como fonte principal de historico recente, com fallback para `ultimos_resultados` persistidos no banco do piloto.
- O frontend do briefing passa a usar um contexto de expectativa mais realista, derivado de:
  - forma recente
  - ritmo de top 5 / podio / vitoria
  - gap para o lider
  - etapas restantes
  - favoritismo relativo
- Textos editoriais deixam de assumir “ataque ao titulo” quando o contexto esportivo for improvavel.

## Impacto esperado

- Reabrir o jogo nao deve zerar a forma recente no briefing.
- Headlines, metas e cenarios passam a refletir campanha realista, evitando promessas irreais de vitoria ou titulo.
