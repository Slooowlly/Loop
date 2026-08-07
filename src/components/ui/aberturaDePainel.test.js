import { ABERTURA_MS, pisoDeAbertura } from "./aberturaDePainel.js";

describe("pisoDeAbertura", () => {
  it("segura a abertura pelo compasso inteiro", async () => {
    vi.useFakeTimers();
    try {
      let resolvido = false;
      pisoDeAbertura(true, { forcar: true }).then(() => {
        resolvido = true;
      });

      // O ponto do compasso: sem ele um save pequeno resolve o invoke em 20ms e
      // a ficha aparece de estalo, sem a sequência de abertura.
      await vi.advanceTimersByTimeAsync(ABERTURA_MS - 50);
      expect(resolvido).toBe(false);

      await vi.advanceTimersByTimeAsync(50);
      expect(resolvido).toBe(true);
    } finally {
      vi.useRealTimers();
    }
  });

  it("nao segura quando o carregamento nao e uma abertura", async () => {
    // Trocar de piloto ou de equipe com o painel já na tela é navegação, e
    // navegação não espera.
    let resolvido = false;
    await pisoDeAbertura(false, { forcar: true }).then(() => {
      resolvido = true;
    });
    expect(resolvido).toBe(true);
  });
});
