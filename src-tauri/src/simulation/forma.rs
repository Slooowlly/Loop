//! As camadas INTERMEDIÁRIAS de performance. **Lógica pura, testável, sem RNG.**
//!
//! O problema que este módulo resolve: a performance de um piloto era um número
//! só. O score de `race/pontuacao.rs` é combinação linear determinística de
//! atributos — idêntica em toda corrida e em todo segmento —, e o único ruído
//! existente é i.i.d. POR SEGMENTO, então ele se auto-cancela na soma de
//! `race/motor.rs`. Resultado: cinco etapas seguidas com o mesmo P1..P12.
//!
//! A performance real se decompõe em camadas com escalas de tempo diferentes.
//! Existiam só as duas pontas — o rating de carreira (lentíssimo) e o ruído por
//! segmento (rapidíssimo e inócuo). Aqui entram as três do meio:
//!
//! 1. **Afinidade piloto × pista** — permanente, vitalícia, com sinal. Por hash
//!    determinístico de `(driver_id, track_id)`: sem coluna, sem migração, sem
//!    quebrar save antigo, 100% reprodutível. "O Kowalczyk voa em Spa e apanha
//!    em Okayama, TODO ano."
//! 2. **Forma do momento** — AR(1) que evolui entre etapas. Única camada com
//!    estado persistido (`drivers.forma`). Gera SEQUÊNCIAS (três pódios e
//!    depois uma queda) que amostragem i.i.d. jamais produz.
//! 3. **Acerto do fim de semana** — sorteado uma vez por (equipe, evento), com
//!    um componente por piloto: um lado da garagem acha o acerto e o outro não.
//!    A propriedade que importa é que ele NÃO varia entre segmentos, e por isso
//!    SOBREVIVE à soma — provavelmente a de maior impacto das três.
//!
//! Nada disto é "mais aleatoriedade": é ESTRUTURA. A ordem se reembaralha por
//! etapa de um jeito consistente e explicável.
//!
//! **Não confundir com [`super::track_knowledge`]**, que é ortogonal: aquilo é
//! só penalidade, temporária, cai conforme o piloto aprende a pista e enferruja
//! se ele some por temporadas. Afinidade é permanente, tem sinal e nunca some.
//! Os dois coexistem e se somam.
//!
//! ## Calibração
//!
//! As magnitudes abaixo são PONTO DE PARTIDA, não valor final — serão
//! recalibradas contra o harness estatístico. Todas estão neste bloco de
//! constantes públicas, de propósito.

use super::track_profile::{get_track_simulation_data, TrackCharacter};

// ─────────────────────────── Magnitudes (calibráveis) ───────────────────────────
//
// As três escalas abaixo são o DESVIO-PADRÃO da camada, em pontos de skill — não
// o máximo. Foi uma escolha deliberada: com distribuição em sino, tratar o número
// como teto deixaria o valor TÍPICO em ~1/3 dele, pequeno demais pra mover a
// ordem de chegada (o `skill` pesa ~0,28 do score de segmento, então um ponto de
// skill vale bem menos que um ponto de score). Como σ, o número diz o que
// acontece num fim de semana comum, que é o que se quer calibrar.
//
// O teto duro é [`TETO_SIGMAS`] desvios: nenhuma camada passa disso.

/// Camada 1 — escala (1 σ) da afinidade piloto × pista, em pontos de skill.
pub const AFINIDADE_ESCALA_PONTOS: f64 = 3.0;

/// Camada 2 — escala (1 σ) da forma do momento, em pontos de skill.
pub const FORMA_ESCALA_PONTOS: f64 = 2.0;

/// Camada 3 — escala (1 σ) do acerto de fim de semana, em pontos de skill.
pub const ACERTO_ESCALA_PONTOS: f64 = 2.5;

/// Camada 2 — ρ do AR(1). Meia-vida ≈ 1,6 etapas; correlação ainda visível a 3-4
/// corridas de distância, que é o tamanho de uma "fase" boa ou ruim.
pub const FORMA_RHO: f64 = 0.65;

/// Teto duro de qualquer camada, em desvios-padrão. Impede que a cauda de uma
/// camada sozinha vire um resultado absurdo.
pub const TETO_SIGMAS: f64 = 2.5;

/// A afinidade pesa MAIS na classificação que na corrida. A volta perfeita é
/// onde o casamento com a pista aparece inteiro; na corrida ele fica diluído em
/// tráfego, pneu e estratégia.
pub const MULT_AFINIDADE_QUALI: f64 = 1.5;

// ─────────────────────────── Repartição interna ───────────────────────────
//
// Os pesos abaixo repartem VARIÂNCIA (por isso entram como raiz quadrada): a
// soma das frações é 1, e o resultado sai com σ = 1 na escala adimensional.

/// Camada 1 — fração da variância que é idiossincrasia pura ("simplesmente se dá
/// bem aqui"); o resto é casamento de estilo com o caráter da pista. O pedaço do
/// estilo é o que torna a afinidade LEGÍVEL: um piloto de `smoothness` alto indo
/// bem em pista fluida se explica; ruído puro não.
const AFINIDADE_FRACAO_IDIOSSINCRATICA: f64 = 0.65;

/// Camada 1 — ganho aplicado ao casamento de estilo (que vive em [-1, 1]) pra
/// ele chegar em σ ≈ 1 numa população de atributos realista.
const AFINIDADE_GANHO_ESTILO: f64 = 1.8;

/// Camada 2 — quanto ânimo (motivação + confiança) empurra a média da forma, em
/// desvios-padrão. É o que fecha o laço pódio → confiança → ritmo → pódio: a
/// confiança já sobe com bons resultados, e agora isso volta pra pista em vez de
/// morrer no perfil.
const FORMA_PESO_ANIMO: f64 = 0.20;

/// Camada 3 — fração da variância que é da EQUIPE (o carro é um só); o resto é o
/// pedaço por piloto, que faz um lado da garagem achar o acerto e o outro não.
const ACERTO_FRACAO_EQUIPE: f64 = 0.70;

// ─────────────────────────── Hash determinístico ───────────────────────────

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;

/// FNV-1a de 64 bits sobre bytes, encadeável a partir de um estado. Mesma
/// constante já usada em `commands/race/simulacao.rs` pra semear as quebras.
fn misturar(estado: u64, bytes: &[u8]) -> u64 {
    let mut h = estado;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

fn misturar_num(estado: u64, valor: i64) -> u64 {
    misturar(estado, &valor.to_le_bytes())
}

/// Finalizador (o mix do splitmix64). O FNV sozinho espalha mal entradas quase
/// iguais — e as três sub-amostras de [`normal_aprox`] são exatamente isso. Sem
/// este passo elas saem CORRELACIONADAS, a média de 3 não estreita como deveria e
/// a camada acaba com σ maior do que a constante diz.
fn misturar_final(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Hash → uniforme em [0, 1). Usa os 53 bits altos (os baixos do FNV são os
/// piores).
fn uniforme(h: u64) -> f64 {
    (misturar_final(h) >> 11) as f64 / (1u64 << 53) as f64
}

/// Hash → valor em sino aproximadamente normal, com **σ = 1** e cauda cortada em
/// [`TETO_SIGMAS`]. A média de 3 uniformes, centrada em zero, tem σ = 1/6; o
/// fator 6 leva pra σ = 1. Extremos são raros por construção — é o que impede a
/// camada de virar um dado de amplitude alta.
fn normal_aprox(h: u64) -> f64 {
    let a = uniforme(misturar_num(h, 1));
    let b = uniforme(misturar_num(h, 2));
    let c = uniforme(misturar_num(h, 3));
    (((a + b + c) / 3.0 - 0.5) * 6.0).clamp(-TETO_SIGMAS, TETO_SIGMAS)
}

/// Normaliza um atributo 0–100 para [-1, 1] em torno de 50.
fn normalizar_atributo(valor: f64) -> f64 {
    ((valor.clamp(0.0, 100.0) - 50.0) / 50.0).clamp(-1.0, 1.0)
}

// ─────────────────────────── Camada 1: afinidade ───────────────────────────

/// Os atributos de ESTILO que casam (ou não) com o caráter de um circuito.
#[derive(Debug, Clone, Copy)]
pub struct EstiloPiloto {
    pub smoothness: f64,
    pub consistencia: f64,
    pub adaptabilidade: f64,
    pub aggression: f64,
}

impl EstiloPiloto {
    /// Atributo que o caráter da pista premia. Segue a leitura já assumida em
    /// [`TrackCharacter`]: fluida pede fluidez, apertada pede precisão, técnica
    /// pede leitura, roval pede coragem.
    fn casamento_com(&self, caracter: TrackCharacter) -> f64 {
        let bruto = match caracter {
            TrackCharacter::Flowing => self.smoothness,
            TrackCharacter::Tight => self.consistencia,
            TrackCharacter::Technical => self.adaptabilidade,
            TrackCharacter::Roval => self.aggression,
        };
        normalizar_atributo(bruto)
    }
}

/// **Camada 1.** Afinidade vitalícia deste piloto com esta pista, em pontos de
/// skill (positivo = voa aqui, negativo = apanha aqui).
///
/// Permanente por CONSTRUÇÃO: sai de `hash(driver_id, track_id)`, então não tem
/// o que persistir nem o que sortear de novo — o mesmo par devolve o mesmo
/// número em toda chamada, em todo save, pra sempre.
pub fn afinidade_pista(driver_id: &str, track_id: u32, estilo: &EstiloPiloto) -> f64 {
    let h = misturar(misturar(FNV_OFFSET, driver_id.as_bytes()), b"|pista|");
    let h = misturar_num(h, track_id as i64);
    let idiossincratico = normal_aprox(h);
    let caracter = get_track_simulation_data(track_id).track_character;
    let estilo = estilo.casamento_com(caracter) * AFINIDADE_GANHO_ESTILO;

    let adimensional = AFINIDADE_FRACAO_IDIOSSINCRATICA.sqrt() * idiossincratico
        + (1.0 - AFINIDADE_FRACAO_IDIOSSINCRATICA).sqrt() * estilo;

    AFINIDADE_ESCALA_PONTOS * adimensional.clamp(-TETO_SIGMAS, TETO_SIGMAS)
}

// ─────────────────────────── Camada 2: forma ───────────────────────────

/// Semente do passo de forma de UM piloto num fim de semana. Determinística:
/// re-simular a mesma etapa dá o mesmo passo.
pub fn semente_forma(temporada: i32, rodada: i32, driver_id: &str) -> u64 {
    let h = misturar(misturar(FNV_OFFSET, b"forma|"), driver_id.as_bytes());
    misturar_num(misturar_num(h, temporada as i64), rodada as i64)
}

/// **Camada 2.** Um passo do AR(1): `forma_t = ρ · forma_{t-1} + ânimo + ruído`.
///
/// O estado é adimensional e **normalizado em σ = 1**: o ruído entra com
/// `√(1 − ρ²)` justamente pra que a variância estacionária feche em 1, e o valor
/// fica cortado em [`TETO_SIGMAS`]. Use [`forma_em_pontos`] pra convertê-lo em
/// pontos de skill. `motivacao` e `confianca` (0–100) entram como um empurrão
/// suave na média — piloto animado tende a fases boas mais longas.
pub fn proxima_forma(forma_atual: f64, semente: u64, motivacao: f64, confianca: f64) -> f64 {
    let animo = (normalizar_atributo(motivacao) + normalizar_atributo(confianca)) / 2.0;
    let ruido = (1.0 - FORMA_RHO * FORMA_RHO).sqrt() * normal_aprox(semente);
    (FORMA_RHO * forma_atual.clamp(-TETO_SIGMAS, TETO_SIGMAS) + FORMA_PESO_ANIMO * animo + ruido)
        .clamp(-TETO_SIGMAS, TETO_SIGMAS)
}

/// Converte o estado adimensional da forma (σ = 1) em pontos de skill.
pub fn forma_em_pontos(estado: f64) -> f64 {
    estado.clamp(-TETO_SIGMAS, TETO_SIGMAS) * FORMA_ESCALA_PONTOS
}

// ─────────────────────────── Camada 3: acerto ───────────────────────────

/// **Camada 3.** Acerto do fim de semana, em pontos de skill. Vale a CORRIDA
/// INTEIRA — não varia entre segmentos, e é justamente por isso que sobrevive à
/// soma de `race/motor.rs` (ao contrário do ruído por segmento, que se cancela).
///
/// Sai de um seed determinístico de `(temporada, etapa, equipe)` mais um tempero
/// por piloto: sem persistência, reprodutível, e sem precisar carregar um `rng`
/// pela esteira.
pub fn acerto_fim_de_semana(temporada: i32, rodada: i32, team_id: &str, driver_id: &str) -> f64 {
    let base = misturar_num(
        misturar_num(
            misturar(misturar(FNV_OFFSET, b"acerto|"), team_id.as_bytes()),
            temporada as i64,
        ),
        rodada as i64,
    );
    let equipe = normal_aprox(base);
    let piloto = normal_aprox(misturar(base, driver_id.as_bytes()));

    let adimensional =
        ACERTO_FRACAO_EQUIPE.sqrt() * equipe + (1.0 - ACERTO_FRACAO_EQUIPE).sqrt() * piloto;

    ACERTO_ESCALA_PONTOS * adimensional.clamp(-TETO_SIGMAS, TETO_SIGMAS)
}

// ─────────────────────────── Agregado ───────────────────────────

/// O que as três camadas descontam (ou dão) num fim de semana, já separado pelos
/// dois canais que a esteira alimenta: o skill de CORRIDA e o ritmo de
/// CLASSIFICAÇÃO. A afinidade é a única que difere entre os dois.
#[derive(Debug, Clone, Copy)]
pub struct AjusteFimDeSemana {
    pub corrida: f64,
    pub classificacao: f64,
}

/// Junta as três camadas num ajuste só. `estado_forma` é o valor JÁ avançado
/// para este fim de semana (ver [`proxima_forma`]).
pub fn ajuste_fim_de_semana(
    driver_id: &str,
    team_id: &str,
    track_id: u32,
    temporada: i32,
    rodada: i32,
    estilo: &EstiloPiloto,
    estado_forma: f64,
) -> AjusteFimDeSemana {
    let afinidade = afinidade_pista(driver_id, track_id, estilo);
    let comum =
        forma_em_pontos(estado_forma) + acerto_fim_de_semana(temporada, rodada, team_id, driver_id);

    AjusteFimDeSemana {
        corrida: afinidade + comum,
        classificacao: afinidade * MULT_AFINIDADE_QUALI + comum,
    }
}

#[cfg(test)]
mod tests;
