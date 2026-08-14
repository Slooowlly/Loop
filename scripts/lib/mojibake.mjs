// Detector de mojibake: texto UTF-8 que alguém leu como Latin-1 e regravou.
//
// Mora aqui, e não dentro do teste, por dois motivos. O primeiro é poder alimentar o
// detector com uma linha sintética, provando que ele ainda morde, em vez de só provar que a
// árvore está limpa hoje. O segundo é que o guard passou a varrer `scripts/`, e este arquivo
// é um arquivo de `scripts/`: os padrões abaixo são escritos em `\uXXXX` de propósito, para
// que o detector não se acuse ao ser varrido por si mesmo. Prosa acentuada normal não o
// aciona — o que ele procura são PARES de bytes que só nascem de decodificação errada.
//
// O sintoma: uma cedilha vira dois caracteres, um travessão vira três, um emoji vira quatro.
// Todos começam pelo primeiro byte da sequência UTF-8 promovido a caractere Latin-1: 0xC3 e
// 0xC2 nos acentos, 0xE2 na pontuação tipográfica, 0xF0 0x9F no emoji.

/// Cada padrão é um byte-líder de UTF-8 seguido de um byte de CONTINUAÇÃO. O último é o BOM
/// de UTF-8 lido como Latin-1.
///
/// A faixa de continuação é 0x80-0xBF, e só ela. A versão anterior aceitava também
/// 0xC0-0xFF depois do líder, e essa metade só rendia falso positivo: byte nessa faixa nunca
/// é continuação de coisa nenhuma, então a sequência não pode ter vindo de UTF-8 mal
/// decodificado. O que ela pegava, na prática, era português CERTO — classe de maiúscula
/// acentuada com A-til encostado em O-til, que aparece em toda regex de nome próprio do
/// repositório. Estreitar a faixa não perde nenhum caso real e dispensa a isenção nesses
/// arquivos.
export const PADROES_SUSPEITOS = [
  /\u00c3[\u0080-\u00bf]/u,
  /\u00c2[\u0080-\u00bf]/u,
  /\u00e2[\u0080-\u00bf]/u,
  /\u00e2[\u2013\u2014\u2018-\u201e\u2020-\u2022\u2030\u2039\u203a\u0152\u0153\u0160\u0161\u0178\u017d\u017e\u02c6\u02dc\u2122]/u,
  /\u00f0\u0178[\u0080-\u00bf]/u,
  /\u00ef\u00bb\u00bf/u,
];

/// A isenção é POR LINHA e pede o motivo escrito ao lado. Não há allowlist de arquivo:
/// isentar um arquivo inteiro é o mesmo que tirá-lo da varredura, e o passivo de encoding
/// entra justamente pela edição seguinte, num trecho que ninguém isentou de propósito.
///
/// O caso real que a abriu: uma classe de caracteres de português maiúsculo acentuado, com
/// A-til colado em O-til, tem exatamente a assinatura do 0xC3 seguido de byte alto. O texto
/// está CERTO; o que engana é a vizinhança.
export const MARCADOR_ISENCAO = "encoding-ok";

/** A linha tem mojibake, e não pediu isenção? */
export function linhaSuspeita(linha) {
  if (linha.includes(MARCADOR_ISENCAO)) return false;
  return PADROES_SUSPEITOS.some((padrao) => padrao.test(linha));
}

/** Devolve `{ linha, texto }` de cada ocorrência. Linha é 1-based, como o editor mostra. */
export function acharMojibake(fonte) {
  const achados = [];
  fonte.split(/\r?\n/u).forEach((linha, i) => {
    if (linhaSuspeita(linha)) achados.push({ linha: i + 1, texto: linha });
  });
  return achados;
}
