//! **Posição na pista**: ar sujo, ultrapassagem como tentativa, e o trem de carros.
//!
//! O que este módulo conserta. Duas medições independentes concordavam: com a grade sorteada
//! ao acaso, ρ(grid × chegada) era 0,179 enquanto ρ(skill × chegada) era 0,885. Lido junto,
//! isso não dizia "largar importa pouco" — dizia que **atravessar o pelotão inteiro não
//! custava nada**. A posição saía de ordenar ritmo absoluto, e ritmo absoluto não sabe que
//! existe um carro na frente.
//!
//! A peça que faltava era o gap entre carros na moeda certa, e ela chegou com a troca de
//! moeda para tempo. Aqui ela vira três regras:
//!
//! 1. **Ar sujo.** Perseguidor dentro da janela perde apoio e anda mais devagar. Quanto
//!    perde depende de quanto AQUELE carro vive de aerodinâmica — o mesmo eixo de
//!    `car_build::vies_de_pico` (apoio × ponta), reaproveitado em vez de reinventado.
//! 2. **Ultrapassagem é TENTATIVA, não ordenação.** Sai de delta de ritmo, `racecraft` do
//!    atacante contra `defesa` do defensor, `aggression` e a dificuldade da pista. Falhar
//!    custa tempo aos dois e pode virar incidente.
//! 3. **O trem.** Quem não passou não pode terminar o trecho à frente de quem estava na
//!    frente. É esta regra, e não as outras duas, que faz o carro rápido EMPILHAR atrás do
//!    lento em vez de atravessá-lo como se a pista estivesse vazia.
//!
//! Duas leituras honestas sobre o efeito disto nas métricas, para não serem confundidas com
//! regressão: ρ(grid × chegada) SOBE (é o objetivo — em corrida de verdade a posição na
//! pista importa muito) e ρ(skill × chegada) DESCE. E o sintoma que originou o projeto,
//! ρ(etapa N × N+1), quase não se move: este módulo não conserta o campeonato travado, ele
//! dá ALAVANCA para os mecanismos que já existem. Quando ultrapassar custa caro, um grid
//! embaralhado por acerto de fim de semana ou por volta perdida deixa de ser cosmético.

use rand::Rng;

use crate::simulation::context::SimDriver;

// ─────────────────────── Constantes (PROVISÓRIAS) ───────────────────────
//
// Nenhuma delas foi calibrada: são pontos de partida legíveis. Quem fecha os valores é o
// harness estatístico, que está construindo a busca sobre este espaço em paralelo.

/// Distância até o carro da frente, em ms, dentro da qual se perde apoio aerodinâmico.
pub const JANELA_AR_SUJO_MS: f64 = 1_000.0;

/// Perda máxima de ritmo (pontos) no carro totalmente colado e totalmente dependente de
/// apoio. Cai linearmente até zero na borda da janela.
pub const PERDA_MAXIMA_AR_SUJO_PONTOS: f64 = 3.0;

/// Espaçamento mínimo entre dois carros em fila, em ms. É o "não dá para atravessar".
pub const GAP_MINIMO_ENTRE_CARROS_MS: f64 = 150.0;

/// Distância até o carro da frente, em ms, dentro da qual dá para TENTAR passar.
pub const JANELA_DE_ATAQUE_MS: f64 = 800.0;

/// Chance de uma tentativa dar certo quando os dois carros têm exatamente o mesmo ritmo e
/// habilidades iguais, em pista de dificuldade neutra.
pub const PROB_BASE_ULTRAPASSAGEM: f64 = 0.35;

/// Delta de ritmo (pontos) que satura a chance de passar — acima disto o atacante é tão mais
/// rápido que a diferença de habilidade quase não segura mais.
pub const DELTA_DE_RITMO_QUE_SATURA: f64 = 6.0;

/// Quanto a diferença `racecraft − defesa` move a chance, no máximo (fração).
pub const PESO_DA_HABILIDADE_NA_ULTRAPASSAGEM: f64 = 0.60;

/// Quanto a agressividade move a chance, no máximo (fração). O outro lado da moeda está no
/// risco de incidente: agressivo passa mais e bate mais.
pub const PESO_DA_AGRESSIVIDADE_NA_ULTRAPASSAGEM: f64 = 0.30;

/// Tempo perdido pelo atacante numa tentativa que não deu certo, em ms.
pub const CUSTO_TENTATIVA_FALHA_ATACANTE_MS: f64 = 350.0;

/// Tempo perdido pelo defensor ao se defender, em ms. Menor que o do atacante: defender é
/// mais barato que atacar, mas não é de graça.
pub const CUSTO_TENTATIVA_FALHA_DEFENSOR_MS: f64 = 150.0;

/// Chance de uma tentativa falha virar contato, no piloto de agressividade média.
pub const RISCO_DE_CONTATO_NA_TENTATIVA_FALHA: f64 = 0.05;

// ─────────────────────── Ar sujo ───────────────────────

/// Sensibilidade deste carro ao ar sujo, em [0, 1], a partir do viés de pico do shape.
///
/// O eixo é o mesmo de `car_build::vies_de_pico`: positivo = carro de apoio (handling), que
/// vive de aerodinâmica e sofre atrás de outro; negativo = carro de ponta/eficiência
/// (power), que perde menos apoio porque dependia menos dele. Um carro balanceado fica no
/// meio da faixa.
pub fn sensibilidade_ao_ar_sujo(vies_de_pico: f64) -> f64 {
    (0.5 + 0.5 * vies_de_pico.clamp(-1.0, 1.0)).clamp(0.0, 1.0)
}

/// Ritmo (pontos) que este carro perde por estar no ar sujo do carro da frente.
///
/// Zero fora da janela; máximo colado. `gap_ms` é a distância para o carro da frente.
pub fn perda_por_ar_sujo(gap_ms: f64, vies_de_pico: f64) -> f64 {
    if !(0.0..JANELA_AR_SUJO_MS).contains(&gap_ms) {
        return 0.0;
    }
    let proximidade = 1.0 - gap_ms / JANELA_AR_SUJO_MS;
    PERDA_MAXIMA_AR_SUJO_PONTOS * proximidade * sensibilidade_ao_ar_sujo(vies_de_pico)
}

/// Que FRAÇÃO de um trecho este carro passa dentro de uma janela do carro da frente.
///
/// Sem isto, o modelo só enxergaria o gap na foto do INÍCIO do trecho e perderia tudo o que
/// acontece dentro dele — que é justamente onde a corrida acontece. Um carro 3 s atrás,
/// fechando 1 s por trecho, nunca apareceria como "em disputa" numa amostragem por
/// fronteira de segmento, mas passa metade do trecho seguinte colado.
///
/// Modelo: o gap varia LINEARMENTE ao longo do trecho, de `gap_entrada_ms` até
/// `gap_entrada_ms − fechamento_ms`. A fração devolvida é a parte do trecho em que o gap
/// fica em `[0, janela)`. Fechamento negativo = o carro da frente está indo embora.
pub fn fracao_do_trecho_na_janela(gap_entrada_ms: f64, fechamento_ms: f64, janela_ms: f64) -> f64 {
    if gap_entrada_ms < 0.0 || janela_ms <= 0.0 {
        return 0.0;
    }
    // Sem aproximação, só vale quem já estava dentro.
    if fechamento_ms <= 0.0 {
        return if gap_entrada_ms < janela_ms { 1.0 } else { 0.0 };
    }
    // Entra na janela quando o gap cai abaixo dela; sai quando alcança (gap zero).
    let entra = ((gap_entrada_ms - janela_ms) / fechamento_ms).clamp(0.0, 1.0);
    let sai = (gap_entrada_ms / fechamento_ms).clamp(0.0, 1.0);
    (sai - entra).max(0.0)
}

// ─────────────────────── Ultrapassagem ───────────────────────

/// Chance de o atacante concluir a ultrapassagem nesta tentativa.
///
/// `delta_de_ritmo` é o ritmo LIMPO do atacante menos o do defensor, em pontos (positivo =
/// atacante mais rápido). `dificuldade` é o `overtaking_difficulty_multiplier` do perfil da
/// pista: acima de 1 é pista onde não se passa.
pub fn prob_de_ultrapassagem(
    delta_de_ritmo: f64,
    racecraft: f64,
    defesa: f64,
    aggression: f64,
    dificuldade: f64,
) -> f64 {
    // Sem vantagem de ritmo, não há de onde tirar a ultrapassagem.
    if delta_de_ritmo <= 0.0 {
        return 0.0;
    }
    let vantagem = (delta_de_ritmo / DELTA_DE_RITMO_QUE_SATURA).clamp(0.0, 1.0);
    let habilidade = ((racecraft - defesa) / 100.0).clamp(-1.0, 1.0);
    let agressivo = (aggression.clamp(0.0, 100.0) - 50.0) / 50.0;

    let p = PROB_BASE_ULTRAPASSAGEM
        * vantagem
        * (1.0 + PESO_DA_HABILIDADE_NA_ULTRAPASSAGEM * habilidade)
        * (1.0 + PESO_DA_AGRESSIVIDADE_NA_ULTRAPASSAGEM * agressivo)
        / dificuldade.max(0.1);

    p.clamp(0.0, 0.95)
}

/// Chance de a tentativa falha terminar em contato entre os dois.
pub fn prob_de_contato(aggression_atacante: f64, dificuldade: f64) -> f64 {
    let agressivo = (aggression_atacante.clamp(0.0, 100.0) - 50.0) / 50.0;
    (RISCO_DE_CONTATO_NA_TENTATIVA_FALHA * (1.0 + 0.8 * agressivo) * dificuldade.max(0.1))
        .clamp(0.0, 0.35)
}

/// O que uma tentativa de ultrapassagem produziu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesfechoDaTentativa {
    /// Passou: o atacante fica livre do trem neste trecho.
    Concluida,
    /// Não passou: os dois perdem tempo e o atacante segue preso.
    Falhou,
    /// Não passou e houve contato.
    FalhouComContato,
}

/// Rola uma tentativa de ultrapassagem.
#[allow(clippy::too_many_arguments)]
pub fn tentar_ultrapassagem(
    atacante: &SimDriver,
    defensor: &SimDriver,
    delta_de_ritmo: f64,
    dificuldade: f64,
    incidentes_ligados: bool,
    rng: &mut impl Rng,
) -> DesfechoDaTentativa {
    let p = prob_de_ultrapassagem(
        delta_de_ritmo,
        atacante.racecraft as f64,
        defensor.defesa as f64,
        atacante.aggression as f64,
        dificuldade,
    );
    if rng.gen::<f64>() < p {
        return DesfechoDaTentativa::Concluida;
    }
    if incidentes_ligados
        && rng.gen::<f64>() < prob_de_contato(atacante.aggression as f64, dificuldade)
    {
        return DesfechoDaTentativa::FalhouComContato;
    }
    DesfechoDaTentativa::Falhou
}

// ─────────────────────── Observáveis ───────────────────────

/// Dado CRU de tráfego de um carro ao longo da corrida. Métrica é do harness — aqui só sai o
/// que aconteceu, sem agregação nem interpretação.
#[derive(Debug, Clone, Default)]
pub struct TrafegoDoCarro {
    /// Posição ao fim de cada um dos 5 segmentos.
    pub posicoes_por_segmento: Vec<i32>,
    /// Gap para o carro da frente ao fim de cada segmento, em ms. `None` = era o líder.
    pub gaps_para_da_frente_ms: Vec<Option<f64>>,
    /// Em quantos segmentos terminou dentro da janela de ar sujo.
    pub segmentos_em_ar_sujo: i32,
    /// Quantas vezes tentou passar alguém.
    pub tentativas_ultrapassagem: i32,
    /// Quantas dessas tentativas vieram.
    pub ultrapassagens_concluidas: i32,
    /// Quantas vezes foi atacado.
    pub tentativas_sofridas: i32,
    /// Maior sequência de segmentos CONSECUTIVOS preso atrás de um carro mais lento
    /// (dentro da janela, com ritmo melhor que o dele, e sem conseguir passar).
    pub maior_sequencia_preso: i32,
    /// Contador vivo da sequência atual — não é observável, é estado do laço.
    pub sequencia_preso_atual: i32,
}

impl TrafegoDoCarro {
    /// Registra mais um segmento preso (ou quebra a sequência).
    pub fn marcar_preso(&mut self, preso: bool) {
        if preso {
            self.sequencia_preso_atual += 1;
            self.maior_sequencia_preso = self.maior_sequencia_preso.max(self.sequencia_preso_atual);
        } else {
            self.sequencia_preso_atual = 0;
        }
    }
}

// `#[path]` explícito: este módulo é carregado por `#[path = "race/trafego.rs"]`, então o
// diretório dos filhos dele é `race/` e não `race/trafego/`. Sem esta linha, o `mod tests`
// daqui resolveria para `race/tests/mod.rs` — o arquivo de testes da CORRIDA — e o
// sequestraria para dentro deste módulo.
#[cfg(test)]
#[path = "trafego/tests.rs"]
mod tests;
