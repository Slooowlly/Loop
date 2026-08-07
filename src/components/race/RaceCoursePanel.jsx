import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import Tooltip from "../ui/Tooltip";
import RaceTraceChart from "./RaceTraceChart";

// O CURSO DA CORRIDA — o painel que faz o resultado explicar o resultado.
//
// A reforma da simulação (moeda em tempo, ar sujo, trem de carros, ultrapassagem que
// pode falhar, estratégia de box, safety car) funcionou nas métricas e era invisível na
// tela: o jogador via "P8" e nenhuma causa. Sem causa legível, variação atribuível a
// qualidade é indistinguível de azar — e é esse buraco que este painel fecha.
//
// Tudo aqui sai do PRÓPRIO `result` (os campos crus viajam no payload da corrida e,
// desde a v55, também no save). Nada de invoke: o dado já está na mão.
//
// Regra de ouro do vazio: corrida sem dado de trecho (save anterior à v55, ou import do
// iRacing, que não tem trecho nenhum) NÃO desenha um traçado chutado — o painel
// simplesmente não aparece. Traçado errado é pior que traçado ausente.

// Quantos carros entram no traçado além do jogador. Mais que isso e o gráfico deixa de
// ser leitura e passa a ser mancha — o ponto é enxergar a SUA corrida contra os que
// disputaram com você, não auditar o pelotão.
const CARROS_NO_TRACADO = 6;

const COR_JOGADOR = "#58a6ff";
const COR_OUTRO = "#4b5563";

// Posição do piloto na ordem registrada antes do safety car (1-based), ou null.
function posicaoNaOrdem(ordem, pilotId) {
  if (!Array.isArray(ordem)) return null;
  const idx = ordem.indexOf(pilotId);
  return idx >= 0 ? idx + 1 : null;
}

// Chip compacto: um número grande e um rótulo miúdo. É o formato do resto da tela.
function Chip({ label, value, tone, title }) {
  return (
    <Tooltip texto={title}>
      <div
        className="flex min-w-[92px] flex-col gap-1 rounded-xl px-3 py-2"
        style={{ background: "rgba(255,255,255,0.03)", border: "1px solid rgba(255,255,255,0.07)" }}
      >
        <span style={{ color: "#6e7681" }} className="text-[9px] uppercase tracking-[0.14em] leading-none">
          {label}
        </span>
        <span style={{ color: tone || "#e6edf3" }} className="text-[15px] font-semibold leading-none tabular-nums">
          {value}
        </span>
      </div>
    </Tooltip>
  );
}

function RaceCoursePanel({ result }) {
  const { t } = useTranslation();

  const jogador = useMemo(
    () => result?.race_results?.find((e) => e.is_jogador) ?? null,
    [result],
  );

  // Os carros que entram no traçado: o jogador sempre, mais os melhores colocados até o
  // teto. Se o jogador terminou fora desse corte ele entra do mesmo jeito — a linha dele
  // é o assunto do gráfico.
  const carrosDoTracado = useMemo(() => {
    const comTrecho = (result?.race_results ?? []).filter(
      (e) => Array.isArray(e.posicoes_por_segmento) && e.posicoes_por_segmento.length > 0,
    );
    if (comTrecho.length === 0) return [];
    const ordenados = [...comTrecho].sort(
      (a, b) => (a.finish_position ?? 999) - (b.finish_position ?? 999),
    );
    const escolhidos = ordenados.slice(0, CARROS_NO_TRACADO);
    if (jogador && !escolhidos.some((e) => e.pilot_id === jogador.pilot_id)) {
      escolhidos.push(jogador);
    }
    return escolhidos;
  }, [result, jogador]);

  // Trechos → voltas. O motor divide a corrida em N trechos iguais e registra a posição
  // ao FIM de cada um, então o trecho i cai na volta (i+1) * voltas/N. O ponto na volta 0
  // é a largada — sem ele a linha começaria já no meio da corrida.
  const { rows, cars, nameByIdx } = useMemo(() => {
    if (carrosDoTracado.length === 0) return { rows: [], cars: [], nameByIdx: {} };
    const nTrechos = Math.max(
      ...carrosDoTracado.map((e) => e.posicoes_por_segmento.length),
    );
    const voltas = result?.total_laps > 0 ? result.total_laps : nTrechos;
    const passo = voltas / nTrechos;

    const nomes = {};
    const carsOut = [];
    carrosDoTracado.forEach((entry, idx) => {
      nomes[idx] = entry.pilot_name;
      carsOut.push({ idx, isPlayer: !!entry.is_jogador });
    });
    // O jogador desenhado por último fica por cima das outras linhas.
    carsOut.sort((a, b) => Number(a.isPlayer) - Number(b.isPlayer));

    const linhas = [];
    for (let s = 0; s <= nTrechos; s += 1) {
      const row = { lap: s === 0 ? 0 : Math.round(passo * s) };
      carrosDoTracado.forEach((entry, idx) => {
        if (s === 0) {
          row[`c${idx}`] = entry.grid_position > 0 ? entry.grid_position : null;
        } else {
          const pos = entry.posicoes_por_segmento[s - 1];
          row[`c${idx}`] = Number.isFinite(pos) && pos > 0 ? pos : null;
        }
      });
      linhas.push(row);
    }
    return { rows: linhas, cars: carsOut, nameByIdx: nomes };
  }, [carrosDoTracado, result]);

  const corPorCarro = useMemo(() => {
    const mapa = {};
    carrosDoTracado.forEach((entry, idx) => {
      mapa[idx] = entry.is_jogador ? COR_JOGADOR : COR_OUTRO;
    });
    return (idx) => mapa[idx] ?? COR_OUTRO;
  }, [carrosDoTracado]);

  // As voltas de safety car viram faixas amarelas no traçado — é o que liga a amarela ao
  // degrau que ela causou na linha do jogador, no mesmo eixo.
  const voltasDeSafetyCar = useMemo(
    () => (result?.safety_cars ?? []).filter((v) => Number.isFinite(v) && v > 0),
    [result],
  );

  // As paradas do jogador, cada uma com o par de posições. Os três vetores são paralelos
  // por contrato do motor; um par ausente não invalida a parada.
  const paradas = useMemo(() => {
    if (!jogador) return [];
    return (jogador.volta_da_parada ?? []).map((volta, i) => ({
      volta,
      antes: jogador.posicao_antes_da_parada?.[i] ?? null,
      depois: jogador.posicao_depois?.[i] ?? null,
    }));
  }, [jogador]);

  if (!jogador) return null;

  const tentativas = jogador.tentativas_ultrapassagem ?? 0;
  const concluidas = jogador.ultrapassagens_concluidas ?? 0;
  const sofridas = jogador.tentativas_sofridas ?? 0;
  const preso = jogador.maior_sequencia_preso ?? 0;
  const arSujo = jogador.segmentos_em_ar_sujo ?? 0;

  const temTracado = rows.length > 0;
  const temTransito = tentativas > 0 || sofridas > 0 || preso > 0 || arSujo > 0;
  const temParada = paradas.length > 0;
  const temSafetyCar = voltasDeSafetyCar.length > 0;

  // Nada gravado = painel ausente. Ver a regra do vazio no topo do arquivo.
  if (!temTracado && !temTransito && !temParada && !temSafetyCar) return null;

  return (
    <div
      style={{ background: "rgba(0,0,0,0.20)", border: "1px solid rgba(255,255,255,0.06)" }}
      className="rounded-2xl p-4"
    >
      <div className="mb-3 flex items-baseline justify-between gap-3">
        <span style={{ color: "#6e7681" }} className="text-[10px] uppercase tracking-[0.16em]">
          {t("raceResult.course.title")}
        </span>
        {jogador.estrategia_id ? (
          <span style={{ color: "#8b949e" }} className="text-[11px]">
            {t("raceResult.course.strategy", { strategy: jogador.estrategia_id })}
          </span>
        ) : null}
      </div>

      {/* A HISTÓRIA EM QUATRO NÚMEROS: largou, parou, voltou, chegou. É o que faz o
          jogador perceber que estratégia existe — antes disto o box era invisível. */}
      <div className="mb-3 flex flex-wrap items-center gap-2 text-[13px]">
        <span style={{ color: "#8b949e" }}>
          {t("raceResult.course.started", { pos: jogador.grid_position })}
        </span>
        {paradas.map((p) => (
          <span key={`pit-${p.volta}`} style={{ color: "#8b949e" }}>
            <span style={{ color: "#3f4650" }}>·</span>{" "}
            {p.antes != null && p.depois != null
              ? t("raceResult.course.pit", { lap: p.volta, before: p.antes, after: p.depois })
              : t("raceResult.course.pitBare", { lap: p.volta })}
          </span>
        ))}
        <span style={{ color: "#8b949e" }}>
          <span style={{ color: "#3f4650" }}>·</span>{" "}
          {jogador.is_dnf
            ? t("raceResult.course.finishedDnf")
            : t("raceResult.course.finished", { pos: jogador.finish_position })}
        </span>
      </div>

      {temTracado && (
        <div className="h-[220px] w-full">
          <RaceTraceChart
            rows={rows}
            cars={cars}
            colorForCar={corPorCarro}
            nameByIdx={nameByIdx}
            mode="position"
            yellowLaps={voltasDeSafetyCar}
          />
        </div>
      )}

      <div className="mt-3 flex flex-wrap gap-2">
        {tentativas > 0 && (
          // A razão entre as duas é a taxa de conversão — que antes da reforma era 100%
          // implícita, e é o mecanismo mais novo do motor e o mais invisível.
          <Chip
            label={t("raceResult.course.overtakes")}
            value={`${concluidas}/${tentativas}`}
            tone={concluidas === 0 ? "#f87171" : concluidas === tentativas ? "#4ade80" : undefined}
            title={t("raceResult.course.overtakesTip")}
          />
        )}
        {sofridas > 0 && (
          <Chip label={t("raceResult.course.defended")} value={sofridas} />
        )}
        {preso > 0 && (
          <Chip
            label={t("raceResult.course.stuck")}
            value={preso}
            tone={preso >= 3 ? "#f0b37a" : undefined}
            title={t("raceResult.course.stuckTip")}
          />
        )}
        {arSujo > 0 && (
          <Chip
            label={t("raceResult.course.dirtyAir")}
            value={arSujo}
            title={t("raceResult.course.dirtyAirTip")}
          />
        )}
        {voltasDeSafetyCar.map((volta, i) => {
          const antes = posicaoNaOrdem(result?.ordem_pre_safety_car?.[i], jogador.pilot_id);
          return (
            <Chip
              key={`sc-${volta}`}
              label={t("raceResult.course.safetyCar")}
              value={t("raceResult.course.safetyCarLap", { lap: volta })}
              tone="#f5c76d"
              title={
                antes != null && !jogador.is_dnf
                  ? t("raceResult.course.safetyCarTip", {
                      lap: volta,
                      before: antes,
                      after: jogador.finish_position,
                    })
                  : t("raceResult.course.safetyCarTipBare", { lap: volta })
              }
            />
          );
        })}
      </div>
    </div>
  );
}

export default RaceCoursePanel;
