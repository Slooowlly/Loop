//! As tabelas de risco: chances-base, o quanto cada segmento e cada condição de pista pesam,
//! e os fatores derivados (risco de lesão e importância narrativa) que carimbam o incidente.

use crate::models::enums::WeatherCondition;

use crate::simulation::race::RaceSegment;

use super::tipos::{IncidentResult, IncidentSeverity, IncidentType};

pub(crate) const MECHANICAL_BASE_CHANCE: f64 = 0.015;
pub(crate) const DRIVER_ERROR_BASE_CHANCE: f64 = 0.017;
pub(crate) const COLLISION_BASE_CHANCE: f64 = 0.006;
pub(crate) const SEGMENTS: f64 = 5.0;

// ── Os números INTERNOS dos sorteios ──────────────────────────────────────────
//
// Estavam soltos no corpo de `sorteios.rs`, o que os deixava fora do alcance da varredura
// de knobs de `calibracao::busca`: ela enxerga o que tem nome e mora numa tabela de
// constantes. O risco não é estético — se a busca fechar os knobs externos com um destes
// errado por baixo, o erro fica travado embaixo de uma calibração "boa".
//
// Nenhum valor mudou ao ganhar nome. O que muda é que agora dá para vê-los.

/// Pivô de confiabilidade do carro: acima dele a pane fica mais rara, abaixo mais comum.
/// A escala é `(rel − pivô)/ESCALA × PESO`, e o resultado é limitado a `[0,1; 3,0]`.
pub(crate) const CONFIABILIDADE_PIVO: f64 = 70.0;
/// Quantos pontos de confiabilidade valem uma unidade inteira do efeito.
pub(crate) const CONFIABILIDADE_ESCALA: f64 = 25.0;
/// Quanto do risco a confiabilidade chega a mover, no máximo.
pub(crate) const CONFIABILIDADE_PESO: f64 = 0.70;

/// Fatia das panes que sai LEVE (perde posições e segue). O resto tira o carro.
pub(crate) const FRACAO_DE_PANE_LEVE: f64 = 0.15;
/// Fatia dos erros de pilotagem que sai LEVE (rodada, susto). O resto tira o carro.
pub(crate) const FRACAO_DE_ERRO_LEVE: f64 = 0.70;
/// Posições perdidas por um incidente leve, sorteadas nesta faixa (inclusiva).
pub(crate) const POSICOES_PERDIDAS_NO_LEVE: (i32, i32) = (1, 4);

/// Quanto do castigo da chuva o `fator_chuva` do piloto chega a absorver.
pub(crate) const ABSORCAO_DE_CHUVA_PELO_PILOTO: f64 = 0.80;

/// Teto do risco de erro de pilotagem por segmento, depois de todos os multiplicadores.
pub(crate) const TETO_DE_RISCO_DE_ERRO: f64 = 0.25;
/// Teto do risco de colisão por segmento, depois de todos os multiplicadores.
pub(crate) const TETO_DE_RISCO_DE_COLISAO: f64 = 0.20;

// ── Segunda leva: o que ainda estava inline em `sorteios.rs` ─────────────────────────────
//
// A primeira leva pegou os números que a vistoria citou por nome (pivô de confiabilidade,
// fração de pane leve, absorção de chuva). Estes são o resto do mesmo problema: cada um deles
// é um fator de uma multiplicação de oito termos, e num produto assim **nenhum coeficiente é
// legível sozinho** — só dá para julgar `0.30` sabendo que ele divide espaço com `0.25`, com
// `0.5` e com `0.4`. Enfileirados aqui, dá.
//
// Nenhum valor mudou. Como o resto do bloco, **nenhum foi medido**.

/// Quanto a experiência do piloto chega a reduzir o erro de pilotagem.
pub(crate) const EXPERIENCIA_REDUZ_O_ERRO: f64 = 0.30;
/// Quanto a suavidade chega a reduzir o erro de pilotagem, por cima da experiência.
pub(crate) const SUAVIDADE_REDUZ_O_ERRO: f64 = 0.25;
/// Divisor da agressividade no núcleo do erro: 200 faz o piloto de agressão máxima errar 1,5×
/// mais que o de agressão zero.
pub(crate) const DIVISOR_DA_AGRESSAO_NO_ERRO: f64 = 200.0;
/// Quanto o pneu no fim da vida chega a somar ao risco de erro.
pub(crate) const PNEU_GASTO_SOMA_AO_ERRO: f64 = 0.5;
/// Quanto a exaustão chega a somar ao risco de erro.
pub(crate) const FADIGA_SOMA_AO_ERRO: f64 = 0.4;

/// Quanto a agressividade chega a somar ao risco de COLISÃO.
pub(crate) const AGRESSAO_SOMA_A_COLISAO: f64 = 0.60;
/// Quanto o `racecraft` chega a subtrair do risco de colisão.
pub(crate) const RACECRAFT_TIRA_DA_COLISAO: f64 = 0.50;
/// Quanto a agressividade MÉDIA DOS VIZINHOS chega a somar ao risco de colisão. É o termo que
/// faz correr no meio de um bando agressivo ser perigoso mesmo para quem não é.
pub(crate) const VIZINHANCA_SOMA_A_COLISAO: f64 = 0.30;

/// Multiplicador de colisão pela FAIXA do pelotão em que o carro está. O meio é o lugar
/// perigoso: na frente há espaço, no fundo o pelotão já esticou.
pub(crate) const COLISAO_NA_PONTA: f64 = 0.7;
pub(crate) const COLISAO_NO_MEIO: f64 = 1.2;
pub(crate) const COLISAO_NO_FUNDO: f64 = 0.9;
/// Fronteiras (em fração da posição sobre o grid) das três faixas acima.
pub(crate) const FRONTEIRA_DA_PONTA: f64 = 0.25;
pub(crate) const FRONTEIRA_DO_MEIO: f64 = 0.75;

/// Repartição da GRAVIDADE de uma colisão, em fatias acumuladas: abaixo da primeira é leve,
/// entre as duas é grave, acima é crítica.
pub(crate) const COLISAO_ATE_AQUI_E_LEVE: f64 = 0.55;
pub(crate) const COLISAO_ATE_AQUI_E_GRAVE: f64 = 0.95;

/// Repartição da CONSEQUÊNCIA de uma colisão, em fatias acumuladas: abaixo da primeira tira o
/// carro; entre as duas custa muitas posições; acima custa poucas.
pub(crate) const COLISAO_ATE_AQUI_TIRA_O_CARRO: f64 = 0.40;
pub(crate) const COLISAO_ATE_AQUI_CUSTA_CARO: f64 = 0.70;
/// Posições perdidas na colisão cara e na barata (faixas inclusivas).
pub(crate) const POSICOES_PERDIDAS_NA_COLISAO_CARA: (i32, i32) = (3, 5);
pub(crate) const POSICOES_PERDIDAS_NA_COLISAO_BARATA: (i32, i32) = (1, 2);

/// Raio, em posições, da vizinhança que conta para a agressividade média do entorno.
pub(crate) const RAIO_DA_VIZINHANCA: i32 = 2;
/// Agressividade suposta quando não há vizinho nenhum — o meio da escala.
pub(crate) const AGRESSAO_NEUTRA: f64 = 50.0;

pub(crate) fn mechanical_segment_mult(segment: RaceSegment) -> f64 {
    match segment {
        RaceSegment::Start => 0.5,
        RaceSegment::Early => 0.8,
        RaceSegment::Mid => 1.0,
        RaceSegment::Late => 1.2,
        RaceSegment::Finish => 1.5,
    }
}

pub(crate) fn driver_error_segment_mult(segment: RaceSegment) -> f64 {
    match segment {
        RaceSegment::Start => 1.5,
        RaceSegment::Early => 1.0,
        RaceSegment::Mid => 1.0,
        RaceSegment::Late => 1.2,
        RaceSegment::Finish => 1.5,
    }
}

pub(crate) fn collision_segment_mult(segment: RaceSegment) -> f64 {
    match segment {
        RaceSegment::Start => 2.5,
        RaceSegment::Early => 1.0,
        RaceSegment::Mid => 0.8,
        RaceSegment::Late => 0.8,
        RaceSegment::Finish => 1.2,
    }
}

pub(crate) fn rain_base(weather: WeatherCondition) -> f64 {
    match weather {
        WeatherCondition::Dry => 0.0,
        WeatherCondition::Damp => 0.30,
        WeatherCondition::Wet => 0.60,
        WeatherCondition::HeavyRain => 1.00,
    }
}

pub(crate) fn rain_collision_mult(weather: WeatherCondition) -> f64 {
    match weather {
        WeatherCondition::Dry => 1.0,
        WeatherCondition::Damp => 1.2,
        WeatherCondition::Wet => 1.4,
        WeatherCondition::HeavyRain => 1.6,
    }
}

pub(crate) fn compute_irm(incident_type: IncidentType, severity: IncidentSeverity) -> f64 {
    match (incident_type, severity) {
        (IncidentType::Collision, IncidentSeverity::Critical) => 1.5,
        (IncidentType::Collision, IncidentSeverity::Major) => 0.45,
        (IncidentType::DriverError, IncidentSeverity::Critical) => 1.0,
        (IncidentType::Mechanical, IncidentSeverity::Critical) => 0.6,
        _ => 0.0,
    }
}

pub(crate) fn injury_base_chance(incident_type: IncidentType) -> f64 {
    match incident_type {
        IncidentType::Collision => 0.50,
        IncidentType::DriverError => 0.40,
        IncidentType::Mechanical => 0.25,
    }
}

/// IRM do **contato de disputa** (`race::motor::empurrar_contato`) — o encostão de uma
/// tentativa de ultrapassagem que não deu certo.
///
/// Não sai de [`compute_irm`] de propósito: pela tabela, `(Collision, Minor)` é 0.0, e um
/// roda-roda de disputa TEM de poder machucar. Mas ele também não pode valer o que valia
/// antes — o call site cravava `1.0`, o que dava `0.50 × 1.0` = **50%** de lesão por
/// contato, em CADA um dos dois carros. Como o contato é tráfego normal (5% das tentativas
/// falhas, até 35% no piloto agressivo, cinco segmentos por corrida), aquilo sozinho
/// respondia pela enchente de lesões: 11,7% das largadas terminavam com piloto machucado.
///
/// `0.40 × 0.50` = **20% por carro por contato** — abaixo da batida crítica (70%) e do erro
/// crítico (40%), acima da pane crítica (15%).
pub(crate) const IRM_CONTATO_DE_DISPUTA: f64 = 0.40;

/// IRM do **dano latente que vira abandono** (`race::danos::process_pending_damage`): o carro
/// que sobreviveu à batida, carregou a avaria e desistiu voltas depois.
///
/// Também fora de [`compute_irm`] — o incidente é carimbado `(Mechanical, Major)`, que na
/// tabela é 0.0, mas o carro está abandonando por consequência direta de uma colisão. O valor
/// é o MESMO da pane crítica (`0.60 × 0.25` = 15%): o piloto não sofre um impacto novo aqui,
/// só recolhe um carro quebrado. Antes o call site cravava `1.5` (37,5%) — mais que a pane
/// crítica, sem nada que justificasse.
pub(crate) const IRM_DANO_LATENTE_COM_ABANDONO: f64 = 0.60;

/// IRM do **dano latente que só custa posições** — o carro segue na pista, torto, e perde
/// terreno. `0.20 × 0.25` = **5%**: existe (o carro avariado ainda pode morder), mas é o
/// menor da escala, abaixo do abandono (15%) e do contato (20%). Antes o call site cravava
/// `1.0` (25%), o que fazia andar avariado machucar mais que bater.
pub(crate) const IRM_DANO_LATENTE_SEM_ABANDONO: f64 = 0.20;

/// Com que frequência um dano latente **capaz de abandono** de fato tira o carro, quando se
/// manifesta. O resto das manifestações custa só posições.
///
/// Era `0.70` — abandonar era mais que o DOBRO de comum que perder posições, o inverso do que
/// a corrida faz: o normal é o carro avariado seguir andando mal. Em `0.35`, perder posições
/// (65%) volta a ser o desfecho comum e o abandono (35%) o desfecho ruim.
pub(crate) const CHANCE_DE_ABANDONO_NA_MANIFESTACAO: f64 = 0.35;

/// Chance de um **contato de disputa** deixar o carro avariado (dano latente), antes da
/// modulação por confiabilidade. Mesmo patamar da colisão `Minor` em
/// `segmento::maybe_add_pending_damage` — um encostão torto é um encostão torto, venha ele da
/// tentativa de ultrapassagem ou do sorteio de colisão do segmento.
pub(crate) const CHANCE_DE_DANO_NO_CONTATO: f64 = 0.25;

/// Chance INICIAL de o dano de um contato se manifestar no segmento seguinte (sobe +0.15 por
/// segmento que passa sem manifestar, como todo dano latente). Um pouco abaixo do 0.20 da
/// colisão de verdade: o encostão avaria menos.
pub(crate) const MANIFESTACAO_INICIAL_DO_DANO_DE_CONTATO: f64 = 0.15;

pub(crate) fn compute_narrative_hint(
    severity: IncidentSeverity,
    incident_type: IncidentType,
) -> u8 {
    match (severity, incident_type) {
        (IncidentSeverity::Critical, _) => 2,
        (IncidentSeverity::Major, IncidentType::Collision) => 1,
        _ => 0,
    }
}

pub(crate) fn make_incident(
    pilot_id: String,
    incident_type: IncidentType,
    severity: IncidentSeverity,
    segment: &str,
    positions_lost: i32,
    is_dnf: bool,
    description: String,
    linked_pilot_id: Option<String>,
    is_two_car_incident: bool,
    catalog_id: Option<String>,
    damage_origin_segment: Option<String>,
) -> IncidentResult {
    IncidentResult {
        injury_risk_multiplier: compute_irm(incident_type, severity),
        narrative_importance_hint: compute_narrative_hint(severity, incident_type),
        pilot_id,
        incident_type,
        severity,
        segment: segment.to_string(),
        positions_lost,
        is_dnf,
        description,
        linked_pilot_id,
        is_two_car_incident,
        catalog_id,
        damage_origin_segment,
    }
}
