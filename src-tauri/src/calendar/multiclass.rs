// FUTURO: Suporte a corridas multiclasse
//
// Este arquivo está reservado para a lógica de corridas onde múltiplas
// categorias correm simultaneamente na mesma pista (ex: GT3 + GT4).
//
// O que deve vir aqui futuramente:
// - Struct MulticlassRace agrupando entradas de calendário de categorias diferentes
// - Resolução de grid combinado (intercalamento por performance relativa entre classes)
// - Resultado multiclasse: vitória por classe + vitória geral
// - Interações entre classes durante a corrida (overtakes, tráfego, bandeiras)
// - Calendário que marca quais rodadas são multiclasse
//
// Hoje: categorias que compartilham pista têm conflito prevenido via has_calendar_conflict()
// em constants/categories.rs, mas correm em entradas separadas sem interação.
//
// Referência: calendar/mod.rs — has_calendar_conflict, generate_all_calendars_with_id_factory
