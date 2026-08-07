import useCareerStore from "../../stores/useCareerStore";
import Tooltip from "../ui/Tooltip";

// Marcador de rivalidade ao lado do nome de um piloto — mesmo vocabulário da estrela
// (estreante) e da semente (rookie) do DriverStatusMarkers. O papel é relativo ao
// JOGADOR (por isso lê o store, não o payload do piloto): 💥 Nemesis, 🔥 Rival.
// Os 3 mostrados vêm de `playerInterests` (1 Nemesis + até 2 Rivais); o resto que o
// motor rastreia não recebe marcador.

/// Retorna { role: "nemesis"|"rival", label: string|null } para um driver_id, ou null.
export function rivalInterestOf(interests, driverId) {
  if (!interests || driverId == null) return null;
  if (interests.nemesis && interests.nemesis.driver_id === driverId) {
    return { role: "nemesis", label: interests.nemesis.label ?? null };
  }
  const r = Array.isArray(interests.rivais)
    ? interests.rivais.find((x) => x.driver_id === driverId)
    : null;
  if (r) return { role: "rival", label: r.label ?? null };
  return null;
}

/// Só o papel (compat).
export function rivalRole(interests, driverId) {
  return rivalInterestOf(interests, driverId)?.role ?? null;
}

export default function RivalMarker({ driverId, className = "" }) {
  const interests = useCareerStore((s) => s.playerInterests);
  const interest = rivalInterestOf(interests, driverId);
  if (!interest) return null;

  const isNemesis = interest.role === "nemesis";
  const roleName = isNemesis ? "Nemesis" : "Rival";
  // Tooltip com o nome da rivalidade quando houver ("Nemesis — A Revanche de Interlagos").
  const title = interest.label ? `${roleName} — ${interest.label}` : roleName;
  return (
    <Tooltip texto={title}>
      <span className={`shrink-0 text-xs ${className}`} aria-label={title}>
        {isNemesis ? "\u{1F4A5}" : "\u{1F525}"}
      </span>
    </Tooltip>
  );
}
