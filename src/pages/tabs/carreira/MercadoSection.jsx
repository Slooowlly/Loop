import { useTranslation } from "react-i18next";
import { Eye, TriangleAlert } from "lucide-react";

import {
  formatCategoryLabel,
  formatContractPeriod,
  formatContractRole,
} from "../../../components/driver/detalhes/formatadores.js";
import { formatMoneyCompact, formatSalaryAnnual } from "../../../utils/formatters";
import { getVividTeamColor } from "../../../utils/teamColors";
import { Bloco, Numero, Vazio } from "./primitivos.jsx";

// F-01 — o mercado FORA da janela de pré-temporada.
//
// A leitura errada que gerou este item dizia que "o mercado não tem tela". Tem: são
// ~2700 linhas em `PreSeasonView` + `season/preseason/` + `PoachAuctionHost`. O
// buraco real é de CONTINUIDADE — durante a temporada o mercado só chega em eventos
// pontuais (assédio, oferta especial, interesse) que exigem resposta na hora e
// desaparecem. Não havia onde consultar, numa terça-feira qualquer: quanto resta do
// meu contrato, quem está de olho em mim, que vaga abriu.
//
// As três respostas, e de onde saem:
//
//   • contrato e valor — `get_driver_detail(jogador).contrato_mercado`, o MESMO
//     payload que as outras seções desta aba já carregam.
//   • quem está de olho — `get_inbox_messages().team_interest`, o interesse ATIVO
//     pela fama. É o mesmo fato que a caixa de entrada mostra na Home; aqui ele
//     deixa de ser uma mensagem que passa e vira um estado que fica.
//   • vagas — `get_season_market_board`, o único dado novo que este item precisou.
//
// O que esta tela deliberadamente NÃO faz: conduzir a janela de transferências. O
// comando `advance_transfer_window` está registrado e o backlog pedia para ligá-lo,
// mas ele é um no-op documentado no próprio Rust — "apenas devolve o estado atual
// das ofertas, sem avançar nada". Quem avança o mercado de verdade é
// `advance_market_week`, que a pré-temporada já conduz. Um botão aqui seria um botão
// que não faz nada.
function MercadoSection({ detail, board, teamInterest }) {
  const { t } = useTranslation();
  const contrato = detail.contrato_mercado?.contrato ?? null;
  const mercado = detail.contrato_mercado?.mercado ?? null;
  const vagas = Array.isArray(board?.vagas) ? board.vagas : [];
  const salarioAtual = contrato?.salario_anual ?? null;
  const salarioEstimado = mercado?.salario_estimado ?? null;
  // A distância entre o que ele ganha e o que o mercado pagaria é o que diz se ele
  // está barato ou caro — nenhum dos dois números responde isso sozinho.
  const premio =
    Number.isFinite(salarioAtual) && Number.isFinite(salarioEstimado) && salarioAtual > 0
      ? salarioEstimado - salarioAtual
      : null;
  // Último ano de contrato é o único estado do contrato que muda o que o jogador
  // faz nesta semana: é quando o assédio compensa e quando renovar deixa de ser
  // automático.
  const ultimoAno = contrato ? (contrato.anos_restantes ?? 0) <= 1 : false;

  return (
    <div className="space-y-4">
      <div className="grid gap-4 lg:grid-cols-2">
        <Bloco titulo={t("carreiraTab.mercado.contract")} testId="carreira-mercado-contrato">
          {contrato ? (
            <>
              <dl className="grid gap-x-6 gap-y-1 sm:grid-cols-2">
                <Linha rotulo={t("driverDetail.moment.team")} valor={contrato.equipe_nome} />
                <Linha
                  rotulo={t("driverDetail.market.role")}
                  valor={formatContractRole(contrato.papel)}
                />
                <Linha
                  rotulo={t("driverDetail.moment.salary")}
                  valor={formatSalaryAnnual(contrato.salario_anual)}
                />
                <Linha
                  rotulo={t("driverDetail.moment.term")}
                  valor={formatContractPeriod(contrato)}
                />
              </dl>

              <div
                className={`mt-3.5 flex items-center gap-2.5 rounded-xl border px-3.5 py-2.5 ${
                  ultimoAno
                    ? "border-status-yellow/35 bg-status-yellow/10"
                    : "border-white/8 bg-black/10"
                }`}
                data-testid="carreira-mercado-prazo"
              >
                {ultimoAno ? (
                  <TriangleAlert
                    size={16}
                    strokeWidth={1.8}
                    aria-hidden="true"
                    className="shrink-0 text-status-yellow"
                  />
                ) : null}
                <span
                  className={`text-sm ${ultimoAno ? "text-status-yellow" : "text-text-secondary"}`}
                >
                  {ultimoAno
                    ? t("carreiraTab.mercado.lastYear")
                    : t("carreiraTab.mercado.yearsLeft", { count: contrato.anos_restantes ?? 0 })}
                </span>
              </div>
            </>
          ) : (
            <Vazio>{t("carreiraTab.mercado.freeAgent")}</Vazio>
          )}
        </Bloco>

        <Bloco titulo={t("carreiraTab.mercado.value")} testId="carreira-mercado-valor">
          {mercado ? (
            <>
              <div className="grid grid-cols-2 gap-4 sm:grid-cols-3">
                <Numero
                  valor={
                    Number.isFinite(mercado.valor_mercado)
                      ? formatMoneyCompact(mercado.valor_mercado)
                      : "-"
                  }
                  rotulo={t("carreiraTab.mercado.marketValue")}
                  nota={
                    // O ordinal é a régua que o valor absoluto não tem: "$23.016"
                    // não diz se é muito sem saber quanto valem os outros carros
                    // do grid. Vem do backend junto com o denominador.
                    mercado.posicao_valor && mercado.total_valor
                      ? t("carreiraTab.mercado.valueRank", {
                          position: mercado.posicao_valor,
                          total: mercado.total_valor,
                        })
                      : null
                  }
                />
                <Numero
                  valor={
                    Number.isFinite(salarioEstimado) ? formatMoneyCompact(salarioEstimado) : "-"
                  }
                  rotulo={t("carreiraTab.mercado.estimatedSalary")}
                  nota={
                    Number.isFinite(premio)
                      ? premio >= 0
                        ? t("carreiraTab.mercado.underpaid", {
                            value: formatMoneyCompact(premio),
                          })
                        : t("carreiraTab.mercado.overpaid", {
                            value: formatMoneyCompact(Math.abs(premio)),
                          })
                      : null
                  }
                />
                {Number.isFinite(mercado.chance_transferencia) ? (
                  <Numero
                    valor={`${mercado.chance_transferencia}%`}
                    rotulo={t("carreiraTab.mercado.transferChance")}
                  />
                ) : null}
              </div>
            </>
          ) : (
            <Vazio>{t("carreiraTab.mercado.noValue")}</Vazio>
          )}
        </Bloco>
      </div>

      <Bloco titulo={t("carreiraTab.mercado.watching")} testId="carreira-mercado-interesse">
        {teamInterest?.teams?.length ? (
          <>
            <p className="text-sm leading-relaxed text-text-secondary">
              {t("carreiraTab.mercado.watchingIntro", { count: teamInterest.teams.length })}
            </p>
            <ul className="mt-3 grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
              {teamInterest.teams.map((equipe, indice) => (
                <li
                  key={`${equipe.team_name}-${indice}`}
                  className="flex items-center gap-2.5 rounded-lg border border-white/6 bg-black/10 px-3 py-2"
                >
                  <Eye
                    size={14}
                    strokeWidth={1.8}
                    aria-hidden="true"
                    className="shrink-0 text-accent-primary"
                  />
                  <span className="min-w-0 flex-1 truncate text-sm text-text-primary">
                    {equipe.team_name}
                  </span>
                  <span className="shrink-0 text-[10px] uppercase tracking-[0.12em] text-text-muted">
                    {formatCategoryLabel(equipe.category)}
                  </span>
                </li>
              ))}
            </ul>
          </>
        ) : (
          // Interesse ativo nasce da FAMA, então "ninguém de olho" não é bug: é o
          // estado de quem ainda não tem apelo comercial. A frase diz isso, em vez
          // de deixar a caixa vazia sugerindo dado que não carregou.
          <Vazio>{t("carreiraTab.mercado.noWatchers")}</Vazio>
        )}
      </Bloco>

      <Bloco
        titulo={t("carreiraTab.mercado.openSeats")}
        testId="carreira-mercado-vagas"
        acao={
          board ? (
            <span className="text-[11px] text-text-secondary">
              {t("carreiraTab.mercado.seatsCount", {
                eligible: board.vagas_elegiveis ?? 0,
                total: vagas.length,
              })}
            </span>
          ) : null
        }
      >
        {vagas.length ? (
          <ul className="grid gap-2">
            {vagas.map((vaga) => {
              const elegivel = vaga.licenca_ok && vaga.tier_ok;
              return (
                <li
                  key={`${vaga.team_id}-${vaga.papel}`}
                  data-elegivel={elegivel ? "true" : "false"}
                  className={`flex flex-wrap items-center gap-x-4 gap-y-1.5 rounded-lg border px-3.5 py-2.5 ${
                    elegivel
                      ? "border-accent-primary/25 bg-accent-primary/[0.07]"
                      : "border-white/6 bg-black/10"
                  }`}
                >
                  <span
                    aria-hidden="true"
                    className="h-6 w-1 shrink-0 rounded-full"
                    style={{ backgroundColor: getVividTeamColor(vaga.team_color || "") }}
                  />
                  <span className="min-w-0 flex-1 truncate text-sm font-medium text-text-primary">
                    {vaga.team_name}
                  </span>
                  <span className="shrink-0 text-xs text-text-secondary">
                    {formatCategoryLabel(
                      vaga.classe ? `${vaga.categoria}:${vaga.classe}` : vaga.categoria,
                    )}
                  </span>
                  <span className="shrink-0 text-xs font-semibold text-text-secondary">
                    {formatContractRole(vaga.papel)}
                  </span>
                  <span
                    className="shrink-0 font-mono text-xs text-text-muted"
                    style={{ fontVariantNumeric: "tabular-nums" }}
                  >
                    {t("carreiraTab.mercado.car", { value: vaga.car_performance_rating })}
                  </span>
                  {/* Salário só vem nas elegíveis: estimar oferta para um assento
                      que o jogador não pode ocupar seria inventar uma proposta que
                      o mercado nunca faria. */}
                  <span className="shrink-0 text-xs">
                    {Number.isFinite(vaga.salario_estimado) ? (
                      <span className="font-semibold text-accent-primary">
                        {formatMoneyCompact(vaga.salario_estimado)}
                      </span>
                    ) : (
                      <span className="text-text-muted">
                        {vaga.licenca_ok
                          ? t("carreiraTab.mercado.outOfTier")
                          : t("carreiraTab.mercado.noLicense")}
                      </span>
                    )}
                  </span>
                </li>
              );
            })}
          </ul>
        ) : (
          <Vazio>{t("carreiraTab.mercado.noSeats")}</Vazio>
        )}
      </Bloco>
    </div>
  );
}

function Linha({ rotulo, valor }) {
  return (
    <div className="flex items-baseline justify-between gap-3 border-b border-white/6 py-2 last:border-b-0">
      <dt className="text-[11px] uppercase tracking-[0.14em] text-text-muted">{rotulo}</dt>
      <dd className="text-right text-sm font-medium text-text-primary">{valor || "-"}</dd>
    </div>
  );
}

export default MercadoSection;
