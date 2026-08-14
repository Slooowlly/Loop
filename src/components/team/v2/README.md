# Atlas Histórico — componentes do v2

Componentes do Atlas Histórico (`src/pages/tabs/atlas/GlobalTeamsTabV2.jsx`) e o
dossiê de equipe (`TeamHistoryDrawerV2.jsx`).

O nome "v2" é histórico: durante o redesenho o atlas v1 vivia um nível acima
(`src/components/team/WorldTeamHistoryGrid.jsx`, `YearWindowScrubber.jsx`) e
servia de rollback, e a regra era não editá-lo. O v1 foi removido em 11/08/2026
— `src/pages/tabs/atlas/index.js` é apenas o ponto de entrada hoje.

O que continua valendo: `../worldTeamChartGeometry.js` (cálculo puro, sem
desenho) e `../teamHistoryDossier.js` (normalização do payload de
`get_team_history_dossier`) moram um nível acima porque são compartilhados.
Mudar o comportamento deles arrasta os dois consumidores.

## O dossiê de equipe, por arquivo

`TeamHistoryDrawerV2.jsx` tinha 4.298 linhas e foi decomposto em 11/08/2026. O
critério da divisão é o mesmo em toda a pasta: **conteúdo** (o que decide ordem,
corte e cor derivada de dado) e **texto** ficam nos módulos puros, **desenho**
fica nos painéis, e o drawer guarda só a composição.

| Arquivo | Papel |
|---|---|
| `TeamHistoryDrawerV2.jsx` | Carregamento do dossiê, estado de alto nível, navegação entre equipes, cabeçalho-herói e as seções Records e Identidade |
| `teamHistoryV2Logic.js` | Conteúdo: paleta de colocação, contraste de tinta, se um gráfico tem dado, ranking de pilotos |
| `teamHistoryV2Labels.js` | Texto: dicas, rótulos de faixa e a geometria dos chips de rodada |
| `teamHistoryV2Primitives.jsx` | Vocabulário comum dos painéis: rótulo de bloco, legenda de medalha, par rótulo-valor, ícone de métrica |
| `TeamHistoryTitles.jsx` | Galeria de títulos: régua de anos, grupo por categoria, campeão de pilotos |
| `TeamHistoryTrajectory.jsx` | Faixa de top 5 por temporada, o balão dela e a fita de forma recente |
| `TeamHistoryChampionship.jsx` | Seletor entre as duas vistas do campeonato |
| `TeamHistoryChampionshipRun.jsx` | Campanha acumulada rodada a rodada (eixo normal) |
| `TeamHistoryChampionshipCurve.jsx` | Posição final por temporada (eixo invertido, P1 no topo) |
| `TeamHistoryResults.jsx` | Assinatura de resultados e confiabilidade |
| `TeamHistoryLineup.jsx` | Galeria de passagens por vaga e ranking dos melhores pilotos |
| `TeamHistoryIdentity.jsx` | Seção Rival: duelo, recrutamento, afinidade por pista |
| `TeamHistoryMoney.jsx` | Seção Gestão: curva de caixa e fluxo de dinheiro |
| `TeamHistoryCategories.jsx` | Seção Categorias: pirâmide da escada, trajetória de nível, tempo por degrau |

As duas geometrias do campeonato moram em arquivos separados de propósito: elas
desenham com eixos opostos, e lado a lado uma acaba lida como a outra.
