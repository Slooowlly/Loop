// O contrato da fala de quebra, lido como texto nos três lugares onde ele vive.
//
// Uma fala de quebra atravessa três camadas que não se conhecem: o Rust decide a ORDEM das
// peças, `engenheiro-pack.mjs` grava um `.wav` por chave, e `engenheiroVoz.js` decide quanto
// silêncio deixar entre uma e outra. Nada no build liga as três — uma peça que existe em duas
// delas compila, passa nos testes e falha na pista.
//
// O lado Rust↔catálogo já tem guard próprio (`quebra::tests`, que roda a matriz inteira contra
// `familia_quebra`). O que falta, e é o que este arquivo cobre, é o pé de disco: o `.opus`
// existe mesmo, e a tabela de pausas tem uma entrada para cada papel de peça.
//
// O pé de disco olha o `.opus`, e não o `.wav`: é o Opus que o app carrega e é o único dos dois
// que está no git. Conferir o WAV faria este guard passar na máquina de quem gerou o acervo e
// falhar em todo clone limpo — ou, pior, passar no CI enquanto o app fica mudo.

import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { duracaoOpus } from "../duracaoAudio.mjs";

const raiz = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const ler = (p) => fs.readFileSync(path.join(raiz, p), "utf8");

const catalogo = JSON.parse(ler("docs/engenheiro-catalogo.json"));
const voz = ler("src/lib/pausasDoRadio.js");
const quebraRs = ler("src-tauri/src/engenheiro/quebra.rs");
const nomesRs = ler("src-tauri/src/engenheiro/nomes.rs");
const tempoRs = ler("src-tauri/src/engenheiro/tempo_volta.rs");

const PREFIXOS = ["qb_", "nm_", "eq_", "ab_", "ap_", "co_", "conj_"];
const daQuebra = (c) => PREFIXOS.some((p) => c.startsWith(p));
const pecas = catalogo.filter((i) => daQuebra(i.chave));
const doNossoCarro = catalogo.filter((i) => i.chave.startsWith("meu_"));
const tempos = catalogo.filter((i) => /^t_\d+$/.test(i.chave));
const daClassificacao = catalogo.filter((i) => i.chave.startsWith("cl_"));

/// Duração de uma peça do acervo, em segundos. O leitor mora em `scripts/duracaoAudio.mjs`
/// porque `analise-radio.mjs` precisa do mesmo número.
const duracao = (relativo) => duracaoOpus(path.join(raiz, relativo));

test("a despedida cabe no tempo que o Rust declara para ela", () => {
  // ESTE é o guard que o desenho da classificação depende, e o modo de falha dele é o pior da
  // família inteira.
  //
  // A despedida é agendada pelo FIM: o observador começa a tocá-la quando falta
  // `DESPEDIDA_MAX_S` até a linha, para que ela ACABE pouco antes. Se uma peça for mais longa
  // que o declarado, o engenheiro atravessa a linha falando — o piloto entra na primeira curva
  // da volta lançada com alguém no ouvido, que é exatamente o que esta família não pode fazer.
  //
  // E o defeito é invisível no Rust: lá o número é uma constante que ninguém confere contra o
  // áudio. Regravar uma redação mais longa passa em tudo e quebra na pista.
  const rs = ler("src-tauri/src/engenheiro/classificacao.rs");
  const m = /const DESPEDIDA_MAX_S: f64 = ([\d.]+)/.exec(rs);
  assert.ok(m, "DESPEDIDA_MAX_S sumiu de classificacao.rs");
  const teto = Number(m[1]);

  const despedidas = daClassificacao.filter((i) => i.chave.startsWith("cl_despedida_"));
  assert.ok(despedidas.length >= 3, `só ${despedidas.length} despedidas no catálogo`);
  for (const { chave } of despedidas) {
    const rel = `src/assets/engenheiro/${chave}.opus`;
    assert.ok(fs.existsSync(path.join(raiz, rel)), `${chave} sem .opus`);
    const s = duracao(rel);
    assert.ok(
      s !== null && s <= teto,
      `${chave} dura ${s?.toFixed(2)}s e o Rust declara teto de ${teto}s — ` +
        `ela atravessaria a linha. Encurte a redação ou suba DESPEDIDA_MAX_S.`,
    );
  }
});

test("toda peça da classificação tem o .opus gravado em disco", () => {
  // Piso de extração. As outras famílias deste arquivo têm contagem exata (`pecas`, `tempos`,
  // `doNossoCarro`), que já as protege; a da classificação é a única filtrada por prefixo sem
  // âncora nenhuma. Renomear `cl_` do lado do Rust esvaziaria a lista e este teste passaria
  // conferindo o disco de zero peças — verde, e cego para a família inteira.
  assert.ok(
    daClassificacao.length >= 14,
    `só ${daClassificacao.length} peças 'cl_' no catálogo (piso 14, 17 hoje) — o prefixo mudou`,
  );
  const faltando = daClassificacao
    .map((i) => i.chave)
    .filter((c) => !fs.existsSync(path.join(raiz, "src/assets/engenheiro", `${c}.opus`)));
  assert.deepEqual(faltando, [], `peças sem .opus: ${faltando}`);
});

test("toda peça da quebra tem o .opus gravado em disco", () => {
  // O modo de falha sem este teste é SILÊNCIO: o Rust manda tocar `nm_wisniewski`, o arquivo
  // não está lá, `falarPecas` pula a peça sem erro, e o engenheiro anuncia uma quebra sem
  // dizer de quem. Indistinguível de o áudio estar desligado.
  const faltando = pecas
    .map((i) => i.chave)
    .filter((c) => !fs.existsSync(path.join(raiz, "src/assets/engenheiro", `${c}.opus`)));
  assert.deepEqual(faltando, [], `peças sem .opus: ${faltando.slice(0, 20)}`);
});

test("o catálogo tem as peças da quebra na proporção medida", () => {
  const conta = (p) => pecas.filter((i) => i.chave.startsWith(p)).length;
  assert.equal(conta("nm_"), 355, "sobrenomes");
  assert.equal(conta("eq_"), 102, "equipes");
  // 12 peças (11 + genérica) × 3 severidades × 3 variações, MAIS os 9 trechos plurais da
  // fusão de duas quebras simultâneas.
  assert.equal(conta("qb_"), 108 + 9, "trechos");
  // 5 aberturas + 3 apostos + 3 codas de ganho + 3 de atrito.
  assert.equal(conta("ab_") + conta("ap_") + conta("co_"), 14, "enquadramento");
  assert.equal(conta("conj_"), 1, "a conjunção da fusão");
  assert.equal(pecas.length, 589);
});

test("a conjunção da fusão está gravada e é audível", () => {
  // ESTA peça quase entrou muda. `"e,"` é uma letra só, e a Chirp 3 devolveu 0,20 s com pico
  // 0,008 — 200 OK com silêncio dentro — de forma reprodutível. O gerador ganhou piso de pico
  // e de duração por causa dela; este teste é o segundo par de olhos, do lado do disco.
  const f = path.join(raiz, "src/assets/engenheiro", "conj_e.opus");
  assert.ok(fs.existsSync(f), "a conjunção não foi gravada");
  // Pela DURAÇÃO, não pelo tamanho em bytes. Em WAV os dois eram a mesma medida; em Opus o
  // tamanho depende do que o codec achou do conteúdo, e uma tomada picada — justamente a que
  // este teste procura — comprime BEM. O byte mentiria a favor do defeito.
  const s = duracaoOpus(f);
  assert.ok(s !== null && s > 0.2, `a conjunção dura ${s?.toFixed(2)}s — tomada picada`);
});

test("o nosso carro tem aviso e desfecho para toda peça, mais os 3 conselhos", () => {
  // 12 peças × 3 redações de AVISO ("estou ouvindo algo estranho no seu motor"), mais as mesmas
  // 12 × 3 severidades × 3 redações de DESFECHO ("acabou por hoje com problema no seu motor"),
  // mais os 3 conselhos de poupar.
  //
  // O desfecho existe porque a quebra do carro do JOGADOR saía pelo rádio da grade, em 3ª
  // pessoa — "o piloto um da tal equipe abandonou", sobre ele mesmo, 1,3 vez por corrida.
  //
  // Mais as 5 da CLASSIFICAÇÃO (`meu_quali_*`): o veredito da batida na quali e o motivo do
  // castigo na largada. Família fixa, sem variação — cada uma sai no máximo uma vez por fim
  // de semana, com o carro parado.
  assert.equal(doNossoCarro.length, 12 * 3 + 12 * 3 * 3 + 3 + 5);
  const faltando = doNossoCarro
    .map((i) => i.chave)
    .filter((c) => !fs.existsSync(path.join(raiz, "src/assets/engenheiro", `${c}.opus`)));
  assert.deepEqual(faltando, [], `peças sem .opus: ${faltando}`);
});

test("o aviso do nosso carro é em 2ª pessoa e nunca diz o número do desgaste", () => {
  // O engenheiro OUVE, não lê sensor. Um número entregaria ao jogador uma leitura que o
  // personagem não tem — e trocaria uma decisão de pilotagem por uma conta. E a pessoa
  // gramatical é o que distingue "o seu carro" de "o carro de alguém": errada, o aviso mais
  // urgente do rádio soa como fofoca sobre outro piloto.
  for (const { chave, texto } of doNossoCarro) {
    assert.ok(!/\d/.test(texto), `${chave} tem número: "${texto}"`);
    if (chave.startsWith("meu_poupar")) continue; // o conselho fala do carro sem possessivo
    if (chave.startsWith("meu_quali")) {
      // A família da classificação fala do FIM DE SEMANA, não de uma peça — "largamos em
      // último", "nosso fim de semana acabou aqui" —, então o possessivo não cabe. A pessoa
      // gramatical continua valendo, e é ela que este guard cobra: ou o "você", ou o nós, que
      // em português mora na DESINÊNCIA do verbo e não num pronome.
      const baixo = texto.toLowerCase();
      const comOPiloto = ["você", "tenta", "seu ", "sua ", "nosso", "a gente"].some((m) =>
        baixo.includes(m),
      );
      assert.ok(
        comOPiloto || /\b\w{2,}mos\b/.test(baixo),
        `${chave} não fala COM o piloto: "${texto}"`,
      );
      continue;
    }
    assert.match(texto, /\b(seu|sua|seus|suas)\b/, `${chave} não está na 2ª pessoa: "${texto}"`);
  }
});

test("nenhum trecho gravado tem pontuação no meio", () => {
  // MEDIDO: das 576 peças, as 13 que tinham travessão saíram com 0,35 s de silêncio DENTRO
  // da fala, contra 0,01 s das outras. Numa fala montada por colagem essa pausa soma com a da
  // emenda e o ouvinte escuta a frase quebrando — que soa como rádio falhando.
  //
  // O guard vive aqui, e não só no Rust, porque o defeito é do ARQUIVO: alguém pode reescrever
  // a redação no Rust e esquecer de apagar o `.wav` antigo, e aí o teste do Rust passa
  // enquanto o disco continua com a tomada partida.
  const sujas = pecas
    .filter((i) => i.chave.startsWith("qb_"))
    .filter((i) => /[—;]|,/.test(i.texto.replace(/\.$/, "")));
  assert.deepEqual(sujas.map((i) => i.chave), [], `trechos com pausa interna: ${sujas.map((i) => i.texto)}`);
});

test("as peças de continuação terminam em vírgula e as de fecho em ponto", () => {
  // A pontuação NÃO é decorativa: ela é o que dá à peça a entoação de continuar ou de
  // encerrar. Sobrenome gravado com ponto sai com cara de fim de frase, e colar isso num
  // verbo produz duas orações onde deveria haver uma.
  for (const { chave, texto } of pecas) {
    if (chave.startsWith("ab_piloto")) {
      assert.ok(texto.endsWith(" da"), `${chave}: "${texto}" devia emendar no artigo`);
    } else if (
      chave.startsWith("nm_") ||
      chave.startsWith("eq_") ||
      chave.startsWith("ab_") ||
      chave.startsWith("ap_") ||
      // A conjunção da fusão emenda dois nomes: `"Cooper," + "e," + "Bianchi,"`. Ela pede
      // continuação nos dois lados, como qualquer peça do meio.
      chave.startsWith("conj_")
    ) {
      assert.ok(texto.endsWith(","), `${chave}: "${texto}" devia pedir continuação`);
    } else {
      assert.ok(texto.endsWith("."), `${chave}: "${texto}" devia encerrar`);
    }
  }
});

test("a tabela de pausas cobre todo papel de peça que o Rust emite", () => {
  // `pausasDoRadio` decide por PREFIXO de chave. Um papel novo no Rust sem entrada aqui cai
  // no ramo padrão — e o padrão é a pausa de dentro da oração, a mais curta. Um fecho de frase
  // colado com 60 ms soa como atropelo.
  const bloco = /export function pausasDoRadio\(chaves\) \{([\s\S]*?)\n\}/.exec(voz);
  assert.ok(bloco, "não achei pausasDoRadio em src/lib/pausasDoRadio.js");
  for (const papel of ["ab_piloto", "ab_", "ap_", "co_", "camp_", "mem_", "voc_"]) {
    assert.ok(bloco[1].includes(`"${papel}"`), `pausasDoRadio não trata ${papel}`);
  }
  // Os quatro valores da audição, com nome. Se um sumir, a derivação perdeu um tipo de
  // fronteira e nada mais reclama.
  const juncao = /const JUNCAO = \{([^}]*)\}/.exec(voz);
  assert.ok(juncao, "não achei a tabela JUNCAO");
  for (const nome of ["artigo", "oracao", "virgula", "frase"]) {
    assert.match(juncao[1], new RegExp(`${nome}:\\s*\\d+`), `JUNCAO sem ${nome}`);
  }
});

test("os prefixos de chave são os mesmos nos dois lados", () => {
  // O JS procura por prefixo literal; o Rust os declara em `const`. Renomear de um lado só
  // deixaria a fala tocando com as pausas erradas, sem erro nenhum.
  assert.match(quebraRs, /pub const PREFIXO: &str = "qb_"/);
  assert.match(nomesRs, /pub const PREFIXO_SOBRENOME: &str = "nm_"/);
  assert.match(nomesRs, /pub const PREFIXO_EQUIPE: &str = "eq_"/);
  assert.match(tempoRs, /pub const PREFIXO_TEMPO: &str = "t_"/);
  assert.match(
    ler("src-tauri/src/engenheiro/tratamento.rs"),
    /pub const PREFIXO: &str = "voc_"/,
  );
});

// ─── Tempo de volta ──────────────────────────────────────────────────────────

test("a faixa de tempo de volta está gravada inteira, sem buraco", () => {
  // Um décimo faltando no meio da faixa é o defeito mais silencioso do acervo: o engenheiro
  // emudece só naquele valor, uma vez a cada mil voltas, e ninguém liga uma coisa à outra.
  const faltando = [];
  for (let d = 300; d <= 2400; d += 1) {
    if (!fs.existsSync(path.join(raiz, "src/assets/engenheiro", `t_${d}.opus`))) faltando.push(d);
  }
  assert.deepEqual(faltando, [], `${faltando.length} tempos sem .opus (ex.: ${faltando.slice(0, 8)})`);
  assert.equal(tempos.length, 2101, "o catálogo mudou de faixa");
});

test("o tempo é dito como o rádio diz, com o zero do segundo de um dígito", () => {
  const m = Object.fromEntries(catalogo.map((i) => [i.chave, i.texto]));
  assert.equal(m.t_453, "quarenta e cinco e três.");
  assert.equal(m.t_924, "um trinta e dois e quatro.");
  // "um nove e zero" ficaria a um passo de soar como 1,9 s.
  assert.equal(m.t_690, "um zero nove e zero.");
  assert.equal(m.t_2400, "quatro zero zero e zero.");
});

test("nenhum tempo de volta tem pontuação no meio", () => {
  // O tempo é a peça mais FUNDIDA do acervo — foi medido que partir o minuto estraga a fala.
  // Uma vírgula no texto reintroduziria a mesma pausa pela porta dos fundos.
  const sujos = tempos.filter((i) => /[,;—]/.test(i.texto.replace(/\.$/, "")));
  assert.deepEqual(sujos.map((i) => i.chave), []);
});

test("a fala da volta mais rápida tem lead, e ele emenda no nome sem ponto", () => {
  const m = Object.fromEntries(catalogo.map((i) => [i.chave, i.texto]));
  // O lead termina EM ABERTO porque o sobrenome vem logo depois. Um ponto aqui daria a ele
  // entoação de fim de frase e o nome sairia como uma oração solta.
  assert.ok(m.tv_melhor_e_do, "falta o lead da volta mais rápida");
  assert.ok(!m.tv_melhor_e_do.endsWith("."), `"${m.tv_melhor_e_do}" não devia encerrar`);
  assert.ok(m.tv_volta_em.endsWith(","), `"${m.tv_volta_em}" devia pedir continuação`);
  for (const chave of ["tv_faltam_1", "tv_faltam_9", "tv_tomamos", "tv_tomamos_3"]) {
    assert.ok(m[chave], `falta ${chave}`);
    assert.match(m[chave], /[.!]$/, `${chave} devia encerrar: "${m[chave]}"`);
  }
  // Concordância: um décimo é singular.
  assert.match(m.tv_faltam_1, /^Falta um décimo/);
  assert.match(m.tv_faltam_2, /^Faltam dois décimos/);
});

test("a junção nome→tempo é de vírgula, não de oração", () => {
  // "…é do Cooper, um trinta e dois e quatro." — a vírgula é do texto. Sem a regra, o nome
  // cairia na pausa curta de dentro da oração e o tempo entraria atropelando o sobrenome.
  const bloco = /export function pausasDoRadio\(chaves\) \{([\s\S]*?)\n\}/.exec(voz);
  assert.ok(bloco[1].includes('proxima.startsWith("t_")'), "pausasDoRadio não trata nome→tempo");
});
