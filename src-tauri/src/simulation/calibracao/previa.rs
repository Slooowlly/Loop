//! **Movimentos da camada de evento** — os dois jeitos de mexer nas escalas do `forma.rs`, e as
//! contas que os tornam comparáveis.
//!
//! ## O que este arquivo era, e o que sobrou
//!
//! Ele nasceu como **espelho da esteira**: enquanto os modificadores de fim de semana viviam dentro
//! de um `#[tauri::command]`, este harness não os alcançava, e a única forma de medir a camada de
//! evento era replicar a aplicação aqui. O espelho era cópia sem guarda e estava documentado como
//! dívida.
//!
//! **O espelho está apagado.** A esteira virou [`crate::simulation::esteira::aplicar_esteira`], pura
//! e chamável, com as escalas por parâmetro em `forma::EscalasDeForma`. O harness passou a chamar a
//! função do jogo, e o problema de guarda sumiu por construção em vez de virar mais um teste.
//!
//! O que sobrou aqui é o que sempre foi meu e continua sendo: **as duas maneiras de mover a camada
//! de evento**, e a aritmética que permite compará-las de forma honesta.

use crate::simulation::forma::EscalasDeForma;

/// Extensões de [`EscalasDeForma`] para os movimentos da fase 1.
pub trait MovimentosDaCamadaDeEvento {
    /// Multiplica as TRÊS escalas pelo mesmo fator — "aumentar tudo junto", que preserva a
    /// repartição interna onde ela está.
    fn escalando(self, fator: f64) -> Self;

    /// Move peso da afinidade para o acerto **mantendo a soma em variância**. `fracao` é quanto da
    /// variância da afinidade migra (0 = nada, 1 = toda).
    ///
    /// A soma preservada é `afinidade² + acerto²`, não `afinidade + acerto`: as camadas são
    /// independentes e somam em variância, então travar a soma linear mudaria o tamanho total da
    /// camada e as duas pernas da comparação deixariam de ser comparáveis. Foi esta decisão que
    /// tornou o resultado "redistribuir vs escalar" legível.
    fn redistribuindo(self, fracao: f64) -> Self;

    fn com_rho(self, rho: f64) -> Self;
    fn com_forma(self, escala: f64) -> Self;

    /// Desvio-padrão total das três camadas, em pontos de skill. Elas somam em variância.
    fn sigma_total(&self) -> f64;

    /// Fatia da afinidade na variância das três — a grandeza da repartição medida no baseline
    /// (46,8% hoje, contra os 20–30% que a reprodutibilidade recomenda).
    fn fatia_da_afinidade(&self) -> f64;
}

impl MovimentosDaCamadaDeEvento for EscalasDeForma {
    fn escalando(mut self, fator: f64) -> Self {
        self.afinidade *= fator;
        self.forma *= fator;
        self.acerto *= fator;
        self
    }

    fn redistribuindo(mut self, fracao: f64) -> Self {
        let f = fracao.clamp(0.0, 1.0);
        let var_af = self.afinidade * self.afinidade;
        let var_ac = self.acerto * self.acerto;
        let migrada = var_af * f;
        self.afinidade = (var_af - migrada).max(0.0).sqrt();
        self.acerto = (var_ac + migrada).sqrt();
        self
    }

    fn com_rho(mut self, rho: f64) -> Self {
        self.rho = rho;
        self
    }

    fn com_forma(mut self, escala: f64) -> Self {
        self.forma = escala;
        self
    }

    fn sigma_total(&self) -> f64 {
        (self.afinidade * self.afinidade + self.forma * self.forma + self.acerto * self.acerto)
            .sqrt()
    }

    fn fatia_da_afinidade(&self) -> f64 {
        let t = self.sigma_total();
        if t <= f64::EPSILON {
            return 0.0;
        }
        (self.afinidade * self.afinidade) / (t * t)
    }
}

/// Aplica o piso de [`super::assinatura::PISO_DE_FORMA_RHO`].
///
/// **A busca da fase 1 tem que chamar isto em todo ponto que avaliar.** Sem o piso, zerar `rho` é o
/// caminho mais barato para derrubar ρ(N × N+1), o portão do orçamento aprova (a variância continua
/// vindo do lugar certo) e a camada da forma perde a razão de existir sem que nenhuma métrica
/// reclame. Ver [`super::assinatura`] para a classe inteira do problema.
pub fn com_piso_de_assinatura(mut escalas: EscalasDeForma) -> EscalasDeForma {
    escalas.rho = escalas.rho.max(super::assinatura::PISO_DE_FORMA_RHO);
    escalas
}
