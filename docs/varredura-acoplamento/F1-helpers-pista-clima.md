# F1 — Helpers de pista e clima duplicados

**Área:** frontend · **Risco:** médio (mexe em visual de 3 telas) · **Conflita com:** nada

## O que foi encontrado

`Header.jsx` reimplementa localmente um conjunto de helpers de pista/clima que já
existem em `src/utils/`. E os próprios utils se duplicam entre si.

### Definições locais em `Header.jsx`

| Linha | Função | Já existe em |
|---|---|---|
| [640](../../src/components/layout/Header.jsx) | `getTrackImageSrc(trackName)` | `utils/trackImages.js:51` e `utils/calendarShared.js:136` |
| [737](../../src/components/layout/Header.jsx) | `getBannerImageSrc(trackName)` | — (só aqui, mas usa a mesma normalização) |
| [757](../../src/components/layout/Header.jsx) | `getBannerImageFocus(trackName)` | — (idem) |
| [766](../../src/components/layout/Header.jsx) | `normalizeTrackName(trackName)` | `utils/trackImages.js`, `utils/calendarShared.js` |
| [773](../../src/components/layout/Header.jsx) | `weatherLabel(value)` | `utils/calendarShared.js:114` |
| [780](../../src/components/layout/Header.jsx) | `weatherEmoji(value)` | — (verificar se há equivalente) |
| [867](../../src/components/layout/Header.jsx) | `trackCountry(trackName)` | `utils/trackCountries.js:5` expõe `TRACK_COUNTRIES` |

### Duplicação entre os próprios utils

`weatherLabel` existe em **quatro** lugares:
- `src/utils/calendarShared.js:114`
- `src/components/race/raceresult/helpers.js:4`
- `src/components/race/RaceResultViewV2.jsx:40`
- `src/components/layout/Header.jsx:773`

`getTrackImageSrc` existe em **duas assinaturas incompatíveis**:
- `src/utils/trackImages.js:51` → `getTrackImageSrc(trackName, trackId)`
- `src/utils/calendarShared.js:136` → `getTrackImageSrc(race)` — recebe o objeto de corrida

`normalizeTrackName` em três: `Header.jsx:766`, `calendarShared.js`, `trackImages.js`.

## Por que importa

Não é só estética. `normalizeTrackName` é a chave de lookup do arquivo de imagem —
três normalizações independentes significam que uma pista nova pode aparecer no
calendário e sumir no header (ou vice-versa) dependendo de qual variante trata o
caso. Mesma coisa com `weatherLabel`: quatro tabelas de tradução de clima que podem
divergir na próxima condição climática adicionada.

## Armadilhas conhecidas

1. **i18n.** `weatherLabel` devolve prosa em PT. O projeto tem hook de pre-commit
   ([.githooks/pre-commit](../../.githooks/pre-commit)) que bloqueia string de UI em
   português fora de `t()` em `.jsx`. Ao mover a função entre arquivos, confira se
   as chaves i18n vão junto e se `localeParity.test.js` continua passando.
2. **Suíte estrutural.** `scripts/tests/` tem guards que leem o código-fonte como
   texto (alinhamento, acentuação de copy PT, encoding). Rode
   `npm run test:structure` — ela pode reclamar de coisas que um refactor "óbvio"
   quebra.
3. **As variantes podem não ser idênticas.** Não presuma. `Header.jsx` tem
   `getBannerImageSrc` + `getBannerImageFocus` que sugerem que o header usa um
   conjunto de imagens *diferente* do calendário (banner panorâmico vs thumbnail).
   Se for o caso, a normalização é compartilhável mas o resolver de arquivo não é.

## O que eu quero da segunda análise

Antes de escrever qualquer código, quero um relatório que responda:

1. **Diff semântico real.** Para cada um dos 4 `weatherLabel`, das 3
   `normalizeTrackName` e das 2+1 `getTrackImageSrc`: as implementações são
   equivalentes? Onde divergem, a divergência é intencional (contrato diferente) ou
   drift acidental? Mostre o diff.
2. **Qual é o dono certo.** `calendarShared.js` deveria delegar para
   `trackImages.js`, ou os dois deveriam virar um só módulo? A assinatura
   `getTrackImageSrc(race)` é um wrapper de conveniência que vale manter, ou um
   acidente?
3. **`Header.jsx` usa mesmo o mesmo conjunto de imagens?** Confirme lendo
   `getBannerImageSrc`/`getBannerImageFocus` e o diretório de assets. Se o header
   tem assets próprios, diga o que é compartilhável (normalização, país, clima) e o
   que não é (resolver de arquivo).
4. **Plano de unificação em passos verificáveis**, com o comando de teste a rodar
   depois de cada passo. Prefiro 4 commits pequenos a 1 grande.
5. **Cobertura de teste hoje.** Existe teste que travaria uma regressão de
   normalização de nome de pista? Se não, o primeiro passo do plano deveria ser
   escrever esse teste.

Não aplique nada ainda — quero ler a análise antes.
