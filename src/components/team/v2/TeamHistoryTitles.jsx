import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Crown } from "lucide-react";
import { getCategoryColor } from "../../../utils/categoryColors";
import Tooltip from "../../ui/Tooltip";
import { MEDAL_COLORS } from "./teamHistoryV2Logic";
import { MedalKey, BlockLabel } from "./teamHistoryV2Primitives.jsx";

// Galeria de títulos do dossiê de equipe v2: a régua de anos, o grupo por
// categoria e a linha do campeão de pilotos.
//
// Extraída de `TeamHistoryDrawerV2.jsx` em 11/08/2026. Entra pela seção Records
// e não conversa com nenhum outro painel: recebe os títulos, as temporadas e o
// par de realce do ano, e devolve a galeria.

// Galeria de títulos.
//
// A versão em cards repetia em SEIS cards a mesma categoria, a mesma contagem de
// vitórias e a mesma frase sobre o mesmo piloto — e, de tanto repetir, escondia
// o que estava acontecendo: seis títulos SEGUIDOS, quatro deles com dobradinha
// do mesmo piloto. Isso é uma dinastia, e o layout contava como seis fatos
// soltos.
//
// Aqui o que se repete virou cabeçalho (a categoria, uma vez, com o resumo do
// reinado) e o que varia virou coluna. Repetição é ilegível espalhada em cards e
// legível empilhada numa coluna: a de pontos passa a mostrar a equipe ganhando
// por menos a cada ano, que nos cards era impossível de ver.
export function TitleGallery({ titles, seasons, anoAceso = null, onAcenderAno = null }) {
  const { t } = useTranslation();
  const dados = useMemo(() => {
    const lista = (Array.isArray(titles) ? titles : []).filter((item) => item.year);
    if (!lista.length) return null;

    const anosTitulo = new Map();
    for (const titulo of lista) {
      anosTitulo.set(Number(titulo.year), titulo);
    }
    // A régua cobre TODAS as temporadas da equipe, não só as de título: sem os
    // anos vazios em volta, seis títulos seguidos desenhariam igual a seis
    // títulos espalhados, que é a diferença entre um reinado e uma coleção.
    const anosCorridos = (Array.isArray(seasons) ? seasons : [])
      .filter((row) => Number(row.races) > 0)
      .map((row) => Number(row.year));
    const todos = [...anosTitulo.keys(), ...anosCorridos];
    const inicio = Math.min(...todos);
    const fim = Math.max(...todos);
    const regua = [];
    for (let ano = inicio; ano <= fim; ano += 1) {
      regua.push({ year: ano, title: anosTitulo.get(ano) ?? null });
    }

    // Um grupo por categoria, na ordem do primeiro título de cada uma.
    const grupos = [];
    for (const titulo of [...lista].sort((a, b) => Number(a.year) - Number(b.year))) {
      const chave = titulo.categoryId || titulo.category;
      let grupo = grupos.find((item) => item.key === chave);
      if (!grupo) {
        grupo = { key: chave, category: titulo.category, categoryId: titulo.categoryId, rows: [] };
        grupos.push(grupo);
      }
      grupo.rows.push(titulo);
    }

    return { regua, grupos };
  }, [titles, seasons]);

  if (!dados) return null;

  // Um título só ganha a MESMA tela de quem tem seis: régua, cabeçalho e tabela.
  //
  // A versão anterior colapsava o caso de um título numa linha, com o argumento
  // de que três níveis de moldura para um fato é mais chrome que conteúdo. Estava
  // errado por dois motivos. O primeiro é que a régua é MAIS informativa aí: ela
  // mostra o único ano que importou dentro de treze temporadas, e um título
  // isolado numa carreira longa é uma história melhor que seis seguidos. O
  // segundo é que dar a tela boa só para a dinastia premia quem já tem muito, e
  // o dossiê existe para contar a história de qualquer equipe do grid.
  return (
    <div className="mt-5" data-testid="team-history-title-gallery">
      <BlockLabel>{t("myTeamTab.history.records.titleGallery")}</BlockLabel>
      <div className="mt-2.5 flex gap-1" data-testid="team-history-title-rail">
        {dados.regua.map((celula) => {
          const cor = celula.title
            ? getCategoryColor(celula.title.categoryId) || celula.title.color
            : null;
          const ano = String(celula.year);
          const aceso = anoAceso === ano;
          // Os dois anéis convivem numa propriedade só: o dourado por DENTRO
          // continua sendo a dobradinha, o branco por fora é o ano aceso. Como
          // o de fora não ocupa espaço de layout, a régua não se mexe ao acender.
          const aneis = [
            celula.title?.championIsTeam ? `inset 0 0 0 1.5px ${MEDAL_COLORS.first}` : null,
            aceso ? "0 0 0 1px rgba(255,255,255,0.55)" : null,
          ].filter(Boolean);
          return (
            <Tooltip
              key={celula.year}
              texto={
                celula.title
                  ? `${celula.year} · ${celula.title.category}`
                  : t("myTeamTab.history.records.titleRailEmpty", { year: celula.year })
              }
            >
              <span
                data-year={celula.year}
                data-title={celula.title ? "true" : undefined}
                data-double={celula.title?.championIsTeam ? "true" : undefined}
                data-aceso={aceso ? "true" : undefined}
                onMouseEnter={() => onAcenderAno?.(ano)}
                onMouseLeave={() => onAcenderAno?.(null)}
                className="h-5 min-w-[10px] flex-1 rounded transition-[box-shadow]"
                style={{
                  backgroundColor: cor || "#141f2c",
                  boxShadow: aneis.length ? aneis.join(", ") : undefined,
                }}
              />
            </Tooltip>
          );
        })}
      </div>
      <TitleRailYears years={dados.regua.map((celula) => celula.year)} />
      <div className="mt-2 flex flex-wrap items-center gap-3 text-[10px] text-text-muted">
        <MedalKey color={dados.grupos[0].categoryId ? getCategoryColor(dados.grupos[0].categoryId) : "#8020D0"} label={t("myTeamTab.history.records.titleRailKey")} />
        <span className="flex items-center gap-1.5">
          <span
            className="h-2.5 w-2.5 rounded-[3px]"
            style={{
              backgroundColor: dados.grupos[0].categoryId ? getCategoryColor(dados.grupos[0].categoryId) : "#8020D0",
              boxShadow: `inset 0 0 0 1.5px ${MEDAL_COLORS.first}`,
            }}
          />
          {t("myTeamTab.history.records.titleRailDoubleKey")}
        </span>
      </div>

      {dados.grupos.map((grupo) => (
        <TitleGroup key={grupo.key} grupo={grupo} />
      ))}
    </div>
  );
}

// Rótulos de ano da régua: um a cada N células, para caber sem virar borrão.
function TitleRailYears({ years }) {
  const passo = Math.max(1, Math.ceil(years.length / 7));
  return (
    <div className="mt-1 flex gap-1">
      {years.map((year, index) => (
        <span key={year} className="min-w-[10px] flex-1 text-center font-mono text-[10px] text-text-muted">
          {index % passo === 0 ? year : ""}
        </span>
      ))}
    </div>
  );
}

function TitleGroup({ grupo }) {
  const { t } = useTranslation();
  const cor = grupo.categoryId ? getCategoryColor(grupo.categoryId) : "#58a6ff";
  const anos = grupo.rows.map((row) => Number(row.year));
  const dobradinhas = grupo.rows.filter((row) => row.championIsTeam).length;
  const span = anos.length > 1 ? `${Math.min(...anos)}–${Math.max(...anos)}` : String(anos[0]);
  return (
    <div className="mt-4" data-testid="team-history-title-group" data-category={grupo.categoryId || undefined}>
      <div className="flex flex-wrap items-baseline gap-x-2 border-l-[3px] pl-2.5" style={{ borderLeftColor: cor }}>
        <strong className="text-sm font-semibold text-text-primary">{grupo.category}</strong>
        <span className="text-[11px] text-text-secondary">
          {t("myTeamTab.history.records.titleCount", { count: grupo.rows.length })}
          {" · "}
          {span}
          {dobradinhas > 0
            ? ` · ${t("myTeamTab.history.records.titleDoubleCount", { count: dobradinhas })}`
            : ""}
        </span>
      </div>
      <div className="mt-2 overflow-hidden rounded-lg border border-white/10">
        <div className="grid grid-cols-[52px_60px_34px_minmax(0,1fr)] gap-x-3 bg-[#0f1c2b] px-3.5 py-1.5 text-[10px] font-semibold text-text-secondary">
          <span>{t("myTeamTab.history.sport.cols.year")}</span>
          <span className="text-right">{t("myTeamTab.history.sport.cols.points")}</span>
          <span className="text-right">{t("myTeamTab.history.sport.cols.wins")}</span>
          <span>{t("myTeamTab.history.records.titleChampionCol")}</span>
        </div>
        {grupo.rows.map((row) => (
          <div
            key={row.year}
            data-title-year={row.year}
            data-double={row.championIsTeam ? "true" : undefined}
            className="grid grid-cols-[52px_60px_34px_minmax(0,1fr)] items-center gap-x-3 border-t border-white/[0.06] px-3.5 py-1.5 text-xs"
          >
            <span className="font-mono font-bold text-[color:var(--team)]">{row.year}</span>
            {/* Aqui os pontos fazem sentido, ao contrário do gráfico: são de uma
                temporada só, e a coluna os empilha para comparação direta. */}
            <span className="text-right font-mono text-text-primary">{row.points}</span>
            <span className="text-right font-mono text-text-primary">{row.wins}</span>
            <ChampionLine title={row} />
          </div>
        ))}
      </div>
    </div>
  );
}

// O campeão de PILOTOS daquele ano — outro campeonato, que pode ter ido para
// outra casa. A frase "campeão de pilotos" não se repete linha a linha: o
// cabeçalho da coluna já diz isso, e o que sobra é o nome. A coroa acesa em
// dourado faz o trabalho que a frase fazia.
function ChampionLine({ title }) {
  if (!title.championDriver) return <span />;
  const dobradinha = title.championIsTeam;
  return (
    <span className="flex min-w-0 items-center gap-1.5">
      <Crown
        size={12}
        strokeWidth={1.8}
        aria-hidden="true"
        className="shrink-0"
        style={{ color: dobradinha ? MEDAL_COLORS.first : MEDAL_COLORS.nearMiss }}
      />
      <span className={`truncate ${dobradinha ? "text-status-yellow" : "text-text-secondary"}`}>
        {title.championDriver}
        {!dobradinha && title.championTeam ? (
          <span className="text-text-muted">{` · ${title.championTeam}`}</span>
        ) : null}
      </span>
    </span>
  );
}
