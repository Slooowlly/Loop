import { useState } from "react";
import { useTranslation } from "react-i18next";

import TeamLogoMark from "../../team/TeamLogoMark";
import { ordinal } from "../../../i18n/format.js";
import { getCategoryColor } from "../../../utils/categoryColors";
import { formatCategoryLabel } from "../detalhes/formatadores.js";
import {
  AlvosDeTemporada,
  CURVA_FITA,
  FechoDoBalao,
  HachuraDefs,
  LinhaDoBalao,
  MINIMO_PARA_O_GRAFICO,
  MolduraDeFundo,
  MolduraDeRodape,
  PonteSobreOVao,
  SerieKey,
  TrocaTooltip,
  ancoraDoBalao,
  caminhoDaFaixa,
  colunasDeCategoria,
  faixasDeDiferenca,
  faixasSemVinculo,
  geometriaDaCurva,
  partirNoPresente,
  partirPorEstilo,
  pontesSemVinculo,
  segmentosContinuos,
  trocasDeEquipe,
  verticesDaSerie,
} from "./curvaDeCarreira.jsx";

// ── Curva de campeonato ──
//
// A carreira lida como POSIÇÃO: onde ele terminou cada temporada. Divide a
// moldura com a curva de mercado da aba Mercado — mesmas colunas de categoria
// ao fundo, mesma fita de equipes no rodapé, mesmas réguas de troca de casa — e
// troca o eixo do dinheiro pelo do campeonato.
//
// As duas séries são a MESMA unidade (posição no campeonato), então dividem um
// eixo só — onde ele terminou contra onde o CARRO daquele ano deveria ter
// terminado. O que se lê não são as linhas, é a distância entre elas: os anos em
// que ele tirou do carro mais do que o carro tinha.
//
// É a mesma estrutura da curva de mercado, e de propósito: lá são salário pago e
// salário justo, aqui são resultado e expectativa. Quem aprendeu a ler uma lê a
// outra sem reaprender nada.
//
// A referência substituiu o "fundo do grid", que era o último lugar de cada ano
// com a área morta abaixo pintada. Aquilo respondia "de quantos carros era este
// campeonato" gastando metade do quadro numa sombra — e o denominador cabe no
// balão ("12º de 18"), enquanto a expectativa não cabe em lugar nenhum senão
// aqui. A expectativa também carrega o tamanho do grid de graça: num campeonato
// de dez carros ela nunca vai apontar P20.
const CURVA_POSICAO = "#f0f6fc";
// Preta — a linha mais escura que o cartão, contra a branca que é a mais clara.
//
// Não é o laranja da curva de mercado, e a diferença não é de gosto. Lá as duas
// linhas são dois FATOS de mesma natureza, e duas cores fortes disputando à
// altura é a leitura certa. Aqui uma é o resultado e a outra é uma REFERÊNCIA, e
// referência que grita vira protagonista — ainda mais sobre colunas de categoria
// que já pintam o fundo com o amarelo da Rookie e o ciano da GT3.
//
// Também não é a cor da equipe do ano, que se tentou e se desfez: pintar a
// referência de vermelho Ferrari e roxo Kitsune punha oito cores saturadas
// atravessando um gráfico de duas linhas, e a que devia recuar virava a mais
// forte do quadro. A identidade da casa já está no chip do rodapé e na coluna
// atrás — o que faltava à linha não era dono, era discrição.
//
// O preto sobre o #0f1c2b do cartão é uma silhueta, e é isso que se quer: some
// quando não se está procurando por ela, e reaparece assim que se olha a
// distância até a branca. O preço é que a legenda precisa de fio de contorno
// para ser achável, porque quadrado preto sobre fundo escuro é buraco.
const CURVA_ESPERADO = "#000000";

// A faixa entre as duas linhas NÃO segue a linha até o preto. Ela é área, não
// traço: preto a 10% sobre um fundo já escuro escurece o que devia destacar, e a
// distância — que é a leitura inteira do gráfico — viraria um buraco no meio do
// quadro. Um neutro claro a 10% continua sendo a mesma marca de sempre.
const CURVA_FAIXA = "#8b949e";
// O título tem cor própria e não é só um ponto maior. Numa carreira de vinte
// temporadas os campeonatos são o que o jogador procura primeiro, e P1 no topo
// do eixo é uma altura, não um fato — o ano em que ele foi vice encosta ali.
const CURVA_TITULO = "#d4a72c";

const LER_POSICAO = (ponto) => ponto.posicao;
const LER_ESPERADO = (ponto) => ponto.esperado;

// Uma cor só para a série inteira; o que varia ano a ano é o TRAÇO.
const ESTILO_DO_CARRO = (ponto) => ({
  cor: CURVA_ESPERADO,
  tracejada: Boolean(ponto.monomarca),
});

export function CurvaDeCampeonato({ pontos, equipeDeEstreia = null }) {
  const { t } = useTranslation();
  const [emFoco, setEmFoco] = useState(null);
  const [trocaEmFoco, setTrocaEmFoco] = useState(null);
  // `null` é "o jogador ainda não escolheu" — e só nesse estado a curva decide
  // sozinha entre desenho e tabela. Uma vez que ele aperta o botão, a escolha
  // dele manda, inclusive ao abrir a ficha do piloto seguinte.
  const [tabela, setTabela] = useState(null);

  // Um ponto só não é trajetória. Duas temporadas é o mínimo para haver subida
  // ou queda a mostrar.
  if (!Array.isArray(pontos) || pontos.length < 2) return null;

  const posicoes = pontos
    .map((ponto) => ponto.posicao)
    .filter((valor) => Number.isFinite(valor) && valor > 0);
  if (!posicoes.length) return null;

  const geo = geometriaDaCurva(pontos);
  const { w, h, padE, padD, padT, alturaPlot, x, xSerie, passo } = geo;

  const escala = escalaDePosicao(pontos);
  // Invertido: P1 no TOPO. É a única orientação que não briga com a leitura —
  // "subiu no campeonato" tem que subir no gráfico, e o número que descreve isso
  // diminui.
  const y = (posicao) => padT + alturaPlot * escala.fracao(posicao);

  const foco = emFoco === null ? null : pontos[emFoco];
  // A temporada em curso é o único ponto parcial da curva: ela ainda está sendo
  // disputada, e é do marcador DELA para a frente que o desenho perde certeza —
  // o traço que sai de um ponto cobre a coluna do próprio ano, e o ano anterior
  // está fechado. `-1` quando não há temporada corrente no eixo (piloto
  // aposentado) ou quando ela é a estreia: sem passado atrás dela não há corte a
  // marcar.
  const indiceAtual = pontos.findIndex((ponto) => ponto.atual);
  const inicioDoFuturo = indiceAtual >= 1 ? indiceAtual : -1;
  // O que rompe o vínculo aqui é a EQUIPE, e não a posição: um piloto pode ter
  // corrido o ano inteiro e não ter classificação arquivada, e isso não é o
  // mesmo que ter passado o ano fora do grid. A hachura nomeia o segundo caso.
  const temEquipeNoAno = (ponto) => Boolean(ponto.equipe_nome);
  const faixas = faixasSemVinculo(pontos, geo, temEquipeNoAno);
  const colunas = colunasDeCategoria(pontos, geo);
  const trocas = trocasDeEquipe(pontos, (ponto) => ponto.posicao);
  const trocaAberta = trocas.find((troca) => troca.indice === trocaEmFoco) ?? null;

  // Abaixo de três temporadas cumpridas o gráfico não tem o que desenhar: dois
  // pontos numa moldura dimensionada para uma carreira inteira leem-se como
  // "faltou informação", quando a informação está toda ali. A tabela diz os
  // mesmos números sem prometer uma trajetória que ainda não existe.
  const anosDePassado = pontos.filter((ponto) => !ponto.atual).length;
  const emTabela = tabela ?? anosDePassado < MINIMO_PARA_O_GRAFICO;
  // Temporada corrida sem classificação no arquivo. Fica fora do desenho — não
  // há altura para o ponto — então precisa ser contada por escrito.
  const semDado = pontos.filter(
    (ponto) => !ponto.atual && ponto.equipe_nome && !Number.isFinite(ponto.posicao),
  ).length;

  return (
    <div
      className="mt-3 rounded-xl bg-[#0f1c2b] px-4 py-3.5"
      data-testid="driver-detail-championship-curve"
    >
      <div className="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1.5">
        <span className="text-xs font-semibold text-text-secondary">
          {t("driverDetail.history.championshipCurve.title")}
        </span>
        <div className="flex items-center gap-3">
          <SerieKey
            cor={CURVA_POSICAO}
            label={t("driverDetail.history.championshipCurve.finish")}
          />
          <SerieKey
            cor={CURVA_ESPERADO}
            label={t("driverDetail.history.championshipCurve.expected")}
          />
          {/* O gráfico nunca é o único caminho para o número: a tabela é o mesmo
              dado sem depender de cor nem de hover. */}
          <button
            type="button"
            onClick={() => setTabela(!emTabela)}
            className="rounded-full border border-white/15 px-2 py-0.5 text-[10px] text-text-secondary transition-colors hover:text-text-primary"
            data-testid="driver-detail-championship-toggle"
          >
            {t(
              emTabela
                ? "driverDetail.market.curve.showChart"
                : "driverDetail.market.curve.showTable",
            )}
          </button>
        </div>
      </div>

      {emTabela ? (
        <CampeonatoEmTabela pontos={pontos} />
      ) : (
        <>
          {/* `relative` porque o balão é HTML posicionado em PORCENTAGEM sobre o
              SVG: o gráfico escala com a largura do modal, e uma posição em
              pixels descolaria do ponto assim que a janela mudasse de tamanho. */}
          <div className="relative mt-2">
            <svg
              viewBox={`0 0 ${w} ${h}`}
              className="w-full"
              role="img"
              aria-label={t("driverDetail.history.championshipCurve.title")}
              onMouseLeave={() => {
                setEmFoco(null);
                setTrocaEmFoco(null);
              }}
            >
              <HachuraDefs />

              <MolduraDeFundo
                geo={geo}
                colunas={colunas}
                faixas={faixas}
                trocas={trocas}
                trocaEmFoco={trocaEmFoco}
                rotuloSemVinculo={t("driverDetail.market.curve.noContract")}
                reguaDoEixo={escala.marcas.map((marca) => (
                  <g key={marca}>
                    <line
                      x1={padE}
                      x2={w - padD}
                      y1={y(marca)}
                      y2={y(marca)}
                      stroke="rgba(255,255,255,0.07)"
                      strokeWidth="1"
                    />
                    <text
                      x={padE - 8}
                      y={y(marca) + 3}
                      textAnchor="end"
                      className="fill-[#6e7681] font-mono text-[9px] tabular-nums"
                    >
                      {ordinal(marca)}
                    </text>
                  </g>
                ))}
              />

              {/* A faixa entre as duas linhas: é ela que carrega a leitura. Some
                  no ano em que falta um dos lados — sem os dois não há distância
                  a preencher. */}
              {faixasDeDiferenca(pontos, LER_ESPERADO, LER_POSICAO).map((faixa) => (
                <path
                  key={`faixa-${faixa.inicio}`}
                  data-faixa="diferenca"
                  d={caminhoDaFaixa(faixa.trecho, geo, y, faixa.inicio, LER_ESPERADO, LER_POSICAO)}
                  fill={CURVA_FAIXA}
                  opacity="0.1"
                />
              ))}

              {/* A expectativa, tracejada onde a categoria é monomarca.

                  Num grid de MX-5 idênticos "o que o carro dava" continua tendo
                  um número — a equipe ainda importa por gente e por dinheiro —
                  mas deixa de medir MÁQUINA, e é máquina o que a palavra "carro"
                  promete. Apagar a linha ali perderia o número; deixá-la cheia
                  afirmaria uma precisão que ela não tem. */}
              {segmentosContinuos(pontos, LER_ESPERADO)
                .flatMap((trecho) => partirPorEstilo(trecho, ESTILO_DO_CARRO))
                .map((trecho) => (
                  <polyline
                    key={`esperado-${trecho.inicio}-${trecho.cor}-${trecho.tracejada}`}
                    data-serie="esperado"
                    data-monomarca={trecho.tracejada ? "" : undefined}
                    points={verticesDaSerie(trecho, geo, y, LER_ESPERADO)}
                    fill="none"
                    stroke={trecho.cor}
                    strokeWidth="2"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeDasharray={trecho.tracejada ? "4 4" : undefined}
                  />
                ))}

              {segmentosContinuos(pontos, LER_POSICAO)
                .flatMap((trecho) => partirNoPresente(trecho, inicioDoFuturo))
                .map((trecho) => (
                  <polyline
                    key={`posicao-${trecho.inicio}-${trecho.futuro}`}
                    data-serie="posicao"
                    data-futuro={trecho.futuro ? "" : undefined}
                    points={verticesDaSerie(trecho, geo, y, LER_POSICAO)}
                    fill="none"
                    stroke={CURVA_POSICAO}
                    strokeWidth="2"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    // Fantasma no trecho que chega à temporada em curso: o
                    // campeonato ainda está sendo disputado, e a posição de hoje
                    // não é onde ele vai terminar. O que muda é a certeza, não a
                    // existência — e certeza se desenha com peso.
                    opacity={trecho.futuro ? 0.4 : 1}
                  />
                ))}

              {/* As pontes que atravessam o ano fora do grid, ligando os pontos
                  das margens.

                  Sem elas a linha simplesmente sumia por um ano e voltava do
                  outro lado, e o vão em branco lia-se como falha de desenho. O
                  tracejado diz o contrário: a carreira continua, o que não
                  houve foi campeonato. Pontilhada só sobre a hachura — ali não
                  houve classificação, e um traço cheio inventaria a posição de
                  cada ponto do caminho.

                  As DUAS séries atravessam. A expectativa não some porque ele
                  ficou um ano sem assento: o carro que ele pegou na volta tinha
                  um lugar no grid, e é a distância entre as duas linhas que este
                  gráfico existe para mostrar. Interromper só a laranja deixava a
                  branca cruzando o vão sozinha, e do outro lado as duas
                  recomeçavam sem que nada dissesse o que houve com a de baixo.

                  A laranja vem antes da branca pela mesma razão que as séries:
                  onde as duas se cruzam, quem fica por cima é o resultado.

                  Vêm depois das séries e antes dos marcadores. */}
              {pontesSemVinculo(faixas, pontos, LER_ESPERADO).map((ponte) => (
                <PonteSobreOVao
                  key={`ponte-esperado-${ponte.de}`}
                  ponte={ponte}
                  x={xSerie}
                  y={y}
                  cor={CURVA_ESPERADO}
                />
              ))}

              {pontesSemVinculo(faixas, pontos, LER_POSICAO).map((ponte) => (
                <PonteSobreOVao
                  key={`ponte-${ponte.de}`}
                  ponte={ponte}
                  x={xSerie}
                  y={y}
                  cor={CURVA_POSICAO}
                />
              ))}

              {/* A régua do presente. Sem ela o traço fantasma seria só uma
                  linha mais fraca sem motivo declarado. */}
              {inicioDoFuturo >= 0 ? (
                <g data-marca="hoje">
                  <line
                    x1={xSerie(inicioDoFuturo)}
                    x2={xSerie(inicioDoFuturo)}
                    y1={padT}
                    y2={padT + alturaPlot}
                    stroke="rgba(255,255,255,0.18)"
                    strokeWidth="1"
                    strokeDasharray="3 3"
                  />
                  <text
                    x={xSerie(inicioDoFuturo) + 5}
                    y={padT + 7}
                    className="fill-[#6e7681] text-[8px] uppercase tracking-[0.12em]"
                  >
                    {t("driverDetail.market.curve.today")}
                  </text>
                </g>
              ) : null}

              {pontos.map((ponto, indice) =>
                Number.isFinite(ponto.esperado) ? (
                  <circle
                    key={`esperado-${ponto.season_number}`}
                    data-marcador="esperado"
                    cx={xSerie(indice)}
                    cy={y(ponto.esperado)}
                    r={indice === emFoco ? 4.5 : 3}
                    fill={CURVA_ESPERADO}
                    // Contorno mais claro que o miolo, e não a cor do cartão
                    // como nos outros marcadores: um disco preto com anel da cor
                    // do fundo é um buraco, e some no meio da própria linha.
                    stroke="#30363d"
                    strokeWidth="2"
                  />
                ) : null,
              )}

              {pontos.map((ponto, indice) =>
                Number.isFinite(ponto.posicao) ? (
                  <g key={ponto.season_number} data-titulo={ponto.titulo ? "" : undefined}>
                    {/* O ano do título ganha um halo antes do marcador: o ponto
                        dourado sozinho se perde no meio de uma linha azul da
                        mesma espessura. */}
                    {ponto.titulo ? (
                      <circle
                        cx={xSerie(indice)}
                        cy={y(ponto.posicao)}
                        r="8"
                        fill={CURVA_TITULO}
                        opacity="0.22"
                      />
                    ) : null}
                    <circle
                      cx={xSerie(indice)}
                      cy={y(ponto.posicao)}
                      // Do mesmo tamanho dos outros. O ponto da temporada em
                      // curso já era vazado, e ainda ganhava metade de raio a
                      // mais — no alto do eixo virava uma bola gigante flutuando
                      // sozinha depois da régua do HOJE, com mais peso visual que
                      // os campeonatos ganhos. É o ponto MENOS confirmado da
                      // curva, e era o maior. O vazado, o traço fantasma que
                      // chega nele e a régua já dizem o que ele é três vezes.
                      r={indice === emFoco ? 4.5 : 3}
                      // Marcador vazado na temporada em curso: mesma posição,
                      // mesmo tamanho, e ainda assim ninguém confunde com
                      // campeonato terminado.
                      fill={
                        ponto.atual ? "#0f1c2b" : ponto.titulo ? CURVA_TITULO : CURVA_POSICAO
                      }
                      stroke={ponto.atual ? CURVA_POSICAO : "#0f1c2b"}
                      strokeWidth="2"
                      data-parcial={ponto.atual ? "" : undefined}
                    />
                  </g>
                ) : null,
              )}

              {/* A posição final escrita em CADA ponto.

                  Era só na última temporada fechada, com o hover respondendo o
                  resto — e hover é uma resposta que exige a pergunta. Numa
                  carreira de vinte anos a curva mostrava a FORMA da trajetória e
                  escondia todos os números dela: dá para ver que 2019 foi pior
                  que 2018 sem descobrir se foi 17º ou 22º, que é a diferença
                  entre um ano ruim e um ano de fundo de grid.

                  A régua ordinal do eixo continua, e não vira redundância: ela é
                  a escala contra a qual a ALTURA se lê, e o rótulo é o valor
                  exato. Sem a régua os rótulos ficariam boiando sem referência
                  de quão longe do topo cada um está. */}
              {pontos.map((ponto, indice) => {
                if (!Number.isFinite(ponto.posicao)) return null;
                const altura = y(ponto.posicao);
                const acima = rotuloVaiAcima(altura, y, ponto, geo);
                return (
                  <text
                    key={`rotulo-${ponto.season_number}`}
                    data-rotulo="posicao"
                    x={xSerie(indice)}
                    // Centrado no ponto, menos no primeiro, que encosta na borda
                    // esquerda do plot: ali o número invadiria a calha do eixo.
                    textAnchor={indice === 0 ? "start" : "middle"}
                    y={acima ? altura - 8 : altura + 14}
                    fill={ponto.titulo ? CURVA_TITULO : CURVA_POSICAO}
                    // A temporada em curso escreve mais fraco, como a linha que
                    // chega nela: o número existe, e não é resultado ainda.
                    opacity={ponto.atual ? 0.55 : 1}
                    // Halo da cor do cartão por baixo do texto. Vinte rótulos
                    // sobre duas linhas e uma coluna colorida cruzam alguma
                    // coisa por definição, e número sobre linha sem separação
                    // fica ilegível nos dois.
                    stroke="#0f1c2b"
                    strokeWidth="3"
                    style={{ paintOrder: "stroke" }}
                    className="font-mono text-[9px] font-semibold tabular-nums"
                  >
                    {ordinal(ponto.posicao)}
                  </text>
                );
              })}

              <MolduraDeRodape
                geo={geo}
                pontos={pontos}
                trocas={trocas}
                emFoco={emFoco}
                setTrocaEmFoco={setTrocaEmFoco}
              />

              <AlvosDeTemporada geo={geo} pontos={pontos} emFoco={emFoco} setEmFoco={setEmFoco} />
            </svg>

            {foco ? (
              <CampeonatoTooltip
                ponto={foco}
                ancora={ancoraDoBalao(
                  [foco.posicao, foco.esperado].filter((valor) => Number.isFinite(valor)).map(y),
                  emFoco,
                  { x: xSerie, w, h },
                )}
              />
            ) : null}

            {trocaAberta ? (
              <TrocaTooltip
                troca={trocaAberta}
                // Preso na fita, e não na altura do ponto: a marca vive no
                // rodapé, e um balão subindo até a curva apontaria para o nada.
                esquerda={`${Math.min(86, Math.max(14, ((x(trocaAberta.indice) - passo / 2) / w) * 100))}%`}
                topo={`${(CURVA_FITA.y / h) * 100}%`}
                formatarValor={(valor) => ordinal(valor)}
                rodape={<DeltaDaTrocaNoCampeonato troca={trocaAberta} />}
              />
            ) : null}
          </div>

          <div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 border-t border-white/[0.06] pt-2 text-[10px] text-text-muted">
            {inicioDoFuturo >= 0 ? (
              <span className="flex items-center gap-1.5" data-chave="parcial">
                <span
                  className="h-0.5 w-3 shrink-0 rounded-full"
                  style={{ backgroundColor: CURVA_POSICAO, opacity: 0.4 }}
                />
                {t("driverDetail.history.championshipCurve.ongoing")}
              </span>
            ) : null}
            {/* A chave do tracejado vive aqui embaixo, junto da do traço
                fantasma: é a linha do cartão que explica PESO de traço. E só
                aparece na carreira que passou por monomarca — legenda de uma
                marca que não está no desenho é ruído. */}
            {pontos.some((ponto) => ponto.monomarca) ? (
              <span className="flex items-center gap-1.5" data-chave="monomarca">
                {/* Sobre uma plaquinha clara: tracejado PRETO sobre o cartão
                    escuro é uma chave que não se vê, e chave que não se vê é
                    pior que chave nenhuma — ocupa a linha e não responde. */}
                <span className="flex h-2.5 w-4 shrink-0 items-center justify-center rounded-sm bg-white/30">
                  <span
                    className="h-0 w-3 border-t border-dashed"
                    style={{ borderColor: CURVA_ESPERADO }}
                  />
                </span>
                {t("driverDetail.history.championshipCurve.sameCar")}
              </span>
            ) : null}
            {/* A equipe de estreia sobreviveu à escada de categorias que este
                gráfico substituiu. Ela não está na fita: o primeiro chip ocupa
                MEIA coluna e pode nem caber a arte, e "por quem ele começou" é
                um fato que a ficha responde por escrito desde sempre. */}
            <span className="ml-auto flex items-center gap-1.5">
              {t("driverDetail.history.debutTeam")}:
              {equipeDeEstreia ? (
                <>
                  <TeamLogoMark
                    teamName={equipeDeEstreia}
                    size="xs"
                    halo
                    testId="driver-detail-debut-team-logo"
                  />
                  <strong className="text-text-secondary">{equipeDeEstreia}</strong>
                </>
              ) : (
                <strong className="text-text-secondary">
                  {t("driverDetail.history.notIdentified")}
                </strong>
              )}
            </span>
          </div>

          {semDado > 0 ? (
            <p className="mt-1 text-[10px] leading-relaxed text-text-muted">
              {t("driverDetail.history.championshipCurve.missing", { count: semDado })}
            </p>
          ) : null}
        </>
      )}
    </div>
  );
}

function CampeonatoTooltip({ ponto, ancora }) {
  const { t } = useTranslation();

  return (
    <div
      // `pointer-events-none` é o que impede o balão de roubar o hover do alvo
      // que o abriu — sem isso ele pisca ao se posicionar sob o cursor.
      className="pointer-events-none absolute z-10 w-max max-w-[240px] rounded-lg border border-white/10 bg-[#0b1622] px-3 py-2 shadow-xl shadow-black/50"
      style={{ left: ancora.esquerda, ...ancora.vertical, transform: ancora.transform }}
      data-testid="driver-detail-championship-tooltip"
    >
      <div className="flex items-center gap-2">
        <TeamLogoMark
          teamName={ponto.equipe_nome}
          size="xs"
          halo
          testId="driver-detail-championship-tooltip-logo"
        />
        <span className="min-w-0 flex-1 truncate text-xs font-semibold text-text-primary">
          {ponto.equipe_nome || t("driverDetail.market.curve.noTeam")}
        </span>
        <span className="shrink-0 font-mono text-[10px] tabular-nums text-text-muted">
          {[ponto.categoria ? formatCategoryLabel(ponto.categoria) : null, ponto.ano]
            .filter(Boolean)
            .join(" · ")}
        </span>
      </div>

      <div className="mt-1.5 space-y-1">
        <LinhaDoBalao
          cor={ponto.titulo ? CURVA_TITULO : CURVA_POSICAO}
          label={t("driverDetail.history.championshipCurve.finish")}
          valor={
            Number.isFinite(ponto.posicao)
              ? // O denominador anda colado no número: "3º" sozinho não diz se
                // foram doze ou trinta carros disputando.
                Number.isFinite(ponto.grid)
                ? t("driverDetail.history.championshipCurve.ofGrid", {
                    position: ordinal(ponto.posicao),
                    count: ponto.grid,
                  })
                : ordinal(ponto.posicao)
              : t("driverDetail.history.championshipCurve.noStandings")
          }
        />
        <LinhaDoBalao
          cor={CURVA_ESPERADO}
          label={t("driverDetail.history.championshipCurve.expected")}
          valor={
            Number.isFinite(ponto.esperado)
              ? ordinal(ponto.esperado)
              : t("driverDetail.history.championshipCurve.noExpected")
          }
        />
        {/* E nada mais. O balão responde à pergunta do gráfico — onde ele
            terminou contra onde o carro terminaria —, e a contagem de corridas,
            vitórias e pódios daquele ano é outra pergunta: ela está na tabela,
            na aba de estatísticas e no bloco de temporada. Três números a mais
            aqui só faziam o leitor procurar qual dos cinco importava. */}
      </div>

      {/* O título ganha o fecho antes da diferença: ser campeão é o fato do ano,
          e "ganhou 2 posições sobre o esperado" é detalhe ao lado disso. */}
      {ponto.titulo ? (
        <FechoDoBalao cor={CURVA_TITULO}>
          {t("driverDetail.history.championshipCurve.champion")}
        </FechoDoBalao>
      ) : ponto.atual ? (
        <FechoDoBalao cor="#8b949e">
          {t("driverDetail.history.championshipCurve.stillRunning")}
        </FechoDoBalao>
      ) : (
        <DeltaDoEsperado ponto={ponto} />
      )}
    </div>
  );
}

// O fecho do balão da temporada: o que ele colocou de si naquele ano.
//
// É a leitura inteira do gráfico dita em uma linha — a mesma distância que a
// faixa desenha, para quem prefere o número. O sinal é INVERTIDO em relação à
// posição: bater a expectativa é o número DIMINUIR.
function DeltaDoEsperado({ ponto }) {
  const { t } = useTranslation();
  if (!Number.isFinite(ponto.posicao) || !Number.isFinite(ponto.esperado)) return null;

  const ganho = ponto.esperado - ponto.posicao;
  if (ganho === 0) {
    return (
      <FechoDoBalao cor="#8b949e">
        {t("driverDetail.history.championshipCurve.onPar")}
      </FechoDoBalao>
    );
  }

  return (
    <FechoDoBalao cor={ganho > 0 ? "#3fb950" : "#f85149"}>
      {t(
        ganho > 0
          ? "driverDetail.history.championshipCurve.above"
          : "driverDetail.history.championshipCurve.below",
        { count: Math.abs(ganho) },
      )}
    </FechoDoBalao>
  );
}

// O fecho do balão da troca: quantas posições ele ganhou ou perdeu ao mudar de
// casa. O sinal é INVERTIDO em relação ao número — subir no campeonato é o
// número diminuir — e é por isso que este cálculo não pode ser o mesmo do
// gráfico de salário.
function DeltaDaTrocaNoCampeonato({ troca }) {
  const { t } = useTranslation();
  if (!Number.isFinite(troca.valorDe) || !Number.isFinite(troca.valorPara)) return null;

  const ganho = troca.valorDe - troca.valorPara;
  if (ganho === 0) {
    return (
      <FechoDoBalao cor="#8b949e">
        {t("driverDetail.history.championshipCurve.sameFinish")}
      </FechoDoBalao>
    );
  }

  return (
    <FechoDoBalao cor={ganho > 0 ? "#3fb950" : "#f85149"}>
      {t(
        ganho > 0
          ? "driverDetail.history.championshipCurve.gained"
          : "driverDetail.history.championshipCurve.lost",
        { count: Math.abs(ganho) },
      )}
    </FechoDoBalao>
  );
}

// O mesmo dado sem cor e sem hover — o caminho de leitura que não depende de
// enxergar onde a linha está no eixo.
function CampeonatoEmTabela({ pontos }) {
  const { t } = useTranslation();

  return (
    <div className="mt-2 max-h-52 overflow-y-auto" data-testid="driver-detail-championship-table">
      <table className="w-full text-[11px]">
        <thead>
          <tr className="text-text-muted">
            <th className="py-1 text-left font-medium">
              {t("driverDetail.market.curve.season")}
            </th>
            <th className="py-1 text-left font-medium">
              {t("driverDetail.market.curve.category")}
            </th>
            <th className="py-1 text-left font-medium">{t("driverDetail.market.curve.team")}</th>
            <th className="py-1 text-right font-medium">
              {t("driverDetail.history.championshipCurve.finish")}
            </th>
            <th className="py-1 text-right font-medium">
              {t("driverDetail.history.championshipCurve.expected")}
            </th>
            <th className="py-1 text-right font-medium">
              {t("driverDetail.history.championshipCurve.tallyHeader")}
            </th>
          </tr>
        </thead>
        <tbody>
          {[...pontos].reverse().map((ponto) => (
            <tr key={ponto.season_number} className="border-t border-white/[0.06]">
              <td className="py-1 font-mono tabular-nums text-text-secondary">{ponto.ano}</td>
              {/* A tabela é o caminho sem cor: aqui a categoria precisa estar
                  escrita, não pintada como na coluna do gráfico. */}
              <td
                className="py-1 text-[10px] uppercase tracking-[0.06em]"
                style={{ color: getCategoryColor(ponto.categoria) }}
              >
                {ponto.categoria ? formatCategoryLabel(ponto.categoria) : "-"}
              </td>
              <td className="py-1 text-text-secondary">
                <span className="flex min-w-0 items-center gap-1.5">
                  <TeamLogoMark
                    teamName={ponto.equipe_nome}
                    size="xs"
                    testId="driver-detail-championship-row-logo"
                  />
                  <span className="min-w-0 truncate">
                    {ponto.equipe_nome || t("driverDetail.market.curve.noTeam")}
                  </span>
                </span>
              </td>
              <td
                className="py-1 text-right font-mono tabular-nums"
                style={{ color: ponto.titulo ? CURVA_TITULO : undefined }}
              >
                {Number.isFinite(ponto.posicao) ? (
                  Number.isFinite(ponto.grid) ? (
                    t("driverDetail.history.championshipCurve.ofGrid", {
                      position: ordinal(ponto.posicao),
                      count: ponto.grid,
                    })
                  ) : (
                    ordinal(ponto.posicao)
                  )
                ) : (
                  <span className="font-sans text-text-muted">
                    {t("driverDetail.history.championshipCurve.noStandings")}
                  </span>
                )}
              </td>
              <td className="py-1 text-right font-mono tabular-nums text-text-secondary">
                {Number.isFinite(ponto.esperado) ? (
                  ordinal(ponto.esperado)
                ) : (
                  <span className="font-sans text-text-muted">
                    {t("driverDetail.history.championshipCurve.noExpected")}
                  </span>
                )}
              </td>
              <td className="py-1 text-right font-mono tabular-nums text-text-secondary">
                {ponto.corridas > 0
                  ? t("driverDetail.history.championshipCurve.tally", {
                      races: ponto.corridas,
                      wins: ponto.vitorias,
                      podiums: ponto.podios,
                    })
                  : "-"}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

// O eixo vai de P1 até a pior das DUAS séries.
//
// Contar a expectativa junto, e não só o resultado, é o que impede a linha
// laranja de sair pelo rodapé nos anos em que ele salvou um carro ruim — que são
// exatamente os anos que o gráfico existe para mostrar.
function escalaDePosicao(pontos) {
  const valores = pontos.flatMap((ponto) =>
    [ponto.posicao, ponto.esperado].filter((valor) => Number.isFinite(valor) && valor > 0),
  );
  const teto = Math.max(...valores, 2);
  // Uma folga de meia posição em cada ponta: sem ela o marcador do campeão fica
  // cortado ao meio pela borda do plot.
  const piso = 0.5;
  const fundo = teto + 0.5;

  return {
    teto,
    marcas: marcasDePosicao(teto),
    fracao: (posicao) => (Math.min(Math.max(posicao, piso), fundo) - piso) / (fundo - piso),
  };
}

// P1 SEMPRE, e depois passos redondos. O primeiro lugar não é uma marca a mais
// da régua: é a referência contra a qual todo o resto do gráfico é lido, e um
// eixo que começa em P5 obriga o leitor a descobrir sozinho onde fica o topo.
function marcasDePosicao(teto) {
  const passo = [1, 2, 5, 10, 20].find((valor) => teto / valor <= 5) ?? 25;
  const marcas = [1];
  for (let valor = passo; valor <= teto; valor += passo) {
    if (valor > 1) marcas.push(valor);
  }
  return marcas;
}

// De que lado do ponto o número se escreve.
//
// Do lado OPOSTO à expectativa, sempre que dá: é ela a outra linha do quadro, e
// é dela que o rótulo se afasta para não cair em cima de um traço. Nos anos em
// que ele ficou abaixo do carro a expectativa passa por cima, e o número desce.
//
// As bordas mandam mais que a preferência. Encostado no teto do eixo o rótulo
// sairia pela borda do cartão — e encostar no teto é justamente o que acontece
// no ano que mais merece o número, o do título.
function rotuloVaiAcima(altura, y, ponto, { padT, alturaPlot }) {
  const alturaEsperada = Number.isFinite(ponto.esperado) ? y(ponto.esperado) : null;
  const acima = alturaEsperada === null || alturaEsperada >= altura;
  if (acima && altura - padT < 12) return false;
  if (!acima && padT + alturaPlot - altura < 16) return true;
  return acima;
}
