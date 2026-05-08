import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import TeamLogoMark, { getTeamLogoSrc } from "./TeamLogoMark";

describe("TeamLogoMark", () => {
  it("resolves normalized LMP2 team logos", () => {
    const lmp2Teams = [
      "United Autosports",
      "Jota Sport",
      "Belgian Racing Team",
      "Prema Powerteam",
      "Cool Racing",
    ];

    lmp2Teams.forEach((teamName) => {
      expect(getTeamLogoSrc(teamName)).toEqual(expect.stringContaining("TimesNormalized/lmp2"));
    });
  });

  it("renders the LMP2 logo image instead of the fallback color mark", () => {
    render(<TeamLogoMark teamName="Belgian Racing Team" color="#0b3d91" />);

    const logo = screen.getByRole("img", { name: "Belgian Racing Team logo" });
    expect(logo).toHaveAttribute("src", expect.stringContaining("belgian%20racing%20team.png"));
    expect(logo).toHaveClass("object-contain");
  });
});
