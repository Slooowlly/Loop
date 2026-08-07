// i18n-ignore-file — bancada de laboratório do push-to-talk, nunca sai para o jogador.
//
// Prova três coisas que só o WebView2 pode responder, e que nenhum teste em Node ou em
// jsdom responderia:
//
//  1. A PERMISSÃO passa sem diálogo (o `--use-fake-ui-for-media-stream` funcionou).
//  2. Qual é o ATRASO entre pedir para gravar e estar gravando. É o número que decide se
//     dá para começar a gravar no toque do botão ou se o piloto perderia a primeira
//     sílaba e precisaríamos de um anel de pré-captura rodando o tempo todo.
//  3. Quanto PESA uma fala real — o tamanho do corpo que sobe para o Scribe.
//
// Segurar a barra de espaço faz o mesmo que segurar o botão, porque é assim que o dedo
// vai usar isso.

import { useCallback, useEffect, useRef, useState } from "react";
import {
  armar,
  comecar,
  desarmar,
  dispositivos,
  estaArmado,
  nivel,
  terminar,
} from "../../lib/microfone";

const fmtMs = (v) => (v == null ? "—" : `${v.toFixed(0)} ms`);
const fmtKb = (n) => `${(n / 1024).toFixed(1)} KB`;

export default function PttPocPage() {
  const [retratoMic, setRetratoMic] = useState(null);
  const [erro, setErro] = useState("");
  const [lista, setLista] = useState([]);
  const [escolhido, setEscolhido] = useState("");
  const [aec, setAec] = useState(true);
  const [gravando, setGravando] = useState(false);
  const [medidor, setMedidor] = useState(0);
  const [tomadas, setTomadas] = useState([]);
  const [ultimaUrl, setUltimaUrl] = useState("");
  // Pico do toque atual: o medidor instantâneo pisca demais para provar que houve voz.
  const picoRef = useRef(0);
  const urlRef = useRef("");
  // O atraso é medido no apertar e usado no soltar. Fica num ref porque entre os dois
  // acontece um `setGravando`, que recria os callbacks — em estado ele chegaria velho.
  const atrasoRef = useRef(null);

  // ── Medidor: um quadro por frame enquanto o microfone estiver armado.
  useEffect(() => {
    if (!retratoMic) return undefined;
    let vivo = true;
    const quadro = () => {
      if (!vivo) return;
      const v = nivel();
      setMedidor(v);
      if (v > picoRef.current) picoRef.current = v;
      requestAnimationFrame(quadro);
    };
    requestAnimationFrame(quadro);
    return () => {
      vivo = false;
    };
  }, [retratoMic]);

  useEffect(() => () => desarmar(), []);

  const ligar = useCallback(async () => {
    setErro("");
    try {
      const r = await armar({ deviceId: escolhido || null, cancelamentoDeEco: aec });
      setRetratoMic(r);
      // Os rótulos dos dispositivos só existem depois da permissão — por isso a lista é
      // buscada AQUI, e não na montagem da página.
      setLista(await dispositivos());
    } catch (e) {
      setRetratoMic(null);
      setErro(String(e.message || e));
    }
  }, [escolhido, aec]);

  const desligar = useCallback(() => {
    desarmar();
    setRetratoMic(null);
    setMedidor(0);
  }, []);

  const apertar = useCallback(async () => {
    if (!estaArmado() || gravando) return;
    picoRef.current = 0;
    atrasoRef.current = null;
    try {
      const { atrasoMs } = await comecar();
      atrasoRef.current = atrasoMs;
      setGravando(true);
    } catch (e) {
      setErro(String(e.message || e));
    }
  }, [gravando]);

  const soltar = useCallback(async () => {
    if (!gravando) return;
    setGravando(false);
    const r = await terminar();
    if (!r) return;
    if (urlRef.current) URL.revokeObjectURL(urlRef.current);
    const url = URL.createObjectURL(new Blob([r.bytes], { type: r.mime }));
    urlRef.current = url;
    setUltimaUrl(url);
    setTomadas((t) => [
      {
        n: t.length + 1,
        atrasoMs: atrasoRef.current,
        duracaoMs: r.duracaoMs,
        bytes: r.bytes.length,
        pico: picoRef.current,
        mime: r.mime,
        curta: r.curtaDemais,
      },
      ...t,
    ]);
  }, [gravando]);

  // Barra de espaço = o botão. `repeat` filtra a repetição automática do teclado, que
  // senão dispararia `comecar` dezenas de vezes enquanto a tecla está descendo.
  useEffect(() => {
    const dn = (e) => {
      if (e.code === "Space" && !e.repeat) {
        e.preventDefault();
        apertar();
      }
    };
    const up = (e) => {
      if (e.code === "Space") {
        e.preventDefault();
        soltar();
      }
    };
    window.addEventListener("keydown", dn);
    window.addEventListener("keyup", up);
    return () => {
      window.removeEventListener("keydown", dn);
      window.removeEventListener("keyup", up);
    };
  }, [apertar, soltar]);

  const medias = resumo(tomadas);

  return (
    <div style={S.pagina}>
      <h1 style={S.titulo}>Push-to-talk — captura do microfone</h1>
      <p style={S.sub}>
        Arme o microfone, segure a barra de espaço (ou o botão) e fale. A tabela guarda o
        atraso de início, a duração e o tamanho do corpo que subiria para o Scribe.
      </p>

      <section style={S.bloco}>
        <div style={S.linha}>
          <select
            value={escolhido}
            onChange={(e) => setEscolhido(e.target.value)}
            style={S.select}
            disabled={Boolean(retratoMic)}
          >
            <option value="">Microfone padrão do Windows</option>
            {lista.map((d) => (
              <option key={d.id} value={d.id}>
                {d.rotulo || d.id.slice(0, 12)}
              </option>
            ))}
          </select>
          <label style={S.check}>
            <input
              type="checkbox"
              checked={aec}
              onChange={(e) => setAec(e.target.checked)}
              disabled={Boolean(retratoMic)}
            />
            Cancelamento de eco
          </label>
          {retratoMic ? (
            <button onClick={desligar} style={S.btn}>
              Desarmar
            </button>
          ) : (
            <button onClick={ligar} style={{ ...S.btn, ...S.btnPrim }}>
              Armar microfone
            </button>
          )}
        </div>

        {erro ? <div style={S.erro}>{erro}</div> : null}

        {retratoMic ? (
          <div style={S.retrato}>
            <Campo r="Dispositivo" v={retratoMic.rotulo || "(sem rótulo)"} />
            <Campo r="Taxa" v={`${retratoMic.taxa} Hz`} />
            <Campo r="Canais" v={retratoMic.canais} />
            <Campo r="Eco" v={retratoMic.cancelamentoDeEco ? "cancelado" : "cru"} />
            <Campo r="Formato" v={retratoMic.formato || "(padrão do webview)"} />
          </div>
        ) : null}
      </section>

      {retratoMic ? (
        <section style={S.bloco}>
          <div style={S.medidorCaixa}>
            <div
              style={{
                ...S.medidorBarra,
                width: `${Math.min(100, medidor * 320)}%`,
                background: medidor > 0.25 ? "#f0883e" : "#3fb950",
              }}
            />
          </div>
          <button
            onMouseDown={apertar}
            onMouseUp={soltar}
            onMouseLeave={soltar}
            style={{ ...S.ptt, ...(gravando ? S.pttOn : {}) }}
          >
            {gravando ? "Gravando — solte para enviar" : "Segure para falar (ou barra de espaço)"}
          </button>
          {ultimaUrl ? <audio src={ultimaUrl} controls style={S.audio} /> : null}
        </section>
      ) : null}

      {tomadas.length ? (
        <section style={S.bloco}>
          <div style={S.resumo}>
            <Campo r="Tomadas" v={tomadas.length} />
            <Campo r="Atraso mediano" v={fmtMs(medias.atrasoMediano)} />
            <Campo r="Atraso máximo" v={fmtMs(medias.atrasoMax)} />
            <Campo r="Peso por segundo" v={`${(medias.bytesPorSeg / 1024).toFixed(1)} KB/s`} />
          </div>
          <table style={S.tabela}>
            <thead>
              <tr>
                {["#", "Atraso", "Duração", "Tamanho", "Pico", "Formato"].map((h) => (
                  <th key={h} style={S.th}>
                    {h}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {tomadas.map((t) => (
                <tr key={t.n} style={t.curta ? S.trCurta : undefined}>
                  <td style={S.td}>{t.n}</td>
                  <td style={S.td}>{fmtMs(t.atrasoMs)}</td>
                  <td style={S.td}>{fmtMs(t.duracaoMs)}</td>
                  <td style={S.td}>{fmtKb(t.bytes)}</td>
                  <td style={S.td}>{t.pico.toFixed(3)}</td>
                  <td style={{ ...S.td, opacity: 0.6 }}>{t.mime}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <p style={S.nota}>
            Linha esmaecida = toque curto demais (menos de 250 ms), o acidente que em
            corrida não merece uma viagem ao Scribe. Pico 0,000 com voz audível significa
            microfone mudo no hardware.
          </p>
        </section>
      ) : null}
    </div>
  );
}

function Campo({ r, v }) {
  return (
    <div style={S.campo}>
      <span style={S.campoR}>{r}</span>
      <span style={S.campoV}>{v}</span>
    </div>
  );
}

/** Mediana em vez de média: um atraso solto de 300 ms na primeira tomada (o driver
 *  acordando) não representa o que o piloto sente da segunda em diante. */
function resumo(tomadas) {
  const atrasos = tomadas.map((t) => t.atrasoMs).filter((v) => v != null);
  const ordenado = [...atrasos].sort((a, b) => a - b);
  const segundos = tomadas.reduce((s, t) => s + t.duracaoMs / 1000, 0);
  const bytes = tomadas.reduce((s, t) => s + t.bytes, 0);
  return {
    atrasoMediano: ordenado.length ? ordenado[Math.floor(ordenado.length / 2)] : null,
    atrasoMax: atrasos.length ? Math.max(...atrasos) : null,
    bytesPorSeg: segundos > 0 ? bytes / segundos : 0,
  };
}

const S = {
  pagina: {
    minHeight: "100vh",
    background: "#0d1117",
    color: "#e6edf3",
    padding: "28px 32px",
    fontFamily: "system-ui, sans-serif",
  },
  titulo: { fontSize: 22, margin: "0 0 4px", fontWeight: 600 },
  sub: { margin: "0 0 22px", color: "#8b949e", fontSize: 13, maxWidth: 620 },
  bloco: {
    background: "#161b22",
    border: "1px solid #30363d",
    borderRadius: 10,
    padding: 16,
    marginBottom: 16,
  },
  linha: { display: "flex", gap: 12, alignItems: "center", flexWrap: "wrap" },
  select: {
    background: "#0d1117",
    color: "#e6edf3",
    border: "1px solid #30363d",
    borderRadius: 6,
    padding: "7px 10px",
    fontSize: 13,
    minWidth: 240,
  },
  check: { display: "flex", gap: 6, alignItems: "center", fontSize: 13, color: "#8b949e" },
  btn: {
    background: "#21262d",
    color: "#e6edf3",
    border: "1px solid #30363d",
    borderRadius: 6,
    padding: "7px 14px",
    fontSize: 13,
    cursor: "pointer",
  },
  btnPrim: { background: "#238636", borderColor: "#2ea043" },
  erro: {
    marginTop: 12,
    padding: "9px 12px",
    background: "rgba(248,81,73,0.1)",
    border: "1px solid #f85149",
    borderRadius: 6,
    fontSize: 13,
    color: "#ff7b72",
  },
  retrato: { display: "flex", gap: 22, flexWrap: "wrap", marginTop: 14 },
  campo: { display: "flex", flexDirection: "column", gap: 2 },
  campoR: { fontSize: 11, color: "#8b949e", textTransform: "uppercase", letterSpacing: 0.4 },
  campoV: { fontSize: 14, fontVariantNumeric: "tabular-nums" },
  medidorCaixa: {
    height: 10,
    background: "#0d1117",
    border: "1px solid #30363d",
    borderRadius: 5,
    overflow: "hidden",
    marginBottom: 14,
  },
  medidorBarra: { height: "100%", transition: "width 60ms linear" },
  ptt: {
    width: "100%",
    padding: "18px 0",
    fontSize: 15,
    fontWeight: 600,
    color: "#e6edf3",
    background: "#21262d",
    border: "1px solid #30363d",
    borderRadius: 8,
    cursor: "pointer",
    userSelect: "none",
  },
  pttOn: { background: "#8b2c2c", borderColor: "#f85149" },
  audio: { width: "100%", marginTop: 14 },
  resumo: { display: "flex", gap: 26, flexWrap: "wrap", marginBottom: 14 },
  tabela: { width: "100%", borderCollapse: "collapse", fontSize: 13 },
  th: {
    textAlign: "left",
    padding: "6px 8px",
    borderBottom: "1px solid #30363d",
    color: "#8b949e",
    fontWeight: 500,
    fontSize: 11,
    textTransform: "uppercase",
    letterSpacing: 0.4,
  },
  td: {
    padding: "6px 8px",
    borderBottom: "1px solid #21262d",
    fontVariantNumeric: "tabular-nums",
  },
  trCurta: { opacity: 0.4 },
  nota: { fontSize: 12, color: "#8b949e", marginTop: 12, marginBottom: 0, maxWidth: 620 },
};
