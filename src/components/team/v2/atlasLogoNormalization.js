// Normalização dos brasões de categoria.
//
// O problema: os arquivos não têm nada em comum entre si. Uns são letreiros
// deitados (~2:1, Global Endurance), outros são escudos quase quadrados (M2 Cup,
// GR Cup), e alguns ainda trazem faixa transparente sobrando dentro do próprio
// arquivo (a MX-5 Cup tem quase um terço de altura vazia embaixo).
//
// Encaixar todos numa caixa comum com `object-contain` iguala a CAIXA, e caixa
// igual não é tamanho igual: um escudo cheio e um letreiro fino da mesma altura
// parecem tamanhos diferentes. Fixar altura por marca na mão resolve um arquivo e
// quebra no próximo que entrar.
//
// A regra aqui é ÁREA: cada brasão é escalado para que o conteúdo visível ocupe
// aproximadamente a mesma área, seja ele quadrado ou deitado. Área é o que o olho
// compara quando duas formas diferentes estão lado a lado.
//
// E é o conteúdo VISÍVEL, não o arquivo: a caixa opaca é medida por varredura de
// alfa, então padding transparente de dentro do arquivo não entra na conta nem
// desloca a marca do centro.

// A moldura que a coluna reserva. Fixa: é ela que mantém os títulos dos cards
// alinhados entre si, independentemente do que o brasão faça por dentro.
export const LOGO_FRAME_WIDTH = 48;
export const LOGO_FRAME_HEIGHT = 30;

// Área-alvo do conteúdo visível, em px². Escolhida para que os dois extremos
// caibam na moldura sem serem cortados: um quadrado com essa área tem ~28px de
// lado (cabe nos 30 de altura) e um 2:1 fica com ~39,5 × 19,7 (cabe nos 48 de
// largura). Se um brasão mais extremo entrar, o `Math.min` abaixo o segura.
export const LOGO_TARGET_AREA = 780;

// Alfa a partir do qual o pixel conta como conteúdo. Alto de propósito, e o valor
// foi medido, não chutado: MX-5 Cup, GT3 e GT4 têm um halo fantasma de alfa baixo
// cobrindo a tela INTEIRA do arquivo. Com limiar de 24, esses três devolviam a
// caixa cheia e a varredura não aparava nada — que é o bug que ela existe para
// evitar. Em 128 a caixa cai para o brasão de verdade (MX-5 Cup: 46% do arquivo é
// vazio), e subir até 250 quase não muda nada, sinal de que a borda do conteúdo é
// dura e 128 está com folga dos dois lados.
export const LOGO_ALPHA_THRESHOLD = 128;

// Maior lado usado na varredura. A caixa opaca não precisa de precisão de pixel:
// 256px de amostra dão erro sub-pixel depois de reescalar para os ~30px finais, e
// evitam ler um bitmap de 900px inteiro por brasão.
const SAMPLE_MAX_SIDE = 256;

// Geometria pura, separada da medição: recebe a caixa opaca e devolve como a
// imagem INTEIRA deve ser posicionada para que essa caixa fique centrada na
// moldura e com a área-alvo. Sem canvas e sem DOM — é o que os testes exercitam.
export function normalizedLogoLayout(box, options = {}) {
  const frameWidth = options.frameWidth ?? LOGO_FRAME_WIDTH;
  const frameHeight = options.frameHeight ?? LOGO_FRAME_HEIGHT;
  const targetArea = options.targetArea ?? LOGO_TARGET_AREA;

  if (!box) return null;
  const { x, y, width, height, naturalWidth, naturalHeight } = box;
  if (!(width > 0 && height > 0 && naturalWidth > 0 && naturalHeight > 0)) return null;

  // Escala que dá a área-alvo ao conteúdo visível, limitada pela moldura: um
  // brasão de proporção extrema encosta na borda em vez de ser cortado.
  const areaScale = Math.sqrt(targetArea / (width * height));
  const scale = Math.min(areaScale, frameWidth / width, frameHeight / height);

  // A imagem inteira é escalada, mas quem é centrado é o MEIO DA CAIXA OPACA —
  // é isso que faz o padding assimétrico de dentro do arquivo parar de empurrar
  // a marca para um lado.
  return {
    width: naturalWidth * scale,
    height: naturalHeight * scale,
    left: frameWidth / 2 - (x + width / 2) * scale,
    top: frameHeight / 2 - (y + height / 2) * scale,
  };
}

// Varre o alfa e devolve a caixa opaca em pixels do arquivo original. `null`
// quando não dá para medir (canvas indisponível, imagem não decodificada, arquivo
// inteiro transparente) — o chamador cai no encaixe simples.
export function measureOpaqueBox(image) {
  const naturalWidth = image?.naturalWidth ?? 0;
  const naturalHeight = image?.naturalHeight ?? 0;
  if (!naturalWidth || !naturalHeight) return null;

  const sampleScale = Math.min(1, SAMPLE_MAX_SIDE / Math.max(naturalWidth, naturalHeight));
  const sampleWidth = Math.max(1, Math.round(naturalWidth * sampleScale));
  const sampleHeight = Math.max(1, Math.round(naturalHeight * sampleScale));

  let pixels;
  try {
    const canvas = document.createElement("canvas");
    canvas.width = sampleWidth;
    canvas.height = sampleHeight;
    const context = canvas.getContext("2d", { willReadFrequently: true });
    if (!context) return null;
    context.drawImage(image, 0, 0, sampleWidth, sampleHeight);
    pixels = context.getImageData(0, 0, sampleWidth, sampleHeight).data;
  } catch {
    // Sem canvas (jsdom) ou canvas contaminado: medir é otimização, não requisito.
    return null;
  }

  let minX = sampleWidth;
  let minY = sampleHeight;
  let maxX = -1;
  let maxY = -1;
  for (let row = 0; row < sampleHeight; row += 1) {
    for (let column = 0; column < sampleWidth; column += 1) {
      const alpha = pixels[(row * sampleWidth + column) * 4 + 3];
      if (alpha <= LOGO_ALPHA_THRESHOLD) continue;
      if (column < minX) minX = column;
      if (column > maxX) maxX = column;
      if (row < minY) minY = row;
      if (row > maxY) maxY = row;
    }
  }
  if (maxX < 0) return null;

  // De volta à escala do arquivo: o layout trabalha em pixels naturais.
  const back = 1 / sampleScale;
  return {
    x: minX * back,
    y: minY * back,
    width: (maxX - minX + 1) * back,
    height: (maxY - minY + 1) * back,
    naturalWidth,
    naturalHeight,
  };
}
