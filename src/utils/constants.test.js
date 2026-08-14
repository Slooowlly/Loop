import { describe, expect, it } from "vitest";

import { LOADING_MESSAGE_INTERVAL_MS } from "./constants";
import enUS from "../i18n/locales/en-US/common.json";
import ptBR from "../i18n/locales/pt-BR/common.json";

// As mensagens do overlay de carregamento do NewCareer moram NO LOCALE, em
// `newCareer.loadingMessages.msg<i>` — que é de onde o `t()` da tela lê. Este arquivo guardava
// uma segunda cópia dos 75 textos em português, em `LOADING_MESSAGES`, usada só pelo `.length`
// do ciclo; o teste protegia a cópia e não o que o jogador via, então dava para reescrever
// qualquer mensagem do locale sem nenhum guard reclamar. A cópia morreu; os testes daqui em
// diante olham a fonte real.

const chavesOrdenadas = (locale) =>
  Object.keys(locale.newCareer.loadingMessages).sort(
    (a, b) => Number(a.replace("msg", "")) - Number(b.replace("msg", "")),
  );

const mensagensPt = () => chavesOrdenadas(ptBR).map((k) => ptBR.newCareer.loadingMessages[k]);

/// Quantas mensagens a tela mostra em um minuto de geração, no intervalo configurado.
const mensagensEmUmMinuto = () => Math.ceil(60_000 / LOADING_MESSAGE_INTERVAL_MS);

describe("mensagens de carregamento (newCareer.loadingMessages)", () => {
  it("numera as chaves de msg0 a msgN-1, sem buraco, nos dois locales", () => {
    // O ciclo da tela é `(i + 1) % total`, então índice faltando vira chave sem tradução na
    // cara do jogador — a falha mais silenciosa possível, porque só aparece no segundo certo.
    for (const [nome, locale] of [
      ["pt-BR", ptBR],
      ["en-US", enUS],
    ]) {
      const chaves = chavesOrdenadas(locale);
      expect(chaves.length, `${nome} sem mensagens de carregamento`).toBeGreaterThan(0);
      expect(chaves, `${nome} com numeração furada`).toEqual(
        chaves.map((_, i) => `msg${i}`),
      );
    }
    expect(chavesOrdenadas(enUS).length).toBe(chavesOrdenadas(ptBR).length);
  });

  it("cobre um minuto de geração histórica sem repetir texto", () => {
    const mensagens = mensagensPt();

    expect(LOADING_MESSAGE_INTERVAL_MS).toBe(2000);
    expect(mensagens.length).toBeGreaterThanOrEqual(mensagensEmUmMinuto());
    expect(new Set(mensagens).size).toBe(mensagens.length);
  });

  it("mantém as primeiras mensagens compatíveis com saves que fecham perto de um minuto", () => {
    const primeiroMinuto = mensagensPt().slice(0, mensagensEmUmMinuto());

    expect(primeiroMinuto.join(" ")).not.toMatch(/2025|fase final|ultimos anos/i);
  });

  it("segue as fases largas da criação do rascunho histórico", () => {
    const mensagens = mensagensPt();
    const acharIndice = (padrao) => mensagens.findIndex((mensagem) => padrao.test(mensagem));

    const mundoBase = acharIndice(/base.*2000/i);
    const primeiraTemporada = acharIndice(/primeiras temporadas|início do arquivo/i);
    const mercado = acharIndice(/movimentando contratos|janela de evolução/i);
    const transicao = acharIndice(/promoções|rebaixamentos/i);
    const arquivo = acharIndice(/arquivos históricos|memória/i);
    const anoJogavel = acharIndice(/2025/i);

    expect(mundoBase).toBeGreaterThanOrEqual(0);
    expect(primeiraTemporada).toBeGreaterThan(mundoBase);
    expect(mercado).toBeGreaterThan(primeiraTemporada);
    expect(transicao).toBeGreaterThan(mercado);
    expect(arquivo).toBeGreaterThan(transicao);
    expect(anoJogavel).toBeGreaterThan(arquivo);
  });
});
