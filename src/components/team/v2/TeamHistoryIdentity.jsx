import { useTranslation } from "react-i18next";
import { getVividTeamColor } from "../../../utils/teamColors";
import { formatMeetingAge } from "./teamHistoryV2Labels";
import TeamLogoMark from "../TeamLogoMark";
import FlagIcon from "../../ui/FlagIcon";
import { BlockLabel, MiniMetric } from "./teamHistoryV2Primitives.jsx";

// Seção Rival do dossiê de equipe v2: o duelo com a equipe mais enfrentada, o
// perfil de recrutamento e a afinidade por pista.
//
// Extraída de `TeamHistoryDrawerV2.jsx` em 11/08/2026. O id da seção é
// `identity` e o rótulo é "Rival" — a divergência é intencional e está explicada
// em `TEAM_HISTORY_SECTIONS`, no drawer.

export function IdentitySection({ dossier }) {
  const { t } = useTranslation();
  const identity = dossier.identity;
  // Origem e atual eram dois cards com a mesma palavra na maioria das equipes —
  // metade da aba gasta para dizer "Mazda Rookie" duas vezes. Viraram um card só,
  // e o caso "nunca saiu" passou a ser afirmação de identidade em vez de repetição.
  const rooted = identity.origin === identity.current;
  const steps = dossier.categoryPath?.length ?? 0;
  const podiumRate = identity.profileRaces
    ? Math.round(((identity.profilePodiums ?? 0) / identity.profileRaces) * 100)
    : null;

  return (
    <section className="grid gap-2.5">
      {/* O duelo abre a aba porque é o assunto dela. Os fatos da equipe descem
          para a faixa de apoio: aqui eles existem para dar contexto a quem está
          do outro lado, não para retratar a casa — isso é papel da aba vizinha. */}
      <DuelPanel dossier={dossier} />

      {/* Perfil ocupa a largura toda: é o único bloco com prosa de verdade, e a
          meia-largura quebrava a frase em quatro linhas enquanto o vizinho tinha
          duas. Os números sobem para a mesma linha do rótulo. */}
      <div className="rounded-xl border border-[color-mix(in_srgb,var(--team)_38%,transparent)] bg-[#0c1626] px-4 py-3.5">
        <BlockLabel>{t("myTeamTab.history.identity.profileLabel")}</BlockLabel>
        <strong className="mt-1 block text-lg font-semibold leading-tight tracking-[-0.01em] text-text-primary">
          {identity.profile}
        </strong>
        <p className="mt-1 text-xs leading-5 text-text-secondary">{identity.summary}</p>
        {/* Os números que sustentam o rótulo viram métricas, não uma linha de mono
            solta: cada um ganha rótulo próprio e alinha em coluna com os vizinhos.
            Corridas é o denominador, e sem ele "33% de pódio" não quer dizer nada. */}
        {identity.profileRaces ? (
          <div className="mt-3 grid grid-cols-2 gap-2 md:grid-cols-4">
            <FactTile label={t("myTeamTab.history.identity.tile.races")} value={identity.profileRaces} />
            <FactTile label={t("myTeamTab.history.identity.tile.wins")} value={identity.profileWins ?? 0} />
            <FactTile label={t("myTeamTab.history.identity.tile.podiums")} value={identity.profilePodiums ?? 0} />
            <FactTile label={t("myTeamTab.history.identity.tile.podiumRate")} value={`${podiumRate}%`} />
          </div>
        ) : null}
        {/* Mesma barra de proporção que Records usa em cada card: é ela que põe a
            cor da equipe na tela e dá ao rótulo uma medida em vez de só prosa.
            Aqui enche com a taxa de pódio, que é o número que sustenta o perfil. */}
        {podiumRate != null ? <TeamShareBar value={podiumRate} /> : null}
      </div>

      {/* Três cards do mesmo peso numa linha só. Antes a trajetória dividia a
          fileira com a pilha de duas pistas e esticava até a altura delas — um
          card gigante com duas linhas de texto dentro. */}
      <div className="grid gap-2.5 md:grid-cols-3">
        <div className="rounded-xl border border-[color-mix(in_srgb,var(--team)_32%,transparent)] bg-[#0c1626]/95 px-4 py-3.5">
          <div className="flex items-baseline justify-between gap-2">
            <BlockLabel>{t("myTeamTab.history.identity.symbolLabel")}</BlockLabel>
            {identity.symbolDriverYears ? (
              <span className="shrink-0 font-mono text-[11px] font-semibold text-text-secondary">
                {identity.symbolDriverYears}
              </span>
            ) : null}
          </div>
          <div className="mt-1.5 flex flex-wrap items-center gap-x-2 gap-y-1">
            {identity.symbolDriverNationality ? (
              <FlagIcon nacionalidade={identity.symbolDriverNationality} />
            ) : null}
            <strong className="text-[15px] font-semibold leading-tight text-text-primary">
              {identity.symbolDriver}
            </strong>
            {/* Um símbolo que ficou e um que foi embora contam histórias opostas —
                o card dizia a mesma coisa nos dois casos. */}
            {identity.symbolDriverYears ? (
              <span
                className={`rounded-md px-1.5 py-0.5 text-[10px] font-semibold ${
                  identity.symbolDriverActive
                    ? "bg-status-green/15 text-status-green"
                    : "bg-white/10 text-text-secondary"
                }`}
              >
                {identity.symbolDriverActive
                  ? t("myTeamTab.history.identity.symbolStillHere")
                  : t("myTeamTab.history.identity.symbolGone")}
              </span>
            ) : null}
          </div>
          {/* A prosa "5 corridas, 1 vitória, 2 pódios" virou métrica: os três
              cards da fileira passam a ter a mesma anatomia, e era a falta disso
              que fazia dois deles sobrarem espaço enquanto o terceiro enchia. */}
          <div className="mt-3 grid grid-cols-3 gap-2">
            <FactTile label={t("myTeamTab.history.identity.tile.races")} value={identity.symbolDriverRaces ?? 0} />
            <FactTile label={t("myTeamTab.history.identity.tile.wins")} value={identity.symbolDriverWins ?? 0} />
            <FactTile label={t("myTeamTab.history.identity.tile.podiums")} value={identity.symbolDriverPodiums ?? 0} />
          </div>
        </div>

        <div className="rounded-xl border border-[color-mix(in_srgb,var(--team)_32%,transparent)] bg-[#0c1626]/95 px-4 py-3.5">
          <BlockLabel>{t("myTeamTab.history.identity.trajectoryLabel")}</BlockLabel>
          {/* O degrau ATUAL vai na cor da equipe e a origem fica apagada: é a cor
              dizendo onde a equipe está, não enfeitando a linha. */}
          <strong className="mt-1.5 block text-[15px] font-semibold leading-tight text-text-primary">
            {rooted ? null : (
              <>
                <span className="text-text-muted">{identity.origin}</span>
                <span className="px-1.5 text-text-muted">→</span>
              </>
            )}
            <span className="text-[color:var(--team)]">{identity.current}</span>
          </strong>
          {/* Contagem de temporadas vem de `seasonResults`, que é lista:
              `sport.seasons` já chega formatado como prosa ("4 Temporadas"). */}
          <div className="mt-3 grid grid-cols-2 gap-2">
            <FactTile
              label={t("myTeamTab.history.identity.tile.seasons")}
              value={dossier.seasonResults?.length ?? 0}
            />
            <FactTile label={t("myTeamTab.history.identity.tile.steps")} value={steps} />
          </div>
        </div>

        {identity.recruitment ? <RecruitmentCard recruitment={identity.recruitment} /> : null}
      </div>

      {identity.bestTrack && identity.worstTrack ? (
        <div className="grid gap-2.5 md:grid-cols-2">
          <TrackAffinityCard affinity={identity.bestTrack} favourite />
          <TrackAffinityCard affinity={identity.worstTrack} />
        </div>
      ) : null}
      {dossier.ownershipEvents?.length > 0 && (
        <div className="rounded-xl border border-white/10 bg-[#0c1626]/95 p-4">
          <BlockLabel>{t("myTeamTab.history.identity.erasLabel")}</BlockLabel>
          <ul className="mt-2.5 grid gap-2.5 md:grid-cols-2">
            {dossier.ownershipEvents.map((event, index) => (
              <li key={index} className="flex items-start gap-3">
                <span className="mt-0.5 font-mono text-xs font-bold text-[color:var(--team)]">{event.year}</span>
                <div className="min-w-0">
                  <strong className="block text-xs font-semibold text-text-primary">{event.title}</strong>
                  <p className="text-[11px] leading-5 text-text-secondary">{event.detail}</p>
                </div>
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}

// Métrica de card: rótulo em cima, número em mono embaixo. É a mesma anatomia do
// `MiniMetric` do cabeçalho, em corpo menor — e é ela que dá aos cards da fileira
// uma altura comum, porque prosa não alinha em coluna e número alinha.
function FactTile({ label, value }) {
  return (
    <div className="rounded-lg bg-[#0f1c2b] px-2.5 py-2">
      {/* Rótulo QUEBRA em vez de truncar: numa coluna estreita "Anos na chegada"
          virava "Anos na c…", e rótulo cortado não informa nada. Como os tiles
          vivem num grid, a segunda linha estica todos juntos e o alinhamento
          entre eles se mantém. */}
      <span className="block text-[11px] font-semibold leading-tight text-text-secondary">{label}</span>
      <strong className="mt-1 block font-mono text-base leading-none text-text-primary">{value}</strong>
    </div>
  );
}

// O duelo: as duas equipes se encarando, com o placar do confronto direto no meio.
//
// A aba antes empilhava seis cards do mesmo tamanho e nenhum era o assunto. Aqui o
// olho cai no centro, e cada lado carrega a cor de quem representa — a casa em
// `--team`, o adversário em `--rival`. Sem rival consolidado o painel desce para
// um estado enxuto em vez de desenhar um duelo que não existe.
function DuelPanel({ dossier }) {
  const { t } = useTranslation();
  const rival = dossier.identity.rival;
  const hasRival = Boolean(rival.color);
  const hasAxes = rival.historicalIntensity != null && rival.recentActivity != null;
  const clashes = rival.headToHeadWins + rival.headToHeadLosses;
  const rivalColor = hasRival ? getVividTeamColor(rival.color) : "var(--team)";

  if (!hasRival) {
    return (
      <div className="rounded-xl border border-white/10 bg-[#0c1626]/95 p-4">
        <BlockLabel>{t("myTeamTab.history.identity.rivalLabel")}</BlockLabel>
        <strong className="mt-1.5 block text-sm font-semibold text-text-primary">{rival.name}</strong>
        <p className="mt-1.5 text-[11px] leading-5 text-text-secondary">{rival.note}</p>
      </div>
    );
  }

  return (
    <div
      className="rounded-xl border border-white/10 bg-[#0c1626] p-4"
      style={{
        "--rival": rivalColor,
        // O painel é dividido ao meio: a metade da casa puxa `--team`, a do
        // adversário puxa `--rival`, e as duas se encontram no centro — onde fica
        // o placar. Antes o fundo inteiro era da cor do rival, então o painel
        // parecia território dele em vez de terreno dividido.
        backgroundImage:
          "linear-gradient(90deg, color-mix(in srgb, var(--team) 20%, transparent) 0%, transparent 46%, transparent 54%, color-mix(in srgb, var(--rival) 20%, transparent) 100%)",
      }}
    >
      <div className="flex flex-wrap items-center justify-between gap-2">
        <BlockLabel>{t("myTeamTab.history.identity.rivalLabel")}</BlockLabel>
        <span className="rounded-md border border-white/15 px-2 py-0.5 text-[10px] font-semibold text-text-secondary">
          {rival.originKind ?? t("myTeamTab.history.identity.rivalHeuristic")}
        </span>
      </div>

      <div className="mt-3 flex flex-wrap items-center gap-x-4 gap-y-3">
        <TeamLogoMark teamName={dossier.name} color={dossier.color} size="md" testId="team-history-duel-home" />
        <div className="min-w-0 flex-1">
          <strong className="block truncate text-base font-semibold leading-tight text-[color:var(--team)]">
            {dossier.name}
          </strong>
          {/* Categoria dos dois lados, e não o perfil da casa: o perfil já está
              no card logo abaixo, e simétrico o painel lê como confronto. */}
          <span className="block truncate text-xs text-text-secondary">{dossier.identity.current}</span>
        </div>
        {/* O placar é a informação que uma rivalidade existe para dar. Fica no
            centro geométrico do painel porque é o centro do assunto, e grande o
            bastante para ser a primeira coisa que o olho encontra. */}
        <div className="shrink-0 px-2 text-center">
          <div className="flex items-baseline justify-center gap-2.5 font-mono text-[32px] font-semibold leading-none tracking-[-0.03em]">
            <span className="text-[color:var(--team)]">{rival.headToHeadWins}</span>
            <span className="text-base text-text-muted">×</span>
            <span className="text-[color:var(--rival)]">{rival.headToHeadLosses}</span>
          </div>
          <span className="mt-2 block text-[11px] font-semibold text-text-secondary">
            {clashes > 0
              ? t("myTeamTab.history.identity.rivalHeadToHead")
              : t("myTeamTab.history.identity.rivalNoClash")}
          </span>
        </div>
        <div className="min-w-0 flex-1 text-right">
          <strong className="block truncate text-base font-semibold leading-tight text-[color:var(--rival)]">
            {rival.name}
          </strong>
          <span className="block truncate text-xs text-text-secondary">{rival.currentCategory}</span>
        </div>
        <TeamLogoMark teamName={rival.name} color={rival.color} size="md" testId="team-history-rival-logo" />
      </div>

      {/* Faixa de rodapé em flex, não em grid: quando o motor não registrou os
          eixos sobrava um item só, e o grid de três colunas o esticava por toda a
          largura como se fosse um bloco. */}
      <div className="mt-3.5 flex flex-wrap items-end gap-x-8 gap-y-3 border-t border-white/5 pt-3">
        <div className="min-w-[13rem] flex-1">
          <BlockLabel>
            {rival.lastMeeting
              ? t("myTeamTab.history.identity.rivalLastMeeting")
              : t("myTeamTab.history.identity.rivalScopeNote")}
          </BlockLabel>
          <p className="mt-1 text-xs leading-5 text-text-secondary">
            {/* "Temporada 3, rodada 5" não diz se foi ontem ou há três anos. O
                tempo decorrido diz, e é ele que faz a rivalidade parecer viva ou
                arquivada — o resultado do encontro segue junto. */}
            {rival.lastMeeting
              ? `${formatMeetingAge(t, rival.lastMeeting.weeksAgo)} — ${t(
                  "myTeamTab.history.identity.rivalLastMeetingResult",
                  {
                    position: rival.lastMeeting.position,
                    rivalPosition: rival.lastMeeting.rivalPosition,
                  },
                )}`
              : rival.note}
          </p>
        </div>
        {hasAxes ? (
          <>
            <IntensityBar
              label={t("myTeamTab.history.identity.rivalAxisHistorical")}
              value={rival.historicalIntensity}
            />
            <IntensityBar label={t("myTeamTab.history.identity.rivalAxisRecent")} value={rival.recentActivity} />
          </>
        ) : null}
      </div>
    </div>
  );
}

// Escola ou mercado: os rótulos de desempenho não separam a equipe que forma
// gente da que compra pronto, e as duas podem ganhar igual. Aqui separam.
function RecruitmentCard({ recruitment }) {
  const { t } = useTranslation();
  return (
    <div className="rounded-xl border border-[color-mix(in_srgb,var(--team)_32%,transparent)] bg-[#0c1626]/95 px-4 py-3.5">
      <div className="flex items-baseline justify-between gap-2">
        <BlockLabel>{t("myTeamTab.history.identity.recruitmentLabel")}</BlockLabel>
        <span className="shrink-0 font-mono text-[11px] font-semibold text-text-secondary">
          {t("myTeamTab.history.identity.recruitmentRatio", {
            rookies: recruitment.rookies,
            drivers: recruitment.drivers,
          })}
        </span>
      </div>
      <strong className="mt-1.5 block text-[15px] font-semibold leading-tight text-[color:var(--team)]">
        {recruitment.profile}
      </strong>
      <div className="mt-3 grid grid-cols-3 gap-2">
        <FactTile
          label={t("myTeamTab.history.identity.tile.rookies")}
          value={`${Math.round(recruitment.rookieShare)}%`}
        />
        <FactTile
          label={t("myTeamTab.history.identity.tile.grid")}
          value={`${Math.round(recruitment.fieldRookieShare)}%`}
        />
        <FactTile
          label={t("myTeamTab.history.identity.tile.onArrival")}
          value={recruitment.averageExperience.toFixed(1)}
        />
      </div>
      {/* A comparação com o grid vira geometria: a barra é a fatia da equipe, o
          traço é a do grid. O rótulo "Escola" ou "Mercado" passa a ser a leitura
          de uma distância que dá para ver, e não uma afirmação para acreditar. */}
      <TeamShareBar value={recruitment.rookieShare} marker={recruitment.fieldRookieShare} />
    </div>
  );
}

// A barra de proporção que Records usa em cada card, trazida para cá — é ela que
// põe a cor da equipe na aba. `marker` desenha a régua de comparação por cima.
function TeamShareBar({ value, marker = null }) {
  const pct = Math.max(0, Math.min(100, Math.round(value)));
  const markerPct = marker == null ? null : Math.max(0, Math.min(100, Math.round(marker)));
  return (
    <div className="relative mt-2.5 h-[3px] overflow-hidden rounded-full bg-white/10">
      <div className="h-full rounded-full bg-[color:var(--team)]" style={{ width: `${pct}%` }} />
      {markerPct == null ? null : (
        <span className="absolute inset-y-0 w-[2px] bg-white/55" style={{ left: `calc(${markerPct}% - 1px)` }} />
      )}
    </div>
  );
}

// Onde a equipe historicamente vai bem e onde apanha. É identidade barata de
// obter e cara de esquecer: o jogador aprende a temer o calendário.
function TrackAffinityCard({ affinity, favourite = false }) {
  const { t } = useTranslation();
  const tone = favourite
    ? "border-status-green/25 bg-[#0b1d19]/95"
    : "border-status-red/25 bg-[#241014]/95";
  const accent = favourite ? "text-status-green" : "text-status-red";
  return (
    <div className={`flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1 rounded-xl border px-4 py-3 ${tone}`}>
      <div className="min-w-0">
        <BlockLabel>
          {favourite
            ? t("myTeamTab.history.identity.trackFavouriteLabel")
            : t("myTeamTab.history.identity.trackBogeyLabel")}
        </BlockLabel>
        <strong className={`mt-1 block truncate text-[15px] font-semibold leading-tight ${accent}`}>
          {affinity.track}
        </strong>
      </div>
      <span className="shrink-0 font-mono text-[11px] font-semibold text-text-secondary">
        {t("myTeamTab.history.identity.trackDetail", {
          count: affinity.races,
          average: affinity.averagePosition.toFixed(1),
          best: affinity.bestPosition,
        })}
      </span>
    </div>
  );
}

// Vive dentro do card do rival, então lê `--rival` do pai — as barras são da cor
// de quem a rivalidade descreve, não de um amarelo qualquer.
function IntensityBar({ label, value }) {
  const pct = Math.max(0, Math.min(100, Math.round(value)));
  return (
    <div className="w-[10rem]">
      <div className="flex items-baseline justify-between gap-2">
        <span className="text-[11px] font-semibold text-text-secondary">{label}</span>
        <span className="font-mono text-[11px] font-semibold text-[color:var(--rival)]">{pct}</span>
      </div>
      <div className="mt-1.5 h-1.5 overflow-hidden rounded-full bg-[#0f1c2b]">
        <div
          className="h-full rounded-full bg-[color-mix(in_srgb,var(--rival)_78%,transparent)]"
          style={{ width: `${pct}%` }}
        />
      </div>
    </div>
  );
}
