// Persistência e comparação da pose dos quads de VR.
//
// Extraído de `OverlayPositionPanel.jsx` em 11/08/2026, seguindo o caminho que o próprio
// projeto já abriu com `atlasV2Geometry.js` e `gridMetrics.js`: a lógica pura sai do
// componente e ganha teste espelho. O painel tinha 682 linhas e nenhuma cobertura, num
// arquivo cuja função é guardar poses calibradas na mão DENTRO do headset — o tipo de valor
// que ninguém reconstrói de memória se o storage der errado.
//
// Nada aqui toca React nem Tauri. O componente continua dono do estado e dos `invoke`.
//
// A pose é a fonte da verdade do LADO DO APP: ela vive no localStorage por alvo e é
// empurrada para o backend, que escreve na memória compartilhada lida pela API layer da
// OpenXR. As poses de fábrica espelham os `def_*` de `commands/vr_overlay.rs`, e o guard
// `scripts/tests/vr-overlay-contrato-dimensoes.test.mjs` cobra essa igualdade.

export const TARGET_KEY = "vrOverlayPanelTarget"; // último alvo escolhido (torre/rádio)

// Cada ALVO tem seus comandos, chaves de storage e padrões de fábrica próprios. Os
// factory espelham os defaults da layer/Rust de cada painel (torre = cockpit; rádio =
// head-locked, menor e mais perto).
export const TARGETS = {
  tower: {
    label: "overlay.positionPanel.targetTower",
    setPose: "vr_overlay_set_pose",
    getPose: "vr_overlay_get_pose",
    recenter: "vr_overlay_recenter",
    setRecenterKey: "vr_overlay_set_recenter_key",
    poseKey: "vrOverlayPose",
    defaultKey: "vrOverlayDefault",
    recenterKeyStore: "vrOverlayRecenterKey",
    recenterPadStore: "vrOverlayRecenterPad",
    alvo: "overlay", // nome do alvo no vigia de volante (Rust)
    factory: { lockMode: 1, x: 0, y: 0.61, z: -1.29, yaw: 0, pitch: 30, scale: 1.7, visible: true },
  },
  radio: {
    label: "overlay.positionPanel.targetRadio",
    setPose: "vr_engineer_set_pose",
    getPose: "vr_engineer_get_pose",
    recenter: "vr_engineer_recenter",
    setRecenterKey: "vr_engineer_set_recenter_key",
    poseKey: "vrEngineerPose",
    defaultKey: "vrEngineerDefault",
    recenterKeyStore: "vrEngineerRecenterKey",
    recenterPadStore: "vrEngineerRecenterPad",
    alvo: "engineer",
    // Padrão de fábrica = pose ajustada pelo usuário (cockpit-lock, pequeno e perto).
    factory: { lockMode: 1, x: 0, y: 0.22, z: -0.73, yaw: 0, pitch: 10, scale: 0.26, visible: true },
  },
};

/// Lê uma chave do localStorage e devolve `null` quando ela não existe, está corrompida ou o
/// storage está indisponível.
///
/// TODA leitura de storage aqui é protegida, e não por excesso de zelo: o painel roda também
/// na janela de overlay, onde o storage pode estar bloqueado, e uma exceção na inicialização
/// do `useState` derruba a janela inteira — que é a janela que fica por cima do jogo.
function lerJson(chave) {
  try {
    const cru = localStorage.getItem(chave);
    return cru ? JSON.parse(cru) : null;
  } catch {
    return null;
  }
}

/// O último alvo escolhido, ou a torre.
export function loadTargetName() {
  try {
    const cru = localStorage.getItem(TARGET_KEY);
    if (cru === "radio" || cru === "tower") return cru;
  } catch {
    /* ignora */
  }
  return "tower";
}

/// A tecla de recentro salva (`{ vk, label }`) ou `null`.
export function loadRecenterKey(cfg) {
  const k = lerJson(cfg.recenterKeyStore);
  // `vk` 0 significa DESLIGADA no contrato com o Rust, então ele não é uma tecla válida.
  if (k && typeof k.vk === "number" && k.vk > 0) return k;
  return null;
}

/// O botão de volante salvo (`{ dispositivo, botao }`) ou `null`.
///
/// Dispositivo e botão são índices do Windows e 0 é válido nos dois, então o teste é por
/// TIPO e não por verdade — um `if (b.botao)` descartaria o primeiro botão de todo volante.
export function loadRecenterPad(cfg) {
  const b = lerJson(cfg.recenterPadStore);
  if (b && Number.isInteger(b.dispositivo) && Number.isInteger(b.botao)) return b;
  return null;
}

/// O padrão EFETIVO do alvo: o que o jogador fixou, ou o de fábrica se nunca fixou.
export function loadDefaults(cfg) {
  const salvo = lerJson(cfg.defaultKey);
  // Mescla sobre o de fábrica em vez de substituir: um padrão salvo por uma versão antiga
  // do painel não tem os campos novos (o `pitch` entrou depois), e substituir deixaria a
  // pose com buraco justo no eixo que o jogador não vê no slider.
  return salvo ? { ...cfg.factory, ...salvo } : { ...cfg.factory };
}

/// A pose atual do alvo, em camadas: fábrica → padrão do jogador → pose salva.
export function loadPose(cfg) {
  const salva = lerJson(cfg.poseKey);
  return salva ? { ...loadDefaults(cfg), ...salva } : loadDefaults(cfg);
}

/// Grava a pose. Falha de storage é silenciosa de propósito: sem persistência o painel
/// continua funcionando na sessão, e um alerta aqui interromperia o ajuste ao vivo.
export function savePose(cfg, pose) {
  try {
    localStorage.setItem(cfg.poseKey, JSON.stringify(pose));
  } catch {
    /* storage cheio/indisponível: sem persistência, tudo bem */
  }
}

/// Igualdade tolerante entre duas poses.
///
/// A layer devolve `f32` e o app guarda `f64`, então a ida e volta pela memória compartilhada
/// muda o último dígito. Comparar por `===` faria o painel achar que a pose mudou a cada
/// leitura e reescrever para sempre — um laço de escrita a cada poll.
export function poseEq(a, b) {
  if (!a || !b) return false;
  const perto = (x, y) => Math.abs(x - y) < 1e-3;
  return (
    a.lockMode === b.lockMode &&
    a.visible === b.visible &&
    perto(a.x, b.x) &&
    perto(a.y, b.y) &&
    perto(a.z, b.z) &&
    perto(a.yaw, b.yaw) &&
    perto(a.pitch ?? 0, b.pitch ?? 0) &&
    perto(a.scale, b.scale)
  );
}

/// Os argumentos do comando de pose, na forma que o Rust espera.
///
/// Existe como função para que o teste consiga cobrar a FORMA do payload: o comando é
/// resolvido por string e os campos por nome, então um campo renomeado aqui vira um
/// `undefined` do lado do Rust sem erro nenhum.
export function posePayload(pose) {
  return {
    lockMode: pose.lockMode,
    x: pose.x,
    y: pose.y,
    z: pose.z,
    yaw: pose.yaw,
    pitch: pose.pitch,
    scale: pose.scale,
    visible: pose.visible,
  };
}
