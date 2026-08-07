// Decodificação do PCM que vem do Gemini TTS: base64 -> bytes -> Float32 mono.
//
// O formato documentado é PCM linear 16 bits, 24 kHz, mono, little-endian. Duas
// armadilhas do mundo real que este arquivo cobre:
//
// 1) O PRIMEIRO bloco pode vir com cabeçalho RIFF/WAVE na frente. Se isso for tocado
//    como amostra, o ouvinte escuta um estalo no começo — justamente no instante que
//    a POC está cronometrando.
// 2) Um bloco pode terminar no MEIO de uma amostra (número ímpar de bytes). Sem
//    guardar o byte solto para o bloco seguinte, todas as amostras dali para frente
//    saem deslocadas em um byte e o áudio vira ruído branco.

/** base64 -> Uint8Array, sem passar por string intermediária maior que o necessário. */
export function base64ParaBytes(b64) {
  const binario = atob(b64);
  const bytes = new Uint8Array(binario.length);
  for (let i = 0; i < binario.length; i += 1) bytes[i] = binario.charCodeAt(i);
  return bytes;
}

/**
 * Se houver cabeçalho RIFF, devolve o offset onde começam as amostras (o corpo do
 * sub-bloco "data"). Sem cabeçalho, devolve 0. Não assume os 44 bytes canônicos: o
 * WAV admite blocos extras antes do "data".
 */
export function offsetDoCorpoWav(bytes) {
  if (bytes.length < 12) return 0;
  const eRiff =
    bytes[0] === 0x52 && bytes[1] === 0x49 && bytes[2] === 0x46 && bytes[3] === 0x46; // "RIFF"
  const eWave =
    bytes[8] === 0x57 && bytes[9] === 0x41 && bytes[10] === 0x56 && bytes[11] === 0x45; // "WAVE"
  if (!eRiff || !eWave) return 0;

  let i = 12;
  while (i + 8 <= bytes.length) {
    const eData =
      bytes[i] === 0x64 && bytes[i + 1] === 0x61 && bytes[i + 2] === 0x74 && bytes[i + 3] === 0x61; // "data"
    const tamanho =
      bytes[i + 4] | (bytes[i + 5] << 8) | (bytes[i + 6] << 16) | (bytes[i + 7] << 24);
    if (eData) return i + 8;
    i += 8 + tamanho + (tamanho % 2); // blocos do WAV são alinhados em 2 bytes
  }
  return 0;
}

/**
 * Conversor com estado: mantém o byte órfão entre blocos e come o cabeçalho RIFF
 * apenas do primeiro. Um por fala.
 */
export function criarConversorPcm() {
  let sobra = null; // byte ímpar que ficou do bloco anterior
  let primeiro = true;

  return {
    /** @returns {Float32Array} amostras em [-1, 1] */
    converter(b64) {
      let bytes = base64ParaBytes(b64);

      if (primeiro) {
        const offset = offsetDoCorpoWav(bytes);
        if (offset > 0) bytes = bytes.subarray(offset);
        primeiro = false;
      }

      if (sobra !== null) {
        const juntos = new Uint8Array(bytes.length + 1);
        juntos[0] = sobra;
        juntos.set(bytes, 1);
        bytes = juntos;
        sobra = null;
      }

      if (bytes.length % 2 === 1) {
        sobra = bytes[bytes.length - 1];
        bytes = bytes.subarray(0, bytes.length - 1);
      }

      const total = bytes.length / 2;
      const amostras = new Float32Array(total);
      for (let i = 0; i < total; i += 1) {
        // s16le -> inteiro com sinal -> [-1, 1). 32768 (e não 32767) para que o
        // valor mínimo mapeie exatamente em -1 sem estourar.
        const bruto = bytes[i * 2] | (bytes[i * 2 + 1] << 8);
        amostras[i] = (bruto & 0x8000 ? bruto - 0x10000 : bruto) / 32768;
      }
      return amostras;
    },
  };
}
