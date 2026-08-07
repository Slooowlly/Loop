import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import FlagIcon from "./FlagIcon";

describe("FlagIcon", () => {
  it("renders the Argentine flag asset for stored AR nationality labels", () => {
    render(<FlagIcon nacionalidade="AR Argentino" />);

    const flag = screen.getByRole("img", { name: "AR Argentino" });
    expect(flag).toHaveAttribute("src", expect.stringContaining("ar.png"));
  });

  // As artes vão de 1:1 (Suíça) a 2:1 (Austrália): numa caixa fixa, `cover`
  // cortava a bandeira em vez de encaixá-la.
  it("encaixa a bandeira na caixa em vez de recortá-la", () => {
    render(<FlagIcon nacionalidade="AU Australiano" />);

    const flag = screen.getByRole("img", { name: "AU Australiano" });
    expect(flag.className).toContain("object-contain");
    expect(flag.className).not.toContain("object-cover");
    expect(flag.className).toContain("h-4 w-6");
  });

  it("deixa o tamanho com o chamador na variante natural", () => {
    render(<FlagIcon variant="natural" nacionalidade="AU Australiano" className="h-7" />);

    const flag = screen.getByRole("img", { name: "AU Australiano" });
    // Sem caixa fixa e sem object-fit: com só a altura, a largura sai da
    // proporção real do arquivo.
    expect(flag.className).not.toContain("w-6");
    expect(flag.className).not.toContain("object-");
    expect(flag.className).toContain("h-7");
  });
});
