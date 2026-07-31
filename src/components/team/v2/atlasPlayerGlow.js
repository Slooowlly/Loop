// Luz de descoberta no card da categoria ATUAL do jogador.
//
// A coluna de rankings é uma lista de nome, pontos e troféus: lê como conteúdo, e
// nada nela diz que cada card é uma porta. O chevron anuncia o caminho para quem
// já está com o olho na linha; a luz existe antes disso — ela puxa o olho para um
// card específico, e o card escolhido é o único que o jogador tem motivo próprio
// para abrir.
//
// Apaga no primeiro clique dentro do card, e não volta mais: a luz é um convite,
// e convite repetido depois de aceito vira ruído. O registro é por instalação, não
// por carreira — é conhecimento sobre a interface, não sobre o mundo do jogo, e
// quem já descobriu não precisa redescobrir a cada save novo.
const STORAGE_KEY = "loop.atlas.playerBandGlow";

export function isPlayerGlowDismissed() {
  try {
    return localStorage.getItem(STORAGE_KEY) === "1";
  } catch {
    // localStorage indisponível — a luz só deixa de persistir, sem quebrar a tela.
    return false;
  }
}

export function dismissPlayerGlow() {
  try {
    localStorage.setItem(STORAGE_KEY, "1");
  } catch {
    /* idem: sem persistência, a luz volta na próxima abertura */
  }
}
