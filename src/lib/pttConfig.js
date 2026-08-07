// Onde a associação do botão de push-to-talk fica guardada.
//
// Mora no front, e não no Rust, pelo mesmo desenho do botão de recentro do VR: o backend
// tem a ligação VIVA (o vigia precisa dela) e o front tem a ligação PERSISTIDA, que ele
// reempurra no boot. Guardar dos dois lados renderia duas fontes de verdade para a mesma
// tecla, e elas divergiriam no primeiro fechamento sujo do app.
//
// Este módulo existe separado porque duas telas precisam dele sem se conhecerem: o
// `EngenheiroPttAuto` reempurra ao subir, e as configurações escrevem.

export const GATILHO_STORE = "loop.pttGatilho";

/** O gatilho salvo, no formato que o Rust espera, ou `null`. */
export function lerGatilhoSalvo() {
  try {
    const cru = localStorage.getItem(GATILHO_STORE);
    if (!cru) return null;
    const g = JSON.parse(cru);
    return validar(g);
  } catch {
    return null;
  }
}

/**
 * Valida o que veio do armazenamento antes de mandar ao backend.
 *
 * Não é paranoia: o `localStorage` sobrevive a versões do app, e um formato antigo — ou
 * um `{tipo:"tecla"}` sem código — chegaria ao Rust como um gatilho que nunca dispara, ou
 * pior, como um que dispara sozinho. Melhor tratar como "não associado".
 */
export function validar(g) {
  if (!g || typeof g !== "object") return null;
  if (g.tipo === "volante") {
    const { dispositivo, botao } = g;
    if (!Number.isInteger(dispositivo) || !Number.isInteger(botao)) return null;
    if (dispositivo < 0 || dispositivo > 15 || botao < 0 || botao > 31) return null;
    return { tipo: "volante", dispositivo, botao };
  }
  if (g.tipo === "tecla") {
    const { codigo } = g;
    // 0 não é Virtual-Key nenhum, e 255 é o teto da tabela do Windows.
    if (!Number.isInteger(codigo) || codigo <= 0 || codigo > 255) return null;
    return { tipo: "tecla", codigo };
  }
  return null;
}

/** Guarda (ou apaga, com `null`) e avisa quem estiver ouvindo na MESMA janela — o evento
 *  `storage` do navegador só chega às outras. */
export function salvarGatilho(g) {
  const limpo = g ? validar(g) : null;
  try {
    if (limpo) localStorage.setItem(GATILHO_STORE, JSON.stringify(limpo));
    else localStorage.removeItem(GATILHO_STORE);
  } catch {
    /* sem persistência: vale só nesta sessão */
  }
  window.dispatchEvent(new Event("loop:ptt-gatilho"));
  return limpo;
}

// ── Microfone escolhido ──────────────────────────────────────────────────────
//
// Guardado pelo mesmo motivo do gatilho: quem abre o microfone de verdade é o
// `EngenheiroPttAuto`, quando a sessão começa — e ele não estava nas configurações para
// saber o que o jogador escolheu. Sem isto, a escolha valia só enquanto a tela estava
// aberta e a corrida caía no padrão do Windows, que num rig de VR costuma ser o áudio
// virtual do headset e não o microfone.
export const MIC_STORE = "loop.pttMicrofone";

/** O `deviceId` escolhido, ou `null` para o padrão do Windows. */
export function lerMicSalvo() {
  try {
    const id = localStorage.getItem(MIC_STORE);
    return id || null;
  } catch {
    return null;
  }
}

export function salvarMic(deviceId) {
  try {
    if (deviceId) localStorage.setItem(MIC_STORE, deviceId);
    else localStorage.removeItem(MIC_STORE);
  } catch {
    /* sem persistência: vale só nesta sessão */
  }
  window.dispatchEvent(new Event("loop:ptt-microfone"));
  return deviceId || null;
}

// ── Modo de teste ────────────────────────────────────────────────────────────
//
// Enquanto o bloco de diagnóstico das configurações está aberto, o orquestrador de
// verdade não pode reagir ao botão: se o jogador estiver com uma sessão aberta atrás,
// segurar o botão para conferir o microfone dispararia uma pergunta ao engenheiro — e,
// sem os endpoints no ar, uma desistência falada em cima do teste.
//
// Estado de módulo simples porque as duas pontas moram na MESMA janela (a principal, a
// única onde o áudio pode tocar). Um evento aqui só acrescentaria cerimônia.
let emTeste = false;

export function entrarEmTeste() {
  emTeste = true;
}
export function sairDeTeste() {
  emTeste = false;
}
export function estaEmTeste() {
  return emTeste;
}

/** Como o jogador lê a associação na tela. */
export function rotuloDoGatilho(g, nomeDaTecla = null) {
  if (!g) return "";
  if (g.tipo === "volante") return `Volante ${g.dispositivo} · botão ${g.botao + 1}`;
  return nomeDaTecla || `Tecla ${g.codigo}`;
}
