#!/usr/bin/env node
// MEDIÇÃO DO GEMINI — o último quinto do orçamento do push-to-talk.
//
// Depois que o desenho ganhou o caminho pré-gravado, o Gemini deixou de estar no caminho da
// maioria das perguntas e passou a atender só o que o acervo não cobre: ritmo, dano no carro,
// pergunta aberta, e os casos de borda que o renderizador recusa. É a perna menos crítica —
// e mesmo assim precisa de número, porque é ela que decide se o caminho lento cabe dentro da
// frase de espera de 2,36 s ou estoura.
//
// ## O material vem do código, não daqui
//
// Os dossiês são gerados pelo teste `dumpa_dossies_para_medicao` do crate Rust, com as mesmas
// funções que a produção usa. Escrever os fatos à mão neste arquivo mediria um prompt que a
// produção nunca vai emitir, e envelheceria no primeiro campo novo do `EstadoAgora`.
//
// Por isso a ordem é: rodar o teste Rust primeiro, depois este script.
//
// ## O que se mede
//
// - **Latência** por modelo, com percentis. É o número que fecha o orçamento.
// - **O texto da resposta.** Tão importante quanto o tempo: um engenheiro que responde certo
//   em três segundos é melhor que um que responde torto em um. O script imprime cada resposta
//   para audição de leitura — a avaliação é humana, e é de propósito.
// - **Aderência aos fatos.** Uma varredura simples atrás de números que NÃO estão no dossiê.
//   Não prova ausência de alucinação, mas pega a categoria mais perigosa dela: o número
//   inventado, que soa igualzinho a um número verdadeiro quando falado em voz alta.
//
// ## Thinking
//
// O `gemini-2.5-flash-lite` vem com raciocínio DESLIGADO por padrão — é o modelo que o proxy
// de notícias já usa. O `gemini-2.5-flash` vem com ele LIGADO, e num pedido de uma frase isso
// é latência pura. `--sem-pensar` tenta desligá-lo; se a API recusar o campo, o erro dela diz
// o nome certo, o que é mais confiável que adivinhar a partir da documentação.
//
// Uso:
//   node scripts/gemini-poc/medir.mjs
//   node scripts/gemini-poc/medir.mjs --repetir 3 --sem-pensar
//   node scripts/gemini-poc/medir.mjs --modelos gemini-2.5-flash-lite

import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const BASE = "https://generativelanguage.googleapis.com/v1beta/models";
const DOSSIES = path.join("docs", "scribe-poc", "dossies.json");
const TRANSCRICOES = path.join("docs", "scribe-poc", "transcricoes.json");

// A INSTRUÇÃO DO ENGENHEIRO. É o prompt de produção, não uma versão de teste — medir com um
// prompt mais curto daria uma latência otimista e um julgamento de qualidade sobre outra
// coisa.
//
// As três regras que mais importam, e por quê:
//
// 1. **Só os fatos da lista.** É a defesa contra o modo de falha que mata este produto: um
//    número inventado dito com a voz do engenheiro é indistinguível de um verdadeiro dentro
//    do carro, e o piloto não tem como conferir.
// 2. **A resposta vai ser LIDA EM VOZ ALTA.** Marcador, abreviação e símbolo viram ruído na
//    boca da TTS. "1,2 s" pode sair como "um vírgula dois esse".
// 3. **Uma ou duas frases.** Não é economia de token — a Cloud TTS é plana em relação ao
//    tamanho do texto. É que um rádio de corrida não comporta parágrafo.
const INSTRUCAO = `Você é o engenheiro de pista de um piloto e está falando com ele pelo rádio DURANTE a corrida.

Responda à pergunta dele usando SOMENTE os fatos da lista que vem junto. Nunca invente número, nome, posição ou situação que não esteja lá.

RESPONDA sempre que os fatos permitirem, mesmo que a pergunta não use as mesmas palavras da lista. "O carro aguenta até o fim?" se responde com o reparo pendente; "dá pra ganhar dele?" se responde com o gap e a diferença de ritmo. Só diga que não consegue ver quando o fato realmente não estiver na lista.

Se a lista tiver algo URGENTE — falta de combustível, bandeira preta, desclassificação, reparo obrigatório, pneu errado para a pista —, comece por isso, mesmo que a pergunta tenha sido genérica. Um engenheiro interrompe a resposta educada para dizer que o tanque não dá.

Os números da lista já estão escritos do jeito que se fala. COPIE-OS como estão: se a lista diz "um e dois", fale "um e dois"; se diz "oito décimos", fale "oito décimos". Não converta, não arredonde e não invente outra forma.

Fale em português do Brasil, na segunda pessoa, direto ao ponto, como quem fala com alguém a 200 por hora. Uma ou duas frases, no máximo.

A sua resposta vai ser LIDA EM VOZ ALTA por um sintetizador. Escreva só o que se fala: nada de listas, marcadores, parênteses, abreviações ou símbolos.

Diga os números como um engenheiro de corrida fala, e NUNCA leia a vírgula:
- 1,2 segundos é "um e dois". 2,1 segundos é "dois e um".
- 0,7 segundos é "sete décimos". 0,4 é "quatro décimos".
- 15 segundos é "quinze segundos".
- 1:32,8 é "um trinta e dois e oito".
Nunca escreva "um ponto dois" nem "um vírgula dois".`;

function lerArgumentos(argv) {
  const o = {
    modelos: ["gemini-2.5-flash-lite", "gemini-2.5-flash"],
    repetir: 2,
    semPensar: false,
  };
  for (let i = 2; i < argv.length; i += 1) {
    const a = argv[i];
    const proximo = () => argv[(i += 1)];
    if (a === "--modelos") o.modelos = proximo().split(",");
    else if (a === "--repetir") o.repetir = Number(proximo());
    else if (a === "--sem-pensar") o.semPensar = true;
  }
  return o;
}

/// A chave, do ambiente ou do arquivo — nunca de argumento, que fica no histórico do shell.
function chave() {
  const doAmbiente = process.env.GEMINI_API_KEY?.trim() || process.env.GOOGLE_API_KEY?.trim();
  if (doAmbiente) return doAmbiente;
  const arquivo = path.join(
    process.env.APPDATA || process.env.HOME || ".",
    "gemini_tts_key.txt",
  );
  if (fs.existsSync(arquivo)) return fs.readFileSync(arquivo, "utf8").trim();
  console.error(
    "Sem chave do Gemini. Defina GEMINI_API_KEY no ambiente ou crie\n" + `  ${arquivo}`,
  );
  process.exit(1);
}

/// A pergunta que casa com cada dossiê. O dossiê traz a intenção; aqui ela vira a frase que o
/// piloto teria dito — de preferência a MESMA que o Scribe transcreveu, para o prompt medido
/// ser o prompt real de ponta a ponta.
function perguntaPara(caso, intencao, transcricoes) {
  const daMedicao = transcricoes.find(
    (t) => t.sujeira === "limpo" && t.intencao_esperada === intencao,
  );
  if (daMedicao) return daMedicao.transcrito;
  const avulsas = {
    frente_trafego: "Qual o gap pro carro da frente?",
    combustivel_apertado: "E aí, como estamos?",
  };
  return avulsas[caso] ?? "E aí, como estamos?";
}

async function responder(modelo, pergunta, linhas, apiKey, semPensar) {
  const corpo = {
    systemInstruction: { parts: [{ text: INSTRUCAO }] },
    contents: [
      {
        role: "user",
        parts: [
          {
            text: `Fatos da corrida agora:\n${linhas.map((l) => `- ${l}`).join("\n")}\n\nPergunta do piloto: "${pergunta}"`,
          },
        ],
      },
    ],
    // 800 e não 200, e o motivo é uma armadilha do Gemini 2.5: **o `maxOutputTokens` é
    // compartilhado com os tokens de raciocínio.** Com 200, o `flash` gastava ~189 pensando e
    // sobravam sete para a resposta — que saía cortada no meio ("Sua última volta foi de um").
    // O sintoma não parece um teto de tokens; parece o modelo sendo burro.
    generationConfig: { maxOutputTokens: 800, temperature: 0.7 },
  };
  if (semPensar) corpo.generationConfig.thinkingConfig = { thinkingBudget: 0 };

  const t0 = performance.now();
  const resposta = await fetch(`${BASE}/${modelo}:generateContent`, {
    method: "POST",
    headers: { "x-goog-api-key": apiKey, "content-type": "application/json" },
    body: JSON.stringify(corpo),
  });
  const ms = Math.round(performance.now() - t0);
  if (!resposta.ok) {
    return { erro: `HTTP ${resposta.status}: ${(await resposta.text()).slice(0, 400)}`, ms };
  }
  const json = await resposta.json();
  const texto = (json.candidates?.[0]?.content?.parts ?? [])
    .map((p) => p.text ?? "")
    .join("")
    .trim();
  const uso = json.usageMetadata ?? {};
  return {
    texto,
    ms,
    entrada: uso.promptTokenCount ?? 0,
    saida: uso.candidatesTokenCount ?? 0,
    pensamento: uso.thoughtsTokenCount ?? 0,
  };
}

/// Números da resposta que NÃO aparecem no dossiê.
///
/// Não prova ausência de alucinação — prova a categoria mais perigosa dela. Um nome inventado
/// o piloto estranha; um NÚMERO inventado soa exatamente como um número verdadeiro quando dito
/// em voz alta, e ele não tem como conferir dentro do carro.
///
/// Números por extenso escapam desta varredura de propósito: pedimos ao modelo que escreva
/// assim, e cruzá-los exigiria um conversor que erraria mais que a checagem valeria. Ela é um
/// alarme barato, não uma prova.
function numerosInventados(texto, linhas) {
  const noDossie = new Set((linhas.join(" ").match(/\d+/g) ?? []));
  return (texto.match(/\d+/g) ?? []).filter((n) => !noDossie.has(n));
}

function percentil(valores, p) {
  if (!valores.length) return 0;
  const o = [...valores].sort((a, b) => a - b);
  return o[Math.min(o.length - 1, Math.floor((p / 100) * o.length))];
}

// ─────────────────────────────────────────────────────────────────────────────

const opcoes = lerArgumentos(process.argv);
const apiKey = chave();

if (!fs.existsSync(DOSSIES)) {
  console.error(
    `Sem ${DOSSIES}. Gere primeiro:\n` +
      "  cargo test --lib dumpa_dossies_para_medicao -- --nocapture",
  );
  process.exit(1);
}
const dossies = JSON.parse(fs.readFileSync(DOSSIES, "utf8"));
const transcricoes = fs.existsSync(TRANSCRICOES)
  ? JSON.parse(fs.readFileSync(TRANSCRICOES, "utf8"))
  : [];

console.log(
  `\nModelos: ${opcoes.modelos.join(", ")} · ${opcoes.repetir}× cada caso · ` +
    `${opcoes.semPensar ? "thinking DESLIGADO" : "thinking no padrão do modelo"}\n`,
);

const linhas = [];
for (const modelo of opcoes.modelos) {
  console.log(`── ${modelo} ${"─".repeat(Math.max(0, 60 - modelo.length))}`);
  for (const d of dossies) {
    const pergunta = perguntaPara(d.caso, d.intencao, transcricoes);
    for (let v = 0; v < opcoes.repetir; v += 1) {
      const r = await responder(modelo, pergunta, d.linhas, apiKey, opcoes.semPensar);
      if (r.erro) {
        console.log(`  ✗ ${d.caso}: ${r.erro}`);
        continue;
      }
      const inventados = numerosInventados(r.texto, d.linhas);
      linhas.push({ modelo, caso: d.caso, ...r, inventados });
      if (v === 0) {
        console.log(`\n  ${d.caso}  (${d.linhas.length} linhas de fato)`);
        console.log(`    piloto:  "${pergunta}"`);
        console.log(`    resposta: "${r.texto}"`);
      }
      const alarme = inventados.length ? `  ⚠ número fora do dossiê: ${inventados.join(", ")}` : "";
      console.log(
        `    ${String(r.ms).padStart(5)} ms  ${String(r.entrada).padStart(4)} in / ` +
          `${String(r.saida).padStart(3)} out` +
          (r.pensamento ? ` / ${r.pensamento} pensando` : "") +
          alarme,
      );
    }
  }
  console.log();
}

// ─── Os números ──────────────────────────────────────────────────────────────

console.log("─".repeat(78));
console.log("\nLATÊNCIA DO GEMINI (chamada inteira, sem streaming)\n");
for (const modelo of opcoes.modelos) {
  const ms = linhas.filter((l) => l.modelo === modelo).map((l) => l.ms);
  if (!ms.length) continue;
  console.log(
    `  ${modelo.padEnd(24)} n ${String(ms.length).padStart(3)}   ` +
      `melhor ${String(percentil(ms, 0)).padStart(5)} ms   mediana ${String(percentil(ms, 50)).padStart(5)} ms   ` +
      `P90 ${String(percentil(ms, 90)).padStart(5)} ms   pior ${String(percentil(ms, 100)).padStart(5)} ms`,
  );
}

console.log("\nTOKENS E ADERÊNCIA\n");
for (const modelo of opcoes.modelos) {
  const g = linhas.filter((l) => l.modelo === modelo);
  if (!g.length) continue;
  const media = (f) => Math.round(g.reduce((s, l) => s + f(l), 0) / g.length);
  const suspeitas = g.filter((l) => l.inventados.length);
  console.log(
    `  ${modelo.padEnd(24)} ${media((l) => l.entrada)} tokens de entrada · ` +
      `${media((l) => l.saida)} de saída` +
      (media((l) => l.pensamento) ? ` · ${media((l) => l.pensamento)} de pensamento` : "") +
      ` · ${suspeitas.length}/${g.length} com número fora do dossiê`,
  );
  for (const s of suspeitas) {
    console.log(`      ${s.caso}: ${s.inventados.join(", ")} — "${s.texto}"`);
  }
}

console.log(
  "\nAs respostas acima são para LEITURA, não para métrica: o que decide se o engenheiro\n" +
    "convence é como ele soa, e isso nenhum número mede.\n",
);
