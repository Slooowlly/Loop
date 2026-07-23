# Tags de menção de piloto (contrato servidor ↔ app)

Problema: o boletim de IA repetia o sobrenome do mesmo piloto dezenas de vezes
("Turner… Turner… Turner"). Queremos que a IA **varie** as referências (nome
completo, primeiro nome, apelidos: "o líder", "o novato da Northgate") **e** que
CADA referência continue clicável/realçável no app.

O casamento por nome sozinho não liga um apelido ao piloto — "o líder" não contém
o nome. Então a IA **marca** cada referência com a identidade canônica, e o app
parseia essa marcação.

## Formato (sintaxe estilo wiki)

    [[Nome Canônico|texto visível]]   → mostra "texto visível", liga ao piloto
    [[Nome Canônico]]                 → mostra e liga o próprio nome

- **Nome Canônico** = o nome do piloto EXATAMENTE como aparece nos `facts`
  enviados ao servidor (é o que resolve o id no app).
- **texto visível** = como o piloto aparece naquele ponto do texto (apelido,
  primeiro nome, sobrenome…).

Exemplos:

    [[Nathaniel Turner]] liderou desde a largada.
    Enquanto [[Nathaniel Turner|o líder]] administrava, o pelotão se reorganizava.
    [[Nathaniel Turner|Turner]] ampliou a vantagem para 17 pontos.

## Lado do app (já implementado)

`src/utils/driverMentions.jsx` → `segmentDriverMentions(text, drivers)` extrai as
tags, resolve `Nome Canônico → id`, renderiza só o texto visível já ligado ao hover
e some com os colchetes. Consumido por: boletim (`NewsMagazineTab`), debrief
(`RaceResultViewV2 / SpeechWords`) e prévia (`NextRaceTab`).

Retrocompatível: texto sem tags cai no casamento por nome de sempre. Tag cujo nome
canônico não resolve nunca mostra colchete cru — cai no texto visível puro.

## Escopo recomendado no prompt

Marcar nomes e **apelidos descritivos** que substituem o piloto. **Não** marcar
pronomes soltos ("ele", "sua") — deixaria o texto com metade das palavras
sublinhadas. Se um dia quiser pronomes clicáveis também, basta ampliar a instrução;
o app já liga qualquer coisa que vier marcada.
