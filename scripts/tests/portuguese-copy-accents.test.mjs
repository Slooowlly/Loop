// A copy em português mantém os acentos.
//
// Este guard nasceu como uma LISTA DE 11 ARQUIVOS com fragmentos proibidos escritos na mão
// ("Proxima corrida", "Voce entra", "Nao foi possivel"). Ele tinha três defeitos que a vistoria
// de 10/08/2026 apontou e que a versão abaixo corrige:
//
//   1. tela nova nascia FORA do guard — só protegia o que alguém lembrasse de listar;
//   2. mover qualquer um dos 11 arquivos fazia o `readFile` estourar, então o guard quebrava
//      por refactor em vez de por regressão de copy;
//   3. e, principalmente, ele olhava para o lugar ERRADO. Desde a migração para o i18next a
//      copy do jogador não mora mais no `.jsx`: mora em `src/i18n/locales/pt-BR/common.json`.
//      Os fragmentos listados já tinham quase todos saído dos arquivos vigiados e migrado para
//      o JSON, onde ninguém olhava — foi assim que "Indice", "Vitorias", "Podios", "Lesoes",
//      "Campeoes", "Titulos", "Classificacao" e mais 22 strings passaram meses sem acento na
//      tela, com o guard verde o tempo todo.
//
// A versão nova varre o locale inteiro procurando palavras que em português praticamente só
// existem acentuadas. É o mesmo movimento do `text-encoding-sanity`, que já varre diretório
// em vez de listar arquivo.
//
// O `en-US` fica de fora de propósito: lá "nao" e "voce" não existem, e palavras inglesas como
// "media" e "series" dariam falso positivo em série.

import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");

// Palavras cuja forma sem acento é sempre erro de digitação em português. Mantida conservadora
// de propósito: falso-negativo (deixa passar) custa menos que ruído, e a lista cresce quando
// alguém encontrar um vão. Fora daqui ficam os homógrafos reais — "meses", "series", "media",
// "area", "categoria" — que existem sem acento e produziriam falso positivo em interpolação.
const PALAVRAS_SEM_ACENTO = [
  "nao", "voce", "sao", "acao", "acoes", "proximo", "proxima", "proximos", "proximas",
  "amanha", "historico", "historicos", "indice", "indices", "vitoria", "vitorias",
  "podio", "podios", "lesao", "lesoes", "posicao", "posicoes", "classificacao",
  "competicao", "competicoes", "convocacao", "convocacoes", "promocao", "promocoes",
  "licenca", "licencas", "campeao", "campeoes", "titulo", "titulos", "salario", "salarios",
  "calendario", "noticia", "noticias", "disponivel", "indisponivel", "consistencia",
  "experiencia", "referencia", "tendencia", "sequencia", "ultimo", "ultima", "ultimos",
  "ultimas", "unico", "unica", "possivel", "impossivel", "valido", "valida", "invalido",
  "invalida", "minimo", "maximo", "numero", "numeros", "criterio", "criterios",
  "estrategia", "estrategias", "veiculo", "trafego", "distancia", "ausencia", "presenca",
  "duvida", "orcamento", "manutencao", "informacao", "informacoes", "configuracao",
  "configuracoes", "simulacao", "simulacoes", "avaliacao", "avaliacoes", "descricao",
  "condicao", "condicoes", "situacao", "situacoes", "rescisao", "renovacao", "reducao",
  "producao", "sessao", "sessoes", "decisao", "decisoes", "pressao", "regiao", "regioes",
  "repertorio", "solido", "solida", "pelotao", "memoria", "pratica",
];

/// Uma palavra da lista aparecendo INTEIRA no texto. A fronteira exclui as letras acentuadas
/// para que "nao" não case dentro de "não" nem "titulo" dentro de "título".
function semAcento(texto) {
  const baixo = texto.toLowerCase();
  return PALAVRAS_SEM_ACENTO.filter((p) =>
    new RegExp(`(^|[^a-zà-ÿ])${p}([^a-zà-ÿ]|$)`).test(baixo),
  );
}

test("a copy pt-BR do i18next mantém os acentos", () => {
  const rel = "src/i18n/locales/pt-BR/common.json";
  const linhas = readFileSync(path.join(raiz, rel), "utf8").split("\n");
  const achados = [];
  linhas.forEach((linha, i) => {
    // Só o VALOR de um par `"chave": "texto"`. A chave é identificador em inglês e nunca
    // chega à tela; cobrá-la geraria ruído puro.
    const m = /"[^"]*"\s*:\s*"(.*)"\s*,?\s*$/.exec(linha);
    if (!m) return;
    // A interpolação é dado, não copy: `{{series}}` e `{{scope}}` não se acentuam.
    const valor = m[1].replace(/\{\{[^}]*\}\}/g, " ");
    const palavras = semAcento(valor);
    if (palavras.length) achados.push(`${rel}:${i + 1}  "${valor.slice(0, 90)}"  → ${palavras}`);
  });
  assert.deepEqual(
    achados,
    [],
    `copy pt-BR sem acento (o jogador lê exatamente isto):\n${achados.join("\n")}`,
  );
});

test("a varredura do locale realmente enxerga a copy", () => {
  // Um guard montado por regex sobre JSON tem um modo de falha silencioso: mudar o formato do
  // arquivo (uma linha por par) faz a extração casar zero e o teste acima fica verde sem ter
  // olhado nada. Este caso é o detector de fumaça dele.
  const linhas = readFileSync(path.join(raiz, "src/i18n/locales/pt-BR/common.json"), "utf8").split("\n");
  const valores = linhas.filter((l) => /"[^"]*"\s*:\s*"(.*)"\s*,?\s*$/.test(l));
  assert.ok(
    valores.length >= 3000,
    `só ${valores.length} valores de copy extraídos do locale — a extração furou e o guard acima ` +
      `estaria passando sem olhar nada.`,
  );
});

// A copy que ainda mora em `.jsx`/`.js` continua coberta por outra via: o `i18nAudit.mjs`
// (pre-commit + `src/i18n/i18nCoverage.test.js`) cobra que ela SAIA do arquivo e entre no
// locale, e uma vez no locale ela cai no primeiro teste deste arquivo. Cobrir os dois lugares
// aqui duplicaria o auditor sem ganho.
test("a copy de UI em português vive no locale, não espalhada no código", () => {
  // Guard de ligação: se alguém desligar o auditor de i18n, o teste acima passa a proteger um
  // arquivo que ninguém mais alimenta. Este caso garante que a outra ponta continua de pé.
  const auditor = readFileSync(path.join(raiz, "scripts/i18nAudit.mjs"), "utf8");
  assert.match(auditor, /export function runAudit/, "runAudit sumiu de scripts/i18nAudit.mjs");
  const hook = readFileSync(path.join(raiz, ".githooks/pre-commit"), "utf8");
  assert.match(hook, /i18nAudit\.mjs/, "o pre-commit parou de rodar o auditor de i18n");
});
