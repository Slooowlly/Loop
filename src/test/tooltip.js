// Busca por tooltip do app nos testes.
//
// O `title=` nativo tinha `getByTitle` de graça. O balão do app guarda o texto
// em `data-tooltip` no gatilho — mesma informação, atributo diferente — e a
// testing-library não tem consulta por atributo arbitrário. Estas funções são
// esse `getByTitle`, aceitando string exata ou regex como ele aceitava.

function alvos(escopo) {
  return [...(escopo ?? document).querySelectorAll("[data-tooltip]")];
}

function casa(elemento, procurado) {
  const texto = elemento.getAttribute("data-tooltip");
  if (procurado instanceof RegExp) return procurado.test(texto);
  return texto === procurado;
}

/// Todos os gatilhos cujo balão casa — vazio se nenhum.
export function todosPorTooltip(procurado, escopo) {
  return alvos(escopo).filter((elemento) => casa(elemento, procurado));
}

/// O primeiro gatilho cujo balão casa. Estoura como as consultas `getBy*`:
/// um teste que não acha o que procurava tem de falhar dizendo o que procurava.
export function porTooltip(procurado, escopo) {
  const [primeiro] = todosPorTooltip(procurado, escopo);
  if (!primeiro) {
    throw new Error(`Nenhum elemento com data-tooltip casando com ${procurado}`);
  }
  return primeiro;
}
