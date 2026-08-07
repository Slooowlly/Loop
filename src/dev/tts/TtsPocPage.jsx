// i18n-ignore-file — painel de laboratório, nunca sai para o jogador.
//
// Etapa 4 — o acionamento manual, e a casa das Etapas 6, 7, 8, 10 e 11.
//
// O painel existe para dois públicos ao mesmo tempo: o número (a tabela e as
// estatísticas) e o ouvido (apertar o botão e SENTIR a espera). Por isso o disparo
// individual está no topo, grande, e a bateria automática logo abaixo.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { estaNoTauri } from "../../lib/tauri";
import { CATEGORIAS, CATEGORIA_ANTECIPADA, varianteDaVez } from "./ttsScripts";
import { gerarFala, tocarReservaLocal, reiniciarRelogioDeUso } from "./ttsRunner";
import {
  resumir,
  classificar,
  formatarMs,
  formatarPercentual,
  VEREDITOS,
} from "./ttsMetrics";
import "./TtsPocPage.css";

// Vozes do Gemini TTS que servem a um engenheiro de pista. `Charon` é a padrão por ser
// grave e informativa; as outras estão aqui para o teste de timbre, não de latência.
const VOZES = [
  "Charon",
  "Fenrir",
  "Orus",
  "Puck",
  "Kore",
  "Algieba",
  "Sadaltager",
  "Rasalgethi",
  "Iapetus",
  "Achird",
];

const MODELOS = [
  { id: "gemini-3.1-flash-tts-preview", rotulo: "Gemini 3.1 Flash TTS (streaming)" },
  { id: "gemini-2.5-flash-tts", rotulo: "Gemini 2.5 Flash TTS (sem streaming)" },
  { id: "gemini-2.5-pro-tts", rotulo: "Gemini 2.5 Pro TTS (sem streaming)" },
];

const REPETICOES_BATERIA = 10;
const ANTECIPACAO_PADRAO_MS = 4000;

export default function TtsPocPage() {
  const [voz, setVoz] = useState(VOZES[0]);
  const [modelo, setModelo] = useState(MODELOS[0].id);
  const [streaming, setStreaming] = useState(true);
  const [radio, setRadio] = useState(false);
  const [usarDirecao, setUsarDirecao] = useState(true);
  const [prebufferMs, setPrebufferMs] = useState(120);
  const [slaMs, setSlaMs] = useState(2500);

  const [registros, setRegistros] = useState([]);
  const [ocupado, setOcupado] = useState(false);
  const [progresso, setProgresso] = useState(null);
  const [aoVivo, setAoVivo] = useState(null);
  const [caminhoLog, setCaminhoLog] = useState("");
  const [aviso, setAviso] = useState(estaNoTauri() ? "" : "Fora do Tauri: rode com `npm run tauri dev`.");
  const [antecipacao, setAntecipacao] = useState(null);

  const contador = useRef({});
  const abortar = useRef(false);
  const cancelarAtual = useRef(null);

  useEffect(() => {
    if (!estaNoTauri()) return;
    invoke("tts_poc_log_caminho").then(setCaminhoLog).catch(() => {});
    invoke("tts_poc_log_ler")
      .then((linhas) => {
        const lidos = linhas
          .map((l) => {
            try {
              return JSON.parse(l);
            } catch {
              return null;
            }
          })
          .filter(Boolean);
        if (lidos.length) setRegistros(lidos);
      })
      .catch(() => {});
    reiniciarRelogioDeUso();
  }, []);

  const opcoes = useMemo(
    () => ({ voz, modelo, streaming, radio, prebufferMs, usarDirecao, slaMs }),
    [voz, modelo, streaming, radio, prebufferMs, usarDirecao, slaMs],
  );

  const disparar = useCallback(
    async (categoria, { indice, sla } = {}) => {
      const n = indice ?? contador.current[categoria.id] ?? 0;
      contador.current[categoria.id] = n + 1;
      const texto = varianteDaVez(categoria, n);

      setAoVivo({ categoria: categoria.id, texto, estado: "enviando" });
      const registro = await gerarFala({
        ...opcoes,
        slaMs: sla ?? opcoes.slaMs,
        categoria,
        texto,
        exporCancelamento: (fn) => {
          cancelarAtual.current = fn;
        },
        onEvento: (e) => {
          if (e.tipo === "primeiroBloco") {
            setAoVivo((a) => (a ? { ...a, estado: "recebendo", msPrimeiroBloco: e.ms } : a));
          }
          if (e.tipo === "primeiroSom") {
            setAoVivo((a) => (a ? { ...a, estado: "tocando", msPrimeiroSom: e.ms } : a));
          }
        },
      });
      cancelarAtual.current = null;
      setRegistros((atuais) => [...atuais, registro]);
      setAoVivo({ categoria: categoria.id, texto, estado: "fim", registro });
      if (registro.erro) setAviso(registro.erro);
      return registro;
    },
    [opcoes],
  );

  const dispararUm = useCallback(
    async (categoria) => {
      if (ocupado) return;
      setOcupado(true);
      setAviso("");
      try {
        await disparar(categoria);
      } finally {
        setOcupado(false);
      }
    },
    [disparar, ocupado],
  );

  // Etapa 6 — a bateria. Sequencial de propósito: chamadas paralelas competiriam pela
  // mesma banda e pela mesma cota, e mediriam a fila, não o serviço.
  const rodarBateria = useCallback(async () => {
    if (ocupado) return;
    setOcupado(true);
    setAviso("");
    abortar.current = false;
    const total = CATEGORIAS.length * REPETICOES_BATERIA;
    let feitos = 0;
    try {
      for (const categoria of CATEGORIAS) {
        for (let i = 0; i < REPETICOES_BATERIA; i += 1) {
          if (abortar.current) return;
          setProgresso({ feitos, total, categoria: categoria.rotulo });
          await disparar(categoria, { indice: i });
          feitos += 1;
          // Respiro entre chamadas: sem ele o teste vira medição de rate limit.
          await new Promise((r) => setTimeout(r, 1200));
        }
      }
      setProgresso({ feitos, total, categoria: "concluída" });
    } finally {
      setOcupado(false);
    }
  }, [disparar, ocupado]);

  // Etapa 11 — pede a comemoração como se o piloto tivesse entrado no último setor e
  // confere se a fala chegou antes da linha.
  const rodarAntecipada = useCallback(async () => {
    if (ocupado) return;
    setOcupado(true);
    setAviso("");
    const linhaEm = performance.now() + ANTECIPACAO_PADRAO_MS;
    setAntecipacao({ estado: "gerando", restanteMs: ANTECIPACAO_PADRAO_MS });
    const tique = setInterval(() => {
      const restante = Math.max(0, linhaEm - performance.now());
      setAntecipacao((a) => (a && a.estado === "gerando" ? { ...a, restanteMs: restante } : a));
    }, 100);
    try {
      // Sem SLA: aqui o ponto não é cortar, é descobrir se 1 a 3 segundos cabem na
      // janela de antecipação.
      const registro = await disparar(CATEGORIA_ANTECIPADA, { sla: 0 });
      const prontaEm = registro.msPrimeiroSom ?? registro.msTotal;
      setAntecipacao({
        estado: "fim",
        prontaEm,
        janelaMs: ANTECIPACAO_PADRAO_MS,
        chegouATempo: Number.isFinite(prontaEm) && prontaEm <= ANTECIPACAO_PADRAO_MS,
        folgaMs: ANTECIPACAO_PADRAO_MS - (prontaEm ?? Infinity),
      });
    } finally {
      clearInterval(tique);
      setOcupado(false);
    }
  }, [disparar, ocupado]);

  const descartarAntecipada = useCallback(() => {
    cancelarAtual.current?.("vitoria-evaporou");
    setAntecipacao({ estado: "descartada" });
  }, []);

  const limpar = useCallback(() => {
    setRegistros([]);
    setProgresso(null);
    setAoVivo(null);
    contador.current = {};
    reiniciarRelogioDeUso();
  }, []);

  const copiarJsonl = useCallback(() => {
    const texto = registros.map((r) => JSON.stringify(r)).join("\n");
    navigator.clipboard?.writeText(texto);
  }, [registros]);

  // ---- Estatística ----
  const porCategoria = useMemo(() => {
    const mapa = {};
    for (const c of [...CATEGORIAS, CATEGORIA_ANTECIPADA]) {
      const dela = registros.filter((r) => r.categoria === c.id);
      if (dela.length) mapa[c.id] = { categoria: c, resumo: resumir(dela) };
    }
    return mapa;
  }, [registros]);

  const resumoGeral = useMemo(() => resumir(registros), [registros]);
  const veredito = useMemo(() => classificar(resumoGeral), [resumoGeral]);

  const quentes = useMemo(
    () => resumir(registros.filter((r) => r.fase === "sequencia")),
    [registros],
  );
  const frias = useMemo(
    () => resumir(registros.filter((r) => r.fase === "fria" || r.fase === "primeira")),
    [registros],
  );

  return (
    <div className="ttspoc">
      <header className="ttspoc__topo">
        <div>
          <h1>Prova de conceito — latência do TTS do Google</h1>
          <p className="ttspoc__sub">
            Mede o tempo entre pedir a fala e o primeiro som sair na caixa. Sem telemetria, sem
            STT, sem intenção, sem cache.
          </p>
        </div>
        <div className="ttspoc__veredito" data-nivel={veredito.id}>
          <strong>{veredito.rotulo}</strong>
          <span>{veredito.detalhe}</span>
        </div>
      </header>

      {aviso ? <div className="ttspoc__aviso">{aviso}</div> : null}

      <section className="ttspoc__controles">
        <label>
          Voz
          <select value={voz} onChange={(e) => setVoz(e.target.value)}>
            {VOZES.map((v) => (
              <option key={v} value={v}>
                {v}
              </option>
            ))}
          </select>
        </label>
        <label>
          Modelo
          <select value={modelo} onChange={(e) => setModelo(e.target.value)}>
            {MODELOS.map((m) => (
              <option key={m.id} value={m.id}>
                {m.rotulo}
              </option>
            ))}
          </select>
        </label>
        <label>
          Pré-buffer
          <input
            type="number"
            min="0"
            max="1000"
            step="20"
            value={prebufferMs}
            onChange={(e) => setPrebufferMs(Number(e.target.value))}
          />
        </label>
        <label>
          Corte (SLA)
          <input
            type="number"
            min="0"
            max="10000"
            step="250"
            value={slaMs}
            onChange={(e) => setSlaMs(Number(e.target.value))}
          />
        </label>
        <label className="ttspoc__chave">
          <input
            type="checkbox"
            checked={streaming}
            onChange={(e) => setStreaming(e.target.checked)}
          />
          Streaming
        </label>
        <label className="ttspoc__chave">
          <input type="checkbox" checked={radio} onChange={(e) => setRadio(e.target.checked)} />
          Cadeia de rádio
        </label>
        <label className="ttspoc__chave">
          <input
            type="checkbox"
            checked={usarDirecao}
            onChange={(e) => setUsarDirecao(e.target.checked)}
          />
          Direção de atuação
        </label>
      </section>

      <section className="ttspoc__disparo">
        {CATEGORIAS.map((c) => (
          <button
            key={c.id}
            type="button"
            className="ttspoc__botao"
            disabled={ocupado}
            onClick={() => dispararUm(c)}
          >
            <strong>{c.rotulo}</strong>
            <span>{c.nota}</span>
          </button>
        ))}
      </section>

      <section className="ttspoc__aovivo">
        {aoVivo ? (
          <>
            <p className="ttspoc__falatexto">“{aoVivo.texto}”</p>
            <div className="ttspoc__numeros">
              <Numero rotulo="1º bloco" valor={aoVivo.msPrimeiroBloco ?? aoVivo.registro?.msPrimeiroBloco} destaque={false} />
              <Numero rotulo="1º SOM" valor={aoVivo.msPrimeiroSom ?? aoVivo.registro?.msPrimeiroSom} destaque />
              <Numero rotulo="Total" valor={aoVivo.registro?.msTotal} destaque={false} />
              <Numero rotulo="Áudio" valor={aoVivo.registro?.duracaoAudioMs} destaque={false} />
              <div className="ttspoc__num">
                <span>Cortes</span>
                <strong>{aoVivo.registro?.interrupcoes ?? "—"}</strong>
              </div>
            </div>
          </>
        ) : (
          <p className="ttspoc__vazio">Aperte uma das três falas acima.</p>
        )}
      </section>

      <section className="ttspoc__bateria">
        <button type="button" onClick={rodarBateria} disabled={ocupado}>
          Rodar bateria ({REPETICOES_BATERIA}× cada, {CATEGORIAS.length * REPETICOES_BATERIA} no total)
        </button>
        <button type="button" onClick={() => (abortar.current = true)} disabled={!ocupado}>
          Parar
        </button>
        <button type="button" onClick={() => tocarReservaLocal()} disabled={ocupado}>
          Ouvir a reserva local
        </button>
        <button type="button" onClick={copiarJsonl} disabled={!registros.length}>
          Copiar JSONL
        </button>
        <button type="button" onClick={limpar} disabled={ocupado || !registros.length}>
          Limpar tabela
        </button>
        {progresso ? (
          <span className="ttspoc__progresso">
            {progresso.feitos}/{progresso.total} — {progresso.categoria}
          </span>
        ) : null}
      </section>

      <section className="ttspoc__antecipada">
        <div>
          <h2>Antecipação — comemoração pedida no último setor</h2>
          <p className="ttspoc__sub">
            Dispara agora e simula {ANTECIPACAO_PADRAO_MS / 1000} s até a linha. A pergunta é se a
            fala chega antes.
          </p>
        </div>
        <button type="button" onClick={rodarAntecipada} disabled={ocupado}>
          Simular última curva
        </button>
        <button type="button" onClick={descartarAntecipada} disabled={!ocupado}>
          Descartar (a vitória evaporou)
        </button>
        {antecipacao?.estado === "gerando" ? (
          <span className="ttspoc__contagem">
            Linha em {(antecipacao.restanteMs / 1000).toFixed(1)} s
          </span>
        ) : null}
        {antecipacao?.estado === "descartada" ? (
          <span className="ttspoc__contagem">Fala descartada antes de tocar.</span>
        ) : null}
        {antecipacao?.estado === "fim" ? (
          <span className="ttspoc__contagem" data-ok={antecipacao.chegouATempo ? "sim" : "nao"}>
            {antecipacao.chegouATempo
              ? `Chegou com ${formatarMs(antecipacao.folgaMs)} de folga.`
              : `Não chegou a tempo (${formatarMs(antecipacao.prontaEm)}).`}
          </span>
        ) : null}
      </section>

      <section className="ttspoc__estatistica">
        <h2>Estatística — tempo até o primeiro som</h2>
        <TabelaResumo
          linhas={[
            ...Object.values(porCategoria).map(({ categoria, resumo }) => ({
              rotulo: categoria.rotulo,
              resumo,
            })),
            { rotulo: "Chamadas em sequência (quentes)", resumo: quentes },
            { rotulo: "Chamadas frias / primeira", resumo: frias },
            { rotulo: "TOTAL", resumo: resumoGeral, forte: true },
          ]}
        />
      </section>

      <section className="ttspoc__historico">
        <h2>Execuções ({registros.length})</h2>
        {caminhoLog ? <p className="ttspoc__caminho">Log: {caminhoLog}</p> : null}
        <div className="ttspoc__tabelaEnvolve">
          <table>
            <thead>
              <tr>
                <th>Hora</th>
                <th>Categoria</th>
                <th>Fase</th>
                <th>Chars</th>
                <th>1º bloco</th>
                <th>1º som</th>
                <th>Total</th>
                <th>Áudio</th>
                <th>Cortes</th>
                <th>Rádio</th>
                <th>Desfecho</th>
              </tr>
            </thead>
            <tbody>
              {registros
                .slice(-60)
                .reverse()
                .map((r) => (
                  <tr key={r.id} data-falha={r.sucesso ? undefined : "sim"}>
                    <td>{r.quando.slice(11, 19)}</td>
                    <td>{r.categoria}</td>
                    <td>{r.fase}</td>
                    <td>{r.caracteres}</td>
                    <td>{formatarMs(r.msPrimeiroBloco)}</td>
                    <td className="ttspoc__forte">{formatarMs(r.msPrimeiroSom)}</td>
                    <td>{formatarMs(r.msTotal)}</td>
                    <td>{formatarMs(r.duracaoAudioMs)}</td>
                    <td>{r.interrupcoes}</td>
                    <td>{r.radio ? "sim" : "não"}</td>
                    <td>
                      {r.sucesso
                        ? "ok"
                        : r.estourouSla
                          ? "corte por SLA"
                          : r.cancelado
                            ? "cancelada"
                            : (r.erro ?? "falha")}
                    </td>
                  </tr>
                ))}
            </tbody>
          </table>
        </div>
      </section>
    </div>
  );
}

function Numero({ rotulo, valor, destaque }) {
  return (
    <div className="ttspoc__num" data-destaque={destaque ? "sim" : undefined}>
      <span>{rotulo}</span>
      <strong>{formatarMs(valor)}</strong>
    </div>
  );
}

function TabelaResumo({ linhas }) {
  return (
    <div className="ttspoc__tabelaEnvolve">
      <table>
        <thead>
          <tr>
            <th>Conjunto</th>
            <th>n</th>
            <th>Melhor</th>
            <th>Mediana</th>
            <th>Média</th>
            <th>P90</th>
            <th>P95</th>
            <th>Pior</th>
            <th>&lt;1s</th>
            <th>&lt;1,5s</th>
            <th>&lt;2s</th>
            <th>&gt;3s</th>
            <th>Falhas</th>
            <th>Cortes</th>
          </tr>
        </thead>
        <tbody>
          {linhas
            .filter((l) => l.resumo && l.resumo.total > 0)
            .map((l) => (
              <tr key={l.rotulo} className={l.forte ? "ttspoc__forte" : undefined}>
                <td>{l.rotulo}</td>
                <td>{l.resumo.sucessos}</td>
                <td>{formatarMs(l.resumo.melhor)}</td>
                <td>{formatarMs(l.resumo.mediana)}</td>
                <td>{formatarMs(l.resumo.media)}</td>
                <td>{formatarMs(l.resumo.p90)}</td>
                <td>{formatarMs(l.resumo.p95)}</td>
                <td>{formatarMs(l.resumo.pior)}</td>
                <td>{formatarPercentual(l.resumo.abaixo1000)}</td>
                <td>{formatarPercentual(l.resumo.abaixo1500)}</td>
                <td>{formatarPercentual(l.resumo.abaixo2000)}</td>
                <td>{formatarPercentual(l.resumo.acima3000)}</td>
                <td>{formatarPercentual(l.resumo.percentualFalhas)}</td>
                <td>{l.resumo.interrupcoes}</td>
              </tr>
            ))}
        </tbody>
      </table>
    </div>
  );
}

export { VEREDITOS };
