import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { estaNoTauri } from "../../lib/tauri";
import * as voz from "../../lib/engenheiroVoz";
import * as microfone from "../../lib/microfone";
import {
  lerGatilhoSalvo,
  lerMicSalvo,
  rotuloDoGatilho,
  salvarGatilho,
  salvarMic,
} from "../../lib/pttConfig";
import PttTestePanel from "./PttTestePanel";

// As três decisões do jogador sobre o push-to-talk: se o engenheiro responde, com que
// botão ele é chamado, e por qual microfone ele escuta.
//
// A associação do botão tem DUAS capturas porque as duas portas são diferentes por
// natureza. Tecla gera `keydown` no webview e se captura por evento. Botão de volante é
// HID: não gera evento nenhum aqui dentro — foi exatamente por isso que associá-lo não
// funcionava no recentro de VR —, então a captura é POLL ao backend perguntando qual
// botão está apertado agora.

/// De quanto em quanto se pergunta ao backend qual botão está apertado, enquanto captura.
const POLL_CAPTURA_MS = 80;
/// Ritmo do ponto que acende enquanto o gatilho está sendo segurado. É a confirmação de
/// que a associação pegou — sem ele, "apertei e não aconteceu nada" não distingue botão
/// errado de encanamento quebrado.
const POLL_TESTE_MS = 100;

export default function PttEngenheiroSettings() {
  const { t } = useTranslation();
  const [ligada, setLigada] = useState(() => voz.estaLigada());
  const [gatilho, setGatilho] = useState(() => lerGatilhoSalvo());
  const [capturandoTecla, setCapturandoTecla] = useState(false);
  const [capturandoBotao, setCapturandoBotao] = useState(false);
  const [volantes, setVolantes] = useState(null);
  const [segurando, setSegurando] = useState(false);
  const [mics, setMics] = useState([]);
  const [micErro, setMicErro] = useState("");
  const [micAtual, setMicAtual] = useState(() => microfone.retratoAtual());
  // O que está ESCOLHIDO (persistido), que não é o mesmo que o que está ABERTO: o
  // dispositivo pode ter sumido e a captura ter caído no padrão. O `select` mostra a
  // escolha; a linha de baixo conta o que de fato abriu.
  const [micEscolhido, setMicEscolhido] = useState(() => lerMicSalvo() || "");
  const [mostrarTeste, setMostrarTeste] = useState(false);
  const capturandoRef = useRef(false);

  const aplicar = useCallback((g) => {
    const limpo = salvarGatilho(g);
    setGatilho(limpo);
    if (estaNoTauri()) invoke("ptt_set_gatilho", { gatilho: limpo }).catch(() => {});
  }, []);

  // ── Captura de TECLA: o próximo `keydown` vira o gatilho. Escape desiste.
  useEffect(() => {
    if (!capturandoTecla) return undefined;
    const aoTeclar = (e) => {
      e.preventDefault();
      if (e.key === "Escape") {
        setCapturandoTecla(false);
        return;
      }
      const codigo = e.keyCode; // Virtual-Key do Windows, que é o que o Rust consulta
      if (!codigo) return;
      setCapturandoTecla(false);
      aplicar({ tipo: "tecla", codigo });
    };
    window.addEventListener("keydown", aoTeclar, true);
    return () => window.removeEventListener("keydown", aoTeclar, true);
  }, [capturandoTecla, aplicar]);

  // ── Captura de BOTÃO DE VOLANTE: poll, porque não há evento a ouvir.
  useEffect(() => {
    capturandoRef.current = capturandoBotao;
    if (!capturandoBotao || !estaNoTauri()) return undefined;
    // "Nenhum dispositivo" é um diagnóstico bem diferente de "apertei e não pegou".
    invoke("volante_dispositivos")
      .then((ds) => setVolantes(ds?.length ?? 0))
      .catch(() => setVolantes(0));
    const timer = setInterval(async () => {
      try {
        const b = await invoke("volante_botao_pressionado");
        if (!b || !capturandoRef.current) return;
        setCapturandoBotao(false);
        aplicar({ tipo: "volante", dispositivo: b.dispositivo, botao: b.botao });
      } catch {
        /* backend ainda não pronto */
      }
    }, POLL_CAPTURA_MS);
    return () => clearInterval(timer);
  }, [capturandoBotao, aplicar]);

  // ── Ponto de teste: acende enquanto o gatilho associado estiver apertado.
  useEffect(() => {
    if (!gatilho || !estaNoTauri() || capturandoBotao) return undefined;
    const timer = setInterval(async () => {
      try {
        setSegurando(Boolean(await invoke("ptt_esta_apertado")));
      } catch {
        setSegurando(false);
      }
    }, POLL_TESTE_MS);
    return () => clearInterval(timer);
  }, [gatilho, capturandoBotao]);

  // ── Microfones. Só a LISTA aqui — abrir o microfone acende o indicador do Windows, e
  // entrar nas configurações não é motivo para isso. Quem abre é o painel de teste, que
  // é onde o jogador pediu para escutar. Os rótulos dependem de uma permissão já
  // concedida; sem ela a lista vem sem nome e o teste resolve na primeira abertura.
  useEffect(() => {
    let parado = false;
    microfone
      .dispositivos()
      .then((l) => !parado && setMics(l))
      .catch(() => {});
    return () => {
      parado = true;
    };
  }, [mostrarTeste]);

  // A escolha é PERSISTIDA antes de ser aplicada. Quem abre o microfone na corrida é o
  // `EngenheiroPttAuto`, que não vê este componente — sem gravar, a troca valia só
  // enquanto esta tela estivesse aberta.
  const trocarMic = async (deviceId) => {
    setMicErro("");
    const id = salvarMic(deviceId || null);
    setMicEscolhido(id || "");
    try {
      setMicAtual(await microfone.armar({ deviceId: id }));
    } catch (e) {
      setMicErro(String(e?.message || e));
    }
    // Segundo aviso, DEPOIS de abrir. O `salvarMic` avisa que a escolha mudou; este avisa
    // que ela já valeu — o painel de teste precisa do segundo, senão leria o retrato do
    // microfone anterior e mostraria o dispositivo antigo com o medidor do novo.
    window.dispatchEvent(new Event("loop:ptt-microfone"));
  };

  const linha = "flex items-center justify-between gap-4 border-t border-white/10 px-5 py-3.5";
  const botao =
    "shrink-0 cursor-pointer rounded-lg bg-white/10 px-4 py-2 text-[12px] font-semibold text-text-primary transition-glass hover:bg-white/20";

  return (
    <>
      {/* Liga/desliga + ouvir. Toca a desistência de propósito: é a frase mais curta do
          acervo e a única que soa igual em qualquer situação de corrida. */}
      <div className={linha}>
        <div className="min-w-0 flex-1 cursor-pointer" onClick={() => alternar()}>
          <p className="text-[13px] font-medium text-text-primary">
            {t("settings.engenheiroPtt.label")}
          </p>
          <p className="text-[11px] text-text-secondary">
            {ligada ? t("settings.engenheiroPtt.on") : t("settings.engenheiroPtt.off")}
          </p>
        </div>
        <button type="button" onClick={() => voz.falarPecas(["nao_sei"])} className={botao}>
          {t("settings.engenheiroPtt.ouvir")}
        </button>
        <div
          className={`h-6 w-11 shrink-0 cursor-pointer rounded-full p-1 transition-all ${ligada ? "bg-accent-primary" : "bg-white/10"}`}
          onClick={() => alternar()}
        >
          <div
            className={`h-4 w-4 rounded-full bg-white transition-all ${ligada ? "translate-x-5" : "translate-x-0"}`}
          />
        </div>
      </div>

      {/* O gatilho. */}
      <div className={linha}>
        <div className="min-w-0 flex-1">
          <p className="flex items-center gap-2 text-[13px] font-medium text-text-primary">
            {t("settings.engenheiroPtt.gatilhoLabel")}
            {segurando ? (
              <span className="h-2 w-2 animate-pulse rounded-full bg-accent-primary" />
            ) : null}
          </p>
          <p className="text-[11px] text-text-secondary">
            {capturandoTecla
              ? t("settings.engenheiroPtt.pressioneTecla")
              : capturandoBotao
                ? t("settings.engenheiroPtt.pressioneBotao", { n: volantes ?? 0 })
                : gatilho
                  ? t("settings.engenheiroPtt.gatilhoAtual", { gatilho: rotuloDoGatilho(gatilho) })
                  : t("settings.engenheiroPtt.semGatilho")}
          </p>
        </div>
        <button
          type="button"
          onClick={() => {
            setCapturandoBotao(false);
            setCapturandoTecla((v) => !v);
          }}
          className={botao}
        >
          {t("settings.engenheiroPtt.capturarTecla")}
        </button>
        <button
          type="button"
          onClick={() => {
            setCapturandoTecla(false);
            setCapturandoBotao((v) => !v);
          }}
          className={botao}
        >
          {t("settings.engenheiroPtt.capturarBotao")}
        </button>
        {gatilho ? (
          <button type="button" onClick={() => aplicar(null)} className={botao}>
            {t("settings.engenheiroPtt.limpar")}
          </button>
        ) : null}
      </div>

      {/* O microfone. */}
      <div className={linha}>
        <div className="min-w-0 flex-1">
          <p className="text-[13px] font-medium text-text-primary">
            {t("settings.engenheiroPtt.microfoneLabel")}
          </p>
          <p className="text-[11px] text-text-secondary">
            {micErro ||
              (micAtual?.caiuNoPadrao
                ? t("settings.engenheiroPtt.microfoneSumiu", { rotulo: micAtual.rotulo })
                : micAtual?.rotulo || t("settings.engenheiroPtt.microfoneNaoAberto"))}
          </p>
        </div>
        <select
          value={micEscolhido}
          onChange={(e) => trocarMic(e.target.value)}
          className="w-[200px] shrink-0 rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-[12px] text-text-primary"
        >
          <option value="">{t("settings.engenheiroPtt.microfonePadrao")}</option>
          {mics.map((d) => (
            <option key={d.id} value={d.id}>
              {d.rotulo || d.id.slice(0, 16)}
            </option>
          ))}
        </select>
      </div>

      {/* A bancada. Fechada por padrão porque abre o microfone — e porque as três
          perguntas que ela responde só se fazem uma vez, ao montar o rig. */}
      <div className={linha}>
        <div className="min-w-0 flex-1">
          <p className="text-[13px] font-medium text-text-primary">
            {t("settings.engenheiroPtt.teste.label")}
          </p>
          <p className="text-[11px] text-text-secondary">{t("settings.engenheiroPtt.teste.desc")}</p>
        </div>
        <button type="button" onClick={() => setMostrarTeste((v) => !v)} className={botao}>
          {mostrarTeste ? t("settings.engenheiroPtt.teste.fechar") : t("settings.engenheiroPtt.teste.abrir")}
        </button>
      </div>
      {mostrarTeste ? <PttTestePanel /> : null}
    </>
  );

  function alternar() {
    const proximo = !ligada;
    setLigada(proximo);
    voz.ligar(proximo);
  }
}
