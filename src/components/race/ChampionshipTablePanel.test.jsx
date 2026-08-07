import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import ChampionshipTablePanel from "./ChampionshipTablePanel";

// A tabela do campeonato explica o DIA de cada piloto no hover. O que se testa aqui é o
// contrato entre o comando `get_weekend_modifiers` e o balão: o formato que o Rust emite
// (`driver_id`, `total_race`, `modifiers[].key`) tem que chegar traduzido na tela.

let instante = new Date(2026, 0, 1).getTime();

beforeEach(() => {
  vi.useFakeTimers();
  instante += 60_000;
  vi.setSystemTime(instante);
});

afterEach(() => {
  vi.useRealTimers();
});

function avancar(ms) {
  act(() => {
    vi.advanceTimersByTime(ms);
  });
}

const PILOTO = {
  id: "D-01",
  nome: "Tim Weber",
  nome_completo: "Tim Weber",
  equipe_id: "T-01",
  equipe_nome: "Phoenix United",
  equipe_nome_curto: "PHU",
  equipe_cor: "#e07a3f",
  posicao_campeonato: 11,
  pontos: 0,
  is_jogador: false,
};

function montar(weekendModifiers) {
  return render(
    <ChampionshipTablePanel
      championshipTable={[PILOTO]}
      constructorsTable={[]}
      playerTeamId="T-09"
      breakdownRiskTeams={new Set()}
      weekendModifiers={weekendModifiers}
      hoveredDriverId={null}
    />,
  );
}

// O que o comando emite: os oito elos, sempre, na ordem em que a esteira os aplica.
const ORDEM = [
  "trackKnowledge",
  "categoryAdaptation",
  "injury",
  "trackAffinity",
  "form",
  "setup",
  "motivation",
  "pressure",
];

function esteira(porElo = {}, rain = null) {
  const modifiers = ORDEM.map((key) => ({
    key,
    race: porElo[key]?.[0] ?? 0,
    qualifying: porElo[key]?.[1] ?? 0,
  }));
  return {
    driver_id: "D-01",
    total_race: modifiers.reduce((soma, m) => soma + m.race, 0),
    total_qualifying: modifiers.reduce((soma, m) => soma + m.qualifying, 0),
    modifiers,
    rain: rain ?? { weather: "dry", rain_skill: 50, penalty: 0, vs_field: 0 },
  };
}

const ROTULOS = [
  "Conhecimento da pista",
  "Adaptação à categoria",
  "Lesão",
  "Afinidade com o traçado",
  "Forma do momento",
  "Acerto do fim de semana",
  "Motivação",
  "Pressão",
];

describe("ChampionshipTablePanel — modificadores do fim de semana", () => {
  it("abre o balão com os elos da esteira que estão pegando no piloto", () => {
    montar(
      new Map([
        [
          "D-01",
          esteira({
            form: [2.1, 2.1],
            trackAffinity: [-1.4, -2.1],
            pressure: [0.6, 0.6],
          }),
        ],
      ]),
    );

    fireEvent.mouseEnter(screen.getByText("Tim Weber").closest("tr"));
    avancar(500);

    const balao = screen.getByTestId("tooltip");
    expect(balao).toHaveTextContent("Forma do momento");
    expect(balao).toHaveTextContent("Afinidade com o traçado");
    expect(balao).toHaveTextContent("Pressão");
    // Sinal explícito nos dois canais — quali diverge da corrida na afinidade.
    expect(balao).toHaveTextContent("+2,1");
    expect(balao).toHaveTextContent("−1,4");
    expect(balao).toHaveTextContent("−2,1");
  });

  // Uma lista que muda de tamanho e de ordem a cada piloto obriga a reler tudo. A do balão é
  // sempre a mesma lista, na ordem em que a esteira aplica os elos.
  it("mostra os oito elos sempre, na ordem da esteira", () => {
    montar(new Map([["D-01", esteira({ injury: [-6.3, -6.3] })]]));

    fireEvent.mouseEnter(screen.getByText("Tim Weber").closest("tr"));
    avancar(500);

    const balao = screen.getByTestId("tooltip");
    const posicoes = ROTULOS.map((rotulo) => balao.textContent.indexOf(rotulo));
    expect(posicoes.every((indice) => indice >= 0)).toBe(true);
    expect([...posicoes].sort((a, b) => a - b)).toEqual(posicoes);
  });

  // Zero é informação: "não está lesionado" e "não existe linha de lesão" viram a mesma tela
  // quando se esconde o zero, e só a primeira é verdade.
  it("apaga o elo que não está pegando, sem tirá-lo da lista", () => {
    montar(new Map([["D-01", esteira({ injury: [-6.3, -6.3] })]]));

    fireEvent.mouseEnter(screen.getByText("Tim Weber").closest("tr"));
    avancar(500);

    const balao = screen.getByTestId("tooltip");
    const linhaDaLesao = balao.querySelector('[data-inativo="true"]');
    expect(linhaDaLesao).not.toBeNull();
    expect(linhaDaLesao).not.toHaveTextContent("Lesão");
    // Sete apagados (os zerados) e a lesão acesa.
    expect(balao.querySelectorAll('[data-inativo="true"]')).toHaveLength(7);
  });

  // A chuva fica fora do total de cima (outra unidade) e o número que decide é o RELATIVO:
  // na chuva o grid inteiro cai, então perder 4,2 só quer dizer alguma coisa comparado ao
  // que o pelotão perde.
  it("mostra a chuva em bloco próprio, com o delta contra o pelotão", () => {
    montar(
      new Map([
        [
          "D-01",
          esteira({}, { weather: "wet", rain_skill: 82, penalty: 2.1, vs_field: 1.9 }),
        ],
      ]),
    );

    fireEvent.mouseEnter(screen.getByText("Tim Weber").closest("tr"));
    avancar(500);

    const balao = screen.getByTestId("tooltip");
    expect(balao).toHaveTextContent("Se molhar");
    expect(balao).toHaveTextContent("Chuva");
    expect(balao).toHaveTextContent("82");
    expect(balao).toHaveTextContent("−2,1");
    expect(balao).toHaveTextContent("+1,9");
    // A chuva não pode ter entrado no saldo dos oito elos, que aqui são todos zero.
    expect(balao).toHaveTextContent("Saldo do dia");
  });

  it("diz que a etapa é seca quando não há chuva prevista", () => {
    montar(new Map([["D-01", esteira()]]));

    fireEvent.mouseEnter(screen.getByText("Tim Weber").closest("tr"));
    avancar(500);

    expect(screen.getByTestId("tooltip")).toHaveTextContent("Etapa seca");
  });

  // Sem o comando respondido (save antigo, etapa inexistente, erro no backend) o balão
  // simplesmente não existe — nada de retângulo vazio seguindo o cursor.
  it("não abre balão nenhum sem dado da esteira", () => {
    montar(new Map());

    fireEvent.mouseEnter(screen.getByText("Tim Weber").closest("tr"));
    avancar(500);

    expect(screen.queryByTestId("tooltip")).toBeNull();
  });
});
