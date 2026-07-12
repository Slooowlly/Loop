# Foco da Equipe + Vínculo piloto-equipe — design

**Data:** 2026-07-11
**Escopo:** ideia 4 do "sistema de equipes vivo" (identidade/DNA), redesenhada como
**fase que evolui** (não personalidade eterna) + a relação de longo prazo piloto-equipe.
Objetivo emocional do usuário: **um time segurar um piloto pra fazer história junto**, em
vez de ele pular de galho em galho.

## Princípio de arquitetura

Todas as 11 ideias aprovadas são **consequências de 2 primitivos novos**. Motor pequeno,
muitos frutos — nada de features soltas.

### ① FOCO DA EQUIPE (estado do time, evolui)

A filosofia atual da equipe, um de 6 focos. **Não é permanente.**

| Foco | Entra quando | Caráter |
|---|---|---|
| **Sobrevivência** | `financial_state` crise/colapso | mercenário, sem lealdade, o mais barato |
| **Reconstrução** | rebaixado (últimas 1-2 temp.) / recém-vendido / plano `rebuild` | aposta com expectativa baixa, paciente, carro fraco |
| **Celeiro** | estável/saudável, não-elite, plano `sustainable` | forma jovem, paciente, desenvolve rápido, pode vender pra cima |
| **Meio-de-grid ambicioso** | estável, meio de tabela, em ascensão | oportunista, quer subir |
| **Projeto de título** | plano `title_push` / saudável com carro competitivo | quer provado, paga alto, cobra muito |
| **Dinastia** | elite designada / plano `elite_dominance` / títulos recentes | prestígio + recursos, cobra mas banca |

**Evolução (3 mecanismos combinados):**
1. **Derivado** de `financial_state` + `strategic_plan` (Pilar C) + trajetória de reputação — já existem.
2. **Vira em eventos** (venda/nova diretoria, promoção/rebaixamento, título) — anunciado como notícia.
3. **Histerese**: dwell mínimo ~2 temporadas; um evento "duro" força a virada na hora, senão espera.

### ② VÍNCULO PILOTO-EQUIPE (por par, acumula)

Valor interno 0–100 por (piloto, equipe); **cresce** a cada temporada juntos (+ rápido vencendo),
**decai devagar** quando separados (mantém a "casa" morna p/ o filho pródigo). Exibido como
**selo qualitativo de 6 níveis** (decisão do usuário — sem número cru):

| Nível | Faixa | Selo |
|---|---|---|
| 1 | 0–14 | **Recém-chegado** |
| 2 | 15–32 | **Entrosado** |
| 3 | 33–52 | **Confiança** |
| 4 | 53–72 | **Pilar do time** |
| 5 | 73–89 | **Símbolo da equipe** |
| 6 | 90–100 | **Casa** |

Acúmulo (a calibrar): base ~+12/temporada juntos; bônus título ~+15, pódios/vitórias menor;
decaimento ~−8/temporada separados. ~1,5–2 temporadas por nível.

## Os 11 frutos → mapeamento

| Fruto | Emerge de | Sistema existente |
|---|---|---|
| Vínculo piloto-equipe | **primitivo ②** | novo (tabela) |
| Contrato de projeto plurianual | Foco (celeiro/dinastia) + Vínculo alto | contratos (temporada_inicio/fim) + Pilar C |
| Confiança acumulada (buffer) | Vínculo perdoa 1 temporada ruim na renovação | mercado/renovação (fim de temporada) |
| Segurar vs vender pra cima | Foco (celeiro vende / dinastia segura) × Vínculo | poaching/mercado |
| Academia / jovens | Foco=Celeiro → recruta jovem + boost crescimento + preferência | evolução de piloto |
| Mentor veterano→jovem | Foco de desenvolvimento pareia jovem+veterano | hierarquia N1/N2 |
| Arco de expectativa | Foco define inclinação; Vínculo indexa | `meta_posicao` |
| Casa espiritual / filho pródigo | Vínculo máximo da carreira | novo (leve) |
| Honras de legado (nº, era dourada) | Vínculo alto + título | **história de equipe (ideia 2)** |
| Saída dolorosa vs troca fria | custo de deixar Vínculo alto | reputação |
| Grade toda constrói duplas longas | os 2 primitivos rodam em TODO par | simétrico jogador+IA |

## Decisões travadas (usuário)

- **Sem dispensa no meio da temporada** — toda consequência de lealdade/paciência cai na
  **renovação (fim de temporada)**. O drama é "vão me renovar?", nunca "fui demitido hoje".
- **Você sempre decide** — quando um time maior te quer e o leal quer segurar, o leal só
  **persuade** (contra-oferta, projeto plurianual, puxão de legado/selo). Nunca te prende.
  Coerente com o mercado atual (jogador tem a palavra final).
- **Selo de 6 níveis** (acima), não número cru.

## Fases

### Fase 1 — Fundação (os 2 primitivos + consequências diretas)
- **Foco**: derivação + eventos + histerese; visível na ficha do time + notícia da virada.
- **Vínculo**: tabela `driver_team_bond` (piloto, equipe, valor); acúmulo/decaimento no offseason;
  selo de 6 níveis na UI.
- **Consequências**: renovação leal (buffer do Vínculo + contrato plurianual conforme Foco+Vínculo);
  segurar-vs-vender (Foco × Vínculo; jogador decide, IA resolve sozinha).
- Simétrico jogador+IA. **Calibração MC**: duração média de vínculo (temporadas/par), % de pilotos
  ≥3 temporadas no mesmo time, nº de "duplas de era" (≥4 temporadas), sem congelar o mercado.

### Fase 2 — Desenvolvimento
- Academia (Foco=Celeiro recruta jovem + boost de crescimento + direito de preferência).
- Mentor veterano→jovem (dupla acelera via hierarquia).
- Arco de expectativa plurianual (aprender→pódio→título; pesa na renovação).

### Fase 3 — Peso emocional / mundo vivo
- Casa espiritual + filho pródigo (Vínculo máximo da carreira; volta restaura rápido).
- Honras de legado (número aposentado, "era dourada" nomeada) — liga na história de equipe.
- Saída dolorosa vs troca fria (custo de reputação ao deixar Vínculo alto).
- Leitura da grade (duplas longas das IAs visíveis em notícias/dossiês).

## Riscos / o que vigiar

- **Não congelar o mercado**: lealdade demais elimina a rotatividade e mata o mercado semanal.
  Alavancas: força do buffer, base de acúmulo, quão forte o Foco "segura". MC precisa mostrar
  duplas longas SEM travar tudo (alguns pilotos ainda circulam; craques ainda são fisgados).
- **Sistema mais central** (mercado/contratos): cirurgia com testes verdes a cada checkpoint.
- **Player-agency**: o time leal persuade, nunca bloqueia (decisão travada).

## Aberto (resolver na implementação)
- Fórmula exata de derivação do Foco (limiares) + tabela de parâmetros por Foco.
- Curva de acúmulo/decaimento do Vínculo (calibrar no MC).
- Onde o buffer entra no fluxo de renovação atual (market/pipeline + janela de transferências).
