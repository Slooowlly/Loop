/**
 * Estado global do auto-update. Um provider só, consumido por:
 *   • UpdateGate — o modal de "atualização disponível" (baixar/instalar)
 *   • UpdateChangelogModal — a tela "o que mudou" (sob demanda)
 *   • MainMenu — botão "verificar" ao lado da versão + 5 cliques = canal beta
 *
 * Máquina de status: idle → checking → available → downloading → installing
 *                     (ou → uptodate / error). O boot dispara 1 check estável.
 */
import {
  createContext,
  useContext,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import {
  checkChannel,
  downloadInstallRelaunch,
  isTauri,
} from "../../lib/updater";

const UpdaterContext = createContext(null);

export function useUpdater() {
  const ctx = useContext(UpdaterContext);
  if (!ctx) throw new Error("useUpdater fora do UpdaterProvider");
  return ctx;
}

export default function UpdaterProvider({ children }) {
  const [update, setUpdate] = useState(null); // objeto Update do plugin
  const [channel, setChannel] = useState("stable");
  const [status, setStatus] = useState("idle");
  const [progress, setProgress] = useState(null); // 0..1 | null (indeterminado)
  const [showChangelog, setShowChangelog] = useState(false);
  // Evita checks concorrentes (boot + clique do usuário ao mesmo tempo).
  const busy = useRef(false);

  const runCheck = useCallback(async (ch, { silent = false } = {}) => {
    if (busy.current || !isTauri()) return null;
    busy.current = true;
    if (!silent) setStatus("checking");
    try {
      const found = await checkChannel(ch);
      if (found) {
        setUpdate(found);
        setChannel(ch);
        setStatus("available");
      } else if (!silent) {
        // Só o fluxo manual mostra "já atualizado"; o boot fica quieto.
        setStatus("uptodate");
      } else {
        setStatus("idle");
      }
      return found;
    } finally {
      busy.current = false;
    }
  }, []);

  // Boot: 1 checagem estável, silenciosa (sem "já atualizado" nem spinner).
  useEffect(() => {
    void runCheck("stable", { silent: true });
  }, [runCheck]);

  const install = useCallback(async () => {
    if (!update) return;
    setStatus("downloading");
    setProgress(null);
    try {
      await downloadInstallRelaunch(update, (frac) => {
        setProgress(frac);
        if (frac === 1) setStatus("installing");
      });
      // relaunch() reinicia o app; daqui não volta.
    } catch (err) {
      console.error("[updater] instalação falhou:", err);
      setStatus("error");
    }
  }, [update]);

  const value = {
    update,
    channel,
    status,
    progress,
    showChangelog,
    // Ações
    checkStable: () => runCheck("stable"),
    checkBeta: () => runCheck("beta"),
    install,
    dismiss: () => setStatus("idle"),
    openChangelog: () => setShowChangelog(true),
    closeChangelog: () => setShowChangelog(false),
  };

  return (
    <UpdaterContext.Provider value={value}>
      {children}
    </UpdaterContext.Provider>
  );
}
