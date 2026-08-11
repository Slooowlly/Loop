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
//     advance_transfer_window (F-01), iracing_career_race_result (§6 do iracing-escopo),
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
//   • DESFAZER SEM BOTÃO AINDA: iracing_desfazer_pinturas, iracing_modo_janela_status,
//     iracing_modo_janela_restaurar. O Loop escreve em DOIS arquivos que não são dele — a
//     pintura do carro (`paint/<carro>/car_<custid>.tga`) e a configuração gráfica
//     (`rendererDX11*.ini`) — e faz as duas coisas sem perguntar. Não perguntar só se
//     sustenta com caminho de volta, e o backend dele existe desde 11/08/2026; o que falta é
//     a seção nas Configurações que o chama, que é de outra frente. Enquanto ela não vem, o
//     jogador só tem o interruptor `auto_paint_car`, que impede as próximas e não devolve as
//     que já foram escritas.
//
//   • CONSUMIDOR REMOVIDO ACIDENTALMENTE: overlay_window_set_interactive. O doc-comment em
//     `commands/overlay_window.rs` afirma "Chamado pelo botão 'Mover' do app" — esse botão
//     não existe mais em lugar nenhum de `src/`. O `OverlayPositionPanel.jsx` reposiciona o
//     overlay pelos comandos de POSE, e nunca alterna o click-through. Consequência real: o
//     overlay fica preso em click-through, e este comando é a única saída. Não foi removido
//     porque a correção é religar a UI, não apagar a alavanca.
const SEM_CONSUMIDOR_CONHECIDO = [
  "advance_transfer_window",
  "engenheiro_catalogo",
  "engenheiro_classificar",
  "engenheiro_dossie_completo",
  "get_race_reading",
  "get_race_results_by_category",
  "iracing_career_race_result",
  "iracing_desfazer_pinturas",
  "iracing_estado_agora",
  "iracing_log_caminho",
  "iracing_modo_janela_restaurar",
  "iracing_modo_janela_status",
  "iracing_poll_race",
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

test("o inventário de comandos sem consumidor no frontend não muda sozinho", () => {
  const registrados = comandosRegistrados();
  // O invoke com nome montado em runtime existe num lugar só, e é declarado: o
  // OverlayPositionPanel guarda os comandos de pose numa tabela por alvo e chama
  // `invoke(cfg.setPose)`. Sem contar essas strings, os seis comandos de VR entrariam na
  // lista de órfãos e o guard passaria a mentir.
  const usados = new Set();
  for (const rel of fontesDoFront()) {
    const texto = ler(rel);
    for (const m of texto.matchAll(/\binvoke\w*\(\s*["']([a-z0-9_]+)["']/g)) usados.add(m[1]);
    for (const m of texto.matchAll(/["']([a-z0-9_]{6,})["']/g)) {
      if (registrados.has(m[1])) usados.add(m[1]);
    }
  }

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
