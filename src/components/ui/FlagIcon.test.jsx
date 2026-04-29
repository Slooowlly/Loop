import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import FlagIcon from "./FlagIcon";

describe("FlagIcon", () => {
  it("renders the Argentine flag asset for stored AR nationality labels", () => {
    render(<FlagIcon nacionalidade="AR Argentino" />);

    const flag = screen.getByRole("img", { name: "AR Argentino" });
    expect(flag).toHaveAttribute("src", expect.stringContaining("ar.png"));
  });
});
