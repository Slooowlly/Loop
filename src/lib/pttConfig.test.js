// A validação da associação salva.
//
// Parece defesa contra o improvável, e não é: o `localStorage` atravessa versões do app.
// Um formato antigo — ou um `{tipo:"tecla"}` sem código — chegaria ao vigia no Rust como
// um gatilho que nunca dispara (o jogador aperta e nada acontece, sem erro nenhum) ou,
// no caso do código 0, como um que o Windows responde de forma indefinida. Os dois modos
// de falha são silenciosos, e é por isso que a peneira fica aqui, antes do `invoke`.

import { beforeEach, describe, expect, it } from "vitest";
import {
  GATILHO_STORE,
  lerGatilhoSalvo,
  lerMicSalvo,
  rotuloDoGatilho,
  salvarGatilho,
  salvarMic,
  validar,
} from "./pttConfig";

describe("validar", () => {
  it("aceita um botão de volante dentro dos tetos da API do Windows", () => {
    expect(validar({ tipo: "volante", dispositivo: 0, botao: 0 })).toEqual({
      tipo: "volante",
      dispositivo: 0,
      botao: 0,
    });
    expect(validar({ tipo: "volante", dispositivo: 15, botao: 31 })).toBeTruthy();
  });

  it("recusa fora dos tetos — 16 dispositivos e 32 botões é o que a API antiga enxerga", () => {
    expect(validar({ tipo: "volante", dispositivo: 16, botao: 0 })).toBeNull();
    expect(validar({ tipo: "volante", dispositivo: 0, botao: 32 })).toBeNull();
    expect(validar({ tipo: "volante", dispositivo: -1, botao: 0 })).toBeNull();
  });

  it("recusa a tecla de código zero", () => {
    // 0 é o que chega quando a captura falha em ler a tecla. Deixá-lo passar produziria
    // um gatilho de comportamento indefinido — no pior caso, o microfone aberto sozinho.
    expect(validar({ tipo: "tecla", codigo: 0 })).toBeNull();
    expect(validar({ tipo: "tecla", codigo: 256 })).toBeNull();
    expect(validar({ tipo: "tecla", codigo: 32 })).toEqual({ tipo: "tecla", codigo: 32 });
  });

  it("recusa o que não é gatilho nenhum", () => {
    for (const lixo of [null, undefined, 7, "volante", {}, { tipo: "pedal" }, { tipo: "volante" }]) {
      expect(validar(lixo)).toBeNull();
    }
  });

  it("descarta campos extras em vez de repassá-los ao backend", () => {
    const g = validar({ tipo: "volante", dispositivo: 1, botao: 2, rotulo: "sobra" });
    expect(Object.keys(g).sort()).toEqual(["botao", "dispositivo", "tipo"]);
  });
});

describe("persistência", () => {
  beforeEach(() => localStorage.clear());

  it("guarda, lê de volta e apaga", () => {
    salvarGatilho({ tipo: "tecla", codigo: 112 });
    expect(lerGatilhoSalvo()).toEqual({ tipo: "tecla", codigo: 112 });
    salvarGatilho(null);
    expect(lerGatilhoSalvo()).toBeNull();
  });

  it("um valor corrompido no armazenamento lê como 'não associado'", () => {
    localStorage.setItem(GATILHO_STORE, "{isso não é json");
    expect(lerGatilhoSalvo()).toBeNull();
  });

  it("um formato antigo lê como 'não associado' em vez de virar gatilho morto", () => {
    localStorage.setItem(GATILHO_STORE, JSON.stringify({ dispositivo: 0, botao: 3 }));
    expect(lerGatilhoSalvo()).toBeNull();
  });
});

describe("microfone escolhido", () => {
  beforeEach(() => localStorage.clear());

  it("guarda e devolve o dispositivo", () => {
    salvarMic("abc123");
    expect(lerMicSalvo()).toBe("abc123");
  });

  it("sem escolha devolve null, que é 'use o padrão do Windows'", () => {
    expect(lerMicSalvo()).toBeNull();
    salvarMic("abc123");
    salvarMic(null);
    expect(lerMicSalvo()).toBeNull();
  });

  it("string vazia conta como sem escolha, e não como um dispositivo chamado ''", () => {
    // O `select` das configurações usa "" para o padrão. Se isso virasse um deviceId,
    // `getUserMedia` receberia `{ exact: "" }` e falharia em toda abertura.
    salvarMic("");
    expect(lerMicSalvo()).toBeNull();
  });
});

describe("rótulo", () => {
  it("conta os botões a partir de 1, que é como o volante é rotulado", () => {
    // A API entrega índice base zero; o botão 1 do volante é o índice 0. Mostrar "botão 0"
    // faria o jogador procurar um botão que não existe na serigrafia.
    expect(rotuloDoGatilho({ tipo: "volante", dispositivo: 0, botao: 0 })).toContain("botão 1");
  });

  it("usa o nome da tecla quando ele existe", () => {
    expect(rotuloDoGatilho({ tipo: "tecla", codigo: 112 }, "F1")).toBe("F1");
    expect(rotuloDoGatilho({ tipo: "tecla", codigo: 112 })).toContain("112");
  });
});
