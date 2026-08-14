// O socorro financeiro entra na rodada DEPOIS do caixa e ANTES do estado.
//
// A ordem das três chamadas em `commands/race/persistencia.rs` é a regra inteira do
// mecanismo, e ela vivia só na leitura do arquivo:
//
//   apply_round_cashflow          — receita e despesa da rodada entram no caixa
//   apply_crisis_event_if_needed  — os quatro portões são avaliados sobre o caixa JÁ movido
//   refresh_team_financial_state  — o estado é recalculado depois de tudo
//
// Cada posição responde por uma coisa. O socorro depois do caixa é o que faz dele uma REAÇÃO
// ao fim de semana: equipe que a rodada salvou não é socorrida, e é assim que o gate de
// necessidade significa alguma coisa. O estado por último é o que impede a equipe de sair da
// rodada rotulada com a foto anterior ao próprio socorro.
//
// Por que este guard existe, e por que ele é o dono da regra desde 14/08/2026: quem provava a
// ligação era `commands::race::tests::test_simulate_race_weekend_applies_crisis_finance_event`,
// asseverando que o socorro TINHA acontecido depois de um fim de semana completo. Aquele teste
// monta um mundo SORTEADO, e o socorro só sai numa janela fechada dos dois lados — acima de
// −2 meses de operação o gate de necessidade fecha, abaixo de −100 mil o cheque especial de
// `finance/cashflow.rs` converte o rombo em dívida e estoura o teto de 4 meses. Rodada boa e
// rodada péssima fecham o portão por motivos opostos, então a asserção caía no CI em uma das
// duas bordas conforme o sorteio. Ela passou a asseverar o contrato dos portões, que vale para
// todo sorteio, e a ORDEM veio para cá, onde é lida como texto e não depende de simulação.
//
// O número exato de um socorro fica em `finance::events::tests::
// um_socorro_injeta_caixa_cria_divida_e_registra`, que é determinístico.
//
// O que o guard NÃO faz, de propósito:
//
//   • não conta chamadas nem linhas — número envelhece no primeiro caminho novo;
//   • não exige que as três estejam coladas. Entre elas pode entrar o que for, desde que a
//     ORDEM relativa se mantenha;
//   • não olha o valor de nenhuma constante de calibração. Gate, teto e principal são
//     decisão de economia e mudam sem avisar este arquivo.

import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const PERSISTENCIA = "src-tauri/src/commands/race/persistencia.rs";

/// As três chamadas, na ordem em que precisam aparecer.
const SEQUENCIA = [
  "apply_round_cashflow",
  "apply_crisis_event_if_needed",
  "refresh_team_financial_state",
];

function fonte() {
  const arquivo = path.join(raiz, PERSISTENCIA);
  assert.ok(
    fs.existsSync(arquivo),
    `${PERSISTENCIA} não existe. Se a persistência da rodada mudou de casa, este guard tem ` +
      `de apontar para o arquivo novo, e não ser apagado.`,
  );
  return fs.readFileSync(arquivo, "utf8");
}

/// Índice da chamada da função (`nome(`), ignorando `use`, comentário e doc.
function indiceDaChamada(texto, nome) {
  const linhas = texto.split("\n");
  let offset = 0;
  for (const linha of linhas) {
    const limpa = linha.trim();
    const eRuido =
      limpa.startsWith("//") || limpa.startsWith("///") || limpa.startsWith("use ");
    if (!eRuido && new RegExp(`\\b${nome}\\s*\\(`).test(linha)) return offset;
    offset += linha.length + 1;
  }
  return -1;
}

test("as três chamadas da rodada existem em persistencia.rs", () => {
  const texto = fonte();
  for (const nome of SEQUENCIA) {
    assert.notEqual(
      indiceDaChamada(texto, nome),
      -1,
      `${nome} sumiu de ${PERSISTENCIA}. As três formam o fecho financeiro da rodada: sem o ` +
        `socorro no meio, equipe em colapso nunca é socorrida em corrida de verdade, e nenhum ` +
        `teste de unidade acusa, porque lá o socorro é chamado à mão.`,
    );
  }
});

test("o socorro entra depois do caixa da rodada e antes do estado", () => {
  const texto = fonte();
  const [caixa, socorro, estado] = SEQUENCIA.map((n) => indiceDaChamada(texto, n));

  assert.ok(
    caixa < socorro,
    `apply_crisis_event_if_needed aparece ANTES de apply_round_cashflow em ${PERSISTENCIA}. ` +
      `Com o socorro avaliado antes da receita, o gate de necessidade julga o caixa da rodada ` +
      `anterior: equipe que o fim de semana salvou seria socorrida assim mesmo, e o socorro ` +
      `deixa de ser reação ao resultado.`,
  );

  assert.ok(
    socorro < estado,
    `refresh_team_financial_state aparece ANTES de apply_crisis_event_if_needed em ` +
      `${PERSISTENCIA}. Com o estado recalculado antes do socorro, a equipe sai da rodada ` +
      `rotulada pela foto anterior ao próprio aporte, e o rótulo é o que abre o portão na ` +
      `rodada seguinte.`,
  );
});
