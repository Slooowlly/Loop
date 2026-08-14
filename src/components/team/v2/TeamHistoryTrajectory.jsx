import { useCallback, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { Layers, X } from "lucide-react";
import Tooltip from "../../ui/Tooltip";
import { getCategoryColor } from "../../../utils/categoryColors";
import { MEDAL_COLORS, placementInk, placementTone } from "./teamHistoryV2Logic";
import { chaveDaRodada, dicaDeTexto, seasonTooltip } from "./teamHistoryV2Labels";
import { MedalKey, BlockLabel } from "./teamHistoryV2Primitives.jsx";

// Trajetória por temporada do dossiê de equipe v2: a faixa de top 5 ano a ano,
// o balão dela e a fita de forma recente.
//
// Extraída de `TeamHistoryDrawerV2.jsx` em 11/08/2026. Os dois blocos viajam
// juntos porque falam da mesma coisa em escalas diferentes — a faixa agrega o
// histórico, a fita mostra as últimas corridas — e porque o balão da faixa é
// desenho local, sem consumidor fora daqui.

// Altura da área de plotagem da faixa e as marcas do eixo Y, em % do top 5.
// Três marcas: sem a de 50% a barra não tem meio de referência; com mais que
// isso a grade compete com as colunas.
const TRAJECTORY_HEIGHT = 92;
const AXIS_TICKS = [0, 50, 100];

// Janela da faixa: as últimas 15 temporadas do mundo. Um save antigo chega a
// 25+ anos, e desenhar tudo espremia as temporadas recentes — que são as que o
// jogador está olhando — em colunas de poucos pixels. Quem quiser a história
// completa tem a tabela da aba Esportivo, que não recorta nada.
const TRAJECTORY_WINDOW_YEARS = 15;

// Trajetória: uma coluna por temporada, altura = % das corridas terminadas no
// top 5, repartida em 1º, 2º, 3º e 4º-5º.
//
// A escala é POR CORRIDA de propósito. Somar pontos misturava coisas que não se
// comparam — uma temporada de 12 corridas rende mais pontos que uma de 6 sem que
// a campanha tenha sido melhor, e categorias diferentes pontuam diferente. "9 no
// top 5 em 13 corridas" atravessa temporada e categoria sem essa distorção.
//
// O top 5 (e não só o pódio) porque uma equipe de meio de grid vivia como faixa
// vazia: sem pódio, o gráfico não dizia NADA sobre ela. O 4º-5º apagado mostra a
// temporada que quase foi, e é a diferença entre "não competiu" e "faltou pouco".
//
// O eixo vai de 0 a 100%: coluna cheia é top 5 em toda corrida do ano.
//
// A dica da coluna sai estruturada, e não como string de `\n`, porque quem desenha é
// o balão do app (`TrajectoryTooltip`) e não o `title` do sistema — o texto dela é
// montado em [teamHistoryV2Labels.js].

// O balão da coluna, no estilo do app.
//
// O `title` nativo desenhava a caixa BRANCA do Windows no meio de um gráfico
// escuro, com a fonte do sistema e o atraso do sistema — meio segundo em que a
// informação simplesmente não existe. Aqui é a mesma casca dos balões dos
// gráficos de corrida: borda clara, fundo quase preto, blur e sombra.
//
// Vai num portal porque a faixa rola no eixo X (`overflow-x-auto`): um painel
// absoluto dentro dela seria recortado na borda da calha.
const TOOLTIP_MARGEM = 12;
const TOOLTIP_FOLGA = 8;

function TrajectoryTooltip({ rect, dica }) {
  const painelRef = useRef(null);
  const [medida, setMedida] = useState({ width: 0, height: 0 });

  // Mede depois de montar para saber se cabe acima da coluna. Enquanto a medida
  // não existe o painel fica invisível — um quadro pulando de posição no
  // primeiro frame se lê como falha de desenho.
  useLayoutEffect(() => {
    if (!painelRef.current) return;
    const next = { width: painelRef.current.offsetWidth, height: painelRef.current.offsetHeight };
    setMedida((atual) => (atual.width === next.width && atual.height === next.height ? atual : next));
  }, [dica]);

  if (!rect || !dica) return null;

  // Abre para cima por padrão — é de onde a coluna cresce e onde o cursor não
  // está. Vira para baixo quando o topo da janela não dá espaço.
  const cabeAcima = rect.top - medida.height - TOOLTIP_FOLGA >= TOOLTIP_MARGEM;
  const topo = cabeAcima ? rect.top - medida.height - TOOLTIP_FOLGA : rect.bottom + TOOLTIP_FOLGA;
  const esquerda = Math.min(
    Math.max(TOOLTIP_MARGEM, rect.left + rect.width / 2 - medida.width / 2),
    Math.max(TOOLTIP_MARGEM, window.innerWidth - medida.width - TOOLTIP_MARGEM),
  );

  return createPortal(
    <div
      ref={painelRef}
      data-testid="team-history-trajectory-tooltip"
      style={{
        position: "fixed",
        top: topo,
        left: esquerda,
        zIndex: 95,
        opacity: medida.height ? 1 : 0,
      }}
      className="pointer-events-none w-max max-w-[280px] rounded-lg border border-white/15 bg-[#0a0f16]/95 px-3 py-2 text-[11px] shadow-[0_12px_32px_rgba(0,0,0,0.55)] backdrop-blur"
    >
      <span className="block font-semibold leading-tight text-text-primary">{dica.header}</span>
      {dica.meta ? <span className="mt-1 block leading-snug text-text-secondary">{dica.meta}</span> : null}
      {dica.linhas.length ? (
        <ul className="mt-1.5 space-y-1 border-t border-white/[0.08] pt-1.5">
          {dica.linhas.map((linha) => (
            <li
              key={linha.id}
              // Sem o rótulo, a linha é só um número — e número em fonte de
              // número, para as contagens ficarem uma embaixo da outra.
              className={`flex items-center gap-1.5 leading-none ${
                linha.color ? "font-mono text-text-primary" : "text-text-muted"
              }`}
            >
              {linha.color ? (
                // O mesmo quadradinho da legenda embaixo do gráfico, na cor do
                // bloco — é o que liga a linha do balão à fatia da barra.
                <span className="h-2 w-2 shrink-0 rounded-sm" style={{ backgroundColor: linha.color }} />
              ) : null}
              {linha.texto}
            </li>
          ))}
        </ul>
      ) : null}
    </div>,
    document.body,
  );
}

export function SeasonTrajectory({
  seasons,
  worldFirstYear,
  worldLastYear,
  outsideSeasons,
  anoAceso = null,
  onAcenderAno = null,
}) {
  const { t } = useTranslation();
  // A coluna sob o cursor, com o retângulo dela medido na hora do hover: o balão
  // vive num portal e não tem como se posicionar pelo pai.
  const [dicaAberta, setDicaAberta] = useState(null);

  const abrirDica = useCallback(
    (event, bar) => {
      setDicaAberta({ rect: event.currentTarget.getBoundingClientRect(), dica: bar.dica });
      onAcenderAno?.(bar.year);
    },
    [onAcenderAno],
  );
  const fecharDica = useCallback(() => {
    setDicaAberta(null);
    onAcenderAno?.(null);
  }, [onAcenderAno]);

  const bars = useMemo(() => {
    // Anos em que a equipe correu, mas em outra escada de categorias. O dossiê
    // recorta os fatos ao grupo comparável ("Grupo GT3"), então esses anos não
    // chegam em `seasons` — e sem eles a coluna virava "×", afirmando que a
    // equipe não disputou nada num ano em que ela disputou outro campeonato.
    const fora = new Map(
      (Array.isArray(outsideSeasons) ? outsideSeasons : []).map((item) => [Number(item.year), item]),
    );
    const rows = Array.isArray(seasons) ? seasons : [];
    const raced = new Map();
    for (const row of rows) {
      if (Number(row.races) <= 0) continue;
      const races = Number(row.races);
      const wins = Number(row.wins) || 0;
      const seconds = Number(row.seconds) || 0;
      const thirds = Number(row.thirds) || 0;
      const nearMiss = (Number(row.fourths) || 0) + (Number(row.fifths) || 0);
      const topFive = wins + seconds + thirds + nearMiss;
      // De cima para baixo, como um pódio se lê: 1º no alto, o "quase" na base.
      // 4º e 5º entram somados: a pergunta que a faixa responde é quantas vezes
      // a equipe chegou perto de pontuar alto, não em qual das duas casas.
      const steps = [
        { id: "first", count: wins, color: MEDAL_COLORS.first },
        { id: "second", count: seconds, color: MEDAL_COLORS.second },
        { id: "third", count: thirds, color: MEDAL_COLORS.third },
        { id: "nearMiss", count: nearMiss, color: MEDAL_COLORS.nearMiss },
      ].filter((step) => step.count > 0);
      const topFiveRate = (topFive / races) * 100;
      const dnfs = Number(row.dnfs) || 0;
      // O vermelho desce do TETO da coluna, enquanto o top 5 sobe do chão: são
      // as duas pontas do ano, e o meio vazio é o que sobrou — as corridas em
      // que a equipe terminou sem chegar perto.
      //
      // O teto é o espaço livre acima do top 5. A conta do abandono é por CARRO
      // sobre corridas, então em tese ela passa de 100% (dois carros, uma
      // corrida) — e nesse caso o bloco para onde o top 5 começa em vez de
      // invadi-lo. O número exato continua no balão, que é onde ele é lido.
      const dnfRate = Math.min((dnfs / races) * 100, 100 - topFiveRate);
      raced.set(Number(row.year), {
        year: String(row.year),
        raced: true,
        topFiveRate,
        dnfRate,
        steps,
        categoryId: row.categoryId || "",
        categoryLabel: row.category || "",
        dica: seasonTooltip(t, { row, races, topFive, steps, dnfs }),
      });
    }

    if (!raced.size) return [];

    // O eixo é o do MUNDO, não o da equipe, e recortado nas últimas
    // TRAJECTORY_WINDOW_YEARS temporadas. Os anos dentro da janela em que ela não
    // correu viram coluna de ausência, com um "×" no lugar da barra: é o que faz
    // uma equipe de 2024 num mundo de 2012 ocupar o gráfico inteiro e mostrar,
    // de relance, que ela chegou tarde.
    const anos = [...raced.keys()];
    const fim = Math.max(worldLastYear || 0, ...anos);
    // A janela nunca abre antes do primeiro ano do mundo: coluna de ausência num
    // ano em que o campeonato não existia seria ausência de mentira.
    const inicio = Math.max(
      Math.min(worldFirstYear || fim, ...anos),
      fim - TRAJECTORY_WINDOW_YEARS + 1,
    );
    const colunas = [];
    for (let year = inicio; year <= fim; year += 1) {
      const noRecorte = raced.get(year);
      if (noRecorte) {
        colunas.push(noRecorte);
        continue;
      }
      const outra = fora.get(year);
      colunas.push({
        year: String(year),
        raced: false,
        elsewhere: Boolean(outra),
        topFiveRate: 0,
        steps: [],
        categoryId: outra?.categoryId || "",
        categoryLabel: outra?.category || "",
        dica: dicaDeTexto(
          outra
            ? t("myTeamTab.history.records.seasonTooltip.elsewhere", {
                year,
                category: outra.category,
              })
            : t("myTeamTab.history.records.seasonTooltip.absent", { year }),
        ),
      });
    }
    return colunas;
  }, [seasons, t, worldFirstYear, worldLastYear, outsideSeasons]);

  if (!bars.length) return null;

  // A barra diz o quão bem a temporada foi, e não dizia NADA sobre onde ela foi
  // — 40% de top 5 na categoria de entrada e 40% na GT3 são campanhas de peso
  // completamente diferente. A tira colorida sob as colunas carrega essa camada
  // sem gastar altura: mesma paleta de categorias do resto do app, e um degrau
  // na escada aparece como troca de cor no ano exato.
  const categorias = [];
  for (const bar of bars) {
    if (!bar.categoryId || categorias.some((cat) => cat.id === bar.categoryId)) continue;
    categorias.push({ id: bar.categoryId, label: bar.categoryLabel || bar.categoryId });
  }

  return (
    <div className="mt-5">
      {/* O intervalo desenhado fica anunciado ao lado do título: a faixa recorta
          as últimas 15 temporadas, e recorte silencioso se lê como "está tudo
          aqui". */}
      <div className="flex items-baseline gap-2">
        <BlockLabel>{t("myTeamTab.history.records.topFivePerRace")}</BlockLabel>
        <span className="font-mono text-[10px] text-text-muted">
          {bars.length > 1 ? `${bars[0].year}–${bars[bars.length - 1].year}` : bars[0].year}
        </span>
      </div>
      {/* Rola no eixo X em vez de espremer: uma carreira longa tem 40 temporadas,
          e coluna de 3px não é leitura, é ruído. A calha do eixo Y fica FORA da
          área rolável, senão os rótulos 0/50/100% deslizariam junto com as
          barras e deixariam de ser régua. */}
      <div className="mt-2.5 flex gap-2 rounded-xl bg-[#0f1c2b] px-3.5 py-3" data-testid="team-history-trajectory">
        <div className="relative w-7 shrink-0" style={{ height: TRAJECTORY_HEIGHT }}>
          {AXIS_TICKS.map((tick) => (
            <span
              key={tick}
              className="absolute right-0 -translate-y-1/2 font-mono text-[10px] text-text-muted"
              style={{ top: `${100 - tick}%` }}
            >
              {`${tick}%`}
            </span>
          ))}
        </div>
        {/* Rolar com o balão aberto deslocaria a coluna por baixo dele — o
            retângulo foi medido antes do scroll. Fecha, e o próximo hover mede
            de novo. */}
        <div className="relative min-w-0 flex-1 overflow-x-auto" onScroll={fecharDica}>
          {/* Linhas de grade tracejadas atrás das colunas — é o que transforma
              "a barra é alta" em "a barra é 70%". Não capturam o mouse para não
              roubar o tooltip da coluna. */}
          <div className="pointer-events-none absolute inset-x-0 top-0" style={{ height: TRAJECTORY_HEIGHT }}>
            {AXIS_TICKS.map((tick) => (
              <div
                key={tick}
                className="absolute inset-x-0 border-t border-dashed border-white/10"
                style={{ top: `${100 - tick}%` }}
              />
            ))}
          </div>
          {/* Piso de 24px e teto de 64px por coluna. O piso protege a carreira de
              40 temporadas (coluna de 3px não é leitura); o teto protege a de 3,
              em que `flex-1` esticava cada barra para ~290px e ela deixava de
              parecer barra. Sobra vazio à direita — o que é honesto: a equipe tem
              mesmo pouca história. */}
          <div className="relative flex min-w-full items-end gap-1.5" style={{ height: TRAJECTORY_HEIGHT }}>
            {bars.map((bar) => (
              <div
                key={bar.year}
                data-year={bar.year}
                data-absent={bar.raced ? undefined : "true"}
                data-aceso={anoAceso === bar.year ? "true" : undefined}
                // Anel branco fino, e não mudança de cor: as cores da coluna são
                // a informação, e acender apagando-as seria trocar o que se
                // quer ler pelo destaque de onde ler.
                className={`relative h-full min-w-[24px] max-w-[64px] flex-1 rounded-md transition-[box-shadow] ${
                  anoAceso === bar.year ? "ring-1 ring-white/45" : ""
                }`}
                aria-label={bar.dica.texto}
                onMouseEnter={(event) => abrirDica(event, bar)}
                onMouseLeave={fecharDica}
              >
                {/* Trilho: a coluna VAZIA, sempre desenhada, de 0 a 100%.
                    Sem ele, uma temporada sem nenhum top 5 não tinha pixel
                    algum e ficava idêntica a uma temporada que não existiu — que
                    era exatamente o que se via numa equipe de meio de grid. Com
                    o trilho, o ano sempre aparece e o que varia é o quanto dele
                    está preenchido.

                    Ano em que a equipe não correu não ganha trilho: ganha um
                    "×". Trilho vazio quer dizer "correu e não pontuou", e as
                    duas coisas não podem desenhar igual.

                    E há um terceiro estado: correu, mas em outra escada de
                    categorias, fora do recorte comparável deste dossiê. Esse ano
                    ganha o ícone de categorias na cor da escada em que ela
                    estava — a tira colorida embaixo fecha a leitura. Antes ele
                    caía no "×" e o gráfico afirmava que a equipe tinha sumido do
                    mundo. */}
                {bar.raced ? (
                  <div className="absolute inset-0 rounded-md bg-white/[0.045]" />
                ) : bar.elsewhere ? (
                  <div
                    className="absolute inset-0 grid place-items-center rounded-md border border-dashed"
                    style={{
                      borderColor: `color-mix(in srgb, ${getCategoryColor(bar.categoryId)} 30%, transparent)`,
                    }}
                  >
                    <Layers
                      size={14}
                      strokeWidth={1.5}
                      aria-hidden="true"
                      style={{ color: `color-mix(in srgb, ${getCategoryColor(bar.categoryId)} 65%, transparent)` }}
                    />
                  </div>
                ) : (
                  <div className="absolute inset-0 grid place-items-center rounded-md border border-dashed border-white/[0.07]">
                    <X size={14} strokeWidth={1.5} aria-hidden="true" className="text-white/20" />
                  </div>
                )}
                {/* As colocações dividem a barra por `flex-grow`, não por altura
                    em porcentagem: assim preenchem exatamente a altura do top 5
                    mesmo que a contagem por colocação e o total divirjam. O piso
                    de 3px garante que um único 2º lugar numa temporada cheia de
                    4ºs continue visível — "quase invisível" e "não tem" são a
                    mesma coisa para quem olha. */}
                {bar.steps.length ? (
                  <div
                    className="absolute inset-x-0 bottom-0 flex flex-col overflow-hidden rounded-md"
                    style={{ height: `${bar.topFiveRate}%`, minHeight: "4px" }}
                  >
                    {bar.steps.map((step, index) => (
                      <div
                        key={step.id}
                        data-step={step.id}
                        className="w-full"
                        style={{
                          flexGrow: step.count,
                          flexBasis: 0,
                          minHeight: "3px",
                          // Gradiente quase reto: ele dá volume à barra, mas o
                          // que era 28% de escurecimento fazia o pé de um bloco
                          // chegar na cor do topo do bloco de baixo — e era
                          // metade do motivo de não dar para dizer se aquilo era
                          // 2º, 4º ou 5º. Volume é enfeite; a cor é a leitura.
                          backgroundImage: `linear-gradient(180deg, ${step.color}, color-mix(in srgb, ${step.color} 88%, #0b1524))`,
                          // Fio escuro entre os blocos: sem ele, dois vizinhos de
                          // brilho parecido encostam e viram uma faixa só.
                          borderTop: index > 0 ? "1px solid rgba(8,15,25,0.65)" : undefined,
                        }}
                      />
                    ))}
                  </div>
                ) : null}
                {/* Abandonos, pendurados no teto da coluna. Piso de 3px pelo
                    mesmo motivo dos degraus: um abandono solo numa temporada
                    longa vale menos de um pixel, e "quase invisível" se lê como
                    "não teve". */}
                {bar.dnfRate > 0 ? (
                  <div
                    data-dnf={bar.year}
                    className="absolute inset-x-0 top-0 rounded-md"
                    style={{
                      height: `${bar.dnfRate}%`,
                      minHeight: "3px",
                      backgroundImage: `linear-gradient(180deg, ${MEDAL_COLORS.dnf}, color-mix(in srgb, ${MEDAL_COLORS.dnf} 88%, #0b1524))`,
                    }}
                  />
                ) : null}
              </div>
            ))}
          </div>
          {/* Tira de categoria: uma célula por ano, com os MESMOS limites da
              coluna, então ela fica alinhada por construção — sem agrupar anos
              em faixas contínuas, que desalinhavam (uma faixa de N anos come N-1
              gaps, e o espaço livre distribuído pelo flex deixava de bater). Anos
              seguidos na mesma categoria já se leem como um trilho único. */}
          <div className="flex min-w-full gap-1.5" data-testid="team-history-trajectory-categories">
            {bars.map((bar) => (
              <span
                key={bar.year}
                data-category={bar.categoryId || undefined}
                // Sem `title`: a categoria já é o segundo termo do cabeçalho do
                // balão da coluna, e um segundo balão do sistema no mesmo
                // gráfico era a caixa branca de volta, 3px abaixo.
                aria-label={bar.categoryLabel || undefined}
                className="mt-1.5 h-[3px] min-w-[24px] max-w-[64px] flex-1 rounded-full"
                style={{ backgroundColor: bar.categoryId ? getCategoryColor(bar.categoryId) : "transparent" }}
              />
            ))}
          </div>
          {/* Mesmos limites da coluna acima — é o que mantém o ano embaixo da sua
              própria barra. */}
          <div className="flex min-w-full gap-1.5">
            {bars.map((bar) => (
              <span key={bar.year} className="mt-1 min-w-[24px] max-w-[64px] flex-1 text-center font-mono text-[10px] text-text-muted">
                {bar.year}
              </span>
            ))}
          </div>
        </div>
      </div>
      <div className="mt-2 flex flex-wrap items-center gap-3 text-[10px] text-text-muted">
        <MedalKey color={MEDAL_COLORS.first} label={t("myTeamTab.history.records.medals.first")} />
        <MedalKey color={MEDAL_COLORS.second} label={t("myTeamTab.history.records.medals.second")} />
        <MedalKey color={MEDAL_COLORS.third} label={t("myTeamTab.history.records.medals.third")} />
        <MedalKey color={MEDAL_COLORS.nearMiss} label={t("myTeamTab.history.records.medals.nearMiss")} />
        <MedalKey color={MEDAL_COLORS.dnf} label={t("myTeamTab.history.records.medals.dnf")} />
        <span>{t("myTeamTab.history.records.topFivePerRaceLegend")}</span>
      </div>
      {categorias.length > 0 && (
        <div className="mt-1.5 flex flex-wrap items-center gap-3 text-[10px] text-text-muted" data-testid="team-history-trajectory-legend">
          <span className="text-text-muted/80">
            {t("myTeamTab.history.records.categoryBand")}
          </span>
          {categorias.map((cat) => (
            <MedalKey key={cat.id} color={getCategoryColor(cat.id)} label={cat.label} />
          ))}
        </div>
      )}
      <TrajectoryTooltip rect={dicaAberta?.rect} dica={dicaAberta?.dica} />
    </div>
  );
}

// Fita de forma recente: as últimas corridas, da mais antiga à mais nova.
//
// É o único bloco do dossiê que fala do PRESENTE. Todo o resto é história
// agregada, e agregado de 87 corridas não mostra que a equipe subiu de categoria
// no ano passado e não anda mais perto do pódio — que é exatamente a pergunta de
// quem abre o dossiê numa janela de transferências.
export function RecentForm({ races, rodadaAcesa = null, onAcenderRodada = null }) {
  const { t } = useTranslation();
  if (!races?.length) return null;
  const primeira = races[0];
  const ultima = races[races.length - 1];
  // Troca de categoria no meio da fita é a explicação de uma queda que, sem ela,
  // se leria como perda de forma.
  const trocou = primeira.categoryId && ultima.categoryId && primeira.categoryId !== ultima.categoryId;
  return (
    <div>
      <BlockLabel>{t("myTeamTab.history.sport.recentForm")}</BlockLabel>
      <div className="mt-2.5 flex gap-1.5" data-testid="team-history-recent-form">
        {races.map((race, index) => {
          const pos = Number(race.position) || 0;
          const chave = chaveDaRodada(race.year, race.round);
          const aceso = chave != null && chave === rodadaAcesa;
          return (
            <Tooltip
              key={`${race.year}-${race.round}-${index}`}
              texto={
                pos
                  ? t("myTeamTab.history.sport.formTooltip", {
                      year: race.year,
                      round: race.round,
                      category: race.category,
                      position: pos,
                    })
                  : t("myTeamTab.history.sport.formTooltipNoPosition", {
                      year: race.year,
                      round: race.round,
                      category: race.category,
                    })
              }
            >
              <span
                data-position={pos || undefined}
                data-round={chave || undefined}
                data-aceso={aceso ? "true" : undefined}
                onMouseEnter={() => onAcenderRodada?.(chave)}
                onMouseLeave={() => onAcenderRodada?.(null)}
                // Anel branco, igual ao da faixa de top 5: o quadrado já é
                // colorido pela colocação, e trocar a cor apagaria o dado.
                className={`grid h-9 flex-1 place-items-center rounded-md font-mono text-[11px] transition-[box-shadow] ${
                  aceso ? "ring-1 ring-white/60" : ""
                }`}
                style={{ backgroundColor: placementTone(pos || 99), color: placementInk(pos || 99) }}
              >
                {pos || "—"}
              </span>
            </Tooltip>
          );
        })}
      </div>
      <div className="mt-1.5 flex items-center justify-between gap-3 font-mono text-[10px] text-text-muted">
        <span>{t("myTeamTab.history.sport.formRound", { year: primeira.year, round: primeira.round })}</span>
        {trocou ? (
          <span className="truncate font-sans text-text-secondary">
            {t("myTeamTab.history.sport.formMoved", { category: ultima.category })}
          </span>
        ) : null}
        <span>{t("myTeamTab.history.sport.formRound", { year: ultima.year, round: ultima.round })}</span>
      </div>
    </div>
  );
}
