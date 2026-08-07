import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, "..", "..");

const ler = (relativo) => readFile(path.join(projectRoot, relativo), "utf8");

const MOLDURA = "src/components/driver/v2/curvaDeCarreira.jsx";
// Os dois gráficos que dividem a moldura: a curva de mercado (aba Mercado) e a
// curva de campeonato (aba Histórico). Eles vivem no MESMO modal, a uma aba de
// distância, e qualquer desalinhamento entre os dois aparece lado a lado.
const CURVAS = [
  "src/components/driver/v2/DriverDetailModalV2.jsx",
  "src/components/driver/v2/CurvaDeCampeonato.jsx",
];

// A geometria da carreira mora num lugar só.
//
// As duas curvas desenham o mesmo quadro — 680x220, calha de 62 à esquerda,
// fita de equipes em y=192 — e é isso que faz a coluna de 2014 cair no mesmo
// pixel nos dois cartões. Uma segunda definição de qualquer uma dessas
// constantes não quebra nada: ela só faz os dois gráficos escorregarem um em
// relação ao outro, em silêncio, até alguém notar que as fitas não batem.
test("a geometria da curva de carreira tem uma definição só", async () => {
  const arquivos = await Promise.all([MOLDURA, ...CURVAS].map(ler));

  for (const constante of ["CURVA_GEO", "CURVA_FITA", "CURVA_CHIP", "CURVA_COLUNA", "CURVA_Y_ANOS"]) {
    const donos = arquivos.filter((fonte) =>
      new RegExp(`^(export )?const ${constante}\\s*=`, "m").test(fonte),
    );
    assert.equal(
      donos.length,
      1,
      `${constante} precisa existir só em ${MOLDURA}; ${donos.length} arquivo(s) a declaram`,
    );
  }

  const moldura = arquivos[0];
  assert.match(
    moldura,
    /^export const CURVA_GEO\s*=/m,
    `esperava que a geometria fosse exportada por ${MOLDURA}`,
  );
});

// As peças da moldura são componentes, não trechos copiados.
//
// O quadro tem umas 250 linhas de SVG comentado — colunas de categoria com
// rótulo de pé, hachura do ano fora do grid, réguas de troca, fita de equipes,
// alvos de hover. Duplicar isso para o segundo gráfico funcionaria hoje e
// divergiria no primeiro ajuste de espaçamento.
test("as duas curvas de carreira desenham a mesma moldura", async () => {
  for (const arquivo of CURVAS) {
    const fonte = await ler(arquivo);
    const importa = fonte.match(/import \{([\s\S]*?)\} from "\.\/curvaDeCarreira\.jsx"/);
    assert.ok(importa, `esperava que ${arquivo} importasse a moldura de ${MOLDURA}`);

    for (const peca of ["MolduraDeFundo", "MolduraDeRodape", "AlvosDeTemporada", "TrocaTooltip"]) {
      assert.ok(
        importa[1].includes(peca),
        `${arquivo} precisa usar <${peca}> da moldura compartilhada em vez de redesenhar o quadro`,
      );
    }
  }
});

// Onde a série pousa no eixo também é da moldura.
//
// Com o marcador no FIM da coluna do ano, cada linha precisa de um vértice de
// abertura na borda esquerda do plot — senão a coluna da estreia fica vazia. É
// uma regra de duas linhas, exatamente do tipo que alguém reescreve inline no
// segundo gráfico e esquece de aplicar no primeiro: os dois passariam a começar
// em anos diferentes, a uma aba de distância um do outro.
test("as duas curvas montam os vértices da série pela mesma régua", async () => {
  for (const arquivo of CURVAS) {
    const fonte = await ler(arquivo);
    const importa = fonte.match(/import \{([\s\S]*?)\} from "\.\/curvaDeCarreira\.jsx"/);
    assert.ok(
      importa[1].includes("verticesDaSerie"),
      `${arquivo} precisa montar os pontos da polyline com verticesDaSerie()`,
    );
    assert.ok(
      !/points=\{[^}]*\.map\(/.test(fonte),
      `${arquivo} não pode montar os pontos da polyline inline — a abertura do primeiro ano se perde`,
    );
  }
});

// A hachura é um `<pattern>`, e id de pattern é global ao documento.
//
// As duas curvas podem estar montadas ao mesmo tempo (o modal troca de aba sem
// desmontar), e dois patterns com ids diferentes seriam duas definições do mesmo
// vocabulário — uma delas ficaria para trás na primeira vez que alguém mexesse
// na listra.
test("a hachura do ano fora do grid é a mesma nos dois gráficos", async () => {
  const moldura = await ler(MOLDURA);
  assert.match(moldura, /export const HACHURA_SEM_VINCULO = "[^"]+"/);

  for (const arquivo of CURVAS) {
    const fonte = await ler(arquivo);
    assert.ok(
      !/id=\{?"?curva-[a-z-]*sem-contrato/.test(fonte),
      `${arquivo} não pode declarar a própria hachura — ela vem de <HachuraDefs>`,
    );
    assert.match(
      fonte,
      /<HachuraDefs \/>/,
      `${arquivo} precisa declarar a hachura pela peça compartilhada`,
    );
  }
});
