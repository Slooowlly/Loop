import { describe, it, expect } from "vitest";
import { base64ParaBytes, offsetDoCorpoWav, criarConversorPcm } from "./ttsPcm";

function bytesParaBase64(bytes) {
  let binario = "";
  for (const b of bytes) binario += String.fromCharCode(b);
  return btoa(binario);
}

/** Monta um WAV mínimo (cabeçalho canônico de 44 bytes) em volta do corpo. */
function comCabecalhoWav(corpo) {
  const cabecalho = new Uint8Array(44);
  const escreverAscii = (offset, texto) => {
    for (let i = 0; i < texto.length; i += 1) cabecalho[offset + i] = texto.charCodeAt(i);
  };
  const escreverU32 = (offset, valor) => {
    cabecalho[offset] = valor & 0xff;
    cabecalho[offset + 1] = (valor >> 8) & 0xff;
    cabecalho[offset + 2] = (valor >> 16) & 0xff;
    cabecalho[offset + 3] = (valor >> 24) & 0xff;
  };
  escreverAscii(0, "RIFF");
  escreverU32(4, 36 + corpo.length);
  escreverAscii(8, "WAVE");
  escreverAscii(12, "fmt ");
  escreverU32(16, 16);
  escreverAscii(36, "data");
  escreverU32(40, corpo.length);

  const tudo = new Uint8Array(44 + corpo.length);
  tudo.set(cabecalho, 0);
  tudo.set(corpo, 44);
  return tudo;
}

describe("ttsPcm", () => {
  it("decodifica base64 preservando os bytes", () => {
    const original = new Uint8Array([0, 1, 2, 250, 255]);
    expect(Array.from(base64ParaBytes(bytesParaBase64(original)))).toEqual(Array.from(original));
  });

  it("acha o corpo depois do cabeçalho RIFF", () => {
    const corpo = new Uint8Array([1, 2, 3, 4]);
    expect(offsetDoCorpoWav(comCabecalhoWav(corpo))).toBe(44);
  });

  it("devolve offset zero quando o bloco é PCM cru", () => {
    expect(offsetDoCorpoWav(new Uint8Array([0, 0, 1, 0, 2, 0]))).toBe(0);
  });

  it("descarta o cabeçalho apenas do primeiro bloco", () => {
    const conversor = criarConversorPcm();
    // Uma amostra de valor 1 (0x0001 little-endian).
    const primeiro = conversor.converter(bytesParaBase64(comCabecalhoWav(new Uint8Array([1, 0]))));
    expect(primeiro.length).toBe(1);
    const segundo = conversor.converter(bytesParaBase64(new Uint8Array([1, 0, 2, 0])));
    expect(segundo.length).toBe(2);
  });

  it("converte s16le com sinal para a faixa [-1, 1]", () => {
    const conversor = criarConversorPcm();
    // 0x8000 = -32768 -> -1 ; 0x4000 = 16384 -> 0.5
    const amostras = conversor.converter(bytesParaBase64(new Uint8Array([0x00, 0x80, 0x00, 0x40])));
    expect(amostras[0]).toBeCloseTo(-1, 5);
    expect(amostras[1]).toBeCloseTo(0.5, 5);
  });

  it("carrega o byte órfão para o bloco seguinte em vez de deslocar as amostras", () => {
    const conversor = criarConversorPcm();
    // Bloco ímpar: o 0x00 final é a metade baixa da próxima amostra.
    const a = conversor.converter(bytesParaBase64(new Uint8Array([0x00, 0x40, 0x00])));
    expect(a.length).toBe(1);
    expect(a[0]).toBeCloseTo(0.5, 5);

    // Chega a metade alta (0x40) -> junto com a sobra forma 0x4000 = 0.5.
    const b = conversor.converter(bytesParaBase64(new Uint8Array([0x40])));
    expect(b.length).toBe(1);
    expect(b[0]).toBeCloseTo(0.5, 5);
  });
});
