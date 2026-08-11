import { escalaDePosicao, marcasDePosicao, rotuloVaiAcima } from "./CurvaDeCampeonato.jsx";

// ---------------------------------------------------------------------------
// A escala da curva de campeonato: eixo INVERTIDO (P1 no topo) e categórico no
// X. Só três contas, mas as três decidem se o ano do título aparece inteiro ou
// cortado pela borda — e nenhuma delas falha de forma visível no console.
// ---------------------------------------------------------------------------

function ano(posicao, esperado) {
  return { posicao, esperado };
}

describe("escalaDePosicao", () => {
  it("conta a expectativa junto com o resultado para o eixo caber nas duas linhas", () => {
    // O ano em que ele salvou um carro ruim: P4 com expectativa de P18. Um eixo
    // que fosse só ate P4 mandaria a linha da referencia para fora do rodape.
    expect(escalaDePosicao([ano(1, 3), ano(4, 18)]).teto).toBe(18);
  });

  it("da meia posicao de folga em cada ponta para o campeao nao sair cortado", () => {
    const escala = escalaDePosicao([ano(1, 2), ano(10, 8)]);

    // Fracao 0 seria a borda: P1 pousa dentro do plot, nao em cima dela.
    expect(escala.fracao(1)).toBeGreaterThan(0);
    expect(escala.fracao(escala.teto)).toBeLessThan(1);
    expect(escala.fracao(0.5)).toBe(0);
    expect(escala.fracao(escala.teto + 0.5)).toBe(1);
  });

  it("grampeia o que passar das pontas em vez de vazar do quadro", () => {
    const escala = escalaDePosicao([ano(1, 2), ano(10, 8)]);

    expect(escala.fracao(-5)).toBe(0);
    expect(escala.fracao(999)).toBe(1);
  });

  it("ignora posicao zerada ou negativa de payload antigo", () => {
    expect(escalaDePosicao([ano(1, 0), ano(3, -1)]).teto).toBe(3);
  });

  it("mantem um eixo utilizavel quando nenhuma temporada tem posicao", () => {
    const escala = escalaDePosicao([]);

    expect(escala.teto).toBe(2);
    expect(Number.isFinite(escala.fracao(1))).toBe(true);
  });
});

describe("marcasDePosicao", () => {
  it("comeca sempre em P1, que e a referencia de leitura do grafico", () => {
    expect(marcasDePosicao(12)[0]).toBe(1);
    expect(marcasDePosicao(2)[0]).toBe(1);
    expect(marcasDePosicao(40)[0]).toBe(1);
  });

  it("escolhe passo redondo e nunca passa de cinco degraus alem do P1", () => {
    expect(marcasDePosicao(2)).toEqual([1, 2]);
    expect(marcasDePosicao(12)).toEqual([1, 5, 10]);
    expect(marcasDePosicao(24)).toEqual([1, 5, 10, 15, 20]);
    expect(marcasDePosicao(30)).toEqual([1, 10, 20, 30]);
    expect(marcasDePosicao(90)).toEqual([1, 20, 40, 60, 80]);
  });

  it("nunca marca alem do teto do eixo", () => {
    for (const teto of [2, 5, 9, 18, 33, 120]) {
      expect(Math.max(...marcasDePosicao(teto))).toBeLessThanOrEqual(teto);
    }
  });

  it("nao repete o P1 quando o passo comeca nele", () => {
    expect(marcasDePosicao(3).filter((marca) => marca === 1)).toHaveLength(1);
  });
});

describe("rotuloVaiAcima", () => {
  const quadro = { padT: 14, alturaPlot: 150 };
  const y = (posicao) => posicao;

  it("foge da linha da expectativa: escreve do lado oposto a ela", () => {
    // Expectativa abaixo do resultado (y maior): o numero sobe.
    expect(rotuloVaiAcima(60, y, { esperado: 120 }, quadro)).toBe(true);
    // Expectativa por cima: o numero desce.
    expect(rotuloVaiAcima(60, y, { esperado: 20 }, quadro)).toBe(false);
  });

  it("prefere acima quando o ano nao tem expectativa medida", () => {
    expect(rotuloVaiAcima(60, y, {}, quadro)).toBe(true);
  });

  it("desiste da preferencia no teto do eixo, onde o rotulo sairia do cartao", () => {
    // Exatamente o ano do titulo: encostado no topo e sem lugar acima.
    expect(rotuloVaiAcima(20, y, { esperado: 120 }, quadro)).toBe(false);
  });

  it("desiste da preferencia no rodape do plot pelo mesmo motivo", () => {
    expect(rotuloVaiAcima(158, y, { esperado: 20 }, quadro)).toBe(true);
  });
});
