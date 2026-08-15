// Todo `invoke("nome")` do frontend existe no `generate_handler!` do Rust, e todo comando
// registrado tem consumidor conhecido.
//
// Este é o contrato mais fino da ponte Rust↔React e o único sem nenhuma verificação: o nome do
// comando é uma STRING dos dois lados. Renomear um comando no Rust, ou errar uma letra no JS,
// compila nos dois, passa no `cargo test`, passa no `vitest` (que mocka `@tauri-apps/api`) e
// só falha no app rodando — com um `Result::Err` genérico que a tela costuma engolir num
// `.catch(() => {})`. É o modo de falha que a vistoria de 10/08/2026 apontou como o buraco
// mais barato de fechar da área de testes.
//
// As duas direções não têm o mesmo peso, e por isso são tratadas diferente:
//
//   • JS → Rust é ERRO. Um invoke que não existe no handler é sempre defeito.
//   • Rust → JS é INVENTÁRIO. Comando registrado sem consumidor pode ser trabalho pronto
//     esperando a tela, pode ser API interna que só o Rust chama, e pode ser código morto de
//     verdade. O guard não decide isso — ele congela a lista e quebra quando ela MUDA, para
//     que a decisão seja tomada por alguém em vez de a lista crescer sozinha.
//
// O que conta como CONSUMIDOR foi apertado em 11/08/2026 (achado V8.2 da vistoria). Antes
// bastava o nome do comando aparecer entre aspas em qualquer `.js`/`.jsx` de `src/`, e isso
// dava três tipos de verde falso:
//
//   • citação em TESTE — `create_career` só existe em `NewCareer.test.jsx`, e numa asserção
//     NEGATIVA (`expect(invoke).not.toHaveBeenCalledWith("create_career")`): o teste prova
//     que ninguém chama o comando, e o guard lia isso como prova de que alguém chama;
//   • citação em COMENTÁRIO — prosa não invoca nada;
//   • citação em MÓDULO MORTO — `RosterGenPanel.jsx` chama seis comandos e não é importado
//     por ninguém a não ser o próprio teste; a tela nunca é montada.
//
// A correção é medir alcance de verdade: monta-se o grafo de imports a partir dos pontos de
// entrada REAIS (os `<script type="module" src="/src/…">` dos HTML da raiz), e só arquivo
// vivo e não-teste pode declarar consumo, com comentário e regex descartados por um
// varredor de literais.

import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const ler = (p) => fs.readFileSync(path.join(raiz, p), "utf8");

/// Os nomes registrados no `generate_handler!`, sem o caminho de módulo.
function comandosRegistrados() {
  const lib = ler("src-tauri/src/lib.rs");
  const bloco = /invoke_handler\(tauri::generate_handler!\[([\s\S]*?)\n\s*\]\)/.exec(lib);
  assert.ok(bloco, "não achei o generate_handler! em src-tauri/src/lib.rs");
  const nomes = [...bloco[1].matchAll(/^\s*(?:[a-z0-9_]+::)*([a-z0-9_]+),\s*$/gm)].map(([, n]) => n);
  // Um guard que não acha o que procura precisa gritar em vez de passar vazio.
  assert.ok(nomes.length >= 150, `só ${nomes.length} comandos extraídos — a extração furou`);
  return new Set(nomes);
}

/// Todo arquivo `.js`/`.jsx` de `src/`, incluindo teste (um mock com nome errado também mente).
function fontesDoFront() {
  const achados = [];
  const varrer = (dir) => {
    for (const item of fs.readdirSync(path.join(raiz, dir), { withFileTypes: true })) {
      const rel = `${dir}/${item.name}`;
      if (item.isDirectory()) varrer(rel);
      else if (/\.(js|jsx)$/.test(item.name)) achados.push(rel);
    }
  };
  varrer("src");
  return achados;
}

const eArquivo = (rel) => {
  const abs = path.join(raiz, rel);
  return fs.existsSync(abs) && fs.statSync(abs).isFile();
};

const eTeste = (rel) => /\.(test|spec)\.[jt]sx?$/.test(rel) || rel.startsWith("src/test/");

/// Os pontos de entrada de verdade: os HTML da raiz que o Vite serve. `index.html` é o app;
/// `preview-curva-campeonato.html` é a casca de preview que também tem entrada própria.
/// Descobrir em vez de listar é de propósito — um preview novo entra sozinho.
function pontosDeEntrada() {
  const achados = [];
  for (const nome of fs.readdirSync(raiz)) {
    if (!nome.endsWith(".html")) continue;
    for (const m of ler(nome).matchAll(/src="\/(src\/[^"]+)"/g)) {
      if (eArquivo(m[1])) achados.push(m[1]);
    }
  }
  assert.ok(achados.length >= 1, "nenhum ponto de entrada achado nos HTML da raiz");
  return achados;
}

/// Resolve um import relativo do jeito que o Vite resolve (extensão implícita e `index`).
/// Import de pacote (`react`, `@tauri-apps/…`) devolve `null` — não é módulo nosso.
function resolverImport(deQual, spec) {
  if (!spec.startsWith(".")) return null;
  const base = path.posix.join(path.posix.dirname(deQual), spec);
  const candidatos = [base, `${base}.js`, `${base}.jsx`, `${base}/index.js`, `${base}/index.jsx`];
  return candidatos.find((c) => /\.(js|jsx)$/.test(c) && eArquivo(c)) ?? null;
}

/// Os módulos que o app REALMENTE carrega: fecho transitivo dos imports a partir dos pontos
/// de entrada. Um `.jsx` fora deste conjunto não roda no app, por mais completo que pareça.
function modulosVivos() {
  const vivos = new Set();
  const fila = pontosDeEntrada();
  while (fila.length) {
    const atual = fila.pop();
    if (vivos.has(atual)) continue;
    vivos.add(atual);
    for (const m of literaisDeImport(ler(atual))) {
      const alvo = resolverImport(atual, m);
      if (alvo) fila.push(alvo);
    }
  }
  assert.ok(vivos.size >= 150, `só ${vivos.size} módulos vivos — o grafo de imports furou`);
  return vivos;
}

/// Os alvos de `import … from "x"`, `export … from "x"` e `import("x")`.
function literaisDeImport(texto) {
  return [...texto.matchAll(/(?:\bfrom|\bimport)\s*\(?\s*["']([^"']+)["']/g)].map(([, s]) => s);
}

/// Os literais de string do código, com comentário (`//` e `/* */`) e corpo de regex fora.
///
/// Um `String.split` por aspas não serve aqui: é justamente a citação em comentário que
/// precisa não contar. O varredor é bobo de propósito — só o suficiente para saber quando
/// está dentro de string, de comentário ou de regex.
function literaisDeString(fonte) {
  const achados = [];
  let i = 0;
  let anterior = "\n"; // último caractere significativo, para distinguir divisão de regex
  while (i < fonte.length) {
    const c = fonte[i];
    if (c === "/" && fonte[i + 1] === "/") {
      while (i < fonte.length && fonte[i] !== "\n") i++;
      continue;
    }
    if (c === "/" && fonte[i + 1] === "*") {
      const fim = fonte.indexOf("*/", i + 2);
      i = fim === -1 ? fonte.length : fim + 2;
      continue;
    }
    if (c === "/" && /[(,=:[!&|?{};+\n]/.test(anterior)) {
      let j = i + 1;
      let emClasse = false;
      let fechou = false;
      while (j < fonte.length && fonte[j] !== "\n") {
        if (fonte[j] === "\\") j += 2;
        else if (fonte[j] === "[") (emClasse = true), j++;
        else if (fonte[j] === "]") (emClasse = false), j++;
        else if (fonte[j] === "/" && !emClasse) {
          fechou = true;
          break;
        } else j++;
      }
      if (fechou) {
        i = j + 1;
        anterior = "/";
        continue;
      }
    }
    if (c === '"' || c === "'" || c === "`") {
      let j = i + 1;
      let texto = "";
      while (j < fonte.length && fonte[j] !== c) {
        if (fonte[j] === "\\") {
          texto += fonte[j + 1] ?? "";
          j += 2;
        } else texto += fonte[j++];
      }
      achados.push(texto);
      i = j + 1;
      anterior = c;
      continue;
    }
    if (!/\s/.test(c)) anterior = c;
    i++;
  }
  return achados;
}

/// Os nomes literais passados a `invoke(...)` no frontend, com arquivo e linha.
function invocacoes() {
  const achados = [];
  for (const rel of fontesDoFront()) {
    ler(rel)
      .split("\n")
      .forEach((linha, i) => {
        for (const m of linha.matchAll(/\binvoke\w*\(\s*["']([a-z0-9_]+)["']/g)) {
          achados.push({ nome: m[1], onde: `${rel}:${i + 1}` });
        }
      });
  }
  assert.ok(achados.length >= 100, `só ${achados.length} invokes extraídos — a extração furou`);
  return achados;
}

test("todo invoke do frontend existe no generate_handler do Rust", () => {
  const registrados = comandosRegistrados();
  const fantasmas = invocacoes()
    .filter(({ nome }) => !registrados.has(nome))
    .map(({ nome, onde }) => `${onde}  invoke("${nome}")`);
  assert.deepEqual(
    fantasmas,
    [],
    `invoke sem comando correspondente no Rust:\n${fantasmas.join("\n")}\n\n` +
      `Ou o comando foi renomeado no Rust, ou falta registrá-lo no generate_handler! de lib.rs.`,
  );
});

// Os comandos registrados que HOJE não têm nenhum consumidor no frontend. A lista é congelada
// de propósito: ela é o inventário que a vistoria pediu, e só muda com decisão consciente.
//
// Reconferido item a item em 11/08/2026 (segunda passada). O que mudou nessa passada:
//
//   • Saíram do generate_handler, junto com a implementação, três comandos comprovadamente
//     mortos: `toggle_maximize_window` e `get_window_maximized` (os controles de janela
//     trabalham com TELA CHEIA — `WindowControlsDrawer.jsx` só chama minimize, start_drag,
//     toggle_fullscreen, get_fullscreen e close) e `get_driver` (nasceu com a assinatura
//     antiga `career_number: u32`, e quem lê um piloto é `get_driver_detail`).
//   • Duas classificações da primeira passada estavam erradas e foram corrigidas abaixo:
//     `get_race_reading` e `ptt_gatilho_atual` NÃO são API interna.
//
// Por que cada um dos que ficaram está aqui:
//
//   • API INTERNA, chamada de dentro do próprio Rust — o registro é que sobra, não a lógica:
//     iracing_process_race_result (dificuldade adaptativa; roda em
//     `commands/iracing/importacao.rs:138`). É o único da lista nessa condição.
//
//   • FEATURE FUTURA — backend pronto, tela ainda não escrita. Documentar e manter; NÃO é
//     licença para implementar a tela:
//     iracing_career_race_result (§6 do iracing-escopo),
//     engenheiro_dossie_completo, get_race_results_by_category (F-03/F-04/F-05, a aba de
//     História), get_race_reading (a leitura da corrida da migração v55 —
//     `get_race_reading_in_base_dir` só é chamado pela própria casca `#[tauri::command]`,
//     então não há consumidor de nenhum lado), radio_log_caminho e radio_log_revelar (o
//     caminho e o "revelar na pasta" do log de rádio existem; o botão das Configurações não).
//
//   • DIAGNÓSTICO E BANCADA, sem tela por opção: iracing_read_session, iracing_read_telemetry,
//     iracing_poll_race, iracing_reset_race, iracing_log_caminho, iracing_estado_agora,
//     iracing_send_chat_macro, iracing_throw_yellow, iracing_spotter_restore,
//     engenheiro_catalogo, engenheiro_classificar.
//
//   • RESERVADO: ptt_gatilho_atual. Nada o chama, em Rust ou em JS — só os testes de
//     `commands/ptt.rs`. Fica porque é o lado de LEITURA de um estado do qual o Rust é dono
//     (o gatilho vive num estático em `ptt.rs`) e faz par com `ptt_set_gatilho`, que é
//     consumido. Sem ele não há como inspecionar esse estado de fora.
//
//
//   • CONSUMIDOR REMOVIDO ACIDENTALMENTE: overlay_window_set_interactive. O botão "Mover"
//     que o chamava não existe mais em lugar nenhum de `src/` (o doc-comment em
//     `commands/overlay_window.rs` já registra isso). O `OverlayPositionPanel.jsx` reposiciona o
//     overlay pelos comandos de POSE, e nunca alterna o click-through. Consequência real: o
//     overlay fica preso em click-through, e este comando é a única saída. Não foi removido
//     porque a correção é religar a UI, não apagar a alavanca.
//
//   • SAIU DA LISTA em 11/08/2026: advance_transfer_window. Era o item mais antigo daqui,
//     classificado como "feature futura, esperando o F-01". O F-01 foi feito — a tela do
//     mercado em temporada era a `MercadoSection.jsx` da aba Carreira — e quem a escreveu
//     recusou ligá-lo, porque o comando era um no-op: corpo idêntico ao de
//     `get_transfer_window_state` e o `accepted_seat_id` ignorado. Comando removido, não
//     religado.
//
//   • NÃO ENTROU NA LISTA em 14/08/2026, e vale registrar por que quase entrou:
//     get_season_market_board nasceu para o F-01 e teve um consumidor só, a
//     `MercadoSection.jsx` da aba Carreira, apagada naquele dia. Ele ficou órfão por meia
//     hora e voltou a ter tela no mesmo commit: o quadro de vagas mudou de casa para
//     `components/driver/v2/MercadoDoJogador.jsx`, montado no fim da aba Mercado da ficha
//     quando `detail.is_jogador`. Órfão temporário não vira entrada congelada.
//
//   • SAÍRAM DA LISTA em 11/08/2026 (A13.4): iracing_desfazer_pinturas,
//     iracing_modo_janela_status e iracing_modo_janela_restaurar. Eram o item "desfazer sem
//     botão ainda" — backend pronto e nenhuma tela chamando. A tela existe agora:
//     `src/components/iracing/IracingDesfazerPanel.jsx`, montada nas Configurações logo
//     abaixo do interruptor `auto_paint_car`.
//
//   • ENTRARAM NA LISTA em 11/08/2026, ao consertar o próprio guard (V8.2). Não são
//     comandos novos nem regressão: são os que a noção antiga de "consumidor" escondia, e
//     estão aqui para que a lista pare de mentir. A DECISÃO sobre cada um fica aberta — este
//     chat corrigiu o instrumento, não o inventário:
//       – create_career: nenhum consumo em lugar nenhum. A única citação é a asserção
//         NEGATIVA de `src/pages/NewCareer.test.jsx:135`, que existe para provar que o
//         assistente NÃO cria a carreira naquele ponto. Se a criação de carreira hoje passa
//         por outro comando, este é código morto; se não passa, é um caminho quebrado. Sem
//         decisão neste chat.
//       – iracing_apply_player_paint, iracing_dump_session_yaml, iracing_export_rain_test,
//         iracing_player_custid, iracing_player_paint e iracing_preview_race_result: os seis
//         são chamados por `src/components/iracing/RosterGenPanel.jsx`, que não é importado
//         por nenhum módulo vivo — só pelo próprio `RosterGenPanel.test.jsx`. O painel está
//         testado e não é montado. Religar a tela ou aposentá-la é decisão de produto.
const SEM_CONSUMIDOR_CONHECIDO = [
  "create_career",
  "engenheiro_catalogo",
  "engenheiro_classificar",
  "engenheiro_dossie_completo",
  "get_race_reading",
  "get_race_results_by_category",
  "iracing_apply_player_paint",
  "iracing_career_race_result",
  "iracing_dump_session_yaml",
  "iracing_estado_agora",
  "iracing_export_rain_test",
  "iracing_log_caminho",
  "iracing_player_custid",
  "iracing_player_paint",
  "iracing_poll_race",
  "iracing_preview_race_result",
  "iracing_process_race_result",
  "iracing_read_session",
  "iracing_read_telemetry",
  "iracing_reset_race",
  "iracing_send_chat_macro",
  "iracing_spotter_restore",
  "iracing_throw_yellow",
  "overlay_window_set_interactive",
  "ptt_gatilho_atual",
  "radio_log_caminho",
  "radio_log_revelar",
];

/// Os comandos com consumidor VIVO: citados como literal em módulo alcançável a partir de um
/// ponto de entrada e que não seja teste.
///
/// Continua bastando o literal, sem exigir que ele esteja dentro de um `invoke(...)`, porque
/// o invoke com nome montado em runtime existe e é declarado: o `OverlayPositionPanel` guarda
/// os comandos de pose numa tabela por alvo e chama `invoke(cfg.setPose)`. Exigir a sintaxe
/// jogaria os seis comandos de VR na lista de órfãos e o guard passaria a mentir para o outro
/// lado. O que mudou é ONDE o literal vale: só em código vivo de produção.
function comandosComConsumidorVivo(registrados) {
  const usados = new Set();
  const vivos = [...modulosVivos()].filter((rel) => !eTeste(rel));
  for (const rel of vivos) {
    for (const s of literaisDeString(ler(rel))) {
      if (registrados.has(s)) usados.add(s);
    }
  }
  assert.ok(usados.size >= 100, `só ${usados.size} comandos consumidos — a extração furou`);
  return usados;
}

test("o inventário de comandos sem consumidor no frontend não muda sozinho", () => {
  const registrados = comandosRegistrados();
  const usados = comandosComConsumidorVivo(registrados);

  const orfaos = [...registrados].filter((n) => !usados.has(n)).sort();
  const novos = orfaos.filter((n) => !SEM_CONSUMIDOR_CONHECIDO.includes(n));
  const resolvidos = SEM_CONSUMIDOR_CONHECIDO.filter((n) => !orfaos.includes(n));

  assert.deepEqual(
    novos,
    [],
    `comando(s) registrado(s) sem nenhum consumidor no frontend:\n  ${novos.join("\n  ")}\n\n` +
      `Ou falta ligar a tela, ou o comando não devia estar no generate_handler!. ` +
      `Se for decisão consciente, some o nome a SEM_CONSUMIDOR_CONHECIDO com o motivo.`,
  );
  assert.deepEqual(
    resolvidos,
    [],
    `estes já têm consumidor e podem sair de SEM_CONSUMIDOR_CONHECIDO:\n  ${resolvidos.join("\n  ")}`,
  );
});

// ── O guard vigiando a si mesmo ────────────────────────────────────────────────────────
//
// Os quatro testes abaixo existem porque o achado V8.2 não foi um comando errado na lista:
// foi o INSTRUMENTO medindo a coisa errada e dando verde. Um guard sem prova de que consegue
// falhar é um `assert(true)` caro.

test("literal citado em comentário ou dentro de regex não conta como consumo", () => {
  const fonte = [
    '// invoke("comando_do_comentario") — prosa não chama nada',
    '/* bloco com invoke("comando_do_bloco") dentro */',
    'const padrao = /invoke\\("comando_da_regex"\\)/;',
    'const url = "https://exemplo.test/nao-e-comentario";',
    'invoke("comando_de_verdade");',
  ].join("\n");
  const literais = literaisDeString(fonte);
  assert.ok(literais.includes("comando_de_verdade"), "o literal vivo tinha de ser visto");
  assert.ok(
    literais.includes("https://exemplo.test/nao-e-comentario"),
    "o `//` dentro de string não é começo de comentário",
  );
  for (const escondido of ["comando_do_comentario", "comando_do_bloco", "comando_da_regex"]) {
    assert.ok(!literais.includes(escondido), `'${escondido}' não devia contar como literal`);
  }
});

test("comando citado só em teste continua órfão", () => {
  const registrados = comandosRegistrados();
  const ondeAparece = fontesDoFront().filter((rel) =>
    literaisDeString(ler(rel)).includes("create_career"),
  );
  assert.ok(ondeAparece.length > 0, "o caso-teste sumiu: 'create_career' não é citado em src/");
  assert.deepEqual(
    ondeAparece.filter((rel) => !eTeste(rel)),
    [],
    `'create_career' passou a ser citado em produção (${ondeAparece.join(", ")}) — ` +
      `se a tela foi ligada, tire-o de SEM_CONSUMIDOR_CONHECIDO`,
  );
  assert.ok(
    !comandosComConsumidorVivo(registrados).has("create_career"),
    "citação em teste voltou a contar como consumidor — foi exatamente o furo do V8.2",
  );
});

test("comando chamado só por componente sem importador vivo continua órfão", () => {
  const painel = "src/components/iracing/RosterGenPanel.jsx";
  assert.ok(eArquivo(painel), `o caso-teste sumiu: ${painel} não existe mais`);
  assert.ok(
    literaisDeString(ler(painel)).includes("iracing_player_custid"),
    `o caso-teste mudou: ${painel} não chama mais 'iracing_player_custid'`,
  );
  assert.ok(
    !modulosVivos().has(painel),
    `${painel} passou a ser importado por módulo vivo — se a tela foi montada, ` +
      `tire os seis comandos dele de SEM_CONSUMIDOR_CONHECIDO`,
  );
  assert.ok(
    !comandosComConsumidorVivo(comandosRegistrados()).has("iracing_player_custid"),
    "módulo morto voltou a contar como consumidor",
  );
});

test("invoke em módulo vivo conta como consumidor", () => {
  const slice = "src/stores/career/careerSlice.js";
  const vivos = modulosVivos();
  assert.ok(vivos.has("src/main.jsx"), "o ponto de entrada não entrou no grafo");
  assert.ok(vivos.has(slice), `${slice} devia ser alcançável a partir do main.jsx`);
  const usados = comandosComConsumidorVivo(comandosRegistrados());
  assert.ok(usados.has("load_career"), "o invoke de um slice vivo tinha de contar");
  assert.ok(
    !SEM_CONSUMIDOR_CONHECIDO.includes("load_career"),
    "consumo vivo e lista de órfãos discordando",
  );
});
