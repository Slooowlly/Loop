// A ARBITRAGEM do canal do engenheiro, exercida com áudio de mentira.
//
// O engenheiro ganhou três bocas — quebra na grade, peça do nosso carro e volta mais rápida —
// e continua com uma voz. Quem decide quem fala primeiro é este módulo, e o modo de falha dele
// não parece falha: a frase sai pela metade, com a voz confiante de sempre, e o jogador
// entende que perdeu alguma coisa sem saber o quê.
//
// Por isso os casos aqui são quase todos sobre ORDEM. O caminho feliz — uma fala sozinha no
// canal — nunca foi o problema.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("./filtroRadio", () => ({
  criarCadeiaRadio: () => ({ entrada: { connect() {} } }),
}));

// O REGISTRO DO RÁDIO, capturado em memória. Fora do shell ele já sairia calado (`estaNoTauri`
// é falso em jsdom), e um dublê calado não deixa provar o que foi escrito — que é justamente o
// que a abertura e o fim de uma sequência precisam demonstrar.
const registros = vi.hoisted(() => []);
vi.mock("./radioRegistro", () => ({
  registrar: (canal, opcoes = {}) => registros.push({ canal, ...opcoes }),
  estaLigado: () => true,
  ligar: () => {},
}));

// O spotter fica calado o tempo todo: o que se testa aqui é engenheiro contra engenheiro. A
// cessão ao spotter é outro mecanismo, e ele já existia antes desta fila.
vi.mock("./spotterVoice", () => ({
  aoFalar: () => () => {},
  estaFalando: () => false,
}));

/** Contexto de áudio de mentira: cada fonte "toca" por `duracaoMs` e avisa quem terminou. */
function montarAudioFalso(duracaoMs = 20) {
  const tocadas = [];
  const emCurso = new Set();
  class FonteFalsa {
    constructor() {
      this.buffer = null;
      this.onended = null;
      this._timer = null;
    }
    connect() {}
    start() {
      emCurso.add(this);
      tocadas.push(this.buffer?.chave ?? "?");
      this._timer = setTimeout(() => {
        emCurso.delete(this);
        this.onended?.();
      }, duracaoMs);
    }
    stop() {
      if (!emCurso.has(this)) return;
      emCurso.delete(this);
      clearTimeout(this._timer);
      this.onended?.();
    }
  }
  const ctx = {
    state: "running",
    destination: {},
    resume: async () => {},
    createGain: () => ({ gain: { value: 1 }, connect() {} }),
    createBufferSource: () => new FonteFalsa(),
    // `sampleRate` e `length` importam desde que o nivelamento passou a medir a fala em
    // janelas de 10 ms: sem taxa não há janela, e sem janela não há gate.
    decodeAudioData: async (bytes) => ({ chave: String(bytes), length: 1000, sampleRate: 48000 }),
  };
  return { ctx, tocadas };
}

let voz;
let tocadas;

beforeEach(async () => {
  vi.resetModules();
  const falso = montarAudioFalso();
  tocadas = falso.tocadas;
  window.AudioContext = function () { return falso.ctx; };
  localStorage.setItem("loop.engenheiroVoz", "1");
  // `carregar` busca a URL do glob e decodifica; o "buffer" é só a chave, para o teste saber
  // o que tocou e em que ordem.
  globalThis.fetch = vi.fn(async (url) => ({
    arrayBuffer: async () => String(url).split("/").pop().replace(/\.opus.*$/, ""),
  }));
  voz = await import("./engenheiroVoz");
  // O timeout generoso não é folga: o módulo resolve um `import.meta.glob` eager sobre os
  // 3.943 `.opus` do acervo, e o `resetModules` faz isso a cada caso. É o preço de testar o
  // módulo de verdade em vez de uma cópia da lógica dele.
}, 60000);

afterEach(() => {
  vi.useRealTimers();
  delete window.OfflineAudioContext;
});

/**
 * Contexto offline de mentira que devolve `amostras` como se fossem o render da cadeia.
 * O que se testa não é a cadeia — é o que `falarRemoto` FAZ com o que ela devolve.
 */
function renderFalso(amostras) {
  window.OfflineAudioContext = function () {
    this.destination = {};
    this.createBufferSource = () => ({ buffer: null, connect() {}, start() {} });
    this.startRendering = async () => ({
      chave: "render",
      getChannelData: () => amostras,
    });
  };
}

const rmsDe = (x) => Math.sqrt(x.reduce((s, v) => s + v * v, 0) / x.length);
const picoDe = (x) => x.reduce((p, v) => Math.max(p, Math.abs(v)), 0);

/** Espera o microtask/timer girar o suficiente para as falas acabarem. */
const respirar = (ms = 200) => new Promise((r) => setTimeout(r, ms));

describe("anúncio contra anúncio", () => {
  it("o segundo anúncio ESPERA o primeiro em vez de cortá-lo", async () => {
    // ESTE é o defeito que a fila existe para consertar. Com `falarPecas` nos dois, o segundo
    // chamaria `pararFonte()` e o primeiro morreria no meio da palavra — e a fala de quebra
    // chega a sete segundos, então a sobreposição não é hipótese remota.
    const a = voz.anunciar(["tv_melhor_e_do", "nm_cooper"]);
    const b = voz.anunciar(["qb_dnf_engine_0"]);
    expect(await a).toBe(true);
    expect(await b).toBe(true);
    expect(tocadas).toEqual(["tv_melhor_e_do", "nm_cooper", "qb_dnf_engine_0"]);
  });

  it("três anúncios saem inteiros e na ordem em que chegaram", async () => {
    const todos = await Promise.all([
      voz.anunciar(["pos_2"]),
      voz.anunciar(["pos_3"]),
      voz.anunciar(["pos_4"]),
    ]);
    expect(todos).toEqual([true, true, true]);
    expect(tocadas).toEqual(["pos_2", "pos_3", "pos_4"]);
  });
});

describe("pergunta contra anúncio", () => {
  it("a resposta ao piloto CORTA o anúncio em curso", async () => {
    // A assimetria é a regra do produto: quem anuncia espera, quem responde corta. O piloto
    // apertou o botão; segurá-lo atrás de uma notícia seria não atender.
    const anuncio = voz.anunciar(["qb_dnf_engine_0", "co_otima"]);
    await respirar(5);
    const resposta = voz.falarPecas(["pos_5"]);
    expect(await anuncio).toBe(false);
    expect(await resposta).toBe(true);
    expect(tocadas).toContain("pos_5");
  });

  it("uma pergunta ESVAZIA a fila de anúncios represados", async () => {
    // Despejar três notícias velhas antes da resposta é o oposto de atender — e elas já eram
    // velhas quando a pergunta chegou.
    const presos = [voz.anunciar(["pos_2"]), voz.anunciar(["pos_3"]), voz.anunciar(["pos_4"])];
    await respirar(5);
    await voz.falarPecas(["pos_5"]);
    await respirar(60);
    const resultados = await Promise.all(presos);
    expect(resultados.slice(1)).toEqual([false, false]);
    expect(tocadas).not.toContain("pos_4");
  });

  it("o anúncio atropelado NÃO deixa o canal travado", async () => {
    // O defeito mais caro do desenho: se o anúncio moribundo soltasse o canal (ou não o
    // soltasse), a fila travaria e o rádio emudeceria de vez depois de um push-to-talk —
    // sintoma que ninguém ligaria à causa.
    const anuncio = voz.anunciar(["pos_2", "pos_3"]);
    await respirar(5);
    await voz.falarPecas(["pos_5"]);
    await anuncio;
    await respirar(60);

    tocadas.length = 0;
    expect(await voz.anunciar(["pos_9"])).toBe(true);
    expect(tocadas).toEqual(["pos_9"]);
  });
});

describe("descarte", () => {
  it("anúncio parado além da validade é descartado em vez de virar notícia velha", async () => {
    vi.useFakeTimers();
    const primeiro = voz.anunciar(["qb_dnf_engine_0", "co_otima"]);
    const atrasado = voz.anunciar(["tv_tomamos"]);
    await vi.advanceTimersByTimeAsync(5); // o primeiro já saiu da fila e está tocando
    // Meio minuto depois, a fala represada não é mais novidade: o jogador já passou por aquilo.
    vi.setSystemTime(Date.now() + 30000);
    await vi.advanceTimersByTimeAsync(500);
    expect(await primeiro).toBe(true);
    expect(await atrasado).toBe(false);
    expect(tocadas).not.toContain("tv_tomamos");
    expect(voz.anunciosDescartados()).toBeGreaterThan(0);
  });

  it("a fila tem teto e o excedente é contado, não engolido em silêncio", async () => {
    // Sem o contador, "o rádio parece calmo" e "o rádio está engolindo metade das falas" são
    // indistinguíveis — e é justamente esse número que a medição do rádio inteiro precisa.
    const antes = voz.anunciosDescartados();
    const pedidos = [];
    for (let i = 0; i < 12; i += 1) pedidos.push(voz.anunciar([`t_${300 + i}`]));
    const resultados = await Promise.all(pedidos);
    expect(resultados.filter((r) => r === false).length).toBeGreaterThan(0);
    expect(voz.anunciosDescartados()).toBeGreaterThan(antes);
  });
});

describe("o portão do momento", () => {
  // Largada, duelo e última volta. A fila de anúncios espera a pista esfriar; a resposta ao
  // piloto e o spotter não passam por aqui.

  it("segura o anúncio enquanto a pista está quente e solta quando esfria", async () => {
    let quente = "Duelo";
    voz.definirPortao(() => quente);

    const a = voz.anunciar(["pos_2"]);
    await respirar(60);
    expect(tocadas).toEqual([]); // ainda calado

    quente = null;
    expect(await a).toBe(true);
    expect(tocadas).toEqual(["pos_2"]);
  });

  it("com a pista fria o anúncio sai na hora, sem esperar o relógio do portão", async () => {
    voz.definirPortao(() => null);
    expect(await voz.anunciar(["pos_3"])).toBe(true);
    expect(tocadas).toEqual(["pos_3"]);
  });

  it("um portão que explode deixa o rádio FALAR — calar por engano é pior", async () => {
    voz.definirPortao(() => {
      throw new Error("sem telemetria");
    });
    expect(await voz.anunciar(["pos_4"])).toBe(true);
    expect(tocadas).toEqual(["pos_4"]);
  });

  it("uma disputa longa faz o anúncio VENCER em vez de travar a fila", async () => {
    // O teto da espera é a validade do próprio anúncio: passado o prazo, quem descarta é a
    // regra que já existia. Sem o teto, um duelo de meia volta prenderia a fila para sempre
    // e o rádio emudeceria sem nada indicar por quê.
    vi.useFakeTimers();
    voz.definirPortao(() => "Duelo");
    const a = voz.anunciar(["pos_5"]);
    await vi.advanceTimersByTimeAsync(21000);
    expect(await a).toBe(false);
    expect(tocadas).toEqual([]);
    expect(voz.anunciosDescartados()).toBeGreaterThan(0);
  });

  it("uma PERGUNTA do piloto atravessa a espera do portão", async () => {
    // A fila esvazia quando o piloto fala. O anúncio que estava esperando o momento tem de
    // sair de cena junto — se ele voltasse depois, a notícia sairia atrás da resposta que a
    // tornou irrelevante.
    voz.definirPortao(() => "Duelo");
    const a = voz.anunciar(["pos_6"]);
    await respirar(30);
    const resposta = voz.falarPecas(["pos_7"]);
    expect(await a).toBe(false);
    expect(await resposta).toBe(true);
    expect(tocadas).toEqual(["pos_7"]);
  });
});

describe("a fala longa dos momentos sem pressa", () => {
  it("ESPERA a vez em vez de cortar — ninguém pediu por ela", async () => {
    // A diferença toda entre `anunciarRemoto` e `falarRemoto`. Lá o piloto perguntou e a
    // resposta corta; aqui o engenheiro abriu o rádio sozinho, e quem não foi pedido espera.
    renderFalso(new Float32Array(1000).fill(0.3));
    const a = voz.anunciar(["pos_2"]);
    const b = voz.anunciarRemoto("QUJD", "audio/mpeg");
    expect(await a).toBe(true);
    expect(await b).toBe(true);
    expect(tocadas).toEqual(["pos_2", "render"]);
  });

  it("uma PERGUNTA do piloto a descarta, como qualquer outra notícia", async () => {
    renderFalso(new Float32Array(1000).fill(0.3));
    voz.definirPortao(() => "Duelo");
    const longa = voz.anunciarRemoto("QUJD", "audio/mpeg");
    await respirar(30);
    const resposta = voz.falarPecas(["pos_7"]);
    expect(await longa).toBe(false);
    expect(await resposta).toBe(true);
    expect(tocadas).toEqual(["pos_7"]);
  });

  it("sem áudio não entra na fila", async () => {
    expect(await voz.anunciarRemoto("", "audio/mpeg")).toBe(false);
    expect(tocadas).toEqual([]);
  });
});

describe("o registro da sequência começa na peça que tocou", () => {
  // A peça que não existe no disco é o modo de falha da família: o `.opus` some do pacote,
  // o app pede um arquivo inexistente e a sequência segue para a próxima sem erro nenhum. A
  // fala sai, o jogador ouve — e o registro do rádio, que ancorava a abertura no índice 0,
  // não escrevia linha nenhuma. Ficava idêntico no arquivo a uma fala que nunca saiu, que é
  // justamente a pergunta que o registro existe para responder.
  //
  // Os dois casos vivem num `it` só de propósito: o `beforeEach` deste arquivo reindexa as
  // 3.943 peças do acervo a cada caso, e são ~7 s por vez. Um caso a mais aqui custa mais do
  // que a separação vale.
  const AUSENTE = "peca_inexistente_no_disco";

  it("abre e fecha o registro quando alguma peça tocou, e cala quando nenhuma tocou", async () => {
    registros.length = 0;
    expect(await voz.falarPecas([AUSENTE, "pos_5"], { canal: "resposta" })).toBe(true);
    // A peça que faltou não soou; a que existe, sim.
    expect(tocadas).toEqual(["pos_5"]);
    expect(registros.filter((r) => r.canal === "resposta").map((r) => r.desfecho)).toEqual([
      "ok",
      "fim",
    ]);

    // Nenhuma peça no disco: nada soou, e aí o silêncio no registro é a verdade.
    registros.length = 0;
    tocadas.length = 0;
    await voz.falarPecas([AUSENTE, `${AUSENTE}_2`], { canal: "resposta" });
    expect(tocadas).toEqual([]);
    expect(registros.filter((r) => r.canal === "resposta")).toEqual([]);
  });
});

describe("nível da voz do modelo", () => {
  // A voz do modelo é medida depois de processada e trazida para o nível do acervo — RMS
  // DA FALA 0,185, medido nos 3.226 arquivos.

  it("abaixa a voz alta demais até o nível das peças", async () => {
    const amostras = new Float32Array(1000).fill(0.6);
    renderFalso(amostras);

    expect(await voz.falarRemoto("QUJD", "audio/mpeg")).toBe(true);

    expect(rmsDe(amostras)).toBeCloseTo(0.185, 3);
    // E toca o buffer RENDERIZADO, não o cru — a cadeia já está dentro dele.
    expect(tocadas).toEqual(["render"]);
  });

  it("mede a FALA, e não o arquivo: o silêncio não conta para o nível", async () => {
    // O defeito que fazia duas respostas no mesmo RMS soarem em alturas diferentes. Esta
    // peça é metade fala e metade silêncio; medida inteira, o RMS cai por um fator raiz
    // de dois e o ganho compensa isso empurrando a FALA para o dobro do alvo.
    const amostras = new Float32Array(4800);
    amostras.fill(0.5, 0, 2400); // 50 ms de "fala", 50 ms de silêncio
    renderFalso(amostras);

    expect(await voz.falarRemoto("QUJD", "audio/mpeg")).toBe(true);

    const falada = amostras.slice(0, 2400);
    expect(rmsDe(falada)).toBeCloseTo(0.185, 3);
    // E o arquivo inteiro fica ABAIXO do alvo, que é o esperado — metade dele é silêncio.
    expect(rmsDe(amostras)).toBeLessThan(0.185);
  });

  it("levanta a voz baixa demais, mas nunca ao custo de um clipe", async () => {
    // Uma frase quase inaudível com um transiente perto do teto: perseguir o RMS aqui
    // multiplicaria o transiente por dez e recortaria. Quem manda no fim é o pico.
    const amostras = new Float32Array(1000).fill(0.01);
    amostras[500] = 0.9;
    renderFalso(amostras);

    expect(await voz.falarRemoto("QUJD", "audio/mpeg")).toBe(true);

    // Encosta no teto, não passa. `toBeCloseTo` e não `<=` porque o `Float32Array`
    // arredonda o produto para cima na sétima casa — 0,97000003 não é um clipe.
    expect(picoDe(amostras)).toBeCloseTo(0.97, 4);
    expect(rmsDe(amostras)).toBeLessThan(0.175);
  });

  it("sem contexto offline, ainda fala — só sem o nivelamento", async () => {
    // Caminho de exceção declarado: emudecer por não conseguir MEDIR seria pior que sair
    // no volume errado.
    expect(await voz.falarRemoto("QUJD", "audio/mpeg")).toBe(true);
    expect(tocadas.length).toBe(1);
  });
});

describe("desligado", () => {
  it("com a voz desligada o anúncio nem entra na fila", async () => {
    localStorage.setItem("loop.engenheiroVoz", "0");
    expect(await voz.anunciar(["pos_2"])).toBe(false);
    expect(tocadas).toEqual([]);
    localStorage.setItem("loop.engenheiroVoz", "1");
  });
});

describe("a peça própria", () => {
  // O sobrenome do jogador, sintetizado uma vez por carreira e guardado no save. Chega em
  // base64 pela ponte, não pelo `import.meta.glob` — é a única peça do rádio que não vem
  // no instalador.

  it("depois de registrada, toca como qualquer peça do acervo", async () => {
    renderFalso(new Float32Array(4800).fill(0.2));
    expect(await voz.registrarPecaPropria("voc_magno", "QUJD")).toBe(true);
    // A prova que importa: o renderizador do Rust emite a chave junto com as outras, e o
    // tocador não distingue as duas origens.
    await voz.falarPecas(["voc_magno", "pos_2"]);
    expect(tocadas).toEqual(["render", "pos_2"]);
  });

  it("passa pelo MESMO nivelamento da resposta do modelo", async () => {
    // Sem isto o nome do piloto sairia com outro volume no meio da própria frase dele — o
    // acervo é nivelado no gerador, e o que chega em base64 não é.
    const amostras = new Float32Array(4800).fill(0.02);
    renderFalso(amostras);
    expect(await voz.registrarPecaPropria("voc_magno", "QUJD")).toBe(true);
    expect(rmsDe(amostras)).toBeCloseTo(0.185, 3);
  });

  it("sem o render, a peça NÃO entra — melhor sem vocativo que com o volume errado", async () => {
    // O oposto da regra da resposta do modelo, e de propósito. Lá, tocar torto é melhor que
    // emudecer: o piloto perguntou e está esperando. Aqui a peça é um enfeite na frente de
    // uma frase que sai igual sem ela.
    expect(await voz.registrarPecaPropria("voc_magno", "QUJD")).toBe(false);
    await voz.falarPecas(["voc_magno", "pos_2"]);
    expect(tocadas).toEqual(["pos_2"]);
  });

  it("registrar duas vezes não refaz o render", async () => {
    renderFalso(new Float32Array(4800).fill(0.2));
    expect(await voz.registrarPecaPropria("voc_magno", "QUJD")).toBe(true);
    // Quem chama é o boot do app, que não sabe se já rodou nesta sessão.
    delete window.OfflineAudioContext;
    expect(await voz.registrarPecaPropria("voc_magno", "QUJD")).toBe(true);
  });

  it("entra na lista de chaves, e o pré-aquecimento não a derruba", async () => {
    renderFalso(new Float32Array(4800).fill(0.2));
    await voz.registrarPecaPropria("voc_magno", "QUJD");
    expect(voz.chavesDisponiveis()).toContain("voc_magno");
    // `preaquecer` percorre a lista inteira decodificando; a peça própria já está pronta e
    // não tem URL nenhuma para buscar. Sem o curto-circuito do mapa de buffers, este passo
    // apagaria o buffer dela.
    await voz.preaquecer(voz.chavesDisponiveis());
    await voz.falarPecas(["voc_magno"]);
    expect(tocadas).toEqual(["render"]);
  });

  it("chave ou áudio vazio não registra nada", async () => {
    renderFalso(new Float32Array(4800).fill(0.2));
    expect(await voz.registrarPecaPropria("", "QUJD")).toBe(false);
    expect(await voz.registrarPecaPropria("voc_x", "")).toBe(false);
  });
});
