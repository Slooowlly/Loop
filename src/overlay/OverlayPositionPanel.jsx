import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";

// Painel de POSIÇÃO do overlay de VR. Deixa você:
//   • escolher o ALVO: TORRE (timing) ou RÁDIO (card do engenheiro) — cada um é um quad
//     independente na layer, com pose/padrão/tecla próprios;
//   • travar no COCKPIT (fixo no mundo) ou na CABEÇA (segue o olhar);
//   • mover em X/Y/Z (metros), angular (yaw), inclinar (pitch) e escalar o painel.
//
// A pose é a fonte da verdade AQUI (persistida em localStorage, por alvo) e empurrada
// pro backend (`vr_overlay_*` pra torre, `vr_engineer_*` pro rádio), que escreve na
// memória compartilhada lida pela OpenXR API layer. Ajuste ao vivo: mexeu no slider,
// mexeu no VR.
//
// Dica de uso no Pico + Virtual Desktop: com o desktop visível no VD dá pra ver o
// painel do app e o overlay ao mesmo tempo — posiciona uma vez e fica gravado.

const IN_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
const TARGET_KEY = "vrOverlayPanelTarget"; // último alvo escolhido (torre/rádio)

// Cada ALVO tem seus comandos, chaves de storage e padrões de fábrica próprios. Os
// factory espelham os defaults da layer/Rust de cada painel (torre = cockpit; rádio =
// head-locked, menor e mais perto).
const TARGETS = {
  tower: {
    label: "overlay.positionPanel.targetTower",
    setPose: "vr_overlay_set_pose",
    getPose: "vr_overlay_get_pose",
    recenter: "vr_overlay_recenter",
    setRecenterKey: "vr_overlay_set_recenter_key",
    poseKey: "vrOverlayPose",
    defaultKey: "vrOverlayDefault",
    recenterKeyStore: "vrOverlayRecenterKey",
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
    // Padrão de fábrica = pose ajustada pelo usuário (cockpit-lock, pequeno e perto).
    factory: { lockMode: 1, x: 0, y: 0.22, z: -0.73, yaw: 0, pitch: 10, scale: 0.26, visible: true },
  },
};

function loadTargetName() {
  try {
    const raw = localStorage.getItem(TARGET_KEY);
    if (raw === "radio" || raw === "tower") return raw;
  } catch {
    /* ignora */
  }
  return "tower";
}

// Carrega a tecla de recentro salva ({ vk, label }) ou null, do alvo dado.
function loadRecenterKey(cfg) {
  try {
    const raw = localStorage.getItem(cfg.recenterKeyStore);
    if (raw) {
      const k = JSON.parse(raw);
      if (k && typeof k.vk === "number" && k.vk > 0) return k;
    }
  } catch {
    /* ignora storage corrompido */
  }
  return null;
}

// O "padrão" efetivo do alvo: o que o usuário fixou, ou os de fábrica se nunca fixou.
function loadDefaults(cfg) {
  try {
    const raw = localStorage.getItem(cfg.defaultKey);
    if (raw) return { ...cfg.factory, ...JSON.parse(raw) };
  } catch {
    /* ignora storage corrompido */
  }
  return { ...cfg.factory };
}

function loadPose(cfg) {
  try {
    const raw = localStorage.getItem(cfg.poseKey);
    if (raw) return { ...loadDefaults(cfg), ...JSON.parse(raw) };
  } catch {
    /* ignora storage corrompido */
  }
  return loadDefaults(cfg);
}

function push(cfg, pose) {
  if (!IN_TAURI) return;
  invoke(cfg.setPose, {
    lockMode: pose.lockMode,
    x: pose.x,
    y: pose.y,
    z: pose.z,
    yaw: pose.yaw,
    pitch: pose.pitch,
    scale: pose.scale,
    visible: pose.visible,
  }).catch(() => {});
}

function save(cfg, pose) {
  try {
    localStorage.setItem(cfg.poseKey, JSON.stringify(pose));
  } catch {
    /* storage cheio/indisponível: sem persistência, tudo bem */
  }
}

// Igualdade tolerante (a layer devolve f32; o app guarda f64).
function poseEq(a, b) {
  if (!a || !b) return false;
  const near = (x, y) => Math.abs(x - y) < 1e-3;
  return (
    a.lockMode === b.lockMode &&
    a.visible === b.visible &&
    near(a.x, b.x) &&
    near(a.y, b.y) &&
    near(a.z, b.z) &&
    near(a.yaw, b.yaw) &&
    near(a.pitch ?? 0, b.pitch ?? 0) &&
    near(a.scale, b.scale)
  );
}

function Slider({ label, value, min, max, step, unit, onChange }) {
  return (
    <label style={{ display: "block", marginBottom: 8 }}>
      <div style={{ display: "flex", justifyContent: "space-between", fontSize: 11, marginBottom: 2 }}>
        <span style={{ color: "#9aa4ad" }}>{label}</span>
        <span style={{ color: "#e6edf3", fontVariantNumeric: "tabular-nums" }}>
          {value.toFixed(2)}
          {unit}
        </span>
      </div>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(parseFloat(e.target.value))}
        style={{ width: "100%", accentColor: "#3fb950" }}
      />
    </label>
  );
}

export default function OverlayPositionPanel() {
  const { t } = useTranslation();
  const [target, setTarget] = useState(loadTargetName);
  const cfgRef = useRef(TARGETS[target]);
  cfgRef.current = TARGETS[target]; // sempre coerente com o render atual

  const [pose, setPose] = useState(() => loadPose(TARGETS[loadTargetName()]));
  const [open, setOpen] = useState(false);
  const [savedAt, setSavedAt] = useState(0); // > 0 = "padrão salvo" (some ao mexer)
  const [recenterKey, setRecenterKey] = useState(() => loadRecenterKey(TARGETS[loadTargetName()]));
  const [capturing, setCapturing] = useState(false); // ouvindo a próxima tecla?
  const poseRef = useRef(pose); // espelho pra comparar sem depender do render

  // Troca de ALVO (Torre/Rádio): carrega a pose/tecla daquele quad e empurra pra layer.
  const switchTarget = (t) => {
    if (t === target) return;
    const c = TARGETS[t];
    cfgRef.current = c;
    setTarget(t);
    try {
      localStorage.setItem(TARGET_KEY, t);
    } catch {
      /* ignora */
    }
    const p = loadPose(c);
    poseRef.current = p;
    setPose(p);
    push(c, p);
    const rk = loadRecenterKey(c);
    setRecenterKey(rk);
    if (IN_TAURI) invoke(c.setRecenterKey, { vk: rk?.vk ?? 0 }).catch(() => {});
    setSavedAt(0);
  };

  // Mudança do USUÁRIO (slider/toggle): estado + empurra pro alvo + persiste.
  const set = (patch) => {
    const next = { ...poseRef.current, ...patch };
    poseRef.current = next;
    setPose(next);
    push(cfgRef.current, next);
    save(cfgRef.current, next);
    setSavedAt(0); // mexeu depois de salvar → a confirmação some
  };
  // Reset volta pra pose PADRÃO do alvo (a que o usuário fixou, ou a de fábrica).
  const reset = () => set(loadDefaults(cfgRef.current));

  // Fixa a pose atual como o novo padrão do alvo (vale pra qualquer VR: é o que o
  // overlay assume ao abrir e o destino do botão "Padrão").
  const setAsDefault = () => {
    try {
      localStorage.setItem(cfgRef.current.defaultKey, JSON.stringify(poseRef.current));
      setSavedAt(Date.now());
    } catch {
      /* storage indisponível: sem persistência do padrão */
    }
  };

  // Recentra AGORA: reancora o world-lock do alvo na cabeça atual (mesma posição sempre).
  const recenter = () => {
    if (!IN_TAURI) return;
    invoke(cfgRef.current.recenter).catch(() => {});
  };

  // No mount: restaura a pose salva na layer (e cria a ponte na memória) + a tecla
  // de recentro (pra valer dentro do VR mesmo sem o app em foco).
  useEffect(() => {
    push(cfgRef.current, poseRef.current);
    if (IN_TAURI) {
      invoke(cfgRef.current.setRecenterKey, { vk: recenterKey?.vk ?? 0 }).catch(() => {});
    }
  }, []);

  // Captura de tecla: enquanto "capturing", a próxima tecla vira a de recentro do alvo.
  useEffect(() => {
    if (!capturing) return undefined;
    const onKey = (e) => {
      e.preventDefault();
      if (e.key === "Escape") {
        setCapturing(false);
        return;
      }
      const vk = e.keyCode; // p/ F1–F12, letras e dígitos = Virtual-Key do Windows
      if (!vk) return;
      const label = e.key.length === 1 ? e.key.toUpperCase() : e.key;
      const k = { vk, label };
      setRecenterKey(k);
      setCapturing(false);
      try {
        localStorage.setItem(cfgRef.current.recenterKeyStore, JSON.stringify(k));
      } catch {
        /* sem persistência: tudo bem */
      }
      if (IN_TAURI) invoke(cfgRef.current.setRecenterKey, { vk }).catch(() => {});
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [capturing]);

  const clearRecenterKey = () => {
    setRecenterKey(null);
    setCapturing(false);
    try {
      localStorage.removeItem(cfgRef.current.recenterKeyStore);
    } catch {
      /* ignora */
    }
    if (IN_TAURI) invoke(cfgRef.current.setRecenterKey, { vk: 0 }).catch(() => {});
  };

  // Poll: adota o que o AJUSTE POR TECLADO mudou na layer, no alvo corrente. NÃO
  // reempurra (senão brigaria com o "segurar tecla" — a layer é a dona da SHM aí).
  useEffect(() => {
    if (!IN_TAURI) return undefined;
    let stopped = false;
    const timer = setInterval(async () => {
      try {
        const p = await invoke(cfgRef.current.getPose);
        if (stopped || !p || poseEq(p, poseRef.current)) return;
        poseRef.current = p;
        setPose(p);
        save(cfgRef.current, p);
        setSavedAt(0); // teclado mexeu → confirmação de "padrão salvo" some
      } catch {
        /* ponte ainda não existe / não é Tauri: ignora */
      }
    }, 500);
    return () => {
      stopped = true;
      clearInterval(timer);
    };
  }, []);

  if (!IN_TAURI) return null;

  if (!open) {
    // Recolhido: só uma engrenagem discreta no canto. Fica translúcida e ganha
    // opacidade no hover — presente pra quem procura, invisível pro resto.
    return (
      <button
        onClick={() => setOpen(true)}
        title={t("overlay.positionPanel.gearTooltip")}
        onMouseEnter={(e) => {
          e.currentTarget.style.opacity = "1";
        }}
        onMouseLeave={(e) => {
          e.currentTarget.style.opacity = "0.25";
        }}
        style={{
          position: "fixed",
          right: 8,
          bottom: 8,
          zIndex: 9999,
          width: 26,
          height: 26,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          background: "transparent",
          color: "#9aa4ad",
          border: "none",
          fontSize: 15,
          lineHeight: 1,
          opacity: 0.25,
          cursor: "pointer",
          transition: "opacity 0.15s ease",
        }}
      >
        ⚙
      </button>
    );
  }

  return (
    <div
      style={{
        position: "fixed",
        right: 8,
        bottom: 40,
        zIndex: 9999,
        width: 230,
        background: "rgba(12,14,17,0.94)",
        color: "#e6edf3",
        border: "1px solid rgba(255,255,255,0.14)",
        borderRadius: 10,
        padding: 12,
        font: "12px system-ui, sans-serif",
        boxShadow: "0 8px 28px rgba(0,0,0,0.5)",
      }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 10 }}>
        <strong style={{ fontSize: 12, letterSpacing: 0.3 }}>{t("overlay.positionPanel.title")}</strong>
        <button
          onClick={() => setOpen(false)}
          style={{ background: "none", border: "none", color: "#9aa4ad", cursor: "pointer", fontSize: 14 }}
        >
          ×
        </button>
      </div>

      {/* Alvo: TORRE vs RÁDIO — decide qual quad os controles abaixo posicionam. */}
      <div style={{ display: "flex", gap: 6, marginBottom: 10 }}>
        {Object.entries(TARGETS).map(([key, cfg]) => (
          <button
            key={key}
            onClick={() => switchTarget(key)}
            style={{
              flex: 1,
              padding: "6px 0",
              borderRadius: 6,
              border: "1px solid " + (target === key ? "#58a6ff" : "rgba(255,255,255,0.14)"),
              background: target === key ? "rgba(88,166,255,0.18)" : "transparent",
              color: target === key ? "#79c0ff" : "#9aa4ad",
              cursor: "pointer",
              fontSize: 11,
              fontWeight: 700,
            }}
          >
            {t(cfg.label)}
          </button>
        ))}
      </div>

      {target === "radio" && (
        <p style={{ fontSize: 10, color: "#6e7681", margin: "0 0 10px", lineHeight: 1.4 }}>
          {t("overlay.positionPanel.radioHintPre")}{" "}
          <strong>{t("overlay.positionPanel.radioHintTerm")}</strong>{" "}
          {t("overlay.positionPanel.radioHintPost")}
        </p>
      )}

      {/* Trava: cockpit (world) vs cabeça (view) */}
      <div style={{ display: "flex", gap: 6, marginBottom: 12 }}>
        {[
          { m: 1, label: "overlay.positionPanel.lockCockpit" },
          { m: 0, label: "overlay.positionPanel.lockHead" },
        ].map(({ m, label }) => (
          <button
            key={m}
            onClick={() => set({ lockMode: m })}
            style={{
              flex: 1,
              padding: "6px 0",
              borderRadius: 6,
              border: "1px solid " + (pose.lockMode === m ? "#3fb950" : "rgba(255,255,255,0.14)"),
              background: pose.lockMode === m ? "rgba(63,185,80,0.18)" : "transparent",
              color: pose.lockMode === m ? "#7ee787" : "#9aa4ad",
              cursor: "pointer",
              fontSize: 11,
              fontWeight: 600,
            }}
          >
            {t(label)}
          </button>
        ))}
      </div>

      <Slider label={t("overlay.positionPanel.sliderHorizontal")} value={pose.x} min={-1.5} max={1.5} step={0.01} unit="m" onChange={(x) => set({ x })} />
      <Slider label={t("overlay.positionPanel.sliderHeight")} value={pose.y} min={-1.2} max={1.2} step={0.01} unit="m" onChange={(y) => set({ y })} />
      <Slider label={t("overlay.positionPanel.sliderDistance")} value={pose.z} min={-2.5} max={-0.3} step={0.01} unit="m" onChange={(z) => set({ z })} />
      <Slider label={t("overlay.positionPanel.sliderRotation")} value={pose.yaw} min={-45} max={45} step={1} unit="°" onChange={(yaw) => set({ yaw })} />
      <Slider
        label={t("overlay.positionPanel.sliderTilt")}
        value={pose.pitch ?? cfgRef.current.factory.pitch}
        min={-45}
        max={45}
        step={1}
        unit="°"
        onChange={(pitch) => set({ pitch })}
      />
      <Slider label={t("overlay.positionPanel.sliderSize")} value={pose.scale} min={0.2} max={2} step={0.02} unit="×" onChange={(scale) => set({ scale })} />

      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginTop: 6 }}>
        <label style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 11, color: "#9aa4ad", cursor: "pointer" }}>
          <input type="checkbox" checked={pose.visible} onChange={(e) => set({ visible: e.target.checked })} />
          {t("overlay.positionPanel.visible")}
        </label>
        <button
          onClick={reset}
          style={{
            background: "none",
            border: "1px solid rgba(255,255,255,0.14)",
            color: "#9aa4ad",
            borderRadius: 6,
            padding: "4px 10px",
            cursor: "pointer",
            fontSize: 11,
          }}
        >
          {t("overlay.positionPanel.resetDefault")}
        </button>
      </div>

      {/* Fixa a pose atual como padrão desta máquina (aplica em qualquer VR). */}
      <button
        onClick={setAsDefault}
        style={{
          width: "100%",
          marginTop: 10,
          background: savedAt ? "rgba(63,185,80,0.18)" : "rgba(63,185,80,0.10)",
          border: "1px solid rgba(63,185,80,0.5)",
          color: "#7ee787",
          borderRadius: 6,
          padding: "6px 0",
          cursor: "pointer",
          fontSize: 11,
          fontWeight: 600,
        }}
      >
        {savedAt ? t("overlay.positionPanel.savedDefault") : t("overlay.positionPanel.setAsDefault")}
      </button>

      {/* ── Recentro: reancora o overlay na cabeça atual (mesma posição sempre) ── */}
      <div style={{ borderTop: "1px solid rgba(255,255,255,0.1)", marginTop: 12, paddingTop: 10 }}>
        <button
          onClick={recenter}
          title={t("overlay.positionPanel.recenterTooltip")}
          style={{
            width: "100%",
            background: "rgba(88,166,255,0.12)",
            border: "1px solid rgba(88,166,255,0.5)",
            color: "#79c0ff",
            borderRadius: 6,
            padding: "7px 0",
            cursor: "pointer",
            fontSize: 12,
            fontWeight: 600,
          }}
        >
          {t("overlay.positionPanel.recenterBtn", { target: t(cfgRef.current.label).toLowerCase() })}
        </button>

        <div style={{ display: "flex", alignItems: "center", gap: 8, marginTop: 8 }}>
          <span style={{ fontSize: 11, color: "#9aa4ad", flex: 1 }}>
            {t("overlay.positionPanel.vrKey")}{" "}
            <strong style={{ color: capturing ? "#e3b341" : "#e6edf3" }}>
              {capturing ? t("overlay.positionPanel.pressKey") : recenterKey ? recenterKey.label : t("overlay.positionPanel.none")}
            </strong>
          </span>
          <button
            onClick={() => setCapturing((c) => !c)}
            style={{
              background: "none",
              border: "1px solid rgba(255,255,255,0.14)",
              color: "#9aa4ad",
              borderRadius: 6,
              padding: "4px 8px",
              cursor: "pointer",
              fontSize: 11,
            }}
          >
            {capturing ? t("overlay.positionPanel.cancel") : t("overlay.positionPanel.capture")}
          </button>
          {recenterKey && !capturing && (
            <button
              onClick={clearRecenterKey}
              title={t("overlay.positionPanel.clearKeyTooltip")}
              style={{
                background: "none",
                border: "1px solid rgba(255,255,255,0.14)",
                color: "#9aa4ad",
                borderRadius: 6,
                padding: "4px 8px",
                cursor: "pointer",
                fontSize: 11,
              }}
            >
              ×
            </button>
          )}
        </div>
        <p style={{ fontSize: 10, color: "#6e7681", margin: "6px 0 0", lineHeight: 1.4 }}>
          {t("overlay.positionPanel.recenterKeyHint")} <strong>Ctrl direito + C</strong>.
        </p>
      </div>
    </div>
  );
}
