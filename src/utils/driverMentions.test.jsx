import { describe, it, expect } from "vitest";
import { buildDriverMentionMatcher, segmentDriverMentions } from "./driverMentions";

// Ajuda: quais ids o matcher extrai de um texto (na ordem em que aparecem).
function mentionsIn(text, drivers) {
  const matcher = buildDriverMentionMatcher(drivers);
  if (!matcher) return [];
  return text
    .split(matcher.regex)
    .map((part) => matcher.byName.get(part))
    .filter(Boolean);
}

// Ajuda: ids na ordem, agora via segmentação (inclui apelidos marcados pela IA).
function segmentIds(text, drivers) {
  return segmentDriverMentions(text, drivers)
    .filter((s) => s.type === "driver")
    .map((s) => s.id);
}

// Ajuda: o texto visível reconstruído (sem colchetes de tag).
function visibleText(text, drivers) {
  return segmentDriverMentions(text, drivers)
    .map((s) => s.text)
    .join("");
}

describe("buildDriverMentionMatcher", () => {
  it("realça o nome completo", () => {
    const drivers = [{ id: "1", nome: "Ramiro Ruiz" }];
    expect(mentionsIn("Grande corrida de Ramiro Ruiz hoje.", drivers)).toEqual(["1"]);
  });

  it("realça o sobrenome sozinho (segunda menção da IA)", () => {
    const drivers = [{ id: "1", nome: "Ramiro Ruiz" }];
    expect(mentionsIn("Ruiz saiu fortalecido do fim de semana.", drivers)).toEqual(["1"]);
  });

  it("realça o primeiro nome sozinho", () => {
    const drivers = [{ id: "1", nome: "Rodrigo Carvalho" }];
    expect(mentionsIn("Rodrigo cometeu um erro caro.", drivers)).toEqual(["1"]);
  });

  it("prioriza o nome completo sobre o sobrenome quando ambos aparecem", () => {
    const drivers = [{ id: "1", nome: "Ramiro Ruiz" }];
    // Um único match para o nome completo, não dois (nome + sobrenome).
    expect(mentionsIn("Ramiro Ruiz venceu.", drivers)).toEqual(["1"]);
  });

  it("não realça sobrenome ambíguo compartilhado por dois pilotos", () => {
    const drivers = [
      { id: "1", nome: "Bruno Silva" },
      { id: "2", nome: "Carlos Silva" },
    ];
    // "Silva" sozinho é ambíguo → não realça (evita acender o piloto errado).
    expect(mentionsIn("Silva liderou a prova.", drivers)).toEqual([]);
    // Mas os nomes completos continuam funcionando.
    expect(mentionsIn("Bruno Silva e Carlos Silva brigaram.", drivers)).toEqual(["1", "2"]);
  });

  it("não realça pedaço de outra palavra", () => {
    const drivers = [{ id: "1", nome: "Ramiro Ruiz" }];
    expect(mentionsIn("O cruzeiro passou.", drivers)).toEqual([]);
  });

  it("retorna null sem pilotos utilizáveis", () => {
    expect(buildDriverMentionMatcher([])).toBeNull();
    expect(buildDriverMentionMatcher(null)).toBeNull();
  });
});

describe("segmentDriverMentions — tags de apelido da IA", () => {
  const drivers = [{ id: "1", nome: "Nathaniel Turner" }];

  it("liga um APELIDO ao piloto e mostra só o texto visível", () => {
    const text = "Enquanto [[Nathaniel Turner|o líder]] administrava.";
    expect(segmentIds(text, drivers)).toEqual(["1"]);
    // O texto visível some com os colchetes; sobra "o líder".
    expect(visibleText(text, drivers)).toBe("Enquanto o líder administrava.");
    expect(segmentDriverMentions(text, drivers).find((s) => s.type === "driver")?.text).toBe(
      "o líder",
    );
  });

  it("aceita a forma curta [[Nome]] (visível = nome)", () => {
    expect(segmentIds("[[Nathaniel Turner]] venceu.", drivers)).toEqual(["1"]);
    expect(visibleText("[[Nathaniel Turner]] venceu.", drivers)).toBe("Nathaniel Turner venceu.");
  });

  it("liga várias referências do MESMO piloto sob nomenclaturas diferentes", () => {
    const text =
      "[[Nathaniel Turner]] largou bem. Depois, [[Nathaniel Turner|o novato]] cravou a volta rápida, e [[Nathaniel Turner|Turner]] fechou o dia.";
    expect(segmentIds(text, drivers)).toEqual(["1", "1", "1"]);
    expect(visibleText(text, drivers)).toBe(
      "Nathaniel Turner largou bem. Depois, o novato cravou a volta rápida, e Turner fechou o dia.",
    );
  });

  it("nunca mostra colchete cru quando o nome canônico não resolve", () => {
    const text = "O comissário [[Fulano Desconhecido|o diretor]] interveio.";
    expect(segmentIds(text, drivers)).toEqual([]);
    expect(visibleText(text, drivers)).toBe("O comissário o diretor interveio.");
  });

  it("mistura tag + nome cru sem tag no mesmo texto", () => {
    // "Turner" solto (sem tag) ainda acende pelo casamento por nome.
    const text = "[[Nathaniel Turner|O piloto da Northgate]] venceu; Turner ampliou a vantagem.";
    expect(segmentIds(text, drivers)).toEqual(["1", "1"]);
  });
});
