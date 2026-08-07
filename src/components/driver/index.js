// Seletor de versão da ficha do piloto.
//
// O v1 é o painel em produção (drawer de borda com abas em pílula) e continua
// exatamente onde sempre esteve: src/components/driver/DriverDetailModal.jsx. O
// v2 é o redesenho — tela centralizada, seções na lateral, números de carreira
// com barra de posição, forma recente em colunas e escada de categorias —
// isolado em ./v2/DriverDetailModalV2.jsx.
//
// Para trocar de versão, mude o número abaixo — é o único ponto de decisão.
// Voltar para o v1 é reverter esta linha; o v1 não desenha diferente por causa
// do v2, e o guard estrutural scripts/tests/driver-detail-modal.test.mjs
// continua valendo sobre ele.
import DriverDetailModalV1 from "./DriverDetailModal";
import { DriverDetailModalV2 } from "./v2/DriverDetailModalV2";

export const DRIVER_DETAIL_VERSION = 2;

export const DriverDetailModal =
  DRIVER_DETAIL_VERSION === 2 ? DriverDetailModalV2 : DriverDetailModalV1;

export { DriverDetailModalV1, DriverDetailModalV2 };
export default DriverDetailModal;
