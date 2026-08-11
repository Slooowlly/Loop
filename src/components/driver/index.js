// Ponto de entrada da ficha do piloto.
//
// Houve aqui um seletor de versão (`DRIVER_DETAIL_VERSION`) enquanto o v1 — o
// drawer de borda com abas em pílula — servia de rollback do redesenho. O v1 foi
// removido em 11/08/2026; sobrou o v2, que é a tela centralizada com seções na
// lateral, números de carreira com barra de posição, forma recente em colunas e
// escada de categorias.
export { DriverDetailModalV2 as DriverDetailModal } from "./v2/DriverDetailModalV2";
export { DriverDetailModalV2 as default } from "./v2/DriverDetailModalV2";
