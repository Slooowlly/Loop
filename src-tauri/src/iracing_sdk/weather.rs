//! Evolução do clima na carreira → keyframes da timeline dinâmica do iRacing.
//! **Lógica pura, testável.** Duas peças:
//!
//! 1. [`rain_skill_penalty`] — quanto de skill a IA PERDE numa corrida molhada.
//!    Insight do design (user): na chuva NÃO se sobe a IA, BAIXA-SE o pelotão todo
//!    (chuva no iRacing é punitiva; subir a IA faria o humano forçar/rodar e ter
//!    medo da chuva). Quem é bom na chuva (`fator_chuva` alto) perde menos. Como o
//!    skill da IA é FIXO na corrida, uma corrida molhada fica molhada o tempo todo
//!    (senão o trecho seco entregaria que baixamos o nível) — mas a INTENSIDADE
//!    pode variar (forte→afrouxa→forte) pra dar dinamismo.
//!
//! 2. [`generate_weather`] — gerador da "história do clima" do fim de semana por
//!    pista+estação, com a exceção roteirizada da 1ª corrida de todo save.
//!
//! Organização dos submódulos:
//! - [`penalidade`] — [`RainIntensity`] e a curva da penalidade de skill.
//! - [`historia`] — estação/tendência, sorteio do cenário e [`WeatherStory`].
//! - [`keyframes`] — cenário → timeline do iRacing e timeline em frações da UI.
//! - [`horario`] — hora de largada (golden hour, noite, sem meio-dia).
//! - [`ambiente`] — temperatura e vento derivados da mesma história.

mod ambiente;
mod historia;
mod horario;
mod keyframes;
mod penalidade;

pub use ambiente::*;
pub use historia::*;
pub use horario::*;
pub use keyframes::*;
pub use penalidade::*;

#[cfg(test)]
mod tests;
