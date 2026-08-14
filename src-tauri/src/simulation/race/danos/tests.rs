//! Dano latente: o que acontece quando a avaria manifesta, e o que PARA de acontecer quando
//! ela manifesta como abandono.

use rand::{Error, RngCore};

use crate::simulation::catalog::{IncidentCatalog, VehicleClass};
use crate::simulation::incidents::PendingDamage;
use crate::simulation::race::{RaceSegment, RaceState};

use super::process_pending_damage;

/// RNG de teste que entrega sempre zero e conta quantos números foram pedidos.
///
/// `gen::<f64>()` sobre um `next_u64` zerado dá exatamente `0.0`, que é menor que qualquer
/// chance positiva: todo dano manifesta, e todo dano capaz de abandono vira abandono. É o
/// que torna o caso terminal determinístico sem depender de caçar semente. O contador é o
/// que deixa medir o consumo de RNG, que é justamente o que o corte do laço muda.
struct RngZeradoContador {
    chamadas: usize,
}

impl RngZeradoContador {
    fn novo() -> Self {
        Self { chamadas: 0 }
    }
}

impl RngCore for RngZeradoContador {
    fn next_u32(&mut self) -> u32 {
        self.chamadas += 1;
        0
    }

    fn next_u64(&mut self) -> u64 {
        self.chamadas += 1;
        0
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.chamadas += 1;
        dest.fill(0);
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

fn dano(origem: &str, capaz_de_abandono: bool) -> PendingDamage {
    PendingDamage {
        catalog_id: format!("dano_{origem}"),
        origin_segment: origem.to_string(),
        // Chance 1.0 com o RNG zerado: manifesta sempre.
        manifest_chance: 1.0,
        is_dnf_capable: capaz_de_abandono,
    }
}

fn estado_com_danos(danos: Vec<PendingDamage>) -> RaceState {
    RaceState {
        driver_id: "P1".to_string(),
        tire_wear: 1.0,
        physical_condition: 1.0,
        tempo_acumulado_ms: 0.0,
        desvio_de_ritmo: 0.0,
        trafego: Default::default(),
        paradas: Default::default(),
        is_dnf: false,
        current_position: 1,
        incidents: Vec::new(),
        dnf_reason: None,
        dnf_reason_key: None,
        dnf_segment: None,
        pending_damage: danos,
    }
}

fn processar(states: &mut [RaceState], segment: RaceSegment, rng: &mut RngZeradoContador) {
    // Catálogo VAZIO de propósito: `select_and_render` devolve `None` sem tocar no RNG, e a
    // descrição cai no texto do locale. Com catálogo cheio haveria um sorteio por incidente
    // e a contagem de chamadas deixaria de medir só o que este teste quer medir.
    process_pending_damage(
        states,
        segment,
        &[],
        &IncidentCatalog::empty(),
        VehicleClass::StreetBased,
        false,
        rng,
    );
}

#[test]
fn abandono_por_dano_latente_para_de_manifestar_os_outros_danos() {
    let mut states = vec![estado_com_danos(vec![
        dano("start", true),
        dano("early", true),
        dano("mid", true),
    ])];
    let mut rng = RngZeradoContador::novo();

    processar(&mut states, RaceSegment::Late, &mut rng);

    let state = &states[0];
    assert!(state.is_dnf, "o primeiro dano capaz de abandono é terminal");
    assert_eq!(
        state.incidents.len(),
        1,
        "o carro parado não manifesta os outros danos no mesmo segmento: {:?}",
        state
            .incidents
            .iter()
            .map(|i| i.description.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        state.incidents.iter().filter(|i| i.is_dnf).count(),
        1,
        "no máximo um incidente terminal por manifestação"
    );
}

#[test]
fn abandono_por_dano_latente_nao_reescreve_motivo_nem_segmento() {
    let mut states = vec![estado_com_danos(vec![
        dano("start", true),
        dano("early", true),
    ])];
    let mut rng = RngZeradoContador::novo();

    processar(&mut states, RaceSegment::Mid, &mut rng);

    let motivo = states[0]
        .dnf_reason
        .clone()
        .expect("o abandono carimba o motivo");
    assert_eq!(states[0].dnf_segment, Some(RaceSegment::Mid));
    assert_eq!(
        states[0].incidents[0].description, motivo,
        "o motivo publicado é o do dano que de fato tirou o carro"
    );

    // O segmento seguinte não pode reabrir o assunto: o guarda do topo do laço já barra o
    // carro abandonado, e o dano que sobrou continua parado na fila sem efeito.
    processar(&mut states, RaceSegment::Finish, &mut rng);

    assert_eq!(states[0].dnf_reason.as_deref(), Some(motivo.as_str()));
    assert_eq!(states[0].dnf_segment, Some(RaceSegment::Mid));
    assert_eq!(states[0].incidents.len(), 1);
}

#[test]
fn manifestacao_nao_terminal_segue_processando_a_fila_inteira() {
    // O corte só vale para o abandono. Três danos incapazes de abandono manifestam os três
    // no mesmo segmento, como sempre foi.
    let mut states = vec![estado_com_danos(vec![
        dano("start", false),
        dano("early", false),
        dano("mid", false),
    ])];
    let mut rng = RngZeradoContador::novo();

    processar(&mut states, RaceSegment::Late, &mut rng);

    assert!(!states[0].is_dnf);
    assert_eq!(states[0].incidents.len(), 3);
    assert!(
        states[0].pending_damage.is_empty(),
        "dano manifestado sai da fila"
    );
}

#[test]
fn o_corte_no_abandono_e_o_unico_lugar_em_que_o_consumo_de_rng_muda() {
    // Caminho comum, sem abandono: um sorteio de manifestação por dano, e nada além disso —
    // `is_dnf_capable = false` faz o `&&` curto-circuitar antes do sorteio de abandono.
    let mut sem_abandono = vec![estado_com_danos(vec![
        dano("start", false),
        dano("early", false),
        dano("mid", false),
    ])];
    let mut rng_comum = RngZeradoContador::novo();
    processar(&mut sem_abandono, RaceSegment::Late, &mut rng_comum);
    assert_eq!(
        rng_comum.chamadas, 3,
        "o caminho sem abandono consome exatamente o que sempre consumiu"
    );

    // Caminho terminal: sorteio de manifestação + sorteio de abandono do PRIMEIRO dano, e
    // acabou. Os dois danos restantes não sorteiam mais nada — antes do corte seriam 6
    // chamadas. Este deslocamento é o preço do conserto, e ele só existe no caso que era
    // bugado (piloto com 2+ danos latentes que abandona antes do último).
    let mut com_abandono = vec![estado_com_danos(vec![
        dano("start", true),
        dano("early", true),
        dano("mid", true),
    ])];
    let mut rng_terminal = RngZeradoContador::novo();
    processar(&mut com_abandono, RaceSegment::Late, &mut rng_terminal);
    assert_eq!(rng_terminal.chamadas, 2);

    // E o carro seguinte na lista continua sendo processado normalmente: o corte é do
    // piloto, não do segmento.
    let mut dois_carros = vec![
        estado_com_danos(vec![dano("start", true), dano("early", true)]),
        RaceState {
            driver_id: "P2".to_string(),
            ..estado_com_danos(vec![dano("start", false)])
        },
    ];
    let mut rng_dois = RngZeradoContador::novo();
    processar(&mut dois_carros, RaceSegment::Late, &mut rng_dois);
    assert!(dois_carros[0].is_dnf);
    assert_eq!(dois_carros[1].incidents.len(), 1);
    assert!(!dois_carros[1].is_dnf);
}
