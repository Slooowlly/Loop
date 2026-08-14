// Duração de uma peça de voz, em segundos, sem decodificar o áudio.
//
// O acervo virou Opus (ver `audio-para-opus.mjs`), e com ele o cabeçalho RIFF — que dava a
// duração por uma divisão — deixou de existir. Em Ogg Opus a informação está no GRANULEPOS da
// última página: um contador de amostras a 48 kHz, sempre 48 kHz, independente da taxa que
// entrou no encoder. Menos o `pre-skip` declarado no `OpusHead`, que são as amostras de
// aquecimento do decodificador e não fazem parte da fala.
//
// Por que isso importa e não é detalhe de formato: dois guards agendam fala pelo FIM dela
// (`DESPEDIDA_MAX_S`, a linha de chegada). Uma duração errada aqui não quebra teste nenhum —
// ela deixa o engenheiro falando enquanto o piloto entra na curva.
//
// A leitura é por seek, não por `readFileSync`. Não é zelo prematuro: `analise-radio.mjs`
// percorre o acervo inteiro a cada rodada, e ler milhares de arquivos por inteiro para
// extrair 16 bytes de cada é o tipo de custo que só aparece quando o acervo dobra.

import fs from "node:fs";

/// Maior página Ogg possível: 27 bytes de cabeçalho + 255 lacetes de segmento + 255×255 de
/// carga. Ler esta cauda garante que a última página está dentro dela.
const CAUDA_MAX = 27 + 255 + 255 * 255;

/// Duração de um `.opus` (Ogg Opus) em segundos, ou `null` se o arquivo não for um.
export function duracaoOpus(arquivo) {
  const fd = fs.openSync(arquivo, "r");
  try {
    const tam = fs.fstatSync(fd).size;
    if (tam < 47) return null;

    // ── pre-skip, da primeira página (é onde o `OpusHead` mora, sozinho) ──
    const cab = Buffer.alloc(64);
    fs.readSync(fd, cab, 0, Math.min(64, tam), 0);
    if (cab.toString("ascii", 0, 4) !== "OggS") return null;
    const inicioPacote = 27 + cab[26];
    if (cab.toString("ascii", inicioPacote, inicioPacote + 8) !== "OpusHead") return null;
    const preSkip = cab.readUInt16LE(inicioPacote + 10);

    // ── granulepos da ÚLTIMA página ──
    const nCauda = Math.min(CAUDA_MAX, tam);
    const cauda = Buffer.alloc(nCauda);
    fs.readSync(fd, cauda, 0, nCauda, tam - nCauda);
    // De trás para frente: a última captura de "OggS" é a última página.
    let ultima = -1;
    for (let i = nCauda - 27; i >= 0; i--) {
      if (cauda[i] === 0x4f && cauda[i + 1] === 0x67 && cauda[i + 2] === 0x67 && cauda[i + 3] === 0x53) {
        ultima = i;
        break;
      }
    }
    if (ultima < 0) return null;

    const granulo = Number(cauda.readBigUInt64LE(ultima + 6));
    const amostras = granulo - preSkip;
    return amostras > 0 ? amostras / 48000 : null;
  } finally {
    fs.closeSync(fd);
  }
}

/// Duração de um `.wav` PCM pelo cabeçalho RIFF. Os masters do acervo continuam em WAV no
/// disco, e as POCs de TTS geram WAV — esta metade não morreu com a conversão.
export function duracaoWav(arquivo) {
  const fd = fs.openSync(arquivo, "r");
  try {
    const tam = fs.fstatSync(fd).size;
    const cab = Buffer.alloc(12);
    fs.readSync(fd, cab, 0, 12, 0);
    if (cab.toString("ascii", 0, 4) !== "RIFF") return null;
    // Percorre os chunks: `fmt ` dá a taxa de bytes, `data` dá o tamanho. A ordem é canônica
    // nos arquivos do gerador, mas o laço não depende disso.
    let pos = 12;
    let bytesPorSegundo = 0;
    const buf = Buffer.alloc(8);
    while (pos + 8 <= tam) {
      fs.readSync(fd, buf, 0, 8, pos);
      const id = buf.toString("ascii", 0, 4);
      const n = buf.readUInt32LE(4);
      if (id === "fmt ") {
        const fmt = Buffer.alloc(16);
        fs.readSync(fd, fmt, 0, 16, pos + 8);
        bytesPorSegundo = fmt.readUInt32LE(8);
      } else if (id === "data") {
        return bytesPorSegundo ? n / bytesPorSegundo : null;
      }
      pos += 8 + n + (n % 2);
    }
    return null;
  } finally {
    fs.closeSync(fd);
  }
}
