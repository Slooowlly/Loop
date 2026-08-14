import { useId, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { RUN_MODE_POSITION } from "./evolutionPreferences.js";
import { campanhaTemDados } from "./teamHistoryV2Logic";
import { chaveDaRodada } from "./teamHistoryV2Labels";
import { BlockLabel } from "./teamHistoryV2Primitives.jsx";

// Campanha do campeonato: a pontuação ACUMULADA rodada a rodada, da equipe do
// dossiê contra todas as outras do mesmo campeonato.
//
// Separada da curva de posição final em 11/08/2026, quando as duas saíram do
// drawer. Elas respondem perguntas diferentes sobre o mesmo campeonato e
// desenham com eixos OPOSTOS: aqui mais pontos é mais alto, lá P1 é o topo. As
// duas famílias de constantes no mesmo arquivo eram convite para uma ser lida
// como a outra.

// Geometria da campanha do campeonato. O eixo aqui NÃO é invertido: pontos são
// pontos, mais é mais alto, e a linha que sobe é a equipe que está ganhando.
const RUN_WIDTH = 640;
const RUN_HEIGHT = 186;
const RUN_LEFT = 46;

// A direita para antes da borda porque a etiqueta da equipe fica FORA do
// desenho, no fim da linha — dentro, ela cobriria justamente o trecho decisivo.
const RUN_RIGHT = 596;
const RUN_TOP = 16;
const RUN_BOTTOM = 132;
const RUN_AXIS = 146;
const RUN_ROUND_Y = 166;
const RUN_SURFACE = "#0f1c2b";

// Cinza das outras equipes. Elas precisam existir — sem o campo, a linha da
// equipe é só um traço subindo, e subir é o que toda linha acumulada faz — mas
// não podem competir: um fio fino, sem marcador e sem etiqueta.
const RUN_FIELD_STROKE = "#3a4d63";

// Campanha do campeonato: a pontuação ACUMULADA rodada a rodada, da equipe do
// dossiê contra todas as outras do mesmo campeonato.
//
// Substitui a curva de posição final por temporada no arranjo arrumado, e a
// diferença é a pergunta: a curva dizia ONDE a equipe terminou cada ano, esta
// diz COMO a temporada mais recente foi disputada. Vinte pontos abertos na
// primeira metade e defendidos até o fim, ou uma virada na última rodada,
// terminam ambos em "P1" — e desenham completamente diferente.
//
// É também o bloco que conversa com a fita de forma recente logo abaixo: as
// mesmas corridas, agora somadas contra quem estava de fato na pista.
export function ChampionshipRun({
  run,
  seletor = null,
  seletorModo = null,
  modo = RUN_MODE_POSITION,
  rodadaAcesa = null,
  onAcenderRodada = null,
}) {
  const { t } = useTranslation();
  const uid = useId().replace(/:/g, "");
  // O modo é o que o eixo MEDE, e é a diferença entre um gráfico com nuance e um
  // feixe de retas paralelas. Ele vem de fora porque é a métrica do BLOCO, não
  // deste gráfico: a curva entre campeonatos mede as mesmas duas coisas, e a
  // escolha atravessa a troca de vista.
  //
  // Em pontos acumulados todo mundo sobe — é o que acumulado faz — e a subida
  // comum a todas as linhas domina o desenho. Pior é o alcance: o eixo tem de
  // caber o líder, então numa temporada de 243 contra 74 o pelotão inteiro se
  // espreme em menos de um terço da altura e as diferenças que decidem o
  // campeonato viram espessura de traço.
  //
  // Descontar o líder não resolve: subtrair a linha dele é uma transformação
  // afim, tira a inclinação comum e mantém a compressão intacta.
  //
  // A COLOCAÇÃO resolve, porque troca o espaço. Em pontos, um líder disparado
  // come o eixo sozinho; em colocação, cada equipe ocupa exatamente uma faixa da
  // altura, por construção — nenhum outlier pode espremer ninguém. E é onde a
  // temporada acontece: as linhas se cruzam quando uma equipe passa a outra, que
  // é o evento que o acumulado esconde atrás de dois traços quase paralelos.
  const dados = useMemo(() => {
    if (!campanhaTemDados(run)) return null;
    const rounds = run.rounds;
    const lines = run.lines;
    const porPosicao = modo === RUN_MODE_POSITION;
    const pontosEm = (line, index) => Number(line.points?.[index] ?? 0);
    // A classificação RODADA A RODADA, refeita a cada uma: é a colocação de
    // então, não a do fim. Empate cai para o id — arbitrário, mas estável, para
    // a linha não trocar de faixa sozinha entre aberturas da mesma tela.
    const classificacao = rounds.map((_, index) => {
      const ordem = [...lines].sort(
        (a, b) => pontosEm(b, index) - pontosEm(a, index) || a.teamId.localeCompare(b.teamId),
      );
      return new Map(ordem.map((line, posicao) => [line.teamId, posicao + 1]));
    });
    const valor = (line, index) =>
      porPosicao ? classificacao[index].get(line.teamId) : pontosEm(line, index);

    // Em colocação o eixo é fixo: P1 no topo, a última do grid embaixo. Em
    // pontos vai de zero ao líder.
    const alto = porPosicao ? 1 : Math.max(1, ...lines.map((line) => pontosEm(line, rounds.length - 1)));
    const baixo = porPosicao ? Math.max(2, lines.length) : 0;
    const x = (index) => RUN_LEFT + ((RUN_RIGHT - RUN_LEFT) * index) / (rounds.length - 1);
    const y = (v) => RUN_BOTTOM - ((RUN_BOTTOM - RUN_TOP) * (v - baixo)) / (alto - baixo);
    const traco = (line) =>
      rounds
        .map((_, index) => `${index ? "L" : "M"} ${x(index).toFixed(1)},${y(valor(line, index)).toFixed(1)}`)
        .join(" ");
    const selecionada = lines.find((line) => line.selected) ?? null;
    // Uma etiqueta a cada N rodadas quando elas não cabem. Abaixo de 34px os
    // rótulos "R10" começam a se encostar e a régua vira tarja.
    const passo = (RUN_RIGHT - RUN_LEFT) / (rounds.length - 1);
    const saltoRotulo = Math.max(1, Math.ceil(34 / Math.max(passo, 1)));
    return {
      rounds,
      porPosicao,
      alto,
      baixo,
      x,
      y,
      traco,
      selecionada,
      outras: lines.filter((line) => !line.selected),
      saltoRotulo,
      pontosSelecionada: selecionada
        ? rounds.map((_, index) => {
            const v = valor(selecionada, index);
            return { index, valor: v, cx: x(index), cy: y(v) };
          })
        : [],
    };
  }, [run, modo]);

  if (!dados) return null;
  const areaId = `run-area-${uid}`;
  const glowId = `run-glow-${uid}`;
  const { selecionada } = dados;
  // Três níveis só. Uma linha por colocação, num grid de dez, faria uma malha
  // que disputa atenção com as próprias linhas.
  const ticks = [...new Set([dados.alto, Math.round((dados.alto + dados.baixo) / 2), dados.baixo])];
  const rotuloTick = (tick) => (dados.porPosicao ? `P${tick}` : `${tick}`);
  const ultima = dados.pontosSelecionada[dados.pontosSelecionada.length - 1];
  // A rodada acesa, traduzida para índice do eixo. Rodada de OUTRO ano (a fita
  // recente atravessa temporadas) simplesmente não acha índice aqui — e não
  // acender é a resposta certa: aquela corrida não está neste gráfico.
  const indiceAceso = rodadaAcesa
    ? dados.rounds.findIndex((round) => chaveDaRodada(run.year, round) === rodadaAcesa)
    : -1;
  const pontoAceso = indiceAceso >= 0 ? dados.pontosSelecionada[indiceAceso] : null;
  // Meia distância entre rodadas: é a largura da faixa invisível que captura o
  // mouse. Menos que isso deixaria vãos mortos entre as rodadas.
  const meiaFaixa = dados.rounds.length > 1 ? (RUN_RIGHT - RUN_LEFT) / (dados.rounds.length - 1) / 2 : 12;
  // A mancha desce até a base do eixo nos dois modos — é o corpo da linha, não
  // uma medida por si.
  const baseArea = dados.y(dados.baixo);

  return (
    <div>
      <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1.5">
        <div className="flex flex-wrap items-center gap-x-2 gap-y-1.5">
          <BlockLabel>{t("myTeamTab.history.sport.championshipTitle")}</BlockLabel>
          {seletor}
          <span className="font-mono text-[10px] text-text-muted">
            {t("myTeamTab.history.sport.runScope", { year: run.year, category: run.category })}
          </span>
          {run.live ? (
            <span className="rounded-full bg-white/[0.07] px-2 py-0.5 text-[10px] font-semibold text-text-muted">
              {t("myTeamTab.history.sport.runLive")}
            </span>
          ) : null}
        </div>
        <div className="flex items-center gap-2">
          {/* Os dois modos ficam à vista, e não num menu: o eixo muda de
              significado entre eles, e um gráfico que mede outra coisa sem
              anunciar é um gráfico que mente. */}
          {seletorModo}
          {/* A colocação vira pílula na cor da equipe: é o veredito do gráfico, e
              procurá-lo contando linhas de cima para baixo seria trabalho. */}
          {selecionada ? (
            <span
              data-testid="team-history-run-position"
              className="flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[10px] font-semibold uppercase tracking-[0.12em]"
              style={{
                borderColor: "color-mix(in srgb, var(--team) 45%, transparent)",
                backgroundColor: "color-mix(in srgb, var(--team) 10%, transparent)",
                color: "var(--team)",
              }}
            >
              {t("myTeamTab.history.sport.runStanding", {
                position: selecionada.position,
                points: selecionada.total,
              })}
            </span>
          ) : null}
        </div>
      </div>

      <div className="mt-2.5 rounded-xl border border-white/[0.06] bg-[#0f1c2b] px-3 py-3 shadow-[inset_0_1px_0_rgba(255,255,255,0.04)]">
        <svg
          viewBox={`0 0 ${RUN_WIDTH} ${RUN_HEIGHT}`}
          className="h-auto w-full"
          data-testid="team-history-championship-run"
        >
          <defs>
            <linearGradient id={areaId} x1="0" y1={RUN_TOP} x2="0" y2={RUN_BOTTOM} gradientUnits="userSpaceOnUse">
              <stop offset="0%" stopColor="var(--team)" stopOpacity="0.34" />
              <stop offset="100%" stopColor="var(--team)" stopOpacity="0.01" />
            </linearGradient>
            <filter id={glowId} x="-12%" y="-40%" width="124%" height="180%">
              <feGaussianBlur stdDeviation="3.2" result="borrao" />
              <feMerge>
                <feMergeNode in="borrao" />
                <feMergeNode in="SourceGraphic" />
              </feMerge>
            </filter>
          </defs>

          {/* Grade: só três níveis. Uma linha por rodada faria uma malha que
              disputa atenção com as vinte linhas do campo. */}
          {ticks.map((tick, index) => (
            <g key={`tick-${index}`}>
              <line
                x1={RUN_LEFT}
                y1={dados.y(tick)}
                x2={RUN_RIGHT}
                y2={dados.y(tick)}
                stroke="#ffffff"
                strokeOpacity={index === 0 ? 0.1 : 0.05}
                strokeDasharray={index === 0 ? undefined : "2 5"}
              />
              <text x={RUN_LEFT - 8} y={dados.y(tick) + 3.4} textAnchor="end" fontSize="10" fill="#66788d">
                {rotuloTick(tick)}
              </text>
            </g>
          ))}
          <text x={RUN_LEFT - 8} y={RUN_TOP - 5} textAnchor="end" fontSize="8.5" fill="#4d5f74" letterSpacing="0.08em">
            {t(dados.porPosicao ? "myTeamTab.history.sport.runPositionAxis" : "myTeamTab.history.sport.runPointsAxis")}
          </text>

          {/* Em colocação, o topo do eixo é a liderança do campeonato. Marcá-lo
              dá o que ler contra: uma linha que encosta ali está brigando pelo
              título, e sem a marca P1 é só mais um valor da escala. */}
          {dados.porPosicao ? (
            <>
              <line
                x1={RUN_LEFT}
                y1={dados.y(1)}
                x2={RUN_RIGHT}
                y2={dados.y(1)}
                stroke="#e2c96a"
                strokeOpacity="0.4"
                strokeDasharray="5 4"
              />
              <text x={RUN_RIGHT - 2} y={dados.y(1) - 5} textAnchor="end" fontSize="8.5" fill="#e2c96a" fillOpacity="0.7" letterSpacing="0.1em">
                {t("myTeamTab.history.sport.runLeaderRef")}
              </text>
            </>
          ) : null}

          <line x1={RUN_LEFT} y1={RUN_TOP - 6} x2={RUN_LEFT} y2={RUN_AXIS} stroke="#ffffff" strokeOpacity="0.12" />
          <line x1={RUN_LEFT} y1={RUN_AXIS} x2={RUN_WIDTH - 8} y2={RUN_AXIS} stroke="#ffffff" strokeOpacity="0.08" />

          {/* Faixas invisíveis, uma por rodada, para o mouse ter onde pousar: os
              únicos alvos do gráfico eram as linhas, e mirar um traço de 1px
              para achar uma rodada não é um alvo.

              Vêm ANTES das linhas de propósito. O que é pintado depois fica por
              cima e continua capturando o mouse — então a linha do campo mantém
              o balão dela, e a faixa pega todo o resto da coluna. */}
          {onAcenderRodada
            ? dados.rounds.map((round, index) => (
                <rect
                  key={`faixa-${round}`}
                  data-round-band={chaveDaRodada(run.year, round) || undefined}
                  x={dados.x(index) - meiaFaixa}
                  y={RUN_TOP - 6}
                  width={meiaFaixa * 2}
                  height={RUN_AXIS - (RUN_TOP - 6)}
                  fill="transparent"
                  onMouseEnter={() => onAcenderRodada(chaveDaRodada(run.year, round))}
                  onMouseLeave={() => onAcenderRodada(null)}
                />
              ))
            : null}

          {/* O campo primeiro, para a linha da equipe passar POR CIMA de todas
              elas — é a única que não pode ser cruzada e escondida. */}
          {dados.outras.map((line) => (
            <path
              key={line.teamId}
              data-line={line.teamId}
              d={dados.traco(line)}
              fill="none"
              stroke={RUN_FIELD_STROKE}
              strokeWidth="1"
              strokeOpacity="0.7"
              strokeLinejoin="round"
            >
              <title>
                {t("myTeamTab.history.sport.runTooltip", {
                  team: line.team,
                  position: line.position,
                  points: line.total,
                })}
              </title>
            </path>
          ))}

          {selecionada ? (
            <>
              <path
                d={`${dados.traco(selecionada)} L ${RUN_RIGHT},${baseArea} L ${RUN_LEFT},${baseArea} Z`}
                fill={`url(#${areaId})`}
              />
              <path
                data-line={selecionada.teamId}
                data-selected="true"
                d={dados.traco(selecionada)}
                fill="none"
                stroke="var(--team)"
                strokeWidth="2.6"
                strokeLinecap="round"
                strokeLinejoin="round"
                filter={`url(#${glowId})`}
              >
                <title>
                  {t("myTeamTab.history.sport.runTooltip", {
                    team: selecionada.team,
                    position: selecionada.position,
                    points: selecionada.total,
                  })}
                </title>
              </path>
              {/* Marcadores só na equipe do dossiê, e vazados na cor do cartão
                  para a linha não passar por dentro deles. */}
              {dados.pontosSelecionada.map((ponto) => (
                <circle
                  key={`ponto-${ponto.index}`}
                  cx={ponto.cx}
                  cy={ponto.cy}
                  r="2.6"
                  fill={RUN_SURFACE}
                  stroke="var(--team)"
                  strokeWidth="1.8"
                />
              ))}
              {ultima ? (
                <g>
                  <circle cx={ultima.cx} cy={ultima.cy} r="4.4" fill="var(--team)" filter={`url(#${glowId})`} />
                  <text
                    x={ultima.cx + 10}
                    y={ultima.cy + 3.6}
                    fontSize="11"
                    fontWeight="700"
                    fill="var(--team)"
                  >
                    {/* A etiqueta mostra o que o EIXO mede. Repetir o total em
                        pontos com a linha desenhada em colocação seria um número
                        que não bate com a altura em que ele está. */}
                    {dados.porPosicao ? `P${ultima.valor}` : selecionada.total}
                  </text>
                </g>
              ) : null}
            </>
          ) : null}

          {/* A rodada acesa: um fio vertical atravessando o desenho e, sobre a
              linha da equipe, o marcador cheio. O fio é quem faz a ponte com a
              fita lá embaixo — ele diz ONDE no eixo aquela corrida caiu, que é
              o que uma fita de quadradinhos não tem como dizer.

              Vem depois das linhas para não ficar por baixo do campo, e é
              `pointer-events-none` para não roubar o mouse das faixas. */}
          {indiceAceso >= 0 ? (
            <g data-testid="team-history-run-round-mark" className="pointer-events-none">
              <line
                x1={dados.x(indiceAceso)}
                y1={RUN_TOP - 6}
                x2={dados.x(indiceAceso)}
                y2={RUN_AXIS}
                stroke="#ffffff"
                strokeOpacity="0.28"
                strokeDasharray="3 3"
              />
              {pontoAceso ? (
                <circle cx={pontoAceso.cx} cy={pontoAceso.cy} r="4.2" fill="var(--team)" />
              ) : null}
            </g>
          ) : null}

          {/* Régua de rodadas embaixo da linha do eixo: acima é pontuação,
              abaixo é quando. */}
          {dados.rounds.map((round, index) =>
            // A régua é esparsa quando as rodadas não cabem — e a acesa aparece
            // mesmo fora do salto: perguntaram por ela, esconder o número dela
            // seria acender sem responder.
            index % dados.saltoRotulo === 0 || index === dados.rounds.length - 1 || index === indiceAceso ? (
              <text
                key={`rodada-${round}`}
                x={dados.x(index)}
                y={RUN_ROUND_Y}
                textAnchor="middle"
                fontSize="9.5"
                fill={index === indiceAceso ? "#e3ebf3" : "#66788d"}
                fontWeight={index === indiceAceso ? "700" : undefined}
              >
                {t("myTeamTab.history.sport.runRound", { value: round })}
              </text>
            ) : null,
          )}
        </svg>
      </div>
      <p className="mt-1.5 text-[10px] text-text-muted">
        {t(
          dados.porPosicao
            ? "myTeamTab.history.sport.runFieldNotePosition"
            : "myTeamTab.history.sport.runFieldNote",
          { value: dados.outras.length },
        )}
      </p>
    </div>
  );
}
