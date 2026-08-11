# Ficha do piloto — componentes do v2

A ficha do piloto (`DriverDetailModalV2.jsx`) e os blocos que só ela usa.

O nome "v2" é histórico: durante o redesenho o v1
(`src/components/driver/DriverDetailModal.jsx`) ficava um nível acima e servia de
rollback, e a regra era não editá-lo. O v1 foi removido em 11/08/2026 — esta é a
única ficha viva, e `src/components/driver/index.js` é apenas o ponto de entrada.

## O corte dos arquivos

O modal desenha a casca, as abas e os blocos de texto. O que é **gráfico** mora em
módulo irmão, e o que é **lógica pura** mora fora do JSX:

- `curvaDeCarreira.jsx` — a moldura que os dois gráficos de carreira dividem
  (colunas de categoria, hachura do ano fora do grid, fita de equipes, balões).
- `CurvaDeCampeonato.jsx` e `CurvaDeMercado.jsx` — as duas séries que vestem essa
  moldura: posição no campeonato contra a expectativa do carro, salário pago
  contra o de mercado.
- `FaixaDeConfronto.jsx` — o duelo com o rival: a faixa cheia do card e a
  miniatura de cada linha da lista.
- `driverDetailV2Logic.js` — cor derivada de resultado, ordenação de traços,
  agrupamento de títulos, leitura do confronto, escala log e a tabela de tons.

Os testes seguem o mesmo corte: `DriverDetailModalV2.test.jsx` guarda cabeçalho,
temporada e perfil, e as fatias `*.historico`, `*.curvaDeCampeonato`, `*.rivais` e
`*.mercado` guardam as suas seções. Os dados e atalhos que todas usam estão em
`driverDetailV2TestKit.jsx` — os `vi.mock`, não: eles valem por arquivo.

O que continua valendo: os módulos compartilhados de `../detalhes/`
(`formatadores.js`, `primitivos.jsx`, `PlayerSkillSection.jsx`) também são
consumidos por fora daqui — `DriverMiniCard.jsx` depende de `cabecalho.jsx` e de
`formatadores.js`. Mudar o comportamento deles arrasta esses consumidores.

Guards estruturais sobre esta pasta: `scripts/tests/driver-detail-v2-frame-stability.test.mjs`,
`scripts/tests/driver-detail-tracos-eixos.test.mjs` e
`scripts/tests/curva-de-carreira-moldura-unica.test.mjs`.
