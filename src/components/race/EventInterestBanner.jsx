import { useTranslation } from "react-i18next";

import Tooltip from "../ui/Tooltip";
import { formatAudience } from "./raceEventContext";

// F-07 — a exibição rica do interesse ESPERADO do evento.
//
// O `event_interest/` modula economia, narrativa e motivação, e até a versão em
// card o jogador via dele um número solto no canto do card de clima: "Público
// 62.000", sem tier, sem escala e sem dizer o que aquilo tem a ver com ele.
// Sentia-se o efeito (o patrocínio rendeu mais, a notícia veio mais forte) sem
// nunca ver a causa.
//
// Ele responde três perguntas, na ordem em que elas importam:
//
//   1. Que tamanho tem esta etapa? — `tier_label`, já traduzido pelo backend, com o
//      público como número grande.
//   2. Onde ela cai no calendário? — o rótulo de porte da ocasião.
//   3. O que isso tem a ver comigo? — a fração do público que a equipe do jogador
//      puxa (`public_fame_share`), que é o fio entre a fama dele e a bilheteria.
//
// Por que faixa centrada no cabeçalho, e não card na coluna 1 (mudança de
// 14/08/2026): como card ele consumia uma caixa inteira da coluna de condições
// para quatro linhas de texto, empurrando risco de quebra e narrativa para baixo
// da dobra. O cabeçalho já era duas ilhas com um vão no meio, e o interesse é
// identificação da etapa pelo mesmo critério que o nome da pista e a data: fala
// do evento, não de uma decisão do jogador. Sem moldura ele lê como legenda do
// cabeçalho, que é o papel dele.
//
// TODO número aqui vem do backend. A única exceção é herdada e está anotada como
// dívida no próprio `raceEventContext.js`: `estimateAudience` inventa o público
// quando o save é antigo e não tem `display_value`. Não amplie o padrão.
//
// A barra do público existe porque um número absoluto não se julga sozinho: 41.000
// é muito ou pouco? Ela mede contra o topo da escala de tiers (`EventoPrincipal`),
// que é o mesmo teto que o backend usa para classificar — não é uma régua nova.
const TETO_DE_PUBLICO = 90000;

function EventInterestBanner({ interestLabel, audienceEstimate, audienceRankLabel, fameSharePct }) {
  const { t } = useTranslation();
  // Sem público não há evento a descrever: save antigo sem `event_interest` cai aqui
  // e a faixa simplesmente não existe, em vez de desenhar uma barra em zero que se
  // leria como "ninguém vai". No cabeçalho isso importa mais que no card: o vão do
  // meio volta a ser vão, e as duas ilhas seguem nas bordas.
  if (!audienceEstimate) return null;

  const fracao = Math.max(0, Math.min(100, (audienceEstimate / TETO_DE_PUBLICO) * 100));

  // O porte da ocasião e a cota da estrela saíram do layout e viraram balão de
  // hover em 14/08/2026. Eles são a leitura de segundo nível: respondem "e daí?"
  // depois que o jogador já leu o tier e o número, e ocupavam duas linhas de texto
  // de 11px que puxavam o peso do bloco para baixo, num cabeçalho que precisa ser
  // varrido de relance.
  //
  // Vai como `texto` e não como nó montado de propósito: string dá ao `Tooltip` o
  // `data-tooltip` no gatilho, que é alça estática para teste e para o inspetor,
  // sem prender o balão nem esperar os 400ms de atraso dele.
  const detalhe = [
    audienceRankLabel,
    fameSharePct ? t("nextRaceTab.eventInterest.fameShare", { percent: fameSharePct }) : "",
  ]
    .filter(Boolean)
    .join(". ");

  return (
    <div
      data-testid="event-interest-banner"
      className="mt-6 flex w-full min-w-0 justify-center md:mt-0 md:w-auto md:flex-1 md:px-8"
    >
      {/* MEDIDA ÚNICA. Sem a moldura do card, o que dá forma ao bloco é a largura
          das linhas, e ela estava em quatro valores diferentes: o rótulo curto, a
          linha grande média, a barra em 16rem e o rodapé transbordando os três. O
          contorno resultante não fechava, e era isso que lia como texto solto no
          meio da tela. Agora as três linhas dividem a mesma coluna de 26rem e a
          barra vai de borda a borda dela. */}
      {/* `isolate` fecha o contexto de empilhamento aqui: sem ele o `-z-10` do brilho
          escapa para o contexto da aba e some atrás da arte de fundo da tela. */}
      {/* O alvo do balão é o miolo de 26rem, e não a caixa `flex-1` que o contém:
          esta última ocupa o vão inteiro do cabeçalho, e o hover abriria a leitura
          com o cursor parado no vazio, longe do que ela explica. */}
      <Tooltip texto={detalhe} lado="baixo">
        {/* O `cursor-help` só existe quando existe balão: num save sem porte e sem
            cota o `Tooltip` devolve o filho cru, e o cursor ficaria prometendo uma
            leitura que nunca abre. */}
        <div
          className={`group relative isolate w-full max-w-[26rem] px-6 py-1 text-center ${
            detalhe ? "cursor-help" : ""
          }`}
        >
          {/* O assentamento de luz: sem caixa, sem borda e sem custo de contraste,
              só o mesmo dourado do rótulo e da barra soprado por trás. Ele diz
              "esta região é uma coisa só", que é o trabalho da moldura. */}
          <div
            aria-hidden="true"
            className="pointer-events-none absolute inset-0 -z-10 bg-[radial-gradient(ellipse_at_center,rgba(245,199,109,0.07),transparent_72%)]"
          />

          {/* Filetes laterais: separam do nome da pista e dos botões sem desenhar
              caixa. Desbotam nas pontas para não virar duas bordas de tabela. */}
          <div
            aria-hidden="true"
            className="pointer-events-none absolute inset-y-2 left-0 hidden w-px bg-gradient-to-b from-transparent via-white/10 to-transparent md:block"
          />
          <div
            aria-hidden="true"
            className="pointer-events-none absolute inset-y-2 right-0 hidden w-px bg-gradient-to-b from-transparent via-white/10 to-transparent md:block"
          />

          <p className="text-[10px] font-bold uppercase tracking-widest text-[#f5c76d]">
            {t("nextRaceTab.eventInterest.label")}
          </p>

          {/* Quebra em vez de `truncate`: os `tier_label` de hoje cabem com folga
              (o mais longo é "Interesse moderado", 192px em 368 disponíveis), e o
              corte silencioso só apareceria num rótulo futuro, comendo letras sem
              ninguém notar. Quebrando, um rótulo comprido custa uma linha e
              continua legível. */}
          <p className="mt-1 flex max-w-full flex-wrap items-baseline justify-center gap-x-2">
            <span className="text-xl font-bold text-white">{interestLabel}</span>
            <span className="text-white/20">•</span>
            <span
              className="text-xl font-bold text-white"
              style={{ fontVariantNumeric: "tabular-nums" }}
            >
              {formatAudience(audienceEstimate)}
            </span>
            <span className="text-[10px] font-bold uppercase tracking-widest text-gray-500">
              {t("nextRaceTab.labels.audience")}
            </span>
          </p>

          <div className="mt-2.5 h-1 w-full overflow-hidden rounded-full bg-white/[0.08]">
            {/* O brilho na ponta preenchida: numa etapa de público baixo a barra é
                um toco de 19% numa pista larga, e sem ele o olho lê falha de
                desenho em vez de medida. Ele acende no hover junto do
                `cursor-help`, que é o aviso de que existe leitura escondida ali. */}
            <div
              className="h-full rounded-full bg-[#f5c76d] shadow-[0_0_10px_rgba(245,199,109,0.45)] transition-shadow group-hover:shadow-[0_0_14px_rgba(245,199,109,0.75)]"
              style={{ width: `${fracao}%` }}
              data-testid="event-interest-bar"
            />
          </div>
        </div>
      </Tooltip>
    </div>
  );
}

export default EventInterestBanner;
