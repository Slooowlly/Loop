import { useState, useEffect, useRef } from "react";
import { formatMoneyCompact } from "../../utils/formatters";
import { resume as resumeAudio, bidRise, auctionHammer } from "../../utils/sfx";
import TeamLogoMark from "../team/TeamLogoMark";

// Leilão de QUEBRA DE CONTRATO ao vivo (Fase 2b.3): um beat de contexto, e então
// duas torres sobem devagar (com som acompanhando) enquanto os times cobrem um ao
// outro, com o valor contando em cima. No fim, a palavra é do jogador: Sair ou Ficar.
//
// Fases: "intro" (contexto) → "bidding" (torres + som) → "decision" → "resolved".

const BID_STEP_MS = 1500; // devagar, pra criar expectativa
const TWEEN_MS = 1050;

// Conta um número em direção ao alvo (easeOutCubic) via requestAnimationFrame.
function useTween(target, duration) {
  const [val, setVal] = useState(0);
  const fromRef = useRef(0);
  useEffect(() => {
    const from = fromRef.current;
    if (Math.abs(from - target) < 1) {
      setVal(target);
      fromRef.current = target;
      return undefined;
    }
    let raf;
    let start = null;
    const tick = (t) => {
      if (start === null) start = t;
      const p = Math.min(1, (t - start) / duration);
      const eased = 1 - Math.pow(1 - p, 3);
      const v = from + (target - from) * eased;
      setVal(v);
      if (p < 1) raf = requestAnimationFrame(tick);
      else fromRef.current = target;
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [target, duration]);
  return val;
}

function Tower({ teamName, color, target, max, leading, dim }) {
  const val = useTween(target, TWEEN_MS);
  const pct = max > 0 ? Math.min(1, val / max) : 0;
  const heightPct = 6 + pct * 94;
  return (
    <div className={`flex flex-1 flex-col items-center gap-2 transition-opacity ${dim ? "opacity-45" : ""}`}>
      <div className="font-black tabular-nums text-xl leading-none" style={{ color }}>
        {val > 0 ? formatMoneyCompact(val) : "—"}
      </div>
      <div className="relative flex h-44 w-full items-end justify-center">
        <div
          className="w-16 rounded-t-lg"
          style={{
            height: `${heightPct}%`,
            background: `linear-gradient(to top, ${color}, ${color}bb)`,
            boxShadow: leading ? `0 0 26px ${color}` : `0 0 10px ${color}55`,
            transition: "box-shadow 0.4s",
          }}
        />
      </div>
      <TeamLogoMark teamName={teamName} color={color} size="md" />
      <div className="max-w-[130px] truncate text-center text-[11px] font-semibold text-gray-300">
        {teamName}
      </div>
    </div>
  );
}

export default function PoachAuctionModal({ offer, onDecide, onClose, isResolving }) {
  const [phase, setPhase] = useState("intro");
  const [step, setStep] = useState(0);
  const [outcome, setOutcome] = useState(null);
  const timerRef = useRef(null);

  const bids = offer?.bids ?? [];
  const suitorColor = offer?.suitor_color || "#f5c518";
  const holderColor = offer?.current_team_color || "#8b93a7";
  const maxBid = bids.reduce((m, b) => Math.max(m, b.salary), offer?.current_salary || 1);

  // Toca os lances um a um; ao fim, revela a decisão.
  useEffect(() => {
    if (phase !== "bidding") return undefined;
    if (step >= bids.length) {
      timerRef.current = setTimeout(() => setPhase("decision"), 650);
      return () => clearTimeout(timerRef.current);
    }
    timerRef.current = setTimeout(() => setStep((n) => n + 1), step === 0 ? 500 : BID_STEP_MS);
    return () => clearTimeout(timerRef.current);
  }, [phase, step, bids.length]);

  // Som: cada torre que sobe emite um glide de pitch acompanhando a coluna.
  useEffect(() => {
    if (phase !== "bidding" || step === 0 || step > bids.length) return;
    const bid = bids[step - 1];
    if (!bid) return;
    const prev = bids
      .slice(0, step - 1)
      .filter((b) => b.is_poacher === bid.is_poacher)
      .reduce((m, b) => Math.max(m, b.salary), 0);
    bidRise(prev / maxBid, bid.salary / maxBid, TWEEN_MS);
  }, [phase, step, bids, maxBid]);

  if (!offer) return null;

  const revealed = bids.slice(0, phase === "bidding" ? step : bids.length);
  const suitorTarget = revealed.filter((b) => b.is_poacher).reduce((m, b) => Math.max(m, b.salary), 0);
  const holderTarget = revealed.filter((b) => !b.is_poacher).reduce((m, b) => Math.max(m, b.salary), 0);
  const suitorLeads = suitorTarget >= holderTarget;

  const start = () => {
    resumeAudio(); // 1º gesto → destrava o áudio
    setStep(0);
    setPhase("bidding");
  };

  const handleDecide = async (accept) => {
    auctionHammer();
    try {
      const result = await onDecide(accept);
      setOutcome(result ?? { left: accept, applied: true });
      setPhase("resolved");
    } catch {
      /* erro tratado no store; mantém a tela */
    }
  };

  return (
    <div className="fixed inset-0 z-[60] flex items-center justify-center bg-black/85 backdrop-blur-sm p-4">
      <div
        className="w-full max-w-xl overflow-hidden rounded-2xl border border-amber-400/25 bg-[#0d1117] shadow-2xl"
        style={{ boxShadow: `0 0 70px ${suitorColor}1f, 0 20px 60px rgba(0,0,0,0.6)` }}
      >
        {/* Cabeçalho enxuto */}
        <div className="border-b border-white/10 bg-gradient-to-r from-amber-400/[0.07] to-transparent px-6 py-4 text-center">
          <div className="text-[10px] font-semibold uppercase tracking-[0.24em] text-amber-300/80">
            ◆ Quebra de contrato
          </div>
          <div className="mt-1 text-xl font-bold text-white">
            A <span style={{ color: suitorColor }}>{offer.suitor_name}</span> quer você
          </div>
          {phase === "bidding" && (
            <div className="mt-0.5 text-[12px] text-gray-400">os times estão brigando por você…</div>
          )}
          {phase === "decision" && (
            <div className="mt-0.5 text-[12px] text-gray-400">quem leva você?</div>
          )}
        </div>

        <div className="p-6">
          {/* Contexto antes da disputa */}
          {phase === "intro" && (
            <div className="flex flex-col items-center gap-4 py-2 text-center">
              <TeamLogoMark teamName={offer.suitor_name} color={suitorColor} size="lg" />
              <div className="text-[15px] text-gray-200">
                Quer te tirar da <span className="font-semibold text-white">{offer.current_team_name}</span>
              </div>
              <div className="inline-flex items-center gap-2 rounded-full border border-white/12 bg-white/5 px-3 py-1 text-[11px] text-gray-300">
                multa de rescisão
                <span className="font-bold tabular-nums text-white">{formatMoneyCompact(offer.buyout)}</span>
              </div>
              <button
                onClick={start}
                className="mt-1 rounded-lg border border-amber-400/40 bg-amber-400/15 px-6 py-2 font-semibold text-amber-300 transition hover:bg-amber-400/25"
              >
                Começar o leilão →
              </button>
            </div>
          )}

          {(phase === "bidding" || phase === "decision") && (
            <div className="flex items-end gap-6">
              <Tower
                teamName={offer.suitor_name}
                color={suitorColor}
                target={suitorTarget}
                max={maxBid}
                leading={phase === "bidding" && suitorLeads && suitorTarget > 0}
                dim={phase === "bidding" && !suitorLeads && suitorTarget > 0}
              />
              <Tower
                teamName={offer.current_team_name}
                color={holderColor}
                target={holderTarget}
                max={maxBid}
                leading={phase === "bidding" && !suitorLeads && holderTarget > 0}
                dim={phase === "bidding" && suitorLeads && holderTarget > 0}
              />
            </div>
          )}

          {/* Decisão */}
          {phase === "decision" && (
            <div className="mt-6 grid grid-cols-2 gap-3">
              <button
                disabled={isResolving}
                onClick={() => handleDecide(true)}
                className="rounded-xl border p-3 text-center transition hover:brightness-125 disabled:opacity-50"
                style={{ borderColor: suitorColor + "66", background: suitorColor + "16" }}
              >
                <div className="text-[10px] uppercase tracking-[0.18em] text-gray-400">Sair</div>
                <div className="mt-1 font-black tabular-nums text-lg" style={{ color: suitorColor }}>
                  {formatMoneyCompact(offer.poacher_best)}
                </div>
              </button>
              <button
                disabled={isResolving}
                onClick={() => handleDecide(false)}
                className="rounded-xl border p-3 text-center transition hover:brightness-125 disabled:opacity-50"
                style={{ borderColor: holderColor + "66", background: holderColor + "16" }}
              >
                <div className="text-[10px] uppercase tracking-[0.18em] text-gray-400">Ficar</div>
                <div className="mt-1 font-black tabular-nums text-lg" style={{ color: holderColor }}>
                  {formatMoneyCompact(Math.max(offer.holder_best, offer.current_salary))}
                </div>
              </button>
            </div>
          )}

          {/* Desfecho */}
          {phase === "resolved" && outcome && (
            <div className="flex flex-col items-center gap-3 py-3 text-center">
              <TeamLogoMark
                teamName={outcome.left ? offer.suitor_name : offer.current_team_name}
                color={outcome.left ? suitorColor : holderColor}
                size="lg"
              />
              <div className="text-lg font-bold text-white">
                {outcome.left ? offer.suitor_name : offer.current_team_name}
              </div>
              <div className="font-black tabular-nums text-2xl" style={{ color: outcome.left ? suitorColor : holderColor }}>
                {formatMoneyCompact(outcome.salary)}<span className="text-sm font-semibold text-gray-400">/ano</span>
              </div>
              <button
                onClick={onClose}
                className="mt-1 rounded-lg border border-white/20 bg-white/10 px-6 py-2 font-semibold text-white transition hover:bg-white/15"
              >
                Continuar
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
