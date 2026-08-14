/**
 * Aviso de consentimento da telemetria de produto.
 *
 * Aparece UMA vez, quando `telemetry_enabled` está `null` no config — que é o estado
 * "nunca perguntei", distinto de `false` ("perguntei e ele recusou"). Responder grava
 * um booleano explícito, e o aviso não volta.
 *
 * Três escolhas deliberadas:
 *
 * 1. **Pergunta depois da primeira corrida**, dirigida no iRacing OU simulada dentro
 *    do app, e não na primeira abertura. No dia 1 o jogador não sabe o que é o Loop; a
 *    pergunta chega a um desconhecido e a resposta é chute. Reabertura de corrida
 *    antiga pela Home não conta: ela não gera evento nenhum, então perguntar ali é
 *    interromper por nada. Quem separa os três casos é o `lastRaceOrigem`, marcado no
 *    store por quem abre a tela de resultado.
 *
 *    A corrida simulada entrou nesta lista quando o `race_sim` passou a existir. Antes
 *    dela, quem jogava só simulando não produzia evento algum, e perguntar a ele era
 *    interromper por nada — era também o motivo de uma temporada inteira simulada ser
 *    invisível do lado do servidor. Ver `docs/telemetry-endpoint.md`.
 * 2. **Abre quando ele FECHA a tela de resultado**, não quando ela aparece. O
 *    resultado é a recompensa da corrida; cobrir isso com uma caixa de diálogo seria
 *    pior do que ter perguntado no boot.
 * 3. **Não fecha por clique no fundo nem tem "X".** Fechar sem responder deixaria o
 *    config em `null` e traria o aviso de volta na próxima corrida — insistir é pior
 *    que perguntar uma vez. Os dois botões custam o mesmo clique, então recusar é tão
 *    fácil quanto aceitar.
 *
 * O config é lido no momento do gatilho, não no boot: assim quem já respondeu pelas
 * Configurações antes de correr não é perguntado de novo.
 *
 * z-120: abaixo do UpdateGate (130). Se as duas aparecerem, atualizar vem primeiro.
 */
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import useCareerStore from "../../stores/useCareerStore";

// Respiro entre fechar o resultado e a pergunta aparecer, pra não emendar uma tela
// na outra no mesmo quadro.
const ENTRY_DELAY_MS = 600;

export default function TelemetryConsentGate() {
  const { t } = useTranslation();
  const showResult = useCareerStore((state) => state.showResult);
  const origem = useCareerStore((state) => state.lastRaceOrigem);
  const wasShowingResult = useRef(showResult);
  const [config, setConfig] = useState(null);
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);

  // Espelho em ref, lido dentro do efeito. Não pode entrar nas dependências: logo
  // depois de fechar o resultado, o `dismissResult` recarrega a carreira e zera o
  // campo — se o efeito re-rodasse por causa disso, o cleanup cancelaria o timer e a
  // pergunta nunca apareceria. O que vale é o valor no instante em que a tela fechou.
  const origemAgora = useRef(origem);
  origemAgora.current = origem;

  useEffect(() => {
    const justClosedResult = wasShowingResult.current && !showResult;
    wasShowingResult.current = showResult;
    // Corrida que GEROU evento, dirigida ou simulada. Reabertura de corrida antiga
    // não gera nada, então perguntar a quem só fez isso é interromper por nada.
    const gerouEvento = ["iracing", "simulada"].includes(origemAgora.current);
    if (!justClosedResult || !gerouEvento) return undefined;

    let alive = true;
    let timer;
    (async () => {
      try {
        const cfg = await invoke("get_config");
        // `== null` de propósito: pega null E undefined (config antigo, sem o campo).
        if (!alive || cfg?.telemetry_enabled != null) return;
        setConfig(cfg);
        timer = setTimeout(() => alive && setOpen(true), ENTRY_DELAY_MS);
      } catch {
        // Sem config não há o que perguntar — e um erro aqui não pode travar o jogo.
      }
    })();
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [showResult]);

  if (!open || !config) return null;

  async function answer(enabled) {
    if (busy) return;
    setBusy(true);
    try {
      await invoke("update_config", { newConfig: { ...config, telemetry_enabled: enabled } });
    } catch (err) {
      // Falhou ao gravar: fecha do mesmo jeito. O config segue `null` (= desligado, nada
      // é enviado) e a pergunta volta na próxima corrida. Insistir agora seria pior.
      console.error("Falha ao salvar a resposta de telemetria:", err);
    } finally {
      setOpen(false);
    }
  }

  return (
    <div className="fixed inset-0 z-[120] flex items-center justify-center">
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" />
      <div className="glass-strong relative w-[420px] rounded-2xl border border-white/[0.12] p-6 shadow-2xl">
        <h3 className="mb-2 text-[15px] font-semibold text-text-primary">
          {t("telemetryConsent.title")}
        </h3>
        <p className="mb-4 text-[13px] leading-relaxed text-text-secondary">
          {t("telemetryConsent.body")}
        </p>

        {/* Rótulo separado do conteúdo: com os dois na mesma cor e peso, o item virava
            um parágrafo e a lista perdia a função. O rótulo usa o mesmo estilo dos
            cabeçalhos de seção das Configurações. */}
        <div className="mb-4 flex flex-col gap-3 rounded-xl border border-white/[0.08] bg-white/[0.04] p-3.5">
          {["sends", "never"].map((key) => (
            <div key={key}>
              <p
                className={`mb-1 text-[10px] font-semibold uppercase tracking-[0.14em] ${
                  key === "sends" ? "text-accent-primary" : "text-text-muted"
                }`}
              >
                {t(`telemetryConsent.${key}Label`)}
              </p>
              <p className="text-[12px] leading-relaxed text-text-secondary">
                {t(`telemetryConsent.${key}`)}
              </p>
            </div>
          ))}
        </div>

        <p className="mb-5 text-[11px] leading-relaxed text-text-secondary">
          {t("telemetryConsent.reversible")}
        </p>

        <div className="flex gap-2">
          <button
            type="button"
            disabled={busy}
            onClick={() => answer(false)}
            className="flex h-9 flex-1 items-center justify-center rounded-xl bg-white/10 text-[13px] font-medium text-text-primary transition-opacity hover:opacity-80 disabled:opacity-50"
          >
            {t("telemetryConsent.decline")}
          </button>
          <button
            type="button"
            disabled={busy}
            onClick={() => answer(true)}
            className="flex h-9 flex-1 items-center justify-center rounded-xl bg-accent-primary text-[13px] font-medium text-white transition-opacity hover:opacity-90 disabled:opacity-50"
          >
            {t("telemetryConsent.accept")}
          </button>
        </div>
      </div>
    </div>
  );
}
