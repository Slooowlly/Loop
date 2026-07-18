// Dados FICTÍCIOS para o protótipo da torre de tempos.
//
// Os nomes batem com os do TeamLogoMark (logos resolvem de verdade) e as CORES
// são as oficiais do jogo, copiadas de `src-tauri/src/constants/teams.rs`
// (`cor_primaria`) — então a cor da linha combina com o logo. Na versão ao vivo
// elas virão do `equipe_cor` do nosso elenco, que é a mesma fonte.
//
// Formato de cada carro:
//   pos        posição na CLASSE
//   name       nome do piloto
//   team       nome da equipe (chave do logo)
//   color      cor primária REAL da equipe (#rrggbb)
//   delta      posições ganhas(+)/perdidas(-) desde a largada
//   stops      paradas de pit
//   points     pontos no campeonato ANTES desta corrida
//   gain       pontos que ele somaria terminando na posição atual
//   fastest    melhor volta (mm:ss.mmm)
//   fol        fastest of lap — se é a volta mais rápida da CLASSE (roxo + pin)
//   pit        está no pit agora (pin)
//   flag       bandeira ativa: "meatball" | "black" | "checkered" | ausente (pin)
//   player     é o carro do jogador (linha verde)
//
// O COMPANHEIRO DE EQUIPE não é campo: é derivado (mesmo `team` do jogador) e
// ganha realce azul. Ver towerRows.js.

export const OVERLAY_MOCK = {
  session: {
    type: "R",
    lap: 36,
    totalLaps: 40,
    flag: "green",
    category: "endurance", // logo + nome + cor da categoria no header
    weather: { condition: "clouds", airTemp: 21, trackTemp: 28 },
  },
  classes: [
    {
      id: "gt3",
      label: "GT3",
      color: "#e73f47",
      cars: [
        { pos: 1, name: "Renan Azeredo", team: "Ferrari", color: "#dc0000", delta: 0, stops: 1, points: 142, gain: 25, fastest: "01:42.198", fol: false, pit: false, player: false },
        { pos: 2, name: "Marco I D'Acunto", team: "Porsche", color: "#111111", delta: 0, stops: 1, points: 138, gain: 18, fastest: "01:42.089", fol: true, pit: false, player: false },
        { pos: 3, name: "Fabian Seischegg", team: "Mercedes-AMG", color: "#00d2be", delta: 1, stops: 1, points: 120, gain: 15, fastest: "01:42.702", fol: false, pit: false, player: false },
        { pos: 4, name: "Sergi Heras", team: "Lamborghini", color: "#ffd100", delta: 1, stops: 1, points: 110, gain: 12, fastest: "01:43.383", fol: false, pit: true, player: false },
        { pos: 5, name: "Rogerio Silva", team: "Audi", color: "#bb0a21", delta: 3, stops: 1, points: 98, gain: 10, fastest: "01:43.946", fol: false, pit: false, player: false },
        { pos: 6, name: "Pawel T. Okreglicki", team: "McLaren", color: "#ff8700", delta: 4, stops: 1, points: 86, gain: 8, fastest: "01:43.893", fol: false, pit: false, player: false },
        { pos: 7, name: "Matthew Koontz", team: "BMW", color: "#0057b8", delta: 2, stops: 1, points: 72, gain: 6, fastest: "01:43.715", fol: false, pit: false, player: false },
        { pos: 8, name: "Mustafa Mezraoui", team: "Aston Martin", color: "#005f48", delta: -1, stops: 2, points: 64, gain: 4, fastest: "01:43.403", fol: false, pit: false, flag: "meatball", tire: "wet", player: false },
        // Trocou pra chuva na 3ª parada (histórico misto: 2 secos + 1 chuva).
        { pos: 9, name: "Adaildo Vieira", team: "Chevrolet", color: "#f9c80e", delta: -6, stops: 2, points: 61, gain: 2, fastest: "01:42.278", fol: false, pit: true, tireHistory: ["dry", "dry", "wet"], player: false },
        // Companheiro de equipe do jogador (mesmo time: Kitsune) -> realce azul.
        { pos: 10, name: "Neil Cooper", team: "Kitsune", color: "#3a0ca3", delta: 1, stops: 1, points: 55, gain: 1, fastest: "01:44.002", fol: false, pit: false, player: false },
        { pos: 11, name: "Rick Van Zwiet", team: "Acura", color: "#c8102e", delta: 1, stops: 2, points: 48, gain: 0, fastest: "01:44.059", fol: false, pit: true, player: false },
        { pos: 12, name: "Istvan Fodor", team: "Kitsune", color: "#3a0ca3", delta: 1, stops: 1, points: 44, gain: 0, fastest: "01:45.035", fol: false, pit: false, player: true },
        { pos: 13, name: "Alexandr Fescov", team: "Valkyrie", color: "#6f4e37", delta: 2, stops: 1, points: 40, gain: 0, fastest: "01:44.822", fol: false, pit: false, player: false },
        { pos: 14, name: "Marius Rieck", team: "Obsidian", color: "#2b2d42", delta: 0, stops: 1, points: 32, gain: 0, fastest: "01:44.754", fol: false, pit: false, flag: "black", player: false },
        { pos: 15, name: "Preston Perlmutter", team: "Audi", color: "#bb0a21", delta: -9, stops: 2, points: 28, gain: 0, fastest: "01:43.111", fol: false, pit: false, player: false },
      ],
    },
    {
      id: "gt4",
      label: "GT4",
      color: "#58a6ff",
      cars: [
        { pos: 1, name: "Sindre Setsaas", team: "Rahal Letterman Racing", color: "#0057b8", delta: 1, stops: 1, points: 156, gain: 25, fastest: "01:38.635", fol: true, pit: false, player: false },
        { pos: 2, name: "Samuel Libeert", team: "Toksport World Racing", color: "#a4161a", delta: 2, stops: 1, points: 140, gain: 18, fastest: "01:39.281", fol: false, pit: false, player: false },
        { pos: 3, name: "Antti Terho", team: "Heart of Racing", color: "#006f52", delta: 0, stops: 1, points: 128, gain: 15, fastest: "01:39.209", fol: false, pit: false, player: false },
      ],
    },
    {
      id: "prod",
      label: "PROD",
      color: "#d29922",
      cars: [
        { pos: 1, name: "Matthew Naylor", team: "Aperture", color: "#1B9CFC", delta: 0, stops: 1, points: 88, gain: 25, fastest: "01:38.021", fol: true, pit: false, player: false },
      ],
    },
  ],
};
