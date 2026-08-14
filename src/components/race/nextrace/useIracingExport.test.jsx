import { vi, describe, it, expect, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";

// O caminho que leva a etapa do Loop para dentro do iRacing: escreve o roster e a temporada,
// instala a macro de bandeira, põe o sim em modo janela e pinta o carro do jogador. A vistoria
// de 10/08/2026 apontou este arquivo como reescrito e sem nenhum teste.
//
// Duas coisas aqui só falham na mão do jogador, e as duas são de tempo:
//
//   • a ORDEM. Roster antes de temporada não é preferência: a temporada exportada já apagou
//     uma vez o que o roster tinha acabado de escrever. Macro e modo janela são não-fatais de
//     propósito — o sim está fechado neste instante, que é a única janela em que a escrita
//     nesses arquivos sobrevive, e falhar ali não pode derrubar a exportação.
//   • os TIMERS. O aviso da pintura tem timer PRÓPRIO, fora do `dismissToasts`; limpo junto,
//     ele nasceria e morreria no mesmo instante. E nenhum dos dois pode sobreviver à saída da
//     aba, que é remontada a cada visita à Sala.

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("../../../utils/sfx", () => ({ exportSuccess: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { exportSuccess } from "../../../utils/sfx";

import { useIracingExport } from "./useIracingExport";
import { IRACING_TUTORIAL_KEY } from "./nextRaceHelpers";

const CARREIRA = "save-1";
const PADRAO = {
  careerId: CARREIRA,
  player: { nome: "Ana Prado" },
  // Id de categoria do catálogo, que é o que o backend espera receber. Aqui havia "mx5",
  // uma chave de CARRO — resquício da época em que o frontend adivinhava o carro.
  playerTeam: { categoria: "mazda_amador" },
};

/// Os nomes dos comandos disparados, na ordem.
const comandos = () => invoke.mock.calls.map(([nome]) => nome);

/// Monta o hook com um `setError` espião, para cobrar o erro que a tela mostraria.
function montar(props = {}) {
  const setError = vi.fn();
  const utils = renderHook((p) => useIracingExport({ ...PADRAO, ...p, setError }), {
    initialProps: props,
  });
  return { ...utils, setError };
}

/// Roda a exportação inteira e deixa as promessas assentarem.
async function exportar(result) {
  await act(async () => {
    await result.current.handleExport();
  });
}

describe("useIracingExport", () => {
  beforeEach(() => {
    localStorage.clear();
    invoke.mockReset();
    exportSuccess.mockClear();
    // Padrão otimista: tudo responde. A pintura devolve `null` (desligada nas Configurações),
    // que é o caso silencioso — cada teste que quer a cor sobrescreve.
    invoke.mockResolvedValue(null);
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  describe("exportação", () => {
    it("escreve o roster ANTES da temporada", async () => {
      const { result } = montar();
      await exportar(result);
      const nomes = comandos();
      expect(nomes.indexOf("iracing_generate_roster")).toBeGreaterThanOrEqual(0);
      expect(nomes.indexOf("iracing_generate_roster")).toBeLessThan(
        nomes.indexOf("iracing_generate_season"),
      );
    });

    it("manda a categoria e o nome do roster, e NENHUMA chave de carro", async () => {
      const { result } = montar();
      await exportar(result);
      const args = invoke.mock.calls.find(([n]) => n === "iracing_generate_roster")[1];
      // Os campos cruzam a ponte POR NOME: um renomeado aqui vira `undefined` do lado do Rust
      // sem erro nenhum, e a etapa sai com o grid errado.
      //
      // A ausência de `carKey` é o contrato desta mudança, e por isso o `toEqual` (exato) em
      // vez de `objectContaining`: quem decide o carro é o Rust, pela identidade da
      // categoria. Uma `carKey` de volta aqui reabriria o caminho do MX-5 por omissão.
      expect(args).toEqual({
        careerId: CARREIRA,
        categoria: "mazda_amador",
        rosterName: "Carreira Ana Prado",
      });
      const season = invoke.mock.calls.find(([n]) => n === "iracing_generate_season")[1];
      expect(season).toEqual(args);
    });

    it("a pintura recebe a categoria, não uma chave de carro", async () => {
      // A pintura tinha uma CÓPIA da regra de substring do lado do Rust. Mandando a
      // categoria, ela passa a usar a mesma tabela do roster e da temporada.
      const { result } = montar();
      await exportar(result);
      const args = invoke.mock.calls.find(([n]) => n === "iracing_auto_paint_player")[1];
      expect(args).toEqual({ careerId: CARREIRA, categoria: "mazda_amador" });
    });

    it("a recusa do backend aparece na tela e não marca como exportado", async () => {
      // Categoria que o export não sabe fazer (GT3, Endurance, ...) volta como erro com o
      // motivo. Antes ela seguia calada e a etapa saía num MX-5.
      const RECUSA = "Esta categoria ainda não tem carro definido para o iRacing.";
      invoke.mockImplementation((nome) =>
        nome === "iracing_generate_roster" ? Promise.reject(RECUSA) : Promise.resolve(null),
      );
      const { result, setError } = montar({ playerTeam: { categoria: "gt3" } });
      await exportar(result);
      expect(setError).toHaveBeenCalledWith(RECUSA);
      expect(result.current.exported).toBe(false);
      expect(comandos()).not.toContain("iracing_generate_season");
    });

    it("instala a macro e o modo janela na mesma ida", async () => {
      // Os dois só "colam" com o simulador fechado, e este é o momento em que ele está.
      const { result } = montar();
      await exportar(result);
      expect(comandos()).toContain("iracing_install_yellow_macro");
      expect(comandos()).toContain("iracing_modo_janela_aplicar");
    });

    it("segue exportando quando a macro e o modo janela falham", async () => {
      // Falha esperada: o simulador aberto faz o modo janela errar de propósito, em vez de
      // mentir sucesso. Derrubar a exportação por isso trocaria um ajuste acessório pela etapa.
      invoke.mockImplementation((nome) =>
        nome === "iracing_install_yellow_macro" || nome === "iracing_modo_janela_aplicar"
          ? Promise.reject(new Error("simulador aberto"))
          : Promise.resolve(null),
      );
      const { result, setError } = montar();
      await exportar(result);
      expect(result.current.exported).toBe(true);
      expect(setError).not.toHaveBeenCalledWith(expect.stringContaining("iRacing"));
    });

    it("diz na tela QUAIS ajustes ficaram de fora quando os dois falham", async () => {
      // A outra metade do caso acima, e o achado V6.2 da vistoria de 11/08/2026: seguir
      // exportando estava certo, engolir CALADO não. Os dois são pré-requisitos de features
      // que o jogador só usa depois — a bandeira amarela automática e o overlay —, então uma
      // falha muda vira "não funciona" semanas depois, sem nada para conferir.
      //
      // O aviso vai na linha discreta, e não em toast nem em `setError`: a exportação DEU
      // CERTO, e o que precisa da atenção agora é o convite de entrar no simulador.
      invoke.mockImplementation((nome) =>
        nome === "iracing_install_yellow_macro" || nome === "iracing_modo_janela_aplicar"
          ? Promise.reject(new Error("simulador aberto"))
          : Promise.resolve(null),
      );
      const { result } = montar();
      await exportar(result);
      expect(result.current.exportNotice).toContain("Macro de bandeira amarela");
      expect(result.current.exportNotice).toContain("Modo janela do simulador");
    });

    it("nomeia só o ajuste que falhou de verdade", async () => {
      // Listar os dois quando só um falhou mandaria o jogador caçar um problema que não
      // existe — e é o tipo de aviso que ensina a ignorar o aviso.
      invoke.mockImplementation((nome) =>
        nome === "iracing_modo_janela_aplicar"
          ? Promise.reject(new Error("simulador aberto"))
          : Promise.resolve(null),
      );
      const { result } = montar();
      await exportar(result);
      expect(result.current.exportNotice).toContain("Modo janela do simulador");
      expect(result.current.exportNotice).not.toContain("Macro de bandeira amarela");
    });

    it("cala quando os dois ajustes entram", async () => {
      const { result } = montar();
      await exportar(result);
      expect(result.current.exportNotice).toBe("");
    });

    it("deixa rastro no log de diagnóstico para cada ajuste que falhou", async () => {
      // O aviso na tela some no próximo simular. O log é o que sobra para quando o relato
      // chegar dias depois, e é o canal que o jogador já sabe anexar.
      invoke.mockImplementation((nome) =>
        nome === "iracing_install_yellow_macro" || nome === "iracing_modo_janela_aplicar"
          ? Promise.reject(new Error("simulador aberto"))
          : Promise.resolve(null),
      );
      const { result } = montar();
      await exportar(result);
      const rotulos = invoke.mock.calls
        .filter(([nome]) => nome === "diagnostico_registrar")
        .map(([, args]) => args.rotulo);
      expect(rotulos).toContain("iracing_install_yellow_macro");
      expect(rotulos).toContain("iracing_modo_janela_aplicar");
    });

    it("não exporta sem carreira ou sem categoria", async () => {
      const { result, setError } = montar();
      await act(async () => {
        result.current.setExportNotice(""); // só para provar que o hook está vivo
      });
      const semTime = renderHook(() =>
        useIracingExport({ ...PADRAO, playerTeam: null, setError }),
      );
      await act(async () => {
        await semTime.result.current.handleExport();
      });
      expect(setError).toHaveBeenCalledWith("Sem carreira ou categoria para exportar.");
      expect(comandos()).not.toContain("iracing_generate_roster");
    });

    it("mostra o erro do backend e não marca como exportado", async () => {
      invoke.mockImplementation((nome) =>
        nome === "iracing_generate_roster"
          ? Promise.reject("Pasta do iRacing não encontrada")
          : Promise.resolve(null),
      );
      const { result, setError } = montar();
      await exportar(result);
      expect(setError).toHaveBeenCalledWith("Pasta do iRacing não encontrada");
      expect(result.current.exported).toBe(false);
      expect(result.current.isExporting).toBe(false); // o botão precisa voltar
    });

    it("toca o som de sucesso uma vez", async () => {
      const { result } = montar();
      await exportar(result);
      expect(exportSuccess).toHaveBeenCalledTimes(1);
    });
  });

  describe("toasts da ida ao iRacing", () => {
    it("o convite de entrar no iRacing surge logo depois do 'exportado'", async () => {
      const { result } = montar();
      await exportar(result);
      expect(result.current.exported).toBe(true);
      expect(result.current.showGoToast).toBe(false);
      act(() => {
        vi.advanceTimersByTime(600);
      });
      expect(result.current.showGoToast).toBe(true);
    });

    it("os dois somem sozinhos em 15 s", async () => {
      // O jogador está indo para o simulador: um toast preso na tela fica preso para sempre,
      // porque ninguém volta a esta aba antes da corrida.
      const { result } = montar();
      await exportar(result);
      act(() => {
        vi.advanceTimersByTime(15000);
      });
      expect(result.current.exported).toBe(false);
      expect(result.current.showGoToast).toBe(false);
    });

    it("uma segunda exportação não deixa o timer da primeira apagar o novo toast", async () => {
      // `dismissToasts` limpa os timers antes de reagendar. Sem isso, o "15 s" da primeira
      // exportação mataria o toast da segunda no meio.
      const { result } = montar();
      await exportar(result);
      act(() => {
        vi.advanceTimersByTime(14000);
      });
      await exportar(result);
      act(() => {
        vi.advanceTimersByTime(2000); // passaria dos 15 s da PRIMEIRA
      });
      expect(result.current.exported).toBe(true);
    });
  });

  describe("aviso da pintura", () => {
    const PINTOU = { path: "car_1.tga", custid: 1, color: "#ff0000" };

    beforeEach(() => {
      invoke.mockImplementation((nome) =>
        nome === "iracing_auto_paint_player" ? Promise.resolve(PINTOU) : Promise.resolve(null),
      );
    });

    it("avisa a cor na primeira exportação do save", async () => {
      const { result } = montar();
      await exportar(result);
      expect(result.current.paintToast).toContain("#ff0000");
    });

    it("o aviso NÃO é apagado junto com os toasts da exportação", async () => {
      // A razão de o timer da pintura viver fora do `dismissTimers`: ele é agendado depois do
      // `dismissToasts` e limpo junto nasceria e morreria no mesmo instante. Ir para o iRacing
      // dispensa os toasts da ida — e é justo o instante em que o aviso da cor precisa ficar,
      // porque é o único que o jogador ainda não leu.
      localStorage.setItem(IRACING_TUTORIAL_KEY, "1");
      invoke.mockImplementation((nome) => {
        if (nome === "iracing_auto_paint_player") return Promise.resolve(PINTOU);
        if (nome === "iracing_focus_window") return Promise.resolve(true);
        return Promise.resolve(null);
      });
      const { result } = montar();
      await exportar(result);
      await act(async () => {
        await result.current.handleGoToIracing(); // chama dismissToasts
      });
      expect(result.current.exported).toBe(false);
      expect(result.current.showGoToast).toBe(false);
      expect(result.current.paintToast).toContain("#ff0000");
    });

    it("o aviso some sozinho em 8 s", async () => {
      const { result } = montar();
      await exportar(result);
      act(() => {
        vi.advanceTimersByTime(8000);
      });
      expect(result.current.paintToast).toBe("");
    });

    it("avisa uma vez só por save", async () => {
      const { result } = montar();
      await exportar(result);
      act(() => {
        vi.advanceTimersByTime(8000);
      });
      await exportar(result);
      expect(result.current.paintToast).toBe("");
    });

    it("um save diferente é avisado de novo", async () => {
      // A chave do aviso é por carreira: quem começa uma carreira nova ganha uma cor nova, e
      // precisa saber de onde ela veio.
      const { result } = montar();
      await exportar(result);
      const outro = renderHook(() =>
        useIracingExport({ ...PADRAO, careerId: "save-2", setError: vi.fn() }),
      );
      await act(async () => {
        await outro.result.current.handleExport();
      });
      expect(outro.result.current.paintToast).toContain("#ff0000");
    });

    it("cala quando a pintura está desligada nas Configurações", async () => {
      // O backend devolve `null` com o interruptor desligado ou sem custid capturado. Nos dois
      // casos não há nada a dizer, e um toast ali seria um aviso sobre coisa nenhuma.
      invoke.mockResolvedValue(null);
      const { result } = montar();
      await exportar(result);
      expect(result.current.paintToast).toBe("");
    });

    it("falha na pintura não derruba a exportação", async () => {
      invoke.mockImplementation((nome) =>
        nome === "iracing_auto_paint_player"
          ? Promise.reject(new Error("sem pasta de pintura"))
          : Promise.resolve(null),
      );
      const { result, setError } = montar();
      await exportar(result);
      expect(result.current.exported).toBe(true);
      expect(setError).not.toHaveBeenCalledWith(expect.stringContaining("pintura"));
    });
  });

  describe("limpeza dos timers", () => {
    it("a desmontagem não deixa nenhum timer pendente", async () => {
      // O contrário disto é um `setState` disparado num hook que já não existe, depois de o
      // jogador sair da Sala — e a aba é montada de novo a cada visita.
      invoke.mockImplementation((nome) =>
        nome === "iracing_auto_paint_player"
          ? Promise.resolve({ path: "p", custid: 1, color: "#0f0" })
          : Promise.resolve(null),
      );
      const { result, unmount } = montar();
      await exportar(result);
      act(() => {
        // O `localStorage.setItem` do aviso da pintura faz o jsdom agendar um `setTimeout(0)`
        // para o evento `storage`. Ele não é nosso e some sozinho; drená-lo aqui deixa a
        // contagem falando só dos timers do hook.
        vi.advanceTimersByTime(0);
      });
      expect(vi.getTimerCount()).toBe(3); // os dois da ida + o da pintura
      unmount();
      expect(vi.getTimerCount()).toBe(0);
    });
  });

  describe("ida ao iRacing", () => {
    it("na primeira vez abre o tutorial em vez de ir direto", async () => {
      const { result } = montar();
      await act(async () => {
        await result.current.handleGoToIracing();
      });
      expect(result.current.showTutorial).toBe(true);
      expect(comandos()).not.toContain("iracing_focus_window");
    });

    it("com o tutorial já visto, foca a janela do simulador", async () => {
      localStorage.setItem(IRACING_TUTORIAL_KEY, "1");
      invoke.mockImplementation((nome) =>
        nome === "iracing_focus_window" ? Promise.resolve(true) : Promise.resolve(null),
      );
      const { result } = montar();
      await exportar(result);
      await act(async () => {
        await result.current.handleGoToIracing();
      });
      expect(comandos()).toContain("iracing_focus_window");
      expect(result.current.exported).toBe(false); // focou → os toasts saem da frente
    });

    it("sem janela aberta, tenta abrir o iRacingUI", async () => {
      localStorage.setItem(IRACING_TUTORIAL_KEY, "1");
      invoke.mockImplementation((nome) => {
        if (nome === "iracing_focus_window") return Promise.resolve(false);
        if (nome === "iracing_launch_ui") return Promise.resolve(true);
        return Promise.resolve(null);
      });
      const { result } = montar();
      await act(async () => {
        await result.current.handleGoToIracing();
      });
      expect(comandos()).toContain("iracing_launch_ui");
      expect(result.current.iracingFocusMsg).toBe("");
    });

    it("sem simulador nem launcher, explica na tela", async () => {
      localStorage.setItem(IRACING_TUTORIAL_KEY, "1");
      invoke.mockResolvedValue(false);
      const { result } = montar();
      await act(async () => {
        await result.current.handleGoToIracing();
      });
      expect(result.current.iracingFocusMsg).toContain("iRacing");
    });

    it("fechar o tutorial marca como visto — ele é só na primeira vez", async () => {
      const { result } = montar();
      act(() => {
        result.current.handleTutorialClose();
      });
      expect(localStorage.getItem(IRACING_TUTORIAL_KEY)).toBe("1");
      expect(result.current.showTutorial).toBe(false);
    });

    it("concluir o tutorial leva ao simulador e fecha tudo", async () => {
      invoke.mockImplementation((nome) =>
        nome === "iracing_focus_window" ? Promise.resolve(true) : Promise.resolve(null),
      );
      const { result } = montar();
      await exportar(result);
      await act(async () => {
        await result.current.handleTutorialDone();
      });
      expect(localStorage.getItem(IRACING_TUTORIAL_KEY)).toBe("1");
      expect(result.current.showTutorial).toBe(false);
      expect(result.current.exported).toBe(false);
    });

    it("concluir o tutorial com o iRacing fechado mantém o tutorial aberto e explica", async () => {
      invoke.mockResolvedValue(false);
      const { result } = montar();
      await act(async () => {
        await result.current.handleTutorialDone();
      });
      expect(result.current.showTutorial).toBe(false); // nunca foi aberto neste caso
      expect(result.current.tutorialMsg).toContain("iRacing");
    });
  });
});
