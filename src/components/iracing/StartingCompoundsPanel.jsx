import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";

// Painel PRÉ-CORRIDA de "Composto de largada": lista, por classe, qual pneu cada carro
// (inclusive a IA) escolheu — SECO ou CHUVA — lido ao vivo do `CarIdxTireCompound` do
// iRacing (a mesma info que o RaceLab expõe). Fica no overlay "iRacing Conectado", então
// você espia na grid antes de largar. Enquanto a IA não escolhe, vem "—".
//
// O composto vem como ÍNDICE (0 = seco, ≥1 = chuva, -1 = desconhecido); nesta série só há
// esses dois. Reusa o mesmo dado da torre (`get_overlay_data`).

const IN_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

function compoundOf(tireCompound) {
  if (typeof tireCompound !== "number" || tireCompound < 0) return null; // desconhecido
  return tireCompound >= 1 ? "wet" : "dry";
}

function CompoundChip({ kind }) {
  const { t } = useTranslation();
  if (!kind) {
    return <span className="rounded px-1.5 py-0.5 text-[10px] font-bold text-text-muted">—</span>;
  }
  const wet = kind === "wet";
  return (
    <span
      className="inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[10px] font-bold"
      style={{
        background: wet ? "rgba(56,139,253,0.16)" : "rgba(245,158,11,0.16)",
        color: wet ? "#79c0ff" : "#f0b429",
      }}
    >
      <span aria-hidden>{wet ? "🌧" : "☀"}</span>
      {wet ? t("startingCompounds.wet") : t("startingCompounds.dry")}
    </span>
  );
}

export default function StartingCompoundsPanel({ careerId }) {
  const { t } = useTranslation();
  const [classes, setClasses] = useState(null);

  useEffect(() => {
    if (!IN_TAURI || !careerId) return undefined;
    let alive = true;
    const tick = async () => {
      try {
        const data = await invoke("get_overlay_data", { careerId });
        if (alive) setClasses(data?.classes ?? null);
      } catch {
        /* sem sessão / sem save — silencioso */
      }
    };
    tick();
    const id = setInterval(tick, 2000);
    return () => {
      alive = false;
      clearInterval(id);
    };
  }, [careerId]);

  // Só vale a pena mostrar quando ALGUÉM já escolheu um composto conhecido.
  const anyKnown =
    classes?.some((c) => c.cars.some((car) => compoundOf(car.tireCompound) != null)) ?? false;

  if (!classes || !anyKnown) return null;

  return (
    <div className="flex flex-col rounded-2xl border border-white/10 bg-[#0d131c]/80 p-3">
      <div className="mb-2 flex items-baseline justify-between">
        <p className="text-[11px] font-bold uppercase tracking-[0.14em] text-text-secondary">
          {t("startingCompounds.header")}
        </p>
        <span className="text-[10px] text-text-muted">{t("startingCompounds.liveFromGrid")}</span>
      </div>

      <div className="grid gap-3 md:grid-cols-2">
        {classes.map((cls) => (
          <div key={cls.id} className="min-w-0">
            <p className="mb-1 text-[10px] font-bold uppercase tracking-wider text-text-muted">
              {cls.label}
            </p>
            <div className="flex flex-col gap-0.5">
              {cls.cars.map((car, i) => (
                <div
                  key={`${car.name}-${i}`}
                  className="flex items-center justify-between gap-2 rounded px-1.5 py-0.5"
                  style={car.player ? { background: "rgba(63,185,80,0.12)" } : undefined}
                >
                  <span className="flex min-w-0 items-center gap-1.5">
                    <span className="w-5 shrink-0 text-right text-[10px] tabular-nums text-text-muted">
                      {car.pos > 0 ? car.pos : "—"}
                    </span>
                    <span
                      className="truncate text-[11px]"
                      style={{ color: car.player ? "#7ee787" : undefined }}
                    >
                      {car.name}
                    </span>
                  </span>
                  <CompoundChip kind={compoundOf(car.tireCompound)} />
                </div>
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
