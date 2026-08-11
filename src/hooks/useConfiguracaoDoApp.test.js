import { vi, describe, it, expect, beforeEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";

import useConfiguracaoDoApp from "./useConfiguracaoDoApp";

const CONFIG_DO_DISCO = {
  version: "0.14.0",
  language: "pt-BR",
  autosave_enabled: true,
  auto_paint_car: true,
  telemetry_enabled: true,
  vr_overlay_mode: "auto",
  monitor_overlay_in_vr: false,
  spotter_takeover: true,
};

/// O que chegou ao backend na última gravação.
function ultimoSalvo() {
  const chamada = invoke.mock.calls.filter(([cmd]) => cmd === "update_config").at(-1);
  return chamada?.[1]?.newConfig ?? null;
}

describe("useConfiguracaoDoApp", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation((comando) => {
      if (comando === "get_config") return Promise.resolve({ ...CONFIG_DO_DISCO });
      return Promise.resolve(null);
    });
  });

  it("carrega o config na montagem", async () => {
    const { result } = renderHook(() => useConfiguracaoDoApp());

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.config).toEqual(CONFIG_DO_DISCO);
  });

  it("manda o objeto INTEIRO ao gravar, e não só o campo mudado", async () => {
    const { result } = renderHook(() => useConfiguracaoDoApp());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      result.current.handleToggle("autosave_enabled");
    });

    const salvo = ultimoSalvo();
    // O merge do lado Rust espera o objeto que o front leu; mandar um pedaço faria os
    // campos ausentes caírem no default do serde.
    expect(Object.keys(salvo).sort()).toEqual(Object.keys(CONFIG_DO_DISCO).sort());
    expect(salvo.autosave_enabled).toBe(false);
    expect(salvo.language).toBe("pt-BR");
  });

  it("handleChange troca o valor e mantém o resto", async () => {
    const { result } = renderHook(() => useConfiguracaoDoApp());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      result.current.handleChange("vr_overlay_mode", "off");
    });

    expect(ultimoSalvo().vr_overlay_mode).toBe("off");
    expect(result.current.config.vr_overlay_mode).toBe("off");
  });

  it("quando a gravação falha, recua para o que está no disco em vez de mentir", async () => {
    const { result } = renderHook(() => useConfiguracaoDoApp());
    await waitFor(() => expect(result.current.loading).toBe(false));

    invoke.mockImplementation((comando) => {
      if (comando === "get_config") return Promise.resolve({ ...CONFIG_DO_DISCO });
      if (comando === "update_config") return Promise.reject(new Error("disco cheio"));
      return Promise.resolve(null);
    });

    await act(async () => {
      result.current.handleToggle("auto_paint_car");
    });

    await waitFor(() => expect(result.current.errorMessage).toContain("disco cheio"));
    await waitFor(() => expect(result.current.config.auto_paint_car).toBe(true));
  });

  it("dois toggles seguidos não perdem o primeiro", async () => {
    const { result } = renderHook(() => useConfiguracaoDoApp());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      result.current.handleToggle("autosave_enabled");
      result.current.handleToggle("auto_paint_car");
    });

    const salvo = ultimoSalvo();
    expect(salvo.autosave_enabled).toBe(false);
    expect(salvo.auto_paint_car).toBe(false);
  });
});
