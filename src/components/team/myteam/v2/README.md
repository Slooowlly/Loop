# Minha Equipe — componentes do v2

Componentes da aba Minha Equipe (`src/pages/tabs/myteam/MyTeamTabV2.jsx`).

O nome "v2" é histórico: durante o redesenho o v1 vivia um nível acima
(`CommandHeader`, `CostChart`, `DriverPanel`, `FinanceDossier`, `GaragePanel`,
`RankingTable`, `TechPanel`) e servia de rollback, e a regra era não editá-lo. O
v1 foi removido em 11/08/2026 — `src/pages/tabs/myteam/index.js` é apenas o ponto
de entrada hoje.

O que sobreviveu um nível acima é o que nunca foi do v1: `../teamMetrics.js`
(cálculo puro) e `../../teamFinanceLabels.js`.

## O que mudou em relação ao v1

O v1 imprimia ~20 valores em dinheiro com o mesmo peso visual, repetia caixa três
vezes (cabeçalho, KPI e bloco de fluxo) e precisava de um parágrafo de legenda
para explicar o horizonte de cada número. O v2 reorganiza por **horizonte de
tempo** (agora / esta rodada / esta temporada) e troca número solto por gráfico
onde havia comparação implícita:

| Gráfico | Componente | Substitui |
|---|---|---|
| Resultado por rodada + prêmio projetado | `RoundLedgerChart` | `FinanceDossier` (barras de caixa + bloco de projeção) |
| Medidores com régua da média do grid | `MeterBar` em `CarPanelV2` | `TechPanel` (barras sem referência) |
| Radar contra média e líder | `GridRadar` | as colunas de tier do `RankingTable` |
| Dispersão caixa × pontos | `EfficiencyScatter` | — (leitura que o v1 não oferecia) |

## Fronteira de dados

`gridMetrics.js` é o único lugar que faz conta. Ele consome exatamente o que a
aba já busca — `get_teams_standings`, `get_team_finance_report` e o `season` do
store — e **nenhum campo novo foi pedido ao backend**. Duas consequências que os
componentes precisam respeitar:

- `TeamStanding` não traz `presenca_publica` nem salários das outras equipes.
  Presença e folha salarial são medidores **sem régua** — mostrar uma média de
  grid ali seria inventar dado.
- O eixo do radar normaliza pelo **máximo do grid**, não por um teto absoluto.
  A forma do polígono é posição relativa, não nota.

## Duas armadilhas que já custaram caro aqui

**SVG com viewBox fixo e `width: 100%` cresce na vertical.** O navegador preserva a
proporção, então um viewBox de 600x172 numa tela de 1920 vira um gráfico de 470px de
altura. Todo gráfico que estica na horizontal mede a largura real do card
(`useElementSize`) e usa essa largura COMO viewBox, com a altura fixada por nós —
`chartView` existe para isso.

**Ref de medição em nó condicional nunca é medido.** O efeito do `useElementSize`
roda uma única vez, na montagem. Os dados chegam por `invoke` DEPOIS da primeira
renderização, então um ref colocado dentro do bloco "tem dados" nasce nulo e fica
nulo: o gráfico desenha com a largura de fallback e o `preserveAspectRatio`
centraliza o desenho, deixando margem morta dos dois lados. O ref vai sempre num
wrapper que existe em todos os estados, inclusive no vazio.

**Escala de eixo não se ancora em valor distante.** Ancorar em zero, ou incluir a
reserva mínima na faixa do eixo, achata a série contra uma borda — com caixa de $1M e
variação de $25k por rodada, 95% da altura vira espaço morto. Por isso o gráfico
principal mostra o resultado POR RODADA, e não o saldo acumulado: o saldo é sempre
plano na própria escala.
