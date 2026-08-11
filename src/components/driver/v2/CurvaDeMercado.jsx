import { useState } from "react";
import { useTranslation } from "react-i18next";

import TeamLogoMark from "../../team/TeamLogoMark";
import { getCategoryColor } from "../../../utils/categoryColors";
import {
  formatMoneyCompact,
  formatSalary,
  formatSignedMoney,
} from "../../../utils/formatters";
import { formatCategoryLabel } from "../detalhes/formatadores.js";
import { TONE_HEX, escalaLog } from "./driverDetailV2Logic";
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
  pontesSemVinculo,
  segmentosContinuos,
  trocasDeEquipe,
  verticesDaSerie,
} from "./curvaDeCarreira.jsx";

// ── Curva de mercado ──
//
// As duas séries são a MESMA unidade (dólar por ano), então dividem um eixo só —
// salário contratado contra o que o modelo diz que ele valia. O que se lê não são
// as linhas, é a distância entre elas: os anos em que ele correu por menos do que
// valia. O selo "Pechincha" do card ao lado é o último ponto deste gráfico.
//
// Saiu de `DriverDetailModalV2.jsx` para cá em 11/08/2026, pelo mesmo corte que
// já tinha posto a curva de campeonato num arquivo próprio: são ~600 linhas de
// SVG que só o cartão de mercado usa, e o par irmão `CurvaDeCampeonato.jsx` já
// mostrava onde elas moram melhor.
//
// Paleta validada contra a superfície #0f1c2b (banda de luminosidade, piso de
// croma, separação sob daltonismo e contraste) — não trocar sem revalidar. As
// duas cores são exportadas porque o card de custo anual desenha as mesmas
// barras logo acima do gráfico: azul é o pago, laranja é o de mercado, e essa
// convenção vale no cartão inteiro.
export const CURVA_PAGO = "#388bfd";
export const CURVA_MERCADO = "#db6d28";

// A moldura do gráfico — colunas de categoria, hachura do ano sem contrato,
// fita de equipes, réguas de troca — mora em `curvaDeCarreira.jsx`, dividida
// com a curva de campeonato do Histórico. O que sobra aqui é a SÉRIE.

// Os dois lados da faixa de diferença, nomeados porque a moldura é genérica: ela
// desenha a distância entre duas leituras quaisquer, e quem diz quais são é cada
// gráfico.
const LER_CONTRATO = (ponto) => ponto.salario_contrato;
const LER_MERCADO = (ponto) => ponto.salario_mercado;

export function MarketCurve({ pontos }) {
  const { t } = useTranslation();
  const [emFoco, setEmFoco] = useState(null);
  const [trocaEmFoco, setTrocaEmFoco] = useState(null);
  // `null` é "o jogador ainda não escolheu" — e só nesse estado a curva decide
  // sozinha entre desenho e tabela. Uma vez que ele aperta o botão, a escolha
  // dele manda, inclusive ao abrir a ficha do piloto seguinte.
  const [tabela, setTabela] = useState(null);

  // Um ponto só não é uma curva — e o par de números de hoje já está nos cards
  // logo abaixo. Duas temporadas é o mínimo para haver trajetória.
  if (!Array.isArray(pontos) || pontos.length < 2) return null;

  const geo = geometriaDaCurva(pontos);
  // `x` é a régua da MOLDURA (centro da coluna do ano: rótulo, fita, alvos de
  // hover); `xSerie` é a da SÉRIE (fim do ano, onde o número fechou). As duas
  // coincidem só na temporada em curso, que ainda não terminou.
  const { w, h, padE, padD, padT, alturaPlot, x, xSerie, passo } = geo;

  const valores = pontos.flatMap((p) =>
    [p.salario_mercado, p.salario_contrato].filter((v) => Number.isFinite(v) && v > 0),
  );
  if (!valores.length) return null;

  const escala = escalaLog(valores);
  const y = (valor) => padT + alturaPlot * (1 - escala.fracao(valor));

  const foco = emFoco === null ? null : pontos[emFoco];
  // Os dois lados se partem por motivos diferentes, então são contados
  // separadamente: falta de arquivo quebra a laranja, ano sem equipe quebra a
  // azul. Uma nota só, genérica, deixaria o jogador adivinhando qual é qual.
  //
  // Temporada futura fica fora da conta: ali a laranja não falta, ela ainda não
  // existe — contá-la mandaria o jogador procurar um arquivo perdido que nunca
  // foi escrito.
  const semDado = pontos.filter(
    (p) => !p.futuro && !Number.isFinite(p.salario_mercado),
  ).length;
  // A fronteira do que já aconteceu — o índice do PRIMEIRO ano ainda por correr,
  // e não o do último cumprido: com o ponto no começo da coluna, o traço que sai
  // do ano N cobre a coluna de N, então é do ponto do primeiro ano contratado em
  // diante que o desenho vira promessa. `-1` quando a curva é toda passado, e é
  // ele que apaga a régua do "hoje" e o traço fantasma junto.
  const primeiroFuturo = pontos.findIndex((p) => p.futuro);
  const inicioDoFuturo = primeiroFuturo >= 1 ? primeiroFuturo : -1;
  // O que rompe o vínculo aqui é a falta de SALÁRIO: o ano sem contrato é
  // exatamente o ano sem a série azul, e as duas marcas têm de nascer do mesmo
  // teste para nunca discordarem sobre onde o vão começa.
  const temContratoNoAno = (p) => Number.isFinite(p.salario_contrato);
  const faixas = faixasSemVinculo(pontos, geo, temContratoNoAno);
  const colunas = colunasDeCategoria(pontos, geo);
  const trocas = trocasDeEquipe(pontos, (p) => p.salario_contrato);
  const trocaAberta = trocas.find((troca) => troca.indice === trocaEmFoco) ?? null;
  // O rótulo direto se prende à última temporada que TEM aquele número, e não à
  // última do eixo — senão a série que acaba antes fica sem ponta rotulada.
  const pontas = pontasRotuladas(pontos, y);

  // Abaixo de três temporadas cumpridas o gráfico não tem o que desenhar: dois
  // pontos numa moldura dimensionada para uma carreira inteira leem-se como
  // "faltou informação", quando na verdade a informação está toda ali. A tabela
  // diz os mesmos números sem prometer uma trajetória que ainda não existe.
  //
  // Conta ANOS ANTERIORES: a temporada em curso está no meio e as assinadas
  // ainda não aconteceram — nenhuma das duas é histórico.
  const anosDePassado = pontos.filter((p) => !p.futuro && !p.atual).length;
  const emTabela = tabela ?? anosDePassado < MINIMO_PARA_O_GRAFICO;

  return (
    <div className="mt-3 rounded-xl bg-[#0f1c2b] px-4 py-3.5" data-testid="driver-detail-market-curve">
      <div className="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1.5">
        <span className="text-xs font-semibold text-text-secondary">
          {t("driverDetail.market.curve.title")}
        </span>
        <div className="flex items-center gap-3">
          <SerieKey cor={CURVA_PAGO} label={t("driverDetail.market.curve.paid")} />
          <SerieKey cor={CURVA_MERCADO} label={t("driverDetail.market.curve.worth")} />
          {/* O gráfico nunca é o único caminho para o número: a tabela é o
              mesmo dado sem depender de cor nem de hover. */}
          <button
            type="button"
            // A escolha automática é um PADRÃO, não uma trava: o gráfico de três
            // pontos continua alcançável para quem quiser vê-lo.
            onClick={() => setTabela(!emTabela)}
            className="rounded-full border border-white/15 px-2 py-0.5 text-[10px] text-text-secondary transition-colors hover:text-text-primary"
            data-testid="driver-detail-curve-toggle"
          >
            {t(emTabela ? "driverDetail.market.curve.showChart" : "driverDetail.market.curve.showTable")}
          </button>
        </div>
      </div>

      {emTabela ? (
        <CurvaEmTabela pontos={pontos} />
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
            aria-label={t("driverDetail.market.curve.title")}
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
                    {formatMoneyCompact(marca)}
                  </text>
                </g>
              ))}
            />

            {/* A faixa entre as duas linhas: é ela que carrega a leitura. Some
                quando não há contrato para comparar naquele ano. */}
            {faixasDeDiferenca(pontos, LER_MERCADO, LER_CONTRATO).map((faixa) => (
              <path
                key={`faixa-${faixa.inicio}`}
                d={caminhoDaFaixa(faixa.trecho, geo, y, faixa.inicio, LER_MERCADO, LER_CONTRATO)}
                fill={CURVA_MERCADO}
                opacity="0.1"
              />
            ))}

            {segmentosContinuos(pontos, LER_CONTRATO)
              .flatMap((trecho) => partirNoPresente(trecho, inicioDoFuturo))
              .map((trecho) => (
                <polyline
                  key={`pago-${trecho.inicio}-${trecho.futuro}`}
                  data-serie="pago"
                  data-futuro={trecho.futuro ? "" : undefined}
                  points={verticesDaSerie(trecho, geo, y, LER_CONTRATO)}
                  fill="none"
                  stroke={CURVA_PAGO}
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  // Fantasma, e não pontilhado: o tracejado já é o vocabulário do
                  // vão sem contrato, e reusá-lo aqui diria "não houve salário"
                  // sobre anos que têm salário assinado. O que muda no futuro é a
                  // certeza, não a existência — e certeza se desenha com peso.
                  opacity={trecho.futuro ? 0.4 : 1}
                />
              ))}

            {segmentosContinuos(pontos, LER_MERCADO).map((trecho) => (
              <polyline
                key={`mercado-${trecho.inicio}`}
                data-serie="mercado"
                points={verticesDaSerie(trecho, geo, y, LER_MERCADO)}
                fill="none"
                stroke={CURVA_MERCADO}
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            ))}

            {/* A ponte que atravessa o vão, ligando os dois pontos azuis das
                margens — e que muda de estilo conforme o chão que pisa.

                Pontilhada sobre a hachura: ali não houve salário, e um traço
                cheio teria de inventar o valor de cada ponto do caminho. Cheia
                fora dela, porque fora dela houve contrato e a série não tem
                motivo para se esconder — a linha do dinheiro é uma só, e era
                pontilhar ano pago que estava errado.

                Vem depois das séries e antes dos marcadores: passa por cima da
                laranja quando cruza com ela, e por baixo das bolinhas. */}
            {pontesSemVinculo(faixas, pontos, LER_CONTRATO).map((ponte) => (
              <PonteSobreOVao key={`ponte-${ponte.de}`} ponte={ponte} x={xSerie} y={y} cor={CURVA_PAGO} />
            ))}

            {/* A régua do presente. Sem ela o traço fantasma seria só uma linha
                mais fraca sem motivo declarado; com ela o gráfico ganha um
                antes e um depois, e o leitor entende de graça por que a laranja
                não acompanha — dali para frente não há com o que comparar. */}
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

            {pontos.map((ponto, indice) => (
              <g key={ponto.season_number}>
                {Number.isFinite(ponto.salario_contrato) ? (
                  <circle
                    cx={xSerie(indice)}
                    cy={y(ponto.salario_contrato)}
                    r={ponto.atual || indice === emFoco ? 4.5 : 3}
                    // Marcador vazado no futuro: mesma posição, mesmo tamanho, e
                    // ainda assim ninguém confunde com temporada cumprida.
                    fill={ponto.futuro ? "#0f1c2b" : CURVA_PAGO}
                    stroke={ponto.futuro ? CURVA_PAGO : "#0f1c2b"}
                    strokeWidth="2"
                    data-futuro={ponto.futuro ? "" : undefined}
                  />
                ) : null}
                {Number.isFinite(ponto.salario_mercado) ? (
                  <circle
                    cx={xSerie(indice)}
                    cy={y(ponto.salario_mercado)}
                    r={ponto.atual || indice === emFoco ? 4.5 : 3}
                    fill={CURVA_MERCADO}
                    stroke="#0f1c2b"
                    strokeWidth="2"
                  />
                ) : null}
              </g>
            ))}

            {/* Rótulo direto só na ponta: um número em cada ponto viraria ruído,
                e o resto é alcançável pelo hover e pela tabela. */}
            {pontas.map((ponta) => (
              <text
                key={ponta.serie}
                data-rotulo="ponta"
                x={xSerie(ponta.indice)}
                y={ponta.y}
                // Ancorado à direita do ponto, sobre a coluna do próprio ano.
                // Com o marcador no começo do ano é ali que sobra espaço — à
                // esquerda o número cairia por cima da linha que chega nele.
                textAnchor="start"
                className={`${ponta.classe} font-mono text-[10px] font-semibold tabular-nums`}
              >
                {formatMoneyCompact(ponta.valor)}
              </text>
            ))}

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
              <CurvaTooltip
                ponto={foco}
                ancora={ancoraDoBalao(
                  [foco.salario_contrato, foco.salario_mercado]
                    .filter((valor) => Number.isFinite(valor))
                    .map(y),
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
                // A margem de 14% impede que ele saia pela borda do cartão nas
                // trocas do primeiro e do último ano.
                esquerda={`${Math.min(86, Math.max(14, ((x(trocaAberta.indice) - passo / 2) / w) * 100))}%`}
                topo={`${(CURVA_FITA.y / h) * 100}%`}
                formatarValor={formatMoneyCompact}
                rodape={<DeltaDaTroca troca={trocaAberta} />}
              />
            ) : null}
          </div>

          {/* O que sobrou da legenda: uma chave só, e só às vezes.

              A chave de categoria virou o nome escrito na própria coluna — em
              pé onde não cabe deitado — e a de troca de equipe deixou de
              existir junto com o losango: duas emendas de chip não pedem
              tradução. Sobra a do traço fantasma, que a régua do "hoje" localiza
              mas não explica.

              A linha inteira só nasce quando há o que dizer. Uma borda superior
              sobre nada era o rodapé anunciando uma seção vazia. */}
          {inicioDoFuturo >= 0 ? (
            <div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 border-t border-white/[0.06] pt-2 text-[10px] text-text-muted">
              <span className="flex items-center gap-1.5" data-chave="futuro">
                <span
                  className="h-0.5 w-3 shrink-0 rounded-full"
                  style={{ backgroundColor: CURVA_PAGO, opacity: 0.4 }}
                />
                {t("driverDetail.market.curve.contracted")}
              </span>
            </div>
          ) : null}

          {/* Sobra aqui só o que o desenho NÃO consegue mostrar: ano sem arquivo
              não tem faixa nem ponto — não há onde ancorar a marca. O ano sem
              contrato saiu desta nota porque virou faixa no gráfico, e a
              explicação de como a laranja é reconstruída saiu porque era
              metodologia, não leitura: quem abre a ficha quer ver a diferença,
              não a procedência da linha.

              Fora do quadro relativo de propósito: é ele que dá ao balão a
              altura contra a qual se alinhar, e a nota inflando esse quadro
              deslocaria o `bottom: 0` do balão para baixo do gráfico. */}
          {semDado > 0 ? (
            <p className="mt-1 text-[10px] leading-relaxed text-text-muted">
              {t("driverDetail.market.curve.missing", { count: semDado })}
            </p>
          ) : null}
        </>
      )}
    </div>
  );
}

function CurvaTooltip({ ponto, ancora }) {
  const { t } = useTranslation();
  // A diferença só existe com os dois lados na mão.
  const diferenca =
    Number.isFinite(ponto.salario_contrato) && Number.isFinite(ponto.salario_mercado)
      ? ponto.salario_mercado - ponto.salario_contrato
      : null;

  return (
    <div
      // `pointer-events-none` é o que impede o balão de roubar o hover do alvo
      // que o abriu — sem isso ele pisca ao se posicionar sob o cursor.
      className="pointer-events-none absolute z-10 w-max max-w-[240px] rounded-lg border border-white/10 bg-[#0b1622] px-3 py-2 shadow-xl shadow-black/50"
      style={{ left: ancora.esquerda, ...ancora.vertical, transform: ancora.transform }}
      data-testid="driver-detail-curve-tooltip"
    >
      <div className="flex items-center gap-2">
        <TeamLogoMark
          teamName={ponto.equipe_nome}
          size="xs"
          halo
          testId="driver-detail-curve-tooltip-logo"
        />
        <span className="min-w-0 flex-1 truncate text-xs font-semibold text-text-primary">
          {ponto.equipe_nome || t("driverDetail.market.curve.noTeam")}
        </span>
        {/* Categoria colada no ano: sem ela o balão diz um salário sem dizer em
            que degrau ele foi pago. */}
        <span className="shrink-0 font-mono text-[10px] tabular-nums text-text-muted">
          {[ponto.categoria ? formatCategoryLabel(ponto.categoria) : null, ponto.ano]
            .filter(Boolean)
            .join(" · ")}
        </span>
      </div>

      <div className="mt-1.5 space-y-1">
        <LinhaDoBalao
          cor={CURVA_PAGO}
          label={t("driverDetail.market.curve.paid")}
          valor={
            // Um traço não diz nada; "sem contrato" diz que o buraco é o dado.
            Number.isFinite(ponto.salario_contrato)
              ? formatSalary(ponto.salario_contrato)
              : t("driverDetail.market.curve.noContract")
          }
        />
        <LinhaDoBalao
          cor={CURVA_MERCADO}
          label={t("driverDetail.market.curve.worth")}
          valor={
            Number.isFinite(ponto.salario_mercado)
              ? formatSalary(ponto.salario_mercado)
              // Temporada que ainda não aconteceu não tem arquivo faltando — ela
              // não tem arquivo ainda, e chamar isso de lacuna mandaria o jogador
              // caçar um dado perdido que não existe.
              : t(
                  ponto.futuro
                    ? "driverDetail.market.curve.notYet"
                    : "driverDetail.market.curve.noArchive",
                )
          }
        />
      </div>

      {diferenca !== null ? (
        <p
          // Centralizado porque é fecho, não mais um item da lista: alinhado à
          // esquerda ele entrava na coluna dos rótulos acima e lia como a quarta
          // linha do mesmo bloco.
          className="mt-1.5 border-t border-white/[0.08] pt-1.5 text-center text-[10px]"
          style={{ color: diferenca >= 0 ? TONE_HEX.success : TONE_HEX.warning }}
        >
          {t("driverDetail.market.curve.gapValue", { value: formatSignedMoney(diferenca) })}
        </p>
      ) : null}
    </div>
  );
}

// O fecho do balão da troca de equipe: o que mudou no salário ao mudar de casa.
// É a pergunta seguinte à troca, e a resposta já está nos dois chips.
function DeltaDaTroca({ troca }) {
  const { t } = useTranslation();
  const diferenca =
    Number.isFinite(troca.valorDe) && Number.isFinite(troca.valorPara)
      ? troca.valorPara - troca.valorDe
      : null;
  if (diferenca === null) return null;

  return (
    <FechoDoBalao cor={diferenca >= 0 ? TONE_HEX.success : TONE_HEX.warning}>
      {t("driverDetail.market.curve.salaryDelta", { value: formatSignedMoney(diferenca) })}
    </FechoDoBalao>
  );
}

// O mesmo dado sem cor e sem hover — o caminho de leitura que não depende de
// enxergar a diferença entre azul e laranja.
function CurvaEmTabela({ pontos }) {
  const { t } = useTranslation();
  return (
    <div className="mt-2 max-h-52 overflow-y-auto" data-testid="driver-detail-curve-table">
      <table className="w-full text-[11px]">
        <thead>
          <tr className="text-text-muted">
            <th className="py-1 text-left font-medium">{t("driverDetail.market.curve.season")}</th>
            <th className="py-1 text-left font-medium">{t("driverDetail.market.curve.category")}</th>
            <th className="py-1 text-left font-medium">{t("driverDetail.market.curve.team")}</th>
            <th className="py-1 text-right font-medium">{t("driverDetail.market.curve.paid")}</th>
            <th className="py-1 text-right font-medium">{t("driverDetail.market.curve.worth")}</th>
          </tr>
        </thead>
        <tbody>
          {[...pontos].reverse().map((ponto) => (
            <tr key={ponto.season_number} className="border-t border-white/[0.06]">
              <td className="py-1 font-mono tabular-nums text-text-secondary">{ponto.ano}</td>
              {/* A tabela é o caminho sem cor: aqui a categoria precisa estar
                  escrita, não pintada como na trilha do gráfico. */}
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
                    testId="driver-detail-curve-row-logo"
                  />
                  <span className="min-w-0 truncate">
                    {ponto.equipe_nome || t("driverDetail.market.curve.noTeam")}
                  </span>
                </span>
              </td>
              {/* A tabela é o caminho de leitura que não depende de ver a linha
                  se partir — então é aqui que a lacuna precisa se nomear. */}
              <td className="py-1 text-right font-mono tabular-nums text-text-primary">
                {Number.isFinite(ponto.salario_contrato) ? (
                  formatSalary(ponto.salario_contrato)
                ) : (
                  <span className="font-sans text-text-muted">
                    {t("driverDetail.market.curve.noContract")}
                  </span>
                )}
              </td>
              <td className="py-1 text-right font-mono tabular-nums text-text-primary">
                {Number.isFinite(ponto.salario_mercado) ? (
                  formatSalary(ponto.salario_mercado)
                ) : (
                  <span className="font-sans text-text-muted">
                    {t(
                      ponto.futuro
                        ? "driverDetail.market.curve.notYet"
                        : "driverDetail.market.curve.noArchive",
                    )}
                  </span>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

// A ponta de cada série, empurrada para longe da outra quando as duas terminam
// na mesma altura. Com posição fixa (uma sempre acima, outra sempre abaixo) os
// dois números se sobrepunham sempre que as linhas se encontravam no fim.
export function pontasRotuladas(pontos, y) {
  const ultimaDe = (ler) => {
    for (let i = pontos.length - 1; i >= 0; i -= 1) {
      if (Number.isFinite(ler(pontos[i]))) return { indice: i, valor: ler(pontos[i]) };
    }
    return null;
  };

  const pago = ultimaDe((p) => p.salario_contrato);
  const mercado = ultimaDe((p) => p.salario_mercado);
  const pontas = [];
  if (pago) pontas.push({ serie: "pago", classe: "fill-[#388bfd]", ...pago });
  if (mercado) pontas.push({ serie: "mercado", classe: "fill-[#db6d28]", ...mercado });

  const colidem =
    pontas.length === 2 &&
    pontas[0].indice === pontas[1].indice &&
    Math.abs(y(pontas[0].valor) - y(pontas[1].valor)) < 16;

  return pontas.map((ponta) => {
    const alto = pontas.length === 2 && ponta.valor >= Math.max(...pontas.map((p) => p.valor));
    // Colidindo, cada uma foge para o seu lado; separadas, ambas ficam acima do
    // ponto, que é onde sobra espaço em um gráfico que termina subindo.
    const deslocamento = colidem ? (alto ? -10 : 17) : -9;
    return { ...ponta, y: y(ponta.valor) + deslocamento };
  });
}
