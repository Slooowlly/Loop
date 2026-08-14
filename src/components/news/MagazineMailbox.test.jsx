// A caixa de entrada desenha nome de piloto, de equipe e de pista que vêm do banco.
// O jogador escolhe o próprio nome na criação da carreira, e o resto do grid é
// gerado a partir de dados que ele também pode editar no save — ou seja, o texto
// que chega aqui não é confiável por construção.
//
// Este teste tranca a única garantia que importa: o que vem do banco aparece como
// TEXTO. Nada de <script>, nada de <img onerror>, nada de <b> forjado pelo dado. O
// corpo da mensagem deixou de ser HTML concatenado (`dangerouslySetInnerHTML`) e
// virou lista de trechos tipados; se alguém voltar atrás, este arquivo quebra.

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

import MagazineMailbox from "./MagazineMailbox";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const SCRIPT = "<script>window.__invadido = 1;</script>";
const IMG = '<img src=x onerror="window.__invadido = 1">';
const NEGRITO = "<b>Falso Negrito</b>";

const FACTS = {
  team_interest: {
    teams: [{ team_name: IMG }, { team_name: NEGRITO }],
    player_fama: 75,
  },
  head_to_head: {
    races_together: 3,
    player_ahead: 1,
    best_finish: 2,
    best_track: IMG,
    rival_name: SCRIPT,
    rival_team: NEGRITO,
  },
  title_favorite: {
    veteran: true,
    career_titles: 2,
    position: 1,
    points_lead: 5,
    leads_player: false,
    strong_attr: "racecraft",
    weak_attr: "defesa",
    driver_name: NEGRITO,
    driver_team: SCRIPT,
  },
};

// Texto puro do leitor, como o jogador lê na tela.
const lido = (container) => container.querySelector(".reader-body").textContent;

describe("MagazineMailbox — dado do banco nunca vira marcação", () => {
  beforeEach(() => {
    invoke.mockReset();
    delete window.__invadido;
  });

  it("mostra os três nomes hostis como texto, em todas as mensagens", async () => {
    invoke.mockResolvedValue(FACTS);
    const { container } = render(<MagazineMailbox careerId="C1" />);

    await waitFor(() => expect(container.querySelectorAll(".mrow")).toHaveLength(3));

    for (const linha of container.querySelectorAll(".mrow")) {
      fireEvent.click(linha);
      await waitFor(() => expect(container.querySelector(".reader-body")).toBeTruthy());

      // Nenhum elemento nasceu do dado.
      expect(container.querySelector("script")).toBeNull();
      expect(container.querySelector("img")).toBeNull();
      expect(window.__invadido).toBeUndefined();
    }
  });

  it("interesse: nome de equipe com <img onerror> e <b> sai literal", async () => {
    invoke.mockResolvedValue(FACTS);
    const { container } = render(<MagazineMailbox careerId="C1" />);

    await waitFor(() => expect(container.querySelector(".reader-body")).toBeTruthy());
    const corpo = lido(container);
    expect(corpo).toContain(IMG);
    expect(corpo).toContain(NEGRITO);
    expect(container.querySelector(".reader-body img")).toBeNull();
  });

  it("confronto direto: rival com <script> e pista com <img> saem literais", async () => {
    invoke.mockResolvedValue(FACTS);
    const { container } = render(<MagazineMailbox careerId="C1" />);

    await waitFor(() => expect(container.querySelectorAll(".mrow")).toHaveLength(3));
    fireEvent.click(container.querySelectorAll(".mrow")[1]);

    await waitFor(() => expect(lido(container)).toContain(SCRIPT));
    expect(lido(container)).toContain(IMG);
    // O assunto também carrega o nome cru — e também é texto.
    expect(screen.getByRole("heading", { level: 3 }).textContent).toContain(SCRIPT);
  });

  it("com dado benigno, o HTML na tela é o mesmo de antes da troca", async () => {
    invoke.mockResolvedValue({
      head_to_head: {
        races_together: 3,
        player_ahead: 1,
        best_finish: 2,
        best_track: "Monza",
        rival_name: "Ruiz",
        rival_team: "RT",
      },
    });
    const { container } = render(<MagazineMailbox careerId="C1" />);

    await waitFor(() => expect(container.querySelector(".reader-body")).toBeTruthy());
    // Byte a byte o que o dangerouslySetInnerHTML produzia. Trocar o sink não podia
    // mexer em um espaço sequer da diagramação.
    expect(container.querySelector(".reader-body").innerHTML).toBe(
      "<p>Você e <b>Ruiz</b> (RT) já largaram juntos 3 vezes nesta categoria." +
        " Você terminou na frente em <b>1</b>, incluindo seu 2º em Monza.</p>",
    );
  });

  it("o negrito da tela é o nosso, não o do dado", async () => {
    invoke.mockResolvedValue(FACTS);
    const { container } = render(<MagazineMailbox careerId="C1" />);

    await waitFor(() => expect(container.querySelector(".reader-body")).toBeTruthy());
    const negritos = [...container.querySelectorAll(".reader-body b")].map((n) => n.textContent);

    // Os <b> que existem são os que o locale pediu: o nome de cada equipe (o texto
    // hostil inteiro, sem interpretar) e o nível de fama.
    expect(negritos).toContain(IMG);
    expect(negritos).toContain(NEGRITO);
    expect(negritos).toContain("Estrela");
    // "Falso Negrito" só existe como pedaço do texto literal, nunca sozinho num <b>.
    expect(negritos).not.toContain("Falso Negrito");
  });
});
