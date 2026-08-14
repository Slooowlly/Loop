import { Fragment } from "react";
import { useTranslation } from "react-i18next";

import Tooltip from "../../ui/Tooltip";
import TeamLogoMark from "../../team/TeamLogoMark";
import { ordinal } from "../../../i18n/format.js";
import { formatSalary, formatSalaryAnnual } from "../../../utils/formatters";
import { formatCategoryLabel, formatContractRole } from "../detalhes/formatadores.js";
import { CURVA_MERCADO, CURVA_PAGO, MarketCurve } from "./CurvaDeMercado.jsx";
import { TONE_HEX, tendenciaDeValor } from "./driverDetailV2Logic";

// ─────────────────────────────── Mercado ───────────────────────────────

// A aba era dois cards de números soltos, e dois deles eram o MESMO número: o
// salário aparecia no contrato e outra vez no mercado, idêntico, porque o
// estimado caía no contratado quando havia contrato. Agora o topo é a chance de
// troca decomposta — a única pergunta viva da aba — e embaixo um card só, com o
// que ele vale e o que custa lado a lado.
export function MarketSection({ detail }) {
  const { t } = useTranslation();
  const contract = detail.contrato_mercado?.contrato;
  const market = detail.contrato_mercado?.mercado;

  return (
    <section>
      {market ? <TransferThermometer market={market} temContrato={Boolean(contract)} /> : null}

      <MarketCurve pontos={detail.contrato_mercado?.curva} />

      {market ? (
        <SituacaoContratual
          contract={contract}
          market={market}
          curva={detail.contrato_mercado?.curva}
        />
      ) : (
        <div className="mt-3 rounded-xl bg-[#0f1c2b] px-4 py-3.5 text-xs text-text-secondary">
          {t("driverDetail.market.noMarketSignals")}
        </div>
      )}
    </section>
  );
}

// Cada força tem cor própria e fixa: quem olha duas fichas seguidas precisa ler
// "o vermelho cresceu" sem reconferir a legenda.
const FORCA_TONE = { contrato: "warning", motivacao: "danger", mercado: "info" };
const FORCAS = ["contrato", "motivacao", "mercado"];

// O termômetro: o número grande e a barra empilhada que o explica.
//
// 57% sozinho não diz se o piloto está infeliz ou se é só o contrato acabando —
// e essas duas situações pedem reações opostas do jogador. A decomposição vem
// pronta do backend justamente porque o cálculo sempre soube a diferença.
//
// Havia aqui uma frase que narrava a força dominante ("o contrato acaba nesta
// janela…"). Saiu: a barra já mostra quem manda e as legendas já dizem o que
// cada uma é — o parágrafo repetia em prosa o que estava desenhado dois pixels
// acima e roubava a altura do cartão.
function TransferThermometer({ market, temContrato }) {
  const { t } = useTranslation();
  const chance = Number.isFinite(market.chance_transferencia)
    ? market.chance_transferencia
    : null;
  if (chance === null) return null;

  const forcas = market.forcas_transferencia;
  const tomDoTotal = chance >= 60 ? "danger" : chance >= 35 ? "warning" : "success";

  return (
    <div className="rounded-xl bg-[#0f1c2b] px-4 py-3.5" data-testid="driver-detail-transfer-meter">
      <div className="flex items-baseline justify-between gap-3">
        <span className="text-xs font-semibold text-text-secondary">
          {t("driverDetail.market.transferMeter")}
        </span>
        <span
          className="font-mono text-2xl font-semibold leading-none tabular-nums"
          style={{ color: TONE_HEX[tomDoTotal] }}
          data-testid="driver-detail-transfer-chance"
        >
          {chance}%
        </span>
      </div>

      {forcas ? (
        <>
          <div className="mt-2.5 flex h-2 gap-0.5 overflow-hidden rounded-full bg-white/[0.07]">
            {FORCAS.map((chave) =>
              forcas[chave] > 0 ? (
                <div
                  key={chave}
                  data-forca={chave}
                  className="h-full first:rounded-l-full last:rounded-r-full"
                  style={{
                    // Em porcentagem da barra, não da escala 0-100: as parcelas
                    // fecham no total, então a barra cheia É a chance.
                    width: `${(forcas[chave] / chance) * 100}%`,
                    backgroundColor: TONE_HEX[FORCA_TONE[chave]],
                    boxShadow: `0 0 10px ${TONE_HEX[FORCA_TONE[chave]]}59`,
                  }}
                />
              ) : null,
            )}
          </div>

          <div className="mt-2 flex flex-wrap gap-x-4 gap-y-1">
            {FORCAS.map((chave) => (
              <ForcaKey
                key={chave}
                chave={chave}
                valor={forcas[chave]}
                apagada={forcas[chave] === 0}
              />
            ))}
          </div>
        </>
      ) : (
        // Sem decomposição não há barra nem legenda, e o cartão ficaria só com
        // um número solto: aqui a frase é o conteúdo inteiro, não um resumo do
        // que já está desenhado.
        <p className="mt-2 text-xs leading-relaxed text-text-secondary">
          {t(
            temContrato
              ? "driverDetail.market.noMarketSignals"
              : "driverDetail.market.freeAgent",
          )}
        </p>
      )}
    </div>
  );
}

// Duas palavras não ensinam uma mecânica. O rótulo diz de que força se trata; o
// balão diz o que a move — sem ele, "Interesse de fora" com um 5 do lado é um
// número sem pergunta, e quem não conhece o motor por trás fica sem saber se
// aquilo é bom, ruim ou coisa que ele possa mexer.
function ForcaKey({ chave, valor, apagada }) {
  const { t } = useTranslation();
  const cor = TONE_HEX[FORCA_TONE[chave]];
  return (
    <Tooltip texto={t(`driverDetail.market.forceHints.${chave}`)}>
      <span
        className={`flex items-center gap-1.5 text-[11px] ${apagada ? "opacity-40" : ""}`}
        data-forca-key={chave}
      >
        <span className="h-2 w-2 shrink-0 rounded-sm" style={{ backgroundColor: cor }} />
        <span className="text-text-secondary">{t(`driverDetail.market.forces.${chave}`)}</span>
        <span className="font-mono tabular-nums text-text-muted">{valor}</span>
      </span>
    </Tooltip>
  );
}

// Um card só, e não dois.
//
// Eram "Contrato" e "Valor de mercado" lado a lado, e o salário contratado
// aparecia nos dois — três vezes na tela, contando a tabela do gráfico. Sobram
// aqui as duas perguntas que nenhum outro elemento da aba responde: quanto ele
// vale COMPARADO AO PELOTÃO dele, e quanto a equipe paga comparado a isso.
//
// A régua de vigência saiu. Ela desenhava um segmento por temporada, e contrato
// de um ano — o caso mais comum — virava uma barra sólida que não media nada; o
// gráfico logo acima já desenha os anos contratados como traço fantasma, com o
// eixo do tempo junto, que é a mesma informação com mais contexto.
function SituacaoContratual({ contract, market, curva }) {
  const { t } = useTranslation();
  const estimado = Number.isFinite(market.salario_estimado) ? market.salario_estimado : null;
  const pago = contract && Number.isFinite(contract.salario_anual) ? contract.salario_anual : null;
  const selo = seloDeSalario(estimado, pago);
  const prazo = prazoDoContrato(contract, t);

  return (
    <div
      className="mt-3 rounded-xl bg-[#0f1c2b] px-4 py-3.5"
      data-testid="driver-detail-situation"
    >
      <div className="flex items-center justify-between gap-3">
        {contract ? (
          <span className="flex min-w-0 items-center gap-2">
            <TeamLogoMark
              teamName={contract.equipe_nome}
              size="xs"
              halo
              testId="driver-detail-contract-logo"
            />
            <span className="truncate text-xs font-semibold text-[color:var(--team)]">
              {contract.equipe_nome}
            </span>
            {/* O papel como chip, e não como linha de tabela: ele é um atributo
                do vínculo, e ao lado do nome da equipe se lê como um fato só. */}
            <span className="shrink-0 rounded-full bg-white/[0.06] px-1.5 py-0.5 text-[10px] font-semibold text-text-secondary">
              {formatContractRole(contract.papel)}
            </span>
          </span>
        ) : (
          <span className="text-xs font-semibold text-text-secondary">
            {t("driverDetail.market.curve.noContract")}
          </span>
        )}

        {prazo ? (
          // Só o estado. Os ANOS descem para a régua logo abaixo, que diz quais
          // já foram — repeti-los aqui seria a mesma vigência escrita duas vezes
          // a três pixels de distância.
          <span
            className="shrink-0 text-[11px] font-semibold"
            style={{ color: TONE_HEX[prazo.tom] }}
            data-prazo={prazo.chave}
          >
            {prazo.texto}
          </span>
        ) : null}
      </div>

      {/* A régua atravessa o card inteiro: ela é o eixo do tempo dos dois
          blocos abaixo, e não propriedade de um deles. */}
      <ReguaDeContrato contract={contract} tom={prazo?.tom ?? "neutral"} />

      <div className="mt-3.5 grid gap-x-6 gap-y-5 border-t border-white/[0.06] pt-3.5 sm:grid-cols-2">
        <ValorDeMercado market={market} curva={curva} />
        <CustoAnual estimado={estimado} pago={pago} selo={selo} />
      </div>
    </div>
  );
}

// A vigência como régua: um trecho por temporada, cheio no que já foi cumprido
// e tracejado no que ainda não aconteceu, com um traço vertical em cada virada
// de ano.
//
// A versão antiga eram barrinhas soltas sem ano nenhum: davam a proporção e
// nada mais, e quem quisesse saber QUAL temporada estava em jogo tinha que ler
// o período em outro canto e contar. Aqui cada trecho é nomeado, e o tracejado
// diz sozinho o que ainda é promessa — o mesmo vocabulário que o gráfico acima
// usa para os anos já contratados.
function ReguaDeContrato({ contract, tom }) {
  if (!contract) return null;

  const inicio = contract.ano_inicio ?? contract.temporada_inicio;
  const fim = contract.ano_fim ?? contract.temporada_fim;
  if (!Number.isFinite(inicio) || !Number.isFinite(fim)) return null;

  const total = Math.max(1, fim - inicio + 1);
  const restantes = Number.isFinite(contract.anos_restantes)
    ? Math.max(0, Math.min(total, contract.anos_restantes))
    : 0;
  const cumpridas = total - restantes;
  const cor = TONE_HEX[tom];
  const apagado = "rgba(255,255,255,0.18)";

  return (
    <div className="mt-3" data-testid="driver-detail-contract-ruler">
      <div className="flex items-center">
        {Array.from({ length: total }, (_, indice) => {
          const ano = inicio + indice;
          const cumprida = indice < cumpridas;
          return (
            <Fragment key={ano}>
              {indice > 0 ? (
                // A virada de ano. Fica no tom do trecho que COMEÇA nela, para
                // a marca da fronteira não sobreviver ao trecho que já acabou.
                <span
                  aria-hidden="true"
                  className="h-3.5 w-0.5 shrink-0 rounded-full"
                  style={{ backgroundColor: cumprida ? cor : apagado }}
                />
              ) : null}
              <span
                data-temporada={ano}
                data-cumprida={cumprida || undefined}
                className="h-1 min-w-0 flex-1 rounded-full"
                style={
                  cumprida
                    ? { backgroundColor: cor, boxShadow: `0 0 8px ${cor}55` }
                    : {
                        backgroundImage: `repeating-linear-gradient(to right, ${apagado} 0 6px, transparent 6px 12px)`,
                      }
                }
              />
            </Fragment>
          );
        })}
      </div>

      <div className="mt-1.5 flex">
        {Array.from({ length: total }, (_, indice) => {
          const ano = inicio + indice;
          const cumprida = indice < cumpridas;
          return (
            <span
              key={ano}
              className="min-w-0 flex-1 text-center font-mono text-[10px] tabular-nums"
              style={{ color: cumprida ? "#8b949e" : "#6e7681" }}
            >
              {ano}
            </span>
          );
        })}
      </div>
    </div>
  );
}

// O prazo em uma expressão só, com o tom já resolvido — "0 ano" pedia uma conta
// de cabeça para chegar no que importa, que é o contrato acabar AGORA.
function prazoDoContrato(contract, t) {
  const restantes = contract && Number.isFinite(contract.anos_restantes)
    ? contract.anos_restantes
    : null;
  if (restantes === null) return null;
  if (restantes <= 0) {
    return { chave: "agora", tom: "danger", texto: t("driverDetail.market.expiresNow") };
  }
  if (restantes === 1) {
    return { chave: "ultimo", tom: "warning", texto: t("driverDetail.market.lastYear") };
  }
  // "2 anos" solto não diz anos de quê. `expiresValue` serve à frase "Expira em
  // 2 anos", que tem o verbo antes; aqui o rótulo tem que se sustentar sozinho.
  return {
    chave: "longo",
    tom: "success",
    texto: t("driverDetail.market.remainingYears", { count: restantes }),
  };
}

// Quanto ele vale — e onde isso cai no pelotão dele.
//
// O número absoluto não se julgava sozinho: "$23,016" não diz se é o carro mais
// caro do grid ou o mais barato, e sem essa régua o valor era enfeite. A barra é
// a fração do pelotão que está ATRÁS dele, então cheia é o mais caro de todos.
function ValorDeMercado({ market, curva }) {
  const { t } = useTranslation();
  const posicao = Number.isFinite(market.posicao_valor) ? market.posicao_valor : null;
  const total = Number.isFinite(market.total_valor) ? market.total_valor : null;
  const posto =
    posicao !== null && total > 1 && market.categoria_valor
      ? (total - posicao + 1) / total
      : null;
  const tendencia = tendenciaDeValor(curva);
  const cor =
    posto === null
      ? null
      : posto >= 0.75
        ? TONE_HEX.success
        : posto >= 0.4
          ? TONE_HEX.info
          : TONE_HEX.neutral;

  return (
    <div className="text-center" data-bloco="valor">
      <div className="flex items-baseline justify-center gap-2">
        <span className="text-xs font-semibold text-text-secondary">
          {t("driverDetail.market.marketValueLabel")}
        </span>
        {tendencia ? <TendenciaDeValor tendencia={tendencia} /> : null}
      </div>
      <span className="mt-1 block font-mono text-[34px] font-semibold leading-none tabular-nums text-text-primary sm:text-[42px]">
        {formatSalary(market.valor_mercado)}
      </span>

      {posto !== null ? (
        <div className="mx-auto mt-3 w-full max-w-[260px]" data-testid="driver-detail-market-rank">
          <div className="h-1.5 rounded-full bg-white/[0.07]">
            <div
              className="h-full rounded-full"
              data-preenchimento="posto"
              style={{
                width: `${Math.max(3, posto * 100)}%`,
                backgroundColor: cor,
                boxShadow: `0 0 8px ${cor}45`,
              }}
            />
          </div>
          <span className="mt-1.5 block text-[11px] text-text-secondary">
            {t("driverDetail.market.rankInGrid", {
              rank: ordinal(posicao),
              total,
              category: formatCategoryLabel(market.categoria_valor),
            })}
          </span>
        </div>
      ) : null}
    </div>
  );
}

// A variação contra o último ano medido, do MESMO número impresso acima.
//
// Sai de `valor_mercado` ponto a ponto e não do salário estimado: os dois
// divergem quando mídia ou desenvolvimento mudam, e um "+18%" tirado do proxy
// seria uma precisão sobre a coisa errada.
function TendenciaDeValor({ tendencia }) {
  const { t } = useTranslation();
  const subiu = tendencia.variacao > 0;
  const cor = subiu ? TONE_HEX.success : TONE_HEX.danger;

  return (
    <Tooltip
      texto={t("driverDetail.market.trendAgainst", {
        year: tendencia.ano,
        value: formatSalary(tendencia.base),
      })}
    >
      <span
        className="flex shrink-0 items-center gap-1 font-mono text-[11px] font-semibold tabular-nums"
        style={{ color: cor }}
        data-tendencia={subiu ? "alta" : "baixa"}
      >
        <span aria-hidden="true">{subiu ? "▲" : "▼"}</span>
        {`${subiu ? "+" : "-"}${Math.round(Math.abs(tendencia.variacao) * 100)}%`}
      </span>
    </Tooltip>
  );
}

// O que custa, contra o que valeria — duas barras na MESMA escala.
//
// Era uma porcentagem solta ("-44%") pendurada num card cujo número grande é
// outro: parecia descontar do valor de passe quando comparava salários, e a
// direção da conta ficava por conta de quem lia. As barras dizem quem é maior
// sem porcentagem nenhuma, e a frase embaixo diz de que lado o desequilíbrio
// cai. As cores são as MESMAS do gráfico acima de propósito — o card é o último
// ponto daquelas duas linhas.
function CustoAnual({ estimado, pago, selo }) {
  const { t } = useTranslation();
  const maximo = Math.max(estimado ?? 0, pago ?? 0);

  return (
    <div data-bloco="custo">
      <div className="flex items-baseline justify-between gap-2">
        <span className="text-xs font-semibold text-text-secondary">
          {t("driverDetail.market.annualCost")}
        </span>
        {selo ? (
          <span
            className="shrink-0 rounded-full px-2 py-0.5 text-[10px] font-semibold"
            data-selo={selo.chave}
            style={{ color: TONE_HEX[selo.tom], backgroundColor: `${TONE_HEX[selo.tom]}1f` }}
          >
            {t(`driverDetail.market.priceTag.${selo.chave}`)}
          </span>
        ) : null}
      </div>

      <div className="mt-2 space-y-2">
        {pago !== null ? (
          <BarraDeSalario
            chave="pago"
            rotulo={t("driverDetail.market.paidShort")}
            valor={pago}
            maximo={maximo}
            cor={CURVA_PAGO}
          />
        ) : null}
        {estimado !== null ? (
          <BarraDeSalario
            chave="mercado"
            rotulo={t("driverDetail.market.worthShort")}
            valor={estimado}
            maximo={maximo}
            cor={CURVA_MERCADO}
          />
        ) : null}
      </div>

      <p className="mt-2 text-[11px] leading-relaxed text-text-secondary">
        {fraseDoDesequilibrio(selo, estimado, pago, t)}
      </p>
    </div>
  );
}

// A frase diz a conta na direção em que ela é verdadeira.
//
// O selo dizia "Acima do mercado" e o número ao lado dizia "-44%", que é o
// quanto o mercado paga a MENOS — duas leituras da mesma razão, invertidas. Aqui
// cada caso usa a sua: quem paga demais paga X% a mais, quem ganha de menos
// ganha Y% a menos, e X e Y não são o mesmo número.
//
// Quem decide o caso é o SELO, e não um segundo jogo de limiares: dois
// conjuntos de cortes acabariam imprimindo "Acima do mercado" com uma frase
// dizendo que o salário está na faixa.
function fraseDoDesequilibrio(selo, estimado, pago, t) {
  if (!Number.isFinite(estimado) || estimado <= 0) return t("driverDetail.market.noEstimate");
  if (!selo) return t("driverDetail.market.freeAgentCost");

  const razao = pago / estimado;
  if (selo.chave === "inflado") {
    return t("driverDetail.market.overpaid", { value: `${Math.round((razao - 1) * 100)}%` });
  }
  if (selo.chave === "pechincha") {
    return t("driverDetail.market.underpaid", { value: `${Math.round((1 - razao) * 100)}%` });
  }
  return t("driverDetail.market.fairPaid");
}

function BarraDeSalario({ chave, rotulo, valor, maximo, cor }) {
  const largura = maximo > 0 && Number.isFinite(valor) ? Math.max(3, (valor / maximo) * 100) : 0;

  return (
    <div className="flex items-center gap-2" data-barra={chave}>
      <span className="w-[68px] shrink-0 truncate text-[11px] text-text-secondary">{rotulo}</span>
      <span className="h-1.5 min-w-0 flex-1 rounded-full bg-white/[0.07]">
        <span
          className="block h-full rounded-full"
          style={{ width: `${largura}%`, backgroundColor: cor }}
        />
      </span>
      <span className="shrink-0 font-mono text-[11px] tabular-nums text-text-primary">
        {formatSalaryAnnual(valor)}
      </span>
    </div>
  );
}

// Sem contrato não há comparação a fazer — o estimado vira o único número e o
// selo some em vez de dizer "na faixa" contra nada.
//
// A porcentagem que acompanhava o selo saiu: ela era `estimado/pago - 1`, e
// pendurada num card cujo número grande é o valor de passe lia-se como desconto
// sobre ele. Quem imprime a distância agora é a frase, que diz a direção junto
// ([`fraseDoDesequilibrio`]).
function seloDeSalario(estimado, pago) {
  if (!Number.isFinite(estimado) || !Number.isFinite(pago) || pago <= 0) return null;
  const razao = estimado / pago;
  if (razao >= 1.15) return { chave: "pechincha", tom: "success" };
  if (razao <= 0.85) return { chave: "inflado", tom: "warning" };
  return { chave: "faixa", tom: "neutral" };
}
