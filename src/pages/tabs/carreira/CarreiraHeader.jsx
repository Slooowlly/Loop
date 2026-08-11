import { useTranslation } from "react-i18next";
import { Award, HeartPulse, Sparkles } from "lucide-react";

import FlagIcon from "../../../components/ui/FlagIcon";
import Tooltip from "../../../components/ui/Tooltip";
import TeamLogoMark from "../../../components/team/TeamLogoMark";
import { formatContractRole } from "../../../components/driver/detalhes/formatadores.js";
import { getVividTeamColor } from "../../../utils/teamColors";

// Cabeçalho da aba Carreira: quem o jogador é, agora.
//
// Fica FORA das pílulas de seção, sempre visível. É o que separa esta tela da ficha
// de um piloto qualquer: o `DriverDetailModal` abre por cima de outra tela e some
// quando o jogador clica fora, então o retrato dele é episódico. Aqui o retrato é o
// chão da tela — trocar de seção não muda quem está sendo olhado.
//
// A hierarquia é a mesma da ficha v2, de propósito: do permanente (nome, título,
// licença) ao volátil (momento, motivação, lesão). Repetir a ordem que o jogador já
// aprendeu vale mais que inventar uma leitura nova para o mesmo material.
const TONS_DE_MOMENTO = {
  forte: "#3fb950",
  estavel: "#d29922",
  em_baixa: "#f85149",
  sem_dados: "#8b949e",
};

function CarreiraHeader({ detail }) {
  const { t } = useTranslation();
  const perfil = detail.perfil ?? {};
  const competitivo = detail.competitivo ?? {};
  const resumo = detail.resumo_atual ?? {};
  const lesao = detail.saude?.lesao_ativa ?? null;
  const titulos = detail.trajetoria?.titulos ?? 0;
  const equipe = perfil.equipe_nome || detail.equipe_nome || "";
  const papel = formatContractRole(detail.papel);
  const corDaEquipe = getVividTeamColor(
    detail.equipe_cor_primaria || perfil.equipe_cor_primaria || "",
  );
  const momento = detail.forma?.momento ?? "sem_dados";
  const corDoMomento = TONS_DE_MOMENTO[momento] ?? TONS_DE_MOMENTO.sem_dados;
  // Só de "Conhecido" (>30) para cima, pela mesma regra da ficha: carimbar
  // "Anônimo" na testa do estreante gasta um chip para dizer que não há o que dizer.
  const nivelDeFama = (detail.estrelato?.fama ?? 0) > 30 ? detail.estrelato?.nivel_fama : null;
  // A posição no campeonato só vale para quem já largou nesta temporada. Antes da
  // primeira corrida o grid está todo com zero ponto e a ordem é desempate — o
  // backend cala os gaps pelo mesmo motivo, e a tela não pode ser mais afirmativa
  // que o dado.
  const correu = (detail.stats_temporada?.corridas ?? 0) > 0;
  const posicao = correu ? resumo.posicao_campeonato : null;

  return (
    <header
      data-testid="carreira-header"
      className="relative overflow-hidden rounded-2xl border border-white/10 px-5 py-4"
      style={{
        "--team": corDaEquipe,
        backgroundImage:
          "linear-gradient(104deg, color-mix(in srgb, var(--team) 16%, transparent), transparent 62%), linear-gradient(180deg, rgba(12,22,38,0.9), rgba(5,11,20,0.94))",
      }}
    >
      <div className="flex flex-wrap items-start justify-between gap-x-6 gap-y-4">
        <div className="flex min-w-0 items-start gap-4">
          {/* Placa 3:2 preenchida pela arte, a mesma caixa da ficha v2 — as
              proporções oficiais vão de 1:1 a 2:1, e sem a placa a mesma altura
              devolveria larguras diferentes a cada nacionalidade. */}
          <span className="flex h-7 w-[42px] shrink-0 items-center justify-center overflow-hidden rounded-md bg-white/[0.06] shadow-[0_0_0_1px_rgba(255,255,255,0.10)]">
            <FlagIcon
              variant="natural"
              nacionalidade={
                perfil.bandeira && perfil.nacionalidade
                  ? `${perfil.bandeira} ${perfil.nacionalidade}`
                  : perfil.nacionalidade || detail.nacionalidade || ""
              }
              className="h-full w-full object-cover text-base"
            />
          </span>

          <div className="min-w-0">
            <div className="flex min-w-0 flex-wrap items-baseline gap-x-3 gap-y-1">
              <h2 className="min-w-0 truncate text-2xl font-semibold leading-none tracking-[-0.03em] text-text-primary">
                {detail.nome}
              </h2>
              <span className="shrink-0 font-mono text-base font-semibold text-text-secondary">
                {t("driverDetail.profile.age", { count: perfil.idade ?? detail.idade })}
              </span>
              {titulos > 0 ? (
                <span
                  data-testid="carreira-header-titulos"
                  className="flex shrink-0 items-center gap-1.5 self-center rounded-full border border-[#f0b23244] bg-[#f0b2321f] px-2.5 py-1 text-xs font-semibold text-[#f0b232]"
                >
                  <Award size={13} strokeWidth={2} aria-hidden="true" />
                  {t("driverDetail.v2.titleBadge", { count: titulos })}
                </span>
              ) : null}
            </div>

            <div className="mt-2 flex flex-wrap items-center gap-1.5">
              {perfil.licenca?.nivel ? <Chip>{perfil.licenca.nivel}</Chip> : null}
              {nivelDeFama ? (
                <Tooltip texto={t("driverDetail.stardom.fame")}>
                  <span className="flex items-center gap-1.5 rounded-full border border-white/15 bg-[#08111f] px-2.5 py-1 text-xs text-text-secondary">
                    <Sparkles size={12} strokeWidth={2} aria-hidden="true" />
                    {nivelDeFama}
                  </span>
                </Tooltip>
              ) : null}
              {lesao ? (
                <span
                  data-testid="carreira-header-lesao"
                  className="flex items-center gap-1.5 rounded-full border border-status-red/40 bg-status-red/15 px-2.5 py-1 text-xs font-semibold text-status-red"
                >
                  <HeartPulse size={12} strokeWidth={2} aria-hidden="true" />
                  {t("carreiraTab.header.injuredFor", { count: lesao.corridas_restantes })}
                </span>
              ) : null}
            </div>
          </div>
        </div>

        {equipe ? (
          <div className="flex min-w-0 items-center gap-3 self-center">
            <TeamLogoMark teamName={equipe} size="md" halo />
            <div className="min-w-0">
              <div className="truncate text-[22px] font-semibold leading-none tracking-[-0.02em] text-[color:var(--team)]">
                {equipe}
              </div>
              {papel !== "-" ? (
                <div className="mt-1.5 truncate text-[10px] font-bold uppercase tracking-[0.2em] text-text-secondary">
                  {papel}
                </div>
              ) : null}
            </div>
          </div>
        ) : (
          <Chip>{t("driverDetail.profile.noTeam")}</Chip>
        )}

        <div className="flex shrink-0 items-center gap-5 self-center">
          {posicao ? (
            <div className="text-right">
              <div
                className="font-mono text-[34px] font-semibold leading-none tracking-[-0.04em] text-text-primary"
                style={{ fontVariantNumeric: "tabular-nums" }}
              >
                P{posicao}
              </div>
              <div className="mt-1.5 text-[10px] font-semibold uppercase tracking-[0.16em] text-text-muted">
                {t("carreiraTab.header.championship")}
              </div>
            </div>
          ) : null}

          <div className="min-w-[120px]">
            <strong
              className="flex items-center gap-1.5 whitespace-nowrap text-sm font-semibold leading-none"
              style={{ color: corDoMomento }}
            >
              <span
                className="h-1.5 w-1.5 shrink-0 rounded-full"
                style={{ backgroundColor: corDoMomento }}
              />
              {t(`driverDetail.momentBuilder.${momento}`)}
            </strong>
            <div className="mt-2.5">
              <div className="mb-1 flex items-baseline justify-between gap-2">
                <span className="text-[10px] font-semibold uppercase tracking-[0.16em] text-text-muted">
                  {t("carreiraTab.header.motivation")}
                </span>
                <span className="font-mono text-[11px] text-text-secondary">
                  {Math.round(competitivo.motivacao ?? 0)}
                </span>
              </div>
              <div className="h-1.5 overflow-hidden rounded-full bg-white/10">
                <div
                  className="h-full rounded-full bg-[color:var(--team)]"
                  style={{
                    width: `${Math.max(0, Math.min(100, competitivo.motivacao ?? 0))}%`,
                  }}
                />
              </div>
            </div>
          </div>
        </div>
      </div>
    </header>
  );
}

function Chip({ children }) {
  return (
    <span className="rounded-full border border-white/15 bg-[#08111f] px-2.5 py-1 text-xs text-text-secondary">
      {children}
    </span>
  );
}

export default CarreiraHeader;
