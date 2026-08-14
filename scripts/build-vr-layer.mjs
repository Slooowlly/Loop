#!/usr/bin/env node
// Compila a API layer do OpenXR (vr-overlay/src/overlay_layer.cpp) para dentro de
// src-tauri/resources/, de onde o bundle do Tauri a distribui pro jogador.
//
// POR QUE ISTO EXISTE
// A layer é o LEITOR do overlay de VR: sem ela instalada, o app escreve os frames na
// memória compartilhada e ninguém os lê — o overlay simplesmente não existe naquela
// máquina. Ela é C++ e vive fora do ciclo do cargo/vite, então precisa de um passo
// próprio, amarrado ao build (ver `beforeBuildCommand` no tauri.conf.json).
//
// A DLL é ARTEFATO, não fonte: não vai pro git. Compilar a partir do .cpp em cada build
// é o que garante que a versão publicada casa com o código — um binário commitado
// envelheceria em silêncio e o sintoma seria um bug de VR que não reproduz em lugar
// nenhum. No build de release, toolchain ausente FALHA o build em vez de deixar passar
// um bundle sem a layer (ver `--opcional` abaixo pro modo dev).
//
// Requisitos: Visual Studio 2022 com "Desktop development with C++" e git no PATH.
// Uso:
//   node scripts/build-vr-layer.mjs             # incremental (pula se a DLL está fresca)
//   node scripts/build-vr-layer.mjs --force     # recompila sempre
//   node scripts/build-vr-layer.mjs --opcional  # AMBIENTE faltando não é erro (modo dev)
//
// `--opcional` distingue duas falhas que não são a mesma coisa: TOOLCHAIN AUSENTE (quem
// só mexe na interface não deveria ficar impedido de rodar `tauri dev` por não ter o
// Visual Studio) e CÓDIGO QUEBRADO (isso falha sempre, em qualquer modo). No build de
// release não se usa a flag: lá a layer é obrigatória.

import { execFileSync, execSync } from "node:child_process";
import { existsSync, mkdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const RAIZ = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const SRC_DIR = path.join(RAIZ, "vr-overlay", "src");
const FONTES = ["overlay_layer.cpp", "shared_frame.h"].map((f) => path.join(SRC_DIR, f));
const OPENXR_DIR = path.join(SRC_DIR, "OpenXR-SDK");
const OPENXR_URL = "https://github.com/KhronosGroup/OpenXR-SDK.git";
// Pin pelo SHA DO COMMIT, não pela tag: tag é ponteiro móvel e quem publica pode reapontar
// `release-1.1.43` para outro commit sem que nada aqui mude. O SHA é o conteúdo. Mesmo pin
// do CMakeLists.txt — o guard scripts/tests/vr-overlay-build-pin.test.mjs cobra a igualdade.
const OPENXR_SHA = "781f2eab3698d653c804ecbd11e0aed47eaad1c6";
const OPENXR_REF = "release-1.1.43"; // só documental: qual release é esse SHA
const OUT_DIR = path.join(RAIZ, "src-tauri", "resources");
const OUT_DLL = path.join(OUT_DIR, "iracer_overlay_layer.dll");
const OBJ_DIR = path.join(RAIZ, "vr-overlay", "build", "cl");

const force = process.argv.includes("--force");
const opcional = process.argv.includes("--opcional");

function log(msg) {
  console.log(`[vr-layer] ${msg}`);
}

function morre(msg) {
  console.error(`[vr-layer] ERRO: ${msg}`);
  process.exit(1);
}

// Falha de AMBIENTE (toolchain/rede). Com `--opcional` vira aviso: o app sobe sem a
// layer e só o overlay de VR não existe naquela sessão.
function faltaAmbiente(msg) {
  if (!opcional) morre(msg);
  console.warn(`[vr-layer] AVISO: ${msg}`);
  console.warn("[vr-layer] seguindo sem a layer — o overlay de VR não vai funcionar nesta sessão.");
  process.exit(0);
}

// ── A DLL já está mais nova que todas as fontes? ──
function estaFresca() {
  if (force || !existsSync(OUT_DLL)) return false;
  const dll = statSync(OUT_DLL).mtimeMs;
  return FONTES.every((f) => existsSync(f) && statSync(f).mtimeMs <= dll);
}

// ── Headers do OpenXR (só headers; uma API layer não linka o loader) ──

/// O commit em que o clone está, ou "" se o diretório não é um repositório utilizável.
function headDoClone() {
  try {
    return execFileSync("git", ["-C", OPENXR_DIR, "rev-parse", "HEAD"], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    }).trim();
  } catch {
    return "";
  }
}

/// Traz o SHA pinado para o clone (novo ou existente) e o deixa em HEAD.
///
/// É init + fetch + checkout em vez de `clone --branch`: o git não clona por SHA, só por
/// ref. O GitHub aceita `fetch <sha>` de qualquer commit alcançável, então esta é a forma
/// de buscar exatamente o commit pinado sem depender de a tag continuar apontando pra ele.
function buscaSha() {
  if (!existsSync(path.join(OPENXR_DIR, ".git"))) {
    mkdirSync(OPENXR_DIR, { recursive: true });
    execFileSync("git", ["-C", OPENXR_DIR, "init", "-q"], { stdio: "inherit" });
    execFileSync("git", ["-C", OPENXR_DIR, "remote", "add", "origin", OPENXR_URL], { stdio: "inherit" });
  }
  execFileSync("git", ["-C", OPENXR_DIR, "fetch", "--depth", "1", "origin", OPENXR_SHA], { stdio: "inherit" });
  execFileSync("git", ["-C", OPENXR_DIR, "checkout", "-q", "--force", OPENXR_SHA], { stdio: "inherit" });
}

function garanteHeaders() {
  const temHeaders = existsSync(path.join(OPENXR_DIR, "include", "openxr", "openxr.h"));
  const head = temHeaders ? headDoClone() : "";
  if (temHeaders && head === OPENXR_SHA) return;

  // Clone existente em outro commit: a máquina de quem clonou antes do pin (ou com a tag
  // ainda móvel) compilaria contra headers diferentes dos do CI e ninguém veria. Reconcilia.
  if (temHeaders) {
    log(`clone do OpenXR está em ${head || "commit desconhecido"} — reconciliando com o pin ${OPENXR_SHA}…`);
  } else {
    log(`buscando headers do OpenXR (${OPENXR_REF} = ${OPENXR_SHA})…`);
  }

  try {
    buscaSha();
  } catch {
    faltaAmbiente(
      `falha ao buscar o commit ${OPENXR_SHA} do OpenXR em ${OPENXR_DIR}. Precisa de git e rede na primeira vez.`,
    );
  }

  const agora = headDoClone();
  if (agora !== OPENXR_SHA) {
    morre(`o clone do OpenXR ficou em ${agora || "commit desconhecido"}, esperado ${OPENXR_SHA}.`);
  }
  if (!existsSync(path.join(OPENXR_DIR, "include", "openxr", "openxr.h"))) {
    morre(`o commit ${OPENXR_SHA} foi buscado mas ${OPENXR_DIR}/include/openxr/openxr.h não existe.`);
  }
}

// ── Onde está o Visual Studio (pra carregar o vcvars64) ──
function achaVcvars() {
  const vswhere = path.join(
    process.env["ProgramFiles(x86)"] || "C:\\Program Files (x86)",
    "Microsoft Visual Studio",
    "Installer",
    "vswhere.exe",
  );
  if (!existsSync(vswhere)) {
    faltaAmbiente("vswhere.exe não encontrado — o Visual Studio 2022 está instalado?");
  }
  let instalacao = "";
  try {
    instalacao = execFileSync(
      vswhere,
      ["-latest", "-products", "*", "-requires", "Microsoft.VisualStudio.Component.VC.Tools.x86.x64", "-property", "installationPath"],
      { encoding: "utf8" },
    ).trim();
  } catch {
    /* trata como ausente abaixo */
  }
  if (!instalacao) {
    faltaAmbiente('Visual Studio com "Desktop development with C++" não encontrado (o workload de C++ é obrigatório).');
  }
  const vcvars = path.join(instalacao, "VC", "Auxiliary", "Build", "vcvars64.bat");
  if (!existsSync(vcvars)) {
    faltaAmbiente(`vcvars64.bat não encontrado em ${vcvars}`);
  }
  return vcvars;
}

if (process.platform !== "win32") {
  // Fora do Windows não há iRacing nem OpenXR/DX11 — o alvo real é windows-latest.
  log("plataforma não-Windows: nada a compilar.");
  process.exit(0);
}

if (estaFresca()) {
  log("DLL já está mais nova que as fontes — nada a fazer.");
  process.exit(0);
}

garanteHeaders();
const vcvars = achaVcvars();
mkdirSync(OUT_DIR, { recursive: true });
mkdirSync(OBJ_DIR, { recursive: true });

// /LD = DLL. Os intermediários (.obj/.lib/.exp) vão pro build/ pra não sujar o src/.
const compilar = [
  `call "${vcvars}"`,
  [
    // /utf-8 = fonte E execução em UTF-8. Sem ele o cl lê o .cpp na codepage do sistema e
    // todo acento das mensagens vira mojibake no %TEMP%\iracer_overlay_layer.log — justo o
    // arquivo que a gente lê pra descobrir por que o overlay não apareceu. Mesmo flag do
    // CMakeLists.txt, que é o outro caminho de build da layer.
    "cl /nologo /std:c++17 /utf-8 /LD /EHsc /W4",
    `/I"${path.join(OPENXR_DIR, "include")}"`,
    // Barra DUPLA antes da aspa: um `\"` seria lido pelo cl como aspa escapada e ele
    // engoliria o resto da linha ("missing source filename").
    `/Fo"${OBJ_DIR}\\\\"`,
    `"${path.join(SRC_DIR, "overlay_layer.cpp")}"`,
    "/link d3d11.lib dxgi.lib user32.lib",
    `/OUT:"${OUT_DLL}"`,
    `/IMPLIB:"${path.join(OBJ_DIR, "overlay_layer.lib")}"`,
  ].join(" "),
].join(" && ");

log("compilando a API layer…");
try {
  // `execSync` (não execFileSync) de propósito: ele monta `cmd /d /s /c "<tudo>"`, e é o
  // /s que preserva as aspas internas dos caminhos com espaço ("Program Files").
  execSync(compilar, { stdio: "inherit", cwd: SRC_DIR });
} catch {
  morre("a compilação da layer falhou (veja a saída do cl acima).");
}

if (!existsSync(OUT_DLL)) {
  morre(`o cl terminou sem erro mas ${OUT_DLL} não existe.`);
}
log(`pronto: ${path.relative(RAIZ, OUT_DLL)} (${statSync(OUT_DLL).size} bytes)`);
