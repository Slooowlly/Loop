//! **O diretor do disparo ao vivo** — o quarto papel de `car::breakdown`, e o único que tem
//! estado que atravessa voltas.
//!
//! Os outros três (o modelo de hazard, as condições de pista e clima, as regras de enduro)
//! são funções puras de uma volta só. Este guarda a grade inteira: um [`LiveBreakdown`] por
//! carro, indexado pelo número no iRacing, mais a última volta processada de cada um. É por
//! isso que ele vive num arquivo próprio — misturado com as funções puras, era a única parte
//! do módulo que precisava ser lida com a cabeça em "quem mudou o quê e quando".

use super::{BreakdownEvent, LiveBreakdown, Weather};
use crate::car::PartType;

/// Orquestra o disparo AO VIVO na grade toda: guarda um [`LiveBreakdown`] por carro (pelo
/// NÚMERO no iRacing) e, quando o monitor avisa que um carro completou uma volta, avança o
/// estado dele com o clima daquela volta e devolve os [`BreakdownEvent`] que aconteceram —
/// o monitor então dispara `ev.command(car_number)` (`!black`/`!dq`) e registra. Puro e
/// testável; a fiação viva (verde→montar a grade; volta→`on_lap`→`send_chat_text`) fica no
/// `race_monitor`. Idempotente por volta: só avança pra frente (reprocessar a mesma volta
/// não dispara de novo).
/// `Clone` porque o monitor guarda uma cópia PRISTINA do diretor no instante da instalação: ao
/// reiniciar a corrida, o estado de desgaste que as voltas da tentativa abandonada consumiram
/// tem de ser jogado fora, e a única forma honesta de fazer isso é voltar ao que foi montado.
#[derive(Debug, Default, Clone)]
pub struct BreakdownDirector {
    cars: std::collections::HashMap<u32, DirectorCar>,
}

#[derive(Debug, Clone)]
struct DirectorCar {
    live: LiveBreakdown,
    /// Última volta já processada deste carro (dedupe).
    last_lap: u32,
    /// Voltas de parada no box (enduro) — troca de peças gastas.
    service_laps: Vec<u32>,
}

impl BreakdownDirector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Nenhum carro montado (grade não carregada) → o monitor não dispara nada.
    pub fn is_empty(&self) -> bool {
        self.cars.is_empty()
    }

    /// Registra um carro do grid pelo número no iRacing (o `LiveBreakdown` já vem com o
    /// desgaste de entrada + proteção do jogador aplicados). `service_laps` vazio em sprints.
    pub fn add_car(&mut self, car_number: u32, live: LiveBreakdown, service_laps: Vec<u32>) {
        self.cars.insert(
            car_number,
            DirectorCar {
                live,
                last_lap: 0,
                service_laps,
            },
        );
    }

    /// Troca a sorte de TODOS os carros montados (ver [`LiveBreakdown::reroll_luck`]). O monitor
    /// chama isto ao restaurar o diretor num reinício de sessão, pra a corrida refeita não ser a
    /// repetição peça-por-peça, volta-por-volta da que o jogador jogou fora.
    pub fn reroll_luck(&mut self, salt: u64) {
        for car in self.cars.values_mut() {
            car.live.reroll_luck(salt);
        }
    }

    /// Peças de `car_number` atualmente na janela de risco (ver [`LiveBreakdown::parts_in_danger`]).
    /// Vazio se o carro não está montado.
    pub fn car_parts_in_danger(&self, car_number: u32) -> Vec<(usize, PartType, f64)> {
        self.cars
            .get(&car_number)
            .map(|c| c.live.parts_in_danger())
            .unwrap_or_default()
    }

    /// Marca a volta de PARTIDA de um carro já montado (evita processar as voltas anteriores).
    /// Útil quando a grade é montada com a corrida já em andamento — a avaliação começa da
    /// PRÓXIMA volta cruzada, não retroage.
    pub fn prime_lap(&mut self, car_number: u32, lap: u32) {
        if let Some(car) = self.cars.get_mut(&car_number) {
            car.last_lap = lap;
        }
    }

    /// O carro `car_number` completou a volta `lap` (1-based). Avança o estado dele até essa
    /// volta com `weather` (o clima vivo do momento) e devolve os eventos de quebra que
    /// aconteceram. Volta ≤ já processada → nada (dedupe). Carro fora (DNF) → nada.
    pub fn on_lap(&mut self, car_number: u32, lap: u32, weather: Weather) -> Vec<BreakdownEvent> {
        self.on_lap_at(car_number, lap, weather, 0.0)
    }

    /// Igual a [`BreakdownDirector::on_lap`], com o `progress` (0..1) da corrida — o enduro usa
    /// pra a rampa de desgaste do fim. O monitor passa o progresso VIVO (tempo de sessão); as
    /// voltas em atraso reaproveitam o mesmo progresso (aproximação suave, a rampa é contínua).
    pub fn on_lap_at(
        &mut self,
        car_number: u32,
        lap: u32,
        weather: Weather,
        progress: f64,
    ) -> Vec<BreakdownEvent> {
        self.on_lap_at_cfg(car_number, lap, weather, progress, true)
    }

    /// Igual a [`BreakdownDirector::on_lap_at`], com o interruptor da FALHA (ver
    /// [`LiveBreakdown::advance_lap_at_cfg`]). O monitor passa `allow_break = false` na
    /// classificatória e na carência de largada: as voltas são avançadas (o carro gasta), mas
    /// nenhuma peça larga.
    pub fn on_lap_at_cfg(
        &mut self,
        car_number: u32,
        lap: u32,
        weather: Weather,
        progress: f64,
        allow_break: bool,
    ) -> Vec<BreakdownEvent> {
        let mut out = Vec::new();
        if let Some(car) = self.cars.get_mut(&car_number) {
            while car.last_lap < lap && !car.live.is_out() {
                car.last_lap += 1;
                if car.service_laps.contains(&car.last_lap) {
                    car.live.service_pit();
                }
                out.extend(car.live.advance_lap_at_cfg(
                    car.last_lap,
                    weather,
                    progress,
                    allow_break,
                ));
            }
        }
        out
    }
}
