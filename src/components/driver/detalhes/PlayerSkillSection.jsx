// ── Dossiê de Habilidade do JOGADOR (atributos inferidos do desempenho real) ──

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";

import i18n from "../../../i18n/index.js";

const PLAYER_SKILL_LABELS = {
  skill: "driverDetail.attributes.skill",
  ritmo_classificacao: "driverDetail.attributes.ritmo_classificacao",
  racecraft: "driverDetail.attributes.racecraft",
  consistencia: "driverDetail.attributes.consistencia",
  habilidade_largada: "driverDetail.attributes.habilidade_largada",
  aggression: "driverDetail.attributes.aggression",
  fator_chuva: "driverDetail.playerSkill.labels.fator_chuva",
  adaptabilidade: "driverDetail.attributes.adaptabilidade",
  experiencia: "driverDetail.playerSkill.labels.experiencia",
  midia: "driverDetail.playerSkill.labels.midia",
};

function playerSkillToneHex(value) {
  const v = Number(value) || 0;
  if (v >= 85) return "#bc8cff"; // elite
  if (v >= 75) return "#3fb950"; // qualidade
  if (v >= 40) return "#58a6ff"; // médio
  if (v >= 26) return "#d29922"; // fraco
  return "#f85149"; // defeito
}

function playerSkillUnlockMessage(attr) {
  const n = attr.remaining;
  if (attr.unlock_kind === "wet_races") {
    return i18n.t("driverDetail.playerSkill.unlock.wetRaces", { count: n });
  }
  if (attr.unlock_kind === "seasons") {
    return i18n.t("driverDetail.playerSkill.unlock.seasons", { count: n });
  }
  if (attr.unlock_kind === "telemetry_races") {
    return i18n.t("driverDetail.playerSkill.unlock.telemetryRaces", { count: n });
  }
  return i18n.t("driverDetail.playerSkill.unlock.races", { count: n });
}

function PlayerSkillLockedRow({ attr }) {
  const { t } = useTranslation();
  const label = t(PLAYER_SKILL_LABELS[attr.key] || attr.key);
  const progress = attr.unlock_threshold > 0
    ? Math.min(100, (attr.sample_count / attr.unlock_threshold) * 100)
    : 0;

  return (
    <div className="grid gap-1.5 rounded-lg border border-white/[0.06] bg-black/10 px-3 py-2.5">
      <div className="flex items-center justify-between gap-3">
        <span className="flex items-center gap-2 text-xs font-semibold uppercase tracking-[0.14em] text-[#6e7681]">
          <span aria-hidden="true">🔒</span>
          {label}
        </span>
        <span className="text-[10px] font-mono text-[#6e7681]">
          {attr.sample_count}/{attr.unlock_threshold}
        </span>
      </div>
      <div className="h-1.5 overflow-hidden rounded-full bg-[#161b22]">
        <div className="h-full rounded-full bg-[#30363d]" style={{ width: `${progress}%` }} />
      </div>
      <span className="text-[11px] text-[#7d8590]">{playerSkillUnlockMessage(attr)}</span>
    </div>
  );
}

function PlayerSkillRow({ attr }) {
  const { t } = useTranslation();
  if (!attr.unlocked) return <PlayerSkillLockedRow attr={attr} />;

  const label = t(PLAYER_SKILL_LABELS[attr.key] || attr.key);
  const value = Number(attr.value) || 0;
  const color = playerSkillToneHex(value);
  const firming = attr.confidence < 0.5;

  return (
    <div className="grid gap-1.5">
      <div className="flex items-baseline justify-between gap-3">
        <span className="text-xs font-semibold uppercase tracking-[0.14em] text-[#c9d1d9]">
          {label}
        </span>
        <span className="flex items-baseline gap-2">
          {attr.tag ? (
            <span className="text-sm font-bold" style={{ color }}>
              {attr.tag}
            </span>
          ) : null}
          <span className="font-mono text-xs text-[#7d8590]">{value}</span>
        </span>
      </div>
      <div className="h-2 overflow-hidden rounded-full bg-[#21262d]">
        <div
          className="h-full rounded-full transition-all duration-500"
          style={{ width: `${value}%`, backgroundColor: color, opacity: firming ? 0.55 : 1 }}
        />
      </div>
      {firming ? (
        <span className="text-[10px] italic text-[#6e7681]">
          {t("driverDetail.playerSkill.firming")}
        </span>
      ) : null}
    </div>
  );
}

export function PlayerSkillSection({ SectionComponent, careerId }) {
  const { t } = useTranslation();
  const [dossier, setDossier] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  useEffect(() => {
    let active = true;

    async function fetchDossier() {
      if (!careerId) {
        if (active) {
          setLoading(false);
          setDossier(null);
        }
        return;
      }
      setLoading(true);
      setError("");
      try {
        const data = await invoke("get_player_dossier", { careerId });
        if (active) setDossier(data);
      } catch (fetchError) {
        if (active) {
          setError(
            typeof fetchError === "string"
              ? fetchError
              : fetchError?.toString?.() ?? t("driverDetail.playerSkill.loadError"),
          );
        }
      } finally {
        if (active) setLoading(false);
      }
    }

    fetchDossier();
    return () => {
      active = false;
    };
  }, [careerId]);

  return (
    <SectionComponent title={t("driverDetail.playerSkill.title")}>
      <div className="grid gap-4">
        <div className="rounded-xl border border-[#58a6ff]/18 bg-[#58a6ff]/[0.06] p-3 text-[11px] leading-relaxed text-[#8b949e]">
          {t("driverDetail.playerSkill.introPre")}<span className="text-[#e6edf3]">{t("driverDetail.playerSkill.introPerf")}</span>{t("driverDetail.playerSkill.introMid")}{" "}
          <span className="text-[#e6edf3]">{t("driverDetail.playerSkill.introNot")}</span>{t("driverDetail.playerSkill.introEnd")}
        </div>

        {loading ? (
          <div className="rounded-xl border border-white/[0.06] bg-black/10 p-4 text-sm text-[#7d8590]">
            {t("driverDetail.playerSkill.loading")}
          </div>
        ) : error ? (
          <div className="rounded-xl border border-[#f85149]/25 bg-[#f85149]/10 p-4 text-sm text-[#f85149]">
            {error}
          </div>
        ) : dossier ? (
          <>
            <div className="glass-light rounded-xl p-4">
              <div className="grid gap-4">
                {dossier.attributes.map((attr) => (
                  <PlayerSkillRow key={attr.key} attr={attr} />
                ))}
              </div>
            </div>
            <div className="text-[11px] text-[#6e7681]">
              {t("driverDetail.playerSkill.racesCount", { count: dossier.total_races })} ·{" "}
              {t("driverDetail.playerSkill.seasonsBase", { count: dossier.total_seasons })}
            </div>
          </>
        ) : (
          <div className="rounded-xl border border-white/[0.06] bg-black/10 p-4 text-sm text-[#7d8590]">
            {t("driverDetail.playerSkill.noHistory")}
          </div>
        )}
      </div>
    </SectionComponent>
  );
}
