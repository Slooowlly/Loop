//! Motor puro do leilão da Janela de Transferências (Fase 1, IA-only, sem DB).
//!
//! Roda o ciclo semanal de dois lados: OFERTAS (equipes ofertam à shortlist, sobem
//! o lance se perderam o alvo) → RESPOSTAS (piloto aceita só a melhor acima do
//! limiar) → RESULTADOS (equipe assina o nº1 entre os que aceitaram; resto = "resto")
//! → ROLLOVER. Fecha quando uma semana passa sem assinatura (ou teto duro).
//!
//! Pesos e regras seguem o design `docs/superpowers/specs/2026-06-20-weekly-transfer-market-design.md`.
//! O wiring (ler vagas/pilotos do DB, prestígio do archive, garantias do pódio e
//! entourage do jogador) é uma camada de cima — aqui é só o leilão puro.

#![allow(dead_code)] // wiring vem na próxima camada

use std::collections::HashMap;

use rand::Rng;
use serde::{Deserialize, Serialize};

/// Uma vaga aberta (assento de uma equipe).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Seat {
    pub id: String,
    pub team_id: String,
    pub category: String,
    pub class: Option<String>,
    pub tier: u8,
    pub is_n1: bool,
    pub car_norm: f64,        // car_performance normalizado 0-100(+)
    pub prestige: f64,        // 0-100 (competitividade dos últimos 10 anos)
    pub required_license: u8, // licença mínima exigida pela categoria/classe
    pub salary_floor: f64,
    pub salary_ceiling: f64, // máximo que o orçamento comporta
}

/// Um piloto disponível (agente livre / "resto").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub id: String,
    pub skill: f64,
    pub tier: u8,                    // tier de skill (onde ele compete bem)
    pub brand: Option<String>,       // "mazda"/"toyota" nos tiers 0-1; None acima
    pub slam_target: Option<String>, // categoria-alvo do slam (bônus)
    pub max_license: u8,             // licença máxima do piloto (limita as categorias acessíveis)
    pub market_value: f64,
    pub ai_respects_brand: bool, // IA respeita a marca; jogador (false) tem liberdade
    #[serde(default)]
    pub category: String, // categoria de ORIGEM (onde correu por último) — p/ promovido/rebaixado
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signing {
    pub seat_id: String,
    pub team_id: String,
    pub driver_id: String,
    pub category: String,
    pub class: Option<String>,
    pub salary: f64,
    pub week: u32,
    #[serde(default)]
    pub from_category: Option<String>, // categoria de origem (None = estreia/desconhecida)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowResult {
    pub signings: Vec<Signing>,
    pub unsigned: Vec<Candidate>, // sobraram sem vaga (pós rede de segurança)
    pub weeks: u32,
}

/// Parâmetros calibrados (design §11).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowConfig {
    pub bid_gap_close: f64,    // 0.30 — fecha 30% da folga até o teto/semana
    pub shortlist_top: usize,  // 3 (tiers >= low_tier_cap+1)
    pub shortlist_low: usize,  // 2 (tiers <= low_tier_cap)
    pub low_tier_cap: u8,      // 2
    pub accept_threshold: f64, // 50.0 (semana 1)
    pub threshold_decay: f64,  // 4.0/semana — o piloto fica menos exigente
    pub threshold_floor: f64,  // 35.0 — mínimo
    pub dignity_tier_gap: u8,  // 2 — recusa cair 2+ tiers abaixo do seu nível
    pub craque_skill: f64,     // 80.0 — sempre acha vaga; pode descer (slam)
    pub hard_week_cap: u32,    // 10
    // pesos do score do piloto — TIER É RELATIVO (subir/lateral/descer)
    pub tier_base: f64,  // 12 — lateral (mesmo tier)
    pub tier_step: f64,  // 7 — por tier de diferença (sobe soma, desce subtrai)
    pub w_prestige: f64, // 22
    pub w_car: f64,      // 18
    pub w_salary: f64,   // 15
    pub w_role_n1: f64,  // 15
    pub w_role_n2: f64,  // 10
    pub slam_bonus: f64, // 18 — categoria-alvo
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            bid_gap_close: 0.30,
            shortlist_top: 3,
            shortlist_low: 2,
            low_tier_cap: 2,
            accept_threshold: 50.0,
            threshold_decay: 4.0,
            threshold_floor: 28.0,
            dignity_tier_gap: 2,
            craque_skill: 80.0,
            hard_week_cap: 10,
            tier_base: 12.0,
            tier_step: 7.0,
            w_prestige: 22.0,
            w_car: 18.0,
            w_salary: 15.0,
            w_role_n1: 15.0,
            w_role_n2: 10.0,
            slam_bonus: 18.0,
        }
    }
}

const MAX_TIER: f64 = 6.0;
const SALARY_REF: f64 = 300_000.0;

/// Score do PILOTO sobre uma oferta (qual ele aceita). Confia mais no histórico
/// (prestígio) que na promessa do carro.
fn driver_offer_score(cfg: &WindowConfig, seat: &Seat, cand: &Candidate, salary: f64) -> f64 {
    // Tier RELATIVO: subir soma, descer subtrai (não é desejabilidade absoluta —
    // senão um piloto de tier baixo nunca aceitaria uma vaga boa do seu tier).
    let tier_delta = seat.tier as f64 - cand.tier as f64;
    let tier_component = (cfg.tier_base + cfg.tier_step * tier_delta).max(0.0);
    let mut score = tier_component
        + (seat.prestige / 100.0) * cfg.w_prestige
        + (seat.car_norm / 100.0).min(1.2) * cfg.w_car
        + (salary / SALARY_REF).min(1.0) * cfg.w_salary
        + if seat.is_n1 {
            cfg.w_role_n1
        } else {
            cfg.w_role_n2
        };
    if cand.slam_target.as_deref() == Some(seat.category.as_str()) {
        score += cfg.slam_bonus;
    }
    score
}

/// O piloto recusa cair 2+ tiers abaixo do seu nível, por qualquer salário (piso
/// de dignidade). Subir/lateral/descer 1 tier é permitido.
fn passes_dignity(cfg: &WindowConfig, seat: &Seat, cand: &Candidate) -> bool {
    cand.tier <= seat.tier + cfg.dignity_tier_gap.saturating_sub(1)
        || cand.tier.saturating_sub(seat.tier) < cfg.dignity_tier_gap
}

/// Score da EQUIPE sobre um candidato (quem entra na shortlist / quem ela assina).
/// Mérito esportivo: skill manda.
fn team_candidate_score(cand: &Candidate) -> f64 {
    cand.skill
}

/// Elegibilidade básica de um candidato pra uma vaga: tier dentro de ±1, OU craque
/// (pode descer pra caçar slam / a equipe abre exceção pelo super-qualificado).
fn eligible(cfg: &WindowConfig, seat: &Seat, cand: &Candidate) -> bool {
    // Licença é obrigatória — sem ela o piloto não pode entrar na categoria/classe.
    if cand.max_license < seat.required_license {
        return false;
    }
    let near = (cand.tier as i16 - seat.tier as i16).abs() <= 1;
    near || cand.skill >= cfg.craque_skill
}

/// Regra de marca (IA, tiers 0-1): se o piloto-IA recebeu alguma oferta da MESMA
/// marca, ele ignora as cross-brand. Jogador (ai_respects_brand=false) não filtra.
fn filter_brand<'a>(
    cand: &Candidate,
    offers: &'a [(usize, f64)],
    seats: &[Seat],
) -> Vec<(usize, f64)> {
    let Some(brand) = cand.brand.as_deref() else {
        return offers.to_vec();
    };
    if !cand.ai_respects_brand {
        return offers.to_vec();
    }
    let same_brand: Vec<(usize, f64)> = offers
        .iter()
        .copied()
        .filter(|(si, _)| seat_brand(&seats[*si].category) == Some(brand))
        .collect();
    if same_brand.is_empty() {
        offers.to_vec()
    } else {
        same_brand
    }
}

/// Marca derivada do id da categoria (só tiers 0-1).
fn seat_brand(category: &str) -> Option<&'static str> {
    if category.starts_with("mazda_") {
        Some("mazda")
    } else if category.starts_with("toyota_") {
        Some("toyota")
    } else {
        None
    }
}

fn shortlist_size(cfg: &WindowConfig, tier: u8) -> usize {
    if tier <= cfg.low_tier_cap {
        cfg.shortlist_low
    } else {
        cfg.shortlist_top
    }
}

fn salkey(seat: &str, driver: &str) -> String {
    format!("{seat}\u{1}{driver}")
}

/// OFERTAS: cada vaga monta a shortlist e oferta (escalando o lance). Devolve
/// `driver_index -> [(seat_index, salário)]` e atualiza `current_salary`.
fn compute_offers(
    open: &[Seat],
    free: &[Candidate],
    current_salary: &mut HashMap<String, f64>,
    cfg: &WindowConfig,
) -> HashMap<usize, Vec<(usize, f64)>> {
    let mut offers_by_driver: HashMap<usize, Vec<(usize, f64)>> = HashMap::new();
    for (si, seat) in open.iter().enumerate() {
        let mut ranked: Vec<usize> = (0..free.len())
            .filter(|&ci| eligible(cfg, seat, &free[ci]))
            .collect();
        ranked.sort_by(|&a, &b| {
            team_candidate_score(&free[b])
                .total_cmp(&team_candidate_score(&free[a]))
                .then_with(|| free[a].id.cmp(&free[b].id))
        });
        for &ci in ranked.iter().take(shortlist_size(cfg, seat.tier)) {
            let key = salkey(&seat.id, &free[ci].id);
            let salary = match current_salary.get(&key) {
                Some(&prev) => (prev + cfg.bid_gap_close * (seat.salary_ceiling - prev))
                    .min(seat.salary_ceiling),
                None => free[ci]
                    .market_value
                    .clamp(seat.salary_floor, seat.salary_ceiling),
            };
            current_salary.insert(key, salary);
            offers_by_driver.entry(ci).or_default().push((si, salary));
        }
    }

    // ── GARANTIA DE OFERTAS AO JOGADOR ──────────────────────────────────────────
    // O jogador (único com ai_respects_brand=false) é rankeado por skill como todos;
    // um piloto fraco (rookie) nunca entraria na shortlist e ficaria SEM nenhuma
    // oferta. Garante ofertas das vagas do NÍVEL DELE (próprio tier), as de maior
    // prestígio primeiro, até um teto por semana — ele recebe das duas marcas.
    if let Some(pi) = free.iter().position(|c| !c.ai_respects_brand) {
        let cap = cfg.shortlist_top as usize + 1;
        let mut count = offers_by_driver.get(&pi).map_or(0, Vec::len);
        let mut tier_seats: Vec<usize> = (0..open.len())
            .filter(|&si| {
                open[si].tier == free[pi].tier
                    && eligible(cfg, &open[si], &free[pi])
                    && passes_dignity(cfg, &open[si], &free[pi])
            })
            .collect();
        tier_seats.sort_by(|&a, &b| open[b].prestige.total_cmp(&open[a].prestige));
        for si in tier_seats {
            if count >= cap {
                break;
            }
            let already = offers_by_driver
                .get(&pi)
                .is_some_and(|v| v.iter().any(|&(s, _)| s == si));
            if already {
                continue;
            }
            let key = salkey(&open[si].id, &free[pi].id);
            let salary = match current_salary.get(&key) {
                Some(&prev) => (prev + cfg.bid_gap_close * (open[si].salary_ceiling - prev))
                    .min(open[si].salary_ceiling),
                None => free[pi]
                    .market_value
                    .clamp(open[si].salary_floor, open[si].salary_ceiling),
            };
            current_salary.insert(key, salary);
            offers_by_driver.entry(pi).or_default().push((si, salary));
            count += 1;
        }
    }

    offers_by_driver
}

/// RESPOSTAS + RESULTADOS de uma semana: cada piloto aceita sua melhor oferta (o
/// JOGADOR usa `player_choice`); cada vaga assina o nº1 entre os que aceitaram.
/// Mutaciona open/free/signings. Devolve se houve alguma assinatura.
#[allow(clippy::too_many_arguments)]
fn resolve_week(
    open: &mut Vec<Seat>,
    free: &mut Vec<Candidate>,
    signings: &mut Vec<Signing>,
    offers_by_driver: &HashMap<usize, Vec<(usize, f64)>>,
    week: u32,
    threshold: f64,
    cfg: &WindowConfig,
    player_id: Option<&str>,
    player_choice: Option<&str>,
) -> bool {
    let mut accepted_by_seat: HashMap<usize, Vec<(usize, f64)>> = HashMap::new();
    for (&ci, raw_offers) in offers_by_driver.iter() {
        let chosen = if player_id == Some(free[ci].id.as_str()) {
            // JOGADOR: aceita a vaga que escolheu (por id), se estiver entre as ofertas;
            // senão (None) espera (não aceita nada nesta semana).
            player_choice.and_then(|seat_id| {
                raw_offers
                    .iter()
                    .copied()
                    .find(|&(si, _)| open[si].id == seat_id)
            })
        } else {
            let offers = filter_brand(&free[ci], raw_offers, open);
            offers
                .iter()
                .copied()
                .filter(|&(si, salary)| {
                    passes_dignity(cfg, &open[si], &free[ci])
                        && driver_offer_score(cfg, &open[si], &free[ci], salary) >= threshold
                })
                .max_by(|&(sa, salary_a), &(sb, salary_b)| {
                    driver_offer_score(cfg, &open[sa], &free[ci], salary_a)
                        .total_cmp(&driver_offer_score(cfg, &open[sb], &free[ci], salary_b))
                })
        };
        if let Some((si, salary)) = chosen {
            accepted_by_seat.entry(si).or_default().push((ci, salary));
        }
    }

    let mut signed_seat_indices: Vec<usize> = Vec::new();
    let mut signed_driver_indices: Vec<usize> = Vec::new();
    let mut any_signed = false;
    let mut seat_order: Vec<usize> = (0..open.len()).collect();
    seat_order.sort_by(|&a, &b| open[b].prestige.total_cmp(&open[a].prestige));
    for si in seat_order {
        if signed_seat_indices.contains(&si) {
            continue;
        }
        let Some(accepters) = accepted_by_seat.get(&si) else {
            continue;
        };
        let available: Vec<(usize, f64)> = accepters
            .iter()
            .copied()
            .filter(|&(ci, _)| !signed_driver_indices.contains(&ci))
            .collect();
        // A aceitação do JOGADOR é honrada: o time que o ofertou assina ELE (não um
        // IA mais forte que também aceitou) — senão um rookie nunca venceria a vaga.
        let pick = available
            .iter()
            .copied()
            .find(|&(ci, _)| !free[ci].ai_respects_brand)
            .or_else(|| {
                available.iter().copied().max_by(|&(ca, _), &(cb, _)| {
                    team_candidate_score(&free[ca]).total_cmp(&team_candidate_score(&free[cb]))
                })
            });
        if let Some((ci, salary)) = pick {
            let seat = &open[si];
            signings.push(Signing {
                seat_id: seat.id.clone(),
                team_id: seat.team_id.clone(),
                driver_id: free[ci].id.clone(),
                category: seat.category.clone(),
                class: seat.class.clone(),
                salary,
                week,
                from_category: (!free[ci].category.is_empty())
                    .then(|| free[ci].category.clone()),
            });
            signed_seat_indices.push(si);
            signed_driver_indices.push(ci);
            any_signed = true;
        }
    }
    signed_seat_indices.sort_unstable_by(|a, b| b.cmp(a));
    for si in signed_seat_indices {
        open.remove(si);
    }
    signed_driver_indices.sort_unstable_by(|a, b| b.cmp(a));
    for ci in signed_driver_indices {
        free.remove(ci);
    }
    any_signed
}

/// Passe de fechamento: cada vaga pega o melhor piloto elegível+dignidade que
/// sobrou (o mercado "esvazia"). Não força ninguém 2+ tiers abaixo.
fn clearing_pass(
    open: &mut Vec<Seat>,
    free: &mut Vec<Candidate>,
    signings: &mut Vec<Signing>,
    cfg: &WindowConfig,
    week: u32,
) {
    loop {
        let mut best_pair: Option<(usize, usize)> = None;
        let mut best_prestige = f64::NEG_INFINITY;
        for si in 0..open.len() {
            let pick = (0..free.len())
                .filter(|&ci| {
                    eligible(cfg, &open[si], &free[ci]) && passes_dignity(cfg, &open[si], &free[ci])
                })
                .max_by(|&a, &b| {
                    team_candidate_score(&free[a]).total_cmp(&team_candidate_score(&free[b]))
                });
            if let Some(ci) = pick {
                if open[si].prestige > best_prestige {
                    best_prestige = open[si].prestige;
                    best_pair = Some((si, ci));
                }
            }
        }
        let Some((si, ci)) = best_pair else { break };
        let seat = open.remove(si);
        let salary = free[ci]
            .market_value
            .clamp(seat.salary_floor, seat.salary_ceiling);
        signings.push(Signing {
            seat_id: seat.id.clone(),
            team_id: seat.team_id.clone(),
            driver_id: free[ci].id.clone(),
            category: seat.category.clone(),
            class: seat.class.clone(),
            salary,
            week,
            from_category: (!free[ci].category.is_empty()).then(|| free[ci].category.clone()),
        });
        free.remove(ci);
        if open.is_empty() || free.is_empty() {
            break;
        }
    }
}

/// Rede de segurança por skill: craque (skill ≥ craque_skill) sempre acha vaga (a
/// melhor que sobrar), IGNORANDO dignidade — backstop final.
fn safety_net(
    open: &mut Vec<Seat>,
    free: &mut Vec<Candidate>,
    signings: &mut Vec<Signing>,
    cfg: &WindowConfig,
    week: u32,
) {
    let mut craques: Vec<usize> = (0..free.len())
        .filter(|&ci| free[ci].skill >= cfg.craque_skill)
        .collect();
    craques.sort_by(|&a, &b| free[b].skill.total_cmp(&free[a].skill));
    for ci in craques {
        if open.is_empty() {
            break;
        }
        let best_si = open
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.prestige.total_cmp(&b.prestige))
            .map(|(i, _)| i)
            .unwrap();
        let seat = open.remove(best_si);
        let salary = free[ci]
            .market_value
            .clamp(seat.salary_floor, seat.salary_ceiling);
        signings.push(Signing {
            seat_id: seat.id.clone(),
            team_id: seat.team_id.clone(),
            driver_id: free[ci].id.clone(),
            category: seat.category.clone(),
            class: seat.class.clone(),
            salary,
            week,
            from_category: (!free[ci].category.is_empty()).then(|| free[ci].category.clone()),
        });
    }
    let signed_ids: std::collections::HashSet<&str> =
        signings.iter().map(|s| s.driver_id.as_str()).collect();
    free.retain(|c| !signed_ids.contains(c.id.as_str()));
}

/// Oferta apresentada ao JOGADOR (uma das que ele recebeu na semana).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerOffer {
    pub seat_id: String,
    pub team_id: String,
    pub category: String,
    pub class: Option<String>,
    pub salary: f64,
    pub is_n1: bool,
}

/// Estado SERIALIZÁVEL e STEPÁVEL da janela — permite rodar semana a semana com a
/// resposta do jogador entre os passos (persistir entre comandos). A Fase 1
/// (IA-only) usa via `run_window`; a Fase 2 (interativa) avança manualmente.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowState {
    open: Vec<Seat>,
    free: Vec<Candidate>,
    signings: Vec<Signing>,
    current_salary: HashMap<String, f64>,
    week: u32,
    consecutive_no_sign: u32,
    closed: bool,
    cfg: WindowConfig,
    player_id: Option<String>,
    /// Ofertas da semana atual já computadas, aguardando resolução (forma
    /// serializável de `offers_by_driver`: [(driver_idx, [(seat_idx, salário)])]).
    pending: Option<Vec<(usize, Vec<(usize, f64)>)>>,
    /// Quantas assinaturas já foram aplicadas no banco (cursor incremental do
    /// wiring): o feed e o banco ficam em sincronia semana a semana.
    #[serde(default)]
    applied_to_db: usize,
}

impl WindowState {
    pub fn new(
        seats: Vec<Seat>,
        candidates: Vec<Candidate>,
        cfg: WindowConfig,
        player_id: Option<String>,
    ) -> Self {
        Self {
            open: seats,
            free: candidates,
            signings: Vec::new(),
            current_salary: HashMap::new(),
            week: 0,
            consecutive_no_sign: 0,
            closed: false,
            cfg,
            player_id,
            pending: None,
            applied_to_db: 0,
        }
    }

    /// Assinaturas ainda NÃO aplicadas no banco (desde o último `mark_applied`).
    pub fn unapplied_signings(&self) -> &[Signing] {
        &self.signings[self.applied_to_db.min(self.signings.len())..]
    }

    /// Marca todas as assinaturas atuais como aplicadas no banco.
    pub fn mark_applied(&mut self) {
        self.applied_to_db = self.signings.len();
    }

    /// Inicia a janela e prepara a semana 1 (ofertas prontas pra mostrar).
    pub fn start(
        seats: Vec<Seat>,
        candidates: Vec<Candidate>,
        cfg: WindowConfig,
        player_id: Option<String>,
    ) -> Self {
        let mut state = Self::new(seats, candidates, cfg, player_id);
        state.prepare();
        state
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }
    pub fn week(&self) -> u32 {
        self.week
    }
    pub fn signings(&self) -> &[Signing] {
        &self.signings
    }

    fn threshold(&self) -> f64 {
        (self.cfg.accept_threshold
            - self.cfg.threshold_decay * (self.week.saturating_sub(1)) as f64)
            .max(self.cfg.threshold_floor)
    }

    /// Computa as ofertas da próxima semana (não resolve). Idempotente se já há
    /// `pending`. No-op se fechada.
    fn prepare(&mut self) {
        if self.closed || self.pending.is_some() {
            return;
        }
        self.week += 1;
        let offers = compute_offers(&self.open, &self.free, &mut self.current_salary, &self.cfg);
        self.pending = Some(offers.into_iter().collect());
    }

    /// As ofertas que o JOGADOR recebeu nesta semana (pra a UI mostrar).
    pub fn player_offers(&self) -> Vec<PlayerOffer> {
        let (Some(player_id), Some(pending)) = (self.player_id.as_deref(), self.pending.as_ref())
        else {
            return Vec::new();
        };
        let Some(player_idx) = self.free.iter().position(|c| c.id == player_id) else {
            return Vec::new();
        };
        pending
            .iter()
            .find(|(ci, _)| *ci == player_idx)
            .map(|(_, offers)| {
                offers
                    .iter()
                    .map(|&(si, salary)| {
                        let seat = &self.open[si];
                        PlayerOffer {
                            seat_id: seat.id.clone(),
                            team_id: seat.team_id.clone(),
                            category: seat.category.clone(),
                            class: seat.class.clone(),
                            salary,
                            is_n1: seat.is_n1,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Resolve a semana atual com a escolha do jogador (`Some(seat_id)` aceita, `None`
    /// espera) e prepara a próxima — ou fecha (passe de fechamento + rede de segurança).
    pub fn advance(&mut self, player_choice: Option<&str>) {
        if self.closed {
            return;
        }
        if self.pending.is_none() {
            self.prepare();
        }
        let Some(pending) = self.pending.take() else {
            return;
        };
        let offers_by_driver: HashMap<usize, Vec<(usize, f64)>> = pending.into_iter().collect();
        let threshold = self.threshold();
        let any_signed = resolve_week(
            &mut self.open,
            &mut self.free,
            &mut self.signings,
            &offers_by_driver,
            self.week,
            threshold,
            &self.cfg,
            self.player_id.as_deref(),
            player_choice,
        );
        if any_signed {
            self.consecutive_no_sign = 0;
        } else {
            self.consecutive_no_sign += 1;
        }
        if self.consecutive_no_sign >= 2
            || self.open.is_empty()
            || self.free.is_empty()
            || self.week >= self.cfg.hard_week_cap
        {
            clearing_pass(
                &mut self.open,
                &mut self.free,
                &mut self.signings,
                &self.cfg,
                self.week,
            );
            safety_net(
                &mut self.open,
                &mut self.free,
                &mut self.signings,
                &self.cfg,
                self.week,
            );
            self.closed = true;
        } else {
            self.prepare();
        }
    }

    pub fn into_result(self) -> WindowResult {
        WindowResult {
            signings: self.signings,
            unsigned: self.free,
            weeks: self.week,
        }
    }
}

/// Roda a janela inteira até esvaziar (IA-only — sem jogador). Implementado sobre o
/// `WindowState` stepável, então motor e modo interativo compartilham a mesma lógica.
pub fn run_window(
    seats: Vec<Seat>,
    candidates: Vec<Candidate>,
    cfg: &WindowConfig,
    _rng: &mut impl Rng,
) -> WindowResult {
    let mut state = WindowState::start(seats, candidates, cfg.clone(), None);
    while !state.is_closed() {
        state.advance(None);
    }
    state.into_result()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, SeedableRng};

    fn seat(id: &str, tier: u8, car: f64, prestige: f64, ceiling: f64) -> Seat {
        Seat {
            id: id.to_string(),
            team_id: format!("T-{id}"),
            category: match tier {
                0 => "mazda_rookie",
                1 => "mazda_amador",
                2 => "bmw_m2",
                3 => "gt4",
                4 => "gt3",
                _ => "endurance",
            }
            .to_string(),
            class: None,
            tier,
            is_n1: true,
            car_norm: car,
            prestige,
            required_license: 0,
            salary_floor: 10_000.0,
            salary_ceiling: ceiling,
        }
    }
    fn cand(id: &str, skill: f64, tier: u8) -> Candidate {
        Candidate {
            id: id.to_string(),
            skill,
            tier,
            brand: None,
            slam_target: None,
            max_license: 10,
            market_value: 50_000.0,
            ai_respects_brand: true,
            category: String::new(),
        }
    }
    fn rng() -> StdRng {
        StdRng::seed_from_u64(1)
    }

    #[test]
    fn best_driver_takes_most_prestigious_seat() {
        // 2 vagas tier 4: uma com prestígio/carro alto, outra baixa. 1 craque + 1 mediano.
        let seats = vec![
            seat("A", 4, 90.0, 90.0, 200_000.0), // top
            seat("B", 4, 50.0, 20.0, 120_000.0), // fraca
        ];
        let cands = vec![cand("star", 88.0, 4), cand("mid", 70.0, 4)];
        let res = run_window(seats, cands, &WindowConfig::default(), &mut rng());
        let star = res.signings.iter().find(|s| s.driver_id == "star").unwrap();
        assert_eq!(
            star.seat_id, "A",
            "craque deve pegar a vaga mais prestigiada"
        );
        assert_eq!(res.signings.len(), 2);
    }

    #[test]
    fn bid_escalates_until_accept() {
        // Vaga forte mas com piso baixo; o piloto só aceita quando o lance sobe.
        // market_value alto força a 1ª oferta a já ser decente; checamos que fecha.
        let seats = vec![seat("A", 3, 80.0, 70.0, 300_000.0)];
        let mut c = cand("p", 80.0, 3);
        c.market_value = 20_000.0; // começa baixo → escala
        let res = run_window(seats, vec![c], &WindowConfig::default(), &mut rng());
        assert_eq!(res.signings.len(), 1);
        assert!(res.signings[0].salary >= 20_000.0);
    }

    #[test]
    fn dignity_floor_blocks_deep_drop() {
        let cfg = WindowConfig::default();
        let star = cand("star", 90.0, 4); // nível tier 4
                                          // Recusa cair 2 tiers (pra tier 2 ou abaixo); aceita lateral/1-tier-abaixo (tier 3).
        assert!(!passes_dignity(
            &cfg,
            &seat("t2", 2, 60.0, 60.0, 1.0),
            &star
        ));
        assert!(!passes_dignity(
            &cfg,
            &seat("t1", 1, 60.0, 60.0, 1.0),
            &star
        ));
        assert!(passes_dignity(&cfg, &seat("t3", 3, 60.0, 60.0, 1.0), &star));
        assert!(passes_dignity(&cfg, &seat("t4", 4, 60.0, 60.0, 1.0), &star));
    }

    #[test]
    fn brand_ladder_prefers_same_brand() {
        // Piloto Mazda recebe oferta Mazda Cup e Toyota Cup (mesmo tier/prestígio).
        let mut mazda = seat("M", 1, 70.0, 70.0, 100_000.0);
        mazda.category = "mazda_amador".to_string();
        let mut toyota = seat("T", 1, 70.0, 70.0, 100_000.0);
        toyota.category = "toyota_amador".to_string();
        let mut c = cand("driver", 75.0, 1);
        c.brand = Some("mazda".to_string());
        let res = run_window(
            vec![mazda, toyota],
            vec![c],
            &WindowConfig::default(),
            &mut rng(),
        );
        let sign = &res.signings[0];
        assert_eq!(sign.category, "mazda_amador", "IA respeita a marca");
    }

    #[test]
    fn cross_brand_only_when_no_same_brand() {
        // Só há vaga Toyota; o piloto Mazda aceita (fallback cross-brand).
        let mut toyota = seat("T", 1, 70.0, 70.0, 100_000.0);
        toyota.category = "toyota_amador".to_string();
        let mut c = cand("driver", 75.0, 1);
        c.brand = Some("mazda".to_string());
        let res = run_window(vec![toyota], vec![c], &WindowConfig::default(), &mut rng());
        assert_eq!(res.signings.len(), 1);
        assert_eq!(res.signings[0].category, "toyota_amador");
    }

    #[test]
    fn craque_safety_net_always_signs() {
        // Vaga indigna (tier 0) e um craque tier 4: aceitação normal recusa, mas a
        // rede de segurança garante que o craque assina.
        let seats = vec![seat("rookie", 0, 50.0, 40.0, 80_000.0)];
        let res = run_window(
            seats,
            vec![cand("star", 90.0, 4)],
            &WindowConfig::default(),
            &mut rng(),
        );
        assert_eq!(res.signings.len(), 1, "craque nunca fica desempregado");
        assert!(res.unsigned.is_empty());
    }

    #[test]
    fn weak_driver_may_go_unsigned() {
        // Sem vaga e piloto fraco → fica sem contrato (não há rede pra ele).
        let res = run_window(
            vec![],
            vec![cand("weak", 50.0, 1)],
            &WindowConfig::default(),
            &mut rng(),
        );
        assert!(res.signings.is_empty());
        assert_eq!(res.unsigned.len(), 1);
    }

    #[test]
    #[ignore] // validação em escala: cargo test --lib zzz_scale_validation -- --ignored --nocapture
    fn zzz_scale_validation() {
        use rand::Rng;
        let mut r = StdRng::seed_from_u64(42);
        let cfg = WindowConfig::default();
        let cats = [
            "mazda_rookie",
            "mazda_amador",
            "bmw_m2",
            "gt4",
            "gt3",
            "endurance",
        ];
        let mut seats = Vec::new();
        let mut cands = Vec::new();
        for tier in 0u8..6 {
            for i in 0..6 {
                seats.push(Seat {
                    id: format!("S{tier}-{i}"),
                    team_id: format!("T{tier}-{i}"),
                    category: cats[tier as usize].to_string(),
                    class: None,
                    tier,
                    is_n1: i % 2 == 0,
                    car_norm: r.gen_range(40.0..95.0),
                    prestige: r.gen_range(10.0..95.0),
                    required_license: tier,
                    salary_floor: 8_000.0,
                    salary_ceiling: 30_000.0 + tier as f64 * 40_000.0 + r.gen_range(0.0..60_000.0),
                });
            }
            for i in 0..7 {
                let skill = (40.0 + tier as f64 * 9.0 + r.gen_range(-8.0..12.0)).clamp(25.0, 98.0);
                cands.push(Candidate {
                    id: format!("D{tier}-{i}"),
                    skill,
                    tier,
                    brand: None,
                    slam_target: None,
                    max_license: 10,
                    market_value: 20_000.0 + skill * 1_000.0,
                    ai_respects_brand: true,
                    category: String::new(),
                });
            }
        }
        let (n_seats, n_cands) = (seats.len(), cands.len());
        let craques = cands.iter().filter(|c| c.skill >= cfg.craque_skill).count();
        let res = run_window(seats, cands, &cfg, &mut r);
        let craque_unsigned = res
            .unsigned
            .iter()
            .filter(|c| c.skill >= cfg.craque_skill)
            .count();
        let sals: Vec<f64> = res.signings.iter().map(|s| s.salary).collect();
        let mn = sals.iter().cloned().fold(f64::INFINITY, f64::min);
        let mx = sals.iter().cloned().fold(0.0_f64, f64::max);
        println!("\n=== VALIDAÇÃO EM ESCALA ===");
        println!("vagas {n_seats} | pilotos {n_cands} | craques (skill≥80) {craques}");
        println!(
            "assinaturas {} | sem vaga {} | semanas {}",
            res.signings.len(),
            res.unsigned.len(),
            res.weeks
        );
        println!("craques sem vaga: {craque_unsigned} (deve ser 0)");
        println!("salário assinado: min {mn:.0} | max {mx:.0}");
        let fill_rate = res.signings.len() as f64 / n_seats as f64;
        println!(
            "taxa de preenchimento: {:.0}% (resto vai pra rookies/promoção)",
            fill_rate * 100.0
        );
        assert_eq!(craque_unsigned, 0, "craque nunca fica sem vaga");
        assert!(
            fill_rate >= 0.80,
            "motor deve preencher a maioria das vagas (resto = rookies)"
        );
    }

    #[test]
    fn player_signs_the_seat_he_accepts() {
        // 2 vagas tier 2; o jogador escolhe a vaga B (mesmo sendo a pior).
        let seats = vec![
            seat("A", 2, 80.0, 70.0, 100_000.0),
            seat("B", 2, 60.0, 40.0, 100_000.0),
        ];
        let mut player = cand("PLAYER", 75.0, 2);
        player.ai_respects_brand = false;
        let mut state = WindowState::start(
            seats,
            vec![player],
            WindowConfig::default(),
            Some("PLAYER".to_string()),
        );
        assert!(
            !state.player_offers().is_empty(),
            "jogador recebe ofertas na semana 1"
        );
        state.advance(Some("B")); // aceita a vaga B
        while !state.is_closed() {
            state.advance(None);
        }
        let res = state.into_result();
        let player_sign = res
            .signings
            .iter()
            .find(|s| s.driver_id == "PLAYER")
            .unwrap();
        assert_eq!(
            player_sign.seat_id, "B",
            "jogador assina a vaga que escolheu"
        );
    }

    #[test]
    fn serialized_state_round_trips() {
        // O estado serializa/deserializa (persistência entre comandos da Fase 2).
        let seats = vec![seat("A", 2, 70.0, 60.0, 100_000.0)];
        let mut player = cand("PLAYER", 75.0, 2);
        player.ai_respects_brand = false;
        let state = WindowState::start(
            seats,
            vec![player],
            WindowConfig::default(),
            Some("PLAYER".to_string()),
        );
        let json = serde_json::to_string(&state).expect("serializa");
        let mut restored: WindowState = serde_json::from_str(&json).expect("deserializa");
        assert_eq!(restored.week(), 1);
        assert!(!restored.player_offers().is_empty());
        restored.advance(Some("A"));
        while !restored.is_closed() {
            restored.advance(None);
        }
        assert!(restored
            .into_result()
            .signings
            .iter()
            .any(|s| s.driver_id == "PLAYER"));
    }

    #[test]
    fn slam_target_pulls_driver_to_category() {
        // Duas vagas equivalentes; o slam-chaser prefere a da categoria-alvo.
        let mut a = seat("A", 2, 70.0, 60.0, 100_000.0);
        a.category = "bmw_m2".to_string();
        let mut b = seat("B", 2, 70.0, 60.0, 100_000.0);
        b.category = "production_challenger".to_string();
        let mut c = cand("chaser", 75.0, 2);
        c.slam_target = Some("production_challenger".to_string());
        let res = run_window(vec![a, b], vec![c], &WindowConfig::default(), &mut rng());
        assert_eq!(res.signings[0].category, "production_challenger");
    }
}
