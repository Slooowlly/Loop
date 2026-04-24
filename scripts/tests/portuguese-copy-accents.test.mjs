import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, "..", "..");

const FILES_AND_FORBIDDEN_COPY = [
  {
    file: "src/utils/formatters.js",
    forbidden: ["Proxima corrida", "amanha", "1 mes", "Sem licenÃ§a", "ConvocaÃ§Ã£o", "ConsistÃªncia"],
  },
  {
    file: "src/pages/NewCareer.jsx",
    forbidden: ["voce vai estrear", "equipe valida", "Nao foi possivel", "Voce entra", "Esta acao criara"],
  },
  {
    file: "src/pages/tabs/StandingsTab.jsx",
    forbidden: ["Nao foi possivel carregar a classificacao", "Carregando classificacao", "Classificacao de pilotos", "Competicao especial ainda nao aconteceu", "A classificacao de equipes sera"],
  },
  {
    file: "src/pages/tabs/NextRaceTab.jsx",
    forbidden: ["Nao foi possivel", "variacao recente", "Avancar para pre-temporada", "Voce nao tem equipe", "Sem historico", "maximo de pontos possivel"],
  },
  {
    file: "src/pages/tabs/NewsTab.jsx",
    forbidden: ["Nao foi possivel", "Sem leitura disponivel"],
  },
  {
    file: "src/components/season/EndOfSeasonView.jsx",
    forbidden: ["Nao alterou", "Licenca", "Perdeu a licenca", "proxima temporada", "promocoes"],
  },
  {
    file: "src/components/layout/WindowControlsDrawer.jsx",
    forbidden: ["Voce pode salvar", "flush nao impede a saida"],
  },
  {
    file: "src/components/layout/Header.jsx",
    forbidden: ["Proximo Evento", "Horario", "Avancar calendario", "Avancar para convocacao", "Campeao definido", "Umido"],
  },
  {
    file: "src/components/layout/TabNavigation.jsx",
    forbidden: ["Noticias", "Calendario"],
  },
  {
    file: "src/components/driver/DriverDetailModalSections.jsx",
    forbidden: ["Vitorias", "Podios", "Nao identificada", "Campeao", "Sem titulo"],
  },
  {
    file: "src/components/season/ConvocationView.jsx",
    forbidden: ["Avancar dia", "Processando convocacao", "Convocacao aceita", "Mercado de Convocacoes", "Convocacao em destaque", "JanelaConvocacao", "Pilotos elegiveis"],
  },
];

test("main UI copy keeps Portuguese accents in user-facing text", async () => {
  const hits = [];

  for (const entry of FILES_AND_FORBIDDEN_COPY) {
    const source = await readFile(path.join(projectRoot, entry.file), "utf8");
    for (const fragment of entry.forbidden) {
      if (source.includes(fragment)) {
        hits.push(`${entry.file}: ${fragment}`);
      }
    }
  }

  assert.deepStrictEqual(
    hits,
    [],
    `expected accented Portuguese copy in UI sources:\n${hits.join("\n")}`,
  );
});
