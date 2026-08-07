// A cadeia de rádio da equipe, montada como GRAFO PERSISTENTE.
//
// Nasceu na POC de TTS (Etapa 10) e mudou de lugar quando deixou de ser experimento: o
// engenheiro tem DOIS caminhos de voz, e eles precisam soar como a mesma pessoa no mesmo
// rádio. As 480 peças do acervo já saem do gerador com esta cadeia GRAVADA dentro (elas
// nunca são coladas umas nas outras, então dá para filtrar de uma vez, offline). O áudio
// que o modelo escreve, não: chega cru da Cloud TTS, em tempo de corrida, e tem de passar
// por aqui ao vivo. Se as duas receitas divergirem, o jogador ouve o engenheiro trocar de
// rádio no meio da conversa.
//
// A receita em si está espelhada em `scripts/tts-poc/filtro-radio.mjs`, que é a versão
// offline usada pelo gerador de pacote e pelo auditor. Mexer numa exige mexer na outra.
//
// O grafo é construído UMA vez, antes do primeiro byte chegar, e cada bloco de áudio se
// liga na entrada dele. Assim o processamento acontece ENQUANTO o áudio toca, não antes:
// nenhum nó daqui adia o agendamento, então o "tempo até o primeiro som" não muda. O
// único custo real é o look-ahead do compressor, poucos milissegundos embutidos no
// caminho de sinal do próprio Chromium.
//
// Cadeia: passa-altas -> passa-baixas -> presença -> saturação -> compressão -> ganho.
// É a receita clássica de rádio de equipe: corta o grave que o alto-falante do capacete
// não reproduz, corta o agudo que a portadora não carrega, empurra a faixa da voz,
// suja de leve e nivela.

const CORTE_GRAVE_HZ = 300; // abaixo disso é peso que o rádio não transmite
const CORTE_AGUDO_HZ = 3200; // acima disso é sibilância que a portadora come
const PRESENCA_HZ = 1800; // faixa da inteligibilidade da fala
const PRESENCA_DB = 6;

/** Curva de saturação suave (soft clip). `k` controla o quanto suja. */
function curvaSaturacao(k = 4, pontos = 1024) {
  const curva = new Float32Array(pontos);
  for (let i = 0; i < pontos; i += 1) {
    const x = (i * 2) / (pontos - 1) - 1; // -1 .. 1
    curva[i] = Math.tanh(k * x) / Math.tanh(k);
  }
  return curva;
}

/**
 * Faixa de entrada para a qual a curva do limitador é desenhada.
 *
 * Vai a ±2 porque é isso que chega depois da recuperação de nível — uma curva desenhada
 * só até ±1 recortaria justamente o que ela existe para salvar.
 *
 * O NÓ, porém, não sabe disso: o `WaveShaper` do Web Audio indexa a curva SEMPRE pela
 * faixa ±1 da entrada, e o que estiver fora é grampeado nas pontas. Desenhar para ±2 sem
 * dividir a entrada por 2 antes fazia o nó calcular `limite(2x)` — um expansor de 2×
 * seguido de parede, e não um limitador. Medido: a mesma fala saía em RMS 0,61 pela
 * cadeia do navegador contra 0,14 pela receita offline, esmagada contra o teto. Era o
 * "áudio ensurdecedor", e o motivo de duas falas no MESMO RMS soarem em alturas
 * diferentes — o esmagamento depende de quanto sinal passou do joelho.
 */
const LIMITADOR_ENTRADA_MAX = 2;

/**
 * Curva do limitador final: abaixo de `t` não toca em nada, acima curva suave até 1,0.
 * Desenhada sobre `entradaMax`; quem chama tem de dividir a entrada por ele. Ver acima.
 */
function curvaLimitador(t = 0.7, teto = 0.97, entradaMax = LIMITADOR_ENTRADA_MAX, pontos = 2048) {
  const curva = new Float32Array(pontos);
  const faixa = teto - t;
  for (let i = 0; i < pontos; i += 1) {
    const x = ((i * 2) / (pontos - 1) - 1) * entradaMax;
    const a = Math.abs(x);
    curva[i] = a <= t ? x : Math.sign(x) * (t + faixa * Math.tanh((a - t) / faixa));
  }
  return curva;
}

/**
 * Monta o grafo e devolve o nó de ENTRADA — é nele que cada bloco de áudio se liga.
 *
 * `saida` é para onde a cadeia despeja; sem ela, direto no destino do contexto. Quem toca
 * pelo mixer do engenheiro passa o gain mestre aqui, senão o volume da voz do modelo
 * ficaria fora do controle que rege as peças gravadas.
 *
 * Com `ligado = false` a entrada vai direto na saída — a voz limpa, sem rádio.
 */
export function criarCadeiaRadio(ctx, { ligado = true, saida = null } = {}) {
  const destino = saida ?? ctx.destination;
  const entrada = ctx.createGain();
  entrada.gain.value = 1;

  if (!ligado) {
    entrada.connect(destino);
    return { entrada, ligado: false, nos: [] };
  }

  const passaAltas = ctx.createBiquadFilter();
  passaAltas.type = "highpass";
  passaAltas.frequency.value = CORTE_GRAVE_HZ;
  passaAltas.Q.value = 0.7;

  const passaBaixas = ctx.createBiquadFilter();
  passaBaixas.type = "lowpass";
  passaBaixas.frequency.value = CORTE_AGUDO_HZ;
  passaBaixas.Q.value = 0.7;

  const presenca = ctx.createBiquadFilter();
  presenca.type = "peaking";
  presenca.frequency.value = PRESENCA_HZ;
  presenca.Q.value = 1.1;
  presenca.gain.value = PRESENCA_DB;

  const saturacao = ctx.createWaveShaper();
  saturacao.curve = curvaSaturacao(3);
  saturacao.oversample = "2x"; // sem isso a saturação gera aliasing audível

  // Normalização ANTES do compressor, de propósito. O corte de banda derruba o nível
  // e isto devolve — mas se o ganho viesse por último, ninguém seguraria o estouro:
  // medido com impulso, a cadeia com ganho na saída entregava pico de 1,298 (clipe
  // garantido). Empurrando aqui, o compressor logo abaixo é quem limita.
  const ganho = ctx.createGain();
  ganho.gain.value = 1.9;

  const compressor = ctx.createDynamicsCompressor();
  compressor.threshold.value = -24;
  compressor.knee.value = 6;
  compressor.ratio.value = 8; // rádio é achatado: dinâmica quase nenhuma
  compressor.attack.value = 0.003;
  compressor.release.value = 0.12;

  // Recuperação de nível DEPOIS do compressor. Sem isso a cadeia sai três a quatro
  // vezes mais baixa que a voz limpa: com limiar em -24 dB e razão 8:1, tudo que é fala fica
  // achatado contra o limiar, e o ganho de entrada só empurra o sinal mais fundo na
  // compressão. Medido em fala real: RMS caía de 0,14 para 0,04.
  //
  // O pico continua seguro porque quem limita é o compressor logo acima — este ganho
  // multiplica um sinal já achatado.
  const recuperacao = ctx.createGain();
  recuperacao.gain.value = 3.5;

  // Encolhe a entrada para a faixa que o `WaveShaper` enxerga (±1). A curva abaixo é
  // desenhada em unidades REAIS de amostra, até ±2 — este nó é o que casa as duas. Sem
  // ele o limitador vira expansor; ver `LIMITADOR_ENTRADA_MAX`.
  const preLimitador = ctx.createGain();
  preLimitador.gain.value = 1 / LIMITADOR_ENTRADA_MAX;

  // Limitador de amostra, por último. O compressor tem ataque de 3 ms e o transiente
  // escapa por baixo dele: medido em fala real, os picos chegavam a 1,7 depois da
  // recuperação. Este nó mexe só nos poucos milissegundos que passam do teto.
  const limitador = ctx.createWaveShaper();
  limitador.curve = curvaLimitador();
  limitador.oversample = "none"; // já opera na faixa da voz; sobreamostrar aqui só custa

  const nos = [
    passaAltas,
    passaBaixas,
    presenca,
    saturacao,
    ganho,
    compressor,
    recuperacao,
    preLimitador,
    limitador,
  ];
  entrada
    .connect(passaAltas)
    .connect(passaBaixas)
    .connect(presenca)
    .connect(saturacao)
    .connect(ganho)
    .connect(compressor)
    .connect(recuperacao)
    .connect(preLimitador)
    .connect(limitador)
    .connect(destino);

  return { entrada, ligado: true, nos };
}
