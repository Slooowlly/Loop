import { useTranslation } from "react-i18next";
import { MEDAL_COLORS, PLACEMENT_COLORS, corDeTextoSobre } from "./teamHistoryV2Logic";
import { rotuloFaixa } from "./teamHistoryV2Labels";
import Tooltip from "../../ui/Tooltip";
import { MedalKey, BlockLabel } from "./teamHistoryV2Primitives.jsx";

// Assinatura de resultados e confiabilidade do dossiê de equipe v2.
//
// Extraídas de `TeamHistoryDrawerV2.jsx` em 11/08/2026. As duas são barras
// empilhadas sobre o MESMO universo de corridas, repartido por critérios
// diferentes — colocação numa, motivo do fim na outra — e dividem o corte mínimo
// de rótulo, que é o que as mantém no mesmo arquivo.

// Assinatura de resultados: TODAS as corridas repartidas por faixa de colocação.
//
// Records dá a taxa de pódio; isto dá a forma dela. Duas equipes com 60% de
// pódio desenham diferente aqui — uma converte em vitória, a outra vive em
// terceiro —, e a taxa sozinha não separa as duas.
// Fatia mínima, em % do total, para o número caber dentro da barra. Abaixo
// disso a caixa tem menos que a largura de "1 (1%)" e o texto sai cortado.
const FAIXA_MIN_ROTULO = 5;

export function ResultSpread({ spread }) {
  const { t } = useTranslation();
  if (!spread || spread.races <= 0) return null;
  const faixas = [
    { id: "first", value: spread.first, color: PLACEMENT_COLORS.first },
    { id: "podium", value: spread.podium, color: PLACEMENT_COLORS.second },
    { id: "nearMiss", value: spread.nearMiss, color: PLACEMENT_COLORS.nearMiss },
    { id: "topTen", value: spread.topTen, color: PLACEMENT_COLORS.topTen },
    { id: "outside", value: spread.outside, color: PLACEMENT_COLORS.outside },
  ]
    .filter((faixa) => faixa.value > 0)
    .map((faixa) => {
      // A proporção é o que a barra desenha; o número é o que ela esconde. "24"
      // não diz se é a equipe toda ou um terço dela, e "73%" não diz de quantas
      // corridas — os dois juntos fecham a leitura sem exigir a legenda.
      const share = (faixa.value / spread.races) * 100;
      return { ...faixa, percent: Math.round(share), cabeNaBarra: share >= FAIXA_MIN_ROTULO };
    });
  if (!faixas.length) return null;
  return (
    <div>
      <div className="flex items-baseline gap-2">
        <BlockLabel>{t("myTeamTab.history.sport.resultSpread")}</BlockLabel>
        <span className="font-mono text-[10px] text-text-muted">
          {t("myTeamTab.history.sport.spreadRaces", { value: spread.races })}
        </span>
      </div>
      <div className="mt-2.5 flex h-7 overflow-hidden rounded-md" data-testid="team-history-spread">
        {faixas.map((faixa) => (
          <Tooltip
            key={faixa.id}
            texto={`${t(`myTeamTab.history.sport.spread.${faixa.id}`)} · ${rotuloFaixa(t, faixa)}`}
          >
            <span
              data-band={faixa.id}
              className="flex items-center justify-center overflow-hidden whitespace-nowrap px-1 font-mono text-[10px] font-semibold"
              style={{
                flexGrow: faixa.value,
                flexBasis: 0,
                backgroundColor: faixa.color,
                color: corDeTextoSobre(faixa.color),
              }}
            >
              {/* Faixa estreita demais recorta o número no meio e vira sujeira.
                  Abaixo do limite ela fica só como cor, e a contagem aparece na
                  legenda — nenhum dado se perde por não caber na barra. */}
              {faixa.cabeNaBarra ? rotuloFaixa(t, faixa) : null}
            </span>
          </Tooltip>
        ))}
      </div>
      <div className="mt-2 flex flex-wrap items-center gap-3 text-[10px] text-text-muted">
        {faixas.map((faixa) => (
          <MedalKey
            key={faixa.id}
            color={faixa.color}
            label={
              faixa.cabeNaBarra
                ? t(`myTeamTab.history.sport.spread.${faixa.id}`)
                : `${t(`myTeamTab.history.sport.spread.${faixa.id}`)} · ${rotuloFaixa(t, faixa)}`
            }
          />
        ))}
      </div>
    </div>
  );
}

// Cores da confiabilidade. Chegar ao fim é o estado bom e usa o verde do pódio;
// as duas causas de abandono precisam ser distinguíveis entre si porque a
// diferença entre elas é a pergunta do bloco — carro ruim ou piloto afoito?
const RELIABILITY_COLORS = {
  finished: "#3fbf7f",
  mechanical: "#e5793a",
  driverError: MEDAL_COLORS.nearMiss,
  other: "#3b4b5e",
};

// Confiabilidade: quantas largadas viraram chegada, e o que levou o resto ao box.
//
// É o buraco que a assinatura de resultados deixa. Lá, abandonar na volta 2 e
// terminar em 14º caem os dois em "fora do top 10" — e são coisas opostas: uma é
// o carro quebrando, a outra é o carro andando devagar.
export function ReliabilityPanel({ reliability, compacto = false }) {
  const { t } = useTranslation();
  if (!reliability || reliability.races <= 0) return null;
  const faixas = [
    { id: "finished", value: reliability.finished },
    { id: "mechanical", value: reliability.mechanical },
    { id: "driverError", value: reliability.driverError },
    { id: "other", value: reliability.other },
  ].filter((faixa) => faixa.value > 0);
  // A comparação com o grupo é o que dá escala ao número: 88% pode ser ótimo ou
  // medíocre conforme o carro, e só o grupo responde isso.
  const delta = reliability.finishRate - reliability.groupFinishRate;

  // Modo compacto: a MESMA gramática da assinatura de resultados — rótulo,
  // barra de 100% e legenda, nas mesmas alturas.
  //
  // No arranjo arrumado os dois blocos dividem uma linha, e enquanto este aqui
  // era um cartão com moldura e um número de 24px ao lado da barra, os dois liam
  // como dois gráficos de telas diferentes encostados: as barras em alturas
  // distintas, uma dentro de caixa e outra solta. Aqui a taxa de chegadas vira um
  // valor pequeno ao lado do rótulo e a barra desce para a mesma linha da outra —
  // a informação é idêntica, o peso é que deixa de brigar.
  if (compacto) {
    const rotulo = (faixa) => `${t(`myTeamTab.history.sport.rel${faixa.id[0].toUpperCase()}${faixa.id.slice(1)}`)} · ${faixa.value}`;
    return (
      <div data-testid="team-history-reliability">
        <div className="flex items-baseline gap-2">
          <BlockLabel>{t("myTeamTab.history.sport.reliability")}</BlockLabel>
          <strong
            className="font-mono text-[11px] font-semibold leading-none"
            style={{ color: RELIABILITY_COLORS.finished }}
            data-testid="team-history-finish-rate"
          >
            {`${reliability.finishRate}%`}
          </strong>
          <span className="font-mono text-[10px] text-text-muted">
            {t("myTeamTab.history.sport.reliabilityRaces", { value: reliability.races })}
          </span>
        </div>
        <div className="mt-2.5 flex h-7 overflow-hidden rounded-md">
          {faixas.map((faixa) => {
            const share = (faixa.value / reliability.races) * 100;
            return (
              <Tooltip key={faixa.id} texto={rotulo(faixa)}>
                <span
                  data-band={faixa.id}
                  className="flex items-center justify-center overflow-hidden whitespace-nowrap px-1 font-mono text-[10px] font-semibold"
                  style={{
                    flexGrow: faixa.value,
                    flexBasis: 0,
                    backgroundColor: RELIABILITY_COLORS[faixa.id],
                    color: corDeTextoSobre(RELIABILITY_COLORS[faixa.id]),
                  }}
                >
                  {/* Só a faixa que cabe mostra o número, pela mesma regra da
                      assinatura: recortado no meio ele vira sujeira. */}
                  {share >= FAIXA_MIN_ROTULO ? faixa.value : null}
                </span>
              </Tooltip>
            );
          })}
        </div>
        <div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-[10px] text-text-muted">
          {faixas.map((faixa) => (
            <MedalKey key={faixa.id} color={RELIABILITY_COLORS[faixa.id]} label={rotulo(faixa)} />
          ))}
          <span
            className="ml-auto font-mono"
            style={{ color: delta >= 0 ? RELIABILITY_COLORS.finished : RELIABILITY_COLORS.mechanical }}
          >
            {t("myTeamTab.history.sport.finishRateVs", { value: reliability.groupFinishRate })}
          </span>
        </div>
        {reliability.worstPart ? (
          <p className="mt-1.5 text-[10px] text-text-muted">
            {t("myTeamTab.history.sport.relWorstPart", { part: reliability.worstPart })}
          </p>
        ) : null}
      </div>
    );
  }

  return (
    <div data-testid="team-history-reliability">
      <div className="flex items-baseline gap-2">
        <BlockLabel>{t("myTeamTab.history.sport.reliability")}</BlockLabel>
        <span className="font-mono text-[10px] text-text-muted">
          {t("myTeamTab.history.sport.reliabilityRaces", { value: reliability.races })}
        </span>
      </div>
      <div className="mt-2.5 flex items-center gap-4 rounded-xl border border-white/[0.06] bg-[#0f1c2b] px-4 py-3">
        <div className="shrink-0">
          <strong
            className="block font-mono text-2xl leading-none tracking-[-0.02em]"
            style={{ color: RELIABILITY_COLORS.finished }}
            data-testid="team-history-finish-rate"
          >
            {`${reliability.finishRate}%`}
          </strong>
          <span className="mt-1 block text-[10px] text-text-muted">
            {t("myTeamTab.history.sport.finishRate")}
          </span>
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex h-5 overflow-hidden rounded-md">
            {faixas.map((faixa) => (
              <Tooltip
                key={faixa.id}
                texto={`${t(`myTeamTab.history.sport.rel${faixa.id[0].toUpperCase()}${faixa.id.slice(1)}`)} · ${faixa.value}`}
              >
                <span
                  data-band={faixa.id}
                  style={{ flexGrow: faixa.value, flexBasis: 0, backgroundColor: RELIABILITY_COLORS[faixa.id] }}
                />
              </Tooltip>
            ))}
          </div>
          <div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-[10px] text-text-muted">
            {faixas.map((faixa) => (
              <MedalKey
                key={faixa.id}
                color={RELIABILITY_COLORS[faixa.id]}
                label={`${t(`myTeamTab.history.sport.rel${faixa.id[0].toUpperCase()}${faixa.id.slice(1)}`)} · ${faixa.value}`}
              />
            ))}
            {/* A média do grupo fica na mesma linha da legenda, e não ao lado do
                número grande: ela é referência, não manchete. O sinal do delta
                vira cor — acima da média é verde, abaixo é laranja. */}
            <span
              className="ml-auto font-mono"
              style={{ color: delta >= 0 ? RELIABILITY_COLORS.finished : RELIABILITY_COLORS.mechanical }}
            >
              {t("myTeamTab.history.sport.finishRateVs", { value: reliability.groupFinishRate })}
            </span>
          </div>
        </div>
      </div>
      {reliability.worstPart ? (
        <p className="mt-1.5 text-[10px] text-text-muted">
          {t("myTeamTab.history.sport.relWorstPart", { part: reliability.worstPart })}
        </p>
      ) : null}
    </div>
  );
}
