import { describe, expect, it } from "vitest";

import { comLimite } from "./filaDeCarga";

// O semáforo do acervo de voz. Ele não é conveniência: sem teto, o `fetch` das 3.943
// peças do engenheiro é RECUSADO pelo navegador (`ERR_INSUFFICIENT_RESOURCES`), e a
// recusa não distingue quem pediu — as 51 falas do spotter morriam junto.
//
// O estado (`ativos`, `espera`) é de MÓDULO, compartilhado entre os testes deste
// arquivo. Por isso todo caso abaixo drena a própria fila antes de terminar: um teste
// que deixa tarefa presa envenena o seguinte com um teto já ocupado.

/// Tarefa que só termina quando o teste mandar. Devolve o gatilho junto.
function tarefaControlada() {
  let liberar;
  let falhar;
  const promessa = new Promise((resolve, reject) => {
    liberar = resolve;
    falhar = reject;
  });
  let iniciou = false;
  const fn = () => {
    iniciou = true;
    return promessa;
  };
  return {
    fn,
    liberar,
    falhar,
    get iniciou() {
      return iniciou;
    },
  };
}

/// Deixa a microtask queue rodar — `comLimite` inicia via `Promise.resolve().then`,
/// então "já começou?" só é verdade depois de um tique.
const tique = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("comLimite", () => {
  it("devolve o valor de quem rodou, sem embrulhar", async () => {
    await expect(comLimite(() => "peça")).resolves.toBe("peça");
    await expect(comLimite(async () => 42)).resolves.toBe(42);
  });

  it("propaga a falha em vez de engolir — quem pediu a peça precisa saber", async () => {
    await expect(comLimite(() => Promise.reject(new Error("404")))).rejects.toThrow("404");
    // Erro SÍNCRONO também: `Promise.resolve().then(fn)` o converte em rejeição, e é
    // isso que impede um `throw` cru de vazar sem liberar a vaga.
    await expect(
      comLimite(() => {
        throw new Error("caminho inválido");
      }),
    ).rejects.toThrow("caminho inválido");
  });

  it("segura o excedente na fila: no máximo 6 rodando ao mesmo tempo", async () => {
    const tarefas = Array.from({ length: 10 }, tarefaControlada);
    const pedidos = tarefas.map((t) => comLimite(t.fn));

    await tique();

    // O teto é 6 — as outras quatro ficam esperando vaga, não disparam junto.
    expect(tarefas.filter((t) => t.iniciou)).toHaveLength(6);
    expect(tarefas.slice(6).every((t) => !t.iniciou)).toBe(true);

    // Uma vaga liberada puxa exatamente UMA da fila.
    tarefas[0].liberar("ok");
    await tique();
    expect(tarefas.filter((t) => t.iniciou)).toHaveLength(7);

    tarefas.forEach((t) => t.liberar("ok"));
    await expect(Promise.all(pedidos)).resolves.toHaveLength(10);
  });

  it("a vaga volta mesmo quando a peça falha — uma falha não pode travar a fila", async () => {
    const tarefas = Array.from({ length: 7 }, tarefaControlada);
    const pedidos = tarefas.map((t) => comLimite(t.fn).catch(() => "falhou"));

    await tique();
    expect(tarefas[6].iniciou).toBe(false);

    // A sétima só entra se o `finally` do módulo devolver a vaga da que estourou.
    tarefas[0].falhar(new Error("ERR_INSUFFICIENT_RESOURCES"));
    await tique();
    expect(tarefas[6].iniciou).toBe(true);

    tarefas.slice(1).forEach((t) => t.liberar("ok"));
    await expect(Promise.all(pedidos)).resolves.toEqual([
      "falhou",
      "ok",
      "ok",
      "ok",
      "ok",
      "ok",
      "ok",
    ]);
  });

  it("preserva a ordem de chegada da fila", async () => {
    const tarefas = Array.from({ length: 9 }, tarefaControlada);
    const pedidos = tarefas.map((t) => comLimite(t.fn));

    await tique();

    // Libera as seis em voo de uma vez: as três da fila entram na ordem em que pediram.
    tarefas.slice(0, 3).forEach((t) => t.liberar("ok"));
    await tique();

    expect(tarefas[6].iniciou).toBe(true);
    expect(tarefas[7].iniciou).toBe(true);
    expect(tarefas[8].iniciou).toBe(true);

    tarefas.forEach((t) => t.liberar("ok"));
    await Promise.all(pedidos);
  });

  it("depois de drenar a fila, o teto volta ao começo — o contador não vaza", async () => {
    // Se `ativos` não voltasse a zero, este bloco de 6 já sairia enfileirado.
    const tarefas = Array.from({ length: 6 }, tarefaControlada);
    const pedidos = tarefas.map((t) => comLimite(t.fn));

    await tique();
    expect(tarefas.filter((t) => t.iniciou)).toHaveLength(6);

    tarefas.forEach((t) => t.liberar("ok"));
    await Promise.all(pedidos);
  });
});
