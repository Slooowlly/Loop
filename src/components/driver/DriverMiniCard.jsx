import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

import AnchoredCard, { NumeroDaFicha, SecaoDaFicha } from "../ui/AnchoredCard";
import FlagIcon from "../ui/FlagIcon";
import useCareerStore from "../../stores/useCareerStore";
import { extractNationalityLabel, formatSalaryAnnual } from "../../utils/formatters";
import { nationalityForFlag } from "./detalhes/cabecalho.jsx";
import { formatAverage } from "./detalhes/formatadores.js";

// A ficha rápida do piloto — o dossiê reduzido ao que decide um assento.
//
// A mecânica do painel (portal, ancoragem, fechar) mora no `AnchoredCard`; aqui
// só vive o que é do piloto.
//
// O CARREIRA vem antes do TEMPORADA de propósito. A ficha abre na janela de
// transferências, onde metade dos nomes é de piloto sem equipe: temporada zerada
// é o caso NORMAL ali, e abrir a ficha com quatro zeros dizia que o piloto não
// vale nada quando o que ele tem são cinquenta corridas e uma vitória na
// carreira. A temporada só aparece quando houve temporada.
//
// A licença saiu: numa escada fechada de nove categorias, a categoria em que o
// piloto corre já é o degrau — a sigla da licença repetia isso com outra grafia.

const LARGURA = 320;

// Cor da chegada, na mesma régua do pódio que o resto do mercado usa.
function corDaChegada(entrada) {
  if (entrada?.dnf) return { fg: "#f85149", bg: "rgba(248,81,73,0.14)" };
  const chegada = entrada?.chegada;
  if (chegada === 1) return { fg: "#ffd700", bg: "rgba(255,215,0,0.16)" };
  if (chegada === 2) return { fg: "#c0c0c0", bg: "rgba(192,192,192,0.14)" };
  if (chegada === 3) return { fg: "#cd7f32", bg: "rgba(205,127,50,0.16)" };
  if (chegada != null && chegada <= 10) return { fg: "#c9d1d9", bg: "rgba(255,255,255,0.06)" };
  return { fg: "#7d8590", bg: "rgba(255,255,255,0.04)" };
}

// O que ele fez NESTA CASA, somado da curva de campeonato.
//
// "55 corridas na carreira" e "Club Racer Motorsport" são dois fatos soltos: o
// que decide se vale renovar é o que aconteceu DEPOIS que ele entrou ali. A
// curva já traz uma temporada por ponto, com equipe, corridas, vitórias, pódios
// e se valeu título — a soma é aritmética, não precisa de consulta nova.
//
// Casa pelo NOME da equipe porque é o que a curva arquiva, e ele é resolvido
// pela identidade atual do time nos dois lados — o mesmo `equipe_nome` que o
// cabeçalho da ficha mostra.
function campanhaNaEquipe(detail) {
  const equipe = detail.equipe_nome;
  const curva = detail.trajetoria?.curva_campeonato ?? [];
  if (!equipe || curva.length === 0) return null;

  const naCasa = curva.filter((ponto) => ponto?.equipe_nome === equipe);
  if (naCasa.length === 0) return null;

  return {
    temporadas: naCasa.length,
    corridas: naCasa.reduce((soma, ponto) => soma + (ponto.corridas ?? 0), 0),
    vitorias: naCasa.reduce((soma, ponto) => soma + (ponto.vitorias ?? 0), 0),
    podios: naCasa.reduce((soma, ponto) => soma + (ponto.podios ?? 0), 0),
    titulos: naCasa.filter((ponto) => ponto.titulo).length,
  };
}

// Salário e prazo numa linha só. Zero temporada restante não é "0 temporadas":
// é o último ano — a informação que decide se vale a pena sondar o piloto agora.
function contratoEmUmaLinha(t, contrato) {
  const salary = formatSalaryAnnual(contrato.salario_anual);
  const restantes = Math.max(contrato.anos_restantes ?? 0, 0);
  return restantes > 0
    ? t("driverMiniCard.contract", { salary, count: restantes })
    : t("driverMiniCard.contractFinalYear", { salary });
}

function Corpo({ detail }) {
  const { t } = useTranslation();
  const perfil = detail.perfil ?? {};
  const nacionalidade = nationalityForFlag(perfil, detail);
  const paisLabel = extractNationalityLabel(nacionalidade) || perfil.nacionalidade || "";
  const temporada = detail.performance?.temporada ?? detail.stats_temporada ?? {};
  const carreira = detail.performance?.carreira ?? detail.stats_carreira ?? {};
  const forma = detail.forma ?? {};
  const ultimas = (forma.ultimas_5 ?? []).slice(-5);
  const contrato = detail.contrato;
  const qualidades = (detail.competitivo?.qualidades ?? []).slice(0, 3);
  const media = forma.media_chegada;
  const correuNaTemporada = (temporada.corridas ?? 0) > 0;
  const titulos = detail.trajetoria?.titulos ?? 0;
  const naEquipe = campanhaNaEquipe(detail);

  return (
    <>
      <div className="flex items-start gap-2.5 px-3 pb-2.5 pt-3">
        <FlagIcon nacionalidade={nacionalidade} className="mt-0.5 shrink-0 rounded-sm" />
        <div className="min-w-0 flex-1">
          <p className="truncate pr-5 text-[15px] font-bold leading-[1.1] text-[color:var(--text-primary)]">
            {detail.nome}
          </p>
          <p className="mt-0.5 flex items-center gap-1.5 truncate text-[10px] text-[color:var(--text-muted)]">
            {paisLabel && <span className="truncate">{paisLabel}</span>}
            {paisLabel && <span aria-hidden="true">·</span>}
            <span className="shrink-0 tabular-nums">
              {t("driverMiniCard.age", { count: detail.idade })}
            </span>
            {detail.personalidade_primaria?.tipo && (
              <>
                <span aria-hidden="true">·</span>
                <span className="truncate">
                  {detail.personalidade_primaria.emoji} {detail.personalidade_primaria.tipo}
                </span>
              </>
            )}
          </p>
        </div>
      </div>

      <div className="flex items-center gap-2 border-t border-white/[0.08] px-3 py-2">
        <span
          aria-hidden="true"
          className="h-6 w-1 shrink-0 rounded-full"
          style={{ background: detail.equipe_cor_primaria ?? "rgba(255,255,255,0.16)" }}
        />
        <div className="min-w-0 flex-1">
          <p className="truncate text-[12px] font-semibold leading-tight text-[color:var(--text-primary)]">
            {detail.equipe_nome ?? t("driverMiniCard.noTeam")}
          </p>
          <p className="truncate text-[10px] leading-tight text-[color:var(--text-muted)]">
            {contrato ? contratoEmUmaLinha(t, contrato) : t("driverMiniCard.noContract")}
          </p>
        </div>
      </div>

      {/* A CASA ATUAL vem antes da carreira, e só para quem tem casa.
          A pergunta da janela de transferências é sobre o assento que está na
          tela: o que ele fez AQUI decide se renova, e o acumulado da carreira é
          o pano de fundo contra o qual isso se lê. Piloto livre não tem nada a
          somar — uma faixa de zeros diria que ele fracassou onde nem esteve.

          Sem o nome da equipe no título: ele está escrito três centímetros
          acima, no cabeçalho. A cor faz o vínculo sem gastar a linha. */}
      {naEquipe && (
        <SecaoDaFicha
          titulo={t("driverMiniCard.thisTeam")}
          cor={detail.equipe_cor_primaria ?? undefined}
        >
          {naEquipe.corridas === 0 ? (
            // Contratado e ainda sem largada por eles. "1 Temporada · 0 · 0 · 0"
            // conta uma campanha que não existe — e cinco zeros lidos rápido
            // dizem "fracassou aqui", que é o contrário do que aconteceu.
            //
            // A palavra é a MESMA que o contador de tempo de casa usa no grid,
            // pela mesma chave: são o mesmo fato visto de dois lugares, e é o
            // grid que o jogador leu primeiro.
            <div className="flex items-center gap-2">
              <span className="shrink-0 rounded-md bg-[#bc8cff22] px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-[0.12em] text-[#bc8cff]">
                {t("preSeason.roster.tenureNew")}
              </span>
              <span className="min-w-0 flex-1 truncate text-[11px] text-[color:var(--text-secondary)]">
                {t("driverMiniCard.notRacedYet")}
              </span>
            </div>
          ) : (
            <div className="flex items-start gap-1">
              <NumeroDaFicha rotulo={t("driverMiniCard.seasons")} valor={naEquipe.temporadas} />
              <NumeroDaFicha rotulo={t("driverMiniCard.races")} valor={naEquipe.corridas} />
              <NumeroDaFicha rotulo={t("driverMiniCard.wins")} valor={naEquipe.vitorias} />
              <NumeroDaFicha rotulo={t("driverMiniCard.podiums")} valor={naEquipe.podios} />
              <NumeroDaFicha rotulo={t("driverMiniCard.titles")} valor={naEquipe.titulos} destaque />
            </div>
          )}
        </SecaoDaFicha>
      )}

      <SecaoDaFicha titulo={t("driverMiniCard.career")}>
        <div className="flex items-start gap-1">
          <NumeroDaFicha rotulo={t("driverMiniCard.titles")} valor={titulos} destaque />
          <NumeroDaFicha rotulo={t("driverMiniCard.races")} valor={carreira.corridas} />
          <NumeroDaFicha rotulo={t("driverMiniCard.wins")} valor={carreira.vitorias} />
          <NumeroDaFicha rotulo={t("driverMiniCard.podiums")} valor={carreira.podios} />
          <NumeroDaFicha rotulo={t("driverMiniCard.poles")} valor={carreira.poles} />
        </div>
      </SecaoDaFicha>

      {correuNaTemporada && (
        <SecaoDaFicha titulo={t("driverMiniCard.season")}>
          <p className="text-[11px] leading-snug text-[color:var(--text-secondary)]">
            {t("driverMiniCard.seasonLine", {
              races: temporada.corridas ?? 0,
              wins: temporada.vitorias ?? 0,
              podiums: temporada.podios ?? 0,
            })}
          </p>
        </SecaoDaFicha>
      )}

      {ultimas.length > 0 && (
        <SecaoDaFicha titulo={t("driverMiniCard.form")}>
          <div className="flex items-center gap-1.5">
            {ultimas.map((entrada, indice) => {
              const cor = corDaChegada(entrada);
              return (
                <span
                  key={`${entrada?.rodada ?? indice}-${indice}`}
                  className="flex h-6 min-w-[24px] items-center justify-center rounded-md px-1 text-[11px] font-black tabular-nums"
                  style={{ color: cor.fg, background: cor.bg }}
                >
                  {entrada?.dnf ? t("driverMiniCard.dnfShort") : entrada?.chegada ?? "—"}
                </span>
              );
            })}
            {media != null && (
              <span className="ml-auto text-[10px] font-semibold tabular-nums text-[color:var(--text-muted)]">
                {t("driverMiniCard.average", { value: formatAverage(media) })}
              </span>
            )}
          </div>
        </SecaoDaFicha>
      )}

      {qualidades.length > 0 && (
        <SecaoDaFicha titulo={t("driverMiniCard.traits")}>
          <div className="flex flex-wrap gap-1.5">
            {qualidades.map((tag) => (
              <span
                key={tag.attribute_name}
                className="rounded-md px-1.5 py-0.5 text-[9.5px] font-bold uppercase tracking-[0.06em]"
                style={{ color: tag.color, background: `${tag.color}1f` }}
              >
                {tag.tag_text}
              </span>
            ))}
          </div>
        </SecaoDaFicha>
      )}
    </>
  );
}

// Busca o dossiê ao montar — e só monta quando a ficha abre, então nenhum grid
// dispara vinte consultas por desenhar vinte nomes.
//
// Sem cache de propósito. A tentação é guardar o dossiê entre aberturas, mas na
// janela de transferências ele é justamente o dado que MUDA a cada semana que o
// jogador avança: equipe, contrato, salário. Um cache que sobrevivesse ao avanço
// mostraria o contrato da semana passada com a cara de dado atual — e a consulta
// que ele economiza é uma leitura de SQLite local disparada por um clique.
function CorpoBuscado({ driverId }) {
  const { t } = useTranslation();
  const careerId = useCareerStore((state) => state.careerId);
  const [detail, setDetail] = useState(null);
  const [erro, setErro] = useState("");

  useEffect(() => {
    let ativo = true;

    invoke("get_driver_detail", { careerId, driverId })
      .then((dados) => {
        if (ativo) setDetail(dados);
      })
      .catch(() => {
        if (ativo) setErro(t("driverMiniCard.loadError"));
      });

    return () => {
      ativo = false;
    };
  }, [careerId, driverId, t]);

  if (erro) return <p className="px-3 py-4 text-[11px] text-[color:var(--status-red)]">{erro}</p>;
  if (!detail) {
    return (
      <p className="px-3 py-4 text-[11px] text-[color:var(--text-muted)]">
        {t("driverMiniCard.loading")}
      </p>
    );
  }
  return <Corpo detail={detail} />;
}

/// Envolve o nome do piloto e abre a ficha rápida no clique.
export default function DriverMiniCard({ driverId, children }) {
  const { t } = useTranslation();

  return (
    <AnchoredCard
      largura={LARGURA}
      testId="driver-mini-card"
      rotuloFechar={t("driverMiniCard.close")}
      desabilitado={!driverId}
      conteudo={<CorpoBuscado driverId={driverId} />}
    >
      {children}
    </AnchoredCard>
  );
}
