#!/usr/bin/env node
// Roda a suíte de guards estruturais (`scripts/tests/*.test.mjs`) com a lista de arquivos
// EXPLÍCITA, e falha quando ela encolhe.
//
// ## Por que não `node --test "scripts/tests/*.test.mjs"`
//
// Era essa a forma do `test:structure`, e ela tem dois donos possíveis para o asterisco. No
// bash o shell expande antes do Node ver; no `npm run` do Windows quem executa é o `cmd.exe`,
// que NÃO expande, então o Node recebe o glob literal e precisa resolvê-lo sozinho. O
// resolvedor de glob do runner do Node só existe a partir do v21 — no v20, que é o que o CI
// usava, o argumento é um caminho que não existe.
//
// E o modo de falha é o pior de todos: medido aqui, um glob que não casa com nada devolve
// `tests 0 · pass 0 · fail 0` e sai com código ZERO. Verde, sem ter rodado nada. Uma suíte
// inteira de guards podia sumir do CI sem que nenhuma linha ficasse vermelha.
//
// Aqui a descoberta é um `readdir` — nenhum shell, nenhuma versão de Node no meio — e a lista
// vai para o `node --test` arquivo por arquivo. O que o runner roda é exatamente o que este
// script contou.
//
// ## O piso
//
// Contar não basta: 51 guards e 3 guards produzem a mesma saída verde. `PISO` é a contagem do
// dia em que a trava entrou, e o script morre se a descoberta trouxer menos que isso. Guard
// novo entra sem mexer em nada; guard APAGADO exige baixar o piso na mão, no mesmo commit e
// com o motivo no assunto. É a mesma ideia do inventário congelado do
// `invoke-contra-generate-handler`.
//
// Uso:
//   node scripts/rodar-guards.mjs                      # todos
//   node scripts/rodar-guards.mjs window-controls      # só os que casam com o filtro
import { readdirSync } from "node:fs";
import path from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const RAIZ = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
export const DIR_GUARDS = path.join("scripts", "tests");

/// Quantos guards existem no disco. Nasceu em 51, com a descoberta que deixou de depender
/// de glob, e subiu para 52 em 12/08/2026, quando o `guards-descobertos` entrou e o piso
/// ficou um atrás da árvore. Em 14/08/2026 foi para 54 com o
/// `socorro-na-ordem-do-fim-de-semana`, que recebeu do teste de fim de semana completo a
/// regra de ordem das três chamadas da rodada.
///
/// É número escrito à mão de propósito. Derivar a contagem do próprio `readdir` daria
/// sempre igual e não travaria nada; derivar do git faria o guard morrer sem repositório
/// e não pegaria o caso que importa, que é o guard apagado e comitado junto. Subir é
/// mecânico e o teste `o piso pega a suíte encolhendo` avisa quando ficou defasado;
/// ABAIXAR é decisão consciente de apagar guard, nunca efeito colateral.
export const PISO = 55;

/** Os arquivos de guard, em ordem estável. Caminhos relativos à raiz do repo. */
export function listarGuards(raiz = RAIZ) {
  return readdirSync(path.join(raiz, DIR_GUARDS))
    .filter((nome) => nome.endsWith(".test.mjs"))
    .sort()
    .map((nome) => path.join(DIR_GUARDS, nome));
}

const chamadoDireto =
  process.argv[1] &&
  import.meta.url === new URL(`file://${process.argv[1].replace(/\\/g, "/")}`).href;

if (chamadoDireto) {
  const todos = listarGuards();

  if (todos.length < PISO) {
    console.error(
      `\n  A descoberta achou ${todos.length} guards em ${DIR_GUARDS}/, e o piso é ${PISO}.\n` +
        `  Ou a varredura quebrou, ou ${PISO - todos.length} guard(s) foram apagados.\n` +
        `  Apagar guard é decisão consciente: baixe o PISO em scripts/rodar-guards.mjs no mesmo\n` +
        `  commit, com o motivo escrito.\n`,
    );
    process.exit(1);
  }

  // O filtro é para rodar um subconjunto à mão. Ele NÃO passa pelo piso de propósito: a trava
  // existe para a rodada completa, e falhar num recorte pedido de propósito ensinaria a
  // ignorar a saída de erro deste script.
  const filtro = process.argv[2];
  const alvos = filtro ? todos.filter((f) => f.includes(filtro)) : todos;

  if (!alvos.length) {
    console.error(`\n  Nenhum guard casa com "${filtro}".\n`);
    process.exit(1);
  }

  console.log(
    `\n${alvos.length} guard(s)${filtro ? ` (filtro "${filtro}", de ${todos.length})` : ` · piso ${PISO}`}\n`,
  );

  // Aviso, nunca falha: guard novo tem de entrar sem mexer em nada. O que isto evita é o
  // piso ficar dez atrás da árvore e passar a não travar mais nada.
  if (!filtro && todos.length > PISO) {
    console.log(
      `  Nota: ${todos.length} guards no disco contra piso ${PISO}. Subir o PISO fecha a folga.\n`,
    );
  }

  // Sem `shell: true`: os caminhos vão como argumentos de verdade, então nome com espaço ou
  // com acento não depende de aspas de shell nenhum.
  const r = spawnSync(process.execPath, ["--test", ...alvos], {
    cwd: RAIZ,
    stdio: "inherit",
  });
  if (r.error) {
    console.error(`\n  Não consegui rodar o node --test: ${r.error.message}\n`);
    process.exit(1);
  }
  process.exit(r.status ?? 1);
}
