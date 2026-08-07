import { render, screen } from "@testing-library/react";

import DriverCard from "./DriverCard";

// As duas leituras que vieram da garagem para cá: tempo de casa (metadados do piloto)
// e o peso dele na folha (colado no salário). Ambas somem quando o payload não as tem.
function renderCard(overrides = {}, props = {}) {
  return render(
    <DriverCard
      driver={{
        role: "N1",
        name: "Test 3",
        nationality: "brasileiro",
        nationalityLabel: "Brasileiro",
        salary: 60_000,
        hasDetail: true,
        age: 26,
        skill: 66,
        midia: 51,
        pontos: 0,
        vitorias: 0,
        podios: 0,
        championshipPosition: 8,
        isRookie: false,
        injury: null,
        costPerPoint: null,
        skillRank: 1,
        midiaRank: 4,
        tenureSeasons: 3,
        ...overrides,
      }}
      averages={{ skill: 50, midia: 50 }}
      hasGrid
      poolSize={12}
      payroll={100_000}
      teammateMedia={40}
      {...props}
    />,
  );
}

describe("DriverCard", () => {
  it("mostra tempo de casa e peso na folha", () => {
    renderCard();

    expect(screen.getByTestId("driver-tenure-N1")).toHaveTextContent("3 temporadas na equipe");
    expect(screen.getByTestId("driver-payroll-share-N1")).toHaveTextContent("60% da folha");
  });

  it("trata a primeira temporada como chegada, não como veterania", () => {
    renderCard({ tenureSeasons: 1 });

    expect(screen.getByTestId("driver-tenure-N1")).toHaveTextContent("1ª temporada na equipe");
  });

  it("diz a fatia da presença pública que cada mídia puxa", () => {
    // Mídia 51 contra 40 do companheiro: ele é o rosto (70%).
    renderCard({ midia: 51 });
    expect(screen.getByTestId("driver-media-N1")).toHaveTextContent(
      "É o rosto da equipe: 70% da presença pública sai da mídia dele.",
    );

    renderCard({ midia: 30 });
    expect(screen.getAllByTestId("driver-media-N1")[1]).toHaveTextContent(
      "Entra com 30% da presença pública da equipe.",
    );
  });

  it("volta à frase genérica sem a mídia do companheiro", () => {
    renderCard({}, { teammateMedia: null });

    expect(screen.getByTestId("driver-media-N1")).toHaveTextContent(
      "Alimenta a presença pública da equipe, que multiplica o patrocínio.",
    );
  });

  it("omite as duas linhas sem dado — save antigo e assento sem salário", () => {
    renderCard({ tenureSeasons: null, salary: 0 }, { payroll: 0 });

    expect(screen.queryByTestId("driver-tenure-N1")).toBeNull();
    expect(screen.queryByTestId("driver-payroll-share-N1")).toBeNull();
  });
});
