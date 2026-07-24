//! Camada de **comportamento por corrida** do export iRacing. **Lógica pura.**
//!
//! Cada corrida o piloto chega com uma "atitude do dia": os atributos secundários
//! (agressividade/otimismo/suavidade) variam MUITO conforme o contexto, mas o PACE
//! (driverSkill) quase não se move (±2, ±4 pra quem é muito afetado) — o pace é a
//! identidade. SÓ no export (a sim offline usa pace_delta + error_mult).
//!
//! Modelo: `atributo_final = base + Σ(sinais) × maleabilidade(mentalidade)`, clamp
//! 0–100. Sem teto artificial — a BASE é a inclinação, os sinais somam a partir dela;
//! um stack de sinais cautelosos vira a mão até do mais agressivo.
//!
//! A **mentalidade** age de DUAS formas (ambas contínuas — ninguém é 0 nem 100):
//! - GANHO: forte = estável (ganho baixo), fraca = volátil (ganho alto).
//! - COMPOSTURA: reduz o IMPACTO dos sinais ADVERSOS (choke, má fase, pista nova,
//!   medo de chuva, status baixo) por corrida, de forma GRADUAL — quanto mais forte,
//!   menor o impacto médio, com variação do dia (de um dia que blinda quase tudo a um
//!   dia ruim que sente quase tudo). Mental baixo leva o adverso quase cheio sempre.
//!   Sinais favoráveis e traços (idade, casa, domínio, calor, humor do dia) sempre valem.
//!
//! Tier 1 (dado já pronto). Tier 2/3 entram depois como +1 função somando aqui.
//!
//! ```text
//! tipos          ← Nudge, Signal, BehaviorInputs, BehaviorOutput
//! mentalidade    ← maleabilidade, compostura (blindagem do adverso), splitmix
//! sinais_tier1   ← pressão (título/casa cheia), forma, pista, clima, idade, status…
//! sinais_tier2   ← sequências, fadiga, contrato, companheiro, categoria, moral…
//! sinais_tier3   ← lua de mel, vingança, nêmesis, campeão, estreia, fama, vínculo…
//! composicao     ← compose() + compute(), a entrada única do export
//! ```

mod composicao;
mod mentalidade;
mod sinais_tier1;
mod sinais_tier2;
mod sinais_tier3;
mod tipos;

pub use composicao::*;
pub use mentalidade::*;
pub use sinais_tier1::*;
pub use sinais_tier2::*;
pub use sinais_tier3::*;
pub use tipos::*;

#[cfg(test)]
mod tests;
