// i18n-ignore-file — a CHROME desta tela já é traduzida (ns seasonChampion), mas o objeto
// DEMO (dados de exemplo, PT) fica até o pipeline real por categoria existir. Sem isto o
// scanner acusaria os dados-de-exemplo. Ao ligar os dados reais, remova esta linha.
import { useCallback, useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import useCareerStore from "../../stores/useCareerStore";
import "./SeasonChampionOverlay.css";

// Pop-up "Campeão da Temporada": celebra o campeão da categoria do jogador ao
// fim da temporada, sobre a home. Montado como host global no App, aparece
// sempre que o store tiver `championOverlay`. Guiado por dados; hoje o botão de
// debug injeta DADOS DE EXEMPLO (o pipeline real de recordes/gráfico — SDK por
// categoria + pontos por etapa — ainda será plugado). Ver useCareerStore.

const YOU_NAME = "R. Silva";

// ─── Dados de exemplo (debug) ────────────────────────────────────────────────
// Formas de pontuação por etapa, reaproveitadas p/ montar o gráfico acumulado.
const SHAPES = [
  [25, 20, 25, 18, 25, 15, 25, 22, 10, 25, 20, 25, 18, 25, 22, 25, 20, 24],
  [20, 25, 18, 25, 15, 25, 20, 18, 25, 20, 25, 15, 25, 18, 25, 20, 22, 25],
  [18, 15, 20, 22, 18, 20, 15, 25, 18, 15, 20, 22, 18, 20, 25, 18, 20, 18],
  [15, 18, 15, 0, 20, 15, 18, 0, 20, 22, 0, 18, 20, 15, 18, 20, 15, 18],
];
const cumulative = (arr) => {
  let s = 0;
  return arr.map((v) => (s += v));
};
const CUMM = SHAPES.map(cumulative);
const TOTALS = CUMM.map((c) => c[c.length - 1]);

const DEMO = {
  year: 2026,
  category: "GT3 Championship",
  drivers: [
    { name: "R. Silva", team: "Meridian GT", flag: "🇧🇷", num: "1", champ: true, you: true },
    { name: "Yuki Tanaka", team: "Kaido Works", flag: "🇯🇵", num: "7" },
    { name: "Felipe Duarte", team: "Verona Corse", flag: "🇧🇷", num: "9" },
    { name: "Marco Bianchi", team: "Scuderia Lume", flag: "🇮🇹", num: "24" },
  ],
  reward: [
    { ic: "💰", t: "Prêmio de campeão", v: "R$ 1.850.000" },
    { ic: "⭐", t: "Fama conquistada", v: '+240 <small>subiu p/ "Estrela"</small>' },
    { ic: "🤝", t: "Reputação da equipe", v: "+1 nível <small>Meridian GT</small>" },
    { ic: "🏆", t: "Sala de troféus", v: "Taça GT3 2026" },
  ],
  awards: [
    { ic: "🏆", t: "Grand Chelem", who: "R. Silva <em>(você)</em>", d: "Em Spa: pole, vitória, volta mais rápida e liderou de ponta a ponta." },
    { ic: "⚔️", t: "Duelo da temporada", who: "Silva × Tanaka", d: "18 trocas de posição entre si ao longo do ano." },
    { ic: "🌟", t: "Revelação do ano", who: "S. Lindqvist", d: "Estreante: 2 pódios e 5º lugar no campeonato." },
    { ic: "🎬", t: "Virada da temporada", who: "Etapa 9 · Nürburgring", d: "O abandono de Tanaka abriu o título na reta final." },
  ],
  records: [
    { e: "🔥", l: "Mais voltas rápidas", who: "R. Silva", v: "9" },
    { e: "🏁", l: "Mais pole positions", who: "R. Silva", v: "11" },
    { e: "🏹", l: "Mais ultrapassagens", who: "Yuki Tanaka", v: "142" },
    { e: "👑", l: "Mais voltas na liderança", who: "Yuki Tanaka", v: "198" },
    { e: "🥇", l: "Mais pódios", who: "Yuki Tanaka", v: "16" },
    { e: "🚀", l: "Maior velocidade de ponta", who: "Marco Bianchi", v: "302<small>km/h</small>" },
    { e: "📈", l: "Maior recuperação", who: "Felipe Duarte", v: "+13<small>Monza</small>" },
    { e: "🕳️", l: "Mais ultrapassado", who: "Marco Bianchi", v: "88" },
    { e: "🌧️", l: "Rei da chuva", who: "R. Silva", v: "3<small>vit.</small>" },
    { e: "⏱️", l: "Pit médio mais rápido", who: "Kaido Works", v: "24.6<small>s</small>" },
    { e: "♟️", l: "Estrategista", who: "Felipe Duarte", v: "+11<small>no pit</small>" },
    { e: "🏳️", l: "Mais abandonos", who: "Marco Bianchi", v: "4<small>DNF</small>" },
  ],
};

const ROUNDS = SHAPES[0].length;
const CHART = { W: 820, H: 300, padL: 42, padR: 124, padT: 14, padB: 30 };

function lineColor(d, rank) {
  if (d.champ) return "#F4C752";
  if (d.you) return "#12E0E0";
  return rank === 1 ? "#9aa6b4" : rank === 2 ? "#c8895a" : "#5b6b7d";
}
function medal(rank) {
  return rank === 0 ? "co-b1" : rank === 1 ? "co-b2" : "co-b3";
}
function initials(n) {
  return n.replace(/\./g, "").split(" ").map((w) => w[0]).join("").slice(0, 2).toUpperCase();
}

function PointsChart({ drivers }) {
  const { t } = useTranslation();
  const { W, H, padL, padR, padT, padB } = CHART;
  const ymax = Math.ceil(Math.max(...TOTALS) / 50) * 50;
  const X = (i) => padL + (i / (ROUNDS - 1)) * (W - padL - padR);
  const Y = (v) => padT + (1 - v / ymax) * (H - padT - padB);

  const rows = drivers.map((d, i) => ({ ...d, cumm: CUMM[i], total: TOTALS[i], rank: i }));
  // desenha campeão/jogador por cima
  const order = [...rows].sort((a, b) => (a.champ || a.you ? 1 : 0) - (b.champ || b.you ? 1 : 0));

  const gridVals = [0, 1, 2, 3, 4].map((g) => (ymax * g) / 4);
  const xTicks = [1, 5, 10, 15, 18];

  return (
    <svg className="co-chart" viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="xMidYMid meet" role="img" aria-label={t("seasonChampion.chart.ariaLabel")}>
      {gridVals.map((val) => {
        const y = Y(val);
        return (
          <g key={`g${val}`}>
            <line x1={padL} y1={y} x2={W - padR} y2={y} stroke="rgba(255,255,255,0.07)" strokeWidth="1" />
            <text x={padL - 8} y={y + 3.5} fill="#6f7c8b" fontSize="11" textAnchor="end" fontFamily="inherit">{Math.round(val)}</text>
          </g>
        );
      })}
      {xTicks.map((r) => (
        <text key={`x${r}`} x={X(r - 1)} y={H - 10} fill="#6f7c8b" fontSize="11" textAnchor="middle" fontFamily="inherit">{t("seasonChampion.chart.stageTick", { n: r })}</text>
      ))}
      {order.map((d) => {
        const col = lineColor(d, d.rank);
        const hl = d.champ || d.you;
        const pts = d.cumm.map((v, i) => `${X(i).toFixed(1)},${Y(v).toFixed(1)}`).join(" ");
        const lx = X(ROUNDS - 1);
        const ly = Y(d.total);
        return (
          <g key={d.name}>
            <polyline points={pts} fill="none" stroke={col} strokeWidth={hl ? 3.4 : 2} strokeLinejoin="round" strokeLinecap="round" opacity={hl ? 1 : 0.8} />
            <circle cx={lx} cy={ly} r={hl ? 4.5 : 3.3} fill={col} />
            <text x={lx + 8} y={ly + 4} fill={col} fontSize={hl ? 13 : 12} fontWeight={hl ? 700 : 500} fontFamily="inherit">
              {`${d.name.split(" ").slice(-1)[0]} ${d.total}`}
            </text>
          </g>
        );
      })}
    </svg>
  );
}

function SeasonChampionOverlay() {
  const { t } = useTranslation();
  const championOverlay = useCareerStore((s) => s.championOverlay);
  const hideChampionOverlay = useCareerStore((s) => s.hideChampionOverlay);
  const closeOverlay = useCallback(() => {
    hideChampionOverlay();
  }, [hideChampionOverlay]);

  useEffect(() => {
    if (!championOverlay) return undefined;
    const onKey = (e) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopImmediatePropagation();
      closeOverlay();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [championOverlay, closeOverlay]);

  // Hoje o overlay usa sempre os dados de exemplo; quando o pipeline real existir,
  // basta `championOverlay` trazer os campos e mesclamos aqui.
  const data = useMemo(() => (championOverlay && typeof championOverlay === "object" && championOverlay.drivers ? championOverlay : DEMO), [championOverlay]);

  if (!championOverlay) return null;

  const drivers = data.drivers;
  const champ = drivers.find((d) => d.champ) || drivers[0];
  const top3 = drivers.map((d, i) => ({ ...d, rank: i, total: TOTALS[i] })).slice(0, 3);
  const podiumVisual = [top3[1], top3[0], top3[2]]; // 2 - 1 - 3
  const discBg = ["var(--gold)", "#d7dee8", "#c8895a"];
  const champTotal = TOTALS[0];
  const margin = TOTALS[0] - TOTALS[1];

  return (
    <div className="champ-ov" onClick={closeOverlay}>
      <div className="co-modal" role="dialog" aria-label={t("seasonChampion.dialogLabel")} onClick={(e) => e.stopPropagation()}>
        <div className="co-head">
          <div className="co-lhs">
            <span className="co-pulse"><span className="a" /><span className="b" /></span>
            <div>
              <h3>{t("seasonChampion.head.title", { year: data.year })}</h3>
              <div className="sub">{champ.you ? t("seasonChampion.head.subtitleYou", { series: data.category.split(" ")[0] }) : t("seasonChampion.head.subtitleOther", { series: data.category.split(" ")[0] })}</div>
            </div>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
            <span className="co-catpill">{data.category}</span>
            <button className="co-x" type="button" onClick={closeOverlay}>{t("seasonChampion.close")}</button>
          </div>
        </div>

        <div className="co-body">
          <div className="co-hero">
            <div className="co-champ">
              <div className="co-bigpts"><div className="v">{champTotal}</div><div className="l">{t("seasonChampion.hero.pointsLabel")}</div></div>
              <div className="co-kicker">{t("seasonChampion.hero.kicker")}</div>
              <div className="co-laurel">🏆</div>
              <div className="co-cname">{champ.name}</div>
              <div className="co-meta">
                <span className="co-flag">{champ.flag}</span>
                <span className="co-num">#{champ.num}</span>
                <span className="co-team">{t("seasonChampion.hero.forTeam")} <b>{champ.team}</b></span>
                {champ.you && <span className="co-youbadge">{t("seasonChampion.hero.youExclaim")}</span>}
              </div>
              <div className="co-clinch">✦ {t("seasonChampion.hero.clinchPre")} <b>{t("seasonChampion.hero.clinchPoints", { count: margin })}</b> {t("seasonChampion.hero.clinchPost")}</div>
            </div>

            <div className="co-reward">
              <h4>🎁 {t("seasonChampion.reward.title")}</h4>
              {data.reward.map((r) => (
                <div className="co-rrow" key={r.t}>
                  <span className="ic">{r.ic}</span>
                  <div className="tx"><div className="t">{r.t}</div><div className="v" dangerouslySetInnerHTML={{ __html: r.v }} /></div>
                </div>
              ))}
            </div>
          </div>

          <div className="co-card">
            <h4>{t("seasonChampion.chart.title")} <span className="hint">{t("seasonChampion.chart.hint", { rounds: ROUNDS })}</span></h4>
            <div className="co-chartwrap"><PointsChart drivers={drivers} /></div>
            <div className="co-legend">
              {drivers.map((d, i) => {
                const tag = d.you ? ` ${t("seasonChampion.youParen")}` : d.champ ? " 🏆" : "";
                return (
                  <span key={d.name}><i style={{ background: lineColor({ ...d, rank: i }, i) }} />{d.name}{tag} · {TOTALS[i]} pts</span>
                );
              })}
            </div>
          </div>

          <div className="co-lower">
            <div className="co-card">
              <h4>{t("seasonChampion.podium.title")}</h4>
              <div className="co-podium">
                {podiumVisual.map((d) => (
                  <div className={`co-step${d.you ? " you" : ""}`} key={d.name}>
                    <div className="co-disc" style={{ background: discBg[d.rank] }}>{initials(d.name)}</div>
                    <div className="co-who">{d.name}{d.you ? ` ${t("seasonChampion.youParen")}` : ""}<small>{d.team} · {d.total} pts</small></div>
                    <div className={`co-bar ${medal(d.rank)}`}><span className="co-rankn">{d.rank + 1}</span></div>
                  </div>
                ))}
              </div>
            </div>

            <div className="co-card">
              <h4>{t("seasonChampion.awards.title")} <span className="hint">{t("seasonChampion.awards.hint")}</span></h4>
              <div className="co-awards">
                {data.awards.map((a) => (
                  <div className="co-award" key={a.t}>
                    <span className="ic">{a.ic}</span>
                    <div>
                      <div className="t">{a.t}</div>
                      <div className="who" dangerouslySetInnerHTML={{ __html: a.who }} />
                      <div className="d">{a.d}</div>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </div>

          <div className="co-card">
            <h4>{t("seasonChampion.records.title")} <span className="hint">{t("seasonChampion.records.hint")}</span></h4>
            <div className="co-supers">
              {data.records.map((r) => {
                const isYou = r.who === YOU_NAME;
                return (
                  <div className={`co-super${isYou ? " you" : ""}`} key={r.l}>
                    <span className="ic">{r.e}</span>
                    <div className="txt">
                      <div className="lab">{r.l}</div>
                      <div className="nm">{r.who}{isYou && <em> {t("seasonChampion.youParen")}</em>}</div>
                    </div>
                    <div className="val" dangerouslySetInnerHTML={{ __html: r.v }} />
                  </div>
                );
              })}
            </div>
          </div>
        </div>

        <div className="co-foot">
          <span className="note">{t("seasonChampion.footer.note")}</span>
          <div className="co-btns">
            <button className="co-btn primary" type="button" onClick={closeOverlay}>{t("seasonChampion.footer.continue")} ▶</button>
          </div>
        </div>
      </div>
    </div>
  );
}

export default SeasonChampionOverlay;
