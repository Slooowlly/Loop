import { afterEach, describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import i18n from "../../../i18n/index.js";
import OfferCardRich from "./OfferCardRich";

// Os pontos fortes e fracos do companheiro atravessam a ponte como ID de atributo
// (`racecraft`, `gestao_pneus`), e não como prosa.
//
// Vinham prontos em português do `driver_strengths_weaknesses` no Rust — "Corpo a corpo",
// "Gestão de pneus" — e o card os imprimia crus. O jogador em en-US lia português no meio
// do scouting, e o auditor de i18n não via nada de errado: não há literal em português no
// JSX, o texto chega do backend.
//
// Aqui se prende as duas pontas: que o card TRADUZA o id, e que a tradução acompanhe o
// idioma ativo.

afterEach(async () => {
  await i18n.changeLanguage("pt-BR");
});

const OFERTA = {
  seat_id: "seat-1",
  team_id: "team-1",
  team_name: "Ardent Motorsport",
  team_color: "#58a6ff",
  category: "gt3",
  category_label: "GT3",
  category_tier: 3,
  role: "N2",
  salary: 480000,
  offer_duration: 2,
  team_country: "🇬🇧 Reino Unido",
  teammate_name: "Willem vanDijk",
  teammate_strengths: ["racecraft", "gestao_pneus"],
  teammate_weaknesses: ["fator_chuva", "habilidade_largada"],
};

describe("OfferCardRich — atributos do companheiro", () => {
  it("traduz o id do atributo em pt-BR", () => {
    render(<OfferCardRich offer={OFERTA} />);

    expect(screen.getByText("Racecraft · Pneus")).toBeInTheDocument();
    expect(screen.getByText("Chuva · Largada")).toBeInTheDocument();
    // O id cru não pode chegar à tela.
    expect(screen.queryByText(/gestao_pneus/)).toBeNull();
    expect(screen.queryByText(/habilidade_largada/)).toBeNull();
  });

  it("acompanha o idioma em en-US", async () => {
    await i18n.changeLanguage("en-US");
    render(<OfferCardRich offer={OFERTA} />);

    expect(screen.getByText("Racecraft · Tires")).toBeInTheDocument();
    expect(screen.getByText("Rain · Start")).toBeInTheDocument();
    // A prosa antiga do backend era esta, e é exatamente o que não pode voltar.
    expect(screen.queryByText(/Pneus|Chuva|Largada/)).toBeNull();
  });
});
