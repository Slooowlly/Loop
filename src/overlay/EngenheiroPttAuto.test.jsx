import { vi, describe, it, expect, beforeEach, afterEach } from "vitest";
import { render, act } from "@testing-library/react";

// O encanamento do push-to-talk do engenheiro. Não desenha nada: liga o vigia de tecla do
// Rust ao orquestrador, arma e desarma o microfone com a sessão, e empurra a configuração
// salva de volta ao backend.
//
// É a peça mais fácil de quebrar em silêncio de todo o rádio. Os dois defeitos que já
// aconteceram aqui não davam erro nenhum na tela:
//
//   • o ouvinte ÓRFÃO. `listen` é assíncrono e a limpeza do efeito é síncrona; sem a bandeira
//     de morto, uma desmontagem antes de a promessa resolver deixa o ouvinte vivo e sem quem
//     o remova. Em dev o StrictMode monta duas vezes, e cada toque no botão virava DUAS
//     perguntas ao engenheiro.
//   • o portão CONSTANTE. `iracing_connected` devolve uma estrutura, e `Boolean(estrutura)` é
//     sempre verdadeiro. O microfone abria uma vez, no menu principal, com o rig ainda
//     desligado — e a corrida inteira era ouvida pela placa errada, sem segunda tentativa.
//
// Nenhum dos dois aparece em screenshot, e os dois só se manifestam com o simulador rodando.

const ouvintes = new Map(); // evento -> callback registrado pelo componente
const removidos = []; // eventos cujo `unlisten` foi chamado
let resolverListen = null; // quando definido, segura o `listen` para o teste soltar

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (evento, cb) => {
    if (resolverListen) await resolverListen;
    ouvintes.set(evento, cb);
    return () => removidos.push(evento);
  }),
}));
vi.mock("../lib/tauri", () => ({ estaNoTauri: vi.fn(() => true) }));
vi.mock("../lib/microfone", () => ({
  armar: vi.fn(async () => {}),
  desarmar: vi.fn(),
  estaArmado: vi.fn(() => false),
}));
vi.mock("../lib/engenheiroVoz", () => ({
  definirPortao: vi.fn(),
  registrarPecaPropria: vi.fn(async () => {}),
  anunciarRemoto: vi.fn(),
}));
vi.mock("../lib/pttEngenheiro", () => ({
  criarOrquestrador: vi.fn(() => ({
    apertar: vi.fn(),
    soltar: vi.fn(),
    cancelar: vi.fn(),
  })),
}));
vi.mock("../lib/pttConfig", () => ({
  GATILHO_STORE: "loop.pttGatilho",
  lerGatilhoSalvo: vi.fn(() => ({ tipo: "tecla", vk: 84 })),
  lerMicSalvo: vi.fn(() => "mic-do-rig"),
  estaEmTeste: vi.fn(() => false),
}));
vi.mock("../stores/useCareerStore", () => ({
  default: (seletor) => seletor({ careerId: "save-1" }),
}));

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { estaNoTauri } from "../lib/tauri";
import * as microfone from "../lib/microfone";
import * as voz from "../lib/engenheiroVoz";
import { criarOrquestrador } from "../lib/pttEngenheiro";
import { estaEmTeste, lerMicSalvo } from "../lib/pttConfig";

import EngenheiroPttAuto from "./EngenheiroPttAuto";

/// O intervalo do laço de sessão, copiado do componente.
const POLL_MS = 2000;

/// Os comandos disparados, na ordem.
const comandos = () => invoke.mock.calls.map(([nome]) => nome);

/// O orquestrador que o componente criou (é um por montagem).
const orquestrador = () => criarOrquestrador.mock.results.at(-1).value;

/// Monta e deixa o primeiro `tick` do laço de sessão assentar (ele roda na montagem).
async function montar() {
  const utils = render(<EngenheiroPttAuto />);
  await act(async () => {});
  return utils;
}

/// Avança o laço de sessão uma volta e deixa as promessas assentarem.
async function proximaVolta() {
  await act(async () => {
    vi.advanceTimersByTime(POLL_MS);
  });
  await act(async () => {});
}

describe("EngenheiroPttAuto", () => {
  beforeEach(() => {
    ouvintes.clear();
    removidos.length = 0;
    resolverListen = null;
    vi.clearAllMocks();
    estaNoTauri.mockReturnValue(true);
    estaEmTeste.mockReturnValue(false);
    microfone.estaArmado.mockReturnValue(false);
    microfone.armar.mockResolvedValue(undefined);
    lerMicSalvo.mockReturnValue("mic-do-rig");
    // Sessão fechada por padrão: cada teste que quer o microfone armado abre a sua.
    invoke.mockResolvedValue(null);
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  describe("fora do Tauri", () => {
    it("não toca em nada no navegador", async () => {
      // A mesma tela roda no `npm run dev` sem shell. Um `invoke` ali estoura no console a
      // cada dois segundos e esconde o erro que importa.
      estaNoTauri.mockReturnValue(false);
      await montar();
      expect(invoke).not.toHaveBeenCalled();
      expect(listen).not.toHaveBeenCalled();
      expect(microfone.armar).not.toHaveBeenCalled();
    });
  });

  describe("ouvintes do vigia", () => {
    it("assina apertar e soltar", async () => {
      await montar();
      expect([...ouvintes.keys()]).toEqual(
        expect.arrayContaining(["ptt-apertou", "ptt-soltou"]),
      );
    });

    it("o toque no botão chega ao orquestrador", async () => {
      await montar();
      const orq = orquestrador();
      act(() => ouvintes.get("ptt-apertou")());
      act(() => ouvintes.get("ptt-soltou")());
      expect(orq.apertar).toHaveBeenCalledTimes(1);
      expect(orq.soltar).toHaveBeenCalledTimes(1);
    });

    it("cala enquanto a bancada de teste das Configurações está aberta", async () => {
      // O bloco de teste usa o MESMO botão. Sem esta trava, conferir o microfone nas
      // Configurações dispararia uma pergunta de verdade ao engenheiro por cima do teste.
      await montar();
      estaEmTeste.mockReturnValue(true);
      act(() => ouvintes.get("ptt-apertou")());
      expect(orquestrador().apertar).not.toHaveBeenCalled();
    });

    it("remove os ouvintes na desmontagem", async () => {
      const { unmount } = await montar();
      unmount();
      expect(removidos).toEqual(expect.arrayContaining(["ptt-apertou", "ptt-soltou"]));
    });

    it("desmontar ANTES de o listen resolver não deixa ouvinte órfão", async () => {
      // O defeito que fazia cada toque virar duas perguntas: a limpeza rodava antes de a
      // promessa do `listen` resolver, e o ouvinte nascia depois, sem ninguém para removê-lo.
      let soltar;
      resolverListen = new Promise((r) => {
        soltar = r;
      });
      const { unmount } = render(<EngenheiroPttAuto />);
      unmount();
      await act(async () => {
        soltar();
      });
      expect(removidos).toEqual(expect.arrayContaining(["ptt-apertou", "ptt-soltou"]));
    });
  });

  describe("microfone e sessão", () => {
    /// Responde ao portão `iracing_connected` com `aberta`.
    const sessao = (aberta) =>
      invoke.mockImplementation((nome) =>
        nome === "iracing_connected" ? Promise.resolve(aberta) : Promise.resolve(null),
      );

    it("fora de sessão o microfone fica fechado", async () => {
      // O indicador de microfone em uso do Windows é visível: aberto no menu principal, ele
      // acende uma luz que o jogador não pediu.
      sessao(false);
      await montar();
      expect(microfone.armar).not.toHaveBeenCalled();
    });

    it("em sessão arma COM o dispositivo escolhido", async () => {
      // Sem o `deviceId` a captura cai no padrão do Windows, que num rig de VR costuma ser o
      // áudio virtual do headset — o engenheiro ouvindo a placa errada, sem nada indicando.
      sessao(true);
      await montar();
      expect(microfone.armar).toHaveBeenCalledWith({ deviceId: "mic-do-rig" });
    });

    it("o portão fecha e abre de novo — não é decidido uma vez só", async () => {
      // O defeito de origem: `Boolean(estrutura)` dava sempre verdadeiro, o microfone abria
      // uma vez no boot e nunca mais. Aqui a sessão precisa poder abrir DEPOIS da montagem.
      sessao(false);
      await montar();
      expect(microfone.armar).not.toHaveBeenCalled();
      sessao(true);
      await proximaVolta();
      expect(microfone.armar).toHaveBeenCalledTimes(1);
    });

    it("já armado, não rearma a cada volta", async () => {
      sessao(true);
      await montar();
      microfone.estaArmado.mockReturnValue(true);
      await proximaVolta();
      await proximaVolta();
      expect(microfone.armar).toHaveBeenCalledTimes(1);
    });

    it("rearma quando a faixa morre", async () => {
      // Quem sabe se há microfone é o módulo, que confere a FAIXA. Uma cópia do estado aqui
      // envelhecia na primeira faixa que morria e travava o rearme para o resto da corrida.
      sessao(true);
      await montar();
      microfone.estaArmado.mockReturnValue(true);
      await proximaVolta();
      microfone.estaArmado.mockReturnValue(false); // a faixa caiu
      await proximaVolta();
      expect(microfone.armar).toHaveBeenCalledTimes(2);
    });

    it("um dispositivo lento não abre duas capturas do mesmo microfone", async () => {
      // `getUserMedia` leva de 100 a 500 ms e o laço bate a cada 2 s. Sem a trava, o
      // dispositivo lento rendia duas aberturas concorrentes.
      sessao(true);
      let concluir;
      microfone.armar.mockImplementation(
        () =>
          new Promise((r) => {
            concluir = r;
          }),
      );
      await montar();
      await proximaVolta();
      expect(microfone.armar).toHaveBeenCalledTimes(1);
      await act(async () => concluir());
    });

    it("ao fechar a sessão, cancela o rádio e desarma", async () => {
      sessao(true);
      await montar();
      const orq = orquestrador();
      microfone.estaArmado.mockReturnValue(true);
      sessao(false);
      await proximaVolta();
      expect(orq.cancelar).toHaveBeenCalledTimes(1);
      expect(microfone.desarmar).toHaveBeenCalledTimes(1);
    });

    it("fora de sessão e já desarmado, não cancela nada", async () => {
      // `cancelar()` cala o rádio. Chamado a cada volta do laço, cortaria as falas que o
      // engenheiro dá fora de sessão.
      sessao(false);
      await montar();
      const orq = orquestrador();
      await proximaVolta();
      await proximaVolta();
      expect(orq.cancelar).not.toHaveBeenCalled();
      expect(microfone.desarmar).not.toHaveBeenCalled();
    });

    it("a bancada de teste manda no microfone enquanto está aberta", async () => {
      sessao(true);
      estaEmTeste.mockReturnValue(true);
      await montar();
      await proximaVolta();
      expect(microfone.armar).not.toHaveBeenCalled();
      expect(microfone.desarmar).not.toHaveBeenCalled();
    });

    it("aquece o servidor uma vez por sessão aberta", async () => {
      // O cold start do Cloud Run é de 20 a 40 s, e a primeira pergunta pode vir na volta 1.
      sessao(true);
      await montar();
      await proximaVolta();
      expect(comandos().filter((n) => n === "ptt_aquecer")).toHaveLength(1);
    });

    it("a desmontagem desarma o microfone", async () => {
      sessao(true);
      const { unmount } = await montar();
      unmount();
      expect(microfone.desarmar).toHaveBeenCalled();
    });
  });

  describe("ligação salva do botão", () => {
    it("reempurra o gatilho ao backend na montagem", async () => {
      // O vigia mora no Rust e não conhece o localStorage. Sem isto, o botão só voltaria a
      // funcionar depois de abrir as Configurações — o último lugar em que se olha.
      await montar();
      const chamada = invoke.mock.calls.find(([n]) => n === "ptt_set_gatilho");
      expect(chamada[1]).toEqual({ gatilho: { tipo: "tecla", vk: 84 } });
    });

    it("reempurra quando a associação muda na MESMA janela", async () => {
      // `salvarGatilho` avisa com um `Event` cru, sem `key` — só o evento `storage` tem uma.
      // Cobrar a chave dos dois descartava justo este caminho, que é o do jogador trocando a
      // tecla nas Configurações com a corrida a caminho.
      await montar();
      const antes = comandos().filter((n) => n === "ptt_set_gatilho").length;
      act(() => {
        window.dispatchEvent(new Event("loop:ptt-gatilho"));
      });
      expect(comandos().filter((n) => n === "ptt_set_gatilho")).toHaveLength(antes + 1);
    });

    it("reempurra quando a troca vem de outra janela", async () => {
      await montar();
      const antes = comandos().filter((n) => n === "ptt_set_gatilho").length;
      act(() => {
        window.dispatchEvent(new StorageEvent("storage", { key: "loop.pttGatilho" }));
      });
      expect(comandos().filter((n) => n === "ptt_set_gatilho")).toHaveLength(antes + 1);
    });

    it("ignora o storage de outra chave", async () => {
      // O evento `storage` chega para TODA chave do domínio, e o app grava muita coisa nele.
      // Sem o filtro, cada gravação de outra tela viraria uma ida ao backend.
      await montar();
      const antes = comandos().filter((n) => n === "ptt_set_gatilho").length;
      act(() => {
        window.dispatchEvent(new StorageEvent("storage", { key: "loop.outraCoisa" }));
      });
      expect(comandos().filter((n) => n === "ptt_set_gatilho")).toHaveLength(antes);
    });

    it("para de ouvir a troca depois de desmontar", async () => {
      const { unmount } = await montar();
      unmount();
      const antes = comandos().filter((n) => n === "ptt_set_gatilho").length;
      act(() => {
        window.dispatchEvent(new Event("loop:ptt-gatilho"));
      });
      expect(comandos().filter((n) => n === "ptt_set_gatilho")).toHaveLength(antes);
    });
  });

  describe("voz própria e portão de momento", () => {
    it("registra a peça do sobrenome do jogador", async () => {
      invoke.mockImplementation((nome) =>
        nome === "engenheiro_voz_propria"
          ? Promise.resolve({ chave: "meu_nome", audio_b64: "AAA", mime: "audio/ogg" })
          : Promise.resolve(null),
      );
      await montar();
      expect(voz.registrarPecaPropria).toHaveBeenCalledWith("meu_nome", "AAA", "audio/ogg");
    });

    it("segue calado quando não há peça (sem rede, save novo)", async () => {
      await montar();
      expect(voz.registrarPecaPropria).not.toHaveBeenCalled();
    });

    it("instala o portão do momento quente e o solta ao desmontar", async () => {
      // O portão vale só para a fila de ANÚNCIOS. Deixá-lo instalado depois da desmontagem
      // deixaria a fila consultando um componente que não existe mais.
      const { unmount } = await montar();
      expect(voz.definirPortao).toHaveBeenCalledWith(expect.any(Function));
      unmount();
      expect(voz.definirPortao).toHaveBeenLastCalledWith(null);
    });
  });

  describe("ocasiões sem pressa", () => {
    it("fala a ocasião com o texto junto, para o registro do rádio", async () => {
      // É a fala mais longa do sistema e a única escrita pelo servidor: sem o texto, é a
      // única que o jogador não consegue reler no registro.
      invoke.mockImplementation((nome) => {
        if (nome === "engenheiro_ocasiao")
          return Promise.resolve({ linhas: ["volta de formação"], ocasiao: "formacao" });
        if (nome === "ptt_responder")
          return Promise.resolve({ audio_b64: "BBB", mime: "audio/ogg", texto: "Boa sorte" });
        return Promise.resolve(null);
      });
      await montar();
      await proximaVolta();
      expect(voz.anunciarRemoto).toHaveBeenCalledWith("BBB", "audio/ogg", {
        canal: "ocasiao",
        texto: "Boa sorte",
      });
    });

    it("sem ocasião, não pergunta nada ao servidor", async () => {
      await montar();
      await proximaVolta();
      expect(comandos()).not.toContain("ptt_responder");
    });
  });
});
