import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

import { bestEffortComRetorno } from "../../utils/bestEffort";
import { novaCategoria } from "../../utils/sfx";
import { AtlasChart } from "../team/v2/AtlasChart";
import { AtlasRankings } from "../team/v2/AtlasRankings";
import { AtlasChampionsPanel } from "../team/v2/AtlasChampionsPanel";
import {
  BAND_GAP,
  DIVISION_BOTTOM_PADDING,
  DIVISION_HEADER_HEIGHT,
  YEAR_HEADER_HEIGHT,
  atlasDivisions,
  axisEndYear,
  buildAtlasTracks,
  buildAtlasVerticalGeometry,
  buildRankingCards,
  firstSeriesYear,
  isLivePayload,
} from "../team/v2/atlasV2Geometry";
import { DEFAULT_START_YEAR, normalizePayload } from "../team/worldTeamChartGeometry";

// O Atlas nascendo, embaixo das mensagens da tela de espera.
//
// A geração do rascunho é um comando bloqueante, e mesmo assim o mundo já existe em
// disco enquanto ela roda: `simulate_historical_range` persiste CADA temporada antes
// de abrir a seguinte, e `get_global_team_history` resolve o banco por `career_id`
// dentro de `saves_dir` — o do rascunho inclusive. Então não há comando novo aqui, e
// nem precisa haver: é a mesma leitura da aba Atlas, apontada para um save que ainda
// está sendo escrito. O SQLite é WAL com `busy_timeout`, que é o que torna a leitura
// concorrente com o escritor um caso previsto e não uma corrida.
//
// O gatilho é o ANO, nunca o relógio. O polling do rascunho bate a cada segundo, e
// recarregar o Atlas nesse ritmo devolveria em disputa de lock parte do tempo que a
// geração acabou de ganhar. Com `anoConcluido` na lista de dependências, uma
// temporada fechada é uma leitura, e um segundo parado não é leitura nenhuma.
//
// O que aparece aqui não é o mundo final: depois do laço ainda rodam
// `purge_never_raced_backstory_orphans` e `reset_historical_finance_for_playable_start`.
// A grade de temporadas e a trajetória das equipes ficam de pé; nomes de pilotos e
// dinheiro mudam no último passo.
const JANELA_DE_ANOS = 32;
// A coluna dos cards. Mais estreita que a da aba cheia, que chega a 445px: aqui ela
// divide a largura com o gráfico dentro de um overlay.
const LARGURA_DOS_CARDS = 400;
// O mesmo vão da aba cheia, e não um número novo: é a distância que o rabicho colorido
// percorre do fim da linha até o card, e ela está cravada dentro do `AtlasRankings`.
const VAO_ENTRE_COLUNAS = 22;
// Altura reservada ao resto da tela de espera: o card da mensagem, o título do painel e
// as margens. O que sobra da viewport é o teto do gráfico.
const ALTURA_DO_RESTO = 400;
const ALTURA_MINIMA = 320;
const ALTURA_MAXIMA = 680;
// Altura de uma linha de equipe. Aqui ela responde só à leitura do traçado, porque a
// etiqueta de estreia não é desenhada neste painel: era ela que impunha um piso, já que
// um grupo que não cabe na área de linhas é empurrado para o topo da faixa, por cima do
// título do campeonato.
const ALTURA_DE_LINHA = 30;
// Quanto tempo a área leva para revelar o campeonato que acabou de nascer.
const ABERTURA_MS = 620;

// A escada nasce no GT3 (1999) e as categorias de baixo aparecem depois: Toyota Cup em
// 2012, Mazda Championship em 2016, Production em 2018 e os Rookie em 2020. O painel
// abre no GT3, que é onde o mundo começa, e SÓ TROCA POR CLIQUE: a geração não arrasta
// a tela para outro campeonato no meio de uma leitura.
const FAMILIA_INICIAL = "gt3";
// A família do jogador ainda não é escolhida quando a geração roda: no wizard, categoria
// e equipe vêm DEPOIS do histórico. `mazda` é o padrão do backend (`DEFAULT_FAMILY`) e o
// primeiro cartão do passo seguinte.
const FAMILIA_DO_JOGADOR = "mazda";

/// Ano em que o primeiro campeonato de uma família existe. Vem do próprio payload —
/// `families` viaja completo, com o `starts_year` de cada faixa — para o front não
/// manter uma segunda cópia da linha do tempo que mora em `constants::historical_timeline`.
function nascimentoDaFamilia(payload, id) {
  const bands = (payload?.families ?? []).find((familia) => familia.id === id)?.bands ?? [];
  const anos = bands.map((band) => band.starts_year).filter((ano) => Number.isFinite(ano));
  return anos.length ? Math.min(...anos) : null;
}

/// As abas do painel, na ordem da escada. Vazio enquanto existir um mundo só: um seletor
/// de um item é enfeite, e é a chegada da segunda família que vale ser anunciada.
///
/// O corte é uma temporada DECIDIDA da família nova, e não o ano em que ela nasce. No
/// ano de estreia o campeonato existe no calendário e ainda não tem resultado: quem
/// clicasse na aba naquele momento trocava um mundo com dezessete temporadas por uma
/// tela com uma coluna e nenhuma classificação fechada.
export function familiasParaAbas(payload) {
  const nascimento = nascimentoDaFamilia(payload, FAMILIA_DO_JOGADOR);
  const decidido = ultimoAnoDecidido(payload);
  if (!Number.isFinite(nascimento) || !Number.isFinite(decidido) || decidido < nascimento) return [];
  return (payload?.families ?? [])
    .filter((item) => [FAMILIA_INICIAL, FAMILIA_DO_JOGADOR].includes(item.id))
    .sort(
      (esquerda, direita) =>
        (nascimentoDaFamilia(payload, esquerda.id) ?? 0) - (nascimentoDaFamilia(payload, direita.id) ?? 0),
    );
}

/// Tira do payload as faixas que ainda não decidiram nenhuma temporada.
///
/// A aba cheia mostra a faixa vazia de propósito: lá o eixo cobre o mundo inteiro, e a
/// hachura conta quando aquele campeonato ainda não existia. Aqui o mundo está sendo
/// escrito, e uma faixa reservando meia altura da tela para um campeonato que só nasce
/// dez temporadas adiante é espaço gasto com nada.
///
/// O critério é a última temporada DECIDIDA, e não "tem algum ponto". O payload injeta
/// a divisão da temporada em andamento em toda faixa que já tem grid montado, com zero
/// ponto para cada equipe — foi por isso que Mazda Production e Mazda Rookie apareceram
/// em 2016, quatro e oito anos antes de existirem. Ter linha no ano em curso não é ter
/// história: a faixa entra quando o primeiro campeonato dela termina.
export function comFaixasQueJaCorreram(payload) {
  if (!payload) return payload;
  const ultimoDecidido = ultimoAnoDecidido(payload);
  if (!Number.isFinite(ultimoDecidido)) return { ...payload, bands: [] };
  const bands = (payload.bands ?? []).filter((band) =>
    (band.rows ?? []).some((row) => (row.points ?? []).some((point) => point.year <= ultimoDecidido)),
  );
  return { ...payload, bands };
}

/// Última temporada com resultado fechado. Com uma geração em curso é o
/// `last_completed_year` que o backend calcula; sem ele, o ano anterior ao que está
/// sendo simulado, que é a mesma coisa dita pela ponta de trás.
function ultimoAnoDecidido(payload) {
  if (!isLivePayload(payload)) return payload?.current_year ?? null;
  if (Number.isFinite(payload.last_completed_year)) return payload.last_completed_year;
  return payload.current_year - 1;
}

/// Os anos do eixo: do primeiro com dado até o último disputado.
///
/// `buildAtlasYears` abre o eixo alguns anos ANTES da primeira temporada, para caber a
/// etiqueta das equipes fundadoras — que é desenhada à esquerda do ponto de estreia. Na
/// aba cheia isso se paga; aqui a folga aparecia como três colunas hachuradas na frente
/// de um mundo que ainda tem poucas temporadas.
export function anosDoPainel(payload) {
  const primeiro = firstSeriesYear(payload);
  const ultimo = axisEndYear(payload);
  if (!Number.isFinite(primeiro) || !Number.isFinite(ultimo) || primeiro > ultimo) return [];
  const anos = [];
  for (let ano = primeiro; ano <= ultimo; ano += 1) anos.push(ano);
  return anos;
}

/// Altura que a grade pede quando ninguém a aperta: cabeçalho e rodapé de cada faixa,
/// os vãos entre elas e uma linha por equipe.
///
/// A régua vertical distribui toda sobra de altura como respiro no pé das divisões, o
/// que na aba cheia é o certo — lá a moldura ocupa a viewport e o vão pertence ao
/// desenho. Aqui a mesma conta abria um campo vazio do tamanho da própria tabela abaixo
/// do Mazda Championship. Pedindo a altura natural, a sobra deixa de existir.
///
/// A linha é a compacta, e não a de 54px que a aba cheia usa como teto: com um
/// campeonato só na tela o painel abria com 14 equipes espalhadas por setecentos pixels.
/// Ele nasce no tamanho do que tem para mostrar e cresce quando o segundo campeonato
/// chega, com a área revelando o que apareceu.
export function alturaNaturalDaGrade(divisions) {
  const lista = divisions ?? [];
  if (!lista.length) return ALTURA_MINIMA;
  const linhas = lista.reduce((soma, divisao) => soma + Math.max(divisao.rowCount ?? 1, 1), 0);
  return (
    lista.length * (DIVISION_HEADER_HEIGHT + DIVISION_BOTTOM_PADDING) +
    Math.max(lista.length - 1, 0) * BAND_GAP +
    linhas * ALTURA_DE_LINHA
  );
}

function tetoDaViewport() {
  const viewport = typeof window === "undefined" ? 900 : window.innerHeight;
  return Math.min(Math.max(viewport - ALTURA_DO_RESTO, ALTURA_MINIMA), ALTURA_MAXIMA);
}

/// Clique de equipe não abre nada aqui: o dossiê fala de dinheiro e elenco, e os dois
/// ainda mudam no último passo da geração. Uma função nomeada em vez de `() => {}`
/// solto, pelo mesmo motivo do `bestEffort`: o vazio é intencional.
function semAcao() {}

function DraftAtlasPreview({ careerId, anoConcluido }) {
  const { t } = useTranslation();
  const [payload, setPayload] = useState(null);
  // A família PEDIDA ao backend. Começa no GT3 e só muda quando o jogador clica numa
  // aba: no meio de uma leitura, a tela trocar de campeonato sozinha é a tela sendo
  // arrancada da mão dele.
  const [familia, setFamilia] = useState(FAMILIA_INICIAL);
  const [faixaDosCampeoes, setFaixaDosCampeoes] = useState(null);
  const [equipeEmFoco, setEquipeEmFoco] = useState(null);
  const [teto] = useState(tetoDaViewport);
  const laneRef = useRef(null);
  const abasJaAnunciadas = useRef(false);

  useEffect(() => {
    if (!careerId) return undefined;

    let vivo = true;
    bestEffortComRetorno(
      invoke("get_global_team_history", {
        careerId,
        family: familia,
        startYear: DEFAULT_START_YEAR,
        windowSize: JANELA_DE_ANOS,
      }),
      "atlas_da_geracao",
    ).then(({ ok, valor }) => {
      if (!vivo || !ok) return;
      const normalizado = normalizePayload(valor);
      if (!normalizado?.bands?.length) return;
      setPayload(comFaixasQueJaCorreram(normalizado));
    });

    return () => {
      vivo = false;
    };
  }, [careerId, anoConcluido, familia]);

  const abas = useMemo(() => familiasParaAbas(payload), [payload]);
  const temAbas = abas.length > 1;

  // O som acompanha a entrada do aviso, e toca UMA vez: o painel relê o mundo a cada
  // temporada, e sem esta guarda o chime viraria um metrônomo até o fim da geração.
  useEffect(() => {
    if (!temAbas || abasJaAnunciadas.current) return;
    abasJaAnunciadas.current = true;
    novaCategoria();
  }, [temAbas]);

  const divisions = useMemo(() => atlasDivisions(payload), [payload]);
  const altura = useMemo(
    () => Math.min(alturaNaturalDaGrade(divisions), teto),
    [divisions, teto],
  );
  const years = useMemo(() => anosDoPainel(payload), [payload]);
  const tracks = useMemo(() => buildAtlasTracks(payload, years), [payload, years]);
  const cards = useMemo(() => buildRankingCards(payload), [payload]);
  const vertical = useMemo(
    () => buildAtlasVerticalGeometry({ totalHeight: altura, divisions }),
    [divisions, altura],
  );

  if (!payload || !years.length) return null;

  // O rabicho que liga a linha ao card só é desenhado quando os dois falam do mesmo
  // ano — e o ano da ponta direita é o da temporada em andamento, quando há uma.
  const ultimoAnoDoEixo = isLivePayload(payload) ? payload.current_year : years[years.length - 1];

  return (
    <>
      {/* O anúncio do mundo novo, ACIMA do card da mensagem.
          `order` em vez de posição fixa. O overlay da espera é uma coluna flex, e o card
          é o primeiro item dela; um elemento fixo no topo da tela caía por cima do card,
          que foi como isto nasceu. Com `order: -1` a barra entra na MESMA coluna, antes
          do card, e o espaçamento vem do `gap` do overlay como o de qualquer irmão. */}
      {temAbas ? (
        <div
          data-testid="draft-atlas-familias"
          className="animate-toast-up rounded-full border border-white/10 bg-app-bg/90 px-2 py-1.5 shadow-[0_10px_30px_rgba(0,0,0,0.45)] backdrop-blur-md"
          style={{ order: -1 }}
        >
          <div className="flex items-center gap-1">
            {abas.map((item) => (
              <button
                key={item.id}
                type="button"
                data-testid={`draft-atlas-familia-${item.id}`}
                onClick={() => setFamilia(item.id)}
                className={`rounded-full px-4 py-1.5 text-xs font-semibold uppercase tracking-[0.18em] transition-colors ${
                  payload.selected_family === item.id
                    ? "bg-accent-primary/20 text-accent-primary"
                    : "text-text-muted hover:text-text-primary"
                }`}
              >
                {item.label}
              </button>
            ))}
          </div>
        </div>
      ) : null}

      <section data-testid="draft-atlas-preview" className="w-full max-w-[1480px] px-6" aria-live="polite">
        <p className="mb-2 text-center text-xs font-semibold uppercase tracking-[0.22em] text-accent-primary">
          {t("newCareer.loading.atlasTitle")}
        </p>
        {/* A área cresce quando um campeonato novo entra na família, e a transição é de
            ALTURA com o excedente recortado: a régua vertical já entregou as posições
            finais, então o que a animação faz é revelar a faixa nova de cima para baixo,
            em vez de empurrar o desenho inteiro. */}
        <div
          className="grid overflow-hidden"
          style={{
            gridTemplateColumns: `minmax(0, 1fr) ${LARGURA_DOS_CARDS}px`,
            gridTemplateRows: `${YEAR_HEADER_HEIGHT}px ${altura}px`,
            columnGap: VAO_ENTRE_COLUNAS,
            height: YEAR_HEADER_HEIGHT + altura,
            transition: `height ${ABERTURA_MS}ms cubic-bezier(0.22, 1, 0.36, 1)`,
          }}
        >
          <AtlasChart
            payload={payload}
            years={years}
            tracks={tracks}
            vertical={vertical}
            focusedTeamId={equipeEmFoco}
            pinnedTeamId={null}
            onFocus={setEquipeEmFoco}
            onTeamClick={semAcao}
            onTeamDoubleClick={semAcao}
            // Quem nomeia as equipes aqui é a coluna de cards, ao lado. A etiqueta de
            // estreia diria o mesmo a meio palmo de distância, e ainda tapava o título
            // do campeonato quando a faixa inteira estreava no mesmo ano.
            mostrarEtiquetasDeEstreia={false}
          />
          {/* A coluna dos cards é quem nomeia as equipes, com escudo, nome e posição da
              temporada corrente. */}
          <AtlasRankings
            laneRef={laneRef}
            cards={cards}
            vertical={vertical}
            lastAxisYear={ultimoAnoDoEixo}
            focusedTeamId={equipeEmFoco}
            pinnedTeamId={null}
            onFocus={setEquipeEmFoco}
            onTeamClick={semAcao}
            onTeamDoubleClick={semAcao}
            onOpenChampions={setFaixaDosCampeoes}
          />
        </div>
      </section>

      {/* O dossiê de equipe continua fora daqui: ele conta finanças e elenco, que o
          último passo da geração ainda reescreve. Os campeões, não — um campeonato
          decidido está decidido. */}
      {faixaDosCampeoes ? (
        <AtlasChampionsPanel
          careerId={careerId}
          band={faixaDosCampeoes}
          onClose={() => setFaixaDosCampeoes(null)}
        />
      ) : null}
    </>
  );
}

export default DraftAtlasPreview;
