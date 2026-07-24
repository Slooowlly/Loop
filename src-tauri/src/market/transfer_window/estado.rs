//! Estado stepável da janela (`WindowState`) e o atalho IA-only (`run_window`).

use std::collections::HashMap;

use rand::Rng;
use serde::{Deserialize, Serialize};

use super::fechamento::{clearing_pass, safety_net};
use super::leilao::{compute_offers, resolve_week};
use super::tipos::{Candidate, PlayerOffer, Seat, Signing, WindowConfig, WindowResult};

/// Meta de contratações por semana — a janela mira esse total (disputa + top-up do
/// clearing) pra espalhar o mercado em vez de despejar tudo no fechamento.
const WEEKLY_FILL_TARGET: usize = 6;

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
                            active_interest: false,
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
        let signed_before = self.signings.len();
        resolve_week(
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
        // PACING: cada semana mira ~WEEKLY_FILL_TARGET contratações no total. A disputa
        // resolve as vagas "quentes"; o TOP-UP completa com o clearing (maior prestígio
        // primeiro → gt3/gt4/endurance aparecem cedo) — espalha o fluxo de forma
        // parelha em vez de despejar tudo no fechamento.
        let contested = self.signings.len() - signed_before;
        let need = WEEKLY_FILL_TARGET.saturating_sub(contested);
        if need > 0 && !self.open.is_empty() && !self.free.is_empty() {
            clearing_pass(
                &mut self.open,
                &mut self.free,
                &mut self.signings,
                &self.cfg,
                self.week,
                Some(need),
            );
        }
        // Fecha quando esvazia ou bate o teto — o fecho TOTAL garante 100% de
        // preenchimento (clearing sem limite + rede de segurança dos craques).
        if self.open.is_empty() || self.free.is_empty() || self.week >= self.cfg.hard_week_cap {
            clearing_pass(
                &mut self.open,
                &mut self.free,
                &mut self.signings,
                &self.cfg,
                self.week,
                None,
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
