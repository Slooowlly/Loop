//! **A esteira de modificadores**, como função PURA.
//!
//! Todo modificador de fim de semana passa por aqui: conhecimento de pista, adaptação de
//! categoria, lesão, as três camadas de `forma`, motivação e pressão. Cada elo desconta (ou
//! dá) pontos de skill ao `SimDriver` antes dele entrar na simulação.
//!
//! ## Por que ela saiu do comando
//!
//! A esteira morava dentro de `commands::race::simulacao`, num caminho que exige `AppHandle` e
//! banco. Consequência medida pelo harness: **o pacote das camadas de forma estava invisível
//! para a régua** — a camada de evento reportada em 3,1%/3,4% media clima e perfil de pista, e
//! mais nada, porque o harness não conseguia chamar a esteira e tinha que ESPELHAR a aplicação
//! dela num `#[cfg(test)]` sem guarda automática. Um espelho sem guarda deriva em silêncio.
//!
//! Aqui ela é chamável dos dois lados, e o espelho pode ser apagado: o comando e o harness
//! passam a rodar a mesma função, o que é melhor que qualquer teste de paridade.
//!
//! ## As duas armadilhas que a esteira carrega, e que esta função finalmente NOMEIA
//!
//! **1. Um ponto de skill não é um ponto de score.** `skill` pesa ~0,28 do score de segmento em
//! `race/pontuacao.rs`; o resto vai para carro, racecraft, pneu, físico e cabeça. O que a
//! esteira cobra chega diluído a menos de um terço na pista — os 8 pontos de `MAX_PENALTY` do
//! conhecimento de pista valem ~2,2 pontos de score.
//!
//! **2. `SimDriver::skill` é `u8`, e cada passo arredonda.** São SEIS passos, não um no fim. Um
//! modificador menor que meio ponto pode sumir inteiro, e o erro dos seis não se cancela.
//!
//! A segunda é a razão de [`DeltasDoPiloto`] existir. Os seis arredondamentos **não têm
//! assinatura**: os oito elos entram todos no mesmo `skill`, no mesmo fim de semana, sem eixo
//! que os distinga por fora — então medir antes/depois entrega o efeito somado e não diz de
//! quem é. Devolver o que cada elo PRETENDEU e o que cada passo APLICOU resolve por subtração,
//! sem simulação adicional: "o `MAX_PENALTY = 8,0` entregou 8 pontos?" vira comparação exata no
//! mesmo passe. É também o que transforma "mudou número" em achado nomeado em vez de suspeita
//! sobre a refatoração.
//!
//! Nenhum dos dois é consertado aqui. São decisões de calibração e o dono é a campanha.

use crate::simulation::context::SimDriver;
use crate::simulation::pressure::PressureEffect;
use crate::simulation::track_knowledge::TrackKnowledge;

// ─────────────────────────── Os eixos do relatório ───────────────────────────

/// Os oito elos da esteira. As três camadas de `forma` aparecem **separadas**: a fase 1 da
/// campanha redistribui entre afinidade, forma e acerto antes de mexer na soma delas, e um
/// número agregado de "forma total" esconderia exatamente o que ela precisa decidir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EloDaEsteira {
    ConhecimentoDePista,
    AdaptacaoDeCategoria,
    Lesao,
    /// `forma.rs` camada 1 — permanente, por hash de (piloto, pista).
    AfinidadeDePista,
    /// `forma.rs` camada 2 — AR(1) entre etapas, a única com estado.
    FormaDoMomento,
    /// `forma.rs` camada 3 — hash de (temporada, etapa, equipe).
    AcertoDeFimDeSemana,
    Motivacao,
    Pressao,
}

/// Os seis passos de arredondamento. **Menos que os elos, de propósito**: as três camadas de
/// `forma` somam em `f64` e arredondam UMA vez, que é o certo para a resolução. Por isso o
/// `aplicado` é reportado por PASSO e o `pretendido` por ELO — atribuir o arredondamento
/// compartilhado a uma das três camadas exigiria inventar uma convenção, e um número inventado
/// é pior que um número ausente.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassoDeArredondamento {
    ConhecimentoDePista,
    AdaptacaoDeCategoria,
    Lesao,
    /// As três camadas de `forma.rs`, somadas antes de arredondar.
    CamadasDeForma,
    Motivacao,
    Pressao,
}

impl EloDaEsteira {
    /// Em qual passo de arredondamento este elo cai.
    pub fn passo(self) -> PassoDeArredondamento {
        match self {
            Self::ConhecimentoDePista => PassoDeArredondamento::ConhecimentoDePista,
            Self::AdaptacaoDeCategoria => PassoDeArredondamento::AdaptacaoDeCategoria,
            Self::Lesao => PassoDeArredondamento::Lesao,
            Self::AfinidadeDePista | Self::FormaDoMomento | Self::AcertoDeFimDeSemana => {
                PassoDeArredondamento::CamadasDeForma
            }
            Self::Motivacao => PassoDeArredondamento::Motivacao,
            Self::Pressao => PassoDeArredondamento::Pressao,
        }
    }
}

/// Os dois canais que a esteira alimenta. São separados porque a afinidade de pista entra com
/// `MULT_AFINIDADE_QUALI` na classificação e as outras camadas não recebem nada — e foi essa
/// assimetria que explicou por que ρ(grid × chegada) não se mexia ao ligar a esteira: a camada
/// mais forte empurra grid e chegada na MESMA direção. Colapsar os dois canais num número
/// esconderia a assimetria justamente no parâmetro candidato a inverter de sinal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Canal {
    /// `SimDriver::skill` — o que a corrida lê.
    Corrida,
    /// `SimDriver::ritmo_classificacao` — o que a classificação lê.
    Classificacao,
}

/// O que um elo QUIS fazer, em pontos de skill, num canal.
#[derive(Debug, Clone, Copy)]
pub struct Pretendido {
    pub elo: EloDaEsteira,
    pub canal: Canal,
    pub pontos: f64,
}

/// O que um passo de arredondamento REALMENTE aplicou ao `u8`, num canal.
#[derive(Debug, Clone, Copy)]
pub struct Aplicado {
    pub passo: PassoDeArredondamento,
    pub canal: Canal,
    /// Diferença efetiva no campo `u8` depois de clamp e arredondamento.
    pub pontos: f64,
}

/// O relatório de um piloto: o que cada elo pretendeu e o que cada passo aplicou.
#[derive(Debug, Clone, Default)]
pub struct DeltasDoPiloto {
    pub driver_id: String,
    pub pretendido: Vec<Pretendido>,
    pub aplicado: Vec<Aplicado>,
}

impl DeltasDoPiloto {
    /// Soma do pretendido de um elo nos dois canais — atalho de leitura.
    pub fn pretendido_de(&self, elo: EloDaEsteira, canal: Canal) -> f64 {
        self.pretendido
            .iter()
            .filter(|p| p.elo == elo && p.canal == canal)
            .map(|p| p.pontos)
            .sum()
    }

    /// O que um passo aplicou num canal.
    pub fn aplicado_de(&self, passo: PassoDeArredondamento, canal: Canal) -> f64 {
        self.aplicado
            .iter()
            .filter(|a| a.passo == passo && a.canal == canal)
            .map(|a| a.pontos)
            .sum()
    }

    /// **A perda por quantização deste piloto**, por passo e canal: o que o elo (ou o grupo de
    /// elos) pediu menos o que o `u8` aceitou. É a resposta direta a "o `MAX_PENALTY = 8,0`
    /// entregou 8 pontos?".
    pub fn perda_por_quantizacao(&self, passo: PassoDeArredondamento, canal: Canal) -> f64 {
        let pedido: f64 = self
            .pretendido
            .iter()
            .filter(|p| p.elo.passo() == passo && p.canal == canal)
            .map(|p| p.pontos)
            .sum();
        pedido - self.aplicado_de(passo, canal)
    }
}

// ─────────────────────────── A entrada ───────────────────────────

/// O que a esteira precisa saber de um piloto e que NÃO cabe no `SimDriver`.
///
/// `Default` é o piloto neutro: pista desconhecida, são, sem pressão. É o que o harness passa
/// — ele monta grid sintético e não tem histórico de circuitos, lesão nem classificação.
#[derive(Debug, Clone, Default)]
pub struct ContextoDoPiloto {
    /// Conhecimento desta pista, do `historico_circuitos` do piloto.
    pub conhecimento_de_pista: TrackKnowledge,
    /// Extensão da pista em km — decide se dominar leva 2 ou 3 corridas.
    pub comprimento_da_pista_km: f64,
    /// Fração do skill que a lesão cobra (`skill_penalty` × recuperação). `None` = são.
    pub fracao_de_lesao: Option<f64>,
    /// Entradas da pressão. `None` = sem pressão (o que o harness passa).
    pub pressao: Option<EntradaDePressao>,
}

/// O que a pressão precisa. As duas primeiras fontes vêm RESOLVIDAS de fora (dependem da
/// classificação do campeonato e do interesse do evento, que o harness não tem); o duelo é
/// resolvido AQUI porque o gate dele lê o `skill` corrente do piloto — já modificado pelos
/// cinco elos anteriores. Passá-lo resolvido de fora mudaria o valor do gate, e isso seria uma
/// mudança de comportamento disfarçada de refatoração.
#[derive(Debug, Clone, Copy, Default)]
pub struct EntradaDePressao {
    /// Pressão de campeonato, já resolvida.
    pub campeonato: PressureEffect,
    /// Pressão de casa cheia, já resolvida.
    pub evento: PressureEffect,
    /// Duelo (Nemesis): (percebida do par, skill do oponente). `None` = sem rival na corrida.
    pub duelo: Option<(f64, f64)>,
    pub mentalidade: f64,
    pub experiencia: f64,
}

impl Default for PressureEffect {
    fn default() -> Self {
        PressureEffect::NONE
    }
}

/// O σ REALIZADO do pretendido de um elo, medido sobre o lote — **só no canal
/// [`Canal::Corrida`]**.
///
/// ## Por que realizado e não nominal
///
/// As três escalas do `forma.rs` são o σ da camada POR DESENHO, então a resposta óbvia seria
/// devolver a constante (ou, agora, o parâmetro). Ela está errada, e erra por fatores
/// DIFERENTES em cada camada — o que é pior que errar por um fator só, porque as três continuam
/// parecendo plausíveis:
///
/// - **Afinidade:** `escala × adimensional.clamp(±TETO_SIGMAS)`. O corte em 2,5σ trunca a cauda
///   e ENCOLHE o σ realizado. É viés, não ruído.
/// - **Forma:** a construção AR(1) fecha em variância estacionária 1, mas há
///   `peso_animo × animo` somado por cima, que ADICIONA variância e é amplificada pela
///   persistência do AR. O σ realizado é maior que a escala nominal.
/// - **Acerto:** também passa pelo clamp, com o mesmo encolhimento da afinidade.
///
/// ## Por que só o canal de corrida
///
/// Um σ por camada, nunca um por canal. Normalizar cada canal pelo σ da própria distribuição
/// parece mais rigoroso e apaga exatamente o que o canal de classificação existe para mostrar:
/// se o σ da quali também for 1,5× maior, os dois z saem idênticos, o campo empata sempre em
/// todas as camadas, e a assimetria do `MULT_AFINIDADE_QUALI` desaparece sem deixar rastro.
/// **O σ da classificação não é exposto de propósito** — é melhor não mandá-lo do que arriscar
/// que alguém o "conserte" para dentro depois.
#[derive(Debug, Clone, Copy)]
pub struct SigmaRealizado {
    pub elo: EloDaEsteira,
    /// σ do pretendido no canal de corrida, sobre o lote medido.
    pub sigma: f64,
    /// Quantos pilotos entraram na conta.
    pub amostras: usize,
}

/// O que a esteira devolve.
#[derive(Debug, Clone)]
pub struct ResultadoDaEsteira {
    /// O grid ajustado, na mesma ordem da entrada.
    pub grid: Vec<SimDriver>,
    /// O estado de forma AVANÇADO de cada piloto, na mesma ordem. **Devolvido, não gravado** —
    /// a persistência é do comando, e é isso que tira o `if persistence_mode` do meio da
    /// simulação.
    pub estado_de_forma: Vec<f64>,
    /// Por piloto: o que cada elo pretendeu e o que cada passo aplicou.
    pub deltas: Vec<DeltasDoPiloto>,
}

impl ResultadoDaEsteira {
    /// σ REALIZADO do pretendido de cada elo neste lote, canal de corrida. Ver
    /// [`SigmaRealizado`] para o porquê de ser realizado e de ser só um canal.
    ///
    /// Um lote é uma corrida: para um grid de 24, são 24 amostras por elo, o que é pouco para
    /// um σ estável. Quem consome deve **agrupar vários lotes** — a conta é sobre a
    /// distribuição, não sobre a corrida.
    pub fn sigma_realizado(&self) -> Vec<SigmaRealizado> {
        [
            EloDaEsteira::ConhecimentoDePista,
            EloDaEsteira::AdaptacaoDeCategoria,
            EloDaEsteira::Lesao,
            EloDaEsteira::AfinidadeDePista,
            EloDaEsteira::FormaDoMomento,
            EloDaEsteira::AcertoDeFimDeSemana,
            EloDaEsteira::Motivacao,
            EloDaEsteira::Pressao,
        ]
        .into_iter()
        .map(|elo| {
            let v: Vec<f64> = self
                .deltas
                .iter()
                .map(|d| d.pretendido_de(elo, Canal::Corrida))
                .collect();
            let n = v.len();
            let sigma = if n < 2 {
                0.0
            } else {
                let media = v.iter().sum::<f64>() / n as f64;
                (v.iter().map(|x| (x - media).powi(2)).sum::<f64>() / n as f64).sqrt()
            };
            SigmaRealizado {
                elo,
                sigma,
                amostras: n,
            }
        })
        .collect()
    }
}

// ─────────────────────────── A esteira ───────────────────────────

/// Aplica `- pontos` a um `u8` com piso, do jeito que os três primeiros elos fazem
/// (`.max(5.0)`, sem teto) e devolve o quanto REALMENTE mudou.
fn descontar(valor: &mut u8, pontos: f64) -> f64 {
    let antes = *valor as f64;
    *valor = (antes - pontos).max(5.0).round() as u8;
    *valor as f64 - antes
}

/// Aplica `+ pontos` a um `u8` com piso e teto, do jeito que os três últimos elos fazem
/// (`.clamp(5.0, 100.0)`) e devolve o quanto REALMENTE mudou.
fn somar(valor: &mut u8, pontos: f64) -> f64 {
    let antes = *valor as f64;
    *valor = (antes + pontos).clamp(5.0, 100.0).round() as u8;
    *valor as f64 - antes
}

/// **A esteira de modificadores de fim de semana.**
///
/// Ordem dos elos e aritmética idênticas às do comando — inclusive o piso `.max(5.0)` dos três
/// primeiros contra o `.clamp(5.0, 100.0)` dos três últimos, e o fato de a lesão e o headroom
/// da pressão lerem o `skill` CORRENTE (já modificado pelos elos anteriores), não o original.
///
/// `contextos` é paralelo a `grid`; falta de entrada vira contexto neutro.
/// `estado_de_forma` é a forma anterior de cada piloto, na ordem do grid.
pub fn aplicar_esteira(
    grid: &[SimDriver],
    contextos: &[ContextoDoPiloto],
    temporada: i32,
    rodada: i32,
    track_id: u32,
    estado_de_forma: &[f64],
    escalas: &crate::simulation::forma::EscalasDeForma,
) -> ResultadoDaEsteira {
    use crate::simulation::{category_adaptation, forma, pressure, track_knowledge};

    let neutro = ContextoDoPiloto::default();
    let mut saida_grid = Vec::with_capacity(grid.len());
    let mut saida_forma = Vec::with_capacity(grid.len());
    let mut saida_deltas = Vec::with_capacity(grid.len());

    for (i, sd) in grid.iter().enumerate() {
        let ctx = contextos.get(i).unwrap_or(&neutro);
        let mut sd = sd.clone();
        let mut d = DeltasDoPiloto {
            driver_id: sd.id.clone(),
            ..Default::default()
        };

        // Fecha um passo: registra o pretendido de cada elo do passo e o aplicado nos dois
        // canais. `pontos_*` é o que os elos daquele passo pediram, somado por canal.
        macro_rules! registrar {
            ($passo:expr, $canal:expr, $aplicado:expr) => {
                d.aplicado.push(Aplicado {
                    passo: $passo,
                    canal: $canal,
                    pontos: $aplicado,
                });
            };
        }
        macro_rules! pedir {
            ($elo:expr, $canal:expr, $pontos:expr) => {
                d.pretendido.push(Pretendido {
                    elo: $elo,
                    canal: $canal,
                    pontos: $pontos,
                });
            };
        }

        // ── 1. Conhecimento de pista: pista nova/pouco conhecida = mais lento. Só penalidade.
        let pen_pista = track_knowledge::track_knowledge_penalty(
            &ctx.conhecimento_de_pista,
            ctx.comprimento_da_pista_km,
            sd.adaptabilidade as f64,
            temporada,
        );
        pedir!(
            EloDaEsteira::ConhecimentoDePista,
            Canal::Corrida,
            -pen_pista
        );
        pedir!(
            EloDaEsteira::ConhecimentoDePista,
            Canal::Classificacao,
            -pen_pista
        );
        registrar!(
            PassoDeArredondamento::ConhecimentoDePista,
            Canal::Corrida,
            descontar(&mut sd.skill, pen_pista)
        );
        registrar!(
            PassoDeArredondamento::ConhecimentoDePista,
            Canal::Classificacao,
            descontar(&mut sd.ritmo_classificacao, pen_pista)
        );

        // ── 2. Adaptação à categoria: quem acabou de subir corre abaixo do próprio número.
        let pen_adapt = category_adaptation::category_adaptation_penalty(
            sd.corridas_na_categoria as u32,
            sd.adaptabilidade as f64,
            sd.experiencia as f64,
        );
        pedir!(
            EloDaEsteira::AdaptacaoDeCategoria,
            Canal::Corrida,
            -pen_adapt
        );
        pedir!(
            EloDaEsteira::AdaptacaoDeCategoria,
            Canal::Classificacao,
            -pen_adapt
        );
        registrar!(
            PassoDeArredondamento::AdaptacaoDeCategoria,
            Canal::Corrida,
            descontar(&mut sd.skill, pen_adapt)
        );
        registrar!(
            PassoDeArredondamento::AdaptacaoDeCategoria,
            Canal::Classificacao,
            descontar(&mut sd.ritmo_classificacao, pen_adapt)
        );

        // ── 3. Lesão: o machucado CORRE, com ritmo reduzido. Lê o skill CORRENTE.
        let pen_lesao = ctx
            .fracao_de_lesao
            .map(|fracao| sd.skill as f64 * fracao)
            .unwrap_or(0.0);
        pedir!(EloDaEsteira::Lesao, Canal::Corrida, -pen_lesao);
        pedir!(EloDaEsteira::Lesao, Canal::Classificacao, -pen_lesao);
        registrar!(
            PassoDeArredondamento::Lesao,
            Canal::Corrida,
            descontar(&mut sd.skill, pen_lesao)
        );
        registrar!(
            PassoDeArredondamento::Lesao,
            Canal::Classificacao,
            descontar(&mut sd.ritmo_classificacao, pen_lesao)
        );

        // ── 4. As TRÊS camadas de forma, somadas antes de arredondar (um passo só).
        let anterior = estado_de_forma.get(i).copied().unwrap_or(0.0);
        // `_com_escalas` e não `_com_rho`: a segunda usa a `const FORMA_PESO_ANIMO` e deixaria
        // `escalas.peso_animo` órfão. Knob órfão numa medição pareada produz tabelas idênticas
        // com e sem o efeito — um zero que parece resposta.
        let agora = forma::proxima_forma_com_escalas(
            anterior,
            forma::semente_forma(temporada, rodada, &sd.id),
            sd.motivacao,
            sd.confianca as f64,
            escalas.rho,
            escalas.peso_animo,
        );
        let estilo = forma::EstiloPiloto {
            smoothness: sd.smoothness as f64,
            consistencia: sd.consistencia as f64,
            adaptabilidade: sd.adaptabilidade as f64,
            aggression: sd.aggression as f64,
        };
        // As três camadas, medidas SEPARADAMENTE — é o que a fase 1 da campanha redistribui.
        let afinidade =
            forma::afinidade_pista_com_escala(&sd.id, track_id, &estilo, escalas.afinidade);
        let forma_pontos = forma::forma_em_pontos_com_escala(agora, escalas.forma);
        // O acerto é pedido UMA VEZ POR CANAL, e não uma vez só entregue aos dois: trim de volta
        // única e trim de distância de prova são setups diferentes, e é essa diferença que
        // impede a corrida de ser decidida no sábado. Ver `forma::CORRELACAO_ENTRE_TRIMS`.
        let acerto_corrida = forma::acerto_fim_de_semana_por_canal(
            temporada,
            rodada,
            &sd.team_id,
            &sd.id,
            escalas.acerto,
            forma::TrimDeAcerto::Corrida,
        );
        let acerto_quali = forma::acerto_fim_de_semana_por_canal(
            temporada,
            rodada,
            &sd.team_id,
            &sd.id,
            escalas.acerto,
            forma::TrimDeAcerto::Classificacao,
        );

        pedir!(EloDaEsteira::AfinidadeDePista, Canal::Corrida, afinidade);
        pedir!(
            EloDaEsteira::AfinidadeDePista,
            Canal::Classificacao,
            afinidade * forma::MULT_AFINIDADE_QUALI
        );
        pedir!(EloDaEsteira::FormaDoMomento, Canal::Corrida, forma_pontos);
        pedir!(
            EloDaEsteira::FormaDoMomento,
            Canal::Classificacao,
            forma_pontos
        );
        pedir!(
            EloDaEsteira::AcertoDeFimDeSemana,
            Canal::Corrida,
            acerto_corrida
        );
        pedir!(
            EloDaEsteira::AcertoDeFimDeSemana,
            Canal::Classificacao,
            acerto_quali
        );

        let ajuste = forma::ajuste_fim_de_semana_com_escalas(
            &sd.id,
            &sd.team_id,
            track_id,
            temporada,
            rodada,
            &estilo,
            agora,
            escalas,
        );
        registrar!(
            PassoDeArredondamento::CamadasDeForma,
            Canal::Corrida,
            somar(&mut sd.skill, ajuste.corrida)
        );
        registrar!(
            PassoDeArredondamento::CamadasDeForma,
            Canal::Classificacao,
            somar(&mut sd.ritmo_classificacao, ajuste.classificacao)
        );

        // ── 5. Motivação: um desmotivado corre ABAIXO do próprio skill.
        let mot = pressure::motivation_pace_delta(sd.motivacao);
        pedir!(EloDaEsteira::Motivacao, Canal::Corrida, mot);
        pedir!(EloDaEsteira::Motivacao, Canal::Classificacao, mot);
        registrar!(
            PassoDeArredondamento::Motivacao,
            Canal::Corrida,
            somar(&mut sd.skill, mot)
        );
        registrar!(
            PassoDeArredondamento::Motivacao,
            Canal::Classificacao,
            somar(&mut sd.ritmo_classificacao, mot)
        );

        // ── 6. Pressão (clutch/choke), com headroom sobre o skill CORRENTE.
        let peff = match ctx.pressao {
            Some(p) => {
                // O duelo é resolvido aqui, com o skill JÁ modificado pelos elos anteriores —
                // é o que o gate de pace lia no comando, e mexer nisso seria mudança de
                // comportamento e não refatoração.
                let duelo = match p.duelo {
                    Some((percebida, skill_do_oponente)) => pressure::rivalry_pressure_for(
                        percebida,
                        sd.skill as f64,
                        skill_do_oponente,
                        p.mentalidade,
                        p.experiencia,
                        1.0,
                    ),
                    None => PressureEffect::NONE,
                };
                pressure::combine(pressure::combine(p.campeonato, p.evento), duelo)
            }
            None => PressureEffect::NONE,
        };
        let hr = pressure::headroom_pace_mult(sd.skill as f64, peff.pace_delta >= 0.0);
        let pace_delta = peff.pace_delta * hr;
        pedir!(EloDaEsteira::Pressao, Canal::Corrida, pace_delta);
        pedir!(EloDaEsteira::Pressao, Canal::Classificacao, pace_delta);
        registrar!(
            PassoDeArredondamento::Pressao,
            Canal::Corrida,
            somar(&mut sd.skill, pace_delta)
        );
        registrar!(
            PassoDeArredondamento::Pressao,
            Canal::Classificacao,
            somar(&mut sd.ritmo_classificacao, pace_delta)
        );
        sd.pressure_error_mult = peff.error_mult;

        saida_grid.push(sd);
        saida_forma.push(agora);
        saida_deltas.push(d);
    }

    ResultadoDaEsteira {
        grid: saida_grid,
        estado_de_forma: saida_forma,
        deltas: saida_deltas,
    }
}

#[cfg(test)]
mod tests;
