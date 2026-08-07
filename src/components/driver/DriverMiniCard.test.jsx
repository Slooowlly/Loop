import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

import DriverMiniCard from "./DriverMiniCard";

vi.mock("../../stores/useCareerStore", () => ({
  default: (selector) => selector({ careerId: "C1" }),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

function dossie(overrides = {}) {
  return {
    id: "D1",
    nome: "Vasco Santos",
    nacionalidade: "🇵🇹 Portugal",
    idade: 24,
    equipe_nome: "Grid Start Racing School",
    equipe_cor_primaria: "#58a6ff",
    personalidade_primaria: { tipo: "Calculista", emoji: "🧠", descricao: "" },
    perfil: { nacionalidade: "🇵🇹 Portugal", licenca: { nivel: "Rookie", sigla: "R" } },
    contrato: { salario_anual: 120000, anos_restantes: 2 },
    performance: {
      temporada: { corridas: 12, vitorias: 3, podios: 7, poles: 2, dnfs: 1 },
      carreira: { corridas: 40, vitorias: 5, podios: 14, poles: 3, dnfs: 4 },
    },
    forma: {
      ultimas_5: [
        { rodada: 8, chegada: 4, dnf: false },
        { rodada: 9, chegada: 1, dnf: false },
        { rodada: 10, chegada: null, dnf: true },
      ],
      media_chegada: 3.5,
    },
    competitivo: {
      qualidades: [{ attribute_name: "ritmo", tag_text: "Ritmo de corrida", color: "#3fb950" }],
    },
    trajetoria: {
      titulos: 2,
      curva_campeonato: [
        // Duas temporadas na casa anterior, duas na atual — uma delas com titulo.
        { ano: 2022, equipe_nome: "Late Apex Contenders", corridas: 10, vitorias: 0, podios: 1, titulo: false },
        { ano: 2023, equipe_nome: "Late Apex Contenders", corridas: 10, vitorias: 1, podios: 3, titulo: false },
        { ano: 2024, equipe_nome: "Grid Start Racing School", corridas: 11, vitorias: 2, podios: 5, titulo: true },
        { ano: 2025, equipe_nome: "Grid Start Racing School", corridas: 12, vitorias: 3, podios: 7, titulo: false },
      ],
    },
    ...overrides,
  };
}

function montar(driverId = "D1") {
  return render(
    <DriverMiniCard driverId={driverId}>
      <p>Vasco Santos</p>
    </DriverMiniCard>,
  );
}

describe("DriverMiniCard", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue(dossie());
  });

  it("nao abre nada antes do clique", () => {
    montar();
    expect(screen.queryByTestId("driver-mini-card")).toBeNull();
    expect(invoke).not.toHaveBeenCalled();
  });

  // O nome sem id de piloto — assento cujo ocupante nao foi resolvido — volta
  // intacto: um nome que parece clicavel e nao abre nada e pior do que um nome
  // que nunca prometeu abrir.
  it("sem driverId devolve o filho sem virar gatilho", () => {
    montar(null);
    const nome = screen.getByText("Vasco Santos");
    expect(nome.getAttribute("role")).toBeNull();
    fireEvent.click(nome);
    expect(screen.queryByTestId("driver-mini-card")).toBeNull();
  });

  it("abre a ficha no clique e mostra o essencial do piloto", async () => {
    montar();
    fireEvent.click(screen.getByText("Vasco Santos"));

    const ficha = await screen.findByTestId("driver-mini-card");
    expect(invoke).toHaveBeenCalledWith("get_driver_detail", {
      careerId: "C1",
      driverId: "D1",
    });

    await waitFor(() => expect(ficha.textContent).toContain("Grid Start Racing School"));
    // Nacionalidade escrita, idade, forma e traco.
    expect(ficha.textContent).toContain("Portugal");
    expect(ficha.textContent).toContain("24 anos");
    expect(ficha.textContent).toContain("Ritmo de corrida");
    expect(ficha.textContent).toContain("AB");
  });

  // A ficha abre na janela de transferencias, onde metade dos nomes e de piloto
  // sem equipe: temporada zerada e o caso NORMAL ali. Abrir com quatro zeros
  // dizia que o piloto nao vale nada quando o que ele tem esta na carreira.
  it("mostra a carreira sempre e a temporada so quando houve temporada", async () => {
    invoke.mockResolvedValue(
      dossie({
        performance: {
          temporada: { corridas: 0, vitorias: 0, podios: 0, poles: 0, dnfs: 0 },
          carreira: { corridas: 50, vitorias: 1, podios: 2, poles: 0, dnfs: 6 },
        },
      }),
    );
    montar();
    fireEvent.click(screen.getByText("Vasco Santos"));

    const ficha = await screen.findByTestId("driver-mini-card");
    await waitFor(() => expect(ficha.textContent).toContain("Carreira"));
    expect(ficha.textContent).toContain("50");
    expect(ficha.textContent).not.toContain("Temporada");
  });

  // Titulo e o numero que ordena a fila, e faltava na ficha inteira.
  it("mostra os titulos de carreira", async () => {
    montar();
    fireEvent.click(screen.getByText("Vasco Santos"));

    const ficha = await screen.findByTestId("driver-mini-card");
    await waitFor(() => expect(ficha.textContent).toContain("Títulos"));
  });

  // "55 corridas na carreira" e "Club Racer Motorsport" sao dois fatos soltos: o
  // que decide renovacao e o que aconteceu DEPOIS que ele entrou ali.
  it("soma a campanha na equipe atual e ignora as casas anteriores", async () => {
    montar();
    fireEvent.click(screen.getByText("Vasco Santos"));

    const ficha = await screen.findByTestId("driver-mini-card");
    await waitFor(() => expect(ficha.textContent).toContain("Nesta Equipe"));

    // 11 + 12 corridas nas duas temporadas da casa atual — as 20 da casa
    // anterior nao entram. Medido DENTRO da secao, e nao no card inteiro: solto
    // no textContent o numero passaria por qualquer outro.
    const secao = screen.getByText("Nesta Equipe").closest("div");
    expect(secao.textContent).toContain("23");
    expect(secao.textContent).toContain("2Temp.");

    // O nome da equipe aparece UMA vez, no cabecalho. Repeti-lo no titulo da
    // secao gastava a linha inteira para nao dizer nada — quem faz o vinculo
    // agora e a cor.
    const ocorrencias = screen.getAllByText(/Grid Start Racing School/);
    expect(ocorrencias).toHaveLength(1);
  });

  // A casa atual vem ANTES da carreira: a pergunta da janela e sobre o assento
  // que esta na tela, e o acumulado e o pano de fundo contra o qual ele se le.
  it("poe a campanha na casa atual acima da carreira", async () => {
    montar();
    fireEvent.click(screen.getByText("Vasco Santos"));

    const ficha = await screen.findByTestId("driver-mini-card");
    await waitFor(() => expect(ficha.textContent).toContain("Nesta Equipe"));
    expect(ficha.textContent.indexOf("Nesta Equipe")).toBeLessThan(
      ficha.textContent.indexOf("Carreira"),
    );
  });

  // Contratado e ainda sem largada por eles: "1 Temporada · 0 · 0 · 0" conta uma
  // campanha que nao existe, e cinco zeros lidos rapido dizem "fracassou aqui".
  it("recem-chegado sem largada diz New em vez de uma temporada zerada", async () => {
    invoke.mockResolvedValue(
      dossie({
        trajetoria: {
          titulos: 0,
          curva_campeonato: [
            { ano: 2025, equipe_nome: "Grid Start Racing School", corridas: 0, vitorias: 0, podios: 0, titulo: false },
          ],
        },
      }),
    );
    montar();
    fireEvent.click(screen.getByText("Vasco Santos"));

    const ficha = await screen.findByTestId("driver-mini-card");
    await waitFor(() => expect(ficha.textContent).toContain("Nesta Equipe"));

    const secao = screen.getByText("Nesta Equipe").closest("div");
    expect(secao.textContent).toContain("New");
    expect(secao.textContent).toContain("Ainda não largou por eles");
    expect(secao.textContent).not.toContain("Temp.");
  });

  it("piloto sem equipe nao ganha o bloco da campanha na casa", async () => {
    invoke.mockResolvedValue(dossie({ equipe_nome: null, contrato: null }));
    montar();
    fireEvent.click(screen.getByText("Vasco Santos"));

    const ficha = await screen.findByTestId("driver-mini-card");
    await waitFor(() => expect(ficha.textContent).toContain("Sem equipe"));
    expect(ficha.textContent).not.toContain("Nesta Equipe");
  });

  // A licenca repetia com outra grafia o degrau em que o piloto corre.
  it("nao mostra a licenca do piloto", async () => {
    montar();
    fireEvent.click(screen.getByText("Vasco Santos"));

    const ficha = await screen.findByTestId("driver-mini-card");
    await waitFor(() => expect(ficha.textContent).toContain("24 anos"));
    expect(ficha.textContent).not.toContain("Rookie");
  });

  it("fecha no Escape", async () => {
    montar();
    fireEvent.click(screen.getByText("Vasco Santos"));
    await screen.findByTestId("driver-mini-card");

    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() => expect(screen.queryByTestId("driver-mini-card")).toBeNull());
  });

  it("segundo clique no mesmo nome fecha a ficha", async () => {
    montar();
    const nome = screen.getByText("Vasco Santos");
    fireEvent.click(nome);
    await screen.findByTestId("driver-mini-card");

    fireEvent.click(nome);
    await waitFor(() => expect(screen.queryByTestId("driver-mini-card")).toBeNull());
  });

  // Ultimo ano de contrato nao e "0 temporadas": e o sinal de que o piloto vai
  // ao mercado, que e a razao de a ficha existir na tela do mercado.
  it("diz ultimo ano quando o contrato acaba nesta virada", async () => {
    invoke.mockResolvedValue(
      dossie({ id: "D2", contrato: { salario_anual: 90000, anos_restantes: 0 } }),
    );
    render(
      <DriverMiniCard driverId="D2">
        <p>Vasco Santos</p>
      </DriverMiniCard>,
    );
    fireEvent.click(screen.getByText("Vasco Santos"));

    const ficha = await screen.findByTestId("driver-mini-card");
    await waitFor(() => expect(ficha.textContent).toContain("Último ano de contrato"));
  });
});
