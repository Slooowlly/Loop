# Ficha do piloto — componentes do v2

Espaço reservado para o redesenho da ficha do piloto
(`src/components/driver/v2/DriverDetailModalV2.jsx`).

Regra: **nada aqui dentro pode ser importado pelo v1.** A ficha atual vive um
nível acima (`src/components/driver/DriverDetailModal.jsx`) e não deve ser
editada durante o redesenho — é isso que mantém o rollback barato: basta voltar
`DRIVER_DETAIL_VERSION` para `1` em `src/components/driver/index.js`.

Reaproveitar código do v1 é permitido, desde que por importação (é o caso de
`detalhes/formatadores.js`, que é formatação pura, e do dossiê de habilidade do
jogador, que já é um bloco fechado). Se for preciso mudar o comportamento de um
módulo compartilhado, copie-o para cá em vez de alterar o original — o guard
estrutural `scripts/tests/driver-detail-modal.test.mjs` lê o fonte do v1 como
texto e quebra se ele mudar de forma.
