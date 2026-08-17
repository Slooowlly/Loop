import {
  alturaNaturalDaGrade,
  anosDoPainel,
  comFaixasQueJaCorreram,
  familiasParaAbas,
} from "./DraftAtlasPreview";

// A linha do tempo real, na forma em que ela chega no payload: cada família com as
// faixas dela e o ano em que cada faixa passa a existir.
const payload = {
  families: [
    {
      id: "mazda",
      label: "Mazda",
      bands: [
        { key: "production_mazda", starts_year: 2018 },
        { key: "mazda_amador", starts_year: 2016 },
        { key: "mazda_rookie", starts_year: 2020 },
      ],
    },
    {
      id: "gt3",
      label: "GT3",
      bands: [
        { key: "endurance_gt3", starts_year: 2004 },
        { key: "gt3", starts_year: 1999 },
      ],
    },
  ],
};

/// O mesmo mundo, visto de um ano qualquer da geração.
function em(ano, extras = {}) {
  return { ...payload, in_progress: true, current_year: ano, ...extras };
}

describe("familiasParaAbas", () => {
  it("não oferece aba enquanto existe um mundo só", () => {
    expect(familiasParaAbas(em(1999))).toEqual([]);
    expect(familiasParaAbas(em(2015))).toEqual([]);
  });

  // O caso da tela vazia: em 2016 o Mazda Championship existe no calendário e ainda não
  // tem campeonato decidido, então clicar na aba trocava um mundo inteiro por uma coluna.
  it("espera a família nova ter uma temporada fechada, e não só nascer", () => {
    expect(familiasParaAbas(em(2016, { last_completed_year: 2015 }))).toEqual([]);
  });

  it("abre as abas quando a primeira temporada da família nova é decidida", () => {
    expect(familiasParaAbas(em(2017, { last_completed_year: 2016 })).map((familia) => familia.id)).toEqual([
      "gt3",
      "mazda",
    ]);
  });

  it("ordena pela escada, com o GT3 na frente por ser onde o mundo começou", () => {
    // No payload o Mazda vem primeiro; a ordem de leitura é a da história.
    expect(familiasParaAbas(em(2025, { last_completed_year: 2024 })).map((f) => f.label)).toEqual([
      "GT3",
      "Mazda",
    ]);
  });

  it("não quebra sem ano e sem linha do tempo, que é o estado da primeira leitura", () => {
    expect(familiasParaAbas(payload)).toEqual([]);
    expect(familiasParaAbas(null)).toEqual([]);
    expect(familiasParaAbas(em(2020, { families: [] }))).toEqual([]);
  });
});

// O mundo em 2002: o GT3 já correu três temporadas, o endurance só nasce em 2004.
const mundoEm2002 = {
  current_year: 2002,
  in_progress: true,
  bands: [
    {
      key: "endurance_gt3",
      starts_year: 2004,
      rows: [{ team_id: "T9", points: [] }],
    },
    {
      key: "gt3",
      starts_year: 1999,
      rows: [
        { team_id: "T1", points: [{ year: 2000, position: 1 }, { year: 2001, position: 2 }] },
        { team_id: "T2", points: [{ year: 2000, position: 2 }, { year: 2002, position: 1 }] },
      ],
    },
  ],
};

describe("comFaixasQueJaCorreram", () => {
  it("tira a faixa que ainda não disputou temporada nenhuma", () => {
    const bands = comFaixasQueJaCorreram(mundoEm2002).bands;
    expect(bands.map((band) => band.key)).toEqual(["gt3"]);
  });

  // O caso que colocou Mazda Production e Mazda Rookie na tela em 2016: o payload
  // injeta a divisão da temporada em curso com zero ponto para cada equipe, e isso
  // passava por qualquer teste de "tem ponto".
  it("ignora o grid da temporada em andamento, que ainda não decidiu nada", () => {
    const soNoAnoEmCurso = {
      ...mundoEm2002,
      bands: mundoEm2002.bands.map((band) =>
        band.key === "endurance_gt3"
          ? { ...band, rows: [{ team_id: "T9", points: [{ year: 2002, position: 1 }] }] }
          : band,
      ),
    };
    expect(comFaixasQueJaCorreram(soNoAnoEmCurso).bands.map((band) => band.key)).toEqual(["gt3"]);
  });

  it("aceita a faixa quando o primeiro campeonato dela fecha", () => {
    const comEndurance = {
      ...mundoEm2002,
      bands: mundoEm2002.bands.map((band) =>
        band.key === "endurance_gt3"
          ? { ...band, rows: [{ team_id: "T9", points: [{ year: 2001, position: 1 }] }] }
          : band,
      ),
    };
    expect(comFaixasQueJaCorreram(comEndurance).bands.map((band) => band.key)).toEqual([
      "endurance_gt3",
      "gt3",
    ]);
  });

  it("usa o last_completed_year quando o backend o informa", () => {
    const comUltimoDecidido = { ...mundoEm2002, current_year: 2005, last_completed_year: 2001 };
    expect(comFaixasQueJaCorreram(comUltimoDecidido).bands.map((band) => band.key)).toEqual(["gt3"]);
  });
});

describe("anosDoPainel", () => {
  it("abre no primeiro ano com dado, sem a folga que a aba cheia reserva às etiquetas", () => {
    // A temporada em curso é 2002 e não vira coluna: o traço dela chega na borda.
    expect(anosDoPainel(comFaixasQueJaCorreram(mundoEm2002))).toEqual([2000, 2001]);
  });

  it("devolve vazio enquanto nenhuma temporada foi arquivada", () => {
    expect(anosDoPainel({ bands: [] })).toEqual([]);
    expect(anosDoPainel(null)).toEqual([]);
  });
});

describe("alturaNaturalDaGrade", () => {
  it("pede uma linha por equipe, mais o cabeçalho e o rodapé da faixa", () => {
    // Uma divisão de 6 equipes: 44 de cabeçalho, 8 de rodapé e 6 linhas de 30.
    expect(alturaNaturalDaGrade([{ id: "mazda_amador", rowCount: 6 }])).toBe(44 + 8 + 6 * 30);
  });

  it("soma o vão entre faixas, e só entre elas", () => {
    const duas = alturaNaturalDaGrade([
      { id: "endurance_gt3", rowCount: 6 },
      { id: "gt3", rowCount: 14 },
    ]);
    expect(duas).toBe(2 * (44 + 8) + 12 + 20 * 30);
  });

  // O crescimento é o que a animação de abertura revela: a segunda faixa chega e a área
  // pede mais altura, em vez de espremer o que já estava na tela.
  it("cresce quando um campeonato novo entra na família", () => {
    const so = alturaNaturalDaGrade([{ id: "gt3", rowCount: 14 }]);
    const comEndurance = alturaNaturalDaGrade([
      { id: "endurance_gt3", rowCount: 6 },
      { id: "gt3", rowCount: 14 },
    ]);
    expect(comEndurance).toBeGreaterThan(so);
  });
});
