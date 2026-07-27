// Estado inicial do store de carreira. Fica isolado porque `clearCareer` o
// reaplica inteiro (`set({ ...initialState })`) e vários slices o referenciam.
export const initialState = {
  isLoaded: false,
  isLoading: false,
  isSimulating: false,
  isAdvancing: false,
  isCalendarAdvancing: false,
  isAdvancingWeek: false,
  isEnteringPreseason: false,
  isRespondingProposal: false,
  isResolvingPoach: false,
  isConvocating: false,
  isDirty: false,
  lastSaved: null,
  error: null,
  careerId: null,
  difficulty: null,
  // Idioma escolhido no menu (Settings). Decide se os FALLBACKS determinísticos
  // (PT) aparecem, ou se, em outro idioma, mostramos "erro na geração de texto"
  // no idioma escolhido em vez de texto em português.
  language: "pt-BR",
  player: null,
  playerTeam: null,
  season: null,
  nextRace: null,
  nextRaceBriefing: null,
  // Categoria atualmente EM EXIBIÇÃO na Home (aba Standings). O jogador troca de
  // série/tier na tabela e o banner cinematográfico do topo acompanha, mostrando a
  // próxima corrida daquela categoria. `null` = vendo a própria categoria do jogador
  // (comportamento padrão do banner, que lê `nextRace`). A StandingsTab mantém isto
  // sincronizado e o zera ao desmontar.
  homeCategory: null,
  // Prévia por IA pré-buscada durante a animação de avanço (evita o flash template→IA).
  preRaceAi: null,
  // Standings pré-buscados junto com a IA (get_drivers_by_category + get_teams_standings
  // + histórico de frases). A pré-corrida é ESTÁTICA até a corrida rodar, então a Sala de
  // Estratégia lê deste cache e abre os Favoritos na hora, sem re-buscar os comandos
  // pesados ao montar. Chaveado por `raceId`; muda de etapa → cache miss → busca de novo.
  preRaceStandings: null,
  temporalSummary: null,
  calendarDisplayDate: null,
  displayDaysUntilNextEvent: null,
  totalDrivers: 0,
  totalTeams: 0,
  // Pilotos de interesse do jogador (1 Nemesis + até 2 Rivais) — decoram os nomes
  // com o marcador de rivalidade (💥/🔥). { nemesis, rivais } ou null enquanto carrega.
  playerInterests: null,
  lastRaceResult: null,
  // ID da corrida do pós-corrida (race_id) — usado para reconstruir o timeline de clima.
  lastRaceId: null,
  // Avaliação de carreira do pós-corrida (expectativa vs resultado, nota, frases).
  // Null para corridas sem avaliação — a tela trata e nunca quebra.
  lastRaceEvaluation: null,
  // Análise de telemetria (ritmo/consistência/rival). Null se não houve.
  lastRaceTelemetry: null,
  // Fatura de manutenção do carro do pós-corrida (gasolina/pneus + conserto). Null se não houve.
  lastRaceMaintenance: null,
  otherCategoriesResult: null,
  showResult: false,
  showRaceBriefing: false,
  // A corrida recém-terminada era o final de campeonato (thematic_slot final)?
  // Usado pelo Dashboard para forçar a aba "Notícias" no pós-corrida.
  lastRaceWasFinale: false,
  // A tela de resultado aberta é de uma corrida ACABADA AGORA (true) ou é uma
  // reabertura de corrida antiga pela Home (false)? Só a fresca aciona a lógica
  // de aba pós-corrida.
  resultIsFresh: false,
  // O resultado na tela veio de uma corrida DIRIGIDA no iRacing (true) ou de uma
  // simulação/reabertura (false)? Marcado explicitamente por quem abre a tela: dá
  // pra tentar inferir pela telemetria, mas ela pode vir vazia numa corrida real.
  lastRaceFromIracing: false,
  // Conserto do carro a mostrar no pop-up ao abrir o resultado (import do iRacing).
  iracingRepair: null,
  // Trava p/ o poller do iRacing não importar a mesma corrida duas vezes em voo.
  iracingImporting: false,
  preseasonState: null,
  preseasonWeeks: [],
  lastMarketWeekResult: null,
  playerProposals: [],
  transferWindow: null,
  preseasonFreeAgents: [],
  poachOffer: null,
  endOfSeasonResult: null,
  showEndOfSeason: false,
  showPreseason: false,
  convocationResult: null,
  showConvocation: false,
  specialWindowState: null,
  playerSpecialOffers: [],
  acceptedSpecialOffer: null,
  // Estado preservado para o futuro overlay de campeão. O componente não é
  // montado nesta versão enquanto os dados reais do backend não existirem.
  championOverlay: null,
};

export default initialState;
