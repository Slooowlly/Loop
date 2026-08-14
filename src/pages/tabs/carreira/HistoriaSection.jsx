import { useTranslation } from "react-i18next";

import { CurvaDeCampeonato } from "../../../components/driver/v2/CurvaDeCampeonato.jsx";
import {
  formatCategoryLabel,
  formatUnemploymentYears,
  formatYearsAverage,
} from "../../../components/driver/detalhes/formatadores.js";
import { getCategoryColor } from "../../../utils/categoryColors";
import { getVividTeamColor } from "../../../utils/teamColors";
import { Bloco, Numero, Vazio } from "./primitivos.jsx";

// F-03 — as temporadas passadas.
//
// O buraco que esta seção fecha é o mais simples de descrever: o `EndOfSeasonView`
// mostra o balanço do ano uma vez, na virada, e nunca mais. A informação era gerada,
// exibida por um instante e sumia. Aqui ela fica.
//
// Nada é calculado no front: a curva, a linha do tempo de categorias e a tabela por
// temporada saem de `trajetoria.curva_campeonato` e `trajetoria.categorias_timeline`,
// que o backend monta a partir do arquivo de temporadas. A curva é o MESMO componente
// da ficha v2 — desenhar um segundo gráfico de campeonato seria dar ao jogador duas
// leituras diferentes do mesmo passado.
function HistoriaSection({ detail }) {
  const { t } = useTranslation();
  const trajetoria = detail.trajetoria ?? {};
  const historico = trajetoria.historico ?? {};
  const presenca = historico.presenca ?? {};
  const mobilidade = historico.mobilidade ?? {};
  const curva = Array.isArray(trajetoria.curva_campeonato) ? trajetoria.curva_campeonato : [];
  const escada = Array.isArray(trajetoria.categorias_timeline)
    ? trajetoria.categorias_timeline
    : [];
  // Da mais recente para a mais antiga: a pergunta "como foi o ano passado" é mais
  // frequente que "como foi a estreia", e quem quer a estreia rola até o fim.
  const temporadas = [...curva].reverse();

  return (
    <div className="space-y-4">
      <Bloco titulo={t("carreiraTab.historia.summary")} testId="carreira-historia-resumo">
        <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-6">
          <Numero
            valor={presenca.temporadas_disputadas ?? 0}
            rotulo={t("carreiraTab.historia.seasons")}
          />
          <Numero valor={presenca.corridas ?? 0} rotulo={t("carreiraTab.historia.races")} />
          <Numero
            valor={presenca.categorias_disputadas ?? 0}
            rotulo={t("carreiraTab.historia.categories")}
          />
          <Numero
            valor={mobilidade.equipes_defendidas ?? 0}
            rotulo={t("carreiraTab.historia.teams")}
            nota={
              Number.isFinite(mobilidade.tempo_medio_por_equipe)
                ? formatYearsAverage(mobilidade.tempo_medio_por_equipe)
                : null
            }
          />
          <Numero
            valor={mobilidade.promocoes ?? 0}
            rotulo={t("carreiraTab.historia.promotions")}
            tom={(mobilidade.promocoes ?? 0) > 0 ? "text-status-green" : undefined}
          />
          <Numero
            valor={mobilidade.rebaixamentos ?? 0}
            rotulo={t("carreiraTab.historia.demotions")}
            tom={(mobilidade.rebaixamentos ?? 0) > 0 ? "text-status-red" : undefined}
          />
        </div>

        {/* Ano parado é fato de carreira, não erro de dado: quem ficou sem vaga
            tem um buraco na linha do tempo, e esconder isso apagaria metade da
            história de um piloto que caiu e voltou. */}
        {(presenca.anos_desempregado ?? 0) > 0 ? (
          <p className="mt-3.5 border-t border-white/[0.08] pt-3 text-sm text-text-secondary">
            {t("carreiraTab.historia.idleYears")}
            {": "}
            {formatUnemploymentYears(presenca)}
          </p>
        ) : null}
      </Bloco>

      {escada.length ? (
        <Bloco titulo={t("carreiraTab.historia.ladder")} testId="carreira-historia-escada">
          {/* A escada em faixas horizontais, uma por passagem de categoria. É a
              leitura que a tabela não dá: subiu, ficou, caiu. */}
          <ol className="space-y-2">
            {escada.map((etapa, indice) => {
              const cor = getCategoryColor(etapa.categoria);
              return (
                <li
                  key={`${etapa.categoria}-${etapa.ano_inicio}-${indice}`}
                  className="flex items-center gap-3 rounded-lg border border-white/[0.06] bg-black/10 px-3 py-2"
                >
                  <span
                    aria-hidden="true"
                    className="h-6 w-1 shrink-0 rounded-full"
                    style={{ backgroundColor: cor }}
                  />
                  <span className="min-w-0 flex-1 truncate text-sm font-medium text-text-primary">
                    {formatCategoryLabel(etapa.categoria)}
                  </span>
                  <span
                    className="shrink-0 font-mono text-xs text-text-secondary"
                    style={{ fontVariantNumeric: "tabular-nums" }}
                  >
                    {etapa.ano_inicio === etapa.ano_fim
                      ? etapa.ano_inicio
                      : `${etapa.ano_inicio} - ${etapa.ano_fim}`}
                  </span>
                </li>
              );
            })}
          </ol>
        </Bloco>
      ) : null}

      {/* A curva se recusa a desenhar com menos de duas temporadas fechadas (ela
          retorna `null` sozinha), então o bloco só entra quando há trajetória. Um
          ponto não é trajetória. */}
      {curva.length >= 2 ? (
        <Bloco titulo={t("carreiraTab.historia.curve")} testId="carreira-historia-curva">
          <CurvaDeCampeonato pontos={curva} equipeDeEstreia={trajetoria.equipe_estreia ?? null} />
        </Bloco>
      ) : null}

      <Bloco
        titulo={t("carreiraTab.historia.seasonTable")}
        testId="carreira-historia-temporadas"
        acao={
          trajetoria.ano_estreia ? (
            <span className="text-[11px] text-text-secondary">
              {t("carreiraTab.historia.debut", {
                year: trajetoria.ano_estreia,
                team: trajetoria.equipe_estreia || t("carreiraTab.historia.unknownTeam"),
              })}
            </span>
          ) : null
        }
      >
        {temporadas.length ? (
          <div className="overflow-x-auto">
            <table className="w-full min-w-[620px] border-collapse text-sm">
              <thead>
                <tr className="text-left text-[10px] uppercase tracking-[0.14em] text-text-muted">
                  <th className="pb-2 pr-3 font-semibold">{t("carreiraTab.historia.colYear")}</th>
                  <th className="pb-2 pr-3 font-semibold">
                    {t("carreiraTab.historia.colCategory")}
                  </th>
                  <th className="pb-2 pr-3 font-semibold">{t("carreiraTab.historia.colTeam")}</th>
                  <th className="pb-2 pr-3 text-right font-semibold">
                    {t("carreiraTab.historia.colPosition")}
                  </th>
                  <th className="pb-2 pr-3 text-right font-semibold">
                    {t("carreiraTab.historia.colRaces")}
                  </th>
                  <th className="pb-2 pr-3 text-right font-semibold">
                    {t("carreiraTab.historia.colWins")}
                  </th>
                  <th className="pb-2 pr-3 text-right font-semibold">
                    {t("carreiraTab.historia.colPodiums")}
                  </th>
                  <th className="pb-2 text-right font-semibold">
                    {t("carreiraTab.historia.colPoints")}
                  </th>
                </tr>
              </thead>
              <tbody>
                {temporadas.map((ponto) => (
                  <tr
                    key={`${ponto.season_number}-${ponto.ano}`}
                    data-atual={ponto.atual ? "true" : undefined}
                    className="border-t border-white/[0.06]"
                  >
                    <td className="py-2 pr-3">
                      <span
                        className="font-mono text-text-primary"
                        style={{ fontVariantNumeric: "tabular-nums" }}
                      >
                        {ponto.ano}
                      </span>
                      {/* A temporada em curso é PARCIAL por natureza. Sem a marca,
                          um P9 de abril se lê como o resultado final do ano. */}
                      {ponto.atual ? (
                        <span className="ml-2 rounded-full border border-accent-primary/40 bg-accent-primary/10 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-[0.1em] text-accent-primary">
                          {t("carreiraTab.historia.inProgress")}
                        </span>
                      ) : null}
                    </td>
                    <td className="py-2 pr-3 text-text-secondary">
                      {ponto.categoria ? formatCategoryLabel(ponto.categoria) : "-"}
                    </td>
                    <td className="py-2 pr-3">
                      {ponto.equipe_nome ? (
                        <span style={{ color: getVividTeamColor(ponto.equipe_cor || "") }}>
                          {ponto.equipe_nome}
                        </span>
                      ) : (
                        <span className="text-text-muted">-</span>
                      )}
                    </td>
                    <td className="py-2 pr-3 text-right">
                      {ponto.posicao ? (
                        <span
                          className={`font-mono ${ponto.titulo ? "font-bold text-[#f0b232]" : "text-text-primary"}`}
                          style={{ fontVariantNumeric: "tabular-nums" }}
                        >
                          P{ponto.posicao}
                          {/* O denominador vem arquivado junto com a posição, e é
                              ele que separa um P8 entre doze de um P8 entre trinta.
                              `None` em save antigo — e aí a célula mostra só a
                              posição em vez de inventar o tamanho do grid. */}
                          {ponto.grid ? (
                            <span className="text-text-muted">/{ponto.grid}</span>
                          ) : null}
                        </span>
                      ) : (
                        <span className="text-text-muted">-</span>
                      )}
                    </td>
                    <td
                      className="py-2 pr-3 text-right font-mono text-text-secondary"
                      style={{ fontVariantNumeric: "tabular-nums" }}
                    >
                      {ponto.corridas ?? 0}
                    </td>
                    <td
                      className="py-2 pr-3 text-right font-mono text-text-primary"
                      style={{ fontVariantNumeric: "tabular-nums" }}
                    >
                      {ponto.vitorias ?? 0}
                    </td>
                    <td
                      className="py-2 pr-3 text-right font-mono text-text-secondary"
                      style={{ fontVariantNumeric: "tabular-nums" }}
                    >
                      {ponto.podios ?? 0}
                    </td>
                    <td
                      className="py-2 text-right font-mono text-text-secondary"
                      style={{ fontVariantNumeric: "tabular-nums" }}
                    >
                      {Number.isFinite(ponto.pontos) ? Math.round(ponto.pontos) : "-"}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <Vazio>{t("carreiraTab.historia.empty")}</Vazio>
        )}
      </Bloco>
    </div>
  );
}

export default HistoriaSection;
