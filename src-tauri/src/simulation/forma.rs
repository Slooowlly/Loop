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

// ── Injetabilidade (pacote H) ──
//
// As quatro escalas abaixo continuam existindo como `const`, e continuam sendo o DEFAULT — é
// isso que garante a equivalência. O que mudou é que existe uma variante de cada função que
// recebe a escala por PARÂMETRO, porque a máquina de busca da campanha só alcança o que flui
// por parâmetro: `const` é inalcançável por construção, e era esse o bloqueio.
//
// A API é ADITIVA de propósito. As quatro funções originais seguem existindo e delegam para as
// variantes com escala usando o default, o que mantém compilando quem já as chamava — inclusive
// o espelho do harness em `calibracao/arena.rs`, que eu não posso editar e que será apagado por
// eles agora que a função pura existe.
//
// **Eu não escolho valor nenhum aqui.** Só torno escolhível.

/// As quatro escalas do `forma.rs`, agrupadas para atravessar a esteira de uma vez.
/// `Default` são exatamente as constantes de hoje.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EscalasDeForma {
    /// σ da afinidade piloto × pista, em pontos de skill.
    pub afinidade: f64,
    /// σ da forma do momento, em pontos de skill.
    pub forma: f64,
    /// σ do acerto de fim de semana, em pontos de skill.
    pub acerto: f64,
    /// ρ do AR(1) da forma.
    pub rho: f64,
    /// Peso do ânimo (motivação + confiança) na média da forma.
    ///
    /// Entrou na lista de injetáveis por um motivo específico: a motivação segue o resultado e
    /// acumula ano a ano (medido: bimodal, p25 em 41, mediana 93, correlacionada com skill), então
    /// este termo injeta na forma um deslocamento PERMANENTE por qualidade do piloto. Parte da
    /// variância da forma não é serial com ρ; é permanente com ρ ≈ 1. Rodar com `peso_animo = 0` e
    /// comparar é o que separa as duas partes — e é por isso que ele precisa ser parâmetro.
    pub peso_animo: f64,
}

impl Default for EscalasDeForma {
    fn default() -> Self {
        Self {
            afinidade: AFINIDADE_ESCALA_PONTOS,
            forma: FORMA_ESCALA_PONTOS,
            acerto: ACERTO_ESCALA_PONTOS,
            rho: FORMA_RHO,
            peso_animo: FORMA_PESO_ANIMO,
        }
    }
}

// ── Dimensionamento das três camadas ──
//
// Os valores de partida (3,0 / 2,0 / 2,5) foram escolhidos por intuição, antes de existir
// medição. A decomposição de variância mediu o que eles produzem: camada de EVENTO em 11,8%
// (rookie) e 9,7% (gt3), contra um alvo de 20–32% e 22–34%. Pequenos por um fator de ~2,5 em
// variância — ou seja, ~1,6 a 1,8 em desvio.
//
// A conta que fixa o multiplicador: escalando a σ de uma camada por k, a fração que ela ocupa
// vai de `e` para `k²e / (1 − e + k²e)`. Com k = 1,8 (k² = 3,24):
//
//   rookie: 3,24 × 0,118 / (0,882 + 0,382) = 30,2%  → dentro de 20–32
//   gt3:    3,24 × 0,097 / (0,903 + 0,314) = 25,8%  → dentro de 22–34
//
// E como as frações somam 1, subir evento derruba `piloto` sem tocar nele — que é exatamente
// onde a rookie está estourada (69,4% contra um alvo de 38–52%).
//
// Isto é a primeira mexida de MAGNITUDE do projeto; tudo antes foi mecanismo e instrumento. O
// número certo sai da campanha, não daqui — o que esta conta entrega é um ponto de partida com
// argumento, na ordem de grandeza medida, em vez de um chute redondo.

// Medido no primeiro giro (calibre único de 1,8): evento subiu para 25,0% / 20,3% — dentro do
// alvo — e ρ(N,N+1) caiu de 0,776 para 0,680 na rookie. Mas o permanente ficou em 0,601, ou
// seja, ρ ficou ACIMA do próprio permanente. O excedente tem dono conhecido.
//
// As três camadas não são intercambiáveis para o sintoma, porque têm autocorrelações
// diferentes entre etapas consecutivas:
//
// | camada    | escopo do sorteio      | a (autocorr.) | eficiência (1 − a) |
// |-----------|------------------------|---------------|--------------------|
// | afinidade | (piloto, pista)        | ~0            | 1,00               |
// | acerto    | (equipe, evento)       | 0             | 1,00               |
// | forma     | AR(1) entre etapas     | 0,65          | 0,35               |
//
// Uma fonte com autocorrelação `a` puxa o ρ agregado NA DIREÇÃO de `a`. A forma, com 0,65,
// está hoje praticamente empatada com o ρ que se quer derrubar — ela enche o orçamento de
// evento sem pagar quase nada no sintoma. Afinidade e acerto sorteiam de novo a cada etapa e
// derrubam ρ com eficiência total.
//
// Daí dois calibres em vez de um. **A razão 3,0 : 2,0 : 2,5 foi abandonada de propósito**: ela
// era uma escolha estética de quando não havia medição, e a medição diz que as camadas fazem
// trabalhos diferentes.
//
// A forma NÃO vai a zero, e a razão não é o orçamento: ela é a única das três que produz
// SEQUÊNCIA — três pódios seguidos e depois uma queda — que é o fenômeno narrativo pelo qual
// ela foi construída. Amostragem independente jamais produz isso. Ela paga em ρ o preço de ser
// a camada com memória, e esse preço é o produto dela.

/// Calibre das camadas SEM memória (afinidade e acerto). Sorteiam de novo a cada etapa, então
/// cada ponto de σ aqui derruba ρ(N,N+1) com eficiência 1,0 — é por aqui que se fecha o sintoma.
pub const CALIBRE_DE_EVENTO: f64 = 2.6;

/// Calibre da camada COM memória (forma). Segurado abaixo do outro de propósito: com ρ = 0,65
/// ela devolve só 0,35 de eficiência no sintoma, e crescê-la junto gastaria orçamento de
/// variância sem comprar queda de correlação.
pub const CALIBRE_SERIAL: f64 = 1.8;

/// Camada 1 — escala (1 σ) da afinidade piloto × pista, em pontos de skill.
pub const AFINIDADE_ESCALA_PONTOS: f64 = 3.0 * CALIBRE_DE_EVENTO;

/// Camada 2 — escala (1 σ) da forma do momento, em pontos de skill.
pub const FORMA_ESCALA_PONTOS: f64 = 2.0 * CALIBRE_SERIAL;

/// Camada 3 — escala (1 σ) do acerto de fim de semana, em pontos de skill.
pub const ACERTO_ESCALA_PONTOS: f64 = 2.5 * CALIBRE_DE_EVENTO;

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

/// Camada 2 — quanto ânimo (motivação + confiança) empurra a média da forma, em desvios-padrão.
///
/// **Desligado (0,0), e não por acaso.** Ele nasceu para fechar o laço pódio → confiança →
/// ritmo → pódio, e o diagnóstico foi que o laço já está fechado DUAS VEZES fora daqui:
///
/// - `pressure::motivation_pace_delta` é elo próprio da esteira ("o desmotivado rende abaixo do
///   próprio skill");
/// - `confianca` entra direto no score de segmento, com peso 0,20 no trecho `Finish` — o maior
///   peso dela em `race/pontuacao.rs`.
///
/// Então este termo era um TERCEIRO caminho para as mesmas duas variáveis de estado, e o único
/// que entrava sem aparecer. Pior: como constante dentro de um AR(1), ele vira média
/// estacionária `c/(1−ρ)` — ×2,86 com ρ = 0,65 — e injetava ~0,7 ponto de espalhamento
/// PERMANENTE, correlacionado com resultado, dentro da camada que toda a análise trata como
/// serial. Consequências medidas: o σ realizado da forma somava serial com permanente, a
/// eficiência-ρ real era menor que a assumida, e a decomposição contabilizava permanente como
/// camada de evento — subestimando o déficit que a campanha vai atacar.
///
/// Um detalhe que fechou o diagnóstico: `normalizar_atributo` centra em 50, mas o neutro
/// declarado da motivação é 70 (`MOTIVATION_REF`, com teste próprio afirmando efeito zero ali).
/// No ponto que o outro módulo chama de neutro, `animo` valia 0,4 — nunca zero.
///
/// **Zerar é remover contagem dupla, não remover o laço.** Ele sobrevive pelos dois caminhos
/// que já o carregam, e a forma volta a ser o que a doc dela afirma: σ = 1 por construção e
/// ρ = 0,65 de verdade, serial de ponta a ponta. O parâmetro continua existindo em
/// [`EscalasDeForma::peso_animo`] para o harness medir a contaminação (0,20 contra 0,0) antes
/// do recongelamento do baseline.
pub const FORMA_PESO_ANIMO: f64 = 0.0;

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
    afinidade_pista_com_escala(driver_id, track_id, estilo, AFINIDADE_ESCALA_PONTOS)
}

/// [`afinidade_pista`] com a escala por parâmetro — o que a campanha varre.
pub fn afinidade_pista_com_escala(
    driver_id: &str,
    track_id: u32,
    estilo: &EstiloPiloto,
    escala: f64,
) -> f64 {
    let h = misturar(misturar(FNV_OFFSET, driver_id.as_bytes()), b"|pista|");
    let h = misturar_num(h, track_id as i64);
    let idiossincratico = normal_aprox(h);
    let caracter = get_track_simulation_data(track_id).track_character;
    let estilo = estilo.casamento_com(caracter) * AFINIDADE_GANHO_ESTILO;

    let adimensional = AFINIDADE_FRACAO_IDIOSSINCRATICA.sqrt() * idiossincratico
        + (1.0 - AFINIDADE_FRACAO_IDIOSSINCRATICA).sqrt() * estilo;

    escala * adimensional.clamp(-TETO_SIGMAS, TETO_SIGMAS)
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
    proxima_forma_com_rho(forma_atual, semente, motivacao, confianca, FORMA_RHO)
}

/// [`proxima_forma`] com ρ por parâmetro — o que a campanha varre.
///
/// O `√(1 − ρ²)` do ruído acompanha o ρ escolhido, e não o `const`: é isso que mantém a
/// variância estacionária em 1 para QUALQUER ρ, e não só para o default. Sem esse acoplamento,
/// varrer ρ mudaria a amplitude junto e a busca mediria dois efeitos de uma vez.
pub fn proxima_forma_com_rho(
    forma_atual: f64,
    semente: u64,
    motivacao: f64,
    confianca: f64,
    rho: f64,
) -> f64 {
    proxima_forma_com_escalas(
        forma_atual,
        semente,
        motivacao,
        confianca,
        rho,
        FORMA_PESO_ANIMO,
    )
}

/// [`proxima_forma`] com ρ E peso do ânimo por parâmetro — o que a campanha varre.
///
/// `peso_animo = 0` desliga o deslocamento por qualidade do piloto e deixa só a parte SERIAL do
/// AR(1). Comparar com e sem é o que separa a variância serial da permanente dentro da forma.
pub fn proxima_forma_com_escalas(
    forma_atual: f64,
    semente: u64,
    motivacao: f64,
    confianca: f64,
    rho: f64,
    peso_animo: f64,
) -> f64 {
    let rho = rho.clamp(0.0, 0.99);
    let animo = (normalizar_atributo(motivacao) + normalizar_atributo(confianca)) / 2.0;
    let ruido = (1.0 - rho * rho).sqrt() * normal_aprox(semente);
    (rho * forma_atual.clamp(-TETO_SIGMAS, TETO_SIGMAS) + peso_animo * animo + ruido)
        .clamp(-TETO_SIGMAS, TETO_SIGMAS)
}

/// Converte o estado adimensional da forma (σ = 1) em pontos de skill.
pub fn forma_em_pontos(estado: f64) -> f64 {
    forma_em_pontos_com_escala(estado, FORMA_ESCALA_PONTOS)
}

/// [`forma_em_pontos`] com a escala por parâmetro — o que a campanha varre.
pub fn forma_em_pontos_com_escala(estado: f64, escala: f64) -> f64 {
    estado.clamp(-TETO_SIGMAS, TETO_SIGMAS) * escala
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
    acerto_fim_de_semana_com_escala(temporada, rodada, team_id, driver_id, ACERTO_ESCALA_PONTOS)
}

/// [`acerto_fim_de_semana`] com a escala por parâmetro — o que a campanha varre.
pub fn acerto_fim_de_semana_com_escala(
    temporada: i32,
    rodada: i32,
    team_id: &str,
    driver_id: &str,
    escala: f64,
) -> f64 {
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

    escala * adimensional.clamp(-TETO_SIGMAS, TETO_SIGMAS)
}

/// Qual acerto se está pedindo: o de volta única ou o de distância de prova.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrimDeAcerto {
    /// Ritmo de prova — pneu aguentando, carro estável com tanque cheio.
    Corrida,
    /// Volta única — o carro no limite por um giro só.
    Classificacao,
}

/// Correlação entre o acerto de classificação e o de corrida do mesmo fim de semana.
///
/// **1,0 era o valor implícito até aqui**, e era o buraco: a esteira pedia
/// [`acerto_fim_de_semana_com_escala`] uma vez e entregava o MESMO número aos dois canais, então
/// quem achava o acerto no sábado tinha exatamente o mesmo ganho no domingo. Consequência medida:
/// com as camadas de evento calibradas, o pole passou a vencer 79% das corridas na rookie e 86%
/// na gt3 (alvos 15–35% e 30–55%) — não por herança de posição, mas porque o bom fim de semana
/// movia classificação e corrida em bloco. **A corrida era decidida no sábado.**
///
/// Trim de quali e trim de corrida são setups diferentes no automobilismo real, e uma equipe
/// pode cravar um e errar o outro. 0,55 mantém o fim de semana coerente na maioria das vezes
/// — quem vai bem no sábado costuma ir bem no domingo — deixando espaço para o caso interessante,
/// que é a pole que não converte e o quarto do grid com ritmo de prova.
///
/// **A σ de cada canal é preservada exatamente** (os pesos são `√ρ` e `√(1−ρ)`, cuja soma de
/// quadrados é 1). Isso é deliberado: o orçamento de variância já está calibrado, e este ajuste
/// só reparte correlação entre canais — não adiciona nem remove variância de evento.
pub const CORRELACAO_ENTRE_TRIMS: f64 = 0.55;

/// [`acerto_fim_de_semana_com_escala`] separado por canal — o que a esteira deve chamar.
///
/// Devolve `√ρ × comum + √(1−ρ) × específico do canal`, com o comum sendo exatamente o acerto de
/// antes. Com `CORRELACAO_ENTRE_TRIMS = 1,0` reproduz o comportamento anterior, o que torna a
/// mudança auditável por um valor de constante.
pub fn acerto_fim_de_semana_por_canal(
    temporada: i32,
    rodada: i32,
    team_id: &str,
    driver_id: &str,
    escala: f64,
    trim: TrimDeAcerto,
) -> f64 {
    let comum = acerto_fim_de_semana_com_escala(temporada, rodada, team_id, driver_id, escala);

    let rotulo: &[u8] = match trim {
        TrimDeAcerto::Corrida => b"trim|corrida",
        TrimDeAcerto::Classificacao => b"trim|classificacao",
    };
    let semente = misturar_num(
        misturar_num(
            misturar(
                misturar(misturar(FNV_OFFSET, rotulo), team_id.as_bytes()),
                driver_id.as_bytes(),
            ),
            temporada as i64,
        ),
        rodada as i64,
    );
    let especifico = escala * normal_aprox(semente).clamp(-TETO_SIGMAS, TETO_SIGMAS);

    CORRELACAO_ENTRE_TRIMS.sqrt() * comum + (1.0 - CORRELACAO_ENTRE_TRIMS).sqrt() * especifico
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
    ajuste_fim_de_semana_com_escalas(
        driver_id,
        team_id,
        track_id,
        temporada,
        rodada,
        estilo,
        estado_forma,
        &EscalasDeForma::default(),
    )
}

/// [`ajuste_fim_de_semana`] com as escalas por parâmetro — o que a campanha varre.
#[allow(clippy::too_many_arguments)]
pub fn ajuste_fim_de_semana_com_escalas(
    driver_id: &str,
    team_id: &str,
    track_id: u32,
    temporada: i32,
    rodada: i32,
    estilo: &EstiloPiloto,
    estado_forma: f64,
    escalas: &EscalasDeForma,
) -> AjusteFimDeSemana {
    let afinidade = afinidade_pista_com_escala(driver_id, track_id, estilo, escalas.afinidade);

    // A forma do momento É comum aos dois canais: ela é o estado do piloto, e um piloto em baixa
    // está em baixa no sábado e no domingo. O ACERTO não — trim de volta única e trim de prova
    // são setups distintos, e é essa separação que impede o fim de semana de andar em bloco.
    // Ver `CORRELACAO_ENTRE_TRIMS` para o número e para o que ele custou descobrir.
    let forma_pontos = forma_em_pontos_com_escala(estado_forma, escalas.forma);
    let acerto = |trim| {
        acerto_fim_de_semana_por_canal(
            temporada,
            rodada,
            team_id,
            driver_id,
            escalas.acerto,
            trim,
        )
    };

    AjusteFimDeSemana {
        corrida: afinidade + forma_pontos + acerto(TrimDeAcerto::Corrida),
        classificacao: afinidade * MULT_AFINIDADE_QUALI
            + forma_pontos
            + acerto(TrimDeAcerto::Classificacao),
    }
}

#[cfg(test)]
mod tests;
