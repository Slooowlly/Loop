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
const OPENXR_TAG = "release-1.1.43"; // mesmo pin do CMakeLists.txt
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
function garanteHeaders() {
  if (existsSync(path.join(OPENXR_DIR, "include", "openxr", "openxr.h"))) return;
  log(`clonando headers do OpenXR (${OPENXR_TAG})…`);
  try {
    execFileSync(
      "git",
      ["clone", "--depth", "1", "--branch", OPENXR_TAG, "https://github.com/KhronosGroup/OpenXR-SDK.git", OPENXR_DIR],
      { stdio: "inherit" },
    );
  } catch {
    faltaAmbiente(`falha ao clonar os headers do OpenXR em ${OPENXR_DIR}. Precisa de git e rede na primeira vez.`);
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
    "cl /nologo /std:c++17 /LD /EHsc /W4",
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
