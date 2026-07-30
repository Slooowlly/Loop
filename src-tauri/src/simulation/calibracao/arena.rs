//! A arena: roda corridas, eventos e temporadas inteiras do jeito mais parecido possível com o
//! que `commands::race` faz na carreira de verdade.
//!
//! O caminho é o mesmo do jogo — `SimulationContext::from_calendar_entry` +
//! `run_full_race_with_breakdowns` — porque uma régua que mede um atalho não mede nada. A única
//! coisa deliberadamente deixada de fora é a QUEBRA DE PEÇA (`mechanicals`), que depende do
//! desgaste real de carros no banco: aqui ela seria um número inventado, e um número inventado
//! só serviria para maquiar a aleatoriedade que estamos tentando medir.
//!
//! A unidade atômica é o [`Evento`] — um fim de semana concreto (pista + clima + temperatura).
//! Separá-lo da temporada é o que permite a decomposição de variância: rodar o MESMO evento N
//! vezes isola o ruído de corrida; rodar eventos DIFERENTES com o mesmo grid isola a camada de
//! evento.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::calendar::CalendarEntry;
use crate::constants::tracks::{get_all_tracks, TrackInfo};
use crate::models::enums::{RaceStatus, SeasonPhase, ThematicSlot, WeatherCondition};
use crate::simulation::catalog::IncidentCatalog;
use crate::simulation::context::{SimDriver, SimulationContext};
use crate::simulation::engine::run_full_race_with_breakdowns;
use crate::simulation::profile::resolve_simulation_profile;
use crate::simulation::qualifying::{simulate_qualifying, QualifyingResult};
use crate::simulation::race::{simulate_race_with_breakdowns, RaceResult};
use crate::simulation::scoring::{assign_points, determine_fastest_lap};

use super::campo::PerfilCampo;

/// Sobrescritas dos multiplicadores do [`SimulationContext`] resolvidos pelo perfil da categoria.
///
/// Existe para a varredura de sensibilidade: os knobs são campos públicos do contexto, então dá
/// para mexer neles DEPOIS que o perfil resolveu, sem tocar em `profile/**`. `None` = fica o que
/// a categoria mandou.
#[derive(Debug, Clone, Default)]
pub struct AjustesCtx {
    pub race_variance_multiplier: Option<f64>,
    pub race_pace_spread_multiplier: Option<f64>,
    pub start_chaos_multiplier: Option<f64>,
    pub qualifying_variance_multiplier: Option<f64>,
    pub pack_density_factor: Option<f64>,
    pub incident_rate_multiplier: Option<f64>,
    pub overtaking_difficulty_multiplier: Option<f64>,
    pub track_difficulty_multiplier: Option<f64>,
    pub rain_sensitivity: Option<f64>,
}

impl AjustesCtx {
    fn aplicar(&self, ctx: &mut SimulationContext) {
        macro_rules! sobrescrever {
            ($($campo:ident),+ $(,)?) => {
                $(if let Some(v) = self.$campo { ctx.$campo = v; })+
            };
        }
        sobrescrever!(
            race_variance_multiplier,
            race_pace_spread_multiplier,
            start_chaos_multiplier,
            qualifying_variance_multiplier,
            pack_density_factor,
            incident_rate_multiplier,
            overtaking_difficulty_multiplier,
            track_difficulty_multiplier,
            rain_sensitivity,
        );
    }
}

/// Um fim de semana concreto. Tudo que caracteriza a camada de EVENTO da variância.
///
/// Sem `derive(Debug)`: `TrackInfo` é um struct de constante do jogo e não implementa `Debug` —
/// fora da fronteira deste pacote. O `Debug` manual abaixo cobre o que interessa.
#[derive(Clone, Copy)]
pub struct Evento {
    pub pista: &'static TrackInfo,
    pub clima: WeatherCondition,
    pub temperatura: f64,
    /// Se é a última etapa da temporada (liga `is_championship_deciding` no contexto).
    pub decisivo: bool,
}

impl std::fmt::Debug for Evento {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Evento")
            .field("pista", &self.pista.nome_curto)
            .field("clima", &self.clima)
            .field("temperatura", &self.temperatura)
            .field("decisivo", &self.decisivo)
            .finish()
    }
}

/// Como uma temporada sintética é montada.
#[derive(Debug, Clone)]
pub struct ConfigTemporada {
    pub perfil: PerfilCampo,
    pub pilotos: usize,
    pub etapas: usize,
    pub duracao_corrida_min: i32,
    /// Fração de etapas com pista molhada. 0.0 = temporada toda no seco.
    pub fracao_chuva: f64,
    /// Liga os incidentes de pilotagem. Com `false`, o [`IncidentCatalog`] fica vazio e a
    /// corrida vira pura pontuação — é assim que se isola o motor de score do ruído de batida.
    pub incidentes: bool,
    /// Sobrescritas de knob para a varredura de sensibilidade.
    pub ajustes: AjustesCtx,
}

impl ConfigTemporada {
    pub fn rookie() -> Self {
        Self {
            perfil: PerfilCampo::rookie(),
            pilotos: 20,
            etapas: 12,
            duracao_corrida_min: 20,
            fracao_chuva: 0.15,
            incidentes: false,
            ajustes: AjustesCtx::default(),
        }
    }

    pub fn gt3() -> Self {
        Self {
            perfil: PerfilCampo::gt3(),
            pilotos: 20,
            etapas: 12,
            duracao_corrida_min: 45,
            fracao_chuva: 0.15,
            incidentes: false,
            ajustes: AjustesCtx::default(),
        }
    }

    pub fn com_incidentes(mut self, ligado: bool) -> Self {
        self.incidentes = ligado;
        self
    }

    pub fn com_ajustes(mut self, ajustes: AjustesCtx) -> Self {
        self.ajustes = ajustes;
        self
    }

    fn is_endurance(&self) -> bool {
        self.perfil.category_id.starts_with("endurance")
    }
}

/// Catálogo de incidentes REAL, montado num SQLite em memória com as migrações do jogo.
/// Sem ele os incidentes não têm o que sortear e a corrida sai limpa demais.
pub fn catalogo_real() -> IncidentCatalog {
    let Ok(conn) = rusqlite::Connection::open_in_memory() else {
        return IncidentCatalog::empty();
    };
    if crate::db::migrations::run_all(&conn).is_err() {
        return IncidentCatalog::empty();
    }
    IncidentCatalog::load(&conn).unwrap_or_else(|_| IncidentCatalog::empty())
}

/// Catálogo coerente com a configuração — vazio quando os incidentes estão desligados.
pub fn catalogo_para(config: &ConfigTemporada) -> IncidentCatalog {
    if config.incidentes {
        catalogo_real()
    } else {
        IncidentCatalog::empty()
    }
}

// ---------------------------------------------------------------------------
// Eventos
// ---------------------------------------------------------------------------

/// Sorteia o calendário de uma temporada: uma varredura por passo largo sobre o catálogo de
/// pistas do jogo, para pegar caráteres diferentes (Flowing, Technical, Tight, Roval) em vez de
/// repetir Laguna Seca doze vezes.
pub fn sortear_eventos(config: &ConfigTemporada, semente: u64) -> Vec<Evento> {
    let mut rng = StdRng::seed_from_u64(semente);
    let todas = get_all_tracks();
    if todas.is_empty() {
        return Vec::new();
    }
    let inicio = rng.gen_range(0..todas.len());
    let passo = (todas.len() / config.etapas.max(1)).max(1);

    (0..config.etapas)
        .map(|i| Evento {
            pista: &todas[(inicio + i * passo) % todas.len()],
            clima: if rng.gen::<f64>() < config.fracao_chuva {
                WeatherCondition::Wet
            } else {
                WeatherCondition::Dry
            },
            temperatura: rng.gen_range(14.0..32.0),
            decisivo: i + 1 == config.etapas,
        })
        .collect()
}

/// Um único evento repetido `n` vezes — a base do isolamento do ruído de corrida.
pub fn evento_unico(semente: u64) -> Evento {
    let mut rng = StdRng::seed_from_u64(semente);
    let todas = get_all_tracks();
    Evento {
        pista: &todas[rng.gen_range(0..todas.len())],
        clima: WeatherCondition::Dry,
        temperatura: 22.0,
        decisivo: false,
    }
}

/// Monta a entrada de calendário de uma etapa, com o número de voltas derivado do tempo de volta
/// que o próprio perfil da categoria resolve para aquela pista.
fn montar_etapa(config: &ConfigTemporada, rodada: usize, evento: &Evento) -> CalendarEntry {
    let provisorio = resolve_simulation_profile(
        &config.perfil.category_id,
        evento.pista.track_id,
        evento.temperatura,
        evento.clima,
        config.duracao_corrida_min,
        10,
    );
    let voltas = ((config.duracao_corrida_min as f64 * 60_000.0)
        / provisorio.base_lap_time_ms.max(1.0))
    .round()
    .clamp(5.0, 200.0) as i32;

    CalendarEntry {
        id: format!("CAL-R{rodada:02}"),
        season_id: "CAL-S01".to_string(),
        categoria: config.perfil.category_id.clone(),
        rodada: rodada as i32,
        nome: format!("Etapa {} - {}", rodada, evento.pista.nome_curto),
        track_id: evento.pista.track_id,
        track_name: evento.pista.nome.to_string(),
        track_config: String::new(),
        clima: evento.clima,
        temperatura: evento.temperatura,
        voltas,
        duracao_corrida_min: config.duracao_corrida_min,
        duracao_classificacao_min: 15,
        status: RaceStatus::Pendente,
        horario: "14:00".to_string(),
        week_of_year: rodada as i32,
        season_phase: SeasonPhase::BlocoRegular,
        display_date: "2026-01-01".to_string(),
        thematic_slot: ThematicSlot::NaoClassificado,
        season_week: Some(rodada as u32),
    }
}

/// Contexto de simulação de um evento, já com os ajustes de knob aplicados.
pub fn contexto_do_evento(
    config: &ConfigTemporada,
    rodada: usize,
    evento: &Evento,
) -> SimulationContext {
    let entrada = montar_etapa(config, rodada, evento);
    let mut ctx = SimulationContext::from_calendar_entry(
        &entrada,
        config.perfil.category_tier,
        evento.decisivo,
    );
    ctx.incidents_enabled = config.incidentes;
    config.ajustes.aplicar(&mut ctx);
    ctx
}

// ---------------------------------------------------------------------------
// Rodadas
// ---------------------------------------------------------------------------

/// Roda UM evento com um RNG próprio. Duas chamadas com o mesmo grid, o mesmo evento e sementes
/// diferentes diferem apenas pelo ruído de corrida — é exatamente esse par que a decomposição de
/// variância usa para medir a fatia "corrida".
pub fn rodar_evento(
    config: &ConfigTemporada,
    grid: &[SimDriver],
    evento: &Evento,
    rodada: usize,
    catalogo: &IncidentCatalog,
    semente: u64,
) -> RaceResult {
    let ctx = contexto_do_evento(config, rodada, evento);
    let mut rng = StdRng::seed_from_u64(semente);
    run_full_race_with_breakdowns(grid, &ctx, config.is_endurance(), catalogo, None, &mut rng)
}

/// Roda um evento com um GRID DE LARGADA IMPOSTO, pulando a classificação.
///
/// **Ponto de merge conhecido**: esta é a única função do pacote que chama `simulate_qualifying`
/// diretamente. O pacote paralelo está introduzindo um `ConfigQuali` nessa assinatura; quando
/// isso entrar, é aqui — e só aqui — que a chamada precisa ser atualizada.
///
/// Serve à pergunta "a corrida acaba na largada?": impondo um grid sorteado, sem relação nenhuma
/// com o ritmo dos pilotos, a correlação da chegada com esse grid mede quanto do resultado é
/// herança da posição inicial e não disputa. Usa `simulate_race_with_breakdowns` e refaz o
/// pós-processamento (volta mais rápida + pontos) exatamente como
/// [`run_full_race_with_breakdowns`] faria — a única diferença é de onde vem a ordem de largada.
pub fn rodar_evento_com_grid_imposto(
    config: &ConfigTemporada,
    grid: &[SimDriver],
    evento: &Evento,
    rodada: usize,
    catalogo: &IncidentCatalog,
    semente: u64,
    embaralhar_largada: bool,
) -> RaceResult {
    let ctx = contexto_do_evento(config, rodada, evento);
    let mut rng = StdRng::seed_from_u64(semente);

    let mut qualifying: Vec<QualifyingResult> = simulate_qualifying(grid, &ctx, &mut rng);
    if embaralhar_largada {
        // Fisher–Yates sobre a ORDEM, preservando os campos de cada piloto. O grid deixa de ter
        // qualquer relação com o ritmo; o tempo de volta anunciado deixa de bater com a posição,
        // o que é irrelevante aqui porque a corrida só lê `position`.
        for i in (1..qualifying.len()).rev() {
            let j = rng.gen_range(0..=i);
            qualifying.swap(i, j);
        }
        for (indice, q) in qualifying.iter_mut().enumerate() {
            q.position = indice as i32 + 1;
            q.is_pole = indice == 0;
        }
    }

    let is_endurance = config.is_endurance();
    let mut resultado = simulate_race_with_breakdowns(
        grid,
        &qualifying,
        &ctx,
        catalogo,
        is_endurance,
        None,
        &mut rng,
    );
    let fastest = determine_fastest_lap(&mut resultado.race_results).unwrap_or_default();
    assign_points(&mut resultado.race_results, is_endurance);
    resultado.fastest_lap_id = fastest;
    resultado
}

/// Roda uma temporada inteira sobre um grid já montado. O grid é o MESMO nas N etapas — é
/// exatamente essa fixidez que faz a correlação entre etapas consecutivas ser uma medida honesta
/// do quanto a simulação sorteia de verdade.
pub fn rodar_temporada(
    config: &ConfigTemporada,
    grid: &[SimDriver],
    catalogo: &IncidentCatalog,
    semente: u64,
) -> Vec<RaceResult> {
    let eventos = sortear_eventos(config, semente);
    eventos
        .iter()
        .enumerate()
        .map(|(indice, evento)| {
            rodar_evento(
                config,
                grid,
                evento,
                indice + 1,
                catalogo,
                semente
                    .wrapping_mul(31)
                    .wrapping_add(indice as u64 * 1_000_003),
            )
        })
        .collect()
}

/// Semente do grid da temporada `t` de uma campanha. Isolada para que a decomposição de variância
/// e a campanha normal partam exatamente dos mesmos grids.
pub fn semente_da_temporada(semente_base: u64, t: usize) -> u64 {
    semente_base
        .wrapping_mul(1_000_003)
        .wrapping_add(t as u64 * 7919)
}

/// Roda `temporadas` campeonatos completos, cada um com **grid próprio** (semente derivada), e
/// devolve as métricas de cada um. Total de corridas = `temporadas * config.etapas`.
pub fn rodar_campanha(
    config: &ConfigTemporada,
    temporadas: usize,
    semente_base: u64,
) -> Vec<super::metricas::MetricasTemporada> {
    let catalogo = catalogo_para(config);

    (0..temporadas)
        .map(|t| {
            let semente = semente_da_temporada(semente_base, t);
            let grid = super::campo::gerar_campo(&config.perfil, config.pilotos, semente);
            let corridas = rodar_temporada(config, &grid, &catalogo, semente ^ 0x5EED);
            super::metricas::medir_temporada(&grid, &corridas)
        })
        .collect()
}

/// Atalho: roda a campanha e já agrega. É o que os testes e o relatório chamam.
pub fn medir(
    rotulo: &str,
    config: &ConfigTemporada,
    temporadas: usize,
    semente_base: u64,
) -> super::metricas::MetricasAgregadas {
    let por_temporada = rodar_campanha(config, temporadas, semente_base);
    super::metricas::agregar(rotulo, &por_temporada)
}

/// Roda a campanha devolvendo as CORRIDAS cruas, para quem precisa medir processo (gaps,
/// inversões, ganho de posição) e não só o resultado agregado.
pub fn rodar_campanha_crua(
    config: &ConfigTemporada,
    temporadas: usize,
    semente_base: u64,
) -> Vec<(Vec<SimDriver>, Vec<RaceResult>)> {
    let catalogo = catalogo_para(config);

    (0..temporadas)
        .map(|t| {
            let semente = semente_da_temporada(semente_base, t);
            let grid = super::campo::gerar_campo(&config.perfil, config.pilotos, semente);
            let corridas = rodar_temporada(config, &grid, &catalogo, semente ^ 0x5EED);
            (grid, corridas)
        })
        .collect()
}
