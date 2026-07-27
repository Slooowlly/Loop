# F2 — `formatLap` e paleta dos gráficos de corrida

**Área:** frontend · **Risco:** baixo · **Conflita com:** nada

## O que foi encontrado

Os componentes de gráfico de corrida e de overlay do iRacing não compartilham nada.
Cada um reimplementa a formatação de tempo de volta e redefine a paleta.

### `formatLap` — 6 definições independentes

- `src/components/race/LapTimeLineChart.jsx`
- `src/components/race/PaceDeltaChart.jsx`
- `src/components/race/RaceCharts.jsx`
- `src/components/race/RaceTelemetryCockpit.jsx`
- `src/components/iracing/PostRacePanel.jsx`
- `src/components/iracing/IracingConnectedOverlay.jsx`

### `PLAYER_COLOR` — 5 definições independentes

Mesmos arquivos, exceto `PaceDeltaChart.jsx`.

### Outras constantes de gráfico repetidas na mesma família de arquivos

`GRID` (7×), `AXIS_TICK` (7×), `YELLOW` (4×), `PALETTE` (4×), `GOOD`/`BAD` (3× cada),
`roundRect` (2×), `torch` (2×).

## Por que importa

É o caso mais mecânico da varredura e o de menor risco, mas o volume é alto: sete
cópias de `GRID`/`AXIS_TICK` querem dizer que ajustar o visual dos gráficos exige
sete edições e que uma delas vai ficar para trás. `formatLap` é pior que constante —
é lógica (mm:ss.mmm, tratamento de volta inválida/nula) replicada seis vezes.

Vale notar que dois desses arquivos (`IracingConnectedOverlay.jsx`) estão no meio de
alterações não commitadas no momento da varredura.

## Armadilhas conhecidas

1. **Overlay é outra webview.** `tauri.conf.json` declara três webviews servindo o
   mesmo `index.html`. Os componentes em `components/iracing/` e `src/overlay/` rodam
   nas janelas `overlay`/`engineer` (transparentes, always-on-top). Um módulo
   compartilhado precisa funcionar nos dois contextos — cuidado com import que
   arrasta o store da carreira para dentro do overlay.
2. **A paleta pode ser divergente de propósito.** O overlay roda sobre o iRacing
   em VR/fundo transparente; as cores que funcionam lá podem ser deliberadamente
   diferentes das do dashboard. Confirme antes de unificar.
3. **`scripts/tests/team-palette-distribution.test.mjs`** faz guard de paleta lendo
   o fonte como texto. Ela está com alterações não commitadas — leia o estado atual
   antes de mexer em qualquer constante de cor.

## O que eu quero da segunda análise

1. **Tabela comparativa das 6 `formatLap`.** São idênticas? Onde divergem (precisão,
   tratamento de `null`/`0`/negativo, separador), qual é o comportamento correto?
2. **As 5 `PLAYER_COLOR` são o mesmo hex?** E as 4 `PALETTE`, 7 `GRID`, 7
   `AXIS_TICK`? Liste os valores lado a lado.
3. **Dashboard e overlay devem compartilhar paleta?** Argumente pelos dois lados e
   recomende. Se a resposta for "não", o módulo compartilhado deve conter só
   `formatLap` e a geometria, não as cores.
4. **Onde mora o módulo novo.** `src/utils/` já tem `colors.js`, `categoryColors.js`,
   `teamColors.js` — cabe lá, ou merece um `src/components/race/chartShared.js`?
   Considere que o overlay importa de `src/overlay/` e `src/components/iracing/`.
5. **Plano em passos**, um commit por família de símbolo (`formatLap` → cores →
   geometria), com `npm run test:ui` e `npm run test:structure` entre eles.

Não aplique nada ainda — quero ler a análise antes.
