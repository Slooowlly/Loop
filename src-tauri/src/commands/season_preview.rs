//! "O Que Esperar" — matéria de expectativas de PRÉ-TEMPORADA.
//!
//! Design completo em `docs/season-preview-design.md`. Resumo do que este módulo faz:
//!
//! * Traduz **número em qualidade**: nenhum skill/nível/salário cru sai daqui. Cada sinal
//!   vira um *token* qualitativo, relativo ao grid (percentil).
//! * Aplica **assimetria de informação**: o jornalista enxerga o que é PÚBLICO (resultados,
//!   fama, estilo de pilotagem) e apenas *intui* o que é OCULTO (ritmo/skill). Por isso a
//!   ordem dos favoritos é a **percepção pública**, não o ranking de skill — quem já tem
//!   pódio pesa mais que um talento oculto ainda não provado.
//! * Descreve **traços de estilo** a partir dos atributos que o iRacing consome
//!   (`aggression`, `smoothness`, `confianca`), que são observáveis em pista.
//! * Levanta as **relações do grid** (quem já correu com quem) — do grid inteiro, sem
//!   privilegiar o jogador.
//!
//! O texto final vem do servidor (`/season-preview`). Se ele falhar, um **montador
//! determinístico** produz a matéria a partir dos MESMOS tokens, respeitando as mesmas
//! regras editoriais (3ª pessoa, sem números) — a aba nunca quebra o personagem.

use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use serde::Serialize;
use tauri::Manager;

use crate::config::app_config::AppConfig;
use crate::db::connection::Database;
use crate::db::queries::ai_story;
use crate::models::driver::Driver;
use crate::narrative::client::{self, SeasonPreview, StoryError};

#[path = "season_preview/comando.rs"]
mod comando;
#[path = "season_preview/comum.rs"]
mod comum;
#[path = "season_preview/contexto.rs"]
mod contexto;
#[path = "season_preview/dossie.rs"]
mod dossie;
#[path = "season_preview/fallback.rs"]
mod fallback;
#[path = "season_preview/fatos.rs"]
mod fatos;
#[path = "season_preview/relacoes.rs"]
mod relacoes;

// Só o comando é público (registrado em lib.rs); o resto circula entre os irmãos.
pub use comando::*;
// Exceção: a percepção pública também ordena a torre do overlay antes de haver tempo,
// pra "expectativa" ser UMA só entre a matéria de pré-temporada e o que aparece na pista.
pub(crate) use comum::perception_score;
use comum::*;
use contexto::*;
use dossie::*;
use fallback::*;
use fatos::*;
use relacoes::*;

// ── Curadoria ────────────────────────────────────────────────────────────────────
/// Quantos nomes entram em FAVORITOS (o topo da percepção pública).
const FAVORITES_COUNT: usize = 5;
/// Quantos entram em PROMESSAS / INCÓGNITAS (o segundo pelotão).
const PROMISES_COUNT: usize = 5;
/// Piso de pilotos com dossiê no bundle. A matéria de IA fica rasa quando só há meia
/// dúzia de nomes para trabalhar — com o grid inteiro pequeno, cobre todo mundo.
const MIN_PROFILED: usize = 10;
/// Quantos nomes o FALLBACK cita no pelotão de trás. O bundle da IA pode ser generoso
/// (é insumo), mas a prosa determinística vira lista se despejar dez nomes.
const FB_PACK_COUNT: usize = 3;
/// Teto de relações do grid citadas (§5.5) — mais que isso vira lista.
const MAX_RELATIONS: usize = 4;
/// Percentis que definem um traço de estilo "marcante" (fora disso, o piloto é mediano
/// naquele eixo e não ganha traço — evita ficha técnica).
const TRAIT_HIGH_PCT: f64 = 0.85;
const TRAIT_LOW_PCT: f64 = 0.15;
/// Máximo de traços por piloto (dá cor sem virar planilha).
const MAX_TRAITS: usize = 2;
/// Intensidade percebida mínima para uma rivalidade virar notícia.
const RIVALRY_MIN_INTENSITY: f64 = 35.0;
/// Fama (escala de exibição) a partir da qual o piloto é "nome de público".
const STAR_MIN_FAMA: f64 = 71.0;

// ── Pesos da PERCEPÇÃO pública (§5.2) ────────────────────────────────────────────
// Resultado ≫ reputação > experiência, e o skill entra só como um empurrão fraco.
// É de propósito: a imprensa não conhece o ritmo real, ela lê o que já aconteceu.
const W_TITLE: f64 = 100.0;
const W_WIN: f64 = 40.0;
const W_PODIUM: f64 = 18.0;
const W_FAME: f64 = 0.35;
const W_CHARISMA: f64 = 0.15;
const W_EXPERIENCE: f64 = 0.8;
/// Experiência satura: a partir daqui, mais corridas não mudam a percepção.
const EXPERIENCE_CAP: f64 = 40.0;
/// O "vazamento" do skill oculto na percepção. Fraco de propósito.
const W_SKILL_HINT: f64 = 0.25;
/// Amplitude do ruído determinístico (±metade), para a percepção não virar um espelho
/// exato do skill quando ninguém tem resultado (arranque de temporada).
const JITTER_RANGE: f64 = 6.0;

#[cfg(test)]
#[path = "season_preview/tests/mod.rs"]
mod tests;
