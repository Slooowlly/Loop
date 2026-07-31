# Atlas Histórico — componentes do v2

Espaço reservado para os componentes do redesenho do Atlas Histórico
(`src/pages/tabs/atlas/GlobalTeamsTabV2.jsx`).

Regra: **nada aqui dentro pode ser importado pelo v1.** Os componentes do atlas
atual vivem um nível acima (`src/components/team/`) e não devem ser editados
durante o redesenho — é isso que mantém o rollback barato: basta voltar
`ATLAS_VERSION` para `1` em `src/pages/tabs/atlas/index.js`.

Reaproveitar código do v1 é permitido, desde que por importação (ex.:
`worldTeamChartGeometry.js`, que é cálculo puro e não desenha nada). Se for
preciso mudar o comportamento de um módulo compartilhado, copie-o para cá em vez
de alterar o original.
