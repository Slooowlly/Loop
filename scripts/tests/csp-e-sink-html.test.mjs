// A CSP do app e o último sink de HTML, lidos como texto.
//
// O `tauri.conf.json` é JSON estrito (o Tauri recusa chave desconhecida), então não
// cabe comentário lá dentro. A derivação da política mora aqui, junto do guard que
// a segura:
//
//   default-src 'self'      tudo que a janela carrega é asset embutido, servido pelo
//                           protocolo do Tauri (http://tauri.localhost no Windows).
//   script-src 'self'       o build do Vite não deixa script inline no index.html
//                           (conferido no dist/): só a tag com src do bundle.
//   style-src + unsafe-inline  128 arquivos usam `style={{...}}` do React, e atributo
//                           style cai em style-src. Sem isto a tela vira texto solto.
//   img-src data:           NewsMagazineTab.css tem um SVG em data: no background.
//   img-src images.unsplash.com  ver PENDENTE, no fim deste arquivo.
//   media-src blob:         o painel de PTT toca o áudio gravado por URL de Blob.
//   font-src 'self'         a Space Grotesk vem do @fontsource, empacotada no bundle.
//   connect-src ipc:/http://ipc.localhost  o `invoke` do Tauri v2 é um fetch para o
//                           protocolo de IPC. Sem esta fonte, TODO comando morre e o
//                           app abre numa tela vazia. `ipc:` cobre macOS/Linux,
//                           `http://ipc.localhost` cobre o Windows, que é o alvo.
//                           O fetch das peças de voz (.opus) é same-origin: 'self'.
//   object-src/frame-src/worker-src 'none'  o app não usa nenhum dos três.
//
// O que NÃO entrou, de propósito: o endpoint do updater e o proxy de notícias por IA
// são HTTP do lado Rust (reqwest), não da webview, e CSP não os alcança.
//
// Em `npm run tauri dev` a política não é aplicada: o HTML vem do Vite, por URL
// externa, e o Tauri só injeta o cabeçalho no que ele mesmo serve pelo protocolo
// (tauri-2.10.3, `manager/mod.rs::get_asset`). Por isso não existe `devCsp` aqui — e
// o preâmbulo inline do react-refresh continua funcionando no dev.

import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const ler = (p) => fs.readFileSync(path.join(raiz, p), "utf8");

const conf = JSON.parse(ler("src-tauri/tauri.conf.json"));
const csp = conf.app?.security?.csp;

// O código do frontend que vai para o build. Teste fica de fora: ele cita o nome do
// sink e o host remoto de propósito, para provar que estão trancados.
function fontesDoFrontend(dir = path.join(raiz, "src"), acc = []) {
  for (const entrada of fs.readdirSync(dir, { withFileTypes: true })) {
    const alvo = path.join(dir, entrada.name);
    if (entrada.isDirectory()) fontesDoFrontend(alvo, acc);
    else if (/\.(js|jsx|css)$/.test(entrada.name) && !/\.(test|spec)\.(js|jsx)$/.test(entrada.name)) {
      acc.push(path.relative(raiz, alvo).split(path.sep).join("/"));
    }
  }
  return acc;
}

const fontes = fontesDoFrontend();

test("a CSP existe e é uma política, não um buraco", () => {
  assert.equal(typeof csp, "string", "app.security.csp voltou a ser null");
  assert.ok(!csp.includes("*"), "curinga na CSP anula a política inteira");
  assert.ok(!csp.includes("unsafe-eval"), "unsafe-eval devolve o eval() ao atacante");
  const scriptSrc = /script-src ([^;]+)/.exec(csp);
  assert.ok(scriptSrc, "sem script-src explícito");
  assert.ok(
    !scriptSrc[1].includes("unsafe-inline"),
    "script-src com unsafe-inline é a mesma porta que a CSP deveria fechar",
  );
  for (const diretiva of ["default-src", "object-src", "base-uri", "frame-ancestors"]) {
    assert.ok(csp.includes(`${diretiva} `), `${diretiva} sumiu da CSP`);
  }
});

test("connect-src carrega o IPC do Tauri — sem isso o app abre morto", () => {
  const connect = /connect-src ([^;]+)/.exec(csp);
  assert.ok(connect, "sem connect-src explícito");
  assert.ok(connect[1].includes("http://ipc.localhost"), "o invoke do Windows precisa de http://ipc.localhost");
  assert.ok(connect[1].includes("ipc:"), "o invoke fora do Windows precisa do esquema ipc:");
  assert.ok(connect[1].includes("'self'"), "o fetch das peças de voz é same-origin");
});

test("nenhum host remoto na CSP sem consumidor no código, e vice-versa", () => {
  const naCsp = new Set((csp.match(/https?:\/\/[a-z0-9.-]+/g) ?? []).filter((h) => !h.includes("ipc.localhost")));

  // `http://www.w3.org/2000/svg` é namespace de XML, não busca de recurso.
  const noCodigo = new Set();
  for (const arquivo of fontes) {
    for (const url of ler(arquivo).match(/https?:\/\/[a-z0-9.-]+/g) ?? []) {
      if (url.includes("w3.org") || url.includes("localhost")) continue;
      noCodigo.add(url);
    }
  }

  assert.deepEqual(
    [...naCsp].sort(),
    [...noCodigo].sort(),
    "a lista de hosts remotos da CSP e a do código divergiram: um recurso novo entrou sem liberação, ou uma liberação ficou órfã",
  );
});

test("nenhum sink de HTML no frontend", () => {
  const culpados = fontes.filter(
    (f) => ler(f).includes("dangerouslySetInnerHTML") || /\.innerHTML\s*=/.test(ler(f)),
  );
  assert.deepEqual(
    culpados,
    [],
    "HTML montado em string volta a expor nome de piloto/equipe do banco como marcação",
  );
});

// PENDENTE — decisão de arquitetura, não de segurança.
//
// `src/pages/tabs/NextRaceTab.jsx` usa uma foto do Unsplash como fundo decorativo da
// aba, por URL remota. É o ÚNICO recurso remoto que a webview busca, e por isso
// `https://images.unsplash.com` está fixado no img-src — host exato, sem curinga.
//
// O certo é trazer a imagem para `src/assets/` e derrubar o host da CSP: o Loop é um
// jogo offline, e hoje essa aba faz uma requisição para fora a cada abertura e
// aparece sem fundo em máquina sem internet. É troca de asset, e fica para quem
// decidir o peso do bundle. Enquanto ela não acontece, o item segue parcial e o
// teste acima trava as duas pontas juntas.
