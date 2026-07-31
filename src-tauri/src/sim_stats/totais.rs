//! Acumulador do Monte Carlo: todos os contadores agregados entre runs.

use super::*;

// ── Acumuladores ──────────────────────────────────────────────────────────────

#[derive(Default)]
pub(super) struct Totals {
    // Denominador: pilotos-temporada observados (soma de pilotos ativos no início de cada temporada)
    pub(super) driver_seasons: u64,
    // Evolução (apenas sobreviventes presentes antes e depois)
    pub(super) survivors: u64,
    pub(super) sobe: u64,
    pub(super) desce: u64,
    pub(super) estagna: u64,
    // Por faixa etária: (sobe, desce, estagna, soma_delta, n)
    pub(super) by_age: BTreeMap<&'static str, [f64; 5]>, // [sobe, desce, estagna, soma_delta, n]
    // Lesões
    pub(super) injured_drivers: u64,
    pub(super) inj_leve: u64,
    pub(super) inj_moderada: u64,
    pub(super) inj_grave: u64,
    pub(super) inj_critica: u64,
    // Aposentadorias
    pub(super) retirements: u64,
    pub(super) retire_age_sum: u64,
    pub(super) retire_reasons: BTreeMap<String, u64>,
    // Promoções/Rebaixamentos de pilotos
    pub(super) promoted: u64,
    pub(super) relegated: u64,
    pub(super) freed_no_license: u64,
    // Por temporada-ciclo: taxas (para média Monte Carlo)
    pub(super) inj_rate_samples: Vec<f64>,
    pub(super) retire_rate_samples: Vec<f64>,
    pub(super) sobe_rate_samples: Vec<f64>,
    pub(super) desce_rate_samples: Vec<f64>,
    pub(super) estagna_rate_samples: Vec<f64>,
    // Trajetória de carreira (primeiro vs último overall observado por piloto,
    // entre pilotos vistos em >= 2 temporadas)
    pub(super) traj_count: u64,
    pub(super) traj_sobe: u64,
    pub(super) traj_desce: u64,
    pub(super) traj_estagna: u64,
    pub(super) traj_delta_sum: f64,
    pub(super) traj_by_age: BTreeMap<&'static str, [f64; 5]>, // [sobe, desce, estagna, soma_delta, n]

    // ── Desempenho de pilotos na temporada ──
    pub(super) total_starts: i64, // soma de corridas
    pub(super) total_dnfs: i64,
    pub(super) drivers_raced: u64, // piloto-temporada com >=1 corrida
    pub(super) win_0: u64,
    pub(super) win_1_2: u64,
    pub(super) win_3_5: u64,
    pub(super) win_6p: u64,
    pub(super) with_podium: u64,
    pub(super) motiv_sum: f64,
    pub(super) motiv_n: u64,
    pub(super) motiv_lt20: u64, // pilotos em zona de risco
    // Duração de carreira ao aposentar
    pub(super) retire_career_len_sum: u64,
    pub(super) retire_career_len_n: u64,
    // #2: perfil de quem larga por falta de motivacao (talento desperdicado?)
    pub(super) motiv_retire_n: u64,
    pub(super) motiv_retire_overall_sum: f64,
    pub(super) motiv_retire_good: u64,

    // ── Equipes ──
    pub(super) team_seasons: u64,
    pub(super) fin_state: BTreeMap<String, u64>,
    // Foco da equipe (ideia 4): distribuição das fases (deve espalhar entre os 6,
    // não travar num só).
    pub(super) focus_dist: BTreeMap<String, u64>,
    pub(super) team_insolvent: u64, // cash<0 ou debt>0
    pub(super) cash_sum: f64,
    pub(super) debt_sum: f64,
    pub(super) car_perf_by_tier: BTreeMap<u8, [f64; 2]>, // [soma, n]
    // KPI anti-deflação: correlação carro↔skill por tier. Para cada assento ocupado
    // (car_performance do time, skill do piloto), acumula os 6 momentos de Pearson
    // [n, Σcar, Σskill, Σcar², Σskill², Σ(car·skill)]. r alto = o melhor carro fica com o
    // melhor piloto (o que o Item 1/2 deve reforçar); r baixo/zero = grade deflacionada.
    pub(super) grid_corr_by_tier: BTreeMap<u8, [f64; 6]>,
    // Nível do Carro (1–10) por CATEGORIA: [soma, n, min, max] — mede se o spread emerge
    // e converge perto dos tetos (calibração do Sistema de Nível do Carro).
    pub(super) car_level_by_category: BTreeMap<String, [f64; 4]>,
    // Nível médio POR PEÇA (só times não-rookie): [soma, n] por nome de peça — mostra em
    // quais peças o cérebro investe.
    pub(super) part_level_by_type: BTreeMap<String, [f64; 2]>,
    // Distribuição de FOCO do carro (potência/handling/aceleração/balanceado): contagem.
    pub(super) shape_focus: BTreeMap<String, u64>,
    // Nível médio POR PEÇA quebrado por ARQUÉTIPO de foco: chave "foco|peça" → [soma, n].
    // Mostra COMO cada arquétipo distribui as peças (onde o de-investimento aparece).
    pub(super) part_level_by_focus: BTreeMap<String, [f64; 2]>,
    // Reputação viva: dispersão por tier [soma, soma², min, max, n] — mede se a
    // reputação deixou de ser plana (semente ~±3) e passou a separar topo/fundo.
    pub(super) rep_by_tier: BTreeMap<u8, [f64; 5]>,
    // Moral viva: dispersão global [soma, soma², min, max, n] — mede se a moral
    // deixou de ficar travada em 1.0 e passou a variar por forma/treta interna.
    pub(super) morale_dist: [f64; 5],
    // Rivalidade entre EQUIPES (Fase 2): confirma que existem e são POUCAS (clássicos,
    // não ruído) e que as 4 fontes contribuem. Medido ao fim de cada run (percebida ≥ 20).
    pub(super) tr_runs: u64,
    pub(super) tr_count_sum: u64, // rivalidades vivas somadas entre runs
    pub(super) tr_perceived_sum: f64,
    pub(super) tr_perceived_max: f64,
    pub(super) tr_by_source: BTreeMap<String, u64>,
    pub(super) team_attr_sum: [f64; 5], // facilities, engineering, reputacao, morale, confiabilidade
    pub(super) team_promoted: u64,
    pub(super) team_relegated: u64,
    // ── Ideia 1 (soft landing): sobrevivência do promovido ──
    // Onde o carro do promovido aterrissou no campo de destino (pós-promoção):
    pub(super) promo_landing_gap_sum: f64, // soma (car_promovido − pior_do_campo); <0 = isolado abaixo do lanterna
    pub(super) promo_landing_n: u64,
    pub(super) promo_landing_rank_worst: u64, // aterrissou como PIOR do campo (rank 0 vindo de baixo)
    pub(super) promo_landing_rank_near: u64,  // logo acima do pior (rank 1–2)
    pub(super) promo_landing_rank_mid: u64,   // meio de tabela+ (rank 3+)
    // Bounce-down: promovida rebaixada logo em seguida (janelas observáveis).
    pub(super) promo_events_obs1: u64, // promoções com S+1 dentro do horizonte
    pub(super) promo_bounce_1: u64,    // dessas, rebaixadas já em S+1
    pub(super) promo_events_obs2: u64, // promoções com S+2 dentro do horizonte
    pub(super) promo_bounce_2: u64,    // dessas, rebaixadas em S+1 ou S+2
    // Ricochete do rebaixado: caiu e voltou a subir em ≤2 temporadas.
    pub(super) releg_events_obs2: u64,
    pub(super) releg_bounce_back_2: u64,
    // Concentração de títulos de construtores (consolidado por run)
    pub(super) title_top_share_sum: f64,
    pub(super) title_runs: u64,
    pub(super) title_teams_with_any_sum: f64,
    // Dinastia por classe premium (Pilar D): vencedores únicos + fatia da top, por classe
    pub(super) premium_unique_sum: f64,
    pub(super) premium_top_share_sum: f64,
    pub(super) premium_class_count: u64,
    // Vínculo piloto-equipe (ideia 4): tenure das duplas ao fim de cada run — mede se
    // times seguram pilotos (duplas de era) SEM congelar o mercado.
    pub(super) bond_tenure_sum: f64,
    pub(super) bond_pairs: u64,
    pub(super) bond_max_tenure: i64,
    pub(super) bond_ge3: u64,
    pub(super) bond_ge4: u64,
    // DIAGNÓSTICO SNOWBALL: distribuição do maior "sobe-e-vence" por equipe (comprimento
    // da cadeia de promoções consecutivas temporada+1/tier+1) e o máximo global.
    pub(super) ladder_chain_hist: BTreeMap<usize, u64>,
    pub(super) max_ladder_chain: usize,
    // Identidade: nomes das equipes que fazem cadeia >=2 e nomes de todo campeão da
    // rookie (promoção do tier 0). Se poucos nomes dominam → é sempre a mesma equipe.
    pub(super) climber_names: BTreeMap<String, u64>,
    pub(super) rookie_champ_names: BTreeMap<String, u64>,

    // ── Salários por tier ── [soma, n, min, max]
    pub(super) salary_by_tier: BTreeMap<u8, [f64; 4]>,

    // ── Recuperação de equipes em colapso (trajetória por equipe) ──
    pub(super) teams_ever_collapse: u64,
    pub(super) teams_recovered: u64, // chegaram a stable+ depois do colapso
    pub(super) teams_escaped: u64,   // saíram do colapso (qualquer estado melhor) depois
    pub(super) teams_stuck: u64,     // colapsaram e TERMINARAM em colapso
    pub(super) recover_time_sum: u64, // temporadas do colapso até recuperação (stable+)
    pub(super) recover_time_n: u64,
    pub(super) collapse_seasons_sum: u64, // total de temporada-em-colapso (para média)
    // Desfecho dos episódios de colapso (contadores de produção)
    pub(super) episodes_self_rescued: u64, // salvaram-se no all-in, sem venda
    pub(super) episodes_sold: u64,         // precisaram ser vendidas
    pub(super) ownership_events_recorded: u64, // linhas em team_ownership_events (verificação)

    // ── Funil de carreira (cohort que começou no rookie, tier 0) ──
    pub(super) rookie_cohort: u64,
    // Por tier de destino (1..=6): quantos chegaram (peak >= t) e soma de temporadas até chegar
    pub(super) reached_tier: [u64; 7],
    pub(super) time_to_tier_sum: [u64; 7],
    pub(super) time_to_tier_n: [u64; 7],
    // Impacto do talento: cohort por faixa de habilidade inicial → [n, soma_peak_tier, chegaram_elite(t>=5)]
    pub(super) skill_band: BTreeMap<&'static str, [u64; 3]>,
    // Aposentadorias por tier (de onde abrem vagas) — contexto p/ emergência
    pub(super) retire_by_tier: [u64; 7],

    // ── Textura de nomes do Rookie (grid ~24 assentos; mede "mercado pequeno,
    //    nomes reconhecíveis"). Acumulado a partir da 2ª temporada (a 1ª é o mundo
    //    recém-criado, em que todo nome é inédito por definição). ──
    pub(super) rookie_season_count: u64, // nº de temporadas-rookie observadas (>= temporada 1)
    pub(super) rookie_obs: u64,          // observações de ocupante de assento rookie
    pub(super) rookie_fresh: u64,        // estreias NOVAS (id nunca visto antes nesta run)
    pub(super) rookie_retained: u64, // mesmo piloto que já estava no rookie na temporada anterior
    pub(super) rookie_returning: u64, // conhecido (já visto) mas não estava no rookie ano passado
    pub(super) rookie_age_sum: u64,
    pub(super) rookie_age_n: u64,
}
