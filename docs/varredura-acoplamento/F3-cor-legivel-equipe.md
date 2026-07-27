# F3 — `getReadableTeamColor` triplicado com fallbacks divergentes

**Área:** frontend · **Risco:** médio (visual, e há guard estrutural de paleta) · **Conflita com:** nada

## O que foi encontrado

Três cópias quase idênticas da função que clareia cor de equipe escura para ficar
legível sobre fundo escuro. Cada uma com um fallback diferente, e uma com o fator de
mistura diferente.

| Arquivo | Linha | Fallback (cor inválida) | `mixWithWhite` |
|---|---|---|---|
| [`components/race/raceGridContext.js`](../../src/components/race/raceGridContext.js) | 16 | `#58a6ff` (azul) | 0.58 |
| [`components/standings/standingsFormatting.js`](../../src/components/standings/standingsFormatting.js) | 13 | `#7d8590` (cinza) | 0.58 |
| [`pages/tabs/newsHelpers.js`](../../src/pages/tabs/newsHelpers.js) | 4 | `#d0d7e2` (cinza claro) | **0.62** |

O resto é byte-a-byte igual: mesma regex `^#([0-9a-f]{6})$`, mesma luminância
`0.2126r + 0.7152g + 0.0722b`, mesmo limiar `0.32`.

Não existe versão canônica: `src/utils/teamColors.js` só exporta `getTeamGlow`.

## Por que importa

As divergências parecem **intencionais** — cada tela tem um fallback que combina com
seu fundo, e a revista de notícias clareia um pouco mais (0.62) porque o fundo dela é
mais claro. Ou seja, isso não é copy-paste burro; é uma função com dois parâmetros que
nunca foram parametrizados.

O risco de unificar sem cuidado é achatar as três num só comportamento e mudar o
visual de três telas de uma vez.

## Armadilhas conhecidas

1. **`scripts/tests/team-palette-distribution.test.mjs`** é um guard estrutural de
   paleta que lê o fonte como texto — e está com alterações não commitadas no
   momento da varredura. Leia o estado atual dela antes de mexer.
2. O comentário em `newsHelpers.js` diz que é o "único sobrevivente do antigo módulo
   de helpers da NewsTab (aposentada)". Confirme que `NewsMagazineTab` é o único
   consumidor — se for, talvez o destino certo seja mover a função para lá, não
   unificar.
3. `#58a6ff` também é o fallback de `getCategoryColor` em
   `utils/categoryColors.js:14`. Pode ser coincidência ou pode indicar que
   `raceGridContext` copiou o default errado.

## O que eu quero da segunda análise

1. **Confirme o diff.** Rode o comparativo você mesmo e confirme que a única
   diferença são fallback e `mixWithWhite`. Se houver mais, liste.
2. **As divergências são intencionais?** Investigue o histórico (`git log -p` nos
   três arquivos) e o contexto visual de cada tela. Quero saber se 0.62 na revista
   foi decisão deliberada ou drift.
3. **Assinatura da função unificada.** Proponha uma —
   `getReadableTeamColor(color, { fallback, mix })`? — e diga qual deve ser o
   default. Um default errado transforma um refactor em mudança visual silenciosa.
4. **Onde ela mora.** `utils/teamColors.js` é o candidato natural (já tem
   `getTeamGlow`), mas confirme que importar de `utils/` não cria ciclo com
   `raceGridContext.js`, que já importa de `pages/tabs/nextRaceBriefing`.
5. **Como provar que nada mudou visualmente.** Existe teste que cubra isso? Se não,
   proponha um teste de tabela (entrada → saída esperada) para as três telas antes
   do refactor, para servir de trava.

Não aplique nada ainda — quero ler a análise antes.
