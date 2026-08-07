//! O ENGENHEIRO NA CLASSIFICAÇÃO: o lado que sabe ONDE estamos.
//!
//! A divisão é a mesma do rádio de ritmo, e pelo mesmo motivo. [`crate::engenheiro::classificacao`]
//! decide **o que dizer** e é puro — testável sem SDK nenhum. Aqui fica o que só o monitor sabe:
//! em que volta estamos, se ela é de preparação ou lançada, quanto falta até a linha, e se o
//! carro tocou fora da pista. Um observador que precisasse saber a taxa de amostragem do chamador
//! deixaria de ser testável, e é justamente o que a família de spotter já pagou uma vez.
//!
//! ## O que é uma volta de preparação, na prática
//!
//! Não existe campo do SDK dizendo "esta é a out-lap". O que existe é o boxe: a volta que
//! **começou saindo dele** é a de preparação, e a seguinte é a tentativa. É bookkeeping de duas
//! variáveis, e ele vive aqui porque depende de ver o `on_pit_road` mudar a 60 Hz.
//!
//! ## A volta suja
//!
//! Tocar fora da pista faz duas coisas ao mesmo tempo, e as duas importam: mata a tentativa (o
//! engenheiro pode falar) e desqualifica a volta como referência (ela não pode virar a curva de
//! comparação, senão o delta passaria a medir contra um erro).

use super::*;
use crate::engenheiro::classificacao as cl;

impl RaceMonitor {
    /// Um tique de classificação. Roda a 60 Hz, como o de ritmo, e sai cedo fora da sessão.
    pub(super) fn tick_classificacao(&mut self, t: &IracingTelemetry) {
        // Só na sessão de classificação. Na corrida o rádio é outro, e a melhor volta de lá não
        // vale aqui — outro tanque, outro pneu, outro objetivo.
        if self.qualy_session_num < 0 || t.session_num != self.qualy_session_num {
            return;
        }
        // Sessão nova zera tudo. Sem isto, a curva do classificatório anterior seguiria valendo
        // e o engenheiro cobraria um ritmo que não é desta pista.
        if self.quali_sessao != t.session_num {
            self.quali_sessao = t.session_num;
            self.classificacao.reiniciar();
            self.volta_ref.reiniciar();
            self.classificacao_log.clear();
            self.quali_volta = -1;
            self.quali_saiu_do_box = false;
            self.quali_sujou = false;
        }

        let no_box = t.player_on_pit_road;
        let na_pista = t.on_track && !t.is_replay_playing && !no_box;

        // Cruzou a linha: fecha a volta que acabou e classifica a que começa.
        let volta = t.lap_completed;
        if volta != self.quali_volta {
            let anterior_era_preparacao = self.quali_saiu_do_box;
            let sujou = self.quali_sujou;
            self.quali_volta = volta;
            // A volta que começa é de PREPARAÇÃO se veio do box; se a anterior era preparação,
            // esta é a tentativa.
            self.quali_saiu_do_box = no_box;
            self.quali_sujou = false;

            // A volta que fechou só vira referência se foi uma tentativa limpa. Volta de
            // preparação é lenta por definição, e volta suja mediria contra um erro.
            if !anterior_era_preparacao && !sujou && t.last_lap_time > 0.0 {
                self.volta_ref.fechar_volta(t.last_lap_time);
            } else {
                self.volta_ref.descartar_volta();
            }
        }

        // Fora da pista SUJA a volta e não é reversível: a tentativa já morreu, e voltar para o
        // asfalto não a ressuscita.
        if na_pista && t.track_surface != SURFACE_ON_TRACK {
            self.quali_sujou = true;
        }
        if no_box {
            // No box não há volta a desenhar, e a próxima começa do zero.
            self.quali_saiu_do_box = true;
            self.volta_ref.descartar_volta();
            return;
        }
        if !na_pista {
            return;
        }

        let em_preparacao = self.quali_saiu_do_box;
        let voando = !em_preparacao;

        // Só a volta LANÇADA e limpa alimenta a curva. Desenhar a de preparação faria a
        // referência ser o passeio de saída do box.
        if voando && !self.quali_sujou {
            self.volta_ref.amostrar(t.lap_dist_pct, t.lap_current_time);
        }

        let fala = self.classificacao.observar(cl::Momento {
            ate_a_linha_s: tempo_ate_a_linha_s(t),
            restante_s: t.session_time_remain.max(0.0),
            volta_referencia_s: self.volta_ref.melhor_s(),
            // Duas mortes possíveis, e as duas contam: o erro visível (saiu da pista) e o
            // invisível (o delta abriu). A primeira não precisa de referência nenhuma, e é por
            // isso que ela existe — na PRIMEIRA tentativa não há curva contra a que comparar.
            volta_morta: voando
                && (self.quali_sujou
                    || self.volta_ref.volta_morta(t.lap_dist_pct, t.lap_current_time)),
            em_preparacao,
            voando,
        });
        if let Some(f) = fala {
            self.classificacao_log.push(f);
        }
    }
}

/// Segundos até cruzar a linha, no ritmo atual.
///
/// É a conta que decide QUANDO a despedida começa, e ela é feita por distância e não por relógio:
/// o que resta da volta em metros, dividido pela velocidade. `f64::INFINITY` quando não dá para
/// saber — carro parado, pista sem comprimento —, e o observador trata isso como "não fale".
fn tempo_ate_a_linha_s(t: &IracingTelemetry) -> f64 {
    let comprimento = crate::iracing_sdk::spotter_frente::comprimento_m();
    if comprimento <= 0.0 || t.speed_ms <= 1.0 {
        return f64::INFINITY;
    }
    let falta_m = (1.0 - t.lap_dist_pct.clamp(0.0, 1.0)) * comprimento;
    falta_m / t.speed_ms
}
