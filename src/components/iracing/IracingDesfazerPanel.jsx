import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

import useCareerStore from "../../stores/useCareerStore";

// Desfazer o que o Loop escreveu na pasta do iRacing.
//
// O app mexe em DOIS arquivos que não são dele, e nos dois sem perguntar: a pintura do
// carro (`paint/<carro>/car_<custid>.tga`) e a configuração gráfica (`rendererDX11*.ini`,
// o modo janela que o overlay exige). Não perguntar só se sustenta com caminho de volta,
// e este painel é esse caminho. Fica fora do Menu Debug de propósito: quem precisa dele é
// o jogador que quer a pasta dele de volta, não quem está calibrando o app.
//
// Sem preview. O backend responde o que fez DEPOIS de fazer, por carro, e essa lista é a
// prestação de contas — inventar um "vou mexer nestes arquivos" antes exigiria um segundo
// comando que leria o disco duas vezes para dizer a mesma coisa.

// Cor por desfecho. Só `preservada` puxa atenção, porque é o único em que o jogador
// continua com a cor da equipe no carro e precisa saber por quê.
const TOM_ESTADO = {
  restaurada: "text-status-green",
  removida: "text-status-green",
  preservada: "text-status-yellow",
  nada: "text-text-muted",
};

function IracingDesfazerPanel() {
  const { t } = useTranslation();
  // A carreira aberta, quando há uma. As Configurações também abrem a partir do menu
  // inicial, sem carreira nenhuma — e aí o backend cai no custid capturado na sessão do
  // iRacing, que é justamente o ID com que os arquivos foram nomeados.
  const careerId = useCareerStore((state) => state.careerId);

  const [janela, setJanela] = useState(null);
  const [janelaBusy, setJanelaBusy] = useState(false);
  const [janelaMsg, setJanelaMsg] = useState("");

  const [pinturas, setPinturas] = useState(null);
  const [pinturaBusy, setPinturaBusy] = useState(false);
  const [pinturaMsg, setPinturaMsg] = useState("");

  const lerStatusJanela = useCallback(async () => {
    try {
      setJanela(await invoke("iracing_modo_janela_status"));
    } catch {
      // Sem status não há botão habilitado, que é o estado seguro.
      setJanela(null);
    }
  }, []);

  useEffect(() => {
    lerStatusJanela();
  }, [lerStatusJanela]);

  async function desfazerJanela() {
    setJanelaBusy(true);
    setJanelaMsg("");
    try {
      const status = await invoke("iracing_modo_janela_restaurar");
      setJanela(status);
      setJanelaMsg(t("settings.desfazer.janela.pronto"));
    } catch (e) {
      setJanelaMsg(String(e));
      // Releitura: o erro pode ser exatamente "o simulador abriu no meio", e aí o botão
      // precisa refletir o disco em vez do estado com que a tela entrou.
      await lerStatusJanela();
    } finally {
      setJanelaBusy(false);
    }
  }

  async function desfazerPinturas() {
    setPinturaBusy(true);
    setPinturaMsg("");
    setPinturas(null);
    try {
      const linhas = await invoke("iracing_desfazer_pinturas", { careerId: careerId ?? "" });
      setPinturas(Array.isArray(linhas) ? linhas : []);
    } catch (e) {
      setPinturaMsg(String(e));
    } finally {
      setPinturaBusy(false);
    }
  }

  // `pode_desfazer` é o que diz se existe `.loop.bak` para devolver. Sem ele o Loop nunca
  // chegou a mudar nada ali, e um botão clicável mentiria. Com o simulador aberto o botão
  // também sai: o iRacing reescreve esses `.ini` ao fechar, então a escrita se perderia.
  const podeDesfazerJanela = Boolean(janela?.pode_desfazer) && !janela?.simulador_aberto;
  const janelaEstado = !janela?.pode_desfazer
    ? t("settings.desfazer.janela.nadaAFazer")
    : janela?.simulador_aberto
      ? t("settings.desfazer.janela.simAberto")
      : t("settings.desfazer.janela.disponivel");

  // A pintura NÃO é bloqueada com o simulador aberto, ao contrário do modo janela: o sim
  // lê os `.tga` ao carregar o carro numa sessão e nunca os reescreve ao fechar. A cor
  // volta na próxima vez que o carro for carregado.
  const preservouAlguma = (pinturas ?? []).some((linha) => linha.estado === "preservada");

  return (
    <div
      id="desfazer"
      style={{ scrollMarginTop: "1rem" }}
      className="border-t border-white/10 px-5 py-3.5"
    >
      <p className="text-[13px] font-medium text-text-primary">{t("settings.desfazer.title")}</p>
      <p className="text-[11px] text-text-secondary">{t("settings.desfazer.desc")}</p>

      {/* ── Modo janela (rendererDX11*.ini) ── */}
      <div className="mt-3 rounded-lg border border-white/10 bg-white/5 px-3 py-2.5">
        <div className="flex items-start justify-between gap-4">
          <div className="min-w-0 pr-2">
            <p className="text-[12px] font-medium text-text-primary">
              {t("settings.desfazer.janela.label")}
            </p>
            <p className="mt-0.5 text-[11px] leading-snug text-text-secondary">{janelaEstado}</p>
          </div>
          <button
            type="button"
            // Os dois botões da seção dizem "Desfazer". O nome acessível precisa dizer
            // desfazer O QUÊ, ou eles ficam indistinguíveis fora do contexto visual.
            aria-label={t("settings.desfazer.janela.acao")}
            onClick={desfazerJanela}
            disabled={!podeDesfazerJanela || janelaBusy}
            className={`shrink-0 rounded-lg px-4 py-2 text-[12px] font-semibold transition-glass ${
              !podeDesfazerJanela || janelaBusy
                ? "cursor-default bg-white/5 text-text-muted"
                : "cursor-pointer bg-white/10 text-text-primary hover:bg-white/20"
            }`}
          >
            {janelaBusy ? t("settings.desfazer.desfazendo") : t("settings.desfazer.desfazer")}
          </button>
        </div>
        {janelaMsg && (
          <p className="mt-2 rounded-lg border border-white/10 bg-white/5 px-3 py-2 font-mono text-[11px] text-text-secondary">
            {janelaMsg}
          </p>
        )}
      </div>

      {/* ── Pintura do carro (car_<custid>.tga) ── */}
      <div className="mt-2 rounded-lg border border-white/10 bg-white/5 px-3 py-2.5">
        <div className="flex items-start justify-between gap-4">
          <div className="min-w-0 pr-2">
            <p className="text-[12px] font-medium text-text-primary">
              {t("settings.desfazer.pintura.label")}
            </p>
            <p className="mt-0.5 text-[11px] leading-snug text-text-secondary">
              {t("settings.desfazer.pintura.desc")}
            </p>
          </div>
          <button
            type="button"
            aria-label={t("settings.desfazer.pintura.acao")}
            onClick={desfazerPinturas}
            disabled={pinturaBusy}
            className={`shrink-0 rounded-lg px-4 py-2 text-[12px] font-semibold transition-glass ${
              pinturaBusy
                ? "cursor-default bg-white/5 text-text-muted"
                : "cursor-pointer bg-white/10 text-text-primary hover:bg-white/20"
            }`}
          >
            {pinturaBusy ? t("settings.desfazer.desfazendo") : t("settings.desfazer.desfazer")}
          </button>
        </div>

        {pinturas != null && (
          <div className="mt-2 space-y-1">
            {pinturas.length === 0 ? (
              <p className="text-[11px] text-text-muted">{t("settings.desfazer.pintura.semCarros")}</p>
            ) : (
              pinturas.map((linha) => (
                <div key={linha.car_key} className="text-[11px]">
                  <div className="flex items-center justify-between gap-3">
                    <span className="font-mono uppercase tracking-[0.1em] text-text-muted">
                      {linha.car_key}
                    </span>
                    <span className={`font-semibold ${TOM_ESTADO[linha.estado] ?? "text-text-primary"}`}>
                      {t(`settings.desfazer.pintura.estado.${linha.estado}`)}
                    </span>
                  </div>
                  {linha.caminho && (
                    <p className="truncate font-mono text-[9px] text-text-muted">{linha.caminho}</p>
                  )}
                </div>
              ))
            )}
            {/* O único desfecho que precisa de explicação: o arquivo continua lá, e o
                jogador não deve concluir que o desfazer falhou. */}
            {preservouAlguma && (
              <p className="pt-1 text-[10px] leading-snug text-status-yellow">
                {t("settings.desfazer.pintura.preservadaNota")}
              </p>
            )}
          </div>
        )}

        {pinturaMsg && (
          <p className="mt-2 rounded-lg border border-white/10 bg-white/5 px-3 py-2 font-mono text-[11px] text-text-secondary">
            {pinturaMsg}
          </p>
        )}
      </div>
    </div>
  );
}

export default IracingDesfazerPanel;
