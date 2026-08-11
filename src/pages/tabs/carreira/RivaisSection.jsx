import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import { DriverDetailModal } from "../../../components/driver";
import { rivalInterestOf } from "../../../components/driver/RivalMarker.jsx";
import { formatCategoryLabel } from "../../../components/driver/detalhes/formatadores.js";
import useCareerStore from "../../../stores/useCareerStore";
import { getVividTeamColor } from "../../../utils/teamColors";
import { Bloco, Vazio } from "./primitivos.jsx";

// F-05 — a sala de rivalidades.
//
// Rivalidade nasceu no Loop como ADJETIVO: uma marcação que qualifica outras telas
// (o marcador no calendário, o chip ao lado do nome, a leitura da corrida). Nunca
// precisou virar substantivo, e por isso não existia lugar onde o jogador pudesse
// perguntar "quem são meus rivais, desde quando, e qual o placar".
//
// Duas fontes, e as duas já existiam:
//
//   • `detail.rivais.itens` — o placar medido: confronto direto na corrida e no
//     sábado, gap médio, tempo de box dividido, e onde o rival está hoje.
//   • `playerInterests` (do store) — quem é NEMESIS e quem é rival mostrado, mais o
//     nome que o motor deu à rivalidade ("A Revanche de Interlagos"). O papel é
//     relativo ao jogador, então ele não pode sair do payload do piloto.
//
// A ordem é a do backend (intensidade percebida), com o Nemesis puxado para o topo:
// ele é o único rival do jogo que tem histerese própria, e enterrá-lo no meio da
// lista por meio ponto de intensidade contaria a história errada.
// A chave crua do motor (`Colisao`, `Pista`) para a chave de i18n em snake_case. O
// mapa é o MESMO do `DriverDetailModalV2`; ele não é importado de lá porque aquele
// arquivo é o modal inteiro, e arrastá-lo para dentro desta tela por dois mapas de
// quatro linhas custaria mais do que repeti-los.
const ORIGENS = {
  Colisao: "colisao",
  Companheiros: "companheiros",
  Campeonato: "campeonato",
  Pista: "pista",
};

const NIVEIS = {
  atrito_leve: "atrito_leve",
  inicial: "inicial",
  clara: "clara",
  forte: "forte",
  intensa: "intensa",
};

function RivaisSection({ detail }) {
  const { t } = useTranslation();
  const interests = useCareerStore((state) => state.playerInterests);
  const [rivalAberto, setRivalAberto] = useState(null);
  const rivais = Array.isArray(detail.rivais?.itens) ? detail.rivais.itens : [];

  const ordenados = useMemo(() => {
    const comPapel = rivais.map((rival) => ({
      rival,
      interesse: rivalInterestOf(interests, rival.driver_id),
    }));
    return comPapel.sort((a, b) => {
      const pesoA = a.interesse?.role === "nemesis" ? 2 : a.interesse?.role === "rival" ? 1 : 0;
      const pesoB = b.interesse?.role === "nemesis" ? 2 : b.interesse?.role === "rival" ? 1 : 0;
      if (pesoA !== pesoB) return pesoB - pesoA;
      return (b.rival.intensidade ?? 0) - (a.rival.intensidade ?? 0);
    });
  }, [rivais, interests]);

  return (
    <>
      <Bloco
        titulo={t("carreiraTab.rivais.title")}
        testId="carreira-rivais"
        acao={
          ordenados.length ? (
            <span className="text-[11px] text-text-secondary">
              {t("carreiraTab.rivais.count", { count: ordenados.length })}
            </span>
          ) : null
        }
      >
        {ordenados.length ? (
          <ul className="grid gap-3">
            {ordenados.map(({ rival, interesse }) => (
              <RivalCard
                key={rival.driver_id}
                rival={rival}
                interesse={interesse}
                onAbrir={() => setRivalAberto(rival.driver_id)}
              />
            ))}
          </ul>
        ) : (
          <Vazio>{t("carreiraTab.rivais.empty")}</Vazio>
        )}
      </Bloco>

      {rivalAberto ? (
        <DriverDetailModal driverId={rivalAberto} onClose={() => setRivalAberto(null)} />
      ) : null}
    </>
  );
}

function RivalCard({ rival, interesse, onAbrir }) {
  const { t } = useTranslation();
  const corDaEquipe = getVividTeamColor(rival.equipe_cor || "");
  const isNemesis = interesse?.role === "nemesis";
  const confrontos = rival.confrontos ?? 0;
  const vitorias = rival.vitorias ?? 0;
  const derrotas = rival.derrotas ?? 0;
  // Vitórias + derrotas < confrontos sempre que alguém abandonou: abandono é
  // encontro, não é duelo. A barra desenha sobre os DUELOS decididos, senão a
  // soma nunca fecharia a largura e a fatia cinza se leria como empate.
  const decididos = vitorias + derrotas;
  const fatiaDeVitoria = decididos > 0 ? (vitorias / decididos) * 100 : 0;
  const anosDeBox = rival.companheirismo?.anos ?? [];

  return (
    <li
      className="rounded-xl border border-white/8 bg-black/15 px-4 py-3.5"
      style={{ borderLeftColor: corDaEquipe, borderLeftWidth: 3 }}
      data-testid="carreira-rival-card"
    >
      <div className="flex flex-wrap items-start justify-between gap-x-4 gap-y-2">
        <div className="min-w-0">
          <div className="flex min-w-0 flex-wrap items-baseline gap-x-2.5 gap-y-1">
            <button
              type="button"
              onClick={onAbrir}
              className="min-w-0 truncate text-base font-semibold text-text-primary transition-glass hover:text-accent-primary"
            >
              {rival.nome}
            </button>
            <span
              className={`shrink-0 rounded-full px-2 py-0.5 text-[10px] font-bold uppercase tracking-[0.12em] ${
                isNemesis
                  ? "bg-status-red/15 text-status-red"
                  : "bg-white/8 text-text-secondary"
              }`}
            >
              {isNemesis ? t("carreiraTab.rivais.nemesis") : t("carreiraTab.rivais.rival")}
            </span>
          </div>
          <div className="mt-1 truncate text-xs text-text-secondary">
            {rival.equipe_nome ? (
              <span style={{ color: corDaEquipe }}>{rival.equipe_nome}</span>
            ) : (
              <span className="text-text-muted">{t("carreiraTab.rivais.noTeam")}</span>
            )}
            {rival.categoria_atual ? ` · ${formatCategoryLabel(rival.categoria_atual)}` : ""}
            {/* Rivalidade que morreu por promoção conta outra história: o placar
                fica de pé, o próximo capítulo não vem. */}
            {rival.categoria_atual && !rival.mesma_categoria
              ? ` · ${t("carreiraTab.rivais.otherGrid")}`
              : ""}
          </div>
          {/* O nome que o motor deu à rivalidade só existe depois do primeiro
              capítulo registrado — sem episódio, o par é intensidade e nada mais. */}
          {interesse?.label ? (
            <p className="mt-1.5 text-xs italic text-text-muted">{interesse.label}</p>
          ) : null}
        </div>

        <div className="shrink-0 text-right">
          <div
            className="font-mono text-xl font-semibold leading-none text-text-primary"
            style={{ fontVariantNumeric: "tabular-nums" }}
          >
            {vitorias}
            <span className="text-text-muted"> - </span>
            {derrotas}
          </div>
          <div className="mt-1.5 text-[10px] font-semibold uppercase tracking-[0.14em] text-text-muted">
            {t("carreiraTab.rivais.raceScore")}
          </div>
        </div>
      </div>

      {decididos > 0 ? (
        <div className="mt-3 flex h-1.5 overflow-hidden rounded-full bg-status-red/25">
          <div className="h-full bg-status-green" style={{ width: `${fatiaDeVitoria}%` }} />
        </div>
      ) : null}

      <dl className="mt-3 grid gap-x-5 gap-y-1.5 border-t border-white/8 pt-3 sm:grid-cols-2 lg:grid-cols-4">
        <Fato
          rotulo={t("carreiraTab.rivais.meetings")}
          valor={t("carreiraTab.rivais.meetingsValue", { count: confrontos })}
        />
        <Fato
          rotulo={t("carreiraTab.rivais.qualiScore")}
          valor={`${rival.vitorias_quali ?? 0} - ${rival.derrotas_quali ?? 0}`}
        />
        {/* Nível e origem reusam as chaves da ficha (`driverDetail.rivals.*`) de
            propósito: a faixa de intensidade sai de `rivalry::intensity_level` no
            Rust, e um segundo jogo de rótulos no JS divergiria do primeiro na
            primeira recalibração. */}
        <Fato
          rotulo={t("carreiraTab.rivais.intensity")}
          valor={t(`driverDetail.rivals.levels.${NIVEIS[rival.nivel_chave] || "atrito_leve"}`)}
        />
        <Fato
          rotulo={t("carreiraTab.rivais.origin")}
          valor={t(`driverDetail.rivals.origins.${ORIGENS[rival.tipo] || "pista"}`)}
        />
        {/* Gap médio positivo = o RIVAL à frente, a mesma convenção do backend.
            É o que separa uma derrota de fotografia de uma surra: 12–33 pode ser
            meio segundo ou meia volta, e o placar não distingue. */}
        {Number.isFinite(rival.gap_medio) ? (
          <Fato
            rotulo={t("carreiraTab.rivais.averageGap")}
            valor={
              rival.gap_medio >= 0
                ? t("carreiraTab.rivais.gapBehind", { value: rival.gap_medio.toFixed(1) })
                : t("carreiraTab.rivais.gapAhead", { value: Math.abs(rival.gap_medio).toFixed(1) })
            }
          />
        ) : null}
        {/* Box dividido é a ÚNICA comparação sem o carro no meio, e por isso vale
            mais que todo o resto junto. */}
        {rival.companheirismo && anosDeBox.length ? (
          <Fato
            rotulo={t("carreiraTab.rivais.teammate")}
            valor={t("carreiraTab.rivais.teammateValue", {
              team: rival.companheirismo.equipe || t("carreiraTab.rivais.noTeam"),
              years:
                anosDeBox.length > 1
                  ? `${anosDeBox[0]} - ${anosDeBox[anosDeBox.length - 1]}`
                  : String(anosDeBox[0]),
              wins: rival.companheirismo.vitorias ?? 0,
              losses: rival.companheirismo.derrotas ?? 0,
            })}
          />
        ) : null}
      </dl>
    </li>
  );
}

function Fato({ rotulo, valor }) {
  return (
    <div className="flex items-baseline justify-between gap-3">
      <dt className="text-[10px] uppercase tracking-[0.14em] text-text-muted">{rotulo}</dt>
      <dd className="text-right text-xs font-medium text-text-primary">{valor}</dd>
    </div>
  );
}

export default RivaisSection;
