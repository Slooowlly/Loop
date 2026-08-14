// i18n-ignore-file — bancada da POC de TTS, fora do caminho do jogador.
// A cadeia de rádio saiu daqui: virou `src/lib/filtroRadio.js` quando o engenheiro passou
// a precisar dela em corrida, e não só na bancada. Este arquivo continua existindo com a
// assinatura antiga (`ligado` posicional) só para a POC não ter de ser reescrita — a
// receita é uma só, do outro lado.

import { criarCadeiaRadio as cadeia } from "../../lib/filtroRadio";

export function criarCadeiaRadio(ctx, ligado) {
  return cadeia(ctx, { ligado });
}
