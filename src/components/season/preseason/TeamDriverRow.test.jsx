import { render, screen } from "@testing-library/react";
import TeamDriverRow from "./TeamDriverRow";

// Os marcadores do assento na FOTO da semana 1. Os dois dizem "este assento pode sumir
// na virada", mas por motivos diferentes, e o jogador precisa distinguir de relance:
// contrato vencendo o piloto ainda pode renovar; aposentadoria é definitiva.
//
// A asserção é no emoji, e não na chave de i18n, porque é o emoji que carrega a
// informação na tela — o tooltip só confirma o que ele já disse.
describe("TeamDriverRow — marcadores da foto", () => {
  it("marca contrato vencendo com o papel", () => {
    render(<TeamDriverRow driverName="Willem vanDijk" tenureSeasons={5} contratoVence />);
    expect(screen.getByText("\u{1F4C4}")).toBeInTheDocument();
    expect(screen.queryByText("\u{1F6AA}")).toBeNull();
  });

  it("marca o aposentado com a porta, e NAO com o papel", () => {
    render(<TeamDriverRow driverName="Bastien Durand" tenureSeasons={3} aposentado />);
    expect(screen.getByText("\u{1F6AA}")).toBeInTheDocument();
    expect(screen.queryByText("\u{1F4C4}")).toBeNull();
  });

  // Quem se aposenta sai independentemente de quanto contrato ainda tinha, então os dois
  // marcadores juntos contariam a mesma saída duas vezes — e o motivo que vale é a porta.
  it("aposentadoria tem precedencia sobre contrato vencendo", () => {
    render(<TeamDriverRow driverName="Bastien Durand" tenureSeasons={3} contratoVence aposentado />);
    expect(screen.getByText("\u{1F6AA}")).toBeInTheDocument();
    expect(screen.queryByText("\u{1F4C4}")).toBeNull();
  });

  it("sem marcador quando o assento esta em dia", () => {
    render(<TeamDriverRow driverName="Jack Clarke" tenureSeasons={5} />);
    expect(screen.queryByText("\u{1F6AA}")).toBeNull();
    expect(screen.queryByText("\u{1F4C4}")).toBeNull();
  });
});
