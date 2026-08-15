# Ficha do piloto — componentes do v2

A ficha do piloto (`DriverDetailModalV2.jsx`) e os blocos que só ela usa.

O nome "v2" é histórico: durante o redesenho o v1
(`src/components/driver/DriverDetailModal.jsx`) ficava um nível acima e servia de
rollback, e a regra era não editá-lo. O v1 foi removido em 11/08/2026 — esta é a
única ficha viva, e `src/components/driver/index.js` é apenas o ponto de entrada.

## O corte dos arquivos

O modal desenha a casca, as abas e o estado de alto nível. O que é **gráfico**
mora em módulo irmão, o que é **seção fechada** também, e o que é **lógica pura**
mora fora do JSX:

- `curvaDeCarreira.jsx` — a moldura que os dois gráficos de carreira dividem
  (colunas de categoria, hachura do ano fora do grid, fita de equipes, balões).
- `CurvaDeCampeonato.jsx` e `CurvaDeMercado.jsx` — as duas séries que vestem essa
  moldura: posição no campeonato contra a expectativa do carro, salário pago
  contra o de mercado.
- `FaixaDeConfronto.jsx` — o duelo com o rival: a faixa cheia do card e a
  miniatura de cada linha da lista.
- `RecentFormStrip.jsx` — a faixa de forma da aba Temporada: uma coluna por
  corrida, agrupada por temporada, com a escala do eixo comum aos grupos.
- `MarketSection.jsx` — a aba Mercado inteira: termômetro de troca, situação
  contratual, régua de vigência, valor de mercado e custo anual. Recebe o
  `detail` e o `careerId`, e não conversa com o resto da ficha.
- `MercadoDoJogador.jsx` — os dois blocos do fim da aba Mercado que só montam
  quando `detail.is_jogador`: quem está de olho no jogador e as vagas abertas do
  mundo com o veredito de elegibilidade. Vieram da seção Mercado da aba Carreira,
  apagada em 14/08/2026, e são a única parte da ficha que busca dado do MUNDO
  (`get_season_market_board`, `get_inbox_messages`) em vez de ler o `detail`. As
  duas buscas são `bestEffort`: falhar cai no estado vazio e deixa rastro no
  `loop.log`, sem derrubar o resto da aba.
- `primitivosDaFicha.jsx` — os tijolos que as seções dividem (`BlockLabel`,
  `Block`, `HeroBadge`, `DataRow`, `RankMarks`, `MotivationBar`, `MedalKey`,
  `MetricIcon`). Não são genéricos: cada um carrega uma decisão de composição
  desta ficha, e por isso não moram em `../../ui/`.
- `driverDetailV2Logic.js` — cor derivada de resultado, ordenação de traços,
  agrupamento de títulos, leitura do confronto, escala log e a tabela de tons.

Os testes seguem o mesmo corte: `DriverDetailModalV2.test.jsx` guarda cabeçalho,
temporada e perfil, e as fatias `*.historico`, `*.curvaDeCampeonato`, `*.rivais` e
`*.mercado` guardam as suas seções — todas entrando pela ficha. Os módulos
extraídos têm ainda um teste próprio (`RecentFormStrip.test.jsx`,
`MarketSection.test.jsx`, `MercadoDoJogador.test.jsx`, `primitivosDaFicha.test.jsx`)
que os monta **sozinhos**, sem store e sem o estado de abas: é o que prova que a
fronteira do corte é real. Por isso eles não importam o `driverDetailV2TestKit.jsx`,
que traz o modal inteiro junto — os dados que precisam são locais. O
`MercadoDoJogador.test.jsx` é o único que mocka o `invoke`, porque é o único módulo
daqui que busca dado; o `MarketSection.test.jsx` não precisa, já que sem
`is_jogador` os blocos dele não montam.

Os dados e atalhos das fatias que entram pela ficha estão em
`driverDetailV2TestKit.jsx` — os `vi.mock`, não: eles valem por arquivo.

O `*.mercado` carrega um preview da curva que **não** roda na suíte normal: ele
despeja o SVG num HTML no temp do sistema para se olhar o gráfico fora do jsdom.
Ligue com `LOOP_PREVIEW_CURVA=1` e leia o caminho no console.

O que continua valendo: os módulos compartilhados de `../detalhes/`
(`formatadores.js`, `primitivos.jsx`, `PlayerSkillSection.jsx`) também são
consumidos por fora daqui — `DriverMiniCard.jsx` depende de `cabecalho.jsx` e de
`formatadores.js`. Mudar o comportamento deles arrasta esses consumidores.

Guards estruturais sobre esta pasta: `scripts/tests/driver-detail-v2-frame-stability.test.mjs`,
`scripts/tests/driver-detail-tracos-eixos.test.mjs` e
`scripts/tests/curva-de-carreira-moldura-unica.test.mjs`.
