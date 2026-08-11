pub mod ai_post_race;
pub mod ai_pre_race;
pub mod ai_story;
pub mod ai_world_notes;
pub mod calendar;
pub mod contracts;
pub mod drivers;
pub mod favorites;
pub mod injuries;
pub mod licenses;
pub mod market_proposals;
pub mod meta;
pub mod milestones;
pub mod news;
pub mod player_nemesis;
pub mod race_breakdowns;
pub mod race_history;
pub mod races;
pub mod rivalries;
pub mod rivalry_episodes;
pub mod seasons;
pub mod special_team_entries;
pub mod standings;
pub mod team_car;
pub mod team_rivalries;
pub mod teams;
pub mod track_history;

/// Testes dos módulos que nasceram fora do padrão do diretório (caches de IA, nêmesis).
#[cfg(test)]
mod tests_caches_de_ia;

/// Guard das projeções nomeadas que substituíram o `SELECT *`.
#[cfg(test)]
mod tests_projecoes;

/// Testes dos dois módulos que fechavam a lista dos que não tinham nenhum.
#[cfg(test)]
mod tests_rivalidade_e_bloco_especial;
