// Animação de posição da torre: quando um piloto ultrapassa (ou é ultrapassado), a
// LINHA INTEIRA dele desliza pro lugar novo em vez de teleportar.
//
// O módulo não sabe desenhar nada. Ele só guarda, por linha, onde ela está AGORA e
// pra onde está indo — `drawTower` pergunta e desenha no y que sair daqui. Manter
// isso separado do canvas é o que deixa a regra testável sem um `ctx`.
//
// Duas transições, de propósito:
//   • DESLIZE — a linha existia antes e mudou de y. É a ultrapassagem.
//   • FADE    — a linha é nova na torre. Acontece porque a torre é uma JANELA
//               (top N + vizinhança do jogador, ver towerRows.js): um carro pode
//               entrar na faixa visível sem ter vindo de lugar nenhum, e aparecer
//               do nada no meio de um deslize fica pior do que surgir suave.
//
// Saída da faixa visível NÃO é animada: a linha some. Animar isso exigiria manter um
// fantasma ocupando um espaço que o layout novo já deu pra outro carro.

export const SLIDE_MS = 260;
export const FADE_MS = 180;

const clamp01 = (n) => (n < 0 ? 0 : n > 1 ? 1 : n);
// Sai rápido, chega devagar — a leitura fica "o carro ganhou a posição", não "as
// linhas trocaram de lugar".
const easeOutCubic = (t) => 1 - (1 - t) ** 3;

/**
 * Chave ESTÁVEL de uma linha. `idx` é o `CarIdx` do iRacing e é o que sobrevive a
 * troca de posição, box e sumiço da telemetria. O nome é reserva pro preview/mock,
 * que não têm idx.
 */
export function rowKey(car) {
  if (!car) return "";
  return Number.isInteger(car.idx) && car.idx >= 0 ? `i${car.idx}` : `n${car.name ?? ""}`;
}

/**
 * Cria o estado de animação da torre. Um por canvas — monitor e VR têm o seu, senão
 * dois ritmos de desenho brigariam pelo mesmo `from`/`to`.
 */
export function createTowerAnimator({ slideMs = SLIDE_MS, fadeMs = FADE_MS } = {}) {
  /** @type {Map<string, {from:number,to:number,startedAt:number|null,bornAt:number}>} */
  let rows = new Map();

  function sampleY(r, now) {
    // `startedAt` é NULO enquanto a linha nunca deslizou. Já foi 0, que colidia com um
    // `now` real de 0 — um deslize que começasse no instante zero do relógio era lido
    // como "parada" e a linha saltava pro destino.
    if (slideMs <= 0 || r.startedAt === null) return r.to;
    const t = clamp01((now - r.startedAt) / slideMs);
    return r.from + (r.to - r.from) * easeOutCubic(t);
  }

  return {
    /**
     * Informa o layout do quadro atual — `Map(chave -> y de destino)`. Roda a cada
     * desenho, não só quando chegam dados novos: é barato e evita um caminho separado.
     * @param {Map<string, number>} layout
     * @param {number} now  ms monotônicos (performance.now)
     */
    sync(layout, now) {
      for (const [key, y] of layout) {
        const cur = rows.get(key);
        if (!cur) {
          // Nasce parada no lugar certo; quem anima a entrada é o fade.
          rows.set(key, { from: y, to: y, startedAt: null, bornAt: now });
        } else if (cur.to !== y) {
          // Começa o deslize DE ONDE A LINHA ESTÁ, não do y antigo: ultrapassagem em
          // cima de ultrapassagem (posição muda de novo no meio do trajeto) tem que
          // continuar de onde parou, senão dá solavanco pra trás.
          cur.from = sampleY(cur, now);
          cur.to = y;
          cur.startedAt = now;
        }
      }
      // Some da torre → esquece. Se voltar, entra como linha nova (fade).
      for (const key of [...rows.keys()]) {
        if (!layout.has(key)) rows.delete(key);
      }
    },

    /**
     * Onde desenhar a linha agora. `null` se a chave não foi sincronizada.
     * @returns {{y:number, alpha:number}|null}
     */
    state(key, now) {
      const r = rows.get(key);
      if (!r) return null;
      const age = now - r.bornAt;
      const alpha = fadeMs > 0 && age < fadeMs ? clamp01(age / fadeMs) : 1;
      return { y: sampleY(r, now), alpha };
    },

    /**
     * Alguma linha está se mexendo agora (deslizando ou aparecendo)? Serve pra quem
     * paga caro por quadro decidir a taxa de desenho: o VR redesenha devagar parado e
     * acelera só durante a ultrapassagem, em vez de manter taxa alta o tempo todo.
     */
    hasMotion(now) {
      for (const r of rows.values()) {
        if (slideMs > 0 && r.startedAt !== null && now - r.startedAt < slideMs) return true;
        if (fadeMs > 0 && now - r.bornAt < fadeMs) return true;
      }
      return false;
    },

    /** Zera tudo (troca de sessão, overlay reaberto). */
    reset() {
      rows = new Map();
    },
  };
}
