//! O laço da corrida: percorre os cinco segmentos aplicando dano latente, incidentes, quebra de
//! peça e pontuação, e no fim monta o [`RaceResult`].

use rand::Rng;

use crate::constants::scoring::{
    ATRASO_LARGADA_MS_POR_POSICAO_POR_VOLTA, MS_POR_PONTO_DE_RITMO_POR_VOLTA, RITMO_DE_REFERENCIA,
    RITMO_PERDIDO_POR_POSICAO_EM_INCIDENTE,
};

use crate::simulation::catalog::IncidentCatalog;
use crate::simulation::context::{SimDriver, SimulationContext};
use crate::simulation::incidents::process_segment_incidents_cfg;
use crate::simulation::qualifying::QualifyingResult;

use super::danos::process_pending_damage;
use super::estrategia::{
    atraso_sob_safety_car, planejar_paradas, traz_bandeira_amarela, ParadasDoCarro, PlanoDeParada,
    CUSTO_DE_PARADA_MS, FRACAO_DO_CUSTO_SOB_SAFETY_CAR,
};
use super::pontuacao::{
    amplitude_de_ritmo_com_relancamento, apply_physical_degradation, apply_tire_degradation,
    calculate_segment_score,
};
use super::resultados::{build_race_results, derive_caution_segments};
use super::tipos::{MechanicalOutcome, RaceResult, RaceSegment, RaceState};
use super::trafego::{
    fracao_do_trecho_na_janela, perda_por_ar_sujo, tentar_ultrapassagem, DesfechoDaTentativa,
    CUSTO_TENTATIVA_FALHA_ATACANTE_MS, CUSTO_TENTATIVA_FALHA_DEFENSOR_MS,
    GAP_MINIMO_ENTRE_CARROS_MS, JANELA_AR_SUJO_MS, JANELA_DE_ATAQUE_MS,
};
use crate::simulation::incidents::{
    IncidentResult, IncidentSeverity, IncidentType, PendingDamage, CHANCE_DE_DANO_NO_CONTATO,
    IRM_CONTATO_DE_DISPUTA, MANIFESTACAO_INICIAL_DO_DANO_DE_CONTATO,
};

/// Registra um contato de disputa no carro. Sem `positions_lost`: o custo da tentativa falha
/// já foi cobrado em TEMPO, e somar posições aqui cobraria o mesmo evento duas vezes.
///
/// O contato também pode deixar o carro AVARIADO (dano latente): até aqui um roda-a-roda não
/// tinha consequência física nenhuma — custava tempo e sumia. Agora ele entra no mesmo
/// mecanismo de dano latente da colisão, então o encostão do trecho 2 pode cobrar posições no
/// trecho 4. `is_dnf_capable: false` de propósito: um encostão não tira o carro sozinho (essa
/// é a via da colisão de verdade); ele custa posições dentro da corrida e desgaste depois dela
/// (ver `car::crash::apply_contact_wear`, aplicado na manutenção pós-corrida).
fn empurrar_contato(
    state: &mut RaceState,
    outro_id: &str,
    seg_str: &str,
    descricao: &str,
    car_reliability: f64,
    rng: &mut impl Rng,
) {
    // Carro frágil se avaria mais fácil — a mesma modulação de `maybe_add_pending_damage`.
    let chance = (CHANCE_DE_DANO_NO_CONTATO * (1.0 - (car_reliability / 100.0 * 0.40))).clamp(0.05, 0.80);
    if rng.gen::<f64>() < chance {
        state.pending_damage.push(PendingDamage {
            catalog_id: String::new(),
            origin_segment: seg_str.to_string(),
            manifest_chance: MANIFESTACAO_INICIAL_DO_DANO_DE_CONTATO,
            is_dnf_capable: false,
        });
    }

    state.incidents.push(IncidentResult {
        pilot_id: state.driver_id.clone(),
        incident_type: IncidentType::Collision,
        severity: IncidentSeverity::Minor,
        segment: seg_str.to_string(),
        positions_lost: 0,
        is_dnf: false,
        description: descricao.to_string(),
        linked_pilot_id: Some(outro_id.to_string()),
        is_two_car_incident: true,
        // Contato de disputa PODE machucar, mas com o peso certo: ver
        // `IRM_CONTATO_DE_DISPUTA` (20% por carro). O `1.0` que estava aqui dava 50%.
        injury_risk_multiplier: IRM_CONTATO_DE_DISPUTA,
        narrative_importance_hint: 1,
        catalog_id: None,
        damage_origin_segment: None,
    });
}

/// Ordena o pelotão: quem terminou na frente, por menor tempo acumulado; abandonos no fim.
fn ordenar_por_tempo(states: &mut [RaceState]) {
    states.sort_by(|a, b| match (a.is_dnf, b.is_dnf) {
        (false, true) => std::cmp::Ordering::Less,
        (true, false) => std::cmp::Ordering::Greater,
        _ => a
            .tempo_acumulado_ms
            .partial_cmp(&b.tempo_acumulado_ms)
            .unwrap_or(std::cmp::Ordering::Equal),
    });
}

/// Executa uma parada: cobra o tempo de box e devolve PNEU NOVO.
///
/// O pneu novo é o mecanismo inteiro do undercut — a penalidade de pneu gasto de
/// `pontuacao.rs` já existia e cobrava ritmo, e a parada é o que permite comprá-la de volta.
fn executar_parada(state: &mut RaceState, volta: u32, custo_ms: f64) {
    state.tempo_acumulado_ms += custo_ms;
    state.tire_wear = 1.0;
    state.paradas.voltas.push(volta);
    state.paradas.posicao_antes.push(state.current_position);
    let indice = state.paradas.voltas.len() - 1;
    state.paradas.aguardando_fechamento.push(indice);
}

/// Segundos parados no box → milissegundos de corrida. Agora que a moeda É tempo, o
/// contrato "15s no box = 15s a mais no tempo de corrida" deixou de precisar de conversão:
/// é uma multiplicação por 1000, e a equivalência é exata por construção em vez de depender
/// de dois coeficientes casarem.
///
/// A equivalência é medida CONTRA O LÍDER (`build_race_results` ancora o tempo no vencedor).
/// Quando quem quebra é o próprio líder, a âncora se move junto e o custo aparente encolhe
/// pela margem que ele tinha — o que é o comportamento certo: ele só perde o que a margem
/// não cobria.
///
/// **Isto conserta um caso que o modelo de pontos errava em silêncio.** Lá, o desconto era
/// `secs·1000 / (K · voltas)` pontos, com um `.max(0.0)` no total acumulado logo em seguida.
/// Um reparo cedo (segmento de largada, quando o carro só tinha o bônus de grid acumulado)
/// batia no chão do clamp e o piloto pagava MENOS que os segundos que ficou parado — quanto
/// mais atrás ele largava, mais barato saía quebrar. Em tempo não existe chão: o relógio
/// anda para todo mundo igual.
fn segundos_de_reparo_em_ms(secs: u32) -> f64 {
    secs as f64 * 1000.0
}

/// Quantas voltas cabem num segmento. Um segmento é 1/5 da corrida por definição (é como
/// [`RaceSegment::from_lap`] reparte as voltas); guardamos a fração para o tempo do segmento
/// e o ruído por volta saírem exatos mesmo quando o total não é múltiplo de 5.
fn voltas_no_segmento(total_laps: i32) -> f64 {
    total_laps.max(1) as f64 / 5.0
}

/// Tempo que um segmento custa a este carro, em ms: quanto ele fica atrás do carro de
/// referência ao longo das voltas daquele trecho.
///
/// É aqui que o score de `pontuacao.rs` deixa de ser um saldo e vira o que sempre deveria
/// ter sido — um RITMO DE VOLTA. Ritmo alto = volta rápida = menos tempo perdido.
fn tempo_do_segmento_ms(ritmo: f64, total_laps: i32) -> f64 {
    (RITMO_DE_REFERENCIA - ritmo) * MS_POR_PONTO_DE_RITMO_POR_VOLTA * voltas_no_segmento(total_laps)
}

/// Atraso de largada acumulado por sair da posição `posicao` do grid, em ms.
fn atraso_de_largada_ms(posicao: i32, total_laps: i32) -> f64 {
    (posicao.max(1) - 1) as f64 * ATRASO_LARGADA_MS_POR_POSICAO_POR_VOLTA * total_laps.max(1) as f64
}

// ─────────────────────── O ruído da corrida ───────────────────────
//
// O modelo antigo sorteava CINCO números independentes por corrida, um por segmento, com a
// mesma amplitude numa prova de 12 voltas e numa de 200. Isso tinha dois defeitos:
//
// 1. **Não sabia a distância.** Sprint e enduro tinham o mesmo caos, e a diferença de
//    caráter entre formatos precisava ser forçada na mão pelo perfil da categoria.
// 2. **Se auto-cancelava.** Cinco sorteios independentes somados tendem à média, então o
//    ruído quase não movia a ordem final — o mesmo defeito que a camada de acerto de fim de
//    semana resolveu no pacote das camadas.
//
// Agora o ruído é POR VOLTA e tem MEMÓRIA:
//
// - σ é por volta, então o desvio de um trecho sai em `σ_volta · √voltas_do_trecho` e o da
//   corrida em `σ_volta · √L`. Como a diferença de RITMO entre dois carros cresce com `L`
//   (linear) e o ruído só com `√L`, a razão ruído/sinal cai como `1/√L`: sprint fica
//   caótico e enduro converge, sem knob nenhum. Isso sai da matemática, não de constante.
// - Os trechos são correlacionados por um AR(1): quem está num ritmo ruim continua num
//   ritmo ruim por alguns trechos. Com ρ = 0,5 a variância da soma vai de 5σ² para
//   ~11,1σ², ou seja, o desvio final é ~1,49× o de sorteios independentes — sem tocar em
//   amplitude nenhuma.

/// Correlação entre trechos vizinhos do ruído de ritmo. PROVISÓRIO — o harness decide.
pub const CORRELACAO_DO_RUIDO_ENTRE_TRECHOS: f64 = 0.5;

/// Distância (em voltas) em que o ruído por volta reproduz o desvio total que o modelo
/// antigo produzia por segmento. É só a ÂNCORA de escala da migração: acima dela a corrida
/// fica mais limpa que antes, abaixo mais caótica — que é justamente o comportamento novo.
/// PROVISÓRIO.
pub const VOLTAS_DE_REFERENCIA_DO_RUIDO: f64 = 20.0;

/// Converte a amplitude de ritmo por SEGMENTO do modelo antigo em um σ de tempo por VOLTA.
///
/// Derivação da âncora: no modelo antigo, o desvio total da corrida era
/// `(A/√3)·MS·(L/5)·√5` (cinco sorteios uniformes independentes de faixa ±A, cada um valendo
/// um segmento de L/5 voltas). No novo é `σ_volta·√L`. Igualando os dois em
/// `L = VOLTAS_DE_REFERENCIA_DO_RUIDO` sai `σ_volta = A·MS·√(L_ref/15)`.
fn sigma_de_ruido_por_volta_ms(amplitude_de_ritmo: f64) -> f64 {
    amplitude_de_ritmo
        * MS_POR_PONTO_DE_RITMO_POR_VOLTA
        * (VOLTAS_DE_REFERENCIA_DO_RUIDO / 15.0).sqrt()
}

/// Fachada de teste de [`sigma_de_ruido_por_volta_ms`]: a propriedade "o ruído sabe a
/// distância" é analítica e passou a ser verificada direto na função, porque depois do
/// pacote D o desvio de POSIÇÃO deixou de ser leitura limpa dela (o trem comprime a ordem).
#[cfg(test)]
pub fn sigma_de_ruido_por_volta_ms_para_teste(amplitude: f64) -> f64 {
    sigma_de_ruido_por_volta_ms(amplitude)
}

/// Sorteio de variância unitária, com a mesma forma uniforme do modelo antigo (a faixa
/// ±√3 é o que dá desvio 1).
fn sorteio_unitario(rng: &mut impl Rng) -> f64 {
    rng.gen_range(-1.0..=1.0) * 3.0_f64.sqrt()
}

/// Qual motor rodar.
///
/// `Atual` é o jogo: ruído por volta com memória, ar sujo, ultrapassagem como tentativa e
/// trem de carros. `LegadoDePontos` reproduz o motor de ANTES da troca de moeda — sorteio por
/// segmento, sem posição na pista — e existe para a prova de equivalência bit-exata da fase 1
/// do pacote C e para o harness medir um contra o outro. Nada além dessas duas coisas deve
/// usar o legado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModoDoMotor {
    Atual,
    LegadoDePontos,
    /// Tudo do atual, MENOS a consequência do safety car: ele é detectado e registrado, mas
    /// não comprime o pelotão, não abre parada barata e não gera relançamento.
    ///
    /// Existe para uma medição que não dá de outro jeito: "o safety car muda quem ganha?" só
    /// tem resposta comparando a MESMA corrida com e sem ele. Comparar corridas-com-SC contra
    /// corridas-sem-SC é confundido — quem tem SC é quem teve batida grande, e a batida já
    /// embaralha por conta própria.
    AtualSemSafetyCar,
}

impl ModoDoMotor {
    /// O motor de antes da troca de moeda (pacote C): sem ruído por volta e sem posição na
    /// pista. Só a prova de equivalência e o harness usam.
    fn e_legado(self) -> bool {
        self == ModoDoMotor::LegadoDePontos
    }

    /// Posição na pista (ar sujo, trem, ultrapassagem) e estratégia de parada valem.
    fn tem_pista_viva(self) -> bool {
        !self.e_legado()
    }

    /// O safety car tem consequência.
    fn tem_safety_car(self) -> bool {
        self == ModoDoMotor::Atual
    }
}

/// Corrida simulada cobrando também os desfechos de QUEBRA DE PEÇA pré-rolados (Fase 7).
///
/// `mechanicals` é a chave de FONTE ÚNICA da pane mecânica:
/// - `Some(..)` — o Sistema de Quebra está no comando. Os desfechos vêm do cérebro
///   `car::breakdown`, rolados sobre o desgaste REAL do carro de cada time (por isso o time
///   pobre, que estica peça por falta de caixa, é o que quebra), e a pane genérica do catálogo
///   de incidentes é DESLIGADA. Lista vazia é válido: significa "a quebra rodou e ninguém
///   quebrou", não "sistema desligado".
/// - `None` — a quebra não roda nesta corrida (rascunho histórico, grid sintético sem carro no
///   banco). A pane do catálogo continua sendo a fonte de falha mecânica.
#[allow(clippy::too_many_arguments)]
pub fn simulate_race_with_breakdowns(
    drivers: &[SimDriver],
    qualifying: &[QualifyingResult],
    ctx: &SimulationContext,
    catalog: &IncidentCatalog,
    is_endurance: bool,
    mechanicals: Option<&[MechanicalOutcome]>,
    rng: &mut impl Rng,
) -> RaceResult {
    simulate_race_com_modo(
        drivers,
        qualifying,
        ctx,
        catalog,
        is_endurance,
        mechanicals,
        ModoDoMotor::Atual,
        rng,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn simulate_race_com_modo(
    drivers: &[SimDriver],
    qualifying: &[QualifyingResult],
    ctx: &SimulationContext,
    catalog: &IncidentCatalog,
    is_endurance: bool,
    mechanicals: Option<&[MechanicalOutcome]>,
    modo: ModoDoMotor,
    rng: &mut impl Rng,
) -> RaceResult {
    let catalog_mechanical = mechanicals.is_none();
    let mechanicals = mechanicals.unwrap_or(&[]);
    let total_drivers = qualifying.len() as i32;
    let mut applied_mechanicals: Vec<usize> = Vec::new();
    let mut states: Vec<RaceState> = qualifying
        .iter()
        .map(|result| {
            // A CHAMADA DA EQUIPE (pacote G), feita antes da largada e determinística: é
            // personagem da equipe naquele evento, não sorte do dia. Por não sair do `rng`, ela
            // não perturba a sequência de sorteios do motor — o que mantém a prova de
            // equivalência bit-exata da fase 1 do pacote C intacta.
            let plano = match (modo, drivers.iter().find(|d| d.id == result.pilot_id)) {
                (m, Some(driver)) if m.tem_pista_viva() => planejar_paradas(
                    &driver.id,
                    &driver.team_id,
                    ctx.track_id,
                    ctx.total_laps,
                    ctx.race_duration_minutes,
                    driver.qualidade_de_estrategia,
                ),
                _ => PlanoDeParada::sem_parada(),
            };
            RaceState {
                driver_id: result.pilot_id.clone(),
                tire_wear: 1.0,
                physical_condition: 1.0,
                tempo_acumulado_ms: atraso_de_largada_ms(result.position, ctx.total_laps),
                desvio_de_ritmo: 0.0,
                trafego: Default::default(),
                paradas: ParadasDoCarro {
                    estrategia_id: plano.estrategia_id,
                    pendentes: plano.voltas_planejadas,
                    ..Default::default()
                },
                is_dnf: false,
                current_position: result.position,
                incidents: Vec::new(),
                dnf_reason: None,
                dnf_segment: None,
                pending_damage: Vec::new(),
            }
        })
        .collect();

    // Safety cars da corrida: volta de entrada e a classificação naquele momento.
    let mut safety_cars: Vec<u32> = Vec::new();
    let mut ordem_pre_safety_car: Vec<Vec<String>> = Vec::new();
    // O trecho seguinte a um safety car é um RELANÇAMENTO: pelotão colado, pneu frio,
    // acordeão. Sem isto o safety car só comprime gaps e não muda nada.
    let mut relancamento_no_proximo_trecho = false;

    for segment in [
        RaceSegment::Start,
        RaceSegment::Early,
        RaceSegment::Mid,
        RaceSegment::Late,
        RaceSegment::Finish,
    ] {
        // Consome o sinal deixado pelo safety car do trecho anterior.
        let relancamento = std::mem::take(&mut relancamento_no_proximo_trecho);

        if ctx.incidents_enabled {
            // Processar danos latentes ANTES dos rolls normais do segmento
            process_pending_damage(
                &mut states,
                segment,
                drivers,
                catalog,
                ctx.vehicle_class,
                is_endurance,
                rng,
            );

            let result = process_segment_incidents_cfg(
                drivers,
                &states,
                segment,
                ctx.weather,
                ctx.is_championship_deciding,
                ctx.incident_rate_multiplier,
                ctx.start_chaos_multiplier,
                ctx.pack_density_factor,
                catalog,
                ctx.vehicle_class,
                is_endurance,
                catalog_mechanical,
                rng,
            );

            for incident in result.incidents {
                if let Some(state) = states.iter_mut().find(|s| s.driver_id == incident.pilot_id) {
                    if incident.is_dnf {
                        state.is_dnf = true;
                        state.dnf_reason = Some(incident.description.clone());
                        state.dnf_segment = Some(segment);
                    }
                    state.incidents.push(incident);
                }
            }

            // Aplicar novos danos latentes gerados neste segmento
            for (driver_id, pd) in result.new_pending_damage {
                if let Some(state) = states.iter_mut().find(|s| s.driver_id == driver_id) {
                    state.pending_damage.push(pd);
                }
            }
        }

        // QUEBRA DE PEÇA (Fase 7): o desfecho pré-rolado cobra o preço nesta altura da corrida.
        // Roda FORA do `incidents_enabled` de propósito — quebra não é incidente de pilotagem;
        // é consequência do desgaste que o time carregou pra cá, e vale mesmo com incidentes
        // desligados. DNF encerra o carro; leve/grave desconta os segundos do box na moeda da
        // sim, então a perda de posição sai da física do resultado, não de um chute.
        for (idx, m) in mechanicals.iter().enumerate() {
            if RaceSegment::from_lap(m.lap, ctx.total_laps) != segment {
                continue;
            }
            let Some(state) = states.iter_mut().find(|s| s.driver_id == m.pilot_id) else {
                continue;
            };
            // Carro já fora (batida antes): a peça largaria num carro que não estava mais lá.
            if state.is_dnf {
                continue;
            }
            if m.is_dnf {
                state.is_dnf = true;
                state.dnf_reason = Some(m.label.clone());
                state.dnf_segment = Some(segment);
            } else {
                state.tempo_acumulado_ms += segundos_de_reparo_em_ms(m.penalty_secs);
            }
            applied_mechanicals.push(idx);
        }

        let seg_str = segment.as_str();

        // ── POSIÇÃO NA PISTA (pacote D): retrato do pelotão ANTES de andar o trecho ──
        //
        // `states` chega aqui já ordenado por tempo (fim do trecho anterior) e, na largada,
        // em ordem de grid. Este retrato é o que responde "quem está atrás de quem e a que
        // distância" — a pergunta que a moeda de pontos não sabia responder.
        let ordem_de_entrada: Vec<usize> =
            (0..states.len()).filter(|&i| !states[i].is_dnf).collect();
        let gap_de_entrada: Vec<f64> = ordem_de_entrada
            .iter()
            .enumerate()
            .map(
                |(pos, &i)| match pos.checked_sub(1).and_then(|p| ordem_de_entrada.get(p)) {
                    Some(&frente) => {
                        states[i].tempo_acumulado_ms - states[frente].tempo_acumulado_ms
                    }
                    None => f64::INFINITY,
                },
            )
            .collect();

        // Ritmo LIMPO (sem ar sujo) de cada carro. Precisa sair antes do laço porque a
        // tentativa de ultrapassagem compara o ritmo dos DOIS lados.
        let mut ritmo_limpo: Vec<f64> = vec![0.0; states.len()];
        for &i in &ordem_de_entrada {
            if let Some(driver) = drivers.iter().find(|d| d.id == states[i].driver_id) {
                ritmo_limpo[i] = calculate_segment_score(driver, &states[i], segment, ctx);
            }
        }

        // Quanto cada carro FECHA no carro da frente ao longo deste trecho, em ms. É a
        // diferença de ritmo convertida em tempo — e é o que permite enxergar a disputa que
        // acontece DENTRO do trecho, e não só na foto da fronteira.
        let fechamento_no_trecho: Vec<f64> = ordem_de_entrada
            .iter()
            .enumerate()
            .map(
                |(pos, &i)| match pos.checked_sub(1).and_then(|p| ordem_de_entrada.get(p)) {
                    Some(&frente) => {
                        (ritmo_limpo[i] - ritmo_limpo[frente])
                            * MS_POR_PONTO_DE_RITMO_POR_VOLTA
                            * voltas_no_segmento(ctx.total_laps)
                    }
                    None => 0.0,
                },
            )
            .collect();

        for (pos_entrada, &i) in ordem_de_entrada.iter().enumerate() {
            let Some(driver) = drivers
                .iter()
                .find(|driver| driver.id == states[i].driver_id)
            else {
                continue;
            };
            let amplitude = amplitude_de_ritmo_com_relancamento(driver, ctx, segment, relancamento);
            let mut ritmo_bruto = ritmo_limpo[i];
            let mut ruido_ms = 0.0;
            let state = &mut states[i];

            match modo {
                // Sorteio por segmento, somado ao RITMO — o modelo de antes da troca de
                // moeda, preservado só para a prova de equivalência.
                ModoDoMotor::LegadoDePontos => {
                    ritmo_bruto = (ritmo_bruto + rng.gen_range(-amplitude..=amplitude)).max(5.0);
                }
                // Ruído por VOLTA, com memória entre trechos, somado ao TEMPO.
                ModoDoMotor::Atual | ModoDoMotor::AtualSemSafetyCar => {
                    let z = sorteio_unitario(rng);
                    let rho = CORRELACAO_DO_RUIDO_ENTRE_TRECHOS;
                    // Primeiro trecho parte da distribuição estacionária (o próprio z);
                    // os seguintes carregam metade do trecho anterior.
                    state.desvio_de_ritmo = if segment == RaceSegment::Start {
                        z
                    } else {
                        rho * state.desvio_de_ritmo + (1.0 - rho * rho).sqrt() * z
                    };
                    ruido_ms = state.desvio_de_ritmo
                        * sigma_de_ruido_por_volta_ms(amplitude)
                        * voltas_no_segmento(ctx.total_laps).sqrt();
                }
            }

            // AR SUJO: atrás de outro carro, o carro perde apoio. Quanto perde depende de
            // quanto ELE vive de aerodinâmica (o viés de pico do shape) e de QUANTO do trecho
            // passou lá atrás — um carro que só alcança no fim do trecho não pagou o trecho
            // inteiro em ar sujo.
            if modo.tem_pista_viva() {
                let fechamento = fechamento_no_trecho[pos_entrada];
                let fracao = fracao_do_trecho_na_janela(
                    gap_de_entrada[pos_entrada],
                    fechamento,
                    JANELA_AR_SUJO_MS,
                );
                if fracao > 0.0 {
                    // O gap MÉDIO enquanto esteve na janela é o que dita a perda.
                    let gap_medio = (gap_de_entrada[pos_entrada] - fechamento * 0.5).max(0.0);
                    let perda_de_apoio = perda_por_ar_sujo(gap_medio, driver.vies_de_pico);
                    if perda_de_apoio > 0.0 {
                        state.trafego.segmentos_em_ar_sujo += 1;
                        ritmo_bruto -= perda_de_apoio * fracao;
                    }
                }
            }

            // Incidente custa ritmo no trecho em que aconteceu.
            let perda: f64 = state
                .incidents
                .iter()
                .filter(|incident| incident.segment == seg_str && !incident.is_dnf)
                .map(|incident| {
                    incident.positions_lost as f64 * RITMO_PERDIDO_POR_POSICAO_EM_INCIDENTE
                })
                .sum();
            // Piso de ritmo: um carro não anda "para trás". (Chão de ritmo, não de tempo
            // — é o que faz a punição do incidente saturar sem limitar o relógio.)
            let ritmo = (ritmo_bruto - perda).max(0.0);
            state.tempo_acumulado_ms += tempo_do_segmento_ms(ritmo, ctx.total_laps) + ruido_ms;

            apply_tire_degradation(state, driver, ctx);
            apply_physical_degradation(state, driver, ctx);
        }

        // ── ULTRAPASSAGEM COMO TENTATIVA + TREM DE CARROS ──
        //
        // Percorre o pelotão da frente para trás, na ordem de ENTRADA do trecho. Quem estava
        // na janela de ataque e tinha ritmo melhor tenta passar; quem não passou não pode
        // terminar o trecho à frente de quem estava na frente. É esta segunda regra que
        // produz o trem — e o trem é o mecanismo, não um efeito colateral dele.
        if modo.tem_pista_viva() {
            for pos_entrada in 1..ordem_de_entrada.len() {
                let atras = ordem_de_entrada[pos_entrada];
                let frente = ordem_de_entrada[pos_entrada - 1];
                // Quem abandonou no meio do trecho não bloqueia mais ninguém.
                if states[atras].is_dnf || states[frente].is_dnf {
                    continue;
                }

                let delta_de_ritmo = ritmo_limpo[atras] - ritmo_limpo[frente];
                // "Na janela" = chegou a ficar em posição de ataque em ALGUM momento do
                // trecho, não só na foto do início dele.
                let na_janela = fracao_do_trecho_na_janela(
                    gap_de_entrada[pos_entrada],
                    fechamento_no_trecho[pos_entrada],
                    JANELA_DE_ATAQUE_MS,
                ) > 0.0;
                let mais_rapido = delta_de_ritmo > 0.0;
                let mut passou = false;

                if na_janela && mais_rapido {
                    let (Some(atacante), Some(defensor)) = (
                        drivers.iter().find(|d| d.id == states[atras].driver_id),
                        drivers.iter().find(|d| d.id == states[frente].driver_id),
                    ) else {
                        continue;
                    };
                    let desfecho = tentar_ultrapassagem(
                        atacante,
                        defensor,
                        delta_de_ritmo,
                        ctx.overtaking_difficulty_multiplier,
                        ctx.incidents_enabled,
                        rng,
                    );
                    states[atras].trafego.tentativas_ultrapassagem += 1;
                    states[frente].trafego.tentativas_sofridas += 1;

                    match desfecho {
                        DesfechoDaTentativa::Concluida => {
                            passou = true;
                            states[atras].trafego.ultrapassagens_concluidas += 1;
                        }
                        DesfechoDaTentativa::Falhou | DesfechoDaTentativa::FalhouComContato => {
                            states[atras].tempo_acumulado_ms += CUSTO_TENTATIVA_FALHA_ATACANTE_MS;
                            states[frente].tempo_acumulado_ms += CUSTO_TENTATIVA_FALHA_DEFENSOR_MS;
                            if desfecho == DesfechoDaTentativa::FalhouComContato {
                                let (id_atras, id_frente) = (
                                    states[atras].driver_id.clone(),
                                    states[frente].driver_id.clone(),
                                );
                                let (nome_atras, nome_frente) =
                                    (atacante.nome.clone(), defensor.nome.clone());
                                let (rel_atras, rel_frente) =
                                    (atacante.car_reliability, defensor.car_reliability);
                                // Contato na disputa: registrado nos DOIS, sem `positions_lost`
                                // — o custo já foi cobrado em tempo logo acima, e somar posições
                                // aqui cobraria duas vezes.
                                empurrar_contato(
                                    &mut states[atras],
                                    &id_frente,
                                    seg_str,
                                    &format!("{nome_atras} bate ao tentar passar {nome_frente}"),
                                    rel_atras,
                                    rng,
                                );
                                empurrar_contato(
                                    &mut states[frente],
                                    &id_atras,
                                    seg_str,
                                    &format!("{nome_frente} leva contato de {nome_atras}"),
                                    rel_frente,
                                    rng,
                                );
                            }
                        }
                    }
                }

                // O TREM: sem ter passado, não se termina o trecho à frente de quem estava na
                // frente. O tempo "perdido" aqui é real — é o tempo que o carro rápido gastou
                // preso. Sem isto, ele atravessaria o pelotão como se a pista estivesse vazia,
                // que é exatamente o que a sonda de grade sorteada media em 0,179.
                //
                // **Só vale para quem estava colado (`na_janela`).** Sem esta condição o
                // bloqueio vira absurdo: um carro que para no box perde 25 s e passaria a
                // ARRASTAR o pelotão inteiro atrás de si, porque todo mundo ficaria preso ao
                // piso do tempo dele. Quem está a segundos de distância não está brigando com
                // ninguém — se o carro da frente tem uma catástrofe, passa-se por ele sem
                // disputa, que é o que acontece na pista.
                if !passou && na_janela {
                    let piso = states[frente].tempo_acumulado_ms + GAP_MINIMO_ENTRE_CARROS_MS;
                    if states[atras].tempo_acumulado_ms < piso {
                        states[atras].tempo_acumulado_ms = piso;
                    }
                }
                // "Preso" = estava na janela, era mais rápido, e mesmo assim não passou.
                let preso = na_janela && mais_rapido && !passou;
                states[atras].trafego.marcar_preso(preso);
            }
        }

        // ── PARADA PLANEJADA (pacote G) ──
        //
        // A parada zera o desgaste de pneu, e é SÓ isso que faz undercut e overcut existirem:
        // a penalidade de pneu gasto de `pontuacao.rs` já cobrava ritmo, e agora dá para
        // comprá-la de volta pagando tempo de box. Nenhuma constante de ritmo nova.
        if modo.tem_pista_viva() {
            for state in states.iter_mut() {
                if state.is_dnf {
                    continue;
                }
                let vence_agora: Vec<u32> = state
                    .paradas
                    .pendentes
                    .iter()
                    .copied()
                    .filter(|v| RaceSegment::from_lap(*v, ctx.total_laps) == segment)
                    .collect();
                for volta in vence_agora {
                    state.paradas.pendentes.retain(|v| *v != volta);
                    executar_parada(state, volta, CUSTO_DE_PARADA_MS);
                }
            }
        }

        ordenar_por_tempo(&mut states);

        // ── SAFETY CAR (pacote G) ──
        //
        // O gatilho já existia e não fazia nada: `derive_caution_segments` derivava a amarela
        // dos incidentes e só registrava. Agora ela tem consequência, e é a maior de todas.
        if modo.tem_safety_car()
            && states
                .iter()
                .flat_map(|s| &s.incidents)
                .any(|inc| inc.segment == seg_str && traz_bandeira_amarela(inc))
        {
            // A volta de entrada: o meio do trecho em que o incidente aconteceu.
            let volta_do_sc = (((segment.ordinal() as f64 + 0.5) * ctx.total_laps as f64 / 5.0)
                .round() as u32)
                .max(1);
            safety_cars.push(volta_do_sc);
            ordem_pre_safety_car.push(
                states
                    .iter()
                    .filter(|s| !s.is_dnf)
                    .map(|s| s.driver_id.clone())
                    .collect(),
            );

            // ZERA OS GAPS. É o que transforma uma liderança confortável em briga — e o que
            // faz do safety car o maior embaralhador do esporte.
            let tempo_do_lider = states
                .iter()
                .find(|s| !s.is_dnf)
                .map(|s| s.tempo_acumulado_ms)
                .unwrap_or(0.0);
            for state in states.iter_mut() {
                if state.is_dnf {
                    continue;
                }
                let gap = state.tempo_acumulado_ms - tempo_do_lider;
                state.tempo_acumulado_ms =
                    tempo_do_lider + atraso_sob_safety_car(gap, state.current_position);
            }

            // REGALA QUEM IA PARAR MESMO: com o pelotão lento, a parada custa uma fração. Quem
            // ainda tinha parada no plano aproveita; quem parou no trecho anterior pagou cheio e
            // perdeu a margem na compressão — é a crucificação, e é ela que dá a frase "ele foi
            // crucificado pelo safety car" em vez de "o dado deu ruim".
            for state in states.iter_mut() {
                if state.is_dnf || state.paradas.pendentes.is_empty() {
                    continue;
                }
                let volta = state.paradas.pendentes.remove(0);
                executar_parada(
                    state,
                    volta_do_sc.max(volta.min(volta_do_sc)),
                    CUSTO_DE_PARADA_MS * FRACAO_DO_CUSTO_SOB_SAFETY_CAR,
                );
            }
            // O trecho seguinte é o relançamento.
            relancamento_no_proximo_trecho = true;
            ordenar_por_tempo(&mut states);
        }

        for (index, state) in states.iter_mut().enumerate() {
            state.current_position = index as i32 + 1;
        }

        // Fecha o par (antes, depois) das paradas feitas neste trecho — é ele que torna o
        // undercut mensurável sem reconstruir a corrida.
        for state in states.iter_mut() {
            let posicao = state.current_position;
            let pendentes = std::mem::take(&mut state.paradas.aguardando_fechamento);
            for _ in pendentes {
                state.paradas.posicao_depois.push(posicao);
            }
        }

        // Retrato do fim do trecho: posição e gap para o carro da frente. É o dado cru que o
        // harness pediu — a posição vivia em `current_position` e morria em
        // `build_race_results`; agora sobrevive um registro por segmento.
        let tempos_ordenados: Vec<(usize, f64)> = states
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.is_dnf)
            .map(|(i, s)| (i, s.tempo_acumulado_ms))
            .collect();
        for (posicao, (i, tempo)) in tempos_ordenados.iter().enumerate() {
            let gap = posicao
                .checked_sub(1)
                .and_then(|p| tempos_ordenados.get(p))
                .map(|(_, tempo_da_frente)| tempo - tempo_da_frente);
            states[*i].trafego.gaps_para_da_frente_ms.push(gap);
        }
        for state in states.iter_mut() {
            state
                .trafego
                .posicoes_por_segmento
                .push(state.current_position);
            // Quem abandonou não tem carro na frente para medir.
            if state.is_dnf {
                state.trafego.gaps_para_da_frente_ms.push(None);
            }
        }
    }

    let mut race_results = build_race_results(drivers, qualifying, ctx, &states, rng);
    let pole_sitter_id = qualifying
        .first()
        .map(|result| result.pilot_id.clone())
        .unwrap_or_default();
    let winner_id = race_results
        .first()
        .map(|result| result.pilot_id.clone())
        .unwrap_or_default();
    let total_incidents: i32 = race_results.iter().map(|r| r.incidents_count).sum();
    let total_dnfs = race_results.iter().filter(|r| r.is_dnf).count() as i32;

    // Aggregate narrative fields
    let main_incident_count: i32 = race_results
        .iter()
        .flat_map(|r| &r.incidents)
        .filter(|i| i.narrative_importance_hint >= 1)
        .count() as i32;

    let notable_incident_pilot_ids: Vec<String> = race_results
        .iter()
        .filter(|r| r.notable_incident.is_some())
        .map(|r| r.pilot_id.clone())
        .collect();

    let most_positions_gained_id = race_results
        .iter()
        .filter(|r| !r.is_dnf && r.positions_gained > 0)
        .max_by_key(|r| r.positions_gained)
        .map(|r| r.pilot_id.clone());

    let caution_segments = derive_caution_segments(race_results.iter().flat_map(|r| &r.incidents));

    RaceResult {
        qualifying_results: qualifying.to_vec(),
        race_results: std::mem::take(&mut race_results),
        pole_sitter_id,
        winner_id,
        fastest_lap_id: String::new(),
        total_laps: ctx.total_laps,
        weather: ctx.weather.as_str().to_string(),
        track_name: ctx.track_name.clone(),
        total_incidents,
        total_dnfs,
        main_incident_count,
        notable_incident_pilot_ids,
        most_positions_gained_id,
        caution_segments,
        applied_mechanicals,
        safety_cars,
        ordem_pre_safety_car,
    }
}
