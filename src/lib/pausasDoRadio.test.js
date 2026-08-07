import { describe, expect, it } from "vitest";

import { JUNCAO, pausasDoRadio } from "./pausasDoRadio";

describe("pausasDoRadio", () => {
  it("devolve uma pausa a menos que o número de peças", () => {
    // A invariante de que tudo o mais depende: quem consome casa `pecas[i]` com `pausas[i]`.
    // Uma pausa a mais ou a menos desloca a fala inteira sem erro nenhum aparecer.
    expect(pausasDoRadio([])).toEqual([]);
    expect(pausasDoRadio(["posicao_5"])).toEqual([]);
    expect(pausasDoRadio(["ab_rival", "nm_cooper", "viz_frente_12"])).toHaveLength(2);
  });

  it("emenda o vocativo com respiro de vírgula", () => {
    expect(pausasDoRadio(["voc_novato", "posicao_5"])).toEqual([JUNCAO.virgula]);
  });

  it("o vocativo ganha da regra de frase inteira que vem depois dele", () => {
    // O caso que a ordem das regras existe para resolver. `camp_` pede 220 ms de quem vier
    // antes; o vocativo pede 90. Fosse a regra de `camp_` a decidir, "Novato," ficaria
    // pendurado longe do resto — e um vocativo separado por esse tanto vira uma frase de uma
    // palavra só.
    expect(pausasDoRadio(["voc_novato", "camp_pos_3"])).toEqual([JUNCAO.virgula]);
    expect(pausasDoRadio(["voc_novato", "mem_menos_4"])).toEqual([JUNCAO.virgula]);
  });

  it("o vocativo não atropela as junções do resto da fala", () => {
    // A fala montada inteira, com o vocativo na frente: ele acrescenta a sua junção e não
    // muda nenhuma das outras.
    const sem = pausasDoRadio(["ab_rival", "nm_cooper", "viz_frente_12", "camp_pos_3"]);
    const com = pausasDoRadio(["voc_novato", "ab_rival", "nm_cooper", "viz_frente_12", "camp_pos_3"]);
    expect(com).toEqual([JUNCAO.virgula, ...sem]);
  });

  it("as junções conhecidas continuam onde estavam", () => {
    expect(pausasDoRadio(["ab_piloto1", "eq_kitsune"])).toEqual([JUNCAO.artigo]);
    expect(pausasDoRadio(["ab_rival", "nm_cooper"])).toEqual([JUNCAO.virgula]);
    expect(pausasDoRadio(["nm_cooper", "t_924"])).toEqual([JUNCAO.virgula]);
    expect(pausasDoRadio(["t_924", "qb_motor"])).toEqual([JUNCAO.frase]);
    expect(pausasDoRadio(["posicao_5", "camp_pos_3"])).toEqual([JUNCAO.frase]);
    // O ramo padrão: dentro da mesma oração.
    expect(pausasDoRadio(["nm_cooper", "qb_motor"])).toEqual([JUNCAO.oracao]);
  });
});
