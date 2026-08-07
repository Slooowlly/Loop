import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { estaNoTauri } from "../../lib/tauri";
import * as microfone from "../../lib/microfone";
import * as voz from "../../lib/engenheiroVoz";
import { criarOrquestrador } from "../../lib/pttEngenheiro";
import { entrarEmTeste, lerMicSalvo, sairDeTeste } from "../../lib/pttConfig";
import useCareerStore from "../../stores/useCareerStore";

// A bancada do push-to-talk dentro das configurações.
//
// Responde três perguntas que só o hardware do jogador responde, e responde as três SEM
// o iRacing aberto — que é o ponto: descobrir que o botão não é lido no meio de uma
// corrida é descobrir tarde demais.
//
//  1. **O volante é visto?** Varre os dispositivos e mostra qual botão está apertado
//     agora, sem precisar associar nada. Distingue as três falhas que de fora parecem a
//     mesma: nenhum dispositivo no Windows, dispositivo visto mas botão não reconhecido,
//     e botão reconhecido mas associado a outra coisa.
//  2. **O microfone escuta?** Medidor ao vivo. Um pico parado em zero com voz audível é
//     microfone mudo no hardware — indistinguível, sem isto, de um encanamento quebrado.
//  3. **O botão abre o microfone?** Segure o botão associado e fale: ao soltar, o que foi
//     capturado toca de volta. É a prova de ponta a ponta, e é a única das três que
//     depende das outras duas estarem certas.
//
// Enquanto este painel está aberto o orquestrador de verdade fica suspenso
// (`entrarEmTeste`), senão o mesmo botão dispararia uma pergunta ao engenheiro por cima
// do teste.

/// Ritmo da varredura de botões. 80 ms é o mesmo da captura e pega um toque curto.
const POLL_VOLANTE_MS = 80;

/// Dossiê de mentira, para quando não há corrida nenhuma aberta.
///
/// Existe por causa de um curto-circuito na primeira linha do renderizador: sem
/// telemetria, TODA pergunta vira "Estou sem dados do carro" antes de qualquer
/// roteamento. Sem estes fatos não haveria como chegar ao modelo com o simulador fechado
/// — e é justamente com ele fechado que se quer medir a latência, sem uma corrida
/// rodando no meio.
///
/// Os números estão escritos como se fala, igual aos fatos de verdade: é isso que o
/// prompt manda o modelo COPIAR em vez de converter, e um dossiê de teste com "1,2 s"
/// mediria um prompt que não é o de produção.
const LINHAS_DEMO = [
  "Você está em quinto de vinte e quatro.",
  "O carro da frente está a um e dois.",
  "Você está oito décimos por volta mais rápido que ele.",
  "O pneu dele é três voltas mais velho que o seu.",
  "Faltam nove voltas.",
];

export default function PttTestePanel() {
  const { t } = useTranslation();
  const [dispositivos, setDispositivos] = useState(null);
  const [botaoAgora, setBotaoAgora] = useState(null);
  const [micRetrato, setMicRetrato] = useState(null);
  const [micErro, setMicErro] = useState("");
  const [nivel, setNivel] = useState(0);
  const [pico, setPico] = useState(0);
  const [gravando, setGravando] = useState(false);
  const [ultima, setUltima] = useState(null);
  const urlRef = useRef("");
  const picoRef = useRef(0);
  const gravandoRef = useRef(false);
  // "eco" = grava e toca de volta, prova o hardware. "cadeia" = o encanamento de verdade.
  const [modo, setModo] = useState("eco");
  const modoRef = useRef("eco");
  const [rastro, setRastro] = useState([]);
  const [pergunta, setPergunta] = useState("");
  const [forcar, setForcar] = useState(false);
  const forcarRef = useRef(false);
  const orqRef = useRef(null);
  // Por REF: o orquestrador nasce uma vez e a carreira pode carregar depois dele.
  const careerId = useCareerStore((s) => s.careerId);
  const careerIdRef = useRef(careerId);
  careerIdRef.current = careerId;

  // Uma instância PRÓPRIA do orquestrador, e não a do `EngenheiroPttAuto`: o painel
  // suspende aquela ao abrir (`entrarEmTeste`) para o mesmo botão não disparar duas
  // perguntas. É o mesmo código rodando — o que muda é quem escuta o rastro.
  if (!orqRef.current) {
    orqRef.current = criarOrquestrador({
      microfone,
      voz,
      invoke,
      forcarModelo: () => forcarRef.current,
      linhasDemo: () => LINHAS_DEMO,
      careerId: () => careerIdRef.current || "",
    });
  }

  useEffect(() => {
    modoRef.current = modo;
    setRastro([]);
  }, [modo]);

  // O orquestrador é criado uma vez e lê o interruptor por função, não por valor: uma
  // cópia capturada na criação ficaria congelada no `false` para sempre.
  useEffect(() => {
    forcarRef.current = forcar;
  }, [forcar]);

  useEffect(() => orqRef.current.aoPasso((p) => setRastro((r) => [...r, p])), []);

  // Suspende o orquestrador enquanto o painel vive, e devolve o botão a ele na saída.
  useEffect(() => {
    entrarEmTeste();
    return () => sairDeTeste();
  }, []);

  // ── 1. O volante.
  useEffect(() => {
    if (!estaNoTauri()) return undefined;
    invoke("volante_dispositivos")
      .then((ds) => setDispositivos(ds ?? []))
      .catch(() => setDispositivos([]));
    const timer = setInterval(async () => {
      try {
        setBotaoAgora(await invoke("volante_botao_pressionado"));
      } catch {
        setBotaoAgora(null);
      }
    }, POLL_VOLANTE_MS);
    return () => clearInterval(timer);
  }, []);

  // ── 2. O microfone. Abre ao entrar no painel: é exatamente o que o jogador veio fazer.
  //
  // Com o dispositivo ESCOLHIDO, e não o padrão. Abrir sem argumento aqui era o defeito:
  // o painel reabria no padrão do Windows por cima da escolha feita na linha acima, e
  // mostrava o áudio virtual do headset com o pico parado em zero.
  useEffect(() => {
    let parado = false;
    microfone
      .armar({ deviceId: lerMicSalvo() })
      .then((r) => !parado && setMicRetrato(r))
      .catch((e) => !parado && setMicErro(String(e?.message || e)));
    return () => {
      parado = true;
      if (urlRef.current) URL.revokeObjectURL(urlRef.current);
    };
  }, []);

  // O microfone pode ser trocado na linha acima enquanto o painel está aberto. Há UM
  // microfone, e o módulo é a fonte — copiá-lo no `useState` foi como o painel passou a
  // mostrar um dispositivo e a medir outro.
  useEffect(() => {
    const aoTrocar = () => {
      const r = microfone.retratoAtual();
      if (r) setMicRetrato(r);
    };
    window.addEventListener("loop:ptt-microfone", aoTrocar);
    return () => window.removeEventListener("loop:ptt-microfone", aoTrocar);
  }, []);

  useEffect(() => {
    if (!micRetrato) return undefined;
    let vivo = true;
    const quadro = () => {
      if (!vivo) return;
      const v = microfone.nivel();
      setNivel(v);
      if (v > picoRef.current) {
        picoRef.current = v;
        setPico(v);
      }
      requestAnimationFrame(quadro);
    };
    requestAnimationFrame(quadro);
    return () => {
      vivo = false;
    };
  }, [micRetrato]);

  // ── 3. Botão + microfone juntos. Eco local: grava enquanto o botão está apertado e
  // toca de volta ao soltar. Sem servidor nenhum no meio — o que se está provando aqui é
  // a ponta do hardware, e um erro de rede só embaralharia o diagnóstico.
  useEffect(() => {
    if (!estaNoTauri()) return undefined;
    const limpezas = [];
    // `listen` é assíncrono e a limpeza do efeito é síncrona: sem esta bandeira, a
    // desmontagem roda ANTES de o ouvinte existir, `limpezas` está vazio, e o ouvinte
    // nasce órfão — vivo e sem ninguém para removê-lo. Em dev o StrictMode monta duas
    // vezes de propósito, então cada toque no botão chegava em dobro: dois `apertar`,
    // dois `soltar`, e um deles registrado como toque acidental.
    let morto = false;
    const registrar = (fn) => {
      if (morto) fn();
      else limpezas.push(fn);
    };
    (async () => {
      registrar(
        await listen("ptt-apertou", async () => {
          if (modoRef.current === "cadeia") {
            setRastro([]);
            setGravando(true);
            orqRef.current.apertar();
            return;
          }
          if (gravandoRef.current || !microfone.estaArmado()) return;
          picoRef.current = 0;
          setPico(0);
          try {
            await microfone.comecar();
            gravandoRef.current = true;
            setGravando(true);
          } catch (e) {
            setMicErro(String(e?.message || e));
          }
        }),
      );
      registrar(
        await listen("ptt-soltou", async () => {
          if (modoRef.current === "cadeia") {
            setGravando(false);
            orqRef.current.soltar();
            return;
          }
          if (!gravandoRef.current) return;
          gravandoRef.current = false;
          setGravando(false);
          const r = await microfone.terminar();
          if (!r) return;
          if (urlRef.current) URL.revokeObjectURL(urlRef.current);
          urlRef.current = URL.createObjectURL(new Blob([r.bytes], { type: r.mime }));
          setUltima({
            duracaoMs: r.duracaoMs,
            bytes: r.bytes.length,
            curta: r.curtaDemais,
            pico: picoRef.current,
            url: urlRef.current,
          });
        }),
      );
    })();
    return () => {
      morto = true;
      limpezas.forEach((fn) => fn && fn());
    };
  }, []);

  // A última pergunta foi respondida pelo curto-circuito de telemetria? É o caso que
  // parece defeito e não é — e o único remédio útil está a um clique dali.
  const semTelemetria = rastro.some(
    (p) => p.nome === "roteou" && String(p.detalhe).includes("sem_telemetria"),
  );

  const rotuloBotao = botaoAgora
    ? t("settings.engenheiroPtt.teste.botaoAgora", {
        d: botaoAgora.dispositivo,
        b: botaoAgora.botao + 1,
      })
    : t("settings.engenheiroPtt.teste.nenhumBotao");

  return (
    <div className="border-t border-white/10 bg-black/20 px-5 py-4">
      {/* 1. Volante */}
      <Item
        titulo={t("settings.engenheiroPtt.teste.volanteTitulo")}
        ok={Boolean(botaoAgora)}
        neutro={dispositivos === null}
      >
        {dispositivos === null
          ? t("settings.engenheiroPtt.teste.procurando")
          : dispositivos.length === 0
            ? t("settings.engenheiroPtt.teste.semDispositivo")
            : `${t("settings.engenheiroPtt.teste.dispositivos", { n: dispositivos.length })} · ${rotuloBotao}`}
      </Item>

      {/* 2. Microfone */}
      <Item
        titulo={t("settings.engenheiroPtt.teste.microfoneTitulo")}
        ok={pico > 0.01}
        neutro={!micRetrato && !micErro}
      >
        {micErro ||
          (micRetrato?.caiuNoPadrao
            ? t("settings.engenheiroPtt.microfoneSumiu", { rotulo: micRetrato.rotulo })
            : micRetrato?.rotulo || t("settings.engenheiroPtt.teste.abrindo"))}
        {micRetrato ? (
          <>
            <div className="mt-2 h-2 w-full overflow-hidden rounded-full bg-white/10">
              <div
                className="h-full transition-[width] duration-75"
                style={{
                  width: `${Math.min(100, nivel * 320)}%`,
                  background: nivel > 0.25 ? "#f0883e" : "#3fb950",
                }}
              />
            </div>
            <span className="mt-1 block tabular-nums text-[10px] text-text-secondary">
              {t("settings.engenheiroPtt.teste.pico", { v: pico.toFixed(3) })}
            </span>
          </>
        ) : null}
      </Item>

      {/* 3. Os dois juntos, em dois modos. */}
      <Item
        titulo={t("settings.engenheiroPtt.teste.juntosTitulo")}
        ok={modo === "eco" ? Boolean(ultima && !ultima.curta && ultima.pico > 0.01) : rastro.some((p) => p.nome === "falou")}
        neutro={!ultima && !gravando && rastro.length === 0}
      >
        <div className="mb-2 flex gap-1">
          {["eco", "cadeia"].map((m) => (
            <button
              key={m}
              type="button"
              onClick={() => setModo(m)}
              className={`rounded-md px-2.5 py-1 text-[11px] font-semibold transition-glass ${
                modo === m ? "bg-accent-primary/25 text-text-primary" : "bg-white/5 text-text-secondary"
              }`}
            >
              {t(`settings.engenheiroPtt.teste.modo_${m}`)}
            </button>
          ))}
        </div>
        <p className="mb-2 text-[11px] text-text-secondary">
          {t(`settings.engenheiroPtt.teste.modo_${modo}_desc`)}
        </p>

        {modo === "eco" ? (
          <>
            {gravando
              ? t("settings.engenheiroPtt.teste.gravando")
              : ultima
                ? t("settings.engenheiroPtt.teste.capturou", {
                    s: (ultima.duracaoMs / 1000).toFixed(1),
                    kb: (ultima.bytes / 1024).toFixed(1),
                  })
                : t("settings.engenheiroPtt.teste.segureEFale")}
            {ultima?.curta ? (
              <span className="block text-status-red">
                {t("settings.engenheiroPtt.teste.curta")}
              </span>
            ) : null}
            {ultima ? <audio src={ultima.url} controls className="mt-2 h-8 w-full" /> : null}
          </>
        ) : (
          <>
            {/* A pergunta ESCRITA pula só a transcrição — a única perna que precisa do
                servidor no caminho gravado. Com ela dá para conferir o classificador, o
                renderizador e a voz sem nada no ar. */}
            <form
              className="mb-2 flex gap-2"
              onSubmit={(e) => {
                e.preventDefault();
                if (!pergunta.trim()) return;
                setRastro([]);
                orqRef.current.perguntarEscrito(pergunta.trim());
              }}
            >
              <input
                value={pergunta}
                onChange={(e) => setPergunta(e.target.value)}
                placeholder={t("settings.engenheiroPtt.teste.escrevaPergunta")}
                className="min-w-0 flex-1 rounded-lg border border-white/10 bg-white/5 px-3 py-1.5 text-[12px] text-text-primary"
              />
              <button
                type="submit"
                className="shrink-0 rounded-lg bg-white/10 px-3 py-1.5 text-[11px] font-semibold text-text-primary transition-glass hover:bg-white/20"
              >
                {t("settings.engenheiroPtt.teste.perguntar")}
              </button>
            </form>
            <label className="mb-2 flex items-center gap-2 text-[11px] text-text-secondary">
              <input type="checkbox" checked={forcar} onChange={(e) => setForcar(e.target.checked)} />
              {t("settings.engenheiroPtt.teste.forcarModelo")}
            </label>
            {/* O aviso só aparece quando o caso ACONTECEU. Uma explicação permanente ao
                lado da caixa seria lida como decoração; esta chega no instante em que a
                dúvida existe — o jogador acabou de ouvir "estou sem dados do carro" e não
                tem como saber que isso é o acervo funcionando, e não o sistema quebrado. */}
            {!forcar && semTelemetria ? (
              <p className="mb-2 rounded-lg border border-status-yellow/30 bg-status-yellow/10 px-3 py-2 text-[11px] text-text-primary">
                {t("settings.engenheiroPtt.teste.avisoSemTelemetria")}
              </p>
            ) : null}
            {gravando ? <p>{t("settings.engenheiroPtt.teste.gravando")}</p> : null}
            <Rastro passos={rastro} vazio={t("settings.engenheiroPtt.teste.semRastro")} />
          </>
        )}
      </Item>
    </div>
  );
}

/// O rastro do encanamento. Existe porque as seis pernas falham do MESMO jeito visto de
/// fora — o engenheiro diz que não sabe —, e sem isto "o Scribe não respondeu" e "o acervo
/// não cobre a pergunta" são a mesma tela em branco.
function Rastro({ passos, vazio }) {
  if (!passos.length) return <p className="text-text-secondary">{vazio}</p>;
  return (
    <ol className="space-y-1">
      {passos.map((p, i) => (
        <li key={i} className="flex gap-2 text-[11px]">
          <span className="w-10 shrink-0 text-right tabular-nums text-text-secondary">
            {p.ms >= 1 ? `${Math.round(p.ms)}ms` : ""}
          </span>
          <span
            className={`w-24 shrink-0 font-semibold ${
              p.nome === "desistiu" ? "text-status-red" : "text-text-primary"
            }`}
          >
            {p.nome}
          </span>
          <span className="min-w-0 flex-1 break-words text-text-secondary">{p.detalhe || ""}</span>
        </li>
      ))}
    </ol>
  );
}

/** Uma linha do diagnóstico, com o ponto que diz de longe se aquela ponta respondeu. */
function Item({ titulo, ok, neutro, children }) {
  const cor = neutro ? "bg-white/25" : ok ? "bg-status-green" : "bg-white/25";
  return (
    <div className="mb-3 last:mb-0">
      <p className="flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.14em] text-text-secondary">
        <span className={`h-1.5 w-1.5 rounded-full ${cor}`} />
        {titulo}
      </p>
      <div className="mt-1 pl-3.5 text-[12px] text-text-primary">{children}</div>
    </div>
  );
}
