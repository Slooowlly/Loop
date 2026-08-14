import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

import IracingDesfazerPanel from "./IracingDesfazerPanel";

// O painel é o caminho de volta dos dois arquivos que o Loop escreve na pasta do iRacing
// sem perguntar. O que os testes protegem é a HONESTIDADE dele: botão desabilitado quando
// não há o que desfazer, botão desabilitado com o simulador aberto (a escrita se perderia),
// e um veredito por carro que nunca esconde o caso "não toquei nesse arquivo".

let mockState = { careerId: "C1" };

vi.mock("../../stores/useCareerStore", () => ({
  default: (selector) => selector(mockState),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

function statusJanela(extra = {}) {
  return {
    encontrado: true,
    em_janela: true,
    simulador_aberto: false,
    pode_desfazer: true,
    arquivos: [],
    ...extra,
  };
}

/// Backend de leitura. Cada caso declara o que os comandos de ESCRITA respondem; o que não
// for declarado estoura, para nenhum teste disparar um desfazer sem dizer.
function configurarBackend({ janela = statusJanela(), respostas = {} } = {}) {
  mockState = { careerId: "C1" };
  invoke.mockReset();
  invoke.mockImplementation(async (comando, args) => {
    if (comando in respostas) {
      const r = respostas[comando];
      return typeof r === "function" ? r(args) : r;
    }
    if (comando === "iracing_modo_janela_status") {
      if (typeof janela === "function") return janela();
      return janela;
    }
    throw new Error(`Comando não esperado: ${comando}`);
  });
}

async function aguardarPainel() {
  await screen.findByText("Desfazer alterações no iRacing");
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("iracing_modo_janela_status"));
}

const botaoJanela = () => screen.getByRole("button", { name: "Desfazer modo janela" });
const botaoPintura = () => screen.getByRole("button", { name: "Desfazer pintura do carro" });

describe("IracingDesfazerPanel — modo janela", () => {
  it("sem backup, o botão fica desabilitado e a tela diz que não há o que desfazer", async () => {
    configurarBackend({ janela: statusJanela({ pode_desfazer: false }) });
    render(<IracingDesfazerPanel />);
    await aguardarPainel();

    await waitFor(() => expect(botaoJanela()).toBeDisabled());
    expect(screen.getByText(/não chegou a mudar a configuração gráfica/i)).toBeInTheDocument();
  });

  it("com o simulador aberto, o botão sai do ar e a tela explica por quê", async () => {
    configurarBackend({ janela: statusJanela({ simulador_aberto: true }) });
    render(<IracingDesfazerPanel />);
    await aguardarPainel();

    await waitFor(() => expect(botaoJanela()).toBeDisabled());
    expect(screen.getByText(/Feche o iRacing para desfazer/i)).toBeInTheDocument();
  });

  it("com backup e sim fechado, desfaz e passa a dizer que não há mais o que desfazer", async () => {
    configurarBackend({
      respostas: {
        iracing_modo_janela_restaurar: statusJanela({ pode_desfazer: false, em_janela: false }),
      },
    });
    render(<IracingDesfazerPanel />);
    await aguardarPainel();

    await waitFor(() => expect(botaoJanela()).toBeEnabled());
    fireEvent.click(botaoJanela());

    await waitFor(() => expect(invoke).toHaveBeenCalledWith("iracing_modo_janela_restaurar"));
    expect(await screen.findByText("Configuração gráfica devolvida ao estado anterior.")).toBeInTheDocument();
    // O status volta do próprio comando: sem relê-lo, o botão continuaria clicável mentindo
    // que ainda há backup.
    await waitFor(() => expect(botaoJanela()).toBeDisabled());
  });

  it("falha ao restaurar mostra o erro e relê o status do disco", async () => {
    configurarBackend({
      respostas: {
        iracing_modo_janela_restaurar: () => Promise.reject(new Error("iRacing abriu no meio")),
      },
    });
    render(<IracingDesfazerPanel />);
    await aguardarPainel();

    fireEvent.click(botaoJanela());

    expect(await screen.findByText(/iRacing abriu no meio/)).toBeInTheDocument();
    await waitFor(() => {
      const leituras = invoke.mock.calls.filter(([c]) => c === "iracing_modo_janela_status");
      expect(leituras.length).toBeGreaterThan(1);
    });
  });

  it("status ilegível não deixa o botão clicável", async () => {
    configurarBackend({ janela: () => Promise.reject(new Error("sem acesso")) });
    render(<IracingDesfazerPanel />);
    await screen.findByText("Desfazer alterações no iRacing");

    await waitFor(() => expect(botaoJanela()).toBeDisabled());
  });
});

describe("IracingDesfazerPanel — pintura", () => {
  it("mostra o resultado por carro, com o rótulo de cada desfecho", async () => {
    configurarBackend({
      respostas: {
        iracing_desfazer_pinturas: [
          { car_key: "mx5", caminho: "C:\\iRacing\\paint\\mx5\\car_1.tga", estado: "restaurada" },
          { car_key: "gr86", caminho: "C:\\iRacing\\paint\\gr86\\car_1.tga", estado: "removida" },
          { car_key: "bmwm2", caminho: "C:\\iRacing\\paint\\bmwm2\\car_1.tga", estado: "nada" },
        ],
      },
    });
    render(<IracingDesfazerPanel />);
    await aguardarPainel();

    fireEvent.click(botaoPintura());

    expect(await screen.findByText("Restaurada")).toBeInTheDocument();
    expect(screen.getByText("Removida")).toBeInTheDocument();
    expect(screen.getByText("Nada a desfazer")).toBeInTheDocument();
    expect(screen.getByText("mx5")).toBeInTheDocument();
    expect(screen.getByText("gr86")).toBeInTheDocument();
    expect(screen.getByText("bmwm2")).toBeInTheDocument();
  });

  it("preservada vem com a explicação de que o arquivo não foi tocado", async () => {
    configurarBackend({
      respostas: {
        iracing_desfazer_pinturas: [
          { car_key: "mx5", caminho: "C:\\iRacing\\paint\\mx5\\car_1.tga", estado: "preservada" },
        ],
      },
    });
    render(<IracingDesfazerPanel />);
    await aguardarPainel();

    fireEvent.click(botaoPintura());

    expect(await screen.findByText("Preservada")).toBeInTheDocument();
    // O ponto do caso: o jogador continua com a cor da equipe no carro e precisa entender
    // que isso é escolha do desfazer, não falha dele.
    expect(
      screen.getByText(/NÃO foi tocado.*não é uma pintura reconhecida do Loop/i),
    ).toBeInTheDocument();
  });

  it("sem nenhuma preservada, a nota explicativa não aparece", async () => {
    configurarBackend({
      respostas: {
        iracing_desfazer_pinturas: [
          { car_key: "mx5", caminho: "C:\\p\\car_1.tga", estado: "removida" },
        ],
      },
    });
    render(<IracingDesfazerPanel />);
    await aguardarPainel();

    fireEvent.click(botaoPintura());

    expect(await screen.findByText("Removida")).toBeInTheDocument();
    expect(screen.queryByText(/NÃO foi tocado/i)).not.toBeInTheDocument();
  });

  it("lista vazia diz que não achou pasta de pintura, em vez de não dizer nada", async () => {
    configurarBackend({ respostas: { iracing_desfazer_pinturas: [] } });
    render(<IracingDesfazerPanel />);
    await aguardarPainel();

    fireEvent.click(botaoPintura());

    expect(
      await screen.findByText("Nenhuma pasta de pintura do iRacing encontrada."),
    ).toBeInTheDocument();
  });

  it("manda a carreira aberta junto — é o save que guarda o ID usado para nomear os arquivos", async () => {
    configurarBackend({ respostas: { iracing_desfazer_pinturas: [] } });
    render(<IracingDesfazerPanel />);
    await aguardarPainel();

    fireEvent.click(botaoPintura());

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("iracing_desfazer_pinturas", { careerId: "C1" }),
    );
  });

  it("aberto pelo menu inicial, sem carreira nenhuma, ainda chama o comando", async () => {
    configurarBackend({ respostas: { iracing_desfazer_pinturas: [] } });
    mockState = { careerId: null };
    render(<IracingDesfazerPanel />);
    await aguardarPainel();

    fireEvent.click(botaoPintura());

    // Sem save o backend cai no custid capturado na sessão do iRacing, que é o mesmo com que
    // os arquivos foram nomeados. Mandar `null` quebraria a desserialização do lado Rust.
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("iracing_desfazer_pinturas", { careerId: "" }),
    );
  });

  it("falha no desfazer mostra a mensagem do backend", async () => {
    configurarBackend({
      respostas: {
        iracing_desfazer_pinturas: () =>
          Promise.reject(new Error("Ainda não sei o seu ID do iRacing")),
      },
    });
    render(<IracingDesfazerPanel />);
    await aguardarPainel();

    fireEvent.click(botaoPintura());

    expect(await screen.findByText(/Ainda não sei o seu ID do iRacing/)).toBeInTheDocument();
  });

  it("nada é escrito só por abrir a tela", async () => {
    configurarBackend();
    render(<IracingDesfazerPanel />);
    await aguardarPainel();

    const comandos = invoke.mock.calls.map(([c]) => c);
    expect(comandos).toEqual(["iracing_modo_janela_status"]);
  });
});
