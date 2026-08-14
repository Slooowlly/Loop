// O contrato de dimensões e pose dos quads de VR, lido como texto nos três lugares onde ele vive.
//
// Um frame de VR atravessa três camadas que não se conhecem e não compartilham nenhum símbolo:
//
//   1. o JS desenha num canvas de `VR_W`×`VR_H` e manda os bytes crus (src/overlay/towerCanvas.js
//      e src/overlay/radioCanvas.js);
//   2. o Rust cria o mapeamento de memória com `w`/`h` no cabeçalho e reserva
//      `header + w*h*4` bytes (src-tauri/src/commands/vr_overlay.rs);
//   3. a layer OpenXR em C++ lê o mesmo mapeamento com `IRACER_*_W/H` compilados dentro dela
//      (vr-overlay/src/shared_frame.h).
//
// Nada no build liga os três. O comentário no topo de towerCanvas.js já EXIGE a igualdade, e um
// comentário não falha. O modo de falha é o pior possível para diagnosticar: se o JS crescer o
// buffer, o `getImageData` devolve mais bytes do que o mapeamento comporta e a escrita é
// truncada; se encolher, a layer lê lixo do fim da página. Nos dois casos o resultado é uma
// imagem rasgada DENTRO do headset — a única superfície do app que ninguém consegue tirar
// screenshot com a ferramenta de sempre, e que só aparece com o iRacing rodando em VR.
//
// Este guard fecha o triângulo. Ele lê os três arquivos como texto porque é a única forma:
// o C++ não é compilado pelo CI do app e o JS não enxerga const do Rust.

import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const ler = (p) => fs.readFileSync(path.join(raiz, p), "utf8");

const towerJs = ler("src/overlay/towerCanvas.js");
const radioJs = ler("src/overlay/radioCanvas.js");
const vrRs = ler("src-tauri/src/commands/vr_overlay.rs");
const sharedH = ler("vr-overlay/src/shared_frame.h");
const layerCpp = ler("vr-overlay/src/overlay_layer.cpp");
const posicaoJs = ler("src/overlay/overlayPose.js");
const radioJsx = ler("src/overlay/EngineerRadio.jsx");
const conf = JSON.parse(ler("src-tauri/tauri.conf.json"));

/// Um `const NOME = <número>;` do JS. Falha alto quando o nome some, porque um guard que não
/// acha o que procura precisa gritar em vez de passar vazio.
function constJs(fonte, nome, arquivo) {
  const m = new RegExp(`(?:export )?const ${nome} = ([\\d.]+)`).exec(fonte);
  assert.ok(m, `${nome} sumiu de ${arquivo}`);
  return Number(m[1]);
}

/// Um `static const uint32_t NOME = <número>;` do C++.
function constCpp(nome) {
  const m = new RegExp(`static const uint32_t ${nome} = (\\d+)`).exec(sharedH);
  assert.ok(m, `${nome} sumiu de vr-overlay/src/shared_frame.h`);
  return Number(m[1]);
}

/// Um campo do literal `pub const <PAINEL>: Panel = Panel { ... }` do Rust.
function campoDoPainel(painel, campo) {
  const bloco = new RegExp(`pub const ${painel}: Panel = Panel \\{([\\s\\S]*?)\\n    \\};`).exec(vrRs);
  assert.ok(bloco, `pub const ${painel}: Panel sumiu de commands/vr_overlay.rs`);
  const m = new RegExp(`\\b${campo}: (-?[\\d.]+)`).exec(bloco[1]);
  assert.ok(m, `o painel ${painel} não declara ${campo}`);
  return Number(m[1]);
}

test("o buffer da TORRE tem o mesmo tamanho nos três lados do contrato", () => {
  // O JS não escreve 1024×2048 literal: ele deriva do layout lógico (512×1024) vezes o
  // supersample. Trocar o SUPERSAMPLE para 3 buscando texto mais nítido é uma mudança de uma
  // linha que dobraria o buffer sem tocar em nenhum número que pareça uma dimensão.
  const supersample = constJs(towerJs, "SUPERSAMPLE", "towerCanvas.js");
  const larguraJs = constJs(towerJs, "LOGICAL_W", "towerCanvas.js") * supersample;
  const alturaJs = constJs(towerJs, "LOGICAL_H", "towerCanvas.js") * supersample;

  assert.equal(larguraJs, campoDoPainel("TOWER", "w"), "largura da torre: JS x Rust");
  assert.equal(alturaJs, campoDoPainel("TOWER", "h"), "altura da torre: JS x Rust");
  assert.equal(larguraJs, constCpp("IRACER_OVERLAY_W"), "largura da torre: JS x layer C++");
  assert.equal(alturaJs, constCpp("IRACER_OVERLAY_H"), "altura da torre: JS x layer C++");
});

test("o buffer do RÁDIO tem o mesmo tamanho nos três lados do contrato", () => {
  const supersample = constJs(radioJs, "RADIO_SUPERSAMPLE", "radioCanvas.js");
  const larguraJs = constJs(radioJs, "LOGICAL_W", "radioCanvas.js") * supersample;
  const alturaJs = constJs(radioJs, "LOGICAL_H", "radioCanvas.js") * supersample;

  assert.equal(larguraJs, campoDoPainel("ENGINEER", "w"), "largura do rádio: JS x Rust");
  assert.equal(alturaJs, campoDoPainel("ENGINEER", "h"), "altura do rádio: JS x Rust");
  assert.equal(larguraJs, constCpp("IRACER_ENGINEER_W"), "largura do rádio: JS x layer C++");
  assert.equal(alturaJs, constCpp("IRACER_ENGINEER_H"), "altura do rádio: JS x layer C++");
});

test("os dois painéis usam mapeamentos de nome diferente, e o nome bate com o da layer", () => {
  // Os dois quads compartilham cabeçalho e magic; o que os distingue é o NOME do mapeamento.
  // Nomes iguais fariam os dois escritores brigarem pela mesma página, e o sintoma seria a
  // torre piscando com o card do rádio por cima.
  const nomeRs = (painel) => {
    const bloco = new RegExp(`pub const ${painel}: Panel = Panel \\{([\\s\\S]*?)\\n    \\};`).exec(vrRs);
    const m = /name: "([^"]+)"/.exec(bloco[1]);
    assert.ok(m, `o painel ${painel} não declara name`);
    // Os dois lados escapam a barra invertida dentro do literal, então a forma crua do texto
    // já é comparável: `Local\\iRacer...` no .rs e no L"..." do .h.
    return m[1];
  };
  assert.notEqual(nomeRs("TOWER"), nomeRs("ENGINEER"), "os dois painéis dividem o mesmo mapeamento");
  assert.ok(
    sharedH.includes(nomeRs("ENGINEER")),
    `shared_frame.h não conhece o mapeamento ${nomeRs("ENGINEER")}`,
  );
});

/// O `defaultCfg` de um `static Panel g_*` da layer C++, na ordem do struct OverlayConfig:
/// lockMode, posX, posY, posZ, yawDeg, pitchDeg, scale, visible.
function defaultCfgCpp(variavel) {
  const bloco = new RegExp(`static Panel ${variavel} = \\{([\\s\\S]*?)\\n\\};`).exec(layerCpp);
  assert.ok(bloco, `static Panel ${variavel} sumiu de vr-overlay/src/overlay_layer.cpp`);
  const cfg = /\{([^}]*)\}/.exec(bloco[1]);
  assert.ok(cfg, `${variavel} não declara o defaultCfg entre chaves`);
  const campos = cfg[1]
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean);
  assert.equal(campos.length, 8, `o defaultCfg de ${variavel} deveria ter 8 campos: {${cfg[1]}}`);

  const travas = { IRACER_LOCK_HEAD: 0, IRACER_LOCK_WORLD: 1 };
  assert.ok(campos[0] in travas, `trava desconhecida em ${variavel}: ${campos[0]}`);
  const num = (i) => {
    const m = /^(-?[\d.]+)f?$/.exec(campos[i]);
    assert.ok(m, `o campo ${i} de ${variavel} não é um float literal: ${campos[i]}`);
    return Number(m[1]);
  };
  return {
    lockMode: travas[campos[0]],
    x: num(1),
    y: num(2),
    z: num(3),
    yaw: num(4),
    pitch: num(5),
    scale: num(6),
    visible: campos[7],
  };
}

test("a pose de fábrica é a mesma nos TRÊS lados: JS, Rust e layer C++", () => {
  // O botão "restaurar" do OverlayPositionPanel devolve a pose `factory` do JS, o Rust aplica
  // `def_*` quando não há pose persistida, e a layer usa o `defaultCfg` do painel enquanto o
  // app ainda não escreveu nada na memória compartilhada. Divergir faz o restaurar mandar o
  // jogador para um lugar que nunca foi o padrão dele — e, no caso da layer, faz o painel
  // NASCER num lugar e pular para outro no instante em que o app conecta. As poses foram
  // calibradas na mão, dentro do headset, um valor de cada vez.
  //
  // O terceiro lado entrou em 11/08/2026: o default do rádio no C++ ainda era o head-locked
  // original (y 0.08, z -1.0, pitch 0, escala 1.0) enquanto Rust e JS já estavam no
  // cockpit-lock recalibrado, e o guard só olhava para esses dois.
  const factoryJs = (painel) => {
    const m = new RegExp(`^  ${painel}: \\{[\\s\\S]*?factory: \\{([^}]*)\\}`, "m").exec(posicaoJs);
    assert.ok(m, `não achei a pose de fábrica de ${painel} em src/overlay/overlayPose.js`);
    return Object.fromEntries(
      [...m[1].matchAll(/(\w+):\s*(-?[\d.]+)/g)].map(([, k, v]) => [k, Number(v)]),
    );
  };
  const campos = [
    ["lockMode", "def_lock"],
    ["x", "def_x"],
    ["y", "def_y"],
    ["z", "def_z"],
    ["yaw", "def_yaw"],
    ["pitch", "def_pitch"],
    ["scale", "def_scale"],
  ];
  for (const [painelJs, painelRs, varCpp] of [
    ["tower", "TOWER", "g_tower"],
    ["radio", "ENGINEER", "g_engineer"],
  ]) {
    const f = factoryJs(painelJs);
    const c = defaultCfgCpp(varCpp);
    for (const [campoJs, campoRs] of campos) {
      const rs = campoDoPainel(painelRs, campoRs);
      assert.equal(f[campoJs], rs, `${painelJs}.${campoJs} divergiu de ${painelRs}.${campoRs}`);
      assert.equal(c[campoJs], rs, `${varCpp}.${campoJs} divergiu de ${painelRs}.${campoRs}`);
    }
    // O Rust não tem `def_visible` (escreve 1 direto) e o JS traz `visible: true`; a layer
    // precisa nascer visível também, senão o painel some até alguém abrir o posicionamento.
    assert.equal(c.visible, "true", `${varCpp} deveria nascer visível`);
  }
});

test("a janela overlay do tauri.conf tem o tamanho lógico que a torre desenha", () => {
  // A janela de monitor mostra o MESMO canvas em escala 1:1 lógica (OverlayLiveView divide por
  // SUPERSAMPLE). Uma janela menor que o desenho corta a torre pela direita sem nenhum aviso.
  const janela = conf.app.windows.find((w) => w.label === "overlay");
  assert.ok(janela, "a janela overlay sumiu do tauri.conf.json");
  assert.equal(janela.width, constJs(towerJs, "LOGICAL_W", "towerCanvas.js"));
  assert.equal(janela.height, constJs(towerJs, "LOGICAL_H", "towerCanvas.js"));
});

test("a posição de fábrica do rádio no monitor espelha a janela engineer do tauri.conf", () => {
  // FACTORY_POS vale para quem baixou o app e nunca arrastou a janela: o EngineerRadio só grava
  // `engineerMonitorPos` DEPOIS do primeiro arrasto. Mudar o x/y no conf sem tocar aqui faz a
  // janela nascer num lugar e pular para outro no primeiro reposicionamento.
  const janela = conf.app.windows.find((w) => w.label === "engineer");
  assert.ok(janela, "a janela engineer sumiu do tauri.conf.json");
  const m = /const FACTORY_POS = \{ x: (\d+), y: (\d+) \}/.exec(radioJsx);
  assert.ok(m, "FACTORY_POS sumiu de EngineerRadio.jsx");
  assert.equal(Number(m[1]), janela.x, "FACTORY_POS.x divergiu do tauri.conf.json");
  assert.equal(Number(m[2]), janela.y, "FACTORY_POS.y divergiu do tauri.conf.json");
});
