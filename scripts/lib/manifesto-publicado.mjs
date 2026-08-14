// Leitura defensiva do `latest.json` que já está no ar.
//
// A etapa de verificação do release lia `live.platforms[target].url` direto. Quando o
// manifesto sobe sem a plataforma do canal — `--channel` trocado na geração, upload no
// caminho do outro canal, manifesto de outra ferramenta — aquilo estourava um
// `TypeError: Cannot read properties of undefined` no meio do script, DEPOIS do upload.
// O jogador ficava com um manifesto que o updater recusa e o operador com um stack trace
// que não diz qual canal olhar.
//
// Aqui a mesma falha vira erro de domínio, dizendo versão, canal, target e o que o
// manifesto de fato tem dentro.

/// Devolve `{ url }` ou `{ erro }`. Nunca lança.
export function urlDoInstalador(live, { alvo, canal, versao }) {
  const cabecalho = `Versão ${versao}, canal ${canal}, target \`${alvo}\`.`;
  const rodape =
    `  O instalador JÁ foi publicado — não desfaça nada na árvore.\n` +
    `  Confira o --channel do make-update-manifest.mjs e o caminho do upload do latest.json.`;

  if (!live || typeof live !== "object" || Array.isArray(live)) {
    return { erro: `O manifesto publicado não é um objeto JSON.\n  ${cabecalho}\n${rodape}` };
  }

  const plataformas = live.platforms;
  if (!plataformas || typeof plataformas !== "object" || Array.isArray(plataformas)) {
    return {
      erro:
        `O manifesto publicado não tem o bloco \`platforms\`.\n` +
        `  ${cabecalho}\n` +
        `  Chaves encontradas: ${listar(Object.keys(live))}\n${rodape}`,
    };
  }

  const plataforma = plataformas[alvo];
  if (!plataforma || typeof plataforma !== "object" || Array.isArray(plataforma)) {
    return {
      erro:
        `O manifesto publicado não tem a plataforma \`${alvo}\`.\n` +
        `  ${cabecalho}\n` +
        `  Plataformas no manifesto: ${listar(Object.keys(plataformas))}\n` +
        `  Sem ela o updater do canal ${canal} não acha download nenhum.\n${rodape}`,
    };
  }

  const url = plataforma.url;
  if (typeof url !== "string" || !url.trim()) {
    return {
      erro:
        `A plataforma \`${alvo}\` do manifesto publicado está sem \`url\` utilizável.\n` +
        `  ${cabecalho}\n` +
        `  Valor lido: ${JSON.stringify(url ?? null)}\n${rodape}`,
    };
  }

  return { url: url.trim() };
}

function listar(chaves) {
  return chaves.length ? chaves.map((c) => `\`${c}\``).join(", ") : "(nenhuma)";
}
