import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Trophy } from "lucide-react";
import {
  BEST_DRIVERS_LIMIT,
  BEST_RANK_COLORS,
  MEDAL_COLORS,
  bestDriversRanking,
} from "./teamHistoryV2Logic";
import Tooltip from "../../ui/Tooltip";
import FlagIcon from "../../ui/FlagIcon";
import TeamLogoMark from "../TeamLogoMark";
import { BlockLabel } from "./teamHistoryV2Primitives.jsx";

// Pilotos do dossiê de equipe v2: a galeria de passagens por vaga e o ranking
// de quem melhor vestiu a equipe.
//
// Extraídos de `TeamHistoryDrawerV2.jsx` em 11/08/2026. Os dois listam as
// mesmas pessoas por critérios diferentes e trocam o realce entre si pelo par
// `pilotoAceso`/`onAcenderPiloto` — separá-los em arquivos distintos partiria
// esse elo no meio.
//
// Os dois blocos listam as MESMAS pessoas por critérios diferentes: a galeria em
// ordem de ano e por vaga, o ranking por currículo. Achar num o nome que está no
// outro é o gesto mais repetido dessa seção — e o mais caro, porque uma equipe
// antiga tem quinze passagens e o ranking corta em dez.
//
// O elo ACENDE e só. Uma versão anterior rolava a página até o par quando ele
// estava fora do quadro, para garantir que o realce fosse visto; a tela se
// mexendo sozinha sob o cursor é pior do que o problema que resolve — quem está
// lendo perde o lugar, e o gesto de passar o mouse deixa de ser inofensivo.

// Pilotos que passaram pela equipe, repartidos pelas DUAS vagas. É o único bloco
// do dossiê que fala de gente — todo o resto trata a equipe como um carro só — e,
// em duas colunas, também responde quanto essa casa troca de piloto: uma que
// manteve o mesmo titular por seis anos e outra que troca todo ano desenham
// diferente antes de qualquer número ser lido.
export function TeamLineup({ lineup, pilotoAceso = null, onAcenderPiloto = null }) {
  const { t } = useTranslation();
  if (!lineup?.length) return null;
  // Vagas 1 e 2 sempre lado a lado; a faixa de "outras passagens" só existe
  // quando alguém correu sem constar como titular de temporada arquivada.
  const colunas = [1, 2]
    .map((slot) => ({ slot, itens: lineup.filter((item) => item.slot === slot) }))
    .filter((coluna) => coluna.itens.length > 0);
  const avulsos = lineup.filter((item) => item.slot !== 1 && item.slot !== 2);
  return (
    <div data-testid="team-history-lineup">
      <div className="flex items-baseline gap-2">
        <BlockLabel>{t("myTeamTab.history.sport.alumni")}</BlockLabel>
        <span className="font-mono text-[10px] text-text-muted">
          {t("myTeamTab.history.sport.lineupCount", { value: lineup.length })}
        </span>
      </div>
      <div className="mt-2.5 grid gap-x-3 gap-y-3 sm:grid-cols-2">
        {colunas.map((coluna) => (
          <div key={coluna.slot} data-slot={coluna.slot}>
            <span className="block text-[10px] font-semibold text-text-muted">
              {t(`myTeamTab.history.sport.lineupSlot${coluna.slot}`)}
            </span>
            <LineupColumn itens={coluna.itens} pilotoAceso={pilotoAceso} onAcenderPiloto={onAcenderPiloto} />
          </div>
        ))}
        {avulsos.length ? (
          <div data-slot="0" className="sm:col-span-2">
            <span className="block text-[10px] font-semibold text-text-muted">
              {t("myTeamTab.history.sport.lineupSlotOther")}
            </span>
            <LineupColumn itens={avulsos} pilotoAceso={pilotoAceso} onAcenderPiloto={onAcenderPiloto} />
          </div>
        ) : null}
      </div>
    </div>
  );
}

// Uma coluna da galeria. As passagens vêm em ordem cronológica do backend, e a
// coluna só se lê como sucessão se essa ordem sobreviver ao desenho.
function LineupColumn({ itens, pilotoAceso = null, onAcenderPiloto = null }) {
  const { t } = useTranslation();
  return (
    <ul className="mt-1.5 grid gap-1.5">
      {itens.map((piloto) => {
          // A linha é sempre a mesma leitura: quanto correu, até onde chegou.
          // O melhor resultado vale para TODO mundo — era ele que separava quem
          // chegou perto de quem nunca ameaçou, e a contagem de pódios tomava o
          // lugar dele em quem tinha pódio. Pior: quem não tinha nada exibia
          // "0V · 0P", que é ruído com aparência de dado.
          // Titular que ainda não largou (save recém-criado, antes da rodada 1)
          // entra na galeria assim mesmo — mas "0 corridas · " é ruído: o que a
          // linha tem a dizer é que a passagem começou e a pista ainda não veio.
          const feitos =
            piloto.races > 0
              ? [t("myTeamTab.history.sport.alumniRaces", { value: piloto.races })]
              : [t("myTeamTab.history.sport.alumniNoRaces")];
          if (piloto.bestPosition > 0) {
            feitos.push(t("myTeamTab.history.sport.alumniBest", { value: piloto.bestPosition }));
          }
          // Vencer é a única coisa que a colocação sozinha não conta: "melhor P1"
          // não diz se foi uma vez ou dez. Só aparece quando houve vitória.
          if (piloto.wins > 0) {
            feitos.push(t("myTeamTab.history.sport.alumniWins", { value: piloto.wins }));
          }
          return (
            <li
              key={`${piloto.driverId}-${piloto.firstYear}`}
              data-driver={piloto.driverId}
              data-player={piloto.isPlayer ? "true" : undefined}
              data-current={piloto.stillHere ? "true" : undefined}
              data-aceso={pilotoAceso === piloto.driverId ? "true" : undefined}
              onMouseEnter={() => onAcenderPiloto?.(piloto.driverId)}
              onMouseLeave={() => onAcenderPiloto?.(null)}
              // Quem está na equipe HOJE é o fim da coluna e o começo da leitura:
              // ganha faixa lateral e fundo na cor da casa. Passagem encerrada
              // fica em cinza — a diferença entre as duas é a informação, e
              // "ainda na equipe" escrito em texto era fácil demais de perder no
              // meio de oito linhas iguais.
              className={`grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-x-3 gap-y-0.5 rounded-lg border px-3 py-2 transition-[box-shadow] ${
                piloto.stillHere
                  ? "border-l-2 border-[color-mix(in_srgb,var(--team)_50%,transparent)] border-l-[color:var(--team)] bg-[color-mix(in_srgb,var(--team)_10%,#0f1c2b)]"
                  : piloto.isPlayer
                    ? "border-[color-mix(in_srgb,var(--team)_45%,transparent)] bg-[color-mix(in_srgb,var(--team)_12%,#0f1c2b)]"
                    : "border-white/[0.06] bg-[#0f1c2b]"
              } ${pilotoAceso === piloto.driverId ? "ring-1 ring-white/45" : ""}`}
            >
              {/* A bandeira ocupa as duas linhas do cartão, à esquerda de tudo:
                  é o retrato do piloto que a galeria não tem. Vem do país porque
                  é o único traço visual que o save guarda dele — e é ele que faz
                  a coluna se ler como gente, e não como oito linhas de texto. */}
              <Tooltip texto={piloto.nationality || undefined}>
                <span
                  className="row-span-2 flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-white/[0.04] ring-1 ring-inset ring-white/[0.07]"
                  data-nationality={piloto.nationality || undefined}
                >
                  <FlagIcon nacionalidade={piloto.nationality} />
                </span>
              </Tooltip>
              <span className="flex min-w-0 items-center gap-1.5">
                <strong className="truncate text-xs font-semibold text-text-primary">{piloto.name}</strong>
                {piloto.isPlayer ? (
                  <span className="shrink-0 rounded px-1 py-px text-[10px] font-semibold text-[color:var(--team)] ring-1 ring-[color:var(--team)]/50">
                    {t("myTeamTab.history.sport.alumniYou")}
                  </span>
                ) : null}
                {piloto.stillHere ? (
                  <span className="shrink-0 rounded bg-[color:var(--team)] px-1 py-px text-[10px] font-semibold text-[#07101d]">
                    {t("myTeamTab.history.sport.lineupCurrent")}
                  </span>
                ) : null}
              </span>
              <span className="shrink-0 font-mono text-[10px] text-text-muted">
                {piloto.firstYear === piloto.lastYear
                  ? t("myTeamTab.history.sport.alumniOneYear", { first: piloto.firstYear })
                  : t("myTeamTab.history.sport.alumniYears", { first: piloto.firstYear, last: piloto.lastYear })}
              </span>
              <span className="truncate font-mono text-[10px] text-text-secondary">{feitos.join(" · ")}</span>
              {/* Quem foi para outra equipe aparece COM a equipe: brasão e cor.
                  "Hoje na GT Pro" dizia a categoria e escondia o que interessa —
                  para onde o piloto que a casa formou acabou indo. */}
              {piloto.currentTeamName ? (
                <span
                  className="flex min-w-0 items-center justify-end gap-1.5 text-[10px]"
                  data-current-team={piloto.currentTeamName}
                  style={{ color: piloto.currentTeamColor || undefined }}
                >
                  {/* Sem `scale`: transform não muda o espaço que o elemento
                      ocupa no fluxo, então o brasão de 36px continuava reservando
                      36px dentro de uma caixa de 20 e vazava por cima do nome. O
                      tamanho vem do próprio TeamLogoMark. */}
                  <TeamLogoMark teamName={piloto.currentTeamName} color={piloto.currentTeamColor} size="xs" />
                  <span className="truncate font-semibold">{piloto.currentTeamName}</span>
                </span>
              ) : piloto.stillHere ? null : (
                // Quem ficou não repete "ainda na equipe" aqui: o selo ao lado do
                // nome já diz, e a mesma frase duas vezes na linha só ocupa o
                // lugar de uma informação que não existe.
                <span className="truncate text-right text-[10px] text-text-muted">{piloto.currentLabel}</span>
              )}
            </li>
          );
        })}
    </ul>
  );
}

// A cronologia de marcos ("Momentos-chave") viveu na trajetória e saiu: cinco
// linhas de prosa que diziam o ano da estreia, do primeiro pódio e do último
// registro — datas de cartório, nenhuma delas sobre QUEM correu. O espaço é
// deste ranking, que responde a pergunta que o grupo faz no título.
//
// Quantos nomes o pódio da casa aguenta. Dez é o corte clássico de tabela de
// recordes, e numa equipe antiga ele alcança quem correu na década anterior — a
// galeria acima lista todo mundo, mas em ordem de ano, onde ninguém compara.
// As três colunas de números do ranking, na ordem em que desempatam a lista.
// Largura fixa e conteúdo alinhado à direita: é o que faz a coluna se ler de
// cima para baixo, que é a leitura que o bloco existe para dar.
// O título abre a fila porque é o primeiro critério de ordem, e leva a taça
// junto do número: as duas colunas de ouro seguidas (título e vitória) seriam
// indistinguíveis de relance, e a diferença entre elas é o assunto do bloco.
//
// A melhor colocação saiu: ela era redundante com as colunas à esquerda — quem
// tem pódio nunca terá "melhor" pior que P3, e quem tem vitória sempre marca P1.
// Só dizia algo de quem não subiu no pódio, e continua dizendo, como critério de
// desempate invisível. No lugar dela entram as CORRIDAS, que dão a escala das
// outras três (seis títulos em quarenta corridas não é seis em duzentas) e, por
// serem o filtro de entrada do ranking, garantem que nenhuma linha fique só com
// travessões.
const BEST_COLUMNS = [
  { id: "titles", label: "bestColTitles", width: "w-9", color: MEDAL_COLORS.first, trophy: true, value: (p) => p.titles },
  { id: "wins", label: "bestColWins", width: "w-8", color: MEDAL_COLORS.first, value: (p) => p.wins },
  { id: "podiums", label: "bestColPodiums", width: "w-8", color: MEDAL_COLORS.second, value: (p) => p.podiums },
  { id: "races", label: "bestColRaces", width: "w-10", color: null, value: (p) => p.races },
];

export function BestDrivers({ lineup, pilotoAceso = null, onAcenderPiloto = null }) {
  const { t } = useTranslation();
  const ranking = useMemo(() => bestDriversRanking(lineup ?? []), [lineup]);
  // Um nome sozinho não é ranking: a galeria acima já o mostra, com mais dado.
  if (ranking.length < 2) return null;
  const primeiros = ranking.slice(0, BEST_DRIVERS_LIMIT);

  return (
    <div data-testid="team-history-best-drivers">
      <div className="flex items-baseline gap-2">
        <BlockLabel>{t("myTeamTab.history.sport.bestDrivers")}</BlockLabel>
        <span className="text-[10px] text-text-muted">{t("myTeamTab.history.sport.bestDriversScope")}</span>
      </div>
      {/* O cabeçalho das colunas paga uma linha e devolve o que a prosa
          "1 vitória · 2 pódios" custava em cada uma das cinco: com ele, os
          números viram três colunas comparáveis de cima para baixo, e a ordem
          da lista fica visível em vez de precisar ser acreditada.
          `pr-[13px]` = o padding da linha mais a borda de 1px dela. */}
      <div className="mt-2.5 flex justify-end gap-3 pr-[13px] text-[9px] uppercase tracking-[0.08em] text-text-muted">
        {BEST_COLUMNS.map((coluna) => (
          <span key={coluna.id} className={`${coluna.width} text-right`}>
            {t(`myTeamTab.history.sport.${coluna.label}`)}
          </span>
        ))}
      </div>
      <ol className="mt-1 grid gap-1.5">
        {primeiros.map((piloto, index) => {
          const cor = BEST_RANK_COLORS[index] || MEDAL_COLORS.nearMiss;
          return (
            <li
              key={piloto.driverId}
              data-driver={piloto.driverId}
              data-rank={index + 1}
              data-player={piloto.isPlayer ? "true" : undefined}
              data-aceso={pilotoAceso === piloto.driverId ? "true" : undefined}
              onMouseEnter={() => onAcenderPiloto?.(piloto.driverId)}
              onMouseLeave={() => onAcenderPiloto?.(null)}
              className={`grid grid-cols-[18px_auto_minmax(0,1fr)_auto] items-center gap-x-3 rounded-lg border px-3 py-2 transition-[box-shadow] ${
                piloto.isPlayer
                  ? "border-[color-mix(in_srgb,var(--team)_45%,transparent)] bg-[color-mix(in_srgb,var(--team)_12%,#0f1c2b)]"
                  : "border-white/[0.06] bg-[#0f1c2b]"
              } ${pilotoAceso === piloto.driverId ? "ring-1 ring-white/45" : ""}`}
            >
              <strong className="text-center font-mono text-sm leading-none" style={{ color: cor }}>
                {index + 1}
              </strong>
              <FlagIcon nacionalidade={piloto.nationality} />
              <span className="flex min-w-0 items-center gap-1.5">
                <strong className="truncate text-xs font-semibold text-text-primary">{piloto.name}</strong>
                {piloto.isPlayer ? (
                  <span className="shrink-0 rounded px-1 py-px text-[10px] font-semibold text-[color:var(--team)] ring-1 ring-[color:var(--team)]/50">
                    {t("myTeamTab.history.sport.alumniYou")}
                  </span>
                ) : null}
                {piloto.stillHere ? (
                  <span className="shrink-0 rounded bg-[color:var(--team)] px-1 py-px text-[10px] font-semibold text-[#07101d]">
                    {t("myTeamTab.history.sport.lineupCurrent")}
                  </span>
                ) : null}
                <span className="shrink-0 font-mono text-[10px] text-text-muted">
                  {piloto.firstYear === piloto.lastYear
                    ? t("myTeamTab.history.sport.alumniOneYear", { first: piloto.firstYear })
                    : t("myTeamTab.history.sport.alumniYears", { first: piloto.firstYear, last: piloto.lastYear })}
                </span>
              </span>
              {/* Números em coluna, e não em prosa. A barra que morava aqui
                  media PÓDIOS, mas a lista ordena por título e vitória antes
                  disso: o 3º colocado, com quatro pódios e nada mais, ganhava a
                  barra mais longa e a figura desmentia a ordem que ela deveria
                  explicar. Nenhum comprimento resolve isso — o que ordena são
                  quatro critérios, e barra tem um eixo só. */}
              <span className="flex justify-self-end gap-3 font-mono text-[11px] text-text-secondary">
                {BEST_COLUMNS.map((coluna) => {
                  const valor = coluna.value(piloto);
                  if (!(valor > 0)) {
                    // Zero vira travessão apagado: "0" alinhado com os outros
                    // números pesa como dado e é ausência de dado.
                    return (
                      <span key={coluna.id} data-col={coluna.id} className={`${coluna.width} text-right text-text-muted/50`}>
                        {t("myTeamTab.history.defaults.dash")}
                      </span>
                    );
                  }
                  return (
                    <span
                      key={coluna.id}
                      data-col={coluna.id}
                      className={`${coluna.width} flex items-center justify-end gap-1`}
                      style={{ color: coluna.color || undefined }}
                    >
                      {coluna.trophy ? <Trophy size={10} strokeWidth={2} aria-hidden="true" /> : null}
                      {valor}
                    </span>
                  );
                })}
              </span>
            </li>
          );
        })}
      </ol>
    </div>
  );
}
