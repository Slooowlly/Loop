// A máquina de estados do push-to-talk, exercida sem áudio nenhum.
//
// É a parte do encanamento em que um erro não aparece como erro: uma resposta velha
// chegando depois de uma pergunta nova sai com a voz confiante do engenheiro e soa
// perfeitamente normal — só está respondendo outra coisa. Daí a maioria dos casos aqui
// serem sobre ORDEM, e não sobre o caminho feliz.

import { describe, expect, it, vi } from "vitest";
import { JUNCAO, pausasDoRadio } from "./pausasDoRadio";
import { base64De, criarOrquestrador, FALANDO, OCIOSO, OUVINDO, PENSANDO } from "./pttEngenheiro";

/** Microfone de mentira: `terminar()` devolve o que o teste mandar. */
function microfoneFalso({ captura = { bytes: new Uint8Array([1, 2, 3]), mime: "audio/webm" } } = {}) {
  return {
    armado: true,
    estaArmado() {
      return this.armado;
    },
    comecar: vi.fn(async () => ({ atrasoMs: 3 })),
    terminar: vi.fn(async () => captura),
    abortar: vi.fn(async () => {}),
  };
}

function vozFalsa() {
  return {
    tocadas: [],
    pausas: [],
    remotas: [],
    cancelar: vi.fn(),
    // A derivação de verdade, e não um `() => []`. As pausas entre peças são o que separa
    // uma fala montada de uma leitura de lista, e um dublê que devolvesse vazio deixaria
    // passar o dia em que o orquestrador parasse de pedi-las.
    pausasDoRadio,
    falarPecas: vi.fn(async function (p, opcoes) {
      this.tocadas.push(p);
      this.pausas.push(opcoes?.pausasMs ?? null);
      return true;
    }),
    falarRemoto: vi.fn(async function (b64) {
      this.remotas.push(b64);
      return true;
    }),
  };
}

/** `invoke` de mentira: mapa de comando -> função. */
function invokeFalso(mapa) {
  return vi.fn(async (cmd, args) => {
    if (!(cmd in mapa)) throw new Error(`comando inesperado: ${cmd}`);
    const r = mapa[cmd];
    return typeof r === "function" ? r(args) : r;
  });
}

const GRAVADA = { caminho: "gravada", intencao: "Posicao", pecas: ["pos_5"] };
const MODELO = { caminho: "modelo", intencao: "Ritmo", linhas: ["Você está em quinto."] };

describe("a carreira aberta viaja com a pergunta", () => {
  it("manda o careerId ao Rust — sem ele não há tabela de campeonato para ler", async () => {
    // O `careerId` é lido por FUNÇÃO, e não copiado na criação do orquestrador: ele nasce
    // antes de a carreira carregar, e um valor capturado ficaria preso no vazio para
    // sempre. O sintoma seria o engenheiro nunca mencionar o campeonato, sem erro nenhum.
    let atual = "";
    const invoke = invokeFalso({
      ptt_transcrever: "em que posição eu estou",
      engenheiro_responder: GRAVADA,
    });
    const o = criarOrquestrador({
      microfone: microfoneFalso(),
      voz: vozFalsa(),
      invoke,
      careerId: () => atual,
    });

    atual = "carreira-42";
    await o.apertar();
    await o.soltar();

    const args = invoke.mock.calls.find((c) => c[0] === "engenheiro_responder")[1];
    expect(args.careerId).toBe("carreira-42");
  });
});

describe("a fala montada respira", () => {
  it("pede as pausas DERIVADAS DAS CHAVES, não a padrão para tudo", async () => {
    // A resposta gravada deixou de ser sempre uma frase só. Com o vizinho nomeado ela é
    // montada, e com o campeonato ela ganha uma segunda frase inteira atrás — junções de
    // tipos diferentes, que com a pausa padrão de 160 ms sairiam todas iguais: a vírgula
    // arrastada e o ponto final atropelado, na mesma fala.
    const voz = vozFalsa();
    const invoke = invokeFalso({
      ptt_transcrever: "quem está na minha frente",
      engenheiro_responder: {
        caminho: "gravada",
        intencao: "Frente",
        pecas: ["ab_rival", "nm_cooper", "viz_frente_gap_1_2", "camp_pos_3"],
      },
    });
    const o = criarOrquestrador({ microfone: microfoneFalso(), voz, invoke });

    await o.apertar();
    await o.soltar();

    expect(voz.pausas[0]).toEqual([
      JUNCAO.virgula, // "Seu rival," → "Cooper,"
      JUNCAO.oracao, //  "Cooper," → "está a um e dois na sua frente."
      JUNCAO.frase, //   "…na sua frente." → "No campeonato, você está em terceiro."
    ]);
  });
});

describe("caminho gravado", () => {
  it("vai do botão às peças sem passar pelo servidor de resposta", async () => {
    const microfone = microfoneFalso();
    const voz = vozFalsa();
    const invoke = invokeFalso({
      ptt_transcrever: "em que posição eu estou",
      engenheiro_responder: GRAVADA,
    });
    const o = criarOrquestrador({ microfone, voz, invoke });

    await o.apertar();
    expect(o.estado()).toBe(OUVINDO);
    await o.soltar();

    expect(voz.tocadas).toEqual([["pos_5"]]);
    expect(o.estado()).toBe(OCIOSO);
    // O ponto do acervo é este: `ptt_responder` nunca foi chamado.
    expect(invoke.mock.calls.map((c) => c[0])).toEqual(["ptt_transcrever", "engenheiro_responder"]);
  });
});

/** Resposta do servidor que demora — para a espera ter contra o que correr. */
function respostaLenta(ms = 30, corpo = { texto: "", audio_b64: "QUJD", mime: "audio/mpeg" }) {
  return async () => {
    await new Promise((r) => setTimeout(r, ms));
    return corpo;
  };
}

describe("caminho do modelo", () => {
  it("com o servidor rápido, NÃO toca a espera", async () => {
    // O caso normal desde que o servidor está quente: a ida e volta dá ~1,4 s, e um
    // "só um segundo" de 0,9 s na frente de uma resposta pronta é atraso puro.
    const microfone = microfoneFalso();
    const voz = vozFalsa();
    const invoke = invokeFalso({
      ptt_transcrever: "como está meu ritmo",
      engenheiro_responder: MODELO,
      ptt_responder: { texto: "Você está indo bem.", audio_b64: "QUJD", mime: "audio/mpeg" },
    });
    const o = criarOrquestrador({ microfone, voz, invoke, aleatorio: () => 0 });

    await o.apertar();
    await o.soltar();

    expect(voz.tocadas).toEqual([]);
    expect(voz.remotas).toEqual(["QUJD"]);
    expect(o.estado()).toBe(OCIOSO);
  });

  it("com o servidor lento, a espera cobre o silêncio — e só neste ramo", async () => {
    const microfone = microfoneFalso();
    const voz = vozFalsa();
    const invoke = invokeFalso({
      ptt_transcrever: "como está meu ritmo",
      engenheiro_responder: MODELO,
      ptt_responder: respostaLenta(30, {
        texto: "Você está indo bem.",
        audio_b64: "QUJD",
        mime: "audio/mpeg",
      }),
    });
    const o = criarOrquestrador({ microfone, voz, invoke, aleatorio: () => 0, limiarEsperaMs: 0 });

    await o.apertar();
    await o.soltar();

    expect(voz.tocadas).toEqual([["espera"]]);
    expect(voz.remotas).toEqual(["QUJD"]);
    expect(o.estado()).toBe(OCIOSO);
  });

  it("pede a resposta ANTES de tocar a espera, para os dois tempos correrem juntos", async () => {
    // Serializar somaria ~1,2 s de espera com o tempo de servidor. A ordem das chamadas é
    // a prova de que não somam: o pedido sai antes de a espera começar a tocar.
    const ordem = [];
    const microfone = microfoneFalso();
    const voz = vozFalsa();
    voz.falarPecas = vi.fn(async (p) => {
      ordem.push(`toca:${p[0]}`);
      return true;
    });
    const invoke = invokeFalso({
      ptt_transcrever: "como está meu ritmo",
      engenheiro_responder: MODELO,
      ptt_responder: async () => {
        ordem.push("pede");
        await new Promise((r) => setTimeout(r, 30));
        return { texto: "", audio_b64: "QUJD", mime: "audio/mpeg" };
      },
    });
    await (async () => {
      const o = criarOrquestrador({ microfone, voz, invoke, aleatorio: () => 0, limiarEsperaMs: 0 });
      await o.apertar();
      await o.soltar();
    })();

    expect(ordem).toEqual(["pede", "toca:espera"]);
  });

  it("servidor fora do ar não ganha um 'só um segundo' antes do 'não sei'", async () => {
    // A rejeição também vence a corrida: dizer "só um segundo" e emendar "não consegui
    // ver isso agora" é prometer o que já se sabe que não vem.
    const microfone = microfoneFalso();
    const voz = vozFalsa();
    const invoke = invokeFalso({
      ptt_transcrever: "como está meu ritmo",
      engenheiro_responder: MODELO,
      ptt_responder: () => {
        throw new Error("Falha de rede ao responder");
      },
    });
    const o = criarOrquestrador({ microfone, voz, invoke, aleatorio: () => 0, limiarEsperaMs: 50 });

    await o.apertar();
    await o.soltar();

    expect(voz.tocadas).toEqual([["nao_sei"]]);
  });
});

describe("modelo forçado (painel de diagnóstico)", () => {
  it("troca de comando: pede o DOSSIÊ em vez de deixar o Rust rotear", async () => {
    // Com o modelo forçado o `engenheiro_responder` não serve: ele já decidiu, e o que
    // ele decide na maioria das perguntas é o acervo. `engenheiro_dossie` devolve os
    // fatos sem escolher nada — é a única porta para o caminho longo.
    const invoke = invokeFalso({
      engenheiro_dossie: {
        intencao: "Posicao",
        linhas: ["Você está em quinto."],
        estado: { conectado: true },
      },
      ptt_responder: { texto: "Você está em quinto.", audio_b64: "QUJD", mime: "audio/mpeg" },
    });
    const voz = vozFalsa();
    const o = criarOrquestrador({
      microfone: microfoneFalso(),
      voz,
      invoke,
      aleatorio: () => 0,
      forcarModelo: () => true,
    });

    await o.perguntarEscrito("em que posição eu estou");

    expect(invoke.mock.calls.map((c) => c[0])).toEqual(["engenheiro_dossie", "ptt_responder"]);
    expect(voz.remotas).toEqual(["QUJD"]);
  });

  it("sem corrida, usa os fatos de demonstração — mesmo com o dossiê trazendo uma linha", async () => {
    // O caso que passou despercebido: sem simulador o dossiê NÃO vem vazio, vem com a
    // linha que diz que não há dados. Um teste de `length` deixava essa linha passar, e o
    // modelo recebia um fato inútil e respondia sobre a falta dele. Quem manda é
    // `estado.conectado`.
    const invoke = invokeFalso({
      engenheiro_dossie: {
        intencao: "Geral",
        linhas: ["Sem telemetria do iRacing."],
        estado: { conectado: false },
      },
      ptt_responder: { texto: "ok", audio_b64: "QUJD", mime: "audio/mpeg" },
    });
    const o = criarOrquestrador({
      microfone: microfoneFalso(),
      voz: vozFalsa(),
      invoke,
      aleatorio: () => 0,
      forcarModelo: () => true,
      linhasDemo: () => ["Você está em quinto de vinte e quatro."],
    });

    await o.perguntarEscrito("como estamos");

    const pedido = invoke.mock.calls.find((c) => c[0] === "ptt_responder")[1];
    expect(pedido.linhas).toEqual(["Você está em quinto de vinte e quatro."]);
  });

  it("sem fato nenhum desiste, em vez de pedir ao modelo que invente", async () => {
    const voz = vozFalsa();
    const invoke = invokeFalso({
      engenheiro_dossie: { intencao: "Geral", linhas: [], estado: { conectado: true } },
    });
    const o = criarOrquestrador({
      microfone: microfoneFalso(),
      voz,
      invoke,
      forcarModelo: () => true,
      linhasDemo: () => [],
    });

    await o.perguntarEscrito("como estamos");

    expect(voz.tocadas).toEqual([["nao_sei"]]);
    expect(invoke.mock.calls.map((c) => c[0])).toEqual(["engenheiro_dossie"]);
  });

  it("desligado, o roteamento continua sendo do Rust", async () => {
    const invoke = invokeFalso({
      engenheiro_responder: GRAVADA,
    });
    const o = criarOrquestrador({ microfone: microfoneFalso(), voz: vozFalsa(), invoke });

    await o.perguntarEscrito("em que posição eu estou");

    expect(invoke.mock.calls.map((c) => c[0])).toEqual(["engenheiro_responder"]);
  });
});

describe("desistência", () => {
  it("o toque acidental não faz o engenheiro falar", async () => {
    const microfone = microfoneFalso({
      captura: { bytes: new Uint8Array([1]), mime: "audio/webm", curtaDemais: true },
    });
    const voz = vozFalsa();
    const invoke = invokeFalso({});
    const o = criarOrquestrador({ microfone, voz, invoke });

    await o.apertar();
    await o.soltar();

    expect(voz.tocadas).toEqual([]);
    expect(invoke).not.toHaveBeenCalled();
    expect(o.estado()).toBe(OCIOSO);
  });

  it("transcrição vazia vira 'não te ouvi' — não 'não sei', que descreveria outra coisa", async () => {
    // Sem esta guarda, o classificador em cima de string vazia devolve a intenção aberta,
    // e o engenheiro responde com o retrato completo da corrida a um botão apertado sem
    // querer com o motor por cima.
    //
    // E a FALA importa tanto quanto a guarda. "Não consegui ver isso agora" diz que o dado
    // não estava lá; aqui o dado nem foi pedido — o que faltou foi a voz. Com a fala
    // errada, o piloto acha que perguntou e fica esperando a resposta.
    const microfone = microfoneFalso();
    const voz = vozFalsa();
    const invoke = invokeFalso({ ptt_transcrever: "" });
    const o = criarOrquestrador({ microfone, voz, invoke });

    await o.apertar();
    await o.soltar();

    expect(voz.tocadas).toEqual([["nao_ouvi"]]);
    // E nada subiu ao modelo: a pergunta que ninguém fez não vira ida ao servidor.
    expect(invoke.mock.calls.map((c) => c[0])).toEqual(["ptt_transcrever"]);
  });

  it("servidor fora do ar vira 'não sei', e não silêncio", async () => {
    const microfone = microfoneFalso();
    const voz = vozFalsa();
    const invoke = invokeFalso({
      ptt_transcrever: () => {
        throw new Error("Falha de rede ao transcrever");
      },
    });
    const o = criarOrquestrador({ microfone, voz, invoke });

    await o.apertar();
    await o.soltar();

    expect(voz.tocadas).toEqual([["nao_sei"]]);
    expect(o.ultimoErro()).toMatch(/rede/);
  });

  it("áudio que chega mas não toca também vira 'não sei'", async () => {
    // A diferença entre "o engenheiro disse que não sabe" e "o engenheiro emudeceu".
    const microfone = microfoneFalso();
    const voz = vozFalsa();
    voz.falarRemoto = vi.fn(async () => false);
    const invoke = invokeFalso({
      ptt_transcrever: "como está meu ritmo",
      engenheiro_responder: MODELO,
      ptt_responder: { texto: "", audio_b64: "QUJD", mime: "audio/mpeg" },
    });
    const o = criarOrquestrador({ microfone, voz, invoke, aleatorio: () => 0 });

    await o.apertar();
    await o.soltar();

    expect(voz.tocadas).toEqual([["nao_sei"]]);
  });

  it("microfone desarmado não deixa o estado preso em 'ouvindo'", async () => {
    const microfone = microfoneFalso();
    microfone.armado = false;
    const o = criarOrquestrador({ microfone, voz: vozFalsa(), invoke: invokeFalso({}) });

    await o.apertar();

    expect(o.estado()).toBe(OCIOSO);
    expect(microfone.comecar).not.toHaveBeenCalled();
  });
});

describe("pergunta nova cancela a anterior", () => {
  it("a resposta velha não fala depois que o piloto perguntou outra coisa", async () => {
    // O caso perigoso do sistema inteiro: a transcrição lenta volta DEPOIS do segundo
    // toque no botão. Se ela seguisse adiante, o engenheiro responderia a pergunta velha
    // — com a voz confiante de sempre, e sem nada parecer errado.
    const microfone = microfoneFalso();
    const voz = vozFalsa();
    let liberar;
    const presa = new Promise((r) => {
      liberar = r;
    });
    const invoke = invokeFalso({
      ptt_transcrever: () => presa,
      engenheiro_responder: GRAVADA,
    });
    const o = criarOrquestrador({ microfone, voz, invoke });

    await o.apertar();
    const primeira = o.soltar();

    // Segundo toque enquanto a primeira ainda está no ar.
    await o.apertar();
    liberar("em que posição eu estou");
    await primeira;

    expect(voz.tocadas).toEqual([]);
    expect(voz.cancelar).toHaveBeenCalled();
    expect(o.estado()).toBe(OUVINDO); // a SEGUNDA pergunta, gravando
  });

  it("soltar sem ter apertado não faz nada", async () => {
    const microfone = microfoneFalso();
    const o = criarOrquestrador({ microfone, voz: vozFalsa(), invoke: invokeFalso({}) });
    await o.soltar();
    expect(microfone.terminar).not.toHaveBeenCalled();
  });
});

describe("estados anunciados", () => {
  it("passa por ouvindo, pensando e falando, nessa ordem", async () => {
    const vistos = [];
    const microfone = microfoneFalso();
    const invoke = invokeFalso({
      ptt_transcrever: "em que posição eu estou",
      engenheiro_responder: GRAVADA,
    });
    const o = criarOrquestrador({ microfone, voz: vozFalsa(), invoke });
    o.aoMudar((e) => vistos.push(e));

    await o.apertar();
    await o.soltar();

    expect(vistos).toEqual([OUVINDO, PENSANDO, FALANDO, OCIOSO]);
  });
});

describe("o freio do rádio", () => {
  /** Um freio de mentira que devolve o veredicto que o teste mandar. */
  function freioFalso(veredictos = []) {
    let bloqueio = 0;
    return {
      bloqueado: () => bloqueio > 0,
      restanteMs: () => bloqueio,
      naJanela: () => 0,
      zerar() {},
      registrar() {
        const v = veredictos.shift() ?? "ok";
        if (v === "corte") bloqueio = 60000;
        return v;
      },
    };
  }

  it("de castigo, o microfone NEM ABRE — o Scribe não cobra pelo que ninguém responde", async () => {
    const microfone = microfoneFalso();
    const voz = vozFalsa();
    const invoke = invokeFalso({});
    const freio = freioFalso(["corte"]);
    freio.registrar(); // entra em castigo
    const o = criarOrquestrador({ microfone, voz, invoke, freio });

    await o.apertar();
    await o.soltar();

    expect(microfone.comecar).not.toHaveBeenCalled();
    expect(invoke).not.toHaveBeenCalled();
    expect(o.estado()).toBe(OCIOSO);
  });

  it("no corte, fala o rádio ruim e NÃO manda o áudio", async () => {
    const microfone = microfoneFalso();
    const voz = vozFalsa();
    const invoke = invokeFalso({});
    const o = criarOrquestrador({ microfone, voz, invoke, freio: freioFalso(["corte"]) });

    await o.apertar();
    await o.soltar();

    // Gravou (o botão foi apertado antes do castigo) mas nada subiu.
    expect(microfone.terminar).toHaveBeenCalled();
    expect(invoke).not.toHaveBeenCalled();
    expect(voz.tocadas).toEqual([["radio_ruim"]]);
    expect(o.estado()).toBe(OCIOSO);
  });

  it("o toque acidental não conta — esbarrar no volante não é tagarelice", async () => {
    const freio = freioFalso();
    const registrar = vi.spyOn(freio, "registrar");
    const o = criarOrquestrador({
      microfone: microfoneFalso({ captura: { curtaDemais: true } }),
      voz: vozFalsa(),
      invoke: invokeFalso({}),
      freio,
    });

    await o.apertar();
    await o.soltar();

    expect(registrar).not.toHaveBeenCalled();
  });

  it("no aviso, RESPONDE e emenda o empurrão na mesma sequência", async () => {
    // A ordem é a regra do produto: quem recusa a informação é o corte. O empurrão é um
    // engenheiro que atende e manda você voltar a correr.
    const voz = vozFalsa();
    const invoke = invokeFalso({
      ptt_transcrever: "em que posição eu estou",
      engenheiro_responder: GRAVADA,
    });
    const o = criarOrquestrador({
      microfone: microfoneFalso(),
      voz,
      invoke,
      freio: freioFalso(["aviso"]),
      aleatorio: () => 0,
    });

    await o.apertar();
    await o.soltar();

    expect(voz.tocadas).toEqual([["pos_5", "foco"]]);
  });

  it("no aviso pelo caminho do modelo, o empurrão vem DEPOIS da voz do modelo", async () => {
    const voz = vozFalsa();
    const invoke = invokeFalso({
      ptt_transcrever: "como está meu ritmo",
      engenheiro_responder: MODELO,
      ptt_responder: { texto: "Você está indo bem.", audio_b64: "QUJD", mime: "audio/mpeg" },
    });
    const o = criarOrquestrador({
      microfone: microfoneFalso(),
      voz,
      invoke,
      freio: freioFalso(["aviso"]),
      aleatorio: () => 0,
    });

    await o.apertar();
    await o.soltar();

    expect(voz.remotas).toEqual(["QUJD"]);
    expect(voz.tocadas).toEqual([["foco"]]);
  });
});

describe("apertar por cima de uma fala termina em estado coerente", () => {
  // O apertar tem dois portões que saem cedo: o rádio de castigo e o microfone sem faixa.
  // Os dois já desfizeram a fala anterior antes de sair — `geracao += 1` órfã quem estava no
  // ar, `voz.cancelar()` cala o áudio —, então ninguém mais volta para devolver o estado: o
  // `ir(OCIOSO)` do fluxo que estava tocando é guardado por `geracao === minhaVez`.
  //
  // Sem uma saída explícita, o estado ficava preso em FALANDO pelo resto da sessão: o
  // indicador aceso, e o `soltar()` — que só age vindo de OUVINDO — recusando cada toque
  // seguinte. O botão emudecia de vez, e nada em lugar nenhum dizia por quê.

  /** Uma voz cuja fala NUNCA termina. É assim que se observa o estado FALANDO parado. */
  function vozPresa() {
    return {
      tocadas: [],
      cancelar: vi.fn(),
      pausasDoRadio,
      falarPecas: vi.fn(async function (p) {
        this.tocadas.push(p);
        await new Promise(() => {});
      }),
      falarRemoto: vi.fn(async () => true),
    };
  }

  /**
   * Leva o orquestrador até FALANDO e o deixa lá.
   *
   * O `soltar()` fica pendurado de propósito e NÃO é devolvido: a fala não termina nunca
   * neste dublê, e devolvê-lo faria o próprio teste esperar por ele para sempre.
   */
  async function ateFalando(o) {
    await o.apertar();
    void o.soltar();
    // Cada perna do encanamento é um `await` de promessa já resolvida; drenar a fila de
    // microtarefas basta para chegar à fala, e não há relógio nenhum no caminho.
    for (let i = 0; i < 20; i += 1) await Promise.resolve();
  }

  const INVOKE = {
    ptt_transcrever: "em que posição eu estou",
    engenheiro_responder: GRAVADA,
  };

  it("de castigo, sai de FALANDO para OCIOSO em vez de travar", async () => {
    let bloqueio = 0;
    const freio = {
      bloqueado: () => bloqueio > 0,
      restanteMs: () => bloqueio,
      naJanela: () => 0,
      zerar() {},
      registrar: () => "ok",
    };
    const voz = vozPresa();
    const o = criarOrquestrador({
      microfone: microfoneFalso(),
      voz,
      invoke: invokeFalso(INVOKE),
      freio,
    });

    await ateFalando(o);
    expect(o.estado()).toBe(FALANDO);

    bloqueio = 60000; // o rádio entra de castigo com a resposta ainda no ar
    await o.apertar();

    expect(o.estado()).toBe(OCIOSO);
    expect(voz.cancelar).toHaveBeenCalled();
  });

  it("sem microfone armado, sai de FALANDO para OCIOSO em vez de travar", async () => {
    const microfone = microfoneFalso();
    const voz = vozPresa();
    const o = criarOrquestrador({ microfone, voz, invoke: invokeFalso(INVOKE) });

    await ateFalando(o);
    expect(o.estado()).toBe(FALANDO);

    // A faixa morreu no meio da resposta — o rig desligou, o dispositivo sumiu.
    microfone.armado = false;
    await o.apertar();

    expect(o.estado()).toBe(OCIOSO);
    expect(microfone.comecar).toHaveBeenCalledTimes(1); // o segundo toque não gravou nada
  });
});

describe("base64De", () => {
  it("converte sem estourar a pilha em áudio de tamanho real", () => {
    // `String.fromCharCode(...bytes)` de uma vez morre por volta de 100 mil argumentos, e
    // três segundos de Opus passam disso quando o WebView2 escolhe uma taxa mais alta.
    const grande = new Uint8Array(200_000).map((_, i) => i % 256);
    const b64 = base64De(grande);
    expect(atob(b64).length).toBe(grande.length);
  });

  it("bate com o que o atob desfaz", () => {
    const bytes = new Uint8Array([0, 1, 254, 255, 65, 66, 67]);
    const voltou = Uint8Array.from(atob(base64De(bytes)), (c) => c.charCodeAt(0));
    expect(Array.from(voltou)).toEqual(Array.from(bytes));
  });
});
