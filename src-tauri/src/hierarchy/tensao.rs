//! Núcleo PURO do eixo de tensão N1/N2 (Passo 7): os deltas de uma rodada e o
//! desfecho do duelo que os alimenta.
//!
//! Está separado de [`super::orders`] de propósito, e o motivo é a calibração. Este
//! arquivo não importa nada do crate — nem `rusqlite`, nem `Team` —, só `rand` nos
//! testes, o que permite medi-lo sozinho quando a árvore não compila:
//!
//! ```text
//! rustc --test --edition 2021 --extern rand=<caminho do librand-*.rlib> \
//!       -L dependency=<dir deps> -o tensao_test.exe src/hierarchy/tensao.rs
//! tensao_test.exe --ignored --nocapture
//! ```
//!
//! Pela via normal é `cargo test --lib calibracao_do_eixo_de_tensao -- --ignored
//! --nocapture`, e dá exatamente os mesmos números.

// ── Tipos do confronto (Passo 4) ──────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum DuelWinner {
    N1,
    N2,
}

/// Resumo do confronto interno de uma equipe em uma rodada.
/// É o insumo para `apply_duel_counters`.
///
/// Só o desfecho: `team_id`/`n1_id`/`n2_id` moravam aqui e nenhum chamador os lia
/// (quem recebe o duelo já está com a equipe e a dupla em mãos). Saíram na revisão
/// do D-09/R4 em 11/08/2026.
#[derive(Debug, Clone)]
pub struct DuelResult {
    /// Passo 3: houve duelo interno válido nesta rodada?
    pub valid: bool,
    /// Some(DuelWinner) se duelo válido; None caso contrário.
    pub vencedor: Option<DuelWinner>,
}

// ── Os deltas ────────────────────────────────────────────────────────────────

/// Tensão ganha quando o N2 bate o N1 na rodada.
pub const TENSAO_DELTA_N2_VENCE: f64 = 3.0;
/// Tensão perdida quando o N1 confirma a hierarquia na rodada.
///
/// CALIBRADO em 11/08/2026, de 2,0 para 0,5, e é a única peça que mudou. Medido pelo
/// harness `calibracao_do_eixo_de_tensao` deste arquivo: em 2,0 o equilíbrio real do
/// eixo ficava em **0,420**, acima de TODA a faixa que o mundo simulado produz
/// (0,227 a 0,325) — a tensão de qualquer dupla caía até o piso e ficava lá. Em 0,5 o
/// equilíbrio cai para **0,308**: dentro da faixa e acima da média, que é onde ele
/// precisa estar (ver [`TAXA_N2_MUNDO_MEDIA`]).
///
/// Em número de jogo, numa temporada de 14 corridas: a dupla 50/50 sai de ~19 para
/// ~25 de tensão média e a fração que passa de 20 ("saiu de estável") vai de 45%
/// para 56%; a dupla mediana do mundo (p ≈ 0,28) continua no chão, com 6,5.
pub const TENSAO_DELTA_N1_VENCE: f64 = 0.5;
/// Decaimento da corrida SEM duelo válido (lesão, ausência): disputa que esfriou
/// por falta de disputa.
pub const TENSAO_DECAIMENTO_SEM_DUELO: f64 = 1.0;
/// Bônus por sequência do N2, aplicados só quando o limiar é atingido exatamente.
pub const TENSAO_BONUS_SEQ_N2_3: f64 = 10.0;
pub const TENSAO_BONUS_SEQ_N2_5: f64 = 15.0;
/// Penalidade por sequência do N1.
///
/// É ELA que faz o equilíbrio ingênuo mentir, e por isso não foi mexida: quando o N2
/// é fraco a sequência de 3 do N1 fecha em ~10% das corridas e a do N2 em ~1,5%, de
/// modo que a penalidade sozinha drena mais tensão por corrida do que a regra base
/// inteira. Baixá-la para 4,0 com o delta em 1,0 dá um mundo estatisticamente igual
/// ao da calibração escolhida (equilíbrio 0,300 contra 0,308, temporada de 50/50 em
/// ~24 contra ~25) — como não muda nada e mexe em duas peças em vez de uma, fica como
/// está. O harness `calibracao_do_eixo_de_tensao` varre as duas dimensões.
pub const TENSAO_PENALIDADE_SEQ_N1_3: f64 = 8.0;

/// Os seis deltas de uma rodada, juntos. Em produção existe um só valor
/// ([`TENSAO_DELTAS`]); vira parâmetro para que o harness de calibração varra
/// candidatos pela MESMA função que roda no jogo, em vez de por uma réplica que
/// passa a mentir assim que alguém mexe na regra.
#[derive(Debug, Clone, Copy)]
pub struct TensaoDeltas {
    pub n2_vence: f64,
    pub n1_vence: f64,
    pub sem_duelo: f64,
    pub bonus_seq_n2_3: f64,
    pub bonus_seq_n2_5: f64,
    pub penalidade_seq_n1_3: f64,
}

/// Os deltas que o jogo usa.
pub const TENSAO_DELTAS: TensaoDeltas = TensaoDeltas {
    n2_vence: TENSAO_DELTA_N2_VENCE,
    n1_vence: TENSAO_DELTA_N1_VENCE,
    sem_duelo: TENSAO_DECAIMENTO_SEM_DUELO,
    bonus_seq_n2_3: TENSAO_BONUS_SEQ_N2_3,
    bonus_seq_n2_5: TENSAO_BONUS_SEQ_N2_5,
    penalidade_seq_n1_3: TENSAO_PENALIDADE_SEQ_N1_3,
};

/// Taxa de vitória do N2 em que a REGRA BASE se anula, ignorando os bônus de
/// sequência: `p* = n1_vence / (n2_vence + n1_vence)`.
///
/// **Este número sozinho engana**, e enganou: era ele que a documentação anterior
/// chamava de "equilíbrio", e foi por ele que o eixo passou meses fora do alcance do
/// mundo. Os bônus de sequência não são simétricos — a sequência de 3 do N1 é comum
/// quando o N2 é fraco e a do N2 é rara — então o equilíbrio de verdade fica bem
/// acima do ingênuo. Quem mede o de verdade, rodando a própria [`delta_da_rodada`],
/// é `tests::equilibrio_empirico`.
///
/// Fica como referência de leitura e termo da conta; sem chamador de produção.
#[cfg_attr(not(test), allow(dead_code))]
pub const TENSAO_EQUILIBRIO_INGENUO: f64 =
    TENSAO_DELTA_N1_VENCE / (TENSAO_DELTA_N2_VENCE + TENSAO_DELTA_N1_VENCE);

/// Delta de tensão de uma rodada, sem clamp e sem tocar na equipe.
///
/// `seq_n2`/`seq_n1` são as sequências JÁ atualizadas por `apply_duel_counters`.
/// Separada de `update_tensao` para que a calibração varra deltas candidatos sem
/// replicar a regra.
pub fn delta_da_rodada(deltas: &TensaoDeltas, duel: &DuelResult, seq_n2: i32, seq_n1: i32) -> f64 {
    let mut delta: f64 = 0.0;

    if duel.valid {
        match &duel.vencedor {
            Some(DuelWinner::N2) => delta += deltas.n2_vence,
            Some(DuelWinner::N1) => delta -= deltas.n1_vence,
            None => {}
        }
    } else {
        delta -= deltas.sem_duelo;
    }

    // Bônus de sequência — apenas quando o limiar é atingido exatamente
    if seq_n2 == 3 {
        delta += deltas.bonus_seq_n2_3;
    } else if seq_n2 == 5 {
        delta += deltas.bonus_seq_n2_5;
    }

    if seq_n1 == 3 {
        delta -= deltas.penalidade_seq_n1_3;
    }

    delta
}

// ── Calibração ───────────────────────────────────────────────────────────────
//
// O eixo tinha um número declarado (0,40 de equilíbrio) que não era o número real,
// porque a conta ingênua ignora os bônus de sequência. O que segue mede o equilíbrio
// DE VERDADE rodando a própria `delta_da_rodada` — é o instrumento que substitui a
// estimativa de cabeça.

/// Faixa da taxa do N2 medida no mundo simulado pelo harness
/// `teammate_tension_climbs_and_finally_creates_rivalries` (12 temporadas, 102
/// equipes): 0,227 e 0,325 em 02/08/2026, 0,213 em 11/08/2026. Média ~0,276.
///
/// O eixo só está vivo se o equilíbrio cair DENTRO dessa faixa e ACIMA da média:
/// abaixo da média o mundo inteiro infla até 100 e "crise" vira o novo "estável";
/// acima do topo, nenhuma equipe sai do chão — que era exatamente o estado anterior.
///
/// **A AMOSTRA É POBRE, e é bom saber disso antes de confiar demais nela.** No banco
/// do run de 11/08/2026, das 102 equipes ativas só 12 tinham algum duelo registrado
/// (328 no total, concentrados em gt3 e endurance) e as 102 estavam com
/// `hierarquia_n1_id`/`n2_id` apontando para pilotos SEM contrato ativo na própria
/// equipe e em outra categoria. Com a dupla defasada, `read_team_duel` nunca acha os
/// dois no resultado, o duelo sai inválido e a tensão só decai — foi o que congelou o
/// harness em média 1,0 nas doze temporadas.
///
/// A causa está fora deste eixo: quem realinha N1/N2 é
/// `hierarchy::transition::validate_and_normalize_team_hierarchies`, e o único
/// chamador de produção é a pré-temporada JOGÁVEL (`commands/career/market_window.rs`).
/// O rascunho histórico não passa por lá. Ou seja: a faixa acima vem das poucas
/// equipes que por acaso mantiveram a dupla alinhada, e o harness de mundo **não
/// consegue** confirmar calibração enquanto o realinhamento não rodar também no
/// caminho histórico. A calibração em si foi medida pela via determinística
/// (`calibracao_do_eixo_de_tensao`), que não depende do mundo.
#[cfg_attr(not(test), allow(dead_code))]
pub const TAXA_N2_MUNDO_MIN: f64 = 0.227;
#[cfg_attr(not(test), allow(dead_code))]
pub const TAXA_N2_MUNDO_MEDIA: f64 = 0.276;
#[cfg_attr(not(test), allow(dead_code))]
pub const TAXA_N2_MUNDO_MAX: f64 = 0.325;

/// Deriva média de tensão por corrida quando o N2 leva a fração `p` dos duelos.
///
/// Mantém as sequências exatamente como `apply_duel_counters` mantém e chama a
/// função de produção. Sem clamp de propósito: o que se mede é a deriva, e o piso em
/// zero é justamente o que esconde uma deriva negativa dentro do jogo.
///
/// `sorteio` entrega um número em [0,1) por corrida — o gerador é do chamador para
/// que este arquivo continue sem dependência nenhuma.
#[cfg_attr(not(test), allow(dead_code))]
pub fn deriva_por_corrida(
    deltas: &TensaoDeltas,
    p: f64,
    corridas: usize,
    sorteio: &mut dyn FnMut() -> f64,
) -> f64 {
    let (mut seq_n2, mut seq_n1) = (0, 0);
    let mut soma = 0.0;

    for _ in 0..corridas {
        let n2_venceu = sorteio() < p;
        if n2_venceu {
            seq_n2 += 1;
            seq_n1 = 0;
        } else {
            seq_n1 += 1;
            seq_n2 = 0;
        }
        let duel = DuelResult {
            valid: true,
            vencedor: Some(if n2_venceu {
                DuelWinner::N2
            } else {
                DuelWinner::N1
            }),
        };
        soma += delta_da_rodada(deltas, &duel, seq_n2, seq_n1);
    }

    soma / corridas as f64
}

/// Taxa do N2 em que a deriva medida cruza zero — o equilíbrio real do eixo.
/// Bissecção sobre [`deriva_por_corrida`], que é monotônica em `p`.
#[cfg_attr(not(test), allow(dead_code))]
pub fn equilibrio_empirico(deltas: &TensaoDeltas, sorteio: &mut dyn FnMut() -> f64) -> f64 {
    const CORRIDAS: usize = 120_000;
    let (mut baixo, mut alto) = (0.0_f64, 1.0_f64);
    for _ in 0..24 {
        let meio = (baixo + alto) / 2.0;
        if deriva_por_corrida(deltas, meio, CORRIDAS, sorteio) < 0.0 {
            baixo = meio;
        } else {
            alto = meio;
        }
    }
    (baixo + alto) / 2.0
}

/// Tensão média ao fim de uma temporada de `corridas` duelos com taxa `p` do N2, e a
/// fração de duplas (em %) que termina acima de 20 ("saiu de estável") e de 40 ("em
/// tensão"). AQUI o clamp entra, porque é o número que o jogador vê: a virada zera a
/// tensão de toda equipe, então nada atravessa a temporada.
#[cfg_attr(not(test), allow(dead_code))]
pub fn tensao_ao_fim_da_temporada(
    deltas: &TensaoDeltas,
    p: f64,
    corridas: usize,
    amostras: usize,
    sorteio: &mut dyn FnMut() -> f64,
) -> (f64, f64, f64) {
    let (mut soma, mut acima_20, mut acima_40) = (0.0, 0.0, 0.0);

    for _ in 0..amostras {
        let (mut seq_n2, mut seq_n1, mut tensao) = (0, 0, 0.0_f64);
        for _ in 0..corridas {
            let n2_venceu = sorteio() < p;
            if n2_venceu {
                seq_n2 += 1;
                seq_n1 = 0;
            } else {
                seq_n1 += 1;
                seq_n2 = 0;
            }
            let duel = DuelResult {
                valid: true,
                vencedor: Some(if n2_venceu {
                    DuelWinner::N2
                } else {
                    DuelWinner::N1
                }),
            };
            tensao = (tensao + delta_da_rodada(deltas, &duel, seq_n2, seq_n1)).clamp(0.0, 100.0);
        }
        soma += tensao;
        if tensao >= 20.0 {
            acima_20 += 1.0;
        }
        if tensao >= 40.0 {
            acima_40 += 1.0;
        }
    }

    let amostras = amostras as f64;
    (
        soma / amostras,
        100.0 * acima_20 / amostras,
        100.0 * acima_40 / amostras,
    )
}

/// O que uma temporada faz com o GATILHO DE INVERSÃO (`orders::has_inversao_trigger`),
/// medido em vez de estimado.
///
/// O gatilho pede cinco coisas ao mesmo tempo: ≥ 8 duelos, sequência do N2 ≥ 5, segundo
/// turno da temporada, ≥ 65% dos duelos com o N2, e o STATUS em "crise". As quatro
/// primeiras descrevem uma situação concreta de pista; a quinta é uma leitura do MESMO
/// eixo por outra régua, e "crise" mora em tensão ≥ 90.
///
/// Devolve `(fração de temporadas em que as QUATRO primeiras armam, tensão média nesse
/// instante, tensão MÁXIMA já vista nele)`. É esse máximo que diz se a quinta condição é
/// alcançável ou é aritmética morta.
#[cfg_attr(not(test), allow(dead_code))]
pub fn gatilho_de_inversao_na_temporada(
    deltas: &TensaoDeltas,
    p: f64,
    corridas: usize,
    amostras: usize,
    sorteio: &mut dyn FnMut() -> f64,
) -> (f64, f64, f64) {
    let (mut armou, mut soma_tensao, mut maxima) = (0.0_f64, 0.0_f64, 0.0_f64);

    for _ in 0..amostras {
        let (mut seq_n2, mut seq_n1, mut tensao) = (0, 0, 0.0_f64);
        let (mut duelos, mut vitorias_n2) = (0_i32, 0_i32);
        let mut armou_nesta = false;

        for rodada in 1..=corridas {
            let n2_venceu = sorteio() < p;
            duelos += 1;
            if n2_venceu {
                vitorias_n2 += 1;
                seq_n2 += 1;
                seq_n1 = 0;
            } else {
                seq_n1 += 1;
                seq_n2 = 0;
            }
            let duel = DuelResult {
                valid: true,
                vencedor: Some(if n2_venceu {
                    DuelWinner::N2
                } else {
                    DuelWinner::N1
                }),
            };
            tensao = (tensao + delta_da_rodada(deltas, &duel, seq_n2, seq_n1)).clamp(0.0, 100.0);

            // As QUATRO condições concretas, exatamente como `has_inversao_trigger` as lê.
            let taxa = vitorias_n2 as f64 / duelos as f64;
            if duelos >= 8 && seq_n2 >= 5 && rodada * 2 > corridas && taxa >= 0.65 {
                if !armou_nesta {
                    armou_nesta = true;
                    armou += 1.0;
                    soma_tensao += tensao;
                }
                maxima = maxima.max(tensao);
            }
        }
    }

    let media = if armou > 0.0 {
        soma_tensao / armou
    } else {
        0.0
    };
    (armou / amostras as f64, media, maxima)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, Rng, SeedableRng};

    fn sorteio_semeado(seed: u64) -> impl FnMut() -> f64 {
        let mut rng = StdRng::seed_from_u64(seed);
        move || rng.gen::<f64>()
    }

    fn equilibrio(deltas: &TensaoDeltas) -> f64 {
        let mut sorteio = sorteio_semeado(20_260_811);
        equilibrio_empirico(deltas, &mut sorteio)
    }

    /// O guard de calibração: prende o equilíbrio medido à faixa medida no mundo.
    ///
    /// Quebrou? Ou alguém mexeu num delta sem refazer a conta, ou o mundo mudou de
    /// taxa. Nos dois casos a saída é rodar `calibracao_do_eixo_de_tensao`
    /// (`--ignored --nocapture`) e o harness de mundo, não afrouxar o limite.
    #[test]
    fn o_equilibrio_empirico_esta_na_faixa_medida_do_mundo() {
        let medido = equilibrio(&TENSAO_DELTAS);
        assert!(
            medido > TAXA_N2_MUNDO_MEDIA && medido < TAXA_N2_MUNDO_MAX,
            "equilíbrio medido em {medido:.3}: precisa ficar entre a média \
             ({TAXA_N2_MUNDO_MEDIA}) e o topo ({TAXA_N2_MUNDO_MAX}) da faixa do mundo"
        );
    }

    /// A armadilha que criou o item: a conta ingênua não é o equilíbrio.
    #[test]
    fn o_equilibrio_ingenuo_subestima_o_real() {
        let real = equilibrio(&TENSAO_DELTAS);
        assert!(
            real > TENSAO_EQUILIBRIO_INGENUO,
            "os bônus de sequência puxam o equilíbrio para cima; ingênuo \
             {TENSAO_EQUILIBRIO_INGENUO:.3} vs real {real:.3}"
        );
    }

    /// O diagnóstico do item, medido: com o delta antigo (−2) o equilíbrio ficava
    /// acima do TOPO da faixa que o mundo produz. Não era "o eixo sobe devagar" —
    /// era um eixo cuja deriva é negativa para toda dupla que o jogo consegue
    /// montar, com piso em zero. Este teste guarda o diagnóstico: se um dia ele
    /// falhar, a regra mudou o bastante para a calibração inteira precisar ser
    /// refeita.
    #[test]
    fn a_calibracao_antiga_ficava_acima_de_todo_o_mundo() {
        let antiga = TensaoDeltas {
            n1_vence: 2.0,
            ..TENSAO_DELTAS
        };
        let medido = equilibrio(&antiga);
        assert!(
            medido > TAXA_N2_MUNDO_MAX,
            "o equilíbrio da calibração antiga era {medido:.3}, e precisava estar \
             acima do topo do mundo ({TAXA_N2_MUNDO_MAX}) — a régua mudou"
        );
    }

    /// Harness do GATILHO DE INVERSÃO: com que frequência as quatro condições concretas
    /// armam, e em que tensão a equipe está quando isso acontece.
    /// `cargo test --lib gatilho_de_inversao_medido -- --ignored --nocapture`
    #[test]
    #[ignore = "harness do gatilho de inversão; rodar sob demanda"]
    fn gatilho_de_inversao_medido() {
        eprintln!("taxa_n2 | temporadas que armam | tensao ao armar | tensao maxima");
        for p in [TAXA_N2_MUNDO_MEDIA, TAXA_N2_MUNDO_MAX, 0.40, 0.50, 0.65, 0.80] {
            let mut s = sorteio_semeado(4_242);
            let (frac, media, maxima) =
                gatilho_de_inversao_na_temporada(&TENSAO_DELTAS, p, 14, 20_000, &mut s);
            eprintln!(
                "  {p:5.3} |        {:6.2}%       |     {media:6.1}      |    {maxima:6.1}",
                100.0 * frac
            );
        }
        eprintln!("\n[LIMIARES] estavel<20 competitivo<40 tensao<60 reavaliacao<75 inversao<90 crise>=90");
    }

    /// Harness de calibração: varre candidatos e imprime o equilíbrio de cada um.
    /// Não assevera nada — é a régua com que se escolhem os deltas.
    /// `cargo test --lib calibracao_do_eixo_de_tensao -- --ignored --nocapture`
    #[test]
    #[ignore = "harness de calibração do eixo de tensão; rodar sob demanda"]
    fn calibracao_do_eixo_de_tensao() {
        eprintln!("n1_vence | pen_seq_n1 | equilibrio | deriva@0.276 | deriva@0.50");
        for penalidade in [8.0, 6.0, 4.0, 2.0, 0.0] {
            for n1_vence in [2.0, 1.5, 1.0, 0.5, 0.0] {
                let deltas = TensaoDeltas {
                    n1_vence,
                    penalidade_seq_n1_3: penalidade,
                    ..TENSAO_DELTAS
                };
                let eq = equilibrio(&deltas);
                let mut s1 = sorteio_semeado(7);
                let no_mundo = deriva_por_corrida(&deltas, TAXA_N2_MUNDO_MEDIA, 120_000, &mut s1);
                let mut s2 = sorteio_semeado(7);
                let parelha = deriva_por_corrida(&deltas, 0.5, 120_000, &mut s2);
                eprintln!(
                    "  {n1_vence:4.1}   |    {penalidade:4.1}    |   {eq:6.3}   |   \
                     {no_mundo:+6.3}     |  {parelha:+6.3}"
                );
            }
        }
        eprintln!(
            "[EM USO] n1_vence={TENSAO_DELTA_N1_VENCE:.1} penalidade={TENSAO_PENALIDADE_SEQ_N1_3:.1} \
             -> equilibrio {:.3} (faixa do mundo {TAXA_N2_MUNDO_MIN}-{TAXA_N2_MUNDO_MAX}, \
             media {TAXA_N2_MUNDO_MEDIA})",
            equilibrio(&TENSAO_DELTAS)
        );

        // O número que o jogador vê: a virada zera a tensão, então o que importa é
        // quanto uma dupla junta DENTRO de uma temporada.
        eprintln!("\ntaxa_n2 | tensao media em 14 corridas | >=20  | >=40");
        for p in [
            0.20,
            TAXA_N2_MUNDO_MEDIA,
            TAXA_N2_MUNDO_MAX,
            0.40,
            0.50,
            0.65,
        ] {
            let mut s = sorteio_semeado(99);
            let (media, acima_20, acima_40) =
                tensao_ao_fim_da_temporada(&TENSAO_DELTAS, p, 14, 20_000, &mut s);
            eprintln!(
                "  {p:5.3} |            {media:6.1}           | {acima_20:4.0}% | {acima_40:4.0}%"
            );
        }
    }
}
