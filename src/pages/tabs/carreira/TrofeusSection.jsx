import { useTranslation } from "react-i18next";
import { Award, Crown, Flag, Medal, Timer, Trophy } from "lucide-react";

import {
  formatCategoryLabel,
  formatRaceMilestone,
  formatSeasonMilestone,
  formatSeasonWithResult,
  formatStreakRaces,
} from "../../../components/driver/detalhes/formatadores.js";
import { getVividTeamColor } from "../../../utils/teamColors";
import { Bloco, Numero, Vazio } from "./primitivos.jsx";

// F-04 — a sala de troféus.
//
// Come da mesma `race_history` que a História, e por isso mora ao lado dela em vez
// de numa tela própria: separar teria triplicado a navegação para uma fonte só.
// A divisão de assunto entre as duas é a que o jogador faz na cabeça: a História é
// a linha do tempo (onde eu estive, em que posição), esta é o acervo (o que eu
// levei para casa).
//
// O que deliberadamente NÃO entra aqui: a posição do jogador nos recordes do mundo.
// Ela existe (`get_driver_dossier_ranks`), mas custa uma varredura de `race_results`
// e do arquivo inteiro — ~500ms num save maduro. A sala de troféus tem que abrir na
// hora, e "205º de 610 em vitórias" é a pergunta seguinte, não esta. Quem quer o
// ranking abre a lista global de pilotos, que é onde ele já vive.
function TrofeusSection({ detail }) {
  const { t } = useTranslation();
  const trajetoria = detail.trajetoria ?? {};
  const historico = trajetoria.historico ?? {};
  const carreira = detail.stats_carreira ?? {};
  const sabado = historico.sabado ?? {};
  const auge = historico.auge ?? {};
  const marcos = historico.primeiros_marcos ?? {};
  const confiabilidade = historico.confiabilidade ?? {};
  const especiais = historico.eventos_especiais ?? {};
  const titulos = Array.isArray(trajetoria.titulos_detalhe) ? trajetoria.titulos_detalhe : [];
  const totalTitulos = trajetoria.titulos ?? 0;
  const eventos = Array.isArray(detail.trajetoria?.marcos) ? detail.trajetoria.marcos : [];

  return (
    <div className="space-y-4">
      <Bloco titulo={t("carreiraTab.trofeus.shelf")} testId="carreira-trofeus-titulos">
        {totalTitulos > 0 ? (
          <>
            {titulos.length ? (
              <ul className="grid gap-2.5 sm:grid-cols-2 lg:grid-cols-3">
                {titulos.map((titulo, indice) => (
                  <li
                    key={`${titulo.ano}-${titulo.categoria}-${indice}`}
                    className="flex items-center gap-3 rounded-xl border border-[#f0b232]/25 bg-[#f0b232]/[0.07] px-3.5 py-3"
                  >
                    <Trophy
                      size={20}
                      strokeWidth={1.8}
                      aria-hidden="true"
                      className="shrink-0 text-[#f0b232]"
                    />
                    <div className="min-w-0">
                      <div
                        className="font-mono text-lg font-semibold leading-none text-[#f0b232]"
                        style={{ fontVariantNumeric: "tabular-nums" }}
                      >
                        {titulo.ano}
                      </div>
                      <div className="mt-1.5 truncate text-xs text-text-secondary">
                        {formatCategoryLabel(titulo.categoria)}
                        {/* A equipe do ano do título é resolvida pela identidade
                            ATUAL do `team_id`: o nome histórico não é arquivado.
                            `None` quando o time não existe mais — e aí a linha
                            mostra só a categoria em vez de um id cru. */}
                        {titulo.equipe ? (
                          <>
                            {" · "}
                            <span style={{ color: getVividTeamColor(titulo.equipe_cor || "") }}>
                              {titulo.equipe}
                            </span>
                          </>
                        ) : null}
                      </div>
                    </div>
                  </li>
                ))}
              </ul>
            ) : (
              // Piloto histórico pré-gerado tem o TOTAL de títulos e não tem
              // arquivo de temporada de onde tirar ano e equipe. O total é
              // verdade; a lista não existe, e inventá-la seria pior.
              <p className="text-sm text-text-secondary">
                {t("carreiraTab.trofeus.titlesWithoutDetail", { count: totalTitulos })}
              </p>
            )}
          </>
        ) : (
          <Vazio>{t("carreiraTab.trofeus.noTitles")}</Vazio>
        )}
      </Bloco>

      <Bloco titulo={t("carreiraTab.trofeus.career")} testId="carreira-trofeus-numeros">
        <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-6">
          <Numero valor={carreira.corridas ?? 0} rotulo={t("carreiraTab.trofeus.starts")} />
          <Numero
            valor={carreira.vitorias ?? 0}
            rotulo={t("carreiraTab.trofeus.wins")}
            tom={(carreira.vitorias ?? 0) > 0 ? "text-[#f0b232]" : undefined}
          />
          <Numero valor={carreira.podios ?? 0} rotulo={t("carreiraTab.trofeus.podiums")} />
          <Numero
            valor={sabado.poles ?? carreira.poles ?? 0}
            rotulo={t("carreiraTab.trofeus.poles")}
            nota={
              (sabado.poles ?? 0) > 0
                ? t("carreiraTab.trofeus.polesConverted", { count: sabado.poles_convertidas ?? 0 })
                : null
            }
          />
          <Numero
            valor={sabado.voltas_rapidas ?? 0}
            rotulo={t("carreiraTab.trofeus.fastestLaps")}
          />
          <Numero
            valor={carreira.pontos ?? 0}
            rotulo={t("carreiraTab.trofeus.points")}
            nota={
              carreira.melhor_resultado
                ? t("carreiraTab.trofeus.bestFinish", { position: carreira.melhor_resultado })
                : null
            }
          />
        </div>

        {Number.isFinite(confiabilidade.taxa_abandono) ? (
          <p className="mt-3.5 border-t border-white/[0.08] pt-3 text-sm text-text-secondary">
            {t("carreiraTab.trofeus.reliability", {
              dnfs: confiabilidade.abandonos ?? 0,
              rate: confiabilidade.taxa_abandono.toFixed(1),
              streak: confiabilidade.maior_sequencia_chegadas ?? 0,
            })}
          </p>
        ) : null}
      </Bloco>

      <div className="grid gap-4 lg:grid-cols-2">
        <Bloco titulo={t("carreiraTab.trofeus.firsts")} testId="carreira-trofeus-primeiros">
          <dl className="grid gap-y-1">
            <Linha
              icone={Medal}
              rotulo={t("carreiraTab.trofeus.firstPodium")}
              valor={formatRaceMilestone(marcos.primeiro_podio_corrida)}
            />
            <Linha
              icone={Crown}
              rotulo={t("carreiraTab.trofeus.firstWin")}
              valor={formatRaceMilestone(marcos.primeira_vitoria_corrida)}
            />
            <Linha
              icone={Award}
              rotulo={t("carreiraTab.trofeus.firstTitle")}
              valor={formatSeasonMilestone(marcos.primeiro_titulo)}
            />
            <Linha
              icone={Flag}
              rotulo={t("carreiraTab.trofeus.firstDnf")}
              valor={formatRaceMilestone(marcos.primeiro_dnf_corrida)}
            />
          </dl>
        </Bloco>

        <Bloco titulo={t("carreiraTab.trofeus.peak")} testId="carreira-trofeus-auge">
          <dl className="grid gap-y-1">
            <Linha
              icone={Trophy}
              rotulo={t("carreiraTab.trofeus.bestSeason")}
              valor={formatSeasonWithResult(auge.melhor_temporada)}
            />
            {/* O período entre parênteses é o que separa um número de uma
                travessia: "8" é uma sequência, "8 (2031–2032)" é uma sequência
                que atravessou a virada do ano. */}
            <Linha
              icone={Crown}
              rotulo={t("carreiraTab.trofeus.winStreak")}
              valor={formatStreakRaces(
                auge.maior_sequencia_vitorias,
                auge.sequencia_ano_inicio,
                auge.sequencia_ano_fim,
              )}
            />
            <Linha
              icone={Medal}
              rotulo={t("carreiraTab.trofeus.podiumStreak")}
              valor={formatStreakRaces(
                auge.maior_sequencia_podios,
                auge.sequencia_podios_ano_inicio,
                auge.sequencia_podios_ano_fim,
              )}
            />
            <Linha
              icone={Timer}
              rotulo={t("carreiraTab.trofeus.top3Seasons")}
              valor={auge.temporadas_no_top3 ?? 0}
            />
          </dl>
        </Bloco>
      </div>

      {/* Os eventos especiais são um acervo à parte: campanhas fora do calendário
          regular, com pódio e vitória próprios. Só entra quando o jogador foi
          convocado alguma vez — para quem nunca foi, um bloco de zeros anunciaria
          um sistema que ele não conhece. */}
      {(especiais.participacoes ?? 0) > 0 ? (
        <Bloco titulo={t("carreiraTab.trofeus.special")} testId="carreira-trofeus-especiais">
          <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
            <Numero
              valor={especiais.participacoes ?? 0}
              rotulo={t("carreiraTab.trofeus.specialEntries")}
            />
            <Numero
              valor={especiais.convocacoes ?? 0}
              rotulo={t("carreiraTab.trofeus.specialCallups")}
            />
            <Numero valor={especiais.vitorias ?? 0} rotulo={t("carreiraTab.trofeus.wins")} />
            <Numero valor={especiais.podios ?? 0} rotulo={t("carreiraTab.trofeus.podiums")} />
          </div>
        </Bloco>
      ) : null}

      {eventos.length ? (
        <Bloco titulo={t("carreiraTab.trofeus.milestones")} testId="carreira-trofeus-marcos">
          <ul className="space-y-2">
            {eventos.map((marco, indice) => (
              <li
                key={`${marco.tipo}-${indice}`}
                className="rounded-lg border border-white/[0.06] bg-black/10 px-3.5 py-2.5"
              >
                <strong className="block text-sm font-semibold text-text-primary">
                  {marco.titulo}
                </strong>
                <p className="mt-1 text-xs leading-relaxed text-text-secondary">
                  {marco.descricao}
                </p>
              </li>
            ))}
          </ul>
        </Bloco>
      ) : null}
    </div>
  );
}

function Linha({ icone: Icone, rotulo, valor }) {
  return (
    <div className="flex items-center justify-between gap-3 border-b border-white/[0.06] py-2 last:border-b-0">
      <dt className="flex min-w-0 items-center gap-2.5 text-xs text-text-secondary">
        <Icone size={14} strokeWidth={1.8} aria-hidden="true" className="shrink-0 text-text-muted" />
        <span className="truncate">{rotulo}</span>
      </dt>
      <dd className="shrink-0 text-right text-sm font-medium text-text-primary">{valor}</dd>
    </div>
  );
}

export default TrofeusSection;
