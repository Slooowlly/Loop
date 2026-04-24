// FUTURO: Sistema de experiência por circuito e condição
//
// Este arquivo está reservado para uma lógica mais granular de experiência,
// além dos contadores simples de temporadas/corridas já existentes no Driver.
//
// O que deve vir aqui futuramente:
// - Histórico por pista: piloto que já correu em Daytona tem bônus de adaptação
//   (hoje existe historico_circuitos como JSON no Driver, mas sem lógica associada)
// - Bônus de familiaridade: corridas repetidas na mesma pista aumentam ritmo_classificacao
//   e reduzem variância de resultado
// - Experiência em condições: piloto com muitas corridas na chuva ganha fator_chuva
// - Curva de aprendizado por categoria: primeiras temporadas numa categoria nova
//   têm penalidade decrescente
// - Experiência como fator no decline: pilotos experientes declinam mais lentamente
//
// Hoje: experiência é tratada como contador simples em pipeline.rs:
//   driver.temporadas_na_categoria += 1
//   driver.corridas_na_categoria += standing.stats.corridas
//
// Referência: models/driver.rs — campos historico_circuitos, temporadas_na_categoria,
//             corridas_na_categoria, corridas_na_categoria
