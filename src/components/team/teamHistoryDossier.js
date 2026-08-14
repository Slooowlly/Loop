// Dossiê de histórico de equipe — normalização do payload de
// `get_team_history_dossier` e as regras derivadas dele.
//
// Este módulo nasceu dentro do antigo `TeamHistoryDrawer.jsx` (o v1 da tela) e
// ficou aqui quando a UI v1 foi removida em 11/08/2026: os dados nunca foram do
// v1, eram só vizinhos dele. O consumidor hoje é
// `src/components/team/v2/TeamHistoryDrawerV2.jsx`.
import { financialState } from "./teamFinanceLabels";
import i18n from "../../i18n/index.js";
import { categoryLabel, formatMoney } from "../../utils/formatters";

export function orderTeamsForHistoryNavigation(teams) {
  return [...(Array.isArray(teams) ? teams : [])].sort((a, b) => {
    const positionDiff = (a.posicao ?? 999) - (b.posicao ?? 999);
    if (positionDiff !== 0) return positionDiff;
    return String(a.nome ?? "").localeCompare(String(b.nome ?? ""), "pt-BR");
  });
}

// Afinidade de pista chega como objeto ou como null — o backend omite os dois
// lados quando não há circuito repetido o bastante para a leitura valer.
function normalizeTrackAffinity(raw) {
  if (!raw) return null;
  return {
    track: raw.track ?? "",
    races: raw.races ?? 0,
    averagePosition: raw.average_position ?? raw.averagePosition ?? 0,
    bestPosition: raw.best_position ?? raw.bestPosition ?? 0,
  };
}

// O último encontro entre as duas equipes. `null` quando nunca se cruzaram no
// recorte — rivalidade registrada pelo mundo pode não ter confronto na categoria.
function normalizeRivalMeeting(raw) {
  if (!raw) return null;
  return {
    year: raw.year ?? 0,
    round: raw.round ?? 0,
    position: raw.position ?? 0,
    rivalPosition: raw.rival_position ?? raw.rivalPosition ?? 0,
    weeksAgo: raw.weeks_ago ?? raw.weeksAgo ?? null,
  };
}

// DNA de recrutamento vem null quando passaram menos de três pilotos pela equipe
// — o backend prefere calar a inventar padrão em cima de duas contratações.
function normalizeRecruitment(raw) {
  if (!raw) return null;
  return {
    profile: raw.profile ?? "",
    drivers: raw.drivers ?? 0,
    rookies: raw.rookies ?? 0,
    averageExperience: raw.average_experience ?? raw.averageExperience ?? 0,
    rookieShare: raw.rookie_share ?? raw.rookieShare ?? 0,
    fieldRookieShare: raw.field_rookie_share ?? raw.fieldRookieShare ?? 0,
  };
}

export function buildTeamHistoryDossier(
  team,
  teams,
  playerTeam,
  activeCategory,
  historyDossier,
  historyStatus = "ready",
  historyError = "",
) {
  const mergedTeam = team?.id === playerTeam?.id
    ? { ...team, ...playerTeam, posicao: team.posicao, pontos: team.pontos ?? playerTeam.pontos }
    : team;
  const category = activeCategory ?? playerTeam?.categoria ?? mergedTeam?.categoria ?? "gt4";
  const categoryName = categoryLabel(category);
  const rankedTeams = Array.isArray(teams) ? teams : [];
  const realHistory = normalizeTeamHistoryPayload(historyDossier);
  const rival = findHistoricRival(mergedTeam, rankedTeams);
  const founded = resolveTeamFoundedYear(mergedTeam);
  const categoryGroup = categoryGroupLabel(category);

  // Fallbacks NEUTROS: quando o histórico real ainda não carregou (ou o backend não
  // tem dados), mostramos "—"/estado honesto — nunca um número inventado. O histórico
  // real (get_team_history_dossier) substitui tudo isto quando pronto.
  return {
    name: mergedTeam?.nome ?? i18n.t("myTeamTab.team.fallbackName"),
    color: mergedTeam?.cor_primaria ?? "#58a6ff",
    state: realHistory?.identity?.heritage ?? teamHeritageLabel(founded),
    founded,
    currentCategory: categoryName,
    recordScope: realHistory?.recordScope ?? categoryGroup,
    historyStatus,
    historyError,
    hasHistory: realHistory?.hasHistory ?? false,
    records: realHistory?.records ?? [],
    titleCategories: realHistory?.titleCategories ?? [],
    sport: realHistory?.sport ?? emptyRealSport(),
    identity: realHistory?.identity ?? {
      origin: i18n.t("myTeamTab.history.defaults.dash"),
      current: categoryName,
      profile: i18n.t("myTeamTab.history.defaults.profileForming"),
      summary: i18n.t("myTeamTab.history.defaults.identityFormingSummary"),
      rival: {
        name: rival?.nome ?? i18n.t("myTeamTab.history.defaults.noRival"),
        currentCategory: categoryName,
        note: rival
          ? i18n.t("myTeamTab.history.defaults.rivalClosest", { group: categoryGroup })
          : i18n.t("myTeamTab.history.defaults.rivalNoDuel"),
      },
      symbolDriver: mergedTeam?.piloto_1_nome ?? i18n.t("myTeamTab.history.defaults.mainDriver"),
      symbolDriverDetail: i18n.t("myTeamTab.history.defaults.symbolDriverDetail"),
    },
    management: realHistory?.management ?? emptyRealManagement(mergedTeam),
    movement: realHistory?.movement ?? {
      promotions: i18n.t("myTeamTab.history.defaults.dash"),
      relegations: i18n.t("myTeamTab.history.defaults.dash"),
      timeByCategory: i18n.t("myTeamTab.history.defaults.unavailable"),
      peakCategory: categoryName,
      homeCategory: categoryName,
      timeLines: [],
      ladder: [],
    },
    categoryPath: realHistory?.categoryPath ?? [],
    timeline: realHistory?.timeline ?? [],
    ownershipEvents: realHistory?.ownershipEvents ?? [],
    highlights: realHistory?.highlights ?? [],
    milestones: realHistory?.milestones ?? [],
    seasonResults: realHistory?.seasonResults ?? [],
    recentForm: realHistory?.recentForm ?? [],
    resultSpread: realHistory?.resultSpread ?? normalizeResultSpread(null),
    championshipRun: realHistory?.championshipRun ?? null,
    lineup: realHistory?.lineup ?? [],
    reliability: realHistory?.reliability ?? normalizeReliability(null),
    outsideScopeSeasons: realHistory?.outsideScopeSeasons ?? [],
    // Intervalo de anos do MUNDO — só o v2 usa, para marcar na faixa os anos em
    // que a equipe não correu. Zero quando o backend não informa.
    worldFirstYear: realHistory?.worldFirstYear ?? 0,
    worldLastYear: realHistory?.worldLastYear ?? 0,
  };
}

function normalizeResultSpread(spread) {
  return {
    races: Number(spread?.races ?? 0),
    first: Number(spread?.first ?? 0),
    podium: Number(spread?.podium ?? 0),
    nearMiss: Number(spread?.near_miss ?? spread?.nearMiss ?? 0),
    topTen: Number(spread?.top_ten ?? spread?.topTen ?? 0),
    outside: Number(spread?.outside ?? 0),
  };
}

// Campanha do campeonato. As rodadas e as linhas vêm cruas do backend; o
// desenho é que decide escala e cor. Um recorte sem pelo menos duas rodadas ou
// sem linha nenhuma vira `null` aqui e não chega à tela como gráfico vazio.
function normalizeChampionshipRun(run) {
  if (!run) return null;
  const rounds = (run.rounds ?? []).map((value) => Number(value ?? 0));
  const lines = (run.lines ?? []).map((line) => ({
    teamId: line.team_id ?? line.teamId ?? "",
    team: line.team ?? "",
    selected: Boolean(line.selected),
    position: Number(line.position ?? 0),
    total: String(line.total ?? "0"),
    points: (line.points ?? []).map((value) => Number(value ?? 0)),
  }));
  if (rounds.length < 2 || !lines.length) return null;
  return {
    year: String(run.year ?? ""),
    category: run.category ?? "",
    categoryId: run.category_id ?? run.categoryId ?? "",
    live: Boolean(run.live),
    rounds,
    lines,
  };
}

// Confiabilidade: contagens cruas, sem prosa. As faixas se somam — quem soma é o
// desenho, então o normalizador só garante que tudo é número.
function normalizeReliability(reliability) {
  return {
    races: Number(reliability?.races ?? 0),
    finished: Number(reliability?.finished ?? 0),
    finishRate: Number(reliability?.finish_rate ?? reliability?.finishRate ?? 0),
    groupFinishRate: Number(reliability?.group_finish_rate ?? reliability?.groupFinishRate ?? 0),
    mechanical: Number(reliability?.mechanical ?? 0),
    driverError: Number(reliability?.driver_error ?? reliability?.driverError ?? 0),
    other: Number(reliability?.other ?? 0),
    worstPart: reliability?.worst_part ?? reliability?.worstPart ?? "",
  };
}

function normalizeTeamHistoryPayload(payload) {
  if (!payload) return null;
  const sport = payload.sport ?? {};
  const identity = payload.identity ?? {};
  const rival = identity.rival ?? {};
  const management = payload.management ?? {};
  return {
    recordScope: payload.record_scope ?? payload.recordScope ?? i18n.t("myTeamTab.history.defaults.recordScope"),
    hasHistory: Boolean(payload.has_history ?? payload.hasHistory),
    records: (payload.records ?? []).map((record) => ({
      // Identificador estável da métrica; o v2 escolhe ícone e layout por ele.
      id: record.id ?? "",
      label: record.label,
      rank: record.rank,
      value: String(record.value),
      // Campos que só o v2 desenha (barra de posição e média do grupo). O v1
      // ignora — carregá-los aqui mantém uma única normalização do payload.
      rankPosition: Number(record.rank_position ?? record.rankPosition ?? 0),
      rankTotal: Number(record.rank_total ?? record.rankTotal ?? 0),
      groupAverage: record.group_average ?? record.groupAverage ?? "",
    })),
    sport: {
      seasons: sport.seasons ?? i18n.t("myTeamTab.history.defaults.noSeasons"),
      currentStreak: sport.current_streak ?? sport.currentStreak ?? i18n.t("myTeamTab.history.defaults.noStreak"),
      bestStreak: sport.best_streak ?? sport.bestStreak ?? i18n.t("myTeamTab.history.defaults.noStreak"),
      podiumRate: sport.podium_rate ?? sport.podiumRate ?? "0%",
      winRate: sport.win_rate ?? sport.winRate ?? "0%",
      races: sport.races ?? 0,
      wins: sport.wins ?? 0,
      podiums: sport.podiums ?? 0,
    },
    timeline: payload.timeline ?? [],
    titleCategories: (payload.title_categories ?? payload.titleCategories ?? []).map((item) => ({
      category: item.category ?? "",
      year: String(item.year ?? ""),
      color: item.color ?? "",
      // Campos que só o v2 desenha: a galeria dele conta como o título foi
      // ganho e quem pilotava, não só em que ano.
      categoryId: item.category_id ?? item.categoryId ?? "",
      points: String(item.points ?? ""),
      wins: Number(item.wins ?? 0),
      championDriver: item.champion_driver ?? item.championDriver ?? "",
      championTeam: item.champion_team ?? item.championTeam ?? "",
      championIsTeam: Boolean(item.champion_is_team ?? item.championIsTeam ?? false),
    })),
    categoryPath: (payload.category_path ?? payload.categoryPath ?? []).map((step) => ({
      category: step.category,
      categoryId: step.category_id ?? step.categoryId ?? "",
      years: step.years,
      startYear: Number(step.start_year ?? step.startYear ?? 0),
      endYear: Number(step.end_year ?? step.endYear ?? 0),
      detail: step.detail,
      color: step.color,
      movement: step.movement ?? "same",
      tier: Number(step.tier ?? 0),
    })),
    worldFirstYear: Number(payload.world_first_year ?? payload.worldFirstYear ?? 0),
    worldLastYear: Number(payload.world_last_year ?? payload.worldLastYear ?? 0),
    movement: payload.movement
      ? {
          promotions: payload.movement.promotions ?? 0,
          relegations: payload.movement.relegations ?? 0,
          timeByCategory: payload.movement.time_by_category ?? payload.movement.timeByCategory ?? "",
          peakCategory: payload.movement.peak_category ?? payload.movement.peakCategory ?? i18n.t("myTeamTab.history.defaults.dash"),
          homeCategory: payload.movement.home_category ?? payload.movement.homeCategory ?? i18n.t("myTeamTab.history.defaults.dash"),
          timeLines: (payload.movement.time_lines ?? payload.movement.timeLines ?? []).map((linha) => ({
            category: linha.category ?? "",
            categoryId: linha.category_id ?? linha.categoryId ?? "",
            tier: Number(linha.tier ?? 0),
            seasons: Number(linha.seasons ?? 0),
            races: Number(linha.races ?? 0),
            wins: Number(linha.wins ?? 0),
            podiums: Number(linha.podiums ?? 0),
          })),
          ladder: (payload.movement.ladder ?? []).map((rung) => ({
            category: rung.category ?? "",
            categoryId: rung.category_id ?? rung.categoryId ?? "",
            tier: Number(rung.tier ?? 0),
            visited: Boolean(rung.visited),
            isPeak: Boolean(rung.is_peak ?? rung.isPeak),
            isCurrent: Boolean(rung.is_current ?? rung.isCurrent),
            seasons: Number(rung.seasons ?? 0),
            years: rung.years ?? "",
          })),
        }
      : null,
    ownershipEvents: (payload.ownership_events ?? payload.ownershipEvents ?? []).map((event) => ({
      year: String(event.year ?? ""),
      eventType: event.event_type ?? event.eventType ?? "sale",
      title: event.title ?? i18n.t("myTeamTab.history.defaults.newBoard"),
      detail: event.detail ?? "",
      financialNote: event.financial_note ?? event.financialNote ?? "",
    })),
    highlights: (payload.highlights ?? []).map((item) => ({
      label: item.label,
      value: String(item.value ?? ""),
      detail: item.detail ?? "",
    })),
    milestones: (payload.milestones ?? []).map((item) => ({
      label: item.label,
      year: String(item.year ?? ""),
      // Identidade do fato, para o v2 fundir marcos e linha do tempo sem casar
      // prosa traduzida. Ver TeamHistoryMilestone::kind no backend.
      kind: item.kind ?? "",
    })),
    seasonResults: (payload.season_results ?? payload.seasonResults ?? []).map((item) => ({
      year: String(item.year ?? ""),
      category: item.category ?? "",
      // Id cru da categoria — o rótulo acima é traduzido e não serve de chave na
      // paleta de categorias. Só o v2 usa, na faixa de pódios por corrida.
      categoryId: item.category_id ?? item.categoryId ?? "",
      position: String(item.position ?? "—"),
      wins: item.wins ?? 0,
      podiums: item.podiums ?? 0,
      points: String(item.points ?? "0"),
      // Denominador das taxas por temporada e os degraus do pódio — só o v2
      // desenha (ver a faixa de top 5 em v2/TeamHistoryTrajectory.jsx).
      races: Number(item.races ?? 0),
      seconds: Number(item.seconds ?? 0),
      thirds: Number(item.thirds ?? 0),
      fourths: Number(item.fourths ?? 0),
      fifths: Number(item.fifths ?? 0),
      // Carros que abandonaram — conta carro, não corrida, e por isso vive fora
      // da soma do top 5. Save antigo não traz o campo e cai em zero.
      dnfs: Number(item.dnfs ?? 0),
    })),
    // Fita de forma recente e distribuição por faixa de colocação — só o v2
    // desenha (ver v2/TeamHistoryTrajectory.jsx e v2/TeamHistoryResults.jsx).
    recentForm: (payload.recent_form ?? payload.recentForm ?? []).map((item) => ({
      year: String(item.year ?? ""),
      round: Number(item.round ?? 0),
      category: item.category ?? "",
      categoryId: item.category_id ?? item.categoryId ?? "",
      position: item.position ?? null,
    })),
    resultSpread: normalizeResultSpread(payload.result_spread ?? payload.resultSpread),
    // Campanha do campeonato rodada a rodada, com a linha de todas as equipes —
    // só o v2 desenha. `null` quando o backend não tem recorte para mandar
    // (equipe com uma corrida só na última temporada, ou save anterior ao
    // campo), e aí o v2 cai na curva de posição por temporada.
    championshipRun: normalizeChampionshipRun(payload.championship_run ?? payload.championshipRun),
    lineup: (payload.lineup ?? []).map((item) => ({
      slot: Number(item.slot ?? 0),
      driverId: item.driver_id ?? item.driverId ?? "",
      name: item.name ?? "",
      nationality: item.nationality ?? "",
      firstYear: String(item.first_year ?? item.firstYear ?? ""),
      lastYear: String(item.last_year ?? item.lastYear ?? ""),
      races: Number(item.races ?? 0),
      wins: Number(item.wins ?? 0),
      podiums: Number(item.podiums ?? 0),
      titles: Number(item.titles ?? 0),
      bestPosition: Number(item.best_position ?? item.bestPosition ?? 0),
      currentTeamName: item.current_team_name ?? item.currentTeamName ?? "",
      currentTeamColor: item.current_team_color ?? item.currentTeamColor ?? "",
      isPlayer: Boolean(item.is_player ?? item.isPlayer ?? false),
      stillHere: Boolean(item.still_here ?? item.stillHere ?? false),
      currentLabel: item.current_label ?? item.currentLabel ?? "",
    })),
    reliability: normalizeReliability(payload.reliability),
    outsideScopeSeasons: (payload.outside_scope_seasons ?? payload.outsideScopeSeasons ?? []).map((item) => ({
      year: String(item.year ?? ""),
      category: item.category ?? "",
      categoryId: item.category_id ?? item.categoryId ?? "",
    })),
    identity: {
      origin: identity.origin ?? i18n.t("myTeamTab.history.defaults.noOrigin"),
      current: identity.current ?? i18n.t("myTeamTab.history.defaults.noCurrentCategory"),
      heritage: identity.heritage ?? null,
      profile: identity.profile ?? i18n.t("myTeamTab.history.defaults.profileForming"),
      summary: identity.summary ?? i18n.t("myTeamTab.history.defaults.identityInsufficient"),
      // Lastro numérico do perfil: sem o denominador, "Dominante" ao lado de 0
      // títulos no cabeçalho lia como erro. `null` = payload antigo, sem lastro.
      profileRaces: identity.profile_races ?? identity.profileRaces ?? null,
      profileWins: identity.profile_wins ?? identity.profileWins ?? null,
      profilePodiums: identity.profile_podiums ?? identity.profilePodiums ?? null,
      rival: {
        name: rival.name ?? i18n.t("myTeamTab.history.defaults.noRival"),
        currentCategory: rival.current_category ?? rival.currentCategory ?? i18n.t("myTeamTab.history.defaults.noCurrentCategory"),
        note: rival.note ?? i18n.t("myTeamTab.history.defaults.noRivalry"),
        color: rival.color ?? "",
        // Vem do motor de rivalidade de equipes. `null` quando o rival saiu da
        // heurística de confronto compartilhado — aí não há origem nem eixos.
        originKind: rival.origin_kind ?? rival.originKind ?? null,
        historicalIntensity: rival.historical_intensity ?? rival.historicalIntensity ?? null,
        recentActivity: rival.recent_activity ?? rival.recentActivity ?? null,
        perceivedIntensity: rival.perceived_intensity ?? rival.perceivedIntensity ?? null,
        headToHeadWins: rival.head_to_head_wins ?? rival.headToHeadWins ?? 0,
        headToHeadLosses: rival.head_to_head_losses ?? rival.headToHeadLosses ?? 0,
        lastMeeting: normalizeRivalMeeting(rival.last_meeting ?? rival.lastMeeting),
      },
      symbolDriver: identity.symbol_driver ?? identity.symbolDriver ?? i18n.t("myTeamTab.history.defaults.noSymbolDriver"),
      symbolDriverDetail: identity.symbol_driver_detail ?? identity.symbolDriverDetail ?? i18n.t("myTeamTab.history.defaults.insufficientResults"),
      symbolDriverYears: identity.symbol_driver_years ?? identity.symbolDriverYears ?? "",
      symbolDriverActive: identity.symbol_driver_active ?? identity.symbolDriverActive ?? false,
      symbolDriverNationality:
        identity.symbol_driver_nationality ?? identity.symbolDriverNationality ?? "",
      symbolDriverRaces: identity.symbol_driver_races ?? identity.symbolDriverRaces ?? 0,
      symbolDriverWins: identity.symbol_driver_wins ?? identity.symbolDriverWins ?? 0,
      symbolDriverPodiums: identity.symbol_driver_podiums ?? identity.symbolDriverPodiums ?? 0,
      // `null` quando não há pistas repetidas o bastante — a aba omite o bloco em
      // vez de eleger fetiche a partir de uma corrida solta.
      bestTrack: normalizeTrackAffinity(identity.best_track ?? identity.bestTrack),
      worstTrack: normalizeTrackAffinity(identity.worst_track ?? identity.worstTrack),
      recruitment: normalizeRecruitment(identity.recruitment),
    },
    management: {
      operationHealth: management.operation_health ?? management.operationHealth ?? i18n.t("myTeamTab.history.defaults.monitored"),
      peakCash: management.peak_cash ?? management.peakCash ?? i18n.t("myTeamTab.history.defaults.noBalance"),
      worstCrisis: management.worst_crisis ?? management.worstCrisis ?? i18n.t("myTeamTab.history.defaults.noCrisis"),
      healthyYears: management.healthy_years ?? management.healthyYears ?? i18n.t("myTeamTab.history.defaults.noSeasons"),
      efficiency: management.efficiency ?? i18n.t("myTeamTab.history.defaults.efficiencyZero"),
      biggestInvestment: management.biggest_investment ?? management.biggestInvestment ?? i18n.t("myTeamTab.history.defaults.noInvestment"),
      summary: management.summary ?? i18n.t("myTeamTab.history.defaults.managementUnread"),
      peakCashDetail: management.peak_cash_detail ?? management.peakCashDetail ?? i18n.t("myTeamTab.history.defaults.noBalanceDetail"),
      worstCrisisDetail: management.worst_crisis_detail ?? management.worstCrisisDetail ?? i18n.t("myTeamTab.history.defaults.noCrisisDetail"),
      healthyYearsDetail: management.healthy_years_detail ?? management.healthyYearsDetail ?? i18n.t("myTeamTab.history.defaults.noHealthDetail"),
      efficiencyDetail: management.efficiency_detail ?? management.efficiencyDetail ?? i18n.t("myTeamTab.history.defaults.noEfficiencyDetail"),
      investmentDetail: management.investment_detail ?? management.investmentDetail ?? i18n.t("myTeamTab.history.defaults.noInvestmentDetail"),
      ledger: normalizeLedger(management.ledger),
    },
  };
}

// Livro-caixa agregado (`team_finance_history`). `null` só em save que não tem a
// tabela escrita — o backend já devolve `None` nesse caso.
//
// Janela ZERADA (`rounds === 0`) NÃO é ausência: é uma carreira que ainda não
// correu, e o backend devolve `Some` de propósito, com `flowNote` explicando a
// causa. Anular aqui derrubava justamente o bloco que existe para explicar, e a
// aba caía inteira nos cards de retrato atual — indistinguível da versão anterior
// ao livro-caixa. Cada bloco decide sozinho: `MoneyFlow` cai na frase quando não
// há repartição, `CashCurve` some quando a série tem menos de dois pontos.
function normalizeLedger(ledger) {
  if (!ledger) return null;
  const rounds = Number(ledger.rounds ?? 0);
  return {
    seasons: Number(ledger.seasons ?? 0),
    rounds: Number.isFinite(rounds) ? rounds : 0,
    firstSeason: Number(ledger.first_season ?? ledger.firstSeason ?? 0),
    lastSeason: Number(ledger.last_season ?? ledger.lastSeason ?? 0),
    peakCash: Number(ledger.peak_cash ?? ledger.peakCash ?? 0),
    peakCashSeason: Number(ledger.peak_cash_season ?? ledger.peakCashSeason ?? 0),
    peakCashRound: Number(ledger.peak_cash_round ?? ledger.peakCashRound ?? 0),
    worstDebt: Number(ledger.worst_debt ?? ledger.worstDebt ?? 0),
    worstDebtSeason: Number(ledger.worst_debt_season ?? ledger.worstDebtSeason ?? 0),
    worstDebtRound: Number(ledger.worst_debt_round ?? ledger.worstDebtRound ?? 0),
    healthySeasons: Number(ledger.healthy_seasons ?? ledger.healthySeasons ?? 0),
    // Janela da repartição: só as temporadas com livro-caixa rodada a rodada. As de
    // backstory gravam só o prêmio de construtores, e entrariam na soma como uma
    // equipe que fatura sem gastar nada.
    flowSeasons: Number(ledger.flow_seasons ?? ledger.flowSeasons ?? 0),
    flowFirstSeason: Number(ledger.flow_first_season ?? ledger.flowFirstSeason ?? 0),
    flowLastSeason: Number(ledger.flow_last_season ?? ledger.flowLastSeason ?? 0),
    flowNote: ledger.flow_note ?? ledger.flowNote ?? "",
    incomeTotal: Number(ledger.income_total ?? ledger.incomeTotal ?? 0),
    expensesTotal: Number(ledger.expenses_total ?? ledger.expensesTotal ?? 0),
    incomeLines: normalizeLedgerLines(ledger.income_lines ?? ledger.incomeLines),
    expenseLines: normalizeLedgerLines(ledger.expense_lines ?? ledger.expenseLines),
    cashCurve: (ledger.cash_curve ?? ledger.cashCurve ?? []).map((point) => ({
      seasonNumber: Number(point.season_number ?? point.seasonNumber ?? 0),
      round: Number(point.round ?? 0),
      cashBalance: Number(point.cash_balance ?? point.cashBalance ?? 0),
      debtBalance: Number(point.debt_balance ?? point.debtBalance ?? 0),
      isSeasonClose: Boolean(point.is_season_close ?? point.isSeasonClose ?? false),
    })),
  };
}

function normalizeLedgerLines(lines) {
  if (!Array.isArray(lines)) return [];
  // Sem `share`: a fatia depende do denominador que a tela escolhe mostrar, e o
  // Sankey mede cada linha contra a receita TOTAL, não contra o seu próprio lado.
  return lines.map((line) => ({
    id: String(line.id ?? ""),
    value: Number(line.value ?? 0),
  }));
}

// Ano de fundação REAL (payload do backend `founded_year`). Sem valor confiável → null,
// e o front mostra estado neutro em vez de inventar um ano.
function resolveTeamFoundedYear(team) {
  const explicitYear = Number(team?.founded_year ?? team?.ano_fundacao);
  if (Number.isFinite(explicitYear) && explicitYear > 1800) {
    return explicitYear;
  }
  return null;
}

function teamHeritageLabel(founded) {
  if (!founded) return i18n.t("myTeamTab.history.heritage.team");
  if (founded <= 1970) return i18n.t("myTeamTab.history.heritage.historic");
  return i18n.t("myTeamTab.history.heritage.consolidated");
}

function emptyRealSport() {
  return {
    seasons: i18n.t("myTeamTab.history.defaults.loadingReal"),
    currentStreak: i18n.t("myTeamTab.history.defaults.loadingReal"),
    bestStreak: i18n.t("myTeamTab.history.defaults.loadingReal"),
    podiumRate: "0%",
    winRate: "0%",
    races: 0,
    wins: 0,
    podiums: 0,
  };
}

// Fallback NEUTRO de gestão: usa só o estado financeiro atual (real); o resto fica "—"
// até o histórico real (get_team_history_dossier) chegar. Sem estimativas fabricadas.
function emptyRealManagement(team) {
  const debt = team?.debt_balance ?? 0;
  return {
    operationHealth: financialState(team?.financial_state),
    peakCash: i18n.t("myTeamTab.history.defaults.dash"),
    worstCrisis: debt > 0 ? i18n.t("myTeamTab.history.defaults.currentDebt", { value: formatMoney(debt) }) : i18n.t("myTeamTab.history.defaults.noRelevantDebt"),
    healthyYears: i18n.t("myTeamTab.history.defaults.dash"),
    efficiency: i18n.t("myTeamTab.history.defaults.dash"),
    biggestInvestment: i18n.t("myTeamTab.history.defaults.dash"),
    summary: i18n.t("myTeamTab.history.defaults.managementUnconsolidated"),
    peakCashDetail: i18n.t("myTeamTab.history.defaults.noBalanceHistoryDetail"),
    worstCrisisDetail: debt > 0 ? i18n.t("myTeamTab.history.defaults.currentLiability") : i18n.t("myTeamTab.history.defaults.noCrisisRegistered"),
    healthyYearsDetail: i18n.t("myTeamTab.history.defaults.noHistoryRegistered"),
    efficiencyDetail: i18n.t("myTeamTab.history.defaults.noHistoryRegistered"),
    investmentDetail: i18n.t("myTeamTab.history.defaults.noHistoryRegistered"),
  };
}

function findHistoricRival(team, teams) {
  return [...(teams ?? [])]
    .filter((entry) => entry.id !== team?.id)
    .sort((a, b) => Math.abs((a.posicao ?? 99) - (team?.posicao ?? 99)) - Math.abs((b.posicao ?? 99) - (team?.posicao ?? 99)))[0];
}

function categoryGroupLabel(category) {
  if (category?.includes("mazda")) return i18n.t("myTeamTab.history.groups.mazda");
  if (category?.includes("toyota")) return i18n.t("myTeamTab.history.groups.toyota");
  if (category === "bmw_m2") return i18n.t("myTeamTab.history.groups.bmw");
  if (category === "gt4") return i18n.t("myTeamTab.history.groups.gt4");
  if (category === "gt3") return i18n.t("myTeamTab.history.groups.gt3");
  if (category === "lmp2") return i18n.t("myTeamTab.history.groups.lmp2");
  if (category === "endurance") return i18n.t("myTeamTab.history.groups.endurance");
  return i18n.t("myTeamTab.history.groups.default");
}

// A saúde da operação é pintada a partir da MESMA palavra que o dossiê monta —
// duas cópias da regra divergiriam na primeira palavra nova.
export function operationHealthTone(label) {
  const normalized = String(label ?? "")
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase();

  if (normalized.includes("pressionada") || normalized.includes("critica") || normalized.includes("crise") || normalized.includes("colapso")) {
    return {
      card: "border-status-red/30 bg-[#241014]/95 bg-[radial-gradient(circle_at_12%_10%,rgba(255,103,103,0.14),transparent_12rem),linear-gradient(145deg,rgba(45,16,21,0.96),rgba(7,16,29,0.99))]",
      text: "text-status-red",
    };
  }

  if (normalized.includes("estavel") || normalized.includes("monitorada")) {
    return {
      card: "border-status-yellow/30 bg-[#201a0b]/95 bg-[radial-gradient(circle_at_12%_10%,rgba(242,196,109,0.14),transparent_12rem),linear-gradient(145deg,rgba(35,29,12,0.96),rgba(7,16,29,0.99))]",
      text: "text-status-yellow",
    };
  }

  return {
    card: "border-status-green/30 bg-[#0b1d19] bg-[radial-gradient(circle_at_12%_10%,rgba(94,231,168,0.14),transparent_12rem),linear-gradient(145deg,rgba(12,35,30,0.96),rgba(7,16,29,0.99))]",
    text: "text-status-green",
  };
}
