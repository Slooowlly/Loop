// Versão do calendário: alterna entre o legado (CalendarTab), o redesign V2
// (CalendarTabRedesignV2, congelado) e o V3 (CalendarTabRedesign, o mais novo).
// O V3 é o padrão; as versões anteriores ficam acessíveis pelo Menu Debug como
// fallback. Persiste em localStorage e avisa via evento pra Dashboard e Settings
// reagirem sem recarregar.
import { useCallback, useEffect, useState } from "react";

const STORAGE_KEY = "iracer_calendar_version";
const LEGACY_BOOL_KEY = "iracer_calendar_v2"; // chave antiga (bool) — migração
const CHANGE_EVENT = "iracer-calendar-version-changed";

export const CALENDAR_VERSIONS = ["legacy", "v2", "v3"];
const DEFAULT_VERSION = "v3";

export function getCalendarVersion() {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw && CALENDAR_VERSIONS.includes(raw)) return raw;
    // Migração da chave booleana antiga: false = legado, ausente/true = redesign.
    const legacyBool = localStorage.getItem(LEGACY_BOOL_KEY);
    if (legacyBool === "false") return "legacy";
    return DEFAULT_VERSION;
  } catch {
    return DEFAULT_VERSION;
  }
}

export function setCalendarVersion(version) {
  const next = CALENDAR_VERSIONS.includes(version) ? version : DEFAULT_VERSION;
  try {
    localStorage.setItem(STORAGE_KEY, next);
  } catch {
    // Ambiente sem localStorage: mantém só o efeito em memória via evento.
  }
  window.dispatchEvent(new CustomEvent(CHANGE_EVENT, { detail: next }));
}

export function useCalendarVersion() {
  const [version, setVersion] = useState(getCalendarVersion);

  useEffect(() => {
    const handler = () => setVersion(getCalendarVersion());
    window.addEventListener(CHANGE_EVENT, handler);
    return () => window.removeEventListener(CHANGE_EVENT, handler);
  }, []);

  const update = useCallback((next) => setCalendarVersion(next), []);

  return [version, update];
}
