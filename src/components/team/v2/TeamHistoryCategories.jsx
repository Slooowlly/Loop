import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { getCategoryColor } from "../../../utils/categoryColors";
import Tooltip from "../../ui/Tooltip";
import { BlockLabel, MiniMetric, InfoCard } from "./teamHistoryV2Primitives.jsx";

// Seção Categorias do dossiê de equipe v2: a pirâmide da escada, a trajetória de
// nível ano a ano e o tempo acumulado em cada degrau.
//
// Extraída de `TeamHistoryDrawerV2.jsx` em 11/08/2026. Os três blocos leem a
// mesma escada (`ladder`) e só ela — nenhum deles conversa com o resto do
// dossiê.

export function CategoriesSection({ dossier }) {
  const { t } = useTranslation();
  const movement = dossier.movement ?? {};
  return (
    <section>
      <div className="grid gap-2.5 sm:grid-cols-2 lg:grid-cols-4">
        <MiniMetric label={t("myTeamTab.history.categories.promotions")} value={movement.promotions} />
        <MiniMetric label={t("myTeamTab.history.categories.relegations")} value={movement.relegations} />
        <MiniMetric label={t("myTeamTab.history.categories.peakCategory")} value={movement.peakCategory} />
        <MiniMetric label={t("myTeamTab.history.categories.homeCategory")} value={movement.homeCategory} />
      </div>
      <CategoryPyramid ladder={movement.ladder} />
      <CategoryTrajectory dossier={dossier} ladder={movement.ladder} />
      <CategoryTimeBars lines={movement.timeLines} fallback={movement.timeByCategory} />
    </section>
  );
}

// A escada INTEIRA do recorte, com os degraus nunca pisados apagados.
//
// A lista de passagens sozinha respondia "onde ela esteve" e escondia a pergunta
// que o jogo levanta o tempo todo: quanto falta para o topo. Uma estreante virava
// um card solitário — sem os dois degraus acima dele, não dava para ver que ela
// está no primeiro. A largura cresce para baixo porque a base é a categoria de
// entrada: o desenho é a pirâmide, não uma lista com título de pirâmide.
function CategoryPyramid({ ladder }) {
  const { t } = useTranslation();
  const degraus = Array.isArray(ladder) ? ladder : [];
  if (degraus.length === 0) return null;

  const doTopo = [...degraus].sort((a, b) => b.tier - a.tier);
  const passo = doTopo.length > 1 ? 42 / (doTopo.length - 1) : 0;

  return (
    <div className="mt-5" data-testid="team-history-category-pyramid">
      <BlockLabel>{t("myTeamTab.history.categories.ladder")}</BlockLabel>
      <div className="mt-2.5 flex flex-col items-center gap-1.5">
        {doTopo.map((degrau, index) => {
          const cor = getCategoryColor(degrau.categoryId) || "#58a6ff";
          const largura = 58 + passo * index;
          return (
            <div
              key={degrau.categoryId || degrau.category}
              data-category={degrau.categoryId || undefined}
              data-visited={degrau.visited ? "1" : "0"}
              className={`flex w-full items-center justify-between gap-3 rounded-lg px-3.5 py-2 ${
                degrau.visited ? "border-l-4" : "border border-dashed border-white/[0.08]"
              }`}
              style={{
                maxWidth: `${largura}%`,
                borderLeftColor: degrau.visited ? cor : undefined,
                backgroundColor: degrau.visited
                  ? `color-mix(in srgb, ${cor} 18%, transparent)`
                  : "transparent",
              }}
            >
              <div className="flex min-w-0 items-center gap-2">
                <strong
                  className={`truncate text-xs ${degrau.visited ? "text-text-primary" : "text-text-muted"}`}
                >
                  {degrau.category}
                </strong>
                {degrau.isCurrent ? (
                  <span
                    className="shrink-0 rounded px-1.5 py-0.5 text-[9px] font-black uppercase tracking-[0.12em]"
                    style={{ backgroundColor: cor, color: "#06101c" }}
                  >
                    {t("myTeamTab.history.categories.rungCurrent")}
                  </span>
                ) : null}
                {degrau.isPeak && !degrau.isCurrent ? (
                  <span className="shrink-0 font-mono text-[10px] text-accent-primary">
                    {t("myTeamTab.history.categories.rungPeak")}
                  </span>
                ) : null}
              </div>
              <span
                className={`shrink-0 font-mono text-[10px] ${degrau.visited ? "" : "text-text-muted"}`}
                style={degrau.visited ? { color: cor } : undefined}
              >
                {degrau.visited
                  ? `${t("myTeamTab.history.categories.rungSeasons", { count: degrau.seasons })} · ${degrau.years}`
                  : t("myTeamTab.history.categories.rungNever")}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

// Faixa ano a ano com ALTURA igual ao degrau. A pirâmide diz onde a equipe
// esteve; esta diz quando, e no mesmo gesto separa quem subiu degrau a degrau de
// quem passou dez anos parada na entrada — que na lista de passagens saíam iguais.
function CategoryTrajectory({ dossier, ladder }) {
  const { t } = useTranslation();
  const dados = useMemo(() => {
    const passagens = (dossier.categoryPath ?? []).filter((step) => step.startYear > 0);
    if (passagens.length === 0) return null;
    const foraDoRecorte = new Map(
      (dossier.outsideScopeSeasons ?? []).map((item) => [Number(item.year), item]),
    );

    const anos = passagens.flatMap((step) => [step.startYear, step.endYear]);
    const mundoInicio = Number(dossier.worldFirstYear ?? 0);
    const mundoFim = Number(dossier.worldLastYear ?? 0);
    const inicio = Math.min(...anos, ...(mundoInicio > 0 ? [mundoInicio] : []));
    const fim = Math.max(...anos, ...(mundoFim > 0 ? [mundoFim] : []));
    if (fim < inicio) return null;

    // A régua da altura é a ESCADA do recorte, não os degraus que esta equipe
    // pisou. Normalizar pela própria equipe punha a estreante de tier 0 na altura
    // máxima — desenhando "no topo" justamente quem está na base.
    const degraus = (Array.isArray(ladder) ? ladder : []).map((rung) => rung.tier);
    const tiers = degraus.length > 0 ? degraus : passagens.map((step) => step.tier);
    const tierMin = Math.min(...tiers);
    const tierMax = Math.max(...tiers);
    const faixa = Math.max(1, tierMax - tierMin + 1);

    const celulas = [];
    for (let ano = inicio; ano <= fim; ano += 1) {
      // A passagem MAIS RECENTE do ano ganha a célula: no ano da troca, pintar a
      // que estava saindo esconderia a subida.
      const passagem = [...passagens]
        .reverse()
        .find((step) => step.startYear <= ano && ano <= step.endYear);
      const fora = passagem ? null : foraDoRecorte.get(ano);
      celulas.push({
        year: ano,
        categoryId: passagem?.categoryId ?? null,
        label: passagem?.category ?? fora?.category ?? null,
        outside: Boolean(fora),
        // Altura proporcional ao degrau. O tier de quem está fora do recorte não
        // é conhecido aqui — a célula fica baixa e neutra, e o rótulo diz por quê.
        altura: passagem ? 12 + ((passagem.tier - tierMin + 1) / faixa) * 26 : 10,
      });
    }
    return { celulas };
  }, [dossier.categoryPath, dossier.outsideScopeSeasons, dossier.worldFirstYear, dossier.worldLastYear, ladder]);

  if (!dados) return null;
  const passo = Math.max(1, Math.ceil(dados.celulas.length / 10));

  return (
    <div className="mt-5" data-testid="team-history-category-trajectory">
      <BlockLabel>{t("myTeamTab.history.categories.trajectory")}</BlockLabel>
      <div className="mt-2.5 rounded-xl bg-[#0f1c2b] px-4 py-3.5">
        <div className="flex h-[38px] items-end gap-1">
          {dados.celulas.map((celula) => (
            <Tooltip
              key={celula.year}
              texto={
                celula.outside
                  ? t("myTeamTab.history.categories.trajectoryOutside", {
                      year: celula.year,
                      category: celula.label,
                    })
                  : celula.label
                    ? t("myTeamTab.history.categories.trajectoryYear", {
                        year: celula.year,
                        category: celula.label,
                      })
                    : t("myTeamTab.history.categories.trajectoryEmpty", { year: celula.year })
              }
            >
              <span
                data-year={celula.year}
                data-category={celula.categoryId || undefined}
                className="min-w-[8px] flex-1 rounded-t"
                style={{
                  height: `${celula.altura}px`,
                  backgroundColor: celula.categoryId
                    ? getCategoryColor(celula.categoryId)
                    : celula.outside
                      ? "#2c3a4c"
                      : "#141f2c",
                }}
              />
            </Tooltip>
          ))}
        </div>
        <div className="mt-1 flex gap-1">
          {dados.celulas.map((celula, index) => (
            <span
              key={`ano-${celula.year}`}
              className="min-w-[8px] flex-1 text-center font-mono text-[10px] text-text-muted"
            >
              {index % passo === 0 ? celula.year : ""}
            </span>
          ))}
        </div>
      </div>
    </div>
  );
}

// Uma linha por categoria: quanto tempo e o que a equipe fez lá.
//
// Era uma string concatenada com "·" — numa equipe de quinze anos e seis
// categorias, uma linha que ninguém lê, e ninguém compara "4 anos" com "9 anos"
// em texto tão rápido quanto em largura. O saldo veio para cá dos cards de
// passagem, que gastavam três linhas cada para repetir o que a pirâmide e a faixa
// ano a ano já desenham. Duas idas ao GT4 são UMA linha, somadas: a viagem é
// assunto da escada; aqui a pergunta é o que ela ganhou em cada degrau.
function CategoryTimeBars({ lines, fallback }) {
  const { t } = useTranslation();
  const linhas = Array.isArray(lines) ? lines : [];
  if (linhas.length === 0) {
    return fallback ? (
      <div className="mt-5">
        <InfoCard label={t("myTeamTab.history.categories.timeByCategory")} value={fallback} />
      </div>
    ) : null;
  }
  const maior = Math.max(...linhas.map((linha) => linha.seasons), 1);

  return (
    <div className="mt-5" data-testid="team-history-category-time">
      <BlockLabel>{t("myTeamTab.history.categories.byCategory")}</BlockLabel>
      <div className="mt-2.5 grid gap-2">
        {linhas.map((linha) => {
          const cor = getCategoryColor(linha.categoryId) || "#58a6ff";
          return (
            <Tooltip
              key={linha.categoryId || linha.category}
              texto={t("myTeamTab.history.categories.timeBarDetail", {
                count: linha.seasons,
                races: linha.races,
              })}
            >
              <div className="flex items-center gap-3">
                <span className="w-[34%] shrink-0 truncate text-[11px] text-text-secondary">
                  {linha.category}
                </span>
                <span className="h-2.5 flex-1 overflow-hidden rounded-full bg-white/[0.06]">
                  <span
                    className="block h-full rounded-full"
                    data-category={linha.categoryId || undefined}
                    style={{ width: `${(linha.seasons / maior) * 100}%`, backgroundColor: cor }}
                  />
                </span>
                <span
                  className="shrink-0 font-mono text-[10px] tabular-nums"
                  data-tally={linha.categoryId || undefined}
                >
                  <span style={{ color: cor }}>
                    {t("myTeamTab.history.categories.tallyWins", { count: linha.wins })}
                  </span>
                  <span className="text-text-muted">
                    {" · "}
                    {t("myTeamTab.history.categories.tallyPodiums", { count: linha.podiums })}
                  </span>
                </span>
              </div>
            </Tooltip>
          );
        })}
      </div>
    </div>
  );
}
