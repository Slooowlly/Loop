import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, "..", "..");

const LOW_AND_MID_CATEGORIES = [
  "mazda_rookie",
  "toyota_rookie",
  "mazda_amador",
  "toyota_amador",
  "bmw_m2",
  "production_challenger",
];

const TOP_TIER_CATEGORIES = ["gt4", "gt3", "lmp2", "endurance"];

function getStringField(block, field) {
  const match = block.match(new RegExp(`${field}:\\s*(?:Some\\()?\"([^\"]*)\"`));
  return match?.[1] ?? null;
}

async function loadTeamTemplates() {
  const source = await readFile(path.join(projectRoot, "src-tauri/src/constants/teams.rs"), "utf8");
  return [...source.matchAll(/TeamTemplate\s*\{([\s\S]*?)\n\s*\}/g)]
    .map((match) => match[1])
    .map((block) => ({
      name: getStringField(block, "nome"),
      shortName: getStringField(block, "nome_curto"),
      category: getStringField(block, "categoria"),
      primaryColor: getStringField(block, "cor_primaria"),
      secondaryColor: getStringField(block, "cor_secundaria"),
    }))
    .filter((team) => team.name);
}

function familyForColor(color) {
  const [r, g, b] = color
    .slice(1)
    .match(/.{2}/g)
    .map((part) => parseInt(part, 16) / 255);
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const lightness = (max + min) / 2;
  const delta = max - min;
  let hue = 0;
  let saturation = 0;

  if (delta !== 0) {
    saturation = lightness > 0.5 ? delta / (2 - max - min) : delta / (max + min);
    if (max === r) hue = ((g - b) / delta + (g < b ? 6 : 0)) * 60;
    if (max === g) hue = ((b - r) / delta + 2) * 60;
    if (max === b) hue = ((r - g) / delta + 4) * 60;
  }

  if (lightness > 0.82 && saturation < 0.25) return "white";
  if (lightness < 0.18 && saturation < 0.25) return "black";
  if (saturation < 0.18) return "gray";
  if (hue < 18 || hue >= 345) return "red";
  if (hue >= 18 && hue < 45 && lightness < 0.48) return "brown";
  if (hue < 38) return "orange";
  if (hue < 70) return "yellow_gold";
  if (hue < 165) return "green_teal";
  if (hue < 255) return "blue_cyan";
  if (hue < 315) return "purple";
  return "pink";
}

test("team templates use one color per team and stay diverse by category tier", async () => {
  const teams = await loadTeamTemplates();

  for (const team of teams) {
    assert.match(team.primaryColor, /^#[0-9a-f]{6}$/i, `${team.shortName} needs a valid primary color`);
    assert.equal(
      team.secondaryColor,
      team.primaryColor,
      `${team.shortName} should mirror secondary color because teams only have one color`,
    );
  }

  for (const category of [...LOW_AND_MID_CATEGORIES, ...TOP_TIER_CATEGORIES]) {
    const categoryTeams = teams.filter((team) => team.category === category);
    if (categoryTeams.length === 0) continue;
    const uniqueColors = new Set(categoryTeams.map((team) => team.primaryColor.toLowerCase()));
    assert.equal(uniqueColors.size, categoryTeams.length, `${category} should not repeat exact team colors`);
  }

  for (const category of LOW_AND_MID_CATEGORIES) {
    const categoryTeams = teams.filter((team) => team.category === category);
    if (categoryTeams.length === 0) continue;
    const families = new Set(categoryTeams.map((team) => familyForColor(team.primaryColor)));
    const expectedMinimum = categoryTeams.length <= 6 ? 5 : 7;
    assert.ok(
      families.size >= expectedMinimum,
      `${category} should use at least ${expectedMinimum} color families; got ${[...families].join(", ")}`,
    );
  }

  const overallFamilies = new Set(teams.map((team) => familyForColor(team.primaryColor)));
  for (const requiredFamily of ["orange", "yellow_gold", "white", "brown"]) {
    assert.ok(overallFamilies.has(requiredFamily), `palette should include ${requiredFamily}`);
  }
});

test("team identity UI renders a single team color", async () => {
  const teamCardSource = await readFile(path.join(projectRoot, "src/components/wizard/TeamCard.jsx"), "utf8");
  const driverDetailSource = await readFile(
    path.join(projectRoot, "src/components/driver/DriverDetailModal.jsx"),
    "utf8",
  );

  assert.doesNotMatch(
    teamCardSource,
    /secondaryColor/,
    "wizard team cards should not render a second team color",
  );
  assert.doesNotMatch(
    driverDetailSource,
    /equipe_cor_secundaria/,
    "driver detail should not render secondary team color gradients",
  );
});

test("team names avoid car-code siglas and raw model names", async () => {
  const source = await readFile(path.join(projectRoot, "src-tauri/src/constants/teams.rs"), "utf8");
  const forbidden = /\b(TRD|GR86|GR|MX5|M2|SPS|SRO|WRT|GT4|GT3|R8G|TR3|R8|M4|Z06|296|720S)\b/i;
  const allowedManufacturers = /\b(Ferrari|BMW|Audi|Chevrolet|McLaren|Mercedes|Porsche|Ford|Lamborghini|Aston Martin|Acura|Toyota|Mazda)\b/;

  for (const block of source.matchAll(/TeamTemplate\s*\{([\s\S]*?)\n\s*\}/g)) {
    const name = getStringField(block[1], "nome");
    const shortName = getStringField(block[1], "nome_curto");
    if (!name || !shortName) continue;

    assert.doesNotMatch(name, forbidden, `${name} should not use forbidden car-code naming`);
    assert.doesNotMatch(shortName, forbidden, `${shortName} should not use forbidden car-code naming`);

    if (allowedManufacturers.test(name)) {
      assert.doesNotMatch(
        name,
        /\b(Cup|Challenge|Masters|Series|Club Racing|Racer Cup)\b/i,
        `${name} should use manufacturer identity as a team name, not a generic car-series label`,
      );
    }
  }
});
