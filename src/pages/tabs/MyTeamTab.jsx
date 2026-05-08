import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";

import GlassCard from "../../components/ui/GlassCard";
import FlagIcon from "../../components/ui/FlagIcon";
import TeamLogoMark from "../../components/team/TeamLogoMark";
import useCareerStore from "../../stores/useCareerStore";
import { categoryLabel, extractNationalityLabel, getCategoryTier } from "../../utils/formatters";

const BUILD_META = {
  balanced: { label: "Balanceado", weights: [34, 33, 33] },
  acceleration_intermediate: { label: "Aceleração", weights: [47, 27, 27] },
  power_intermediate: { label: "Potência", weights: [27, 47, 27] },
  handling_intermediate: { label: "Dirigibilidade", weights: [27, 27, 47] },
  acceleration_extreme: { label: "Aceleração extrema", weights: [60, 20, 20] },
  power_extreme: { label: "Potência extrema", weights: [20, 60, 20] },
  handling_extreme: { label: "Dirigibilidade extrema", weights: [20, 20, 60] },
};

const TECH_AXES = [
  { id: "development", label: "Desenvolvimento" },
  { id: "reliability", label: "Confiabilidade" },
  { id: "pit", label: "Pit e corrida" },
];

const TEAM_HISTORY_TABS = [
  { id: "records", label: "Records" },
  { id: "sport", label: "Esportivo" },
  { id: "identity", label: "Identidade" },
  { id: "management", label: "Gestão" },
  { id: "categories", label: "Categorias" },
];

const KNOWN_TEAM_FOUNDING_YEARS = [
  { names: ["ferrari"], year: 1929 },
  { names: ["porsche", "wright motorsports", "ebimotors", "gpx racing"], year: 1931 },
  { names: ["ford mustang", "multimatic motorsports"], year: 1903 },
  { names: ["chevrolet", "corvette"], year: 1911 },
  { names: ["bmw", "tr3 racing"], year: 1916 },
  { names: ["mercedes-amg", "sunenergy1", "team korthoff"], year: 1967 },
  { names: ["lamborghini", "paul miller"], year: 1963 },
  { names: ["mclaren", "k-pax", "balfe endurance"], year: 1963 },
  { names: ["acura team penske"], year: 1966 },
  { names: ["acura"], year: 1986 },
  { names: ["aston martin", "heart of racing gt3"], year: 1913 },
  { names: ["audi", "r8g esports"], year: 1909 },
];

const CATEGORY_FOUNDING_BASE_YEARS = {
  mazda_rookie: 2020,
  mazda_amador: 2016,
  toyota_rookie: 2021,
  toyota_amador: 2012,
  bmw_m2: 2015,
  gt4: 2002,
  gt3: 1999,
  production_challenger: 2018,
  endurance: 1998,
};

function MyTeamTab() {
  const careerId = useCareerStore((state) => state.careerId);
  const player = useCareerStore((state) => state.player);
  const playerTeam = useCareerStore((state) => state.playerTeam);
  const [drivers, setDrivers] = useState([]);
  const [teams, setTeams] = useState([]);
  const [activeAxis, setActiveAxis] = useState("development");
  const [selectedHistoryTeam, setSelectedHistoryTeam] = useState(null);
  const [activeHistoryTab, setActiveHistoryTab] = useState("records");
  const [error, setError] = useState("");

  useEffect(() => {
    let mounted = true;

    async function load() {
      if (!careerId || !playerTeam?.categoria) return;
      try {
        setError("");
        const [loadedDrivers, loadedTeams] = await Promise.all([
          invoke("get_drivers_by_category", { careerId, category: playerTeam.categoria }),
          invoke("get_teams_standings", { careerId, category: playerTeam.categoria }),
        ]);
        if (mounted) {
          setDrivers(Array.isArray(loadedDrivers) ? loadedDrivers : []);
          setTeams(Array.isArray(loadedTeams) ? loadedTeams : []);
        }
      } catch (invokeError) {
        if (mounted) {
          setError(typeof invokeError === "string" ? invokeError : "Não foi possível carregar os dados da equipe.");
        }
      }
    }

    load();
    return () => {
      mounted = false;
    };
  }, [careerId, playerTeam?.categoria]);

  const piloto1 = drivers.find((driver) => driver.id === playerTeam?.piloto_1_id);
  const piloto2 = drivers.find((driver) => driver.id === playerTeam?.piloto_2_id);
  const standing = teams.find((team) => team.id === playerTeam?.id);
  const driverRows = [
    buildDriverRow("N1", piloto1, playerTeam, player?.id),
    buildDriverRow("N2", piloto2, playerTeam, player?.id),
  ];

  return (
    <div className="space-y-5">
      <CommandHeader team={playerTeam} standing={standing} />

      {error ? (
        <div className="rounded-2xl border border-status-red/30 bg-status-red/10 px-4 py-3 text-sm text-status-red">
          {error}
        </div>
      ) : null}

      <div className="grid gap-5 xl:grid-cols-[0.72fr_1.28fr]">
        <div className="space-y-5" data-testid="my-team-side-rail">
          <DriverPanel drivers={driverRows} salaryCeiling={playerTeam?.salary_ceiling ?? 0} />
          <TechPanel team={playerTeam} activeAxis={activeAxis} setActiveAxis={setActiveAxis} />
          <CostChart />
        </div>
        <FinanceDossier team={playerTeam} drivers={driverRows} />
      </div>

      <RankingTable
        teams={teams}
        playerTeam={playerTeam}
        historyTeamId={selectedHistoryTeam?.id}
        onTeamHistoryOpen={(team) => {
          setSelectedHistoryTeam(team);
          setActiveHistoryTab("records");
        }}
      />

      {selectedHistoryTeam ? (
        <TeamHistoryDrawer
          careerId={careerId}
          team={selectedHistoryTeam}
          teams={teams}
          playerTeam={playerTeam}
          activeCategory={playerTeam?.categoria}
          activeTab={activeHistoryTab}
          onTabChange={setActiveHistoryTab}
          onSelectTeam={setSelectedHistoryTeam}
          onClose={() => setSelectedHistoryTeam(null)}
        />
      ) : null}
    </div>
  );
}

function CommandHeader({ team, standing }) {
  return (
    <GlassCard hover={false} className="rounded-[30px]" data-testid="my-team-command-header">
      <div className="grid gap-5 lg:grid-cols-[1.18fr_0.82fr] lg:items-center">
        <div className="flex items-center gap-4">
          <TeamLogoMark
            teamName={team?.nome}
            color={team?.cor_primaria}
            size="lg"
            testId="my-team-command-logo"
          />
          <div>
            <p className="text-[11px] uppercase tracking-[0.24em] text-accent-primary">Central de gestão</p>
            <h2 className="mt-2 text-3xl font-semibold text-text-primary">{team?.nome ?? "Equipe"}</h2>
          </div>
        </div>
        <HeaderFinanceStat team={team} standing={standing} />
      </div>
    </GlassCard>
  );
}

function HeaderFinanceStat({ team, standing }) {
  const stateTone = financialStateTone(team?.financial_state);
  return (
    <div
      data-testid="header-finance-stat"
      className="justify-self-stretch text-right lg:justify-self-end"
    >
      <div className="flex min-w-0 flex-col items-end">
        <div className="max-w-full">
          <p className={`break-words font-mono text-5xl font-semibold leading-none ${moneyTone(team?.cash_balance ?? 0)}`}>
            {formatMoney(team?.cash_balance ?? 0)}
          </p>
        </div>
        <div className="mt-3 flex flex-wrap items-center justify-end gap-3">
          <span className={`rounded-full border px-3 py-1 text-xs font-semibold ${stateTone}`}>
            {financialState(team?.financial_state)}
          </span>
          <span className="text-[10px] uppercase tracking-[0.16em] text-text-muted">
            Posição <span className="font-mono text-sm font-bold text-status-yellow">{formatOrdinal(standing?.posicao)}</span>
          </span>
        </div>
      </div>
    </div>
  );
}

function DriverPanel({ drivers, salaryCeiling }) {
  return (
    <GlassCard hover={false} className="rounded-[28px]">
      <p className="text-[10px] uppercase tracking-[0.22em] text-accent-primary">Dupla de pilotos</p>
      <h3 className="mt-2 text-xl font-semibold text-text-primary">Contratos e peso na folha</h3>
      <div className="mt-5 space-y-3">
        {drivers.map((driver) => (
          <DriverRow key={driver.role} driver={driver} salaryCeiling={salaryCeiling} />
        ))}
      </div>
    </GlassCard>
  );
}

function DriverRow({ driver, salaryCeiling }) {
  const weight = salaryCeiling > 0 ? (driver.salary / salaryCeiling) * 100 : 0;
  return (
    <div className={`rounded-[22px] border p-4 ${driver.highlight ? "border-accent-primary/35 bg-accent-primary/10" : "border-white/8 bg-white/[0.03]"}`}>
      <div className="flex items-start justify-between gap-3">
        <div>
          <p className="text-[10px] uppercase tracking-[0.2em] text-text-muted">{driver.role}</p>
          <h4 className="mt-1 text-base font-semibold text-text-primary">{driver.name}</h4>
          <p className="mt-2 inline-flex items-center gap-2 rounded-full border border-white/8 bg-black/10 px-2.5 py-1 text-xs text-text-secondary">
            <FlagIcon nacionalidade={driver.nationality} className="shrink-0" />
            <span>{driver.nationalityLabel}</span>
          </p>
        </div>
        <div className="text-right">
          <p className="text-[10px] uppercase tracking-[0.18em] text-text-muted">Salário {driver.role}</p>
          <p className="mt-1 font-mono text-sm text-status-green">{formatMoney(driver.salary)}</p>
        </div>
      </div>
      <div className="mt-4">
        <div className="mb-2 flex items-center justify-between text-[10px] uppercase tracking-[0.16em] text-text-muted">
          <span>Peso na folha</span>
          <span>{formatPercent(weight)}</span>
        </div>
        <div className="h-2 rounded-full bg-white/10">
          <div className="h-2 rounded-full bg-gradient-to-r from-accent-primary to-status-green" style={{ width: `${clamp(weight, 4, 100)}%` }} />
        </div>
      </div>
    </div>
  );
}

function TechPanel({ team, activeAxis, setActiveAxis }) {
  const metrics = technicalMetrics(team, activeAxis);
  return (
    <GlassCard hover={false} className="rounded-[28px]">
      <p className="text-[10px] uppercase tracking-[0.22em] text-accent-primary">Operação técnica</p>
      <h3 className="mt-2 text-xl font-semibold text-text-primary">Eixos técnicos</h3>
      <div className="mt-5 grid grid-cols-3 gap-2">
        {TECH_AXES.map((axis) => (
          <button
            key={axis.id}
            type="button"
            onClick={() => setActiveAxis(axis.id)}
            className={`rounded-2xl border px-3 py-2 text-[10px] font-semibold uppercase tracking-[0.13em] transition-glass ${
              axis.id === activeAxis
                ? "border-accent-primary/40 bg-accent-primary/15 text-accent-primary"
                : "border-white/8 bg-black/10 text-text-muted hover:text-text-primary"
            }`}
          >
            {axis.label}
          </button>
        ))}
      </div>
      <div className="mt-5 rounded-[24px] border border-white/8 bg-black/10 p-4">
        <div className="mt-5 space-y-4">
          {metrics.map((metric) => (
            <MetricBar key={metric.label} {...metric} />
          ))}
        </div>
      </div>
    </GlassCard>
  );
}

function FinanceDossier({ team, drivers }) {
  const [showSecondaryCashIndicators, setShowSecondaryCashIndicators] = useState(false);
  const net = team?.last_round_net ?? 0;
  const timeline = cashTimeline(team);
  const payroll = drivers.reduce((sum, driver) => sum + driver.salary, 0);
  const peakCash = Math.max(...timeline.map((point) => point.value));
  const lowCash = Math.min(...timeline.map((point) => point.value));
  const openingCash = (team?.cash_balance ?? 0) - net;
  const projectedCash = team?.cash_balance ?? 0;
  const strategyLabel = seasonStrategy(team?.season_strategy);
  const debt = team?.debt_balance ?? 0;
  return (
    <GlassCard hover={false} className="rounded-[28px]">
      <p className="text-[10px] uppercase tracking-[0.24em] text-accent-primary">Rodada atual + acumulado</p>
      <h3 className="mt-2 text-2xl font-semibold text-text-primary">Dossiê financeiro</h3>

      <div className="mt-6 grid gap-3 sm:grid-cols-2 xl:grid-cols-5">
        <Kpi label="Caixa" value={formatMoney(team?.cash_balance ?? 0)} caption="Saldo da operação" />
        <Kpi label="Resultado rodada" value={formatSignedMoney(net)} caption="Última rodada" tone={net >= 0 ? "text-status-green" : "text-status-red"} />
        <Kpi label="Dívida" value={formatMoney(debt)} caption="Passivo atual" tone={debt > 0 ? "text-status-red" : "text-text-primary"} />
        <Kpi label="Teto salarial" value={formatMoney(team?.salary_ceiling ?? 0)} caption="Limite atual" />
        <Kpi label="Poder de gasto" value={formatSignedMoney(team?.spending_power ?? 0)} caption="Margem de investimento" tone={(team?.spending_power ?? 0) >= 0 ? "text-status-green" : "text-status-red"} />
      </div>

      <div className="mt-5 grid gap-4 lg:grid-cols-2">
        <Ledger title="Entradas da rodada" rows={incomeRows(team)} positive />
        <Ledger title="Saídas da rodada" rows={expenseRows(team)} />
      </div>

      <div className="mt-5 rounded-[24px] border border-white/8 bg-black/10 p-5">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <p className="text-[10px] uppercase tracking-[0.22em] text-text-muted">Linha do tempo do caixa acumulado</p>
            <h4 className="mt-2 text-lg font-semibold text-text-primary">Projeção de caixa</h4>
          </div>
          <div className="flex flex-wrap gap-2">
            <span className="rounded-full border border-accent-primary/25 bg-accent-primary/10 px-3 py-1 text-[10px] uppercase tracking-[0.16em] text-accent-primary">Acumulado</span>
            <span className="rounded-full border border-white/10 bg-white/[0.04] px-3 py-1 text-[10px] uppercase tracking-[0.16em] text-text-secondary">
              Estratégia da temporada: <span className="text-text-primary">{strategyLabel}</span>
            </span>
          </div>
        </div>

        <div className="mt-5 grid gap-3 sm:grid-cols-2 xl:grid-cols-5">
          <Kpi compact label="Caixa inicial estimado" value={formatMoney(openingCash)} tone={moneyTone(openingCash)} />
          <Kpi compact label="Entradas" value={`+${formatMoney(team?.last_round_income ?? 0)}`} tone="text-status-green" />
          <Kpi compact label="Saídas" value={`-${formatMoney(team?.last_round_expenses ?? 0)}`} tone="text-status-red" />
          <Kpi compact label="Dívida" value={formatMoney(debt)} tone={debt > 0 ? "text-status-red" : "text-text-primary"} />
          <Kpi compact label="Caixa projetado" value={formatMoney(projectedCash)} tone={moneyTone(projectedCash)} />
        </div>

        <div className="mt-6 flex h-56 items-end gap-2 rounded-[22px] border border-white/6 bg-white/[0.02] px-4 pb-4 pt-8">
          {timeline.map((point) => (
            <div key={point.label} className="flex h-full flex-1 flex-col justify-end gap-2">
              <div
                className={`min-h-3 rounded-t-xl bg-gradient-to-t ${
                  point.value < 0 ? "from-status-red to-status-red" : "from-accent-primary/70 to-accent-hover"
                }`}
                data-testid={point.value < 0 ? "cash-timeline-negative" : undefined}
                style={{ height: `${point.height}%` }}
                title={`${point.label}: ${formatMoney(point.value)}`}
              />
              <span className="text-center font-mono text-[10px] text-text-muted">{point.label}</span>
            </div>
          ))}
        </div>
        <div className="mt-4 rounded-[22px] border border-white/8 bg-white/[0.025] p-3">
          <button
            type="button"
            onClick={() => setShowSecondaryCashIndicators((value) => !value)}
            className="flex w-full items-center justify-between gap-3 rounded-2xl px-2 py-1 text-left text-[10px] font-semibold uppercase tracking-[0.16em] text-text-muted transition-glass hover:text-text-primary"
          >
            <span>
              {showSecondaryCashIndicators
                ? "Ocultar indicadores secundários"
                : "Ver indicadores secundários"}
            </span>
            <span className="text-accent-primary">{showSecondaryCashIndicators ? "−" : "+"}</span>
          </button>

          {showSecondaryCashIndicators ? (
            <>
              <FinancialRiskPanel
                cash={team?.cash_balance ?? 0}
                debt={debt}
                income={team?.last_round_income ?? 0}
                net={net}
              />
              <div className="mt-3 grid gap-3 sm:grid-cols-4">
                <Kpi compact label="Pico de caixa" value={formatMoney(peakCash)} tone={moneyTone(peakCash)} />
                <Kpi compact label="Pior trecho" value={formatMoney(lowCash)} tone={moneyTone(lowCash)} />
                <Kpi compact label="Média por rodada" value={formatSignedMoney(net)} tone={moneyTone(net)} />
                <Kpi compact label="Folha anual" value={formatMoney(payroll)} />
              </div>
            </>
          ) : null}
        </div>
        {team?.parachute_payment_remaining > 0 ? (
          <p className="mt-4 rounded-2xl border border-accent-primary/20 bg-accent-primary/10 px-4 py-3 text-sm text-accent-primary">
            Auxílio de rebaixamento restante: {formatMoney(team.parachute_payment_remaining)}
          </p>
        ) : null}
      </div>

      <div className="mt-5">
        <ExecutiveReading team={team} net={net} payroll={payroll} />
      </div>
    </GlassCard>
  );
}

function Kpi({ label, value, caption, tone = "text-text-primary", compact = false }) {
  return (
    <div className={`rounded-2xl border border-white/8 bg-white/[0.03] ${compact ? "p-3" : "p-4"}`}>
      <p className="text-[10px] uppercase tracking-[0.18em] text-text-muted">{label}</p>
      <p className={`mt-2 font-mono ${compact ? "text-sm" : "text-lg"} font-semibold ${tone}`}>{value}</p>
      {caption ? <p className="mt-1 text-xs text-text-secondary">{caption}</p> : null}
    </div>
  );
}

function FinancialRiskPanel({ cash, debt, income, net }) {
  const liquidBalance = cash - debt;
  const margin = income > 0 ? (net / income) * 100 : 0;
  const runway = operationalRunway(cash, net);

  return (
    <div className="mt-5 rounded-[22px] border border-white/8 bg-white/[0.025] p-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <p className="text-[10px] uppercase tracking-[0.2em] text-text-muted">
          Painel de risco financeiro
        </p>
        <span className="rounded-full border border-white/10 bg-black/10 px-3 py-1 text-[10px] uppercase tracking-[0.16em] text-text-secondary">
          Leitura rapida
        </span>
      </div>
      <div className="mt-4 grid gap-3 md:grid-cols-3">
        <RiskCard
          label="Saldo líquido"
          value={formatMoney(liquidBalance)}
          caption="Caixa descontando a dívida"
          tone={moneyTone(liquidBalance)}
        />
        <RiskCard
          label="Margem da rodada"
          value={formatPercent(margin)}
          caption="Resultado dividido pelas entradas"
          tone={margin >= 0 ? "text-status-green" : "text-status-red"}
        />
        <RiskCard
          label="Fôlego operacional"
          value={runway.value}
          caption={runway.caption}
          tone={runway.tone}
        />
      </div>
    </div>
  );
}

function RiskCard({ label, value, caption, tone }) {
  return (
    <div className="rounded-2xl border border-white/8 bg-black/10 p-4">
      <p className="text-[10px] uppercase tracking-[0.18em] text-text-muted">{label}</p>
      <p className={`mt-2 font-mono text-lg font-semibold ${tone}`}>{value}</p>
      <p className="mt-1 text-xs leading-5 text-text-secondary">{caption}</p>
    </div>
  );
}

function Ledger({ title, rows, positive = false }) {
  return (
    <div className="rounded-[24px] border border-white/8 bg-white/[0.03] p-4">
      <p className="text-[10px] uppercase tracking-[0.2em] text-text-muted">{title}</p>
      <div className="mt-4 space-y-3">
        {rows.map((row) => (
          <div key={row.label} className="flex items-center justify-between gap-3 border-b border-white/6 pb-2 last:border-0 last:pb-0">
            <span className="text-sm text-text-primary">{row.label}</span>
            <span className={`font-mono text-sm ${positive ? "text-status-green" : "text-status-red"}`}>{positive ? "+" : "-"}{formatMoney(row.value)}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function CostChart() {
  const rows = [
    ["Salários", 42, "#ff6b6b"],
    ["Operação", 24, "#58a6ff"],
    ["Manutenção", 20, "#f59e0b"],
    ["Investimento", 14, "#22c55e"],
  ];
  let cursor = 0;
  const gradient = rows.map(([, percent, color]) => {
    const start = cursor;
    cursor += percent;
    return `${color} ${start}% ${cursor}%`;
  }).join(", ");
  return (
    <div className="rounded-[24px] border border-white/8 bg-white/[0.03] p-5">
      <p className="text-[10px] uppercase tracking-[0.2em] text-text-muted">Distribuição dos custos acumulados</p>
      <div className="mt-5 grid gap-5 sm:grid-cols-[140px_1fr] xl:grid-cols-1 2xl:grid-cols-[150px_1fr]">
        <div className="mx-auto grid h-36 w-36 place-items-center rounded-full 2xl:h-40 2xl:w-40" style={{ background: `conic-gradient(${gradient})` }}>
          <div className="grid h-20 w-20 place-items-center rounded-full bg-bg-primary text-[10px] font-semibold uppercase tracking-[0.14em] text-text-primary 2xl:h-24 2xl:w-24">Custos</div>
        </div>
        <div className="space-y-3 self-center">
          {rows.map(([label, percent, color]) => (
            <div key={label} className="rounded-2xl border border-white/6 bg-black/10 px-3 py-2 text-xs">
              <div className="flex items-center justify-between gap-2">
                <span className="flex items-center gap-2 text-text-secondary"><span className="h-2 w-2 rounded-full" style={{ backgroundColor: color }} />{label}</span>
                <span className="font-mono text-text-primary">{percent}%</span>
              </div>
              <div className="mt-2 h-1.5 rounded-full bg-white/10">
                <div className="h-1.5 rounded-full" style={{ width: `${percent}%`, backgroundColor: color }} />
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

function ExecutiveReading({ team, net, payroll }) {
  const signals = buildExecutiveSignals(team, net, payroll);
  return (
    <div className="rounded-[24px] border border-white/8 bg-white/[0.03] p-5">
      <p className="text-[10px] uppercase tracking-[0.2em] text-text-muted">Leitura executiva</p>
      <div className="mt-4 grid gap-3 sm:grid-cols-2">
        {signals.map((signal) => (
          <div key={signal.label} className="rounded-2xl border border-white/8 bg-black/10 p-3">
            <p className={`text-sm font-semibold ${signal.tone}`}>{signal.label}</p>
            <p className="mt-1 text-xs leading-5 text-text-secondary">{signal.detail}</p>
          </div>
        ))}
      </div>
    </div>
  );
}

function buildExecutiveSignals(team, net, payroll) {
  const cash = team?.cash_balance ?? 0;
  const debt = team?.debt_balance ?? 0;
  const spending = team?.spending_power ?? 0;
  const salaryCeiling = team?.salary_ceiling ?? 0;
  const payrollPressure = salaryCeiling > 0 ? (payroll / salaryCeiling) * 100 : 0;
  const debtPressure = cash > 0 ? debt / cash : debt > 0 ? 1 : 0;

  return [
    {
      label: net >= 0 ? "Rodada positiva" : "Rodada negativa",
      detail: `${net >= 0 ? "Ganho" : "Perda"} de ${formatMoney(Math.abs(net))} na última rodada`,
      tone: net >= 0 ? "text-status-green" : "text-status-red",
    },
    {
      label: debtPressure > 0.5 ? "Dívida alta" : "Dívida controlada",
      detail: `${formatMoney(debt)} em passivo`,
      tone: debtPressure > 0.5 ? "text-status-red" : "text-text-primary",
    },
    {
      label: spending < 0 ? "Gasto restrito" : "Margem de gasto",
      detail: formatSignedMoney(spending),
      tone: spending < 0 ? "text-status-red" : "text-status-green",
    },
    {
      label: "Folha salarial",
      detail: `${formatPercent(payrollPressure)} do teto`,
      tone: payrollPressure > 90 ? "text-status-red" : "text-text-primary",
    },
  ];
}

function RankingTable({ teams, playerTeam, historyTeamId, onTeamHistoryOpen }) {
  const rows = Array.isArray(teams) ? teams : [];
  const [sort, setSort] = useState({ key: "default", direction: "asc" });
  const sortedRows = sortRankingRows(rows, sort);

  function handleSort(key) {
    setSort((current) => {
      if (current.key === key) {
        return { key, direction: current.direction === "asc" ? "desc" : "asc" };
      }
      return { key, direction: defaultSortDirection(key) };
    });
  }

  return (
    <GlassCard hover={false} className="rounded-[28px]">
      <p className="text-[10px] uppercase tracking-[0.22em] text-accent-primary">Comparativo de gestão e performance</p>
      <h3 className="mt-2 text-xl font-semibold text-text-primary">Ranking da categoria</h3>
      <div className="mt-5 overflow-x-auto">
        <table className="min-w-full text-left text-sm" aria-label="Ranking da categoria">
          <thead>
            <tr className="border-b border-white/8 text-[10px] uppercase tracking-[0.18em] text-text-muted">
              <SortableHeader label="#" sortKey="posicao" sort={sort} onSort={handleSort} className="py-3 pr-4" />
              <SortableHeader label="Equipe" sortKey="nome" sort={sort} onSort={handleSort} />
              <SortableHeader label="Dinheiro" sortKey="cash_balance" sort={sort} onSort={handleSort} />
              <SortableHeader label="Nível do carro" sortKey="car_performance" sort={sort} onSort={handleSort} />
              <SortableHeader label="Tipo do carro" sortKey="car_build_profile" sort={sort} onSort={handleSort} />
              <SortableHeader label="Pontos" sortKey="pontos" sort={sort} onSort={handleSort} />
            </tr>
          </thead>
          <tbody>
            {sortedRows.slice(0, 10).map((team, index) => (
              <tr
                key={team.id}
                className={[
                  "border-b border-white/6 last:border-0 transition-all duration-200",
                  team.id === historyTeamId
                    ? "bg-status-yellow/10 text-text-primary ring-1 ring-status-yellow/45 shadow-[inset_4px_0_0_rgba(242,196,109,0.95)]"
                    : team.id === playerTeam?.id
                      ? "bg-accent-primary/10 text-text-primary"
                      : "text-text-secondary",
                ].join(" ")}
                data-history-active={team.id === historyTeamId ? "true" : undefined}
              >
                <td className="py-3 pr-4 font-mono text-xs text-text-muted">{String(team.posicao ?? index + 1).padStart(2, "0")}</td>
                <td className="px-4 py-3 font-semibold">
                  <div className="flex items-center gap-3">
                    <TeamLogoMark
                      teamName={team.nome}
                      color={team.cor_primaria}
                      size="sm"
                      testId="ranking-team-logo"
                    />
                    <button
                      type="button"
                      data-testid="ranking-team-name"
                      onDoubleClick={() => onTeamHistoryOpen?.(team)}
                      className="rounded-lg text-left transition-glass hover:brightness-125 focus:outline-none focus:ring-2 focus:ring-accent-primary/45"
                      style={{ color: team.cor_primaria ?? "#f0f6fc" }}
                      title="Duplo clique para abrir o histórico da equipe"
                    >
                      {team.nome}
                    </button>
                  </div>
                </td>
                <td className="px-4 py-3 font-mono">{formatMoney(team.cash_balance ?? 0)}</td>
                <td className="px-4 py-3 font-mono">{carLevel(team.car_performance)}</td>
                <td className="px-4 py-3 font-mono">{buildMeta(team.car_build_profile).label}</td>
                <td className="px-4 py-3 font-mono text-text-primary">{team.pontos ?? 0}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </GlassCard>
  );
}

function SortableHeader({ label, sortKey, sort, onSort, className = "px-4 py-3" }) {
  const isActive = sort.key === sortKey;
  const indicator = isActive ? (sort.direction === "asc" ? "↑" : "↓") : "↕";

  return (
    <th className={className}>
      <button
        type="button"
        onClick={() => onSort(sortKey)}
        className="inline-flex items-center gap-1 rounded-lg text-left transition-glass hover:text-text-primary"
      >
        <span>{label}</span>
        <span className={isActive ? "text-accent-primary" : "text-text-muted"}>{indicator}</span>
      </button>
    </th>
  );
}

function TeamNavChevron({ direction }) {
  const path = direction === "up" ? "M2 7.5 6 3.5l4 4" : "M2 4.5l4 4 4-4";
  return (
    <svg
      viewBox="0 0 12 12"
      aria-hidden="true"
      className="h-3.5 w-3.5 flex-shrink-0"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d={path} />
    </svg>
  );
}

function TeamNavigatorButton({ label, direction, disabled, onClick }) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      aria-label={label}
      className={[
        "flex h-10 w-10 items-center justify-center rounded-2xl border backdrop-blur-md transition-all duration-200 ease-out",
        disabled
          ? "cursor-not-allowed border-white/[0.05] bg-[#11151b]/90 text-[#5b616b]"
          : "border-white/[0.12] bg-[#111d31]/95 text-text-secondary shadow-[0_14px_34px_rgba(0,0,0,0.34)] hover:border-white/[0.18] hover:bg-[#18263d] hover:text-text-primary focus-visible:border-white/[0.18] focus-visible:bg-[#18263d] focus-visible:text-text-primary",
      ].join(" ")}
    >
      <TeamNavChevron direction={direction} />
    </button>
  );
}

function TeamHistoryEdgeNavigator({ previousTeam, nextTeam, onSelectTeam, placement = "right" }) {
  return (
    <div
      className={[
        "pointer-events-auto fixed top-24 z-[91] flex flex-col gap-2 max-lg:hidden sm:top-28",
        placement === "left" ? "animate-edge-rail-in-right" : "animate-edge-rail-in",
      ].join(" ")}
      style={placement === "left"
        ? { left: "calc(min(50vw, 720px) + 14px)" }
        : { right: "calc(min(50vw, 720px) + 14px)" }}
    >
      <TeamNavigatorButton
        label="Equipe anterior"
        direction="up"
        disabled={!previousTeam}
        onClick={() => previousTeam && onSelectTeam(previousTeam)}
      />
      <TeamNavigatorButton
        label="Próxima equipe"
        direction="down"
        disabled={!nextTeam}
        onClick={() => nextTeam && onSelectTeam(nextTeam)}
      />
    </div>
  );
}

export function TeamHistoryDrawer({
  careerId,
  team,
  teams,
  playerTeam,
  activeCategory,
  activeTab,
  onTabChange,
  onSelectTeam,
  onClose,
  placement = "right",
}) {
  const [historyDossier, setHistoryDossier] = useState(null);
  const [historyStatus, setHistoryStatus] = useState("loading");
  const [historyError, setHistoryError] = useState("");
  const dossier = buildTeamHistoryDossier(
    team,
    teams,
    playerTeam,
    activeCategory,
    historyDossier,
    historyStatus,
    historyError,
  );
  const orderedTeams = orderTeamsForHistoryNavigation(teams);
  const currentTeamIndex = orderedTeams.findIndex((entry) => entry.id === team?.id);
  const previousTeam = currentTeamIndex > 0 ? orderedTeams[currentTeamIndex - 1] : null;
  const nextTeam = currentTeamIndex >= 0 && currentTeamIndex < orderedTeams.length - 1
    ? orderedTeams[currentTeamIndex + 1]
    : null;

  useEffect(() => {
    let mounted = true;
    if (!careerId || !team?.id) {
      setHistoryStatus("error");
      setHistoryError("Histórico real indisponível.");
      return undefined;
    }

    setHistoryStatus("loading");
    setHistoryError("");
    setHistoryDossier(null);
    invoke("get_team_history_dossier", {
      careerId,
      teamId: team.id,
      category: activeCategory ?? playerTeam?.categoria ?? team?.categoria ?? "",
    })
      .then((payload) => {
        if (!mounted) return;
        setHistoryDossier(payload);
        setHistoryStatus("ready");
      })
      .catch((invokeError) => {
        if (!mounted) return;
        setHistoryError(typeof invokeError === "string" ? invokeError : "Não foi possível carregar o histórico real da equipe.");
        setHistoryStatus("error");
      });

    return () => {
      mounted = false;
    };
  }, [activeCategory, careerId, team?.id, team?.categoria, playerTeam?.categoria]);

  const drawerLayer = (
    <div className="fixed inset-0 z-[90]" data-testid="team-history-layer" aria-hidden={false}>
      <button
        type="button"
        aria-label="Fechar histórico da equipe"
        onClick={onClose}
        className="absolute inset-0 cursor-default bg-black/70 backdrop-blur-[3px]"
      />
      <TeamHistoryEdgeNavigator
        previousTeam={previousTeam}
        nextTeam={nextTeam}
        onSelectTeam={onSelectTeam}
        placement={placement}
      />
      <aside
        role="dialog"
        aria-modal="true"
        aria-labelledby="team-history-title"
        className={[
          placement === "left" ? "animate-drawer-in-left left-0 border-r shadow-[28px_0_80px_rgba(0,0,0,0.72)]" : "animate-drawer-in right-0 border-l shadow-[-28px_0_80px_rgba(0,0,0,0.72)]",
          "absolute inset-y-0 w-[min(50vw,720px)] overflow-y-auto border-white/15 bg-[#07101d] max-lg:w-full",
        ].join(" ")}
        data-testid="team-history-drawer"
        style={{
          "--team": dossier.color,
          backgroundImage:
            "radial-gradient(circle at 10% 4%, color-mix(in srgb, var(--team) 16%, transparent), transparent 18rem), linear-gradient(180deg, rgba(12,22,38,0.98), rgba(5,11,20,0.995))",
        }}
      >
        <div className="h-1.5 bg-[linear-gradient(90deg,var(--team),rgba(255,255,255,0.1))]" />
        <button
          type="button"
          onClick={onClose}
          aria-label="Fechar"
          className="absolute right-4 top-4 grid h-9 w-9 place-items-center rounded-xl border border-white/15 bg-[#0d1727] text-text-secondary transition-glass hover:bg-[#14233a] hover:text-text-primary"
        >
          x
        </button>

        <div className="px-6 pb-7 pt-6">
          <section className="rounded-[26px] border border-[color-mix(in_srgb,var(--team)_42%,transparent)] bg-[#0c1626]/95 p-5 shadow-[0_18px_55px_rgba(0,0,0,0.32)]">
            <div className="grid min-w-0 gap-5 pr-10 sm:grid-cols-[168px_minmax(0,1fr)] sm:items-center">
              <TeamLogoMark
                teamName={dossier.name}
                color={dossier.color}
                size="hero"
                testId="team-history-logo"
              />
              <div className="min-w-0">
                <h2 id="team-history-title" className="min-w-0 truncate text-3xl font-semibold leading-none tracking-[-0.04em] text-text-primary">
                  {dossier.name}
                </h2>
                <div className="mt-4 flex flex-wrap gap-2">
                  <span className="rounded-full border border-white/15 bg-[#08111f] px-3 py-1 text-xs text-text-primary">
                    {dossier.state}
                  </span>
                  <span className="rounded-full border border-white/15 bg-[#08111f] px-3 py-1 text-xs text-text-primary">
                    Fundada em {dossier.founded}
                  </span>
                </div>
              </div>
            </div>
          </section>

          <div role="tablist" aria-label="Abas do arquivo compacto" className="mt-4 flex gap-2 overflow-x-auto pb-1">
            {TEAM_HISTORY_TABS.map((tab) => (
              <button
                key={tab.id}
                type="button"
                role="tab"
                aria-selected={activeTab === tab.id}
                onClick={() => onTabChange(tab.id)}
                className={`shrink-0 rounded-full border px-3 py-2 text-[10px] font-semibold uppercase tracking-[0.13em] transition-glass ${
                  activeTab === tab.id
                    ? "border-[color-mix(in_srgb,var(--team)_55%,transparent)] bg-[color-mix(in_srgb,var(--team)_18%,transparent)] text-text-primary"
                    : "border-white/12 bg-[#0b1524] text-text-secondary hover:border-white/20 hover:bg-[#111d31] hover:text-text-primary"
                }`}
              >
                {tab.label}
              </button>
            ))}
          </div>

          <div className="mt-4">
            {activeTab === "records" ? <TeamHistoryRecords dossier={dossier} /> : null}
            {activeTab === "sport" ? <TeamHistorySport dossier={dossier} /> : null}
            {activeTab === "identity" ? <TeamHistoryIdentity dossier={dossier} /> : null}
            {activeTab === "management" ? <TeamHistoryManagement dossier={dossier} /> : null}
            {activeTab === "categories" ? <TeamHistoryCategories dossier={dossier} /> : null}
          </div>
        </div>
      </aside>
    </div>
  );

  return createPortal(drawerLayer, document.body);
}

function TeamHistoryRecords({ dossier }) {
  return (
    <section>
      <h3 className="text-[11px] uppercase tracking-[0.2em] text-accent-primary">Records históricos</h3>
      <p className="mt-2 rounded-2xl border border-white/12 bg-[#08111f]/95 px-4 py-3 text-xs leading-5 text-text-secondary">
        Comparativo em <strong className="text-text-primary">{dossier.recordScope}</strong>. Grupos equivalentes entram juntos para evitar comparar carros de mundos diferentes.
      </p>
      {dossier.historyStatus !== "ready" ? (
        <HistoryStateMessage dossier={dossier} />
      ) : null}
      <div className="mt-4 space-y-2">
        {dossier.records.map((record) => (
          <div key={record.label} className="flex items-center justify-between gap-3 border-b border-white/6 py-3 last:border-0">
            <span className="text-[10px] uppercase tracking-[0.16em] text-text-muted">
              {record.label} <em className="not-italic text-accent-primary">({record.rank})</em>
            </span>
            <strong className="font-mono text-lg text-text-primary">{record.value}</strong>
          </div>
        ))}
      </div>
      <div className="mt-4 grid gap-2">
        {dossier.titleCategories.map((item) => (
          <div key={`${item.category}-${item.year}`} className="rounded-2xl border border-l-4 border-white/12 bg-[#0c1626]/95 px-4 py-3" style={{ borderLeftColor: item.color }}>
            <div className="flex items-center justify-between gap-3">
              <strong className="text-sm text-text-primary">{item.category}</strong>
              <span className="font-mono text-xs text-status-yellow">{item.year}</span>
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

function TeamHistorySport({ dossier }) {
  return (
    <section>
      <h3 className="text-[11px] uppercase tracking-[0.2em] text-accent-primary">Resumo esportivo</h3>
      {dossier.historyStatus !== "ready" ? (
        <HistoryStateMessage dossier={dossier} />
      ) : null}
      <div className="mt-4 grid gap-3">
        <HistoryInfoCard label="Temporadas disputadas" value={dossier.sport.seasons} detail={`Dentro de ${dossier.recordScope}.`} />
        <HistoryInfoCard label="Sequência atual" value={dossier.sport.currentStreak} />
        <HistoryInfoCard label="Melhor sequência" value={dossier.sport.bestStreak} />
      </div>
      <div className="mt-4 grid grid-cols-2 gap-3">
        <HistoryMiniMetric label="Taxa de pódio" value={dossier.sport.podiumRate} />
        <HistoryMiniMetric label="Taxa de vitória" value={dossier.sport.winRate} />
      </div>
      <TimelineBlock items={dossier.timeline} />
    </section>
  );
}

function HistoryStateMessage({ dossier }) {
  const message = dossier.historyStatus === "error"
    ? dossier.historyError
    : "Carregando histórico real da equipe...";
  return (
    <div className="mt-4 rounded-2xl border border-white/10 bg-[#08111f]/95 px-4 py-3 text-xs text-text-secondary">
      {message}
    </div>
  );
}

function TeamHistoryIdentity({ dossier }) {
  return (
    <section>
      <h3 className="text-[11px] uppercase tracking-[0.2em] text-accent-primary">Identidade da equipe</h3>
      <div className="mt-4 grid gap-3">
        <div className="rounded-[22px] border border-[color-mix(in_srgb,var(--team)_38%,transparent)] bg-[#0c1626] bg-[radial-gradient(circle_at_10%_8%,color-mix(in_srgb,var(--team)_20%,transparent),transparent_12rem),linear-gradient(145deg,rgba(14,26,44,0.96),rgba(7,16,29,0.99))] p-4">
          <span className="text-[9px] font-black uppercase tracking-[0.17em] text-text-muted">Perfil histórico</span>
          <strong className="mt-2 block text-2xl font-semibold leading-none tracking-[-0.03em] text-text-primary">{dossier.identity.profile}</strong>
          <p className="mt-3 text-xs leading-5 text-text-secondary">{dossier.identity.summary}</p>
        </div>
        <div className="grid items-stretch gap-3 md:grid-cols-[1fr_auto_1fr]">
          <HistoryInfoCard label="Categoria de origem" value={dossier.identity.origin} detail="Onde a equipe construiu sua primeira base esportiva." />
          <div className="hidden place-items-center font-mono font-black text-[color:var(--team)] md:grid">-&gt;</div>
          <HistoryInfoCard label="Categoria atual" value={dossier.identity.current} detail="Contexto em que tenta consolidar reputação e legado." />
        </div>
        <div className="grid gap-3 md:grid-cols-2">
          <div className="rounded-[18px] border border-status-yellow/30 bg-[#201a0b]/95 p-4">
            <span className="text-[9px] font-black uppercase tracking-[0.17em] text-text-muted">Maior rival histórico</span>
            <strong className="mt-2 block text-base font-semibold text-status-yellow">{dossier.identity.rival.name}</strong>
            <p className="mt-2 text-xs leading-5 text-text-secondary">
              Hoje em {dossier.identity.rival.currentCategory}. {dossier.identity.rival.note}
            </p>
          </div>
          <div className="rounded-[18px] border border-[color-mix(in_srgb,var(--team)_35%,transparent)] bg-[#0c1626]/95 p-4">
            <span className="text-[9px] font-black uppercase tracking-[0.17em] text-text-muted">Piloto símbolo</span>
            <strong className="mt-2 block text-base font-semibold text-text-primary">{dossier.identity.symbolDriver}</strong>
            <p className="mt-2 text-xs leading-5 text-text-secondary">{dossier.identity.symbolDriverDetail}</p>
          </div>
        </div>
      </div>
    </section>
  );
}

function TeamHistoryManagement({ dossier }) {
  const operationTone = operationHealthTone(dossier.management.operationHealth);

  return (
    <section>
      <h3 className="text-[11px] uppercase tracking-[0.2em] text-accent-primary">Gestão e dinheiro</h3>
      <div className="mt-4 grid gap-3">
        <div className={`rounded-[22px] border p-4 ${operationTone.card}`}>
          <span className="text-[9px] font-black uppercase tracking-[0.17em] text-text-muted">Saúde da operação</span>
          <strong className={`mt-2 block text-2xl font-semibold ${operationTone.text}`}>{dossier.management.operationHealth}</strong>
          <p className="mt-2 text-xs leading-5 text-text-secondary">{dossier.management.summary}</p>
        </div>
        <div className="grid items-stretch gap-3 md:grid-cols-[1fr_auto_1fr]">
          <div className="rounded-[18px] border border-status-green/30 bg-[#0b1d19]/95 p-4">
            <span className="text-[9px] font-black uppercase tracking-[0.17em] text-text-muted">Maior saldo histórico</span>
            <strong className="mt-2 block font-mono text-base text-status-green">{dossier.management.peakCash}</strong>
            <p className="mt-2 text-xs leading-5 text-text-secondary">{dossier.management.peakCashDetail}</p>
          </div>
          <div className="hidden place-items-center font-mono font-black text-text-muted md:grid">&lt;&gt;</div>
          <div className="rounded-[18px] border border-status-red/30 bg-[#241014]/95 p-4">
            <span className="text-[9px] font-black uppercase tracking-[0.17em] text-text-muted">Pior crise financeira</span>
            <strong className="mt-2 block font-mono text-base text-status-red">{dossier.management.worstCrisis}</strong>
            <p className="mt-2 text-xs leading-5 text-text-secondary">{dossier.management.worstCrisisDetail}</p>
          </div>
        </div>
        <div className="grid gap-3 md:grid-cols-2">
          <HistoryInfoCard label="Temporadas saudáveis" value={dossier.management.healthyYears} detail={dossier.management.healthyYearsDetail} />
          <HistoryInfoCard label="Saldo recorde" value={dossier.management.peakCash} detail="Melhor folga já registrada pela operação." />
        </div>
        <HistoryInfoCard label="Maior investimento técnico" value={dossier.management.biggestInvestment} detail={dossier.management.investmentDetail} />
      </div>
    </section>
  );
}

function TeamHistoryCategories({ dossier }) {
  return (
    <section>
      <h3 className="text-[11px] uppercase tracking-[0.2em] text-accent-primary">Movimento por categorias</h3>
      <div className="mt-4 grid grid-cols-2 gap-3">
        <HistoryMiniMetric label="Promoções" value={dossier.movement.promotions} />
        <HistoryMiniMetric label="Rebaixamentos" value={dossier.movement.relegations} />
      </div>
      <div className="mt-4 grid gap-3">
        <HistoryInfoCard label="Tempo por categoria" value={dossier.movement.timeByCategory} />
        <HistoryInfoCard label="Melhor categoria" value={dossier.movement.bestCategory} />
        <HistoryInfoCard label="Categoria mais dificil" value={dossier.movement.hardestCategory} />
      </div>
      <div className="mt-4 grid gap-3">
        {dossier.categoryPath.map((step) => (
          <div key={step.category} className="rounded-2xl border border-l-4 border-white/12 bg-[#0c1626]/95 p-4" style={{ borderLeftColor: step.color }}>
            <div className="flex items-start justify-between gap-3">
              <strong className="text-sm text-text-primary">{step.category}</strong>
              <span className="font-mono text-xs font-semibold" style={{ color: step.color }}>{step.years}</span>
            </div>
            <p className="mt-2 text-xs leading-5 text-text-secondary">{step.detail}</p>
          </div>
        ))}
      </div>
    </section>
  );
}

function HistoryInfoCard({ label, value, detail = "" }) {
  return (
    <div className="rounded-[18px] border border-white/12 bg-[#0c1626]/95 p-4">
      <div className="flex items-start justify-between gap-3">
        <strong className="text-sm text-text-primary">{label}</strong>
        <span className="text-right font-mono text-xs font-semibold text-status-yellow">{value}</span>
      </div>
      {detail ? <p className="mt-2 text-xs leading-5 text-text-secondary">{detail}</p> : null}
    </div>
  );
}

function HistoryMiniMetric({ label, value }) {
  return (
    <div className="rounded-2xl border border-white/12 bg-[#08111f]/95 p-3">
      <span className="text-[9px] font-black uppercase tracking-[0.15em] text-text-muted">{label}</span>
      <strong className="mt-2 block font-mono text-lg text-text-primary">{value}</strong>
    </div>
  );
}

function TimelineBlock({ items }) {
  return (
    <div className="mt-5 rounded-[22px] border border-white/12 bg-[#0c1626]/95 p-4">
      <h4 className="text-[10px] uppercase tracking-[0.2em] text-text-muted">Momentos-chave</h4>
      <div className="mt-4 space-y-3">
        {items.map((item) => (
          <div key={item.year} className="grid grid-cols-[52px_1fr] gap-3 border-b border-white/6 pb-3 last:border-0 last:pb-0">
            <span className="font-mono text-xs font-semibold text-accent-primary">{item.year}</span>
            <p className="text-xs leading-5 text-text-secondary">{item.text}</p>
          </div>
        ))}
      </div>
    </div>
  );
}

function sortRankingRows(rows, sort) {
  if (sort.key === "default") return rows;
  const direction = sort.direction === "asc" ? 1 : -1;

  return [...rows].sort((a, b) => {
    const result = compareRankingValues(rankingSortValue(a, sort.key), rankingSortValue(b, sort.key));
    if (result !== 0) return result * direction;
    return compareRankingValues(a.posicao ?? 999, b.posicao ?? 999);
  });
}

function rankingSortValue(team, key) {
  if (key === "nome") return team.nome ?? "";
  if (key === "car_build_profile") return buildMeta(team.car_build_profile).label;
  return team?.[key] ?? 0;
}

function compareRankingValues(a, b) {
  if (typeof a === "string" || typeof b === "string") {
    return String(a).localeCompare(String(b), "pt-BR");
  }
  return Number(a) - Number(b);
}

function defaultSortDirection(key) {
  return ["cash_balance", "car_performance", "pontos"].includes(key) ? "desc" : "asc";
}

function orderTeamsForHistoryNavigation(teams) {
  return [...(Array.isArray(teams) ? teams : [])].sort((a, b) => {
    const positionDiff = (a.posicao ?? 999) - (b.posicao ?? 999);
    if (positionDiff !== 0) return positionDiff;
    return String(a.nome ?? "").localeCompare(String(b.nome ?? ""), "pt-BR");
  });
}

function buildTeamHistoryDossier(
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
  const rankingIndex = rankedTeams.findIndex((entry) => entry.id === mergedTeam?.id);
  const currentPosition = mergedTeam?.posicao ?? (rankingIndex >= 0 ? rankingIndex + 1 : 1);
  const rival = findHistoricRival(mergedTeam, rankedTeams);
  const peakCash = Math.max(mergedTeam?.cash_balance ?? 0, (mergedTeam?.cash_balance ?? 0) + Math.max(mergedTeam?.last_round_income ?? 0, 0) * 3);
  const debt = mergedTeam?.debt_balance ?? estimateHistoricDebt(mergedTeam);
  const founded = resolveTeamFoundedYear(mergedTeam, rankedTeams, currentPosition, category);
  const origin = originCategoryLabel(category, currentPosition);
  const profile = teamHistoryProfile(mergedTeam, currentPosition);

  return {
    name: mergedTeam?.nome ?? "Equipe",
    color: mergedTeam?.cor_primaria ?? "#58a6ff",
    state: teamHeritageLabel(founded),
    founded,
    currentCategory: categoryName,
    recordScope: realHistory?.recordScope ?? categoryGroupLabel(category),
    historyStatus,
    historyError,
    hasHistory: realHistory?.hasHistory ?? false,
    records: realHistory?.records ?? [],
    titleCategories: realHistory?.titleCategories ?? [],
    sport: realHistory?.sport ?? emptyRealSport(),
    identity: realHistory?.identity ?? {
      origin,
      current: categoryName,
      profile,
      summary: identitySummary(mergedTeam, profile, currentPosition),
      rival: {
        name: rival?.nome ?? "Sem rival consolidado",
        currentCategory: categoryName,
        note: rival
          ? `Disputa direta de referência dentro do ${categoryGroupLabel(category)}, com proximidade em pontos e desenvolvimento.`
          : "Histórico ainda sem confronto forte o bastante para formar rivalidade.",
      },
      symbolDriver: mergedTeam?.piloto_1_nome ?? mergedTeam?.driver_name ?? "Piloto principal",
      symbolDriverDetail: "Nome mais associado ao momento competitivo recente da escuderia.",
    },
    management: realHistory?.management ?? {
      operationHealth: financialState(mergedTeam?.financial_state),
      peakCash: formatMoney(peakCash),
      worstCrisis: debt > 0 ? `${formatMoney(debt)} de dívida` : "Sem dívida relevante",
      healthyYears: `${healthySeasonEstimate(mergedTeam, founded)} Temporadas`,
      efficiency: managementEfficiency(mergedTeam),
      biggestInvestment: `${2026 - Math.min(1, currentPosition - 1)} - pacote técnico`,
      summary: managementSummary(mergedTeam),
      peakCashDetail: "Pico estimado a partir do caixa atual, prêmio recente e força de patrocínio.",
      worstCrisisDetail: debt > 0 ? "Período de maior pressão financeira registrado no ciclo recente." : "Operação sem crise financeira severa no recorte atual.",
      healthyYearsDetail: "Histórico sem dívida relevante.",
      efficiencyDetail: "Pontos conquistados em relação ao dinheiro disponível.",
      investmentDetail: "Ano em que a equipe mais converteu recursos em evolução do carro.",
    },
    movement: {
      promotions: Math.max(0, getCategoryTier(category) - 2),
      relegations: mergedTeam?.relegations ?? 0,
      timeByCategory: `${origin}: ${Math.max(1, 2026 - founded - 1)} anos | ${shortCategoryLabel(categoryName)}: ${Math.max(1, Math.min(4, 2026 - founded))} anos`,
      bestCategory: categoryName,
      hardestCategory: currentPosition <= 3 ? origin : categoryName,
    },
    categoryPath: realHistory?.categoryPath ?? [],
    timeline: realHistory?.timeline ?? [],
  };
}

function normalizeTeamHistoryPayload(payload) {
  if (!payload) return null;
  const sport = payload.sport ?? {};
  const identity = payload.identity ?? {};
  const rival = identity.rival ?? {};
  const management = payload.management ?? {};
  return {
    recordScope: payload.record_scope ?? payload.recordScope ?? "Grupo da categoria",
    hasHistory: Boolean(payload.has_history ?? payload.hasHistory),
    records: (payload.records ?? []).map((record) => ({
      label: record.label,
      rank: record.rank,
      value: String(record.value),
    })),
    sport: {
      seasons: sport.seasons ?? "Sem temporadas registradas",
      currentStreak: sport.current_streak ?? sport.currentStreak ?? "Sem sequência registrada",
      bestStreak: sport.best_streak ?? sport.bestStreak ?? "Sem sequência registrada",
      podiumRate: sport.podium_rate ?? sport.podiumRate ?? "0%",
      winRate: sport.win_rate ?? sport.winRate ?? "0%",
      races: sport.races ?? 0,
      wins: sport.wins ?? 0,
      podiums: sport.podiums ?? 0,
    },
    timeline: payload.timeline ?? [],
    titleCategories: payload.title_categories ?? payload.titleCategories ?? [],
    categoryPath: payload.category_path ?? payload.categoryPath ?? [],
    identity: {
      origin: identity.origin ?? "Sem origem registrada",
      current: identity.current ?? "Sem categoria atual",
      profile: identity.profile ?? "Perfil em formação",
      summary: identity.summary ?? "Histórico real insuficiente para formar identidade.",
      rival: {
        name: rival.name ?? "Sem rival consolidado",
        currentCategory: rival.current_category ?? rival.currentCategory ?? "Sem categoria atual",
        note: rival.note ?? "Histórico real ainda sem rivalidade consolidada.",
      },
      symbolDriver: identity.symbol_driver ?? identity.symbolDriver ?? "Sem piloto símbolo",
      symbolDriverDetail: identity.symbol_driver_detail ?? identity.symbolDriverDetail ?? "Sem resultados suficientes.",
    },
    management: {
      operationHealth: management.operation_health ?? management.operationHealth ?? "Monitorada",
      peakCash: management.peak_cash ?? management.peakCash ?? "Sem saldo registrado",
      worstCrisis: management.worst_crisis ?? management.worstCrisis ?? "Sem crise registrada",
      healthyYears: management.healthy_years ?? management.healthyYears ?? "Sem temporadas registradas",
      efficiency: management.efficiency ?? "0 pts/R$ mi",
      biggestInvestment: management.biggest_investment ?? management.biggestInvestment ?? "Sem investimento registrado",
      summary: management.summary ?? "Gestão real ainda sem leitura consolidada.",
      peakCashDetail: management.peak_cash_detail ?? management.peakCashDetail ?? "Sem detalhe de saldo registrado.",
      worstCrisisDetail: management.worst_crisis_detail ?? management.worstCrisisDetail ?? "Sem detalhe de crise registrado.",
      healthyYearsDetail: management.healthy_years_detail ?? management.healthyYearsDetail ?? "Sem detalhe de saúde financeira registrado.",
      efficiencyDetail: management.efficiency_detail ?? management.efficiencyDetail ?? "Sem detalhe de eficiência registrado.",
      investmentDetail: management.investment_detail ?? management.investmentDetail ?? "Sem detalhe de investimento registrado.",
    },
  };
}

function resolveTeamFoundedYear(team, rankedTeams, currentPosition, category) {
  const explicitYear = Number(team?.founded_year);
  if (Number.isFinite(explicitYear) && explicitYear > 1800) {
    return explicitYear;
  }

  const knownYear = knownTeamFoundedYear(team?.nome);
  if (knownYear) {
    return knownYear;
  }

  const totalTeams = Math.max(1, rankedTeams.length);
  const rankIndex = clamp((currentPosition || 1) - 1, 0, totalTeams - 1);
  const rankRatio = totalTeams > 1 ? rankIndex / (totalTeams - 1) : 0.5;
  const baseYear = CATEGORY_FOUNDING_BASE_YEARS[category] ?? 2016;

  return Math.round(baseYear + rankRatio * 4);
}

function knownTeamFoundedYear(teamName) {
  const normalizedName = normalizeTeamNameForHistory(teamName);
  const match = KNOWN_TEAM_FOUNDING_YEARS.find(({ names }) =>
    names.some((name) => normalizedName.includes(name)),
  );
  return match?.year ?? null;
}

function normalizeTeamNameForHistory(teamName) {
  return String(teamName ?? "")
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase();
}

function teamHeritageLabel(founded) {
  if (founded <= 1970) {
    return "Equipe histórica";
  }
  return "Projeto consolidado";
}

function emptyRealSport() {
  return {
    seasons: "Carregando histórico real",
    currentStreak: "Carregando histórico real",
    bestStreak: "Carregando histórico real",
    podiumRate: "0%",
    winRate: "0%",
    races: 0,
    wins: 0,
    podiums: 0,
  };
}

function teamWins(team) {
  return team?.vitorias ?? team?.wins ?? Math.max(0, Math.round((team?.pontos ?? 0) / 24));
}

function teamPodiums(team) {
  return team?.podios ?? team?.podiums ?? Math.max(teamWins(team), Math.round((team?.pontos ?? 0) / 8));
}

function teamTitles(team) {
  return team?.titulos ?? team?.titles ?? ((team?.posicao ?? 99) === 1 ? 1 : 0);
}

function teamRaces(team) {
  return Math.max(10, Math.round((team?.pontos ?? 0) / 4) || 10);
}

function rankForMetric(teams, selectedTeam, metric) {
  const ordered = [...(teams ?? [])].sort((a, b) => metric(b) - metric(a));
  const index = ordered.findIndex((entry) => entry.id === selectedTeam?.id);
  return formatOrdinal(index >= 0 ? index + 1 : 1);
}

function findHistoricRival(team, teams) {
  return [...(teams ?? [])]
    .filter((entry) => entry.id !== team?.id)
    .sort((a, b) => Math.abs((a.posicao ?? 99) - (team?.posicao ?? 99)) - Math.abs((b.posicao ?? 99) - (team?.posicao ?? 99)))[0];
}

function managementEfficiency(team) {
  const cashMillions = Math.max(1, Math.abs(team?.cash_balance ?? 0) / 1_000_000);
  return `${((team?.pontos ?? 0) / cashMillions).toFixed(2).replace(".", ",")} pts / R$ mi`;
}

function estimateHistoricDebt(team) {
  if ((team?.cash_balance ?? 0) < 2_000_000) return Math.round(Math.abs(team?.cash_balance ?? 0) * 0.45);
  return 0;
}

function healthySeasonEstimate(team, founded) {
  const total = Math.max(1, 2026 - founded);
  if ((team?.financial_state ?? "stable") === "crisis") return Math.max(1, total - 2);
  if ((team?.financial_state ?? "stable") === "pressured") return Math.max(1, total - 1);
  return total;
}

function buildTitleCategories(categoryName, titles) {
  if (titles <= 0) {
    return [{ category: categoryName, year: "buscando 1º título", color: "#58a6ff" }];
  }
  return Array.from({ length: titles }, (_, index) => ({
    category: categoryName,
    year: String(2026 - index),
    color: ["#58a6ff", "#f2c46d", "#5ee7a8", "#ff6b6b"][index % 4],
  }));
}

function buildCategoryPath(origin, current, founded) {
  if (origin === current) {
    return [
      {
        category: current,
        years: `${founded}-atual`,
        detail: "Trajetória concentrada no mesmo grupo, com evolução interna de estrutura e carro.",
        color: "#58a6ff",
      },
    ];
  }
  return [
    {
      category: origin,
      years: `${founded}-${Math.min(2025, founded + 1)}`,
      detail: "Base inicial da equipe e construção de identidade competitiva.",
      color: "#5ee7a8",
    },
    {
      category: current,
      years: `${Math.min(2026, founded + 2)}-atual`,
      detail: "Fase atual de consolidação, gestão financeira e busca por resultados.",
      color: "#58a6ff",
    },
  ];
}

function originCategoryLabel(category, position) {
  if (category?.includes("toyota")) return "Toyota GR86";
  if (category?.includes("mazda")) return "Mazda MX-5";
  if (category === "bmw_m2") return "Mazda MX-5";
  if (category === "gt4") return position <= 3 ? "BMW M2" : "Toyota GR86";
  if (category === "gt3") return "GT4 Series";
  if (category === "lmp2" || category === "endurance") return "GT3 Championship";
  return "Mazda MX-5";
}

function categoryGroupLabel(category) {
  if (category?.includes("mazda")) return "Grupo Mazda";
  if (category?.includes("toyota")) return "Grupo Toyota";
  if (category === "bmw_m2") return "Grupo BMW";
  if (category === "gt4") return "Grupo GT4";
  if (category === "gt3") return "Grupo GT3";
  if (category === "lmp2") return "Grupo LMP2";
  if (category === "endurance") return "Grupo Endurance";
  return "Grupo da categoria";
}

function shortCategoryLabel(label) {
  return String(label).replace(" Series", "").replace(" Championship", "");
}

function teamHistoryProfile(team, position) {
  if (position <= 2) return "Dominante";
  if ((team?.cash_balance ?? 0) < 2_000_000) return "Sobrevivente Competitiva";
  if ((team?.car_performance ?? 0) >= 7) return "Especialista em Evolução";
  return "Equipe de Meio de Grid";
}

function identitySummary(team, profile, position) {
  if (profile === "Dominante") {
    return `${team?.nome ?? "A equipe"} transformou resultado, caixa e carro forte em referência do grid.`;
  }
  if (profile === "Sobrevivente Competitiva") {
    return `${team?.nome ?? "A equipe"} vive de resiliência: pressão financeira alta, mas ainda capaz de incomodar rivais diretas.`;
  }
  if (profile === "Especialista em Evolução") {
    return `${team?.nome ?? "A equipe"} construiu reputação por desenvolver carro e operação acima do esperado.`;
  }
  return `${team?.nome ?? "A equipe"} ocupa o bloco de disputa constante, perto o bastante para crescer e longe o bastante para precisar escolher bem seus investimentos.`;
}

function managementSummary(team) {
  const state = financialState(team?.financial_state).toLowerCase();
  return `Operação ${state}, com leitura baseada no caixa, dívida e pressão financeira atual.`;
}

function MetricBar({ label, value, rawValue }) {
  const clamped = clamp(Math.round(value), 0, 100);
  return (
    <div>
      <div className="mb-2 flex items-center justify-between text-sm text-text-secondary">
        <span>{label}</span>
        <span className="font-mono text-text-primary">{rawValue}</span>
      </div>
      <div className="h-3 rounded-full bg-white/10">
        <div className="h-3 rounded-full bg-gradient-to-r from-accent-primary to-accent-hover transition-glass" style={{ width: `${Math.max(6, clamped)}%` }} />
      </div>
    </div>
  );
}

function buildDriverRow(role, driver, team, playerId) {
  const isN1 = role === "N1";
  const fallbackName = isN1 ? team?.piloto_1_nome : team?.piloto_2_nome;
  const fallbackSalary = isN1 ? team?.piloto_1_salario_anual : team?.piloto_2_salario_anual;
  const fallbackId = isN1 ? team?.piloto_1_id : team?.piloto_2_id;
  const salary = fallbackSalary ?? driver?.salario_anual ?? estimateSalary(team, driver, isN1 ? 0.58 : 0.42);
  return {
    role,
    name: driver?.nome ?? fallbackName ?? "-",
    nationality: driver?.nacionalidade ?? "",
    nationalityLabel: extractNationalityLabel(driver?.nacionalidade) || driver?.nacionalidade || "Piloto ainda sem dados detalhados",
    salary,
    highlight: driver?.id === playerId || fallbackId === playerId,
  };
}

function estimateSalary(team, driver, share) {
  const ceiling = team?.salary_ceiling ?? 0;
  if (ceiling <= 0) return 0;
  return Math.round(ceiling * share * clamp((driver?.skill ?? 70) / 75, 0.75, 1.25));
}

function technicalMetrics(team, axis) {
  const meta = buildMeta(team?.car_build_profile);
  if (axis === "reliability") {
    return [
      { label: "Confiabilidade", value: team?.confiabilidade ?? 0, rawValue: Math.round(team?.confiabilidade ?? 0) },
      { label: "Pressao financeira", value: 100 - clamp(team?.budget_index ?? team?.budget ?? 0, 0, 100), rawValue: financialState(team?.financial_state) },
      { label: "Risco operacional", value: team?.pit_strategy_risk ?? 0, rawValue: pitRisk(team?.pit_strategy_risk ?? 0) },
    ];
  }
  if (axis === "pit") {
    return [
      { label: "Qualidade do pit crew", value: team?.pit_crew_quality ?? 0, rawValue: pitCrew(team?.pit_crew_quality ?? 0) },
      { label: "Risco de pit strategy", value: team?.pit_strategy_risk ?? 0, rawValue: pitRisk(team?.pit_strategy_risk ?? 0) },
      { label: "Consistencia geral", value: ((team?.pit_crew_quality ?? 0) + (team?.confiabilidade ?? 0)) / 2, rawValue: "Pit + confiabilidade" },
    ];
  }
  return [
    { label: "Pacote do carro", value: normalizeCar(team?.car_performance ?? 0), rawValue: `Nível ${carLevel(team?.car_performance)}/10` },
    { label: "Foco do projeto", value: profileFocusScore(meta.weights), rawValue: meta.label },
    { label: "Equilíbrio do acerto", value: profileBalanceScore(meta.weights), rawValue: profileBalanceLabel(meta.weights) },
  ];
}

function incomeRows(team) {
  return splitAmount(Math.max(0, team?.last_round_income ?? 0), [
    ["Patrocínios", 0.5],
    ["Bônus de resultado", 0.24],
    ["Prêmio parcial", 0.18],
    ["Auxílios", 0.08],
  ]);
}

function expenseRows(team) {
  return splitAmount(Math.max(0, team?.last_round_expenses ?? 0), [
    ["Salários", 0.42],
    ["Operação do evento", 0.24],
    ["Manutenção estrutural", 0.2],
    ["Investimento técnico", 0.14],
  ]);
}

function splitAmount(total, parts) {
  return parts.map(([label, share], index) => {
    const previous = parts.slice(0, index).reduce((sum, [, partShare]) => sum + Math.round(total * partShare), 0);
    return { label, value: index === parts.length - 1 ? Math.max(0, total - previous) : Math.round(total * share) };
  });
}

function cashTimeline(team) {
  const cash = team?.cash_balance ?? 0;
  const net = team?.last_round_net ?? 0;
  const points = Array.from({ length: 10 }, (_, index) => ({
    label: `R${index + 1}`,
    value: cash - net * (9 - index) + ((index % 3) - 1) * Math.abs(net) * 0.22,
  }));
  const values = points.map((point) => point.value);
  const min = Math.min(...values);
  const span = Math.max(1, Math.max(...values) - min);
  return points.map((point) => ({ ...point, height: 22 + ((point.value - min) / span) * 72 }));
}

function buildMeta(profile) {
  return BUILD_META[profile] ?? BUILD_META.balanced;
}

function normalizeCar(value) {
  return clamp(((value + 5) / 21) * 100, 0, 100);
}

function carLevel(value) {
  return clamp(Math.round(value ?? 0), 1, 10);
}

function formatOrdinal(value) {
  return Number.isFinite(value) ? `${value}º` : "-";
}

function profileBalanceScore(weights) {
  const spread = Math.max(...weights) - Math.min(...weights);
  return clamp(100 - spread, 0, 100);
}

function profileFocusScore(weights) {
  return Math.max(...weights);
}

function profileBalanceLabel(weights) {
  const spread = Math.max(...weights) - Math.min(...weights);
  if (spread <= 5) return "Acerto equilibrado";
  if (spread <= 20) return "Leve especialização";
  return "Especialização forte";
}

function moneyTone(value) {
  return value < 0 ? "text-status-red" : "text-text-primary";
}

function operationalRunway(cash, net) {
  if (net >= 0) {
    return {
      value: "Estável",
      caption: "Rodada positiva preserva o caixa",
      tone: "text-status-green",
    };
  }

  const rounds = Math.max(0, Math.floor(cash / Math.abs(net)));
  return {
    value: `${rounds} rodadas`,
    caption: "Estimativa no ritmo atual",
    tone: rounds >= 5 ? "text-text-primary" : "text-status-red",
  };
}

function financialState(state) {
  return {
    elite: "Elite financeira",
    healthy: "Saudável",
    stable: "Estável",
    pressured: "Pressionada",
    crisis: "Em crise",
    collapse: "Colapso",
  }[state] ?? "Estável";
}

function operationHealthTone(label) {
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

function financialStateTone(state) {
  if (state === "elite" || state === "healthy") {
    return "border-status-green/25 bg-status-green/10 text-status-green";
  }
  if (state === "pressured" || state === "crisis" || state === "collapse") {
    return "border-status-red/25 bg-status-red/10 text-status-red";
  }
  return "border-status-yellow/25 bg-status-yellow/10 text-status-yellow";
}

function seasonStrategy(strategy) {
  return {
    expansion: "Expansão",
    balanced: "Equilíbrio",
    austerity: "Austeridade",
    all_in: "All-in",
    survival: "Sobrevivência",
  }[strategy] ?? "Equilíbrio";
}

function pitRisk(value) {
  if (value <= 20) return "Ultra conservador";
  if (value <= 40) return "Conservador";
  if (value <= 55) return "Equilibrado";
  if (value <= 75) return "Agressivo";
  return "Oportunista";
}

function pitCrew(value) {
  if (value <= 20) return "Muito fraco";
  if (value <= 40) return "Fraco";
  if (value <= 60) return "Ok";
  if (value <= 80) return "Forte";
  return "Elite";
}

function formatMoney(value) {
  return new Intl.NumberFormat("pt-BR", {
    style: "currency",
    currency: "BRL",
    maximumFractionDigits: 0,
  }).format(Math.round(value ?? 0));
}

function formatSignedMoney(value) {
  return `${value >= 0 ? "+" : ""}${formatMoney(value)}`;
}

function formatPercent(value) {
  return `${Math.round(value ?? 0)}%`;
}

function clamp(value, min, max) {
  return Math.max(min, Math.min(max, value));
}

export default MyTeamTab;
