// FUTURO: Orquestrador de alto nível da evolução de pilotos
//
// Este arquivo está reservado para uma camada acima de pipeline.rs,
// caso a lógica de fim de temporada cresça o suficiente para justificar separação.
//
// O que deve vir aqui futuramente:
// - Coordenação entre evolução individual (growth/decline) e dinâmicas de grupo
//   (ex: rivalidades afetando motivação, hierarquia afetando crescimento)
// - Lógica de "arco de carreira" por piloto: jovem promissor → auge → declínio → aposentadoria
// - Eventos especiais de evolução: lesão que muda trajetória, mudança de equipe que
//   acelera ou freia desenvolvimento
// - Evolução de equipes (car_performance, facilities, engineering) ao longo das temporadas
// - Interface unificada para módulos externos chamarem evolução sem conhecer internals
//
// Hoje: a orquestração da virada está em evolution/pipeline/orquestracao.rs — função
// run_end_of_season(). O evolution/pipeline.rs é só o índice dos submódulos, e a ORDEM
// dos passos é travada pelo guard evolution/pipeline/tests/ordem.rs, que lê aquele fonte.
// Enquanto orquestracao.rs for suficiente, este arquivo permanece reservado.
//
// Referência: evolution/pipeline/orquestracao.rs — run_end_of_season() é o ponto de
// entrada atual
