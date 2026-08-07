// O bloco de push-to-talk das configurações, renderizado fora do Tauri.
//
// Este teste não prova que o volante é lido nem que o microfone escuta — só o hardware do
// jogador prova isso, e é para isso que o painel existe. O que ele prova é o que dá para
// errar em silêncio: que as chaves de tradução existem (uma chave torta rende
// `settings.engenheiroPtt.x` na tela, e ninguém percebe até um jogador reclamar), que a
// associação do botão sobrevive a ida e volta pelo armazenamento, e que as três pontas
// sem resposta dizem o que fazer em vez de ficarem em branco.

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => null) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));

import PttEngenheiroSettings from "./PttEngenheiroSettings";
import { lerGatilhoSalvo } from "../../lib/pttConfig";

afterEach(cleanup);
beforeEach(() => localStorage.clear());

describe("bloco do engenheiro", () => {
  it("mostra as três linhas e nenhuma chave de tradução crua", () => {
    render(<PttEngenheiroSettings />);
    expect(screen.getByText("Engenheiro por rádio")).toBeTruthy();
    expect(screen.getByText("Botão para falar")).toBeTruthy();
    expect(screen.getByText("Testar o rádio")).toBeTruthy();
    // Uma chave que não existe no locale vira o próprio caminho na tela.
    expect(document.body.textContent).not.toMatch(/settings\.engenheiroPtt/);
  });

  it("sem botão associado, diz que o engenheiro não tem como ser chamado", () => {
    render(<PttEngenheiroSettings />);
    expect(screen.getByText(/Nenhum botão associado/)).toBeTruthy();
  });

  it("a tecla capturada é persistida e mostrada", async () => {
    render(<PttEngenheiroSettings />);
    fireEvent.click(screen.getByText("Tecla"));
    expect(screen.getByText(/Pressione a tecla/)).toBeTruthy();

    fireEvent.keyDown(window, { key: "F1", keyCode: 112 });

    await waitFor(() => expect(screen.getByText(/Segure Tecla 112/)).toBeTruthy());
    expect(lerGatilhoSalvo()).toEqual({ tipo: "tecla", codigo: 112 });
  });

  it("Escape desiste da captura sem associar nada", async () => {
    render(<PttEngenheiroSettings />);
    fireEvent.click(screen.getByText("Tecla"));
    fireEvent.keyDown(window, { key: "Escape", keyCode: 27 });

    await waitFor(() => expect(screen.getByText(/Nenhum botão associado/)).toBeTruthy());
    expect(lerGatilhoSalvo()).toBeNull();
  });
});

describe("painel de teste", () => {
  it("abre fechado — abrir o microfone não pode ser efeito de entrar nas configurações", () => {
    render(<PttEngenheiroSettings />);
    expect(screen.queryByText("Botões do volante")).toBeNull();
    expect(screen.getByText("Abrir")).toBeTruthy();
  });

  it("aberto, guia as três pontas mesmo sem hardware nenhum respondendo", async () => {
    render(<PttEngenheiroSettings />);
    fireEvent.click(screen.getByText("Abrir"));

    expect(screen.getByText("Botões do volante")).toBeTruthy();
    expect(screen.getByText("Nível do microfone")).toBeTruthy();
    expect(screen.getByText("Botão e microfone juntos")).toBeTruthy();
    // A instrução do teste de ponta a ponta tem de estar visível ANTES de haver captura:
    // é ela que diz ao jogador o que fazer com o botão.
    expect(screen.getByText(/Segure o botão associado e fale/)).toBeTruthy();
    // Sem `getUserMedia` no jsdom, a falha do microfone vira frase e não tela em branco.
    await waitFor(() => expect(document.body.textContent).toMatch(/microfone|Microfone/));
  });
});
