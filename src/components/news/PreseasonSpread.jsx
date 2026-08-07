import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import MagazineCredits from "./MagazineCredits";
import StandingsList from "./StandingsList";
import WorldNotes from "./WorldNotes";
import { renderBulletinParagraphs, splitBulletinParagraphs } from "./bulletinText";
import { getTrackImageSrc } from "../../utils/trackImages";

// ── Distribuição do texto no spread ──────────────────────────────────────────────
// A matéria de IA cresce com o tamanho do grid: num grid cheio ela passa de sete
// parágrafos, a página esquerda estica por duas telas e a direita fica um vazio.
// Acima deste tamanho a matéria CONTINUA na página direita, como numa revista.
const CORPO_LONGO_CHARS = 1500;
// A esquerda leva um pouco mais de texto porque a direita já carrega foto e tabela.
const FATIA_ESQUERDA = 0.55;

// ── Altura da lista × volume da matéria ─────────────────────────────────────────
// A página direita entra com foto, legenda e cabeçalho ANTES da lista; a esquerda só
// tem título e matéria. Com matéria curta, um grid de 12 pilotos em coluna única
// estica o spread para baixo à toa. Em vez de esconder piloto, a lista quebra em duas
// colunas e passa a caber na altura que o texto dá. A âncora é o texto, nunca o DOM:
// o mesmo cálculo vale já na primeira pintura, sem medir nem repintar.
const CHARS_POR_LINHA = 42; // corpo de 13px numa coluna de ~300px
const LINHA_CORPO_PX = 22; // line-height 1.7 + margem amortizada
const LINHA_LISTA_PX = 31; // .res-row + gap
// Quanto a direita já está mais baixa que a esquerda quando a lista começa.
const VANTAGEM_DIREITA_PX = 180;
// Abaixo disso a quebra em duas colunas fica ridícula (3 linhas de cada lado).
const MIN_LINHAS_PARA_QUEBRAR = 8;

// Altura estimada de um bloco de prosa rodando em duas colunas.
function alturaProsaPx(paragrafos) {
  const chars = paragrafos.reduce((soma, p) => soma + p.length, 0);
  return Math.ceil(chars / CHARS_POR_LINHA / 2) * LINHA_CORPO_PX;
}

// Título gigante com manchete longa se sobrepõe (Anton em line-height fechado).
// Quanto mais longa a manchete/olho, menor o corpo — o bloco continua com o mesmo peso.
function displayClass(headline, standfirst) {
  const h = headline?.length ?? 0;
  const s = standfirst?.length ?? 0;
  if (h > 64 || s > 170) return "display display--xdense";
  if (h > 38 || s > 105) return "display display--dense";
  return "display";
}

// "O Que Esperar": matéria de PRÉ-TEMPORADA. Só existe antes da 1ª corrida —
// substitui o "livro fechado" por um spread aberto. Ver docs/season-preview-design.md.
function PreseasonSpread({
  catLabel,
  year,
  preview,
  openingRace,
  standings,
  driverStandings,
  playerTeam,
  worldNotes,
}) {
  const { t } = useTranslation();
  // Classificação ao lado da matéria: alterna entre o grid de pilotos (padrão, é
  // quem a matéria cita) e as equipes.
  const [standingsView, setStandingsView] = useState("pilotos");
  const [hoveredDriverId, setHoveredDriverId] = useState(null);

  // Coluna direita da pré-temporada: as equipes da categoria com o campeonato
  // ainda ZERADO. Potencial é dado interno — não vira tabela pública.
  const construtores = useMemo(() => {
    return standings.map((tm, i) => ({
      id: tm.id,
      pos: tm.posicao ?? i + 1,
      name: tm.nome,
      pts: tm.pontos ?? 0,
      color: tm.cor_primaria || "#888",
      me: playerTeam?.id != null && tm.id === playerTeam.id,
    }));
  }, [standings, playerTeam]);

  // Grid de pilotos da pré-temporada. Sem pontos não há classificação: a ordem é a
  // da expectativa — segue a lista de construtores (que antes da 1ª corrida vem da
  // temporada passada), da equipe mais cotada para a menos, e companheiros de
  // equipe ficam lado a lado. Quem sobra (sem equipe ou de equipe fora da lista)
  // fecha o grid.
  const pilotos = useMemo(() => {
    const porEquipe = new Map();
    const semEquipe = [];
    driverStandings.forEach((d) => {
      if (d.equipe_id == null) {
        semEquipe.push(d);
        return;
      }
      const lista = porEquipe.get(d.equipe_id);
      if (lista) lista.push(d);
      else porEquipe.set(d.equipe_id, [d]);
    });

    const ordenados = [];
    construtores.forEach((tm) => {
      const doTime = porEquipe.get(tm.id);
      if (doTime) {
        ordenados.push(...doTime);
        porEquipe.delete(tm.id);
      }
    });
    porEquipe.forEach((lista) => ordenados.push(...lista));
    ordenados.push(...semEquipe);

    return ordenados.map((d, i) => ({
      id: d.id,
      pos: i + 1,
      name: d.nome,
      pts: d.pontos ?? 0,
      teamName: d.equipe_nome,
      color: d.equipe_cor || "#888",
      me: d.is_jogador,
    }));
  }, [driverStandings, construtores]);

  const mentionDrivers = useMemo(
    () => driverStandings.map((d) => ({ id: d.id, nome: d.nome })),
    [driverStandings],
  );
  // Nome realçado na matéria → acende o próprio piloto (view pilotos) ou a equipe
  // dele (view construtores).
  const hoveredTeamId = useMemo(
    () => driverStandings.find((d) => d.id === hoveredDriverId)?.equipe_id ?? null,
    [driverStandings, hoveredDriverId],
  );

  // Corte da matéria entre as duas páginas — por volume de texto, nunca no meio de um
  // parágrafo, e sempre deixando ao menos dois de cada lado (senão não é continuação).
  const [paragrafosEsq, paragrafosDir] = useMemo(() => {
    const paras = splitBulletinParagraphs(preview?.body);
    const total = paras.reduce((soma, p) => soma + p.length, 0);
    if (total <= CORPO_LONGO_CHARS || paras.length < 4) return [paras, []];
    let acumulado = 0;
    let corte = paras.length - 1;
    for (let i = 0; i < paras.length; i += 1) {
      acumulado += paras[i].length;
      if (acumulado >= total * FATIA_ESQUERDA) {
        corte = i + 1;
        break;
      }
    }
    corte = Math.min(Math.max(corte, 2), paras.length - 2);
    return [paras.slice(0, corte), paras.slice(corte)];
  }, [preview?.body]);

  // Coluna única enquanto a matéria sustentar a altura da lista; senão, duas colunas.
  // Uma continuação na página direita gasta altura que a lista deixa de ter.
  const gridEmDuasColunas = useMemo(() => {
    if (pilotos.length < MIN_LINHAS_PARA_QUEBRAR) return false;
    const continuacao = paragrafosDir.length > 0 ? alturaProsaPx(paragrafosDir) + 40 : 0;
    const disponivel = alturaProsaPx(paragrafosEsq) - VANTAGEM_DIREITA_PX - continuacao;
    return pilotos.length * LINHA_LISTA_PX > disponivel;
  }, [pilotos.length, paragrafosEsq, paragrafosDir]);

  const headline = preview?.headline || t("newsMagazine.preseason.titleLine");
  const standfirst = preview?.standfirst || t("newsMagazine.preseason.seasonLine", { year });

  return (
    <>
      <div className="spread">
        {/* PÁGINA ESQUERDA — a matéria "O Que Esperar" */}
        <div className="page page-l">
          <div className="flag" />
          <div className="kicker">
            {catLabel} · {t("newsMagazine.preseason.kicker")}
          </div>
          <h1 className={displayClass(headline, standfirst)}>
            <span className="l1">{headline}</span>
            <span className="l2">{standfirst}</span>
          </h1>
          <span className="ai-tag">
            {preview?.loading
              ? t("newsMagazine.preseason.aiTag.generating")
              : preview?.body
              ? t("newsMagazine.preseason.aiTag.ready")
              : t("newsMagazine.preseason.aiTag.comingSoon")}
          </span>

          <div className="prose-cols">
            <h3 className="subhead">{t("newsMagazine.preseason.articleHead")}</h3>
            {paragrafosEsq.length > 0 ? (
              renderBulletinParagraphs(
                paragrafosEsq,
                mentionDrivers,
                preview?.teams,
                hoveredDriverId,
                setHoveredDriverId,
                "esq",
              )
            ) : preview?.loading ? (
              <p>{t("newsMagazine.preseason.generating")}</p>
            ) : (
              <p>{t("newsMagazine.preseason.placeholder")}</p>
            )}
          </div>
        </div>

        {/* PÁGINA DIREITA — pista de abertura + construtores zerados */}
        <div className="page page-r">
          <MagazineCredits catLabel={catLabel} />
          {openingRace ? (
            <img
              className="photo"
              src={getTrackImageSrc(openingRace.track_name, openingRace.track_id)}
              alt={openingRace.track_name}
            />
          ) : (
            <div className="photo" />
          )}
          <p className="cap">
            {openingRace
              ? t("newsMagazine.preseason.caption", { track: openingRace.track_name })
              : t("newsMagazine.preseason.captionGeneric")}
          </p>

          <div className="r-grid r-grid-single">
            <div>
              <div className="nm-standings-head">
                <h3 className="subhead">
                  {standingsView === "construtores"
                    ? t("newsMagazine.standings.constructors")
                    : t("newsMagazine.standings.drivers")}
                  {year ? <> · {year}</> : null}
                </h3>
                <div className="nm-toggle">
                  <button
                    type="button"
                    className={`nm-toggle-btn${standingsView === "pilotos" ? " active" : ""}`}
                    onClick={() => setStandingsView("pilotos")}
                  >
                    {t("newsMagazine.standings.drivers")}
                  </button>
                  <button
                    type="button"
                    className={`nm-toggle-btn${standingsView === "construtores" ? " active" : ""}`}
                    onClick={() => setStandingsView("construtores")}
                  >
                    {t("newsMagazine.standings.constructors")}
                  </button>
                </div>
              </div>
              {standingsView === "construtores" ? (
                construtores.length > 0 ? (
                  <StandingsList
                    rows={construtores}
                    hoveredId={hoveredTeamId}
                    logoTestId="news-team-logo"
                  />
                ) : (
                  <p>{t("newsMagazine.standings.teamsUnavailable")}</p>
                )
              ) : pilotos.length > 0 ? (
                <StandingsList
                  rows={pilotos}
                  hoveredId={hoveredDriverId}
                  onHover={setHoveredDriverId}
                  split={gridEmDuasColunas}
                  logoTestId="news-driver-team-logo"
                />
              ) : (
                <p>{t("newsMagazine.standings.driversUnavailable")}</p>
              )}
            </div>
          </div>

          {paragrafosDir.length > 0 ? (
            <div className="prose-cols prose-cont">
              <h3 className="subhead">{t("newsMagazine.preseason.articleCont")}</h3>
              {renderBulletinParagraphs(
                paragrafosDir,
                mentionDrivers,
                preview?.teams,
                hoveredDriverId,
                setHoveredDriverId,
                "dir",
              )}
            </div>
          ) : null}
        </div>
      </div>

      <WorldNotes notes={worldNotes} />

      <div className="mag-foot">
        <div className="foot-left">
          <div className="brand">
            GRID<span>·</span>MAGAZINE
          </div>
        </div>
        <div className="foot-meta">
          {year ? t("newsMagazine.preseason.footSeason", { year }) : t("newsMagazine.preseason.foot")}
        </div>
      </div>
    </>
  );
}

export default PreseasonSpread;
