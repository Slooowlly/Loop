//! Testes da calibração, em duas camadas.
//!
//! **Camada leve (sempre roda)** — testa o HARNESS: a estatística está certa, o gerador é
//! reprodutível e produz o campo que promete, a arena roda uma temporada de ponta a ponta. Não
//! afirma nada sobre o realismo da simulação, então não quebra enquanto o motor estiver sendo
//! consertado em paralelo.
//!
//! **Camada pesada (`#[ignore]`)** — as asserções de faixa de verdade, ~1000 corridas cada.
//! Elas FALHAM hoje, de propósito: são o critério de aceitação do conserto da simulação, não um
//! retrato do estado atual. O baseline medido está em `imprime_baseline`.
//!
//! ```text
//! cargo test --manifest-path src-tauri/Cargo.toml calibracao -- --ignored --nocapture
//! ```
//!
//! # Uma classe de fragilidade a evitar de propósito
//!
//! **Asserção exata sobre propriedade EMERGENTE quebra por deriva de semente, não por defeito.**
//! O `closed_system_playable_world` da verificação do commit é o caso de escola: exigia
//! `active_never_raced == 0` exato sobre algo que emerge da geração de mundo. Ele passava em HEAD,
//! falhava com a reforma pareada ao teste antigo, e voltava a passar com o teste reescrito — e
//! distinguir "defeito" de "outra semente" custou três execuções.
//!
//! É a mesma classe do `reparo_custa`. Por isso, aqui:
//!
//! - o que é ESTRUTURAL (quantas equipes para 21 pilotos, o alargado conter o predicado atual,
//!   níveis com o volume declarado) pode e deve ser exato;
//! - o que é EMERGENTE (correlações, dispersão, frequências, arrependimento) é sempre faixa ou
//!   comparação relativa, medida sobre volume grande e várias sementes;
//! - e quando uma faixa aperta, a média sai sobre eixos e sementes múltiplos, não sobre uma
//!   medição só — foi assim que a guarda da peneira T1 deixou de ser frágil.
//!
//! Vale relembrar quando a campanha começar a mover parâmetros: ali a deriva de semente é a regra,
//! não a exceção.

use super::alvos::{Alvos, Faixa};
use super::ancora;
use super::arena::{self, AjustesCtx, ConfigTemporada};
use super::assinatura;
use super::atrito;
use super::busca;
use super::campo::{gerar_campo, melhor_do_grid, nivelar_carros, nivelar_pilotos, PerfilCampo};
use super::consumo;
use super::metricas::{self, spearman};
use super::previa::MovimentosDaCamadaDeEvento;
use super::processo;
use super::relatorio;
use super::seguranca;
use super::snapshot;
use super::variancia::{self, ConfigDecomposicao};
use super::varredura::{self, Knob};
use crate::simulation::forma::EscalasDeForma;

// ---------------------------------------------------------------------------
// Camada leve — o harness
// ---------------------------------------------------------------------------

#[test]
fn spearman_de_ordens_identicas_e_um() {
    let a = [1.0, 2.0, 3.0, 4.0, 5.0];
    let rho = spearman(&a, &a).expect("correlação");
    assert!((rho - 1.0).abs() < 1e-9, "rho={rho}");
}

#[test]
fn spearman_de_ordens_invertidas_e_menos_um() {
    let a = [1.0, 2.0, 3.0, 4.0, 5.0];
    let b = [5.0, 4.0, 3.0, 2.0, 1.0];
    let rho = spearman(&a, &b).expect("correlação");
    assert!((rho + 1.0).abs() < 1e-9, "rho={rho}");
}

#[test]
fn spearman_e_monotonico_nao_linear() {
    // Spearman só olha posto: uma transformação monotônica não pode mexer no valor.
    let a: [f64; 5] = [1.0, 2.0, 3.0, 4.0, 5.0];
    let b: Vec<f64> = a.iter().map(|x| x.powi(3)).collect();
    let rho = spearman(&a, &b).expect("correlação");
    assert!((rho - 1.0).abs() < 1e-9, "rho={rho}");
}

#[test]
fn faixa_classifica_baixo_ok_e_alto() {
    let f = Faixa::nova(0.2, 0.5);
    assert_eq!(f.veredito(0.1), "BAIXO");
    assert_eq!(f.veredito(0.35), "ok");
    assert_eq!(f.veredito(0.9), "ALTO");
    assert!(f.contem(0.2) && f.contem(0.5));
    assert!(!f.contem(f64::NAN));
}

#[test]
fn campo_e_reprodutivel_pela_semente() {
    let perfil = PerfilCampo::rookie();
    let a = gerar_campo(&perfil, 20, 99);
    let b = gerar_campo(&perfil, 20, 99);
    let c = gerar_campo(&perfil, 20, 100);

    let assinatura = |g: &[crate::simulation::context::SimDriver]| -> Vec<(String, u8, u8)> {
        g.iter()
            .map(|d| (d.id.clone(), d.skill, d.consistencia))
            .collect()
    };

    assert_eq!(assinatura(&a), assinatura(&b), "mesma semente, mesmo grid");
    assert_ne!(assinatura(&a), assinatura(&c), "semente nova, grid novo");
}

/// **O guard mais importante do gerador**: a estrutura de correlação deste harness tem que bater
/// com a do gerador REAL do jogo, atributo por atributo.
///
/// Não compara constantes copiadas — compara contra `Driver::generate_for_category`. Se alguém
/// mudar a repartição em `driver_generation.rs`, este teste acusa, e é o único jeito de a régua
/// não medir um mundo que não existe.
///
/// O defeito que ele existe para não repetir: sete atributos saíam do talento quando no jogo são
/// sorteados livres — `gestao_pneus`, `habilidade_largada`, `adaptabilidade`, `mentalidade`,
/// `fator_chuva`, `confianca` e `smoothness`. Justamente os eixos dos pacotes D e G.
#[test]
fn reparticao_espelha_a_geracao_do_jogo() {
    for perfil in [PerfilCampo::rookie(), PerfilCampo::gt3()] {
        let comparacao = super::campo::comparar_com_gerador_real(&perfil, 300, 4242);
        for (nome, jogo, harness) in &comparacao {
            // A tolerância é larga de propósito: o que importa é a CLASSE (correlacionado vs
            // independente), não a terceira casa. Um atributo que o jogo sorteia livre (ρ≈0) não
            // pode sair daqui com ρ alto, e vice-versa.
            // A asserção é sobre a CLASSE (correlacionado vs independente), não sobre o valor.
            //
            // Comparar o valor seria errado: ρ(skill, atributo correlacionado) depende do
            // ESPALHAMENTO do skill no campo. Com ±8 fixo, campo apertado dá ρ baixo e campo largo
            // dá ρ alto — pela MESMA regra de geração. O harness espalha o talento de propósito
            // (cauda longa, para ter craque e fraco no mesmo grid), então ρ alto num atributo
            // correlacionado é esperado e não é divergência. O espalhamento em si é medido
            // separadamente por `espalhamento_do_skill`.
            let mesma_classe = (jogo.abs() > 0.30) == (harness.abs() > 0.30);
            assert!(
                mesma_classe,
                "{} [{}]: o jogo correlaciona a {:.2} e o harness a {:.2} — CLASSES diferentes",
                nome,
                perfil.rotulo(),
                jogo,
                harness
            );
        }
    }
}

/// O espalhamento do talento é a outra metade do problema: a repartição pode estar certa e o campo
/// ainda medir errado se ele for mais largo em skill que o do jogo — porque toda vantagem
/// determinística escala com essa largura.
#[test]
#[ignore = "diagnóstico; roda com --nocapture"]
fn imprime_espalhamento_do_skill() {
    println!("\n== ESPALHAMENTO DO SKILL: jogo vs harness ==\n");
    println!(
        "| {:<14} | {:>12} | {:>12} | {:>10} |",
        "categoria", "jogo (m/dp)", "harness", "razão dp"
    );
    println!("|----------------|--------------|--------------|------------|");
    for perfil in [PerfilCampo::rookie(), PerfilCampo::gt3()] {
        let ((mj, dj), (mh, dh)) = super::campo::espalhamento_do_skill(&perfil, 400, 4242);
        println!(
            "| {:<14} | {:>5.1}/{:<6.1} | {:>5.1}/{:<6.1} | {:>10.2} |",
            perfil.rotulo(),
            mj,
            dj,
            mh,
            dh,
            dh / dj.max(1e-9)
        );
    }
    println!(
        "\nRazão de dp acima de ~1,3 significa que o harness infla a vantagem determinística e \
         mede mais determinismo do que existe."
    );
}

/// A segunda metade do alinhamento da régua, e ela vale um teste próprio: **o espalhamento do
/// talento multiplica toda vantagem determinística.** Um campo mais largo em skill que o do jogo
/// mede determinismo que não existe, mesmo com a repartição dos atributos perfeita.
///
/// O defeito que ele pega: o gt3 do harness espalhava 38% mais que o do jogo (dp 6,1 contra 4,4).
///
/// Ressalva honesta: isto casa MÉDIA e DESVIO, não a forma da distribuição. O jogo sorteia skill
/// quase uniforme numa faixa (`roll_stat`), com estrutura extra no tier 0 (perfis de rookie e
/// prodígio); o harness usa normal com cauda esticada. Casar os dois primeiros momentos resolve o
/// erro de primeira ordem — a diferença de forma continua sendo uma aproximação conhecida.
#[test]
fn espalhamento_do_skill_acompanha_o_jogo() {
    for perfil in [PerfilCampo::rookie(), PerfilCampo::gt3()] {
        let ((media_jogo, dp_jogo), (media_harness, dp_harness)) =
            super::campo::espalhamento_do_skill(&perfil, 400, 4242);
        let razao = dp_harness / dp_jogo;
        assert!(
            (0.80..=1.25).contains(&razao),
            "{}: desvio do skill em {:.2}× o do jogo (harness {:.1}, jogo {:.1}) — o campo \
             está distorcendo a vantagem determinística",
            perfil.rotulo(),
            razao,
            dp_harness,
            dp_jogo
        );
        assert!(
            (media_harness - media_jogo).abs() < 5.0,
            "{}: média do skill {:.1} contra {:.1} do jogo",
            perfil.rotulo(),
            media_harness,
            media_jogo
        );
    }
}

#[test]
fn atributos_livres_no_jogo_nao_saem_do_talento() {
    // A metade que mais dói se estiver errada: são os eixos que os pacotes D e G fortaleceram.
    let grid = gerar_campo(&PerfilCampo::gt3(), 400, 91);
    let assinatura = super::campo::assinatura_do_campo(&grid);
    for nome in [
        "gestao_pneus",
        "habilidade_largada",
        "adaptabilidade",
        "mentalidade",
        "fator_chuva",
        "confianca",
    ] {
        let (_, rho) = assinatura
            .iter()
            .find(|(n, _)| *n == nome)
            .expect("atributo na assinatura");
        assert!(
            rho.abs() < 0.15,
            "{nome} deveria ser independente do talento, e saiu com rho={rho:.3}"
        );
    }
}

#[test]
fn smoothness_e_o_inverso_da_agressividade() {
    // O trade-off "quem anda no limite castiga o pneu". Derivar smoothness do talento o destrói,
    // e `gestao_pneus`/`smoothness` alimentam o undercut do pacote G.
    let grid = gerar_campo(&PerfilCampo::rookie(), 300, 77);
    let agr: Vec<f64> = grid.iter().map(|d| d.aggression as f64).collect();
    let smo: Vec<f64> = grid.iter().map(|d| d.smoothness as f64).collect();
    let rho = spearman(&agr, &smo).expect("correlação");
    assert!(
        rho < -0.75,
        "smoothness tem que ser o inverso da agressividade (rho={rho:.3})"
    );
}

#[test]
fn campo_nao_e_uma_escada_perfeita() {
    // O defeito que este gerador existe para não repetir: no grid dos testes de `engine.rs`
    // o melhor de skill é também o melhor de largada, de pneu e de racecraft. Aqui a ordem por
    // skill e a ordem por largada TÊM que divergir.
    let grid = gerar_campo(&PerfilCampo::rookie(), 24, 5);

    let por_skill: Vec<f64> = grid.iter().map(|d| d.skill as f64).collect();
    let por_largada: Vec<f64> = grid.iter().map(|d| d.habilidade_largada as f64).collect();
    let por_consistencia: Vec<f64> = grid.iter().map(|d| d.consistencia as f64).collect();

    let rho_largada = spearman(&por_skill, &por_largada).expect("correlação");
    let rho_consistencia = spearman(&por_skill, &por_consistencia).expect("correlação");

    // A largada é INDEPENDENTE do talento no jogo (`roll_stat(40, 70)`), então aqui ela tem que
    // sair descorrelacionada — não só "não idêntica".
    assert!(
        rho_largada.abs() < 0.20,
        "largada é sorteio livre no jogo e não pode acompanhar o skill (rho={rho_largada:.3})"
    );
    // A consistência, ao contrário, o jogo CORRELACIONA (`correlated_stat(skill, 10)`). A versão
    // anterior deste teste exigia rho < 0,75 e estava asseverando o BUG: o gerador soltava a
    // consistência do talento quando o jogo a amarra. Correlação alta aqui é o comportamento certo.
    assert!(
        rho_consistencia > 0.60,
        "consistência acompanha o talento no jogo (±10) e tem que acompanhar aqui \
         (rho={rho_consistencia:.3})"
    );
}

#[test]
fn campo_tem_cauda_superior_e_nao_um_sino() {
    // "Poucos craques, um monte de medianos, alguns fracos": a distância do topo à mediana tem
    // que ser maior que a da mediana ao fundo.
    let grid = gerar_campo(&PerfilCampo::rookie(), 200, 17);
    let mut skills: Vec<f64> = grid.iter().map(|d| d.skill as f64).collect();
    skills.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let mediana = skills[skills.len() / 2];
    let topo = skills[skills.len() - 1];
    let fundo = skills[0];

    assert!(
        topo - mediana > (mediana - fundo) * 0.9,
        "a cauda de cima deveria ser pelo menos comparável à de baixo \
         (topo={topo:.1}, mediana={mediana:.1}, fundo={fundo:.1})"
    );
    assert!(
        topo - mediana >= 10.0,
        "sem craque destacado: topo={topo:.1}, mediana={mediana:.1}"
    );

    // E o meio tem que ser gordo: a maioria perto da mediana.
    let perto_da_mediana = skills
        .iter()
        .filter(|s| (*s - mediana).abs() <= 8.0)
        .count();
    assert!(
        perto_da_mediana as f64 / skills.len() as f64 > 0.45,
        "só {perto_da_mediana}/{} pilotos perto da mediana — o campo não tem miolo",
        skills.len()
    );
}

#[test]
fn campo_respeita_o_numero_de_pilotos_e_o_tamanho_de_equipe() {
    let perfil = PerfilCampo::gt3();
    let grid = gerar_campo(&perfil, 21, 3);
    assert_eq!(grid.len(), 21);

    let mut equipes: Vec<&str> = grid.iter().map(|d| d.team_id.as_str()).collect();
    equipes.sort_unstable();
    equipes.dedup();
    // 21 pilotos, 2 por equipe → 11 equipes (a última com um só).
    assert_eq!(equipes.len(), 11);
}

#[test]
fn melhor_do_grid_e_o_de_maior_skill() {
    let grid = gerar_campo(&PerfilCampo::gt3(), 20, 8);
    let esperado = grid
        .iter()
        .max_by_key(|d| d.skill)
        .map(|d| d.id.clone())
        .unwrap();
    assert_eq!(melhor_do_grid(&grid), esperado);
}

#[test]
fn temporada_curta_roda_de_ponta_a_ponta() {
    let config = ConfigTemporada {
        etapas: 4,
        pilotos: 14,
        ..ConfigTemporada::rookie()
    };
    let grid = gerar_campo(&config.perfil, config.pilotos, 21);
    let corridas = arena::rodar_temporada(
        &config,
        &grid,
        &crate::simulation::catalog::IncidentCatalog::empty(),
        21,
        1,
    );

    assert_eq!(corridas.len(), 4);
    for corrida in &corridas {
        assert_eq!(corrida.race_results.len(), 14);
        assert_eq!(corrida.qualifying_results.len(), 14);
        assert!(!corrida.winner_id.is_empty());
        assert!(!corrida.pole_sitter_id.is_empty());
    }

    // As pistas variam dentro da temporada — é isso que dá caráter diferente por etapa.
    let mut pistas: Vec<&str> = corridas.iter().map(|c| c.track_name.as_str()).collect();
    pistas.sort_unstable();
    pistas.dedup();
    assert!(pistas.len() >= 3, "pistas repetidas demais: {pistas:?}");
}

#[test]
fn metricas_saem_num_dominio_valido() {
    let config = ConfigTemporada {
        etapas: 6,
        pilotos: 16,
        ..ConfigTemporada::rookie()
    };
    let agregado = arena::medir("sanidade", &config, 3, 4);

    assert_eq!(agregado.temporadas, 3);
    assert_eq!(agregado.corridas_totais, 18);
    assert!((-1.0..=1.0).contains(&agregado.spearman_grid_chegada));
    assert!((-1.0..=1.0).contains(&agregado.spearman_etapas_consecutivas));
    assert!((0.0..=1.0).contains(&agregado.pct_vitorias_do_pole));
    assert!((0.0..=1.0).contains(&agregado.p_melhor_fora_top5));
    assert!(agregado.vencedores_distintos >= 1.0);
    assert!(agregado.desvio_posicao >= 0.0);
}

#[test]
fn decisao_do_campeonato_fica_entre_zero_e_um() {
    let config = ConfigTemporada {
        etapas: 10,
        pilotos: 18,
        ..ConfigTemporada::gt3()
    };
    let agregado = arena::medir("decisao", &config, 2, 12);
    let f = agregado.fracao_decisao_campeonato;
    assert!(f > 0.0 && f <= 1.0, "fração fora do domínio: {f}");
}

#[test]
fn incidentes_ligados_produzem_abandono() {
    // Guarda do harness, não da simulação: confirma que o catálogo real carrega e que ligar
    // incidentes de fato muda o desfecho. Sem isto, uma medição "com incidentes" poderia estar
    // silenciosamente rodando com catálogo vazio.
    let config = ConfigTemporada {
        etapas: 8,
        pilotos: 20,
        ..ConfigTemporada::rookie()
    }
    .com_incidentes(true);

    let agregado = arena::medir("incidentes", &config, 3, 31);
    assert!(
        agregado.dnfs_por_etapa > 0.0,
        "com incidentes ligados deveria haver abandono; catálogo carregou vazio?"
    );
}

/// Guarda de NÃO-REGRESSÃO, calibrada folgadamente sobre o baseline medido hoje. Não afirma que
/// a simulação é boa — afirma que ela não pode ficar PIOR do que o estado ruim conhecido
/// enquanto o conserto acontece. Um valor de 1.0 aqui significaria determinismo puro.
#[test]
fn nao_regride_para_determinismo_absoluto() {
    let config = ConfigTemporada {
        etapas: 8,
        pilotos: 20,
        ..ConfigTemporada::rookie()
    };
    let agregado = arena::medir("nao-regressao", &config, 5, 77);

    assert!(
        agregado.spearman_etapas_consecutivas < 0.999,
        "etapas consecutivas viraram cópia carbono (rho={:.4})",
        agregado.spearman_etapas_consecutivas
    );
    assert!(
        agregado.desvio_posicao > 0.0,
        "ninguém mudou de posição em nenhuma etapa da campanha"
    );
}

// ---------------------------------------------------------------------------
// Camada leve — decomposição de variância
// ---------------------------------------------------------------------------

#[test]
fn nivelar_carros_iguala_todo_mundo() {
    let mut grid = gerar_campo(&PerfilCampo::gt3(), 20, 44);
    let antes: Vec<f64> = grid.iter().map(|d| d.car_performance).collect();
    assert!(
        antes.windows(2).any(|w| (w[0] - w[1]).abs() > 0.1),
        "o grid de gt3 deveria ter carros diferentes de saída"
    );

    nivelar_carros(&mut grid);
    let depois: Vec<f64> = grid.iter().map(|d| d.car_performance).collect();
    assert!(depois.windows(2).all(|w| (w[0] - w[1]).abs() < 1e-9));
    // Nivelar pela média não pode mexer na média.
    let m_antes = antes.iter().sum::<f64>() / antes.len() as f64;
    assert!((depois[0] - m_antes).abs() < 1e-9);
}

#[test]
fn nivelar_pilotos_iguala_atributos_e_preserva_carro() {
    let mut grid = gerar_campo(&PerfilCampo::gt3(), 20, 45);
    let carros: Vec<f64> = grid.iter().map(|d| d.car_performance).collect();
    nivelar_pilotos(&mut grid);

    assert!(grid.windows(2).all(|w| w[0].skill == w[1].skill));
    assert!(grid
        .windows(2)
        .all(|w| w[0].consistencia == w[1].consistencia));
    assert!(grid
        .iter()
        .zip(carros.iter())
        .all(|(d, c)| (d.car_performance - c).abs() < 1e-9));
}

#[test]
fn decomposicao_fecha_em_cem_por_cento() {
    let config = ConfigDecomposicao {
        base: ConfigTemporada {
            pilotos: 12,
            ..ConfigTemporada::rookie()
        },
        eventos: 4,
        replicas: 3,
        grids: 2,
    };
    let o = variancia::decompor_variancia("sanidade", &config, 5);

    let soma = o.frac_piloto + o.frac_carro + o.frac_evento + o.frac_corrida;
    assert!(
        (soma - 1.0).abs() < 0.02,
        "o orçamento tem que fechar em 100% (soma={soma:.4})"
    );
    assert!(o.frac_evento_pista + o.frac_evento_clima <= o.frac_evento + 1e-9);
    for f in [o.frac_piloto, o.frac_carro, o.frac_evento, o.frac_corrida] {
        assert!((0.0..=1.0).contains(&f), "fração fora do domínio: {f}");
    }
}

#[test]
fn variancia_total_bate_com_a_teorica_de_postos_uniformes() {
    // Aferição do método: sem DNF, a posição de chegada é uma permutação de 1..N, então a
    // variância total tem que cair em cima de (N²−1)/12. Se não cair, a matriz está montada
    // errada e todo o orçamento é lixo.
    let config = ConfigDecomposicao {
        base: ConfigTemporada {
            pilotos: 16,
            ..ConfigTemporada::rookie()
        },
        eventos: 4,
        replicas: 4,
        grids: 2,
    };
    let o = variancia::decompor_variancia("aferição", &config, 6);
    let erro = (o.var_total - o.var_total_teorica).abs() / o.var_total_teorica;
    assert!(
        erro < 0.15,
        "var total={:.2}, teórica={:.2} (erro {:.1}%)",
        o.var_total,
        o.var_total_teorica,
        erro * 100.0
    );
}

#[test]
fn categoria_spec_nao_tem_variancia_de_carro() {
    // Segunda aferição do método: na rookie o carro é spec por construção
    // (`math::category_car_performance`), então nivelar carros não pode mudar nada e a fatia de
    // carro TEM que dar ~0. Se der diferente de zero, o congelamento seletivo está vazando.
    //
    // **A esteira fica DESLIGADA aqui, e isso é a asserção, não uma conveniência.** Este teste
    // afere o MÉTODO (o congelamento seletivo isola o canal do carro), não o orçamento do jogo. Com
    // a esteira ligada a rookie passa a ter 2,4% de fatia de equipe — legítimos, porque
    // `ACERTO_FRACAO_EQUIPE = 0,70` faz o acerto de fim de semana ser majoritariamente da EQUIPE, e
    // equipe existe mesmo onde o carro é igual para todos. Seria um teste medindo duas coisas ao
    // mesmo tempo e falhando por causa da que ele não pretendia medir.
    let config = ConfigDecomposicao {
        base: ConfigTemporada {
            pilotos: 14,
            esteira_de_forma: false,
            ..ConfigTemporada::rookie()
        },
        eventos: 4,
        replicas: 4,
        grids: 3,
    };
    let o = variancia::decompor_variancia("mazda_rookie", &config, 7);
    assert!(
        o.frac_carro < 0.03,
        "categoria spec não pode ter fatia de carro: {:.3}",
        o.frac_carro
    );
}

// ---------------------------------------------------------------------------
// Camada leve — processo
// ---------------------------------------------------------------------------

#[test]
fn processo_conta_trocas_e_normaliza() {
    let config = ConfigTemporada {
        etapas: 3,
        pilotos: 12,
        ..ConfigTemporada::rookie()
    };
    let p = processo::medir_campanha_processo("sanidade", &config, 2, 13);

    assert_eq!(p.corridas, 6);
    assert!(p.trocas >= 0.0);
    assert!(
        (0.0..=1.0).contains(&p.trocas_normalizadas),
        "trocas normalizadas fora do domínio: {}",
        p.trocas_normalizadas
    );
    assert!(p.pelotoes >= 1.0);
    assert!(
        p.ganho_p90 >= p.ganho_medio_abs - 1e-9,
        "p90 abaixo da média"
    );
}

#[test]
fn grid_sorteado_realmente_descorrelaciona_a_largada() {
    // Guarda do harness: o experimento do poder da largada só vale se o embaralhamento de fato
    // separar o grid do ritmo. Aqui isso é verificado direto — com grid sorteado, a correlação
    // entre skill e posição de LARGADA tem que ir a ~0.
    let config = ConfigTemporada {
        etapas: 1,
        pilotos: 20,
        ..ConfigTemporada::rookie()
    };
    let catalogo = arena::catalogo_para(&config);
    let mut correlacoes = Vec::new();

    for i in 0..30 {
        let semente = arena::semente_da_temporada(101, i);
        let grid = gerar_campo(&config.perfil, config.pilotos, semente);
        let eventos = arena::sortear_eventos(&config, semente);
        let r = arena::rodar_evento_com_grid_imposto(
            &config,
            &grid,
            &eventos[0],
            1,
            &catalogo,
            semente,
            true,
        );
        let mut xs = Vec::new();
        let mut ys = Vec::new();
        for linha in &r.race_results {
            if let Some(d) = grid.iter().find(|d| d.id == linha.pilot_id) {
                xs.push(-(d.skill as f64));
                ys.push(linha.grid_position as f64);
            }
        }
        if let Some(rho) = spearman(&xs, &ys) {
            correlacoes.push(rho);
        }
    }

    let media = correlacoes.iter().sum::<f64>() / correlacoes.len() as f64;
    assert!(
        media.abs() < 0.20,
        "o grid sorteado ainda carrega ritmo (rho={media:.3})"
    );
}

// ---------------------------------------------------------------------------
// Camada leve — varredura
// ---------------------------------------------------------------------------

#[test]
fn ajuste_de_knob_chega_ao_contexto() {
    // Guarda do harness: se a sobrescrita não chegar ao `SimulationContext`, a varredura inteira
    // reportaria "MORTO" para todo mundo — o falso negativo mais caro possível deste pacote.
    let config = ConfigTemporada::rookie().com_ajustes(AjustesCtx {
        race_variance_multiplier: Some(42.0),
        overtaking_difficulty_multiplier: Some(7.5),
        ..AjustesCtx::default()
    });
    let evento = arena::sortear_eventos(&config, 3)[0];
    let ctx = arena::contexto_do_evento(&config, 1, &evento);

    assert_eq!(ctx.race_variance_multiplier, 42.0);
    assert_eq!(ctx.overtaking_difficulty_multiplier, 7.5);
    // O que não foi sobrescrito continua vindo do perfil da categoria.
    assert!(
        ctx.start_chaos_multiplier > 1.0,
        "rookie tem caos de largada alto"
    );
}

#[test]
fn varredura_devolve_um_ponto_por_valor() {
    let config = ConfigTemporada {
        etapas: 4,
        pilotos: 12,
        ..ConfigTemporada::rookie()
    };
    let valores = [0.5, 1.0, 2.0];
    let v = varredura::varrer("rookie", &config, Knob::RaceVariance, &valores, 2, 9);

    assert_eq!(v.pontos.len(), 3);
    assert_eq!(v.knob, Knob::RaceVariance);
    for (p, esperado) in v.pontos.iter().zip(valores.iter()) {
        assert_eq!(p.valor, *esperado);
        assert!(p.metricas.spearman_etapas_consecutivas.is_finite());
    }
}

/// **A guarda do morto por MAGNITUDE** — o caso que a varredura pega e a guarda de fonte não.
///
/// `track_difficulty_multiplier` é lido por `race/pontuacao.rs`, então `consumo.rs` não tem o que
/// reclamar. Mas o efeito é `adaptabilidade/100 × (mult−1) × 0,05`: décimos de ponto num score de
/// 60–70. Varrê-lo de 0 a 10 não move métrica nenhuma.
///
/// Este teste substituiu o antigo `knob_nao_lido_pela_simulacao_e_morto_por_construcao`, que
/// asseverava a morte por INEXISTÊNCIA de `overtaking_difficulty_multiplier`. Aquele knob foi
/// conectado pelo pacote D e o teste cumpriu o papel: falhou no momento da ligação.
#[test]
fn knob_morto_por_magnitude_continua_sem_alavanca() {
    // 12 temporadas, e não 3. A calibração das camadas de evento aumentou muito a variância da
    // árvore, e com ela o ruído de amostragem desta medida: com 3 réplicas a alavanca medida
    // batia 0,0222 contra um limite de 0,02 — ou seja, a guarda passou a disparar dentro do
    // próprio ruído, o que a torna pior que inútil (um vermelho que não significa nada treina
    // todo mundo a ignorá-la). A varredura completa, com 1008 corridas, mede 0,004 no mesmo
    // knob; o limite de 0,02 é o correto e quem estava errado era o tamanho da amostra.
    let config = ConfigTemporada {
        etapas: 6,
        pilotos: 16,
        ..ConfigTemporada::rookie()
    };
    let v = varredura::varrer(
        "rookie",
        &config,
        Knob::TrackDifficulty,
        &[0.0, 1.0, 10.0],
        12,
        23,
    );

    assert!(
        v.alavanca_consecutivas() < 0.02,
        "track_difficulty ganhou alavanca ({:.4}) — alguém aumentou a magnitude do efeito, e a \
         campanha de calibração precisa passar a tratá-lo como knob de verdade",
        v.alavanca_consecutivas()
    );
    assert!(consumo::MORTOS_POR_MAGNITUDE.contains(&"track_difficulty_multiplier"));
}

/// **A guarda inversa**, que passa a valer agora que os dois órfãos foram conectados: se a ligação
/// se desfizer numa refatoração, a alavanca volta a 0,000 exato e ninguém percebe olhando o fonte
/// (o campo continuaria lá, o perfil continuaria calculando).
///
/// `consumo.rs` pega o desaparecimento do ACESSO; esta pega o desaparecimento do EFEITO.
#[test]
fn knobs_ressuscitados_tem_efeito_mensuravel() {
    let base = ConfigTemporada {
        etapas: 8,
        pilotos: 20,
        ..ConfigTemporada::rookie()
    };

    // Ultrapassagem: conectada pelo pacote D em `race/trafego.rs`.
    let ultrapassagem = varredura::varrer(
        "rookie",
        &base,
        Knob::OvertakingDifficulty,
        &[0.0, 1.0, 10.0],
        6,
        23,
    );
    assert!(
        ultrapassagem.alavanca_consecutivas() > 0.0,
        "overtaking_difficulty voltou a não ter efeito — a ligação do pacote D se desfez"
    );

    // Chuva: conectada pelo pacote G. Só faz sentido com pista molhada, então a temporada é
    // toda na chuva — senão o knob não está em jogo e o teste mediria zero por construção.
    let na_chuva = ConfigTemporada {
        fracao_chuva: 1.0,
        ..base
    };
    let chuva = varredura::varrer(
        "rookie",
        &na_chuva,
        Knob::RainSensitivity,
        &[0.0, 1.0, 10.0],
        6,
        23,
    );
    assert!(
        chuva.alavanca_consecutivas() > 0.0,
        "rain_sensitivity voltou a não ter efeito mesmo com chuva em todas as etapas — a ligação \
         do pacote G se desfez"
    );
}

#[test]
fn baseline_congelado_esta_completo_e_coerente() {
    assert_eq!(snapshot::CONGELADO.len(), 4);
    for linha in snapshot::CONGELADO {
        assert!(snapshot::buscar(linha.rotulo).is_some());
        assert!((0.0..=1.0).contains(&linha.pct_vitorias_do_pole));
        assert!((-1.0..=1.0).contains(&linha.spearman_etapas_consecutivas));
        assert!(linha.vencedores_distintos >= 1.0);
        assert!(linha.desvio_posicao >= 0.0);
    }
}

// ---------------------------------------------------------------------------
// Camada leve — atrito de posição (as métricas do pacote D)
// ---------------------------------------------------------------------------

/// Monta uma matriz de posições por segmento a partir de colunas escritas à mão.
/// `colunas[s][i]` = posição do piloto `i` ao fim do segmento `s`.
fn posicoes_sinteticas(ids: &[&str], colunas: [&[i32]; 5]) -> Vec<(String, Vec<i32>)> {
    ids.iter()
        .enumerate()
        .map(|(i, id)| {
            (
                (*id).to_string(),
                (0..5).map(|s| colunas[s][i]).collect::<Vec<i32>>(),
            )
        })
        .collect()
}

/// Grid sintético com skills conhecidos e decrescentes: A é o mais rápido, E o mais lento.
fn grid_de_teste() -> Vec<crate::simulation::context::SimDriver> {
    let mut grid = gerar_campo(&PerfilCampo::rookie(), 5, 3);
    for (i, d) in grid.iter_mut().enumerate() {
        d.id = ["A", "B", "C", "D", "E"][i].to_string();
        d.skill = 90 - (i as u8) * 10; // 90, 80, 70, 60, 50
    }
    grid
}

#[test]
fn corrida_decidida_na_largada_estabiliza_no_segmento_1() {
    // Ordem final formada já no Start e nunca mais mexida — o cenário que o pacote D existe
    // para eliminar. A métrica tem que gritar: ρ = 1,0 desde o começo, estabilização em 1.
    let ordem: &[i32] = &[1, 2, 3, 4, 5];
    let posicoes = posicoes_sinteticas(&["A", "B", "C", "D", "E"], [ordem; 5]);
    let m = atrito::medir_segmentos(&grid_de_teste(), &posicoes, &[5, 4, 3, 2, 1]);

    assert!((m.rho_por_segmento[0] - 1.0).abs() < 1e-9);
    assert_eq!(m.segmento_de_estabilizacao, 1.0);
    // Grid invertido virando ordem final no Start = todas as inversões possíveis num só segmento.
    assert_eq!(m.trocas_por_segmento[0], 10.0);
    for s in 1..5 {
        assert_eq!(
            m.trocas_por_segmento[s], 0.0,
            "segmento {s} deveria ser inerte"
        );
    }
}

#[test]
fn corrida_que_evolui_estabiliza_tarde() {
    // O rápido (A) larga em último e vai subindo um lugar por segmento.
    let posicoes = posicoes_sinteticas(
        &["A", "B", "C", "D", "E"],
        [
            &[5, 1, 2, 3, 4],
            &[4, 1, 2, 3, 5],
            &[3, 1, 2, 4, 5],
            &[2, 1, 3, 4, 5],
            &[1, 2, 3, 4, 5],
        ],
    );
    let m = atrito::medir_segmentos(&grid_de_teste(), &posicoes, &[5, 1, 2, 3, 4]);

    assert!(
        m.rho_por_segmento[0] < m.rho_por_segmento[4],
        "ρ tem que crescer ao longo da corrida: {:?}",
        m.rho_por_segmento
    );
    assert!(
        m.segmento_de_estabilizacao >= 4.0,
        "ordem se fechou cedo demais: {}",
        m.segmento_de_estabilizacao
    );
    // Trocas distribuídas, não concentradas na largada.
    assert!(m.trocas_por_segmento[1..].iter().all(|t| *t > 0.0));
}

#[test]
fn trem_de_carros_e_detectado_e_medido() {
    // A (skill 90) fica preso atrás de E (skill 50) os cinco segmentos: comboio puro.
    let ordem: &[i32] = &[2, 3, 4, 5, 1];
    let posicoes = posicoes_sinteticas(&["A", "B", "C", "D", "E"], [ordem; 5]);
    let m = atrito::medir_segmentos(&grid_de_teste(), &posicoes, ordem);

    assert_eq!(
        m.maior_sequencia_travado, 5.0,
        "A ficou 5 segmentos atrás de E e isso tem que aparecer"
    );
    // Só A está atrás de alguém estruturalmente mais lento — B, C e D estão atrás de gente mais
    // rápida, o que é ordem normal. 1 piloto em 5, nos 5 segmentos = 0,2 das células.
    assert!(
        (m.fracao_travado - 0.2).abs() < 1e-9,
        "fração travada: {}",
        m.fracao_travado
    );
}

#[test]
fn sem_transito_a_fracao_travado_e_zero() {
    // Ordem perfeitamente alinhada ao ritmo: ninguém está atrás de alguém mais lento.
    let ordem: &[i32] = &[1, 2, 3, 4, 5];
    let posicoes = posicoes_sinteticas(&["A", "B", "C", "D", "E"], [ordem; 5]);
    let m = atrito::medir_segmentos(&grid_de_teste(), &posicoes, ordem);

    assert_eq!(m.fracao_travado, 0.0);
    assert_eq!(m.maior_sequencia_travado, 0.0);
}

#[test]
fn sequencia_de_trem_quebra_ao_trocar_de_carro_da_frente() {
    // A fica atrás de E nos dois primeiros segmentos, depois atrás de D nos três seguintes.
    // A maior sequência é 3, não 5: são dois comboios distintos.
    let posicoes = posicoes_sinteticas(
        &["A", "B", "C", "D", "E"],
        [
            &[2, 3, 4, 5, 1],
            &[2, 3, 4, 5, 1],
            &[2, 3, 4, 1, 5],
            &[2, 3, 4, 1, 5],
            &[2, 3, 4, 1, 5],
        ],
    );
    let m = atrito::medir_segmentos(&grid_de_teste(), &posicoes, &[2, 3, 4, 5, 1]);
    assert_eq!(m.maior_sequencia_travado, 3.0);
}

#[test]
fn metricas_de_atrito_saem_de_corrida_real() {
    let config = ConfigTemporada {
        etapas: 2,
        pilotos: 16,
        ..ConfigTemporada::rookie()
    };
    let grid = gerar_campo(&config.perfil, config.pilotos, 55);
    let corridas = arena::rodar_temporada(
        &config,
        &grid,
        &crate::simulation::catalog::IncidentCatalog::empty(),
        55,
        1,
    );
    let m = atrito::medir_atrito(&corridas[0]);

    assert!(m.recuperacao_maxima >= 0.0);
    assert!((0.0..=1.0).contains(&m.fracao_em_janela));
    assert!(m.cv_gaps.is_finite() && m.cv_gaps >= 0.0);
    assert!(m.trens >= 1.0);
}

#[test]
fn adaptador_de_posicoes_por_segmento_ainda_esta_pendente() {
    // Guarda de ligação: quando `RaceDriverResult::posicoes_por_segmento` entrar e o adaptador
    // for conectado, ESTE teste falha — e isso é o lembrete de trocar as métricas de segmento
    // de entrada sintética para corrida de verdade, e de refazer os alvos do D contra elas.
    let config = ConfigTemporada {
        etapas: 1,
        pilotos: 12,
        ..ConfigTemporada::rookie()
    };
    let grid = gerar_campo(&config.perfil, config.pilotos, 61);
    let corridas = arena::rodar_temporada(
        &config,
        &grid,
        &crate::simulation::catalog::IncidentCatalog::empty(),
        61,
        1,
    );

    assert!(
        atrito::posicoes_por_segmento(&corridas[0]).is_none(),
        "o campo chegou! conecte o adaptador em atrito.rs e ligue as métricas de segmento \
         ao relatório do pacote D"
    );
}

#[test]
fn alvos_do_d_sao_faixas_coerentes() {
    for alvos in [atrito::AlvosAtrito::entrada(), atrito::AlvosAtrito::topo()] {
        for (nome, f) in [
            ("rho_grid_sorteado", alvos.rho_grid_sorteado),
            ("rho_skill", alvos.rho_skill_com_grid_sorteado),
            ("rho_start_vs_final", alvos.rho_start_vs_final),
            ("fracao_travado", alvos.fracao_travado),
            ("fracao_em_janela", alvos.fracao_em_janela),
        ] {
            assert!(f.min < f.max, "{nome}: faixa invertida");
            assert!(
                (0.0..=1.0).contains(&f.min) && (0.0..=1.0).contains(&f.max),
                "{nome}: fora do domínio de correlação/fração"
            );
        }
        // Grid e ritmo não podem explicar 100% do resultado — tem que sobrar espaço para o
        // que C e D vão introduzir, senão o alvo é outra forma de determinismo.
        let teto =
            alvos.rho_grid_sorteado.max.powi(2) + alvos.rho_skill_com_grid_sorteado.max.powi(2);
        assert!(teto < 1.05, "grid + ritmo saturam o resultado: {teto:.2}");
    }
}

#[test]
fn topo_tem_mais_transito_que_entrada() {
    // A hipótese do desenho: onde as duas categorias DEVEM se separar é no trânsito, não na
    // correlação final. Ar sujo cria comboio; o ρ final se cancela contra corrida mais longa.
    let entrada = atrito::AlvosAtrito::entrada();
    let topo = atrito::AlvosAtrito::topo();

    assert!(topo.fracao_travado.min > entrada.fracao_travado.min);
    assert!(topo.maior_sequencia_travado.min > entrada.maior_sequencia_travado.min);
    assert!(topo.rho_grid_sorteado.min > entrada.rho_grid_sorteado.min);
    assert!(
        topo.rho_skill_com_grid_sorteado.max < entrada.rho_skill_com_grid_sorteado.max,
        "no topo o ritmo deve se impor menos que na entrada"
    );
}

#[test]
fn recuperacao_alvo_esta_acima_do_medido_hoje() {
    // A armadilha número um do pacote D: atrito e recuperação sobem JUNTOS. Os alvos têm que
    // pedir mais recuperação que os 4,1 (rookie) e 2,0 (gt3) de hoje, não menos.
    assert!(atrito::AlvosAtrito::entrada().recuperacao_maxima.min > 4.11);
    assert!(atrito::AlvosAtrito::topo().recuperacao_maxima.min > 1.99);
}

// ---------------------------------------------------------------------------
// Camada leve — a máquina de busca
// ---------------------------------------------------------------------------

#[test]
fn distancia_e_zero_dentro_da_faixa_e_cresce_fora() {
    let m = busca::Metrica {
        nome: "teste",
        extrair: |_| 0.0,
        faixa: Faixa::nova(0.2, 0.5),
        escala: busca::Escala::Linear,
    };
    assert_eq!(m.distancia(0.35), 0.0);
    assert_eq!(m.distancia(0.2), 0.0);
    assert_eq!(m.distancia(0.5), 0.0);
    // Fora, cresce linearmente e é normalizada pela largura da faixa (0,3).
    assert!((m.distancia(0.8) - 1.0).abs() < 1e-9);
    assert!((m.distancia(-0.1) - 1.0).abs() < 1e-9);
    assert!(m.distancia(1.1) > m.distancia(0.8), "sem gradiente fora");
    assert!(m.distancia(f64::NAN).is_infinite());
}

#[test]
fn escala_de_correlacao_restaura_gradiente_na_saturacao() {
    // O buraco que motivou mudar a função-objetivo. Perto de ρ = 1 a escala crua achata: um passo
    // grande de parâmetro produz um passo minúsculo de objetivo. A escala atanh estica a região
    // saturada e devolve gradiente utilizável.
    let faixa = Faixa::nova(0.20, 0.55);
    let crua = busca::Metrica {
        nome: "crua",
        extrair: |_| 0.0,
        faixa,
        escala: busca::Escala::Linear,
    };
    let fisher = busca::Metrica {
        nome: "fisher",
        extrair: |_| 0.0,
        faixa,
        escala: busca::Escala::Correlacao,
    };

    // O passo que a varredura de fato consegue produzir na região saturada: 0,976 → 0,850.
    let passo_cru = crua.distancia(0.976) - crua.distancia(0.850);
    let passo_fisher = fisher.distancia(0.976) - fisher.distancia(0.850);

    assert!(
        passo_fisher > passo_cru * 1.5,
        "atanh tem que ampliar o passo na saturação (cru={passo_cru:.3}, fisher={passo_fisher:.3})"
    );
    // E não pode inverter o sinal: mais perto do alvo continua sendo distância menor.
    assert!(fisher.distancia(0.850) < fisher.distancia(0.976));
    assert!(
        fisher.distancia(0.55) == 0.0,
        "a borda da faixa continua sendo a borda"
    );
}

#[test]
fn escala_de_fracao_estica_perto_dos_extremos() {
    let m = busca::Metrica {
        nome: "f",
        extrair: |_| 0.0,
        faixa: Faixa::nova(0.15, 0.35),
        escala: busca::Escala::Fracao,
    };
    assert_eq!(m.distancia(0.25), 0.0);
    // 0,99 está muito mais longe que 0,60, e a logit tem que refletir isso com folga.
    assert!(m.distancia(0.99) > m.distancia(0.60) * 2.0);
    assert!(
        m.distancia(0.0).is_finite(),
        "clamp evita infinito no extremo"
    );
}

#[test]
fn niveis_de_triagem_tem_o_volume_do_plano() {
    // T1 = 15 × 10 = 150. A forma foi escolhida por ARREPENDIMENTO medido, não por volume: com
    // 12 × 6 (72 corridas) a peneira custava 0,050 da amplitude do eixo; com 15 × 10 custa 0,000.
    // Margem de segurança, não conserto — 0,05 já era aceitável, mas o erro compõe ao longo de
    // duas passadas da descida coordenada, e T1 continua 2,4× mais barato que T2.
    assert_eq!(busca::Nivel::T1.corridas(), 150);
    assert_eq!(busca::Nivel::T2.corridas(), 360);
    assert_eq!(busca::Nivel::T3.corridas(), 1008);
}

#[test]
fn orcamento_de_avaliacoes_e_respeitado() {
    let mut avaliador = busca::Avaliador::novo(
        ConfigTemporada {
            etapas: 4,
            pilotos: 10,
            ..ConfigTemporada::rookie()
        },
        Alvos::entrada(),
        3,
        3,
    );
    let ponto: busca::Ponto = Default::default();
    assert!(avaliador.avaliar(&ponto, busca::Nivel::T1).is_some());
    assert!(avaliador.avaliar(&ponto, busca::Nivel::T1).is_some());
    assert!(avaliador.avaliar(&ponto, busca::Nivel::T1).is_some());
    assert!(
        avaliador.avaliar(&ponto, busca::Nivel::T1).is_none(),
        "estourou o teto de avaliações"
    );
    assert_eq!(avaliador.gastas, 3);
}

#[test]
fn ajuste_do_ponto_chega_ao_contexto() {
    // Se o ponto não chegar ao `SimulationContext`, a busca inteira mede o mesmo ponto N vezes e
    // reporta "sem alavanca" por bug em vez de por achado — o falso negativo mais caro possível.
    let mut ponto: busca::Ponto = Default::default();
    ponto.insert("race_variance_multiplier", 7.0);
    ponto.insert("pack_density_factor", 3.0);

    let config = ConfigTemporada::rookie().com_ajustes(busca::ajustes_de_ponto(&ponto));
    let evento = arena::sortear_eventos(&config, 3)[0];
    let ctx = arena::contexto_do_evento(&config, 1, &evento);

    assert_eq!(ctx.race_variance_multiplier, 7.0);
    assert_eq!(ctx.pack_density_factor, 3.0);
}

/// **O critério de aceitação invertido.** A busca roda sobre o espaço de parâmetros de HOJE, que
/// a varredura já provou não ter alavanca. Ela TEM que falhar, e falhar bem: reportar
/// inalcançável para as métricas centrais e não devolver vencedor.
///
/// Se este teste um dia passar a ver `fracassou == false`, ou o espaço ganhou mecanismo (ótimo —
/// e aí é o D funcionando) ou a máquina de busca está mentindo. Nos dois casos, ler antes de
/// mexer.
#[test]
fn busca_no_espaco_morto_falha_e_falha_bem() {
    let base = ConfigTemporada {
        pilotos: 20,
        ..ConfigTemporada::rookie()
    };
    let r = busca::buscar(
        "espaço morto",
        base,
        Alvos::entrada(),
        &busca::espaco_atual(),
        60,
        2026,
    );

    // (0) Falha explícita, não "melhor ponto encontrado".
    //
    // NOTA: o motivo da falha aqui NÃO é o que se supunha ao escrever este teste. A varredura
    // por eixo dizia que nenhum knob chega ao alvo, e isso é verdade POR EIXO — mas a combinação
    // `race_variance = 10` × `pack_density = 10` atinge as oito métricas de resultado. O que a
    // reprova é o portão do orçamento de variância: nesse ponto a distribuição está certa e o
    // motivo, errado. Ruído dimensionado para imitar disputa não é calibração.
    assert!(
        r.fracassou,
        "a busca não pode reportar sucesso sobre o espaço atual"
    );

    // (1) A reprovação tem que vir do orçamento, não de um empate qualquer.
    assert!(
        !r.falhas_de_orcamento.is_empty(),
        "o portão do orçamento tem que reprovar o ponto encontrado: {:?}",
        r.falhas_de_orcamento
    );

    // (2) Veredito por métrica, para todas.
    assert_eq!(r.vereditos.len(), 8);

    // (3) O diagnóstico tem que NOMEAR o problema, não só marcar.
    assert!(
        r.diagnostico.iter().any(|d| {
            d.contains("falta mecanismo") || d.contains("desacople") || d.contains("MOTIVO ERRADO")
        }),
        "o diagnóstico tem que dizer o que está errado: {:?}",
        r.diagnostico
    );

    // (3b) O ótimo na borda tem que ser sinalizado — é o segundo sinal de que o ponto é suspeito.
    assert!(
        !r.otimos_na_borda.is_empty(),
        "o ótimo saiu na borda da faixa varrida e isso tem que aparecer"
    );

    // (4) O melhor de cada métrica em qualquer ponto tem que estar preenchido — é o que responde
    //     'é alcançável de todo?'.
    assert_eq!(r.melhor_por_metrica.len(), 8);
    assert!(r.melhor_por_metrica.iter().all(|(_, d, _)| d.is_finite()));

    // (5) O orçamento foi respeitado.
    assert!(r.avaliacoes <= 60, "estourou o orçamento: {}", r.avaliacoes);
}

#[test]
fn busca_com_alvo_trivial_passa_nas_metricas_mas_reprova_no_orcamento() {
    // O contraste necessário: se a busca falhasse SEMPRE, o teste acima não provaria nada. Com
    // faixas absurdamente largas, todas as métricas estão dentro e o veredito é sucesso.
    let largo = Alvos {
        spearman_grid_chegada: Faixa::nova(-1.0, 1.0),
        pct_vitorias_do_pole: Faixa::nova(0.0, 1.0),
        vencedores_distintos: Faixa::nova(0.0, 100.0),
        desvio_posicao: Faixa::nova(0.0, 100.0),
        p_melhor_fora_top5: Faixa::nova(0.0, 1.0),
        spearman_etapas_consecutivas: Faixa::nova(-1.0, 1.0),
        trocas_de_lideranca: Faixa::nova(0.0, 100.0),
        margem_do_campeao: Faixa::nova(0.0, 1.0),
    };
    let base = ConfigTemporada {
        pilotos: 14,
        ..ConfigTemporada::rookie()
    };
    let r = busca::buscar(
        "alvo trivial",
        base,
        largo,
        &busca::espaco_atual()[..2],
        20,
        7,
    );

    // O contraste: com faixa que cobre tudo, NENHUMA métrica de resultado reprova. Prova que a
    // máquina não está viciada em falhar.
    //
    // E, ainda assim, a busca FRACASSA — pelo portão do orçamento de variância, que é
    // independente dos alvos de resultado. É exatamente o ponto: alargar a faixa até tudo passar
    // não compra sucesso, porque a distribuição continua certa pelo motivo errado.
    assert!(
        r.fracassou && !r.falhas_de_orcamento.is_empty(),
        "o portão do orçamento não depende dos alvos de resultado e tem que reprovar assim mesmo"
    );
    assert!(r
        .vereditos
        .iter()
        .all(|(_, v)| *v == busca::VereditoMetrica::Atingido));
}

#[test]
fn otimo_na_borda_e_sinalizado() {
    // Guarda do requisito 3. Com um eixo de um valor só, o ótimo está necessariamente na borda.
    let base = ConfigTemporada {
        pilotos: 12,
        ..ConfigTemporada::rookie()
    };
    let espaco = vec![busca::Eixo {
        knob: Knob::RaceVariance,
        valores: vec![10.0],
    }];
    let r = busca::buscar("borda", base, Alvos::entrada(), &espaco, 10, 11);

    assert!(r.otimos_na_borda.contains(&"race_variance_multiplier"));
    assert!(
        r.diagnostico.iter().any(|d| d.contains("BORDA")),
        "a borda tem que aparecer no diagnóstico: {:?}",
        r.diagnostico
    );
}

// ---------------------------------------------------------------------------
// Camada leve — safety car e o diagnóstico do gatilho
// ---------------------------------------------------------------------------

#[test]
fn metricas_de_sc_saem_do_race_result() {
    // Guarda de ligação: `safety_cars` e `ordem_pre_safety_car` existem no `RaceResult` desde o
    // pacote G, e o harness LÊ os dois. A primeira versão desta seção documentou os campos como
    // "pedidos ao G" e nunca voltou a ler o resultado — o bloqueio era imaginário.
    let config = ConfigTemporada {
        etapas: 8,
        pilotos: 20,
        ..ConfigTemporada::rookie()
    }
    .com_incidentes(true);

    let agregado = arena::medir("sc", &config, 12, 31);
    assert!(
        agregado.scs_por_etapa.is_finite() && agregado.scs_por_etapa >= 0.0,
        "SC/etapa tem que sair do RaceResult: {}",
        agregado.scs_por_etapa
    );
    assert!(
        agregado.scs_por_etapa > 0.0,
        "com incidentes ligados em 96 corridas deveria haver algum safety car"
    );
    // Onde houve SC, a correlação pré-SC × chegada tem que estar no domínio.
    assert!(
        agregado.rho_pre_sc_chegada.is_nan() || (-1.0..=1.0).contains(&agregado.rho_pre_sc_chegada),
        "rho pré-SC fora do domínio: {}",
        agregado.rho_pre_sc_chegada
    );
}

#[test]
fn gatilho_alargado_e_superconjunto_do_atual() {
    // O contrafactual só é válido se ele de fato contiver o predicado do jogo. Testado sobre todas
    // as combinações de tipo × severidade × DNF, para não depender de sorteio.
    use crate::simulation::incidents::{IncidentResult, IncidentSeverity, IncidentType};
    use crate::simulation::race::estrategia::traz_bandeira_amarela;

    let mut vistos_atual = 0;
    for tipo in [
        IncidentType::Collision,
        IncidentType::DriverError,
        IncidentType::Mechanical,
    ] {
        for sev in [
            IncidentSeverity::Minor,
            IncidentSeverity::Major,
            IncidentSeverity::Critical,
        ] {
            for dnf in [false, true] {
                let inc = IncidentResult {
                    pilot_id: "P".into(),
                    incident_type: tipo,
                    severity: sev,
                    segment: "MID".into(),
                    positions_lost: 0,
                    is_dnf: dnf,
                    description: String::new(),
                    linked_pilot_id: None,
                    is_two_car_incident: false,
                    injury_risk_multiplier: 1.0,
                    narrative_importance_hint: 0,
                    catalog_id: None,
                    damage_origin_segment: None,
                };
                let atual = traz_bandeira_amarela(&inc);
                let alargado = seguranca::traz_bandeira_amarela_alargado(&inc);
                if atual {
                    vistos_atual += 1;
                    assert!(
                        alargado,
                        "o alargado tem que conter o atual: {tipo:?}/{sev:?}/dnf={dnf}"
                    );
                }
            }
        }
    }
    assert!(
        vistos_atual > 0,
        "o predicado atual nunca disparou no varredor"
    );
}

/// **Tripwire, não asserção de qualidade.** Este teste fica VERDE enquanto o gatilho do jogo ainda
/// exige `is_dnf`, e fica VERMELHO no dia em que o alargamento entrar — que é o comportamento
/// desejado.
///
/// O motivo: `gatilho_alargado_e_superconjunto_do_atual` passa antes E depois do alargamento, então
/// ele não avisa nada. E alargar o gatilho move duas faixas de partida de uma vez — `SC/etapa` (de
/// 0,079 e 0,033 para ~0,184 e ~0,068, projetado) e `ρ(pré-SC × chegada)` —, o que invalida a linha
/// do safety car no [BASELINE.md](../BASELINE.md) sem invalidar nenhum teste. É exatamente a classe
/// de mudança que passa em silêncio.
///
/// Quando ele quebrar, na ordem: remedir `imprime_diagnostico_do_gatilho_de_sc`, remedir a coluna
/// `SC/etapa` da matriz de alavanca, atualizar a seção 11 do BASELINE, e **só então** apagar este
/// teste. Recongelar o `snapshot::CONGELADO` é decisão separada e não é consequência desta.
#[test]
fn gatilho_do_jogo_ainda_exige_dnf() {
    use crate::simulation::incidents::{IncidentResult, IncidentSeverity, IncidentType};
    use crate::simulation::race::estrategia::traz_bandeira_amarela;

    let colisao_major_sem_dnf = IncidentResult {
        pilot_id: "P".into(),
        incident_type: IncidentType::Collision,
        severity: IncidentSeverity::Major,
        segment: "MID".into(),
        positions_lost: 0,
        is_dnf: false,
        description: String::new(),
        linked_pilot_id: None,
        is_two_car_incident: false,
        injury_risk_multiplier: 1.0,
        narrative_importance_hint: 0,
        catalog_id: None,
        damage_origin_segment: None,
    };

    assert!(
        seguranca::traz_bandeira_amarela_alargado(&colisao_major_sem_dnf),
        "o contrafactual perdeu o caso que ele existe para medir"
    );
    assert!(
        !traz_bandeira_amarela(&colisao_major_sem_dnf),
        "O GATILHO DE SC FOI ALARGADO. Isto não é um defeito — é o aviso de que a frequência de \
         safety car e o embaralhamento precisam ser remedidos antes de qualquer fase da campanha \
         que use SC/etapa como saída. Ver a doc deste teste para a ordem."
    );
}

/// **A medição que separa os três consertos possíveis** para a frequência de SC 6–10× abaixo do
/// alvo. Não afirma qual é — imprime o veredito, porque o conserto é de outro pacote.
#[test]
#[ignore = "diagnóstico do gatilho de safety car; roda com --nocapture"]
fn imprime_diagnostico_do_gatilho_de_sc() {
    println!("\n== DIAGNÓSTICO DO GATILHO DE SAFETY CAR ==\n");
    for (rotulo, base) in [
        ("mazda_rookie", ConfigTemporada::rookie()),
        ("gt3", ConfigTemporada::gt3()),
    ] {
        let config = ConfigTemporada {
            etapas: 12,
            pilotos: 20,
            ..base
        }
        .com_incidentes(true);
        let d = seguranca::diagnosticar_gatilho(rotulo, &config, 84, 2026);
        let alvo = seguranca::alvo_de_frequencia(&config.perfil.category_id);

        println!("### {} ({} corridas)\n", d.rotulo, d.corridas);
        println!("| {:<44} | {:>9} |", "medida", "valor");
        println!("|----------------------------------------------|-----------|");
        for (nome, v) in [
            ("Incidentes por corrida", d.incidentes_por_corrida),
            ("Incidentes GRAVES por corrida", d.graves_por_corrida),
            ("Qualificam pelo gatilho ATUAL", d.qualificam_atual),
            (
                "Qualificam pelo gatilho ALARGADO (sem is_dnf)",
                d.qualificam_alargado,
            ),
            ("Safety cars por corrida", d.scs_por_corrida),
            (
                "Aproveitamento do gatilho (atual/graves)",
                d.aproveitamento_do_gatilho,
            ),
            ("Ganho de alargar (alargado/atual)", d.ganho_de_alargar),
            ("Conversão (SC/qualificam)", d.conversao),
        ] {
            println!("| {nome:<44} | {v:>9.3} |");
        }
        println!(
            "\nAlvo de frequência: {:.2}–{:.2} SC/corrida — medido {:.3} ({}).\n",
            alvo.min,
            alvo.max,
            d.scs_por_corrida,
            alvo.veredito(d.scs_por_corrida)
        );
        println!(
            "Projeção alargando o gatilho: {:.3} SC/corrida (fator de gravidade ainda faltante: \
             {:.1}×)\n",
            d.sc_projetado_alargando(),
            d.fator_de_gravidade_faltante(alvo)
        );
        println!("VEREDITO: {}\n", d.veredito(alvo));
    }
}

// ---------------------------------------------------------------------------
// Camada leve — a repartição-alvo do orçamento de variância
// ---------------------------------------------------------------------------

#[test]
fn orcamento_alvo_fecha_em_cem_por_cento() {
    for alvo in [
        variancia::OrcamentoAlvo::entrada(),
        variancia::OrcamentoAlvo::topo(),
    ] {
        let min: f64 = alvo.piloto.min + alvo.carro.min + alvo.evento.min + alvo.corrida.min;
        let max: f64 = alvo.piloto.max + alvo.carro.max + alvo.evento.max + alvo.corrida.max;
        assert!(
            min <= 1.0 && max >= 1.0,
            "o intervalo do orçamento tem que CONTER 100%: [{min:.2}, {max:.2}]"
        );
    }
}

#[test]
fn orcamento_alvo_pede_muito_menos_permanente_que_hoje() {
    // A âncora empírica: em séries reais a correlação entre chegadas consecutivas fica em
    // 0,40–0,60, e essa correlação É a fração permanente. Hoje o Loop mede 0,97.
    for alvo in [
        variancia::OrcamentoAlvo::entrada(),
        variancia::OrcamentoAlvo::topo(),
    ] {
        let (min, max) = alvo.permanente();
        assert!(
            (0.35..=0.75).contains(&min) && (0.35..=0.75).contains(&max),
            "permanente fora da âncora empírica: [{min:.2}, {max:.2}]"
        );
    }
}

#[test]
fn topo_concentra_mais_permanente_e_erra_menos_que_a_entrada() {
    let entrada = variancia::OrcamentoAlvo::entrada();
    let topo = variancia::OrcamentoAlvo::topo();

    assert!(
        topo.carro.max > entrada.carro.max,
        "o carro tem que pesar mais no topo — é design declarado (dinastias)"
    );
    assert!(
        topo.corrida.max < entrada.corrida.max,
        "pelotão profissional erra menos"
    );
    assert!(
        topo.teto_de_azar < entrada.teto_de_azar,
        "no topo o resultado deve ser mais merecido"
    );
}

#[test]
fn orcamento_de_hoje_reprova_contra_o_alvo() {
    // Aferição do próprio comparador: o baseline medido TEM que reprovar, e por muito.
    let config = ConfigDecomposicao {
        base: ConfigTemporada {
            pilotos: 12,
            ..ConfigTemporada::rookie()
        },
        eventos: 4,
        replicas: 3,
        grids: 2,
    };
    let medido = variancia::decompor_variancia("hoje", &config, 5);
    let falhas = variancia::OrcamentoAlvo::entrada().conferir(&medido);

    assert!(
        falhas.len() >= 2,
        "o orçamento de hoje deveria reprovar em várias fontes, e só reprovou em: {falhas:?}"
    );
    assert!(
        falhas.iter().any(|f| f.starts_with("piloto")),
        "a fatia de piloto (96%) é a que mais destoa e tem que aparecer: {falhas:?}"
    );
}

// ---------------------------------------------------------------------------
// Camada leve — âncora contra o iRacing real
// ---------------------------------------------------------------------------

/// Monta um `aiseasons/<Season>.json` no formato REAL do iRacing (0-based nas posições, como o
/// arquivo de verdade), com `corridas` eventos de `pilotos` carros cada.
///
/// `ordem(evento, i)` devolve a posição de chegada 1-based do piloto `i` naquele evento — é o
/// gancho que deixa cada teste desenhar o cenário que quer medir.
fn aiseason_json(
    corridas: usize,
    pilotos: usize,
    ordem: impl Fn(usize, usize) -> i32,
) -> serde_json::Value {
    let eventos: Vec<serde_json::Value> = (0..corridas)
        .map(|e| {
            let linhas: Vec<serde_json::Value> = (0..pilotos)
                .map(|i| {
                    // Nomes de campo do JSON REAL do iRacing: snake_case, posições 0-based.
                    serde_json::json!({
                        "position": ordem(e, i) - 1,
                        "finish_position_in_class": ordem(e, i) - 1,
                        "starting_position": i as i32,
                        "incidents": 0,
                        "reason_out": "Running",
                        "best_lap_time": 900_000.0,
                        "interval": 0,
                        "laps_lead": 0,
                        "laps_complete": 12,
                        "carNumber": format!("{}", i + 1),
                        "cust_id": 9_700 + i as i64,
                        "display_name": format!("Piloto {i:02}"),
                    })
                })
                .collect();
            serde_json::json!({
                "results": {
                    "trackId": 47,
                    "race_summary": { "laps_complete": 12 },
                    "session_results": [
                        { "simsession_type": 6, "results": linhas }
                    ]
                }
            })
        })
        .collect();

    serde_json::json!({ "events": eventos })
}

#[test]
fn ingestor_le_o_formato_real_do_aiseason() {
    // Guarda de ponta a ponta do caminho de ingestão: se o formato do JSON do iRacing mudar, ou
    // se `parse_event_result` mudar de contrato, é aqui que aparece — antes de alguém coletar
    // dado de verdade e descobrir que o ingestor não lê.
    let json = aiseason_json(6, 10, |_, i| i as i32 + 1);
    let corridas = ancora::ler_temporada(&json);

    assert_eq!(corridas.len(), 6, "seis eventos com resultado final");
    assert_eq!(corridas[0].linhas.len(), 10);
    assert_eq!(corridas[0].track_id, 47);
    // Posições saem 1-based deste lado, como as do harness.
    assert!(corridas[0].linhas.iter().any(|l| l.chegada == 1));
    assert!(corridas[0].linhas.iter().all(|l| !l.dnf));
}

#[test]
fn metricas_reais_detectam_ordem_congelada() {
    // O cenário que o projeto inteiro existe para diagnosticar, agora vindo pelo caminho do dado
    // real: todo mundo termina sempre no mesmo lugar.
    let json = aiseason_json(8, 12, |_, i| i as i32 + 1);
    let m = ancora::medir_reais(&ancora::ler_temporada(&json));

    assert_eq!(m.corridas, 8);
    assert_eq!(m.pilotos_distintos, 12);
    assert_eq!(m.desvio_posicao, 0.0, "ordem congelada = desvio zero");
    assert!((m.spearman_corridas_consecutivas - 1.0).abs() < 1e-9);
    assert_eq!(m.vencedores_distintos, 1);
    assert_eq!(m.pct_vitorias_do_pole, 1.0);
}

#[test]
fn metricas_reais_detectam_ordem_embaralhada() {
    // O contraste: a ordem gira uma posição a cada corrida. Desvio alto, correlação entre
    // corridas consecutivas ainda alta (giro é monotônico), mas vencedores distintos = 8.
    let json = aiseason_json(8, 12, |e, i| ((i + e) % 12) as i32 + 1);
    let m = ancora::medir_reais(&ancora::ler_temporada(&json));

    assert!(
        m.desvio_posicao > 3.0,
        "rodízio de posição tem que dar desvio alto: {}",
        m.desvio_posicao
    );
    assert_eq!(
        m.vencedores_distintos, 8,
        "um vencedor diferente por corrida"
    );
    assert!(m.recuperacao_maxima > 0.0);
}

#[test]
fn ingestor_ignora_evento_sem_resultado() {
    // Temporada em andamento: eventos ainda não corridos não podem virar corrida de zero linha.
    let mut json = aiseason_json(3, 8, |_, i| i as i32 + 1);
    json["events"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({ "results": { "trackId": 99 } }));

    assert_eq!(ancora::ler_temporada(&json).len(), 3);
}

#[test]
fn pasta_inexistente_nao_explode() {
    let vazio = ancora::ler_pasta(std::path::Path::new("nao/existe/mesmo"));
    assert!(vazio.is_empty());
    assert!(ancora::medir_pasta(std::path::Path::new("nao/existe/mesmo")).is_none());
}

// ---------------------------------------------------------------------------
// Camada leve — guarda de consumo de knob
// ---------------------------------------------------------------------------

/// **Este teste está VERMELHO de propósito, e o vermelho é o relatório.**
///
/// Ele acusa `EscalasDeForma::peso_animo`: o campo existe, está documentado com a razão certa, e a
/// esteira não o lê — `esteira.rs` chama `forma::proxima_forma_com_rho` (que usa a const
/// `FORMA_PESO_ANIMO`) em vez de `proxima_forma_com_escalas` (que recebe o parâmetro). É uma linha,
/// e está fora da fronteira deste pacote.
///
/// Não silencie declarando `("peso_animo", false)`. A guarda distingue "não é consumido e sabemos"
/// de "não é consumido e ninguém percebeu", e mentir na coluna destruiria justamente a parte que
/// funciona — é a mesma decisão já registrada para o `track_difficulty_multiplier`.
///
/// Some sozinho quando a linha for trocada.
#[test]
fn knobs_calculados_e_nunca_lidos_continuam_declarados() {
    let divergencias = consumo::divergencias();
    assert!(
        divergencias.is_empty(),
        "a classificação de consumo em consumo.rs saiu da realidade:\n{}\n\
         (se for o `peso_animo`: esteira.rs chama `proxima_forma_com_rho`; trocar para \
         `proxima_forma_com_escalas(..., escalas.peso_animo)` fecha)",
        divergencias.join("\n")
    );
}

#[test]
fn a_varredura_de_knobs_esta_sincronizada_com_a_de_consumo() {
    // Os dois arquivos falam dos mesmos parâmetros por caminhos diferentes (um varre o valor, o
    // outro varre o fonte). Se um ganhar um knob e o outro não, a próxima calibração mede
    // errado sem avisar.
    let na_varredura: Vec<&str> = Knob::todos().iter().map(|k| k.nome()).collect();
    for nome in na_varredura {
        assert!(
            consumo::CLASSIFICACAO.iter().any(|(n, _)| *n == nome),
            "`{nome}` está na varredura mas não na classificação de consumo"
        );
    }
}

// ---------------------------------------------------------------------------
// Camada pesada — as asserções de faixa (critério de aceitação do conserto)
// ---------------------------------------------------------------------------

/// Configuração dos testes pesados: 20 pilotos × 12 etapas × 84 temporadas = 1008 corridas.
fn campanha_pesada(base: ConfigTemporada) -> ConfigTemporada {
    ConfigTemporada {
        pilotos: 20,
        etapas: 12,
        ..base
    }
}

const TEMPORADAS_PESADAS: usize = 84;

fn conferir(agregado: &metricas::MetricasAgregadas, alvos: &Alvos) -> Vec<String> {
    let mut falhas = Vec::new();
    let mut checar = |nome: &str, valor: f64, faixa: Faixa| {
        if !faixa.contem(valor) {
            falhas.push(format!(
                "{nome}: medido {valor:.3}, alvo {:.3}–{:.3} ({})",
                faixa.min,
                faixa.max,
                faixa.veredito(valor)
            ));
        }
    };

    checar(
        "spearman_grid_chegada",
        agregado.spearman_grid_chegada,
        alvos.spearman_grid_chegada,
    );
    checar(
        "pct_vitorias_do_pole",
        agregado.pct_vitorias_do_pole,
        alvos.pct_vitorias_do_pole,
    );
    checar(
        "vencedores_distintos",
        agregado.vencedores_distintos,
        alvos.vencedores_distintos,
    );
    checar(
        "desvio_posicao",
        agregado.desvio_posicao,
        alvos.desvio_posicao,
    );
    checar(
        "p_melhor_fora_top5",
        agregado.p_melhor_fora_top5,
        alvos.p_melhor_fora_top5,
    );
    checar(
        "trocas_de_lideranca",
        agregado.trocas_de_lideranca,
        alvos.trocas_de_lideranca,
    );
    checar(
        "margem_do_campeao",
        agregado.margem_do_campeao,
        alvos.margem_do_campeao,
    );
    checar(
        "spearman_etapas_consecutivas",
        agregado.spearman_etapas_consecutivas,
        alvos.spearman_etapas_consecutivas,
    );
    falhas
}

#[test]
#[ignore = "pesado (~1000 corridas) e FALHA hoje — é o critério de aceitação do conserto"]
fn rookie_distribui_como_corrida_de_verdade() {
    let config = campanha_pesada(ConfigTemporada::rookie());
    let agregado = arena::medir("mazda_rookie", &config, TEMPORADAS_PESADAS, 2026);
    let alvos = Alvos::entrada();

    let falhas = conferir(&agregado, &alvos);
    assert!(
        falhas.is_empty(),
        "{}\n{}",
        relatorio::tabela(&agregado, &alvos),
        falhas.join("\n")
    );
}

#[test]
#[ignore = "pesado (~1000 corridas) e FALHA hoje — é o critério de aceitação do conserto"]
fn gt3_distribui_como_corrida_de_verdade() {
    let config = campanha_pesada(ConfigTemporada::gt3());
    let agregado = arena::medir("gt3", &config, TEMPORADAS_PESADAS, 2027);
    let alvos = Alvos::topo();

    let falhas = conferir(&agregado, &alvos);
    assert!(
        falhas.is_empty(),
        "{}\n{}",
        relatorio::tabela(&agregado, &alvos),
        falhas.join("\n")
    );
}

/// A hipótese central do pacote: rookie e topo deveriam ser caóticas por motivos OPOSTOS e em
/// graus diferentes. Se as duas medirem igual, alguma coisa está achatando as duas.
#[test]
#[ignore = "pesado (~2000 corridas) e FALHA hoje — é o critério de aceitação do conserto"]
fn rookie_e_mais_caotica_que_o_topo() {
    let rookie = arena::medir(
        "mazda_rookie",
        &campanha_pesada(ConfigTemporada::rookie()),
        TEMPORADAS_PESADAS,
        2026,
    );
    let gt3 = arena::medir(
        "gt3",
        &campanha_pesada(ConfigTemporada::gt3()),
        TEMPORADAS_PESADAS,
        2027,
    );

    assert!(
        rookie.spearman_etapas_consecutivas + 0.10 < gt3.spearman_etapas_consecutivas,
        "rookie deveria embaralhar CLARAMENTE mais entre etapas \
         (rookie={:.3}, gt3={:.3})",
        rookie.spearman_etapas_consecutivas,
        gt3.spearman_etapas_consecutivas
    );
    assert!(
        rookie.desvio_posicao > gt3.desvio_posicao + 0.5,
        "rookie deveria ter posição de chegada bem mais volátil \
         (rookie={:.2}, gt3={:.2})",
        rookie.desvio_posicao,
        gt3.desvio_posicao
    );
}

/// Os quatro cenários do baseline congelado, com os rótulos que `snapshot::CONGELADO` usa.
fn cenarios_do_baseline() -> Vec<(&'static str, ConfigTemporada, Alvos, u64)> {
    vec![
        (
            "mazda_rookie (sem incidentes)",
            ConfigTemporada::rookie(),
            Alvos::entrada(),
            snapshot::SEMENTE_ROOKIE,
        ),
        (
            "mazda_rookie (com incidentes)",
            ConfigTemporada::rookie().com_incidentes(true),
            Alvos::entrada(),
            snapshot::SEMENTE_ROOKIE,
        ),
        (
            "gt3 (sem incidentes)",
            ConfigTemporada::gt3(),
            Alvos::topo(),
            snapshot::SEMENTE_GT3,
        ),
        (
            "gt3 (com incidentes)",
            ConfigTemporada::gt3().com_incidentes(true),
            Alvos::topo(),
            snapshot::SEMENTE_GT3,
        ),
    ]
}

/// Imprime o baseline completo. Não afirma nada — é a fonte dos números do relatório.
#[test]
#[ignore = "gerador do relatório de baseline; roda com --nocapture"]
fn imprime_baseline() {
    let blocos: Vec<_> = cenarios_do_baseline()
        .into_iter()
        .map(|(rotulo, base, alvos, semente)| {
            (
                arena::medir(rotulo, &campanha_pesada(base), TEMPORADAS_PESADAS, semente),
                alvos,
            )
        })
        .collect();

    println!("{}", relatorio::relatorio(&blocos));

    // Literal pronto para colar em `snapshot::CONGELADO` — recongelar é copiar e colar.
    println!("\n---- literal para snapshot::CONGELADO ----\n");
    for (m, _) in &blocos {
        print!("{}", snapshot::literal(m));
    }
}

/// **Antes vs depois.** Re-mede exatamente os cenários e sementes de `snapshot::CONGELADO` e
/// imprime o diff. É o comando a rodar depois de cada pacote (B, C, D, ...).
#[test]
#[ignore = "diff contra o baseline congelado; roda com --nocapture"]
fn compara_com_congelado() {
    let mut saida = String::from("\n== DIFF CONTRA O BASELINE CONGELADO ==\n");
    for (rotulo, base, _, semente) in cenarios_do_baseline() {
        let agregado = arena::medir(rotulo, &campanha_pesada(base), TEMPORADAS_PESADAS, semente);
        saida.push_str(&snapshot::diff(&agregado));
    }
    saida.push_str("\n* = mudança acima de 0,005. Sem asteriscos, nada mudou.\n");
    println!("{saida}");
}

/// Imprime o orçamento de variância das duas categorias — a peça que diz O QUE ajustar.
#[test]
#[ignore = "pesado; gerador do relatório de decomposição de variância"]
fn imprime_decomposicao_de_variancia() {
    let mut saida = String::from("\n== DECOMPOSIÇÃO DE VARIÂNCIA ==\n");
    for (rotulo, base, semente) in [
        ("mazda_rookie", ConfigTemporada::rookie(), 2026_u64),
        ("gt3", ConfigTemporada::gt3(), 2027),
    ] {
        let config = ConfigDecomposicao::padrao(base);
        saida.push_str(&relatorio::tabela_variancia(
            &variancia::decompor_variancia(rotulo, &config, semente),
        ));
    }
    println!("{saida}");
}

/// Imprime o orçamento COM a esteira de forma ligada, lado a lado com o sem.
///
/// É a medição que responde se o déficit da camada de evento é do tamanho que eu reportei. Sem a
/// esteira o harness não exercita nenhuma das três camadas do pacote B — ver
/// [`arena::aplicar_esteira_de_forma`].
#[test]
#[ignore = "pesado; gerador do comparativo com/sem a esteira de forma"]
fn imprime_decomposicao_com_esteira() {
    let mut saida = String::from("\n== DECOMPOSIÇÃO: SEM vs COM A ESTEIRA DE FORMA ==\n");
    for (rotulo, base, semente) in [
        ("mazda_rookie", ConfigTemporada::rookie(), 2026_u64),
        ("gt3", ConfigTemporada::gt3(), 2027),
    ] {
        for (sufixo, ligada) in [(" [SEM esteira]", false), (" [COM esteira]", true)] {
            let config = ConfigDecomposicao::padrao(base.clone().com_esteira_de_forma(ligada));
            saida.push_str(&relatorio::tabela_variancia(
                &variancia::decompor_variancia(&format!("{rotulo}{sufixo}"), &config, semente),
            ));
        }
    }
    println!("{saida}");
}

/// Imprime as oito métricas de resultado COM a esteira ligada — o efeito no sintoma central.
#[test]
#[ignore = "pesado; gerador do comparativo de resultado com a esteira"]
fn imprime_resultado_com_esteira() {
    let mut blocos = Vec::new();
    for (rotulo, base, alvos, semente) in [
        (
            "mazda_rookie",
            ConfigTemporada::rookie(),
            Alvos::entrada(),
            snapshot::SEMENTE_ROOKIE,
        ),
        (
            "gt3",
            ConfigTemporada::gt3(),
            Alvos::topo(),
            snapshot::SEMENTE_GT3,
        ),
    ] {
        for (sufixo, ligada) in [("sem esteira", false), ("COM esteira", true)] {
            let config = base.clone().com_esteira_de_forma(ligada);
            blocos.push((
                arena::medir(
                    &format!("{rotulo} ({sufixo})"),
                    &config,
                    snapshot::TEMPORADAS,
                    semente,
                ),
                alvos.clone(),
            ));
        }
    }
    println!("{}", relatorio::relatorio(&blocos));
}

// ---------------------------------------------------------------------------
// Prévia da fase 1 — ver `previa.rs` para o porquê de ser prévia e não resposta
// ---------------------------------------------------------------------------

/// O passo parametrizado tem que REDUZIR à função do jogo quando ρ é o do jogo. Sem isto, tudo o
/// que a prévia mede é a diferença entre duas implementações da forma.
#[test]
fn passo_de_forma_com_rho_do_jogo_e_o_do_jogo() {
    use crate::simulation::forma;
    for (mot, conf) in [(50.0, 50.0), (80.0, 30.0), (20.0, 90.0), (100.0, 100.0)] {
        for anterior in [-2.0, -0.5, 0.0, 0.7, 2.4] {
            for id in ["P-01", "P-17", "zzz"] {
                let s = forma::semente_forma(3, 7, id);
                let oficial = forma::proxima_forma(anterior, s, mot, conf);
                let meu = forma::proxima_forma_com_rho(anterior, s, mot, conf, forma::FORMA_RHO);
                assert!(
                    (oficial - meu).abs() < 1e-12,
                    "divergiu em ({mot},{conf},{anterior},{id}): {oficial} vs {meu}"
                );
            }
        }
    }
}

/// `redistribuir` preserva a SOMA EM VARIÂNCIA, não a soma linear — senão as duas pernas da
/// comparação teriam camadas de evento de tamanhos diferentes e nada seria comparável.
#[test]
fn redistribuir_preserva_a_soma_em_variancia() {
    let base = EscalasDeForma::default();
    for f in [0.0, 0.25, 0.5, 0.9, 1.0] {
        let movido = base.redistribuindo(f);
        assert!(
            (movido.sigma_total() - base.sigma_total()).abs() < 1e-9,
            "σ mudou ao redistribuir {f}: {} vs {}",
            movido.sigma_total(),
            base.sigma_total()
        );
        assert!(movido.afinidade <= base.afinidade + 1e-9);
        assert!(movido.acerto >= base.acerto - 1e-9);
    }
    assert!(base.redistribuindo(1.0).afinidade < 1e-9);
}

/// A fatia da afinidade hoje é a que a medição do baseline reportou como pesada demais.
#[test]
fn fatia_da_afinidade_bate_com_o_medido() {
    let f = EscalasDeForma::default().fatia_da_afinidade();
    assert!(
        (0.40..0.55).contains(&f),
        "fatia da afinidade fora do esperado: {f:.3}"
    );
}

/// **A predição falsificável.** Subir a amplitude da forma com ρ = 0,65 tem que empurrar
/// ρ(N × N+1) para CIMA, porque com autocorrelação a forma é fonte parcialmente permanente.
///
/// Gerador, não asserção: o veredito é do relatório. Mas ele existe como teste para que a
/// predição fique no código, feita ANTES da medição.
#[test]
#[ignore = "pesado; prévia — varredura do par FORMA_ESCALA × FORMA_RHO"]
fn imprime_previa_da_forma() {
    let mut saida = String::from(
        "\n== PRÉVIA: o par FORMA_ESCALA × FORMA_RHO ==\n\
         PREDIÇÃO (escrita antes): com ρ alto, subir a escala SOBE ρ(N,N+1).\n\
         Com ρ baixo, subir a escala DESCE ρ(N,N+1). O sinal inverte em algum ρ intermediário.\n",
    );
    for (rotulo, base, semente) in [
        ("mazda_rookie", ConfigTemporada::rookie(), 2026_u64),
        ("gt3", ConfigTemporada::gt3(), 2027),
    ] {
        saida.push_str(&format!("\n### {rotulo}\n\n| ρ \\ escala |"));
        let escalas = [2.0, 4.0, 6.0, 9.0];
        for e in escalas {
            saida.push_str(&format!(" {e:>6.1} |"));
        }
        saida.push_str("\n|------------|");
        for _ in escalas {
            saida.push_str("--------|");
        }
        saida.push('\n');

        for rho in [0.0, 0.35, 0.65, 0.85] {
            saida.push_str(&format!("| {rho:>10.2} |"));
            for e in escalas {
                let cfg = base
                    .clone()
                    .com_escalas_da_previa(EscalasDeForma::default().com_forma(e).com_rho(rho));
                let m = arena::medir("previa", &cfg, 30, semente);
                saida.push_str(&format!(" {:>6.3} |", m.spearman_etapas_consecutivas));
            }
            saida.push('\n');
        }
    }
    println!("{saida}");
}

/// **Redistribuir contra escalar, com a soma em variância travada.**
#[test]
#[ignore = "pesado; prévia — redistribuir vs escalar"]
fn imprime_previa_da_redistribuicao() {
    let mut saida = String::from(
        "\n== PRÉVIA: redistribuir vs escalar (soma em variância constante por linha) ==\n",
    );
    for (rotulo, base, semente) in [
        ("mazda_rookie", ConfigTemporada::rookie(), 2026_u64),
        ("gt3", ConfigTemporada::gt3(), 2027),
    ] {
        saida.push_str(&format!(
            "\n### {rotulo}\n\n| movimento           | σ eventoterreno | fatia afin. | ρ(N,N+1) | desvio | vencedores |\n\
             |---------------------|-----------------|-------------|----------|--------|------------|\n"
        ));
        let jogo = EscalasDeForma::default();
        let linha = |nome: &str, esc: EscalasDeForma, saida: &mut String| {
            let cfg = base.clone().com_escalas_da_previa(esc);
            let m = arena::medir("previa", &cfg, 30, semente);
            saida.push_str(&format!(
                "| {:<19} | {:>15.2} | {:>11.1}% | {:>8.3} | {:>6.2} | {:>10.2} |\n",
                nome,
                esc.sigma_total(),
                esc.fatia_da_afinidade() * 100.0,
                m.spearman_etapas_consecutivas,
                m.desvio_posicao,
                m.vencedores_distintos
            ));
        };

        linha("hoje", jogo, &mut saida);
        for fator in [1.5, 2.0] {
            let escalado = jogo.escalando(fator);
            linha(&format!("escalar ×{fator}"), escalado, &mut saida);
            // Mesma σ total, peso movido da afinidade para o acerto.
            linha(
                &format!("redistribuir ×{fator}"),
                escalado.redistribuindo(0.6),
                &mut saida,
            );
        }
    }
    println!("{saida}");
}

/// O excesso de emenda tem que ser ~0 quando a fonte não tem memória, por construção do nulo.
/// Sem isto, qualquer número que a assinatura reportar pode ser viés do instrumento.
///
/// **40 temporadas, o mesmo volume do relatório que sustenta a faixa** — e isso é a asserção, não
/// uma escolha de conveniência. A 12 temporadas o nulo mede −0,057, a 40 mede −0,016 a −0,002: o
/// instrumento é não-viesado, mas o ruído dele é da ordem de 0,05 em volume baixo. Se a guarda
/// rodasse com menos, ela estaria travando o ruído em vez do viés — e o piso de
/// `FAIXA_DE_EMENDA` (0,08) foi escolhido contra o ruído deste volume, não de outro.
#[test]
fn assinatura_e_zero_sem_memoria() {
    let config = ConfigTemporada::rookie()
        .com_escalas_da_previa(EscalasDeForma::default().com_forma(6.0).com_rho(0.0));
    let campanha = arena::rodar_campanha_crua(&config, 40, 4242);
    let a = assinatura::medir(&campanha, 7);
    assert!(
        a.excesso_de_emenda.abs() < 0.20,
        "com ρ = 0 o excesso deveria ser ~0, veio {:.3}",
        a.excesso_de_emenda
    );
    // A métrica que virou alvo precisa do mesmo aval, e é ela que define o piso da faixa.
    assert!(
        a.excesso_de_emenda_percebida.abs() < 0.04,
        "com ρ = 0 o excesso de emenda percebida deveria ser ~0, veio {:.3}",
        a.excesso_de_emenda_percebida
    );
}

/// A faixa da nona métrica tem que estar ACIMA do que o jogo entrega hoje — senão ela nasce
/// satisfeita e não é alvo de nada.
#[test]
fn faixa_de_emenda_exige_mais_do_que_hoje() {
    let (piso, teto) = assinatura::FAIXA_DE_EMENDA;
    assert!(
        piso > 0.046,
        "o piso ({piso:.3}) tem que superar o medido hoje (0,046)"
    );
    assert!(teto > piso && teto <= 0.25);
}

/// **O piso de `FORMA_RHO`**: varre ρ e mede quanta sequência sobra, para o piso sair de medição.
#[test]
#[ignore = "pesado; prévia — o piso de FORMA_RHO pela assinatura temporal"]
fn imprime_piso_de_forma_rho() {
    let mut saida = String::from(
        "\n== PISO DE FORMA_RHO — assinatura temporal ==\n\
         Emenda = maior sequência de resultados acima da mediana pessoal, em corridas.\n\
         Nulo = a MESMA temporada com a ordem embaralhada (teste de permutação).\n",
    );
    for (rotulo, base, semente, etapas) in [
        (
            "mazda_rookie (12 etapas)",
            ConfigTemporada::rookie(),
            2026_u64,
            12,
        ),
        (
            "mazda_rookie (24 etapas)",
            ConfigTemporada::rookie(),
            2026,
            24,
        ),
        ("gt3 (12 etapas)", ConfigTemporada::gt3(), 2027, 12),
    ] {
        saida.push_str(&format!(
            "\n### {rotulo}\n\n| FORMA_RHO | P(emenda 3+) | nulo | **excesso** | exc. compr. |\n\
             |-----------|-------------|-------------|-------------|-------------------|\n"
        ));
        for rho in [0.0, 0.25, 0.35, 0.45, 0.55, 0.65, 0.80] {
            let mut cfg = base
                .clone()
                .com_escalas_da_previa(EscalasDeForma::default().com_forma(6.0).com_rho(rho));
            cfg.etapas = etapas;
            let campanha = arena::rodar_campanha_crua(&cfg, 40, semente);
            let a = assinatura::medir(&campanha, 99);
            saida.push_str(&format!(
                "| {rho:>9.2} | {:>11.3} | {:>11.3} | {:>11.3} | {:>17.2} |\n",
                a.p_emenda_percebida,
                a.p_emenda_percebida_nula,
                a.excesso_de_emenda_percebida,
                a.excesso_de_emenda
            ));
        }
    }
    // A pergunta que decide se o piso resolve: a emenda depende mais de ρ ou da AMPLITUDE?
    saida.push_str(
        "\n### Efeito da amplitude, com ρ = 0,65 fixo (rookie, 12 etapas)\n\n\
         | FORMA_ESCALA | P(emenda 3+) | nulo | **excesso** |\n\
         |--------------|-------------|-------------|-------------|\n",
    );
    for escala in [2.0, 4.0, 6.0, 9.0, 14.0] {
        let cfg = ConfigTemporada::rookie()
            .com_escalas_da_previa(EscalasDeForma::default().com_forma(escala).com_rho(0.65));
        let campanha = arena::rodar_campanha_crua(&cfg, 40, 2026);
        let a = assinatura::medir(&campanha, 99);
        saida.push_str(&format!(
            "| {escala:>12.1} | {:>11.3} | {:>11.3} | {:>11.3} |\n",
            a.p_emenda_percebida, a.p_emenda_percebida_nula, a.excesso_de_emenda_percebida
        ));
    }

    saida.push_str(&format!(
        "\nPiso adotado: {:.2} (ver `assinatura::PISO_DE_FORMA_RHO`).\n",
        assinatura::PISO_DE_FORMA_RHO
    ));
    println!("{saida}");
}

/// **A contaminação do ânimo**: quanto da camada de EVENTO é, na verdade, permanente.
///
/// O termo `peso_animo × animo` é somado dentro da construção AR(1). Como `normalizar_atributo`
/// centra em 50 e o neutro da motivação é 70, o termo **não zera no ponto neutro** — e termo
/// constante num AR(1) vira média estacionária `c/(1−ρ)`, que com ρ = 0,65 multiplica por 2,86.
/// O resultado é um deslocamento por piloto, praticamente permanente, morando dentro da camada
/// que a decomposição contabiliza como evento.
///
/// A medição é pareada: `peso_animo = 0` desliga o deslocamento e deixa só a parte serial. O delta
/// é a contaminação. Mesmo padrão dos pares casados e do diagnóstico do safety car.
#[test]
#[ignore = "pesado; contaminação do ânimo na camada de evento"]
fn imprime_contaminacao_do_animo() {
    let mut saida = String::from(
        "\n== CONTAMINAÇÃO DO ÂNIMO NA CAMADA DE EVENTO ==\n\
         Pareado: mesma semente, mesmo grid, só `peso_animo` muda.\n",
    );
    for (rotulo, base, semente) in [
        ("mazda_rookie", ConfigTemporada::rookie(), 2026_u64),
        ("gt3", ConfigTemporada::gt3(), 2027),
    ] {
        // O padrão do jogo virou 0 — a remoção foi decisão de mecanismo, não de magnitude. Então a
        // comparação útil é contra o valor ANTIGO (0,20): ela mede o que a remoção comprou, e
        // corrige o orçamento que eu havia reportado com a contaminação dentro.
        for (sufixo, peso) in [
            (" [ânimo 0,20 — como era]", 0.20),
            (" [ânimo 0 — o jogo hoje]", 0.0),
        ] {
            let mut esc = EscalasDeForma::default();
            esc.peso_animo = peso;
            let config = ConfigDecomposicao::padrao(base.clone().com_escalas_da_previa(esc));
            saida.push_str(&relatorio::tabela_variancia(
                &variancia::decompor_variancia(&format!("{rotulo}{sufixo}"), &config, semente),
            ));
        }
    }
    println!("{saida}");
}

/// Imprime as métricas de processo e o experimento do poder da largada.
#[test]
#[ignore = "pesado; gerador do relatório de métricas de processo"]
fn imprime_processo() {
    let mut saida = String::from("\n== MÉTRICAS DE PROCESSO ==\n");
    for (rotulo, base, semente) in [
        ("mazda_rookie", ConfigTemporada::rookie(), 2026_u64),
        ("gt3", ConfigTemporada::gt3(), 2027),
    ] {
        let config = campanha_pesada(base);
        saida.push_str(&relatorio::tabela_processo(
            &processo::medir_campanha_processo(rotulo, &config, TEMPORADAS_PESADAS, semente),
        ));
        saida.push_str(&relatorio::tabela_largada(
            &processo::medir_poder_da_largada(rotulo, &config, 400, semente),
        ));
    }
    saida.push_str("\n### Lacuna conhecida\n\n");
    saida.push_str(processo::LACUNA_SEGMENTO);
    saida.push('\n');
    println!("{saida}");
}

/// **Diagnóstico do próprio método**: o T1 de 24 corridas tem sinal suficiente para triagem, ou o
/// ruído de amostragem descarta ponto bom por sorteio? Compara o desvio-padrão do objetivo ao
/// repetir a MESMA configuração com o intervalo que ele percorre ao varrer um eixo.
#[test]
#[ignore = "diagnóstico do método de busca; roda com --nocapture"]
fn imprime_sinal_sobre_ruido_da_triagem() {
    let base = ConfigTemporada {
        pilotos: 20,
        ..ConfigTemporada::rookie()
    };
    let alvos = Alvos::entrada();
    let eixo = busca::Eixo {
        knob: Knob::RaceVariance,
        valores: varredura::faixa_padrao(Knob::RaceVariance),
    };

    println!("\n== SINAL / RUÍDO DA TRIAGEM ==\n");
    println!(
        "| {:<6} | {:>9} | {:>10} | {:>10} | {:>9} |",
        "Nível", "corridas", "ruído (dp)", "amplitude", "sinal/ruído"
    );
    println!("|--------|-----------|------------|------------|-----------|");

    for nivel in [busca::Nivel::T1, busca::Nivel::T2, busca::Nivel::T3] {
        let (_, dp) = busca::ruido_de_amostragem(&base, &alvos, nivel, 12);
        let amplitude = busca::amplitude_do_eixo(&base, &alvos, &eixo, nivel, 4242);
        println!(
            "| {:<6} | {:>9} | {:>10.3} | {:>10.3} | {:>9.1} |",
            format!("{nivel:?}"),
            nivel.corridas(),
            dp,
            amplitude,
            amplitude / dp.max(1e-9)
        );
    }
    println!(
        "\nRegra: sinal/ruído abaixo de ~3 significa que a triagem está descartando ponto bom \
         por sorteio."
    );

    // A validação que importa: a peneira preserva a ORDEM que o nível caro daria?
    //
    // Medida sobre VÁRIOS eixos e várias sementes, porque um ρ sobre os 8 pontos de um eixo só é
    // estimativa ruidosa — foi assim que a primeira versão desta guarda ficou frágil.
    println!("\n-- concordância de ordenação T1 × T2 (Spearman por eixo) --");
    let mut todos = Vec::new();
    for eixo_knob in [
        Knob::RaceVariance,
        Knob::PackDensity,
        Knob::StartChaos,
        Knob::RacePaceSpread,
        Knob::OvertakingDifficulty,
    ] {
        let e = busca::Eixo {
            knob: eixo_knob,
            valores: varredura::faixa_padrao(eixo_knob),
        };
        let por_semente: Vec<f64> = [4242_u64, 777, 31337]
            .iter()
            .map(|s| {
                busca::concordancia_de_triagem(
                    &base,
                    &alvos,
                    &e,
                    busca::Nivel::T1,
                    busca::Nivel::T2,
                    *s,
                )
            })
            .collect();
        let m = por_semente.iter().sum::<f64>() / por_semente.len() as f64;
        todos.push(m);
        println!(
            "  {:<34} rho = {:.3}  (por semente: {:.2} {:.2} {:.2})",
            eixo_knob.nome(),
            m,
            por_semente[0],
            por_semente[1],
            por_semente[2]
        );
    }
    println!(
        "  {:<34} rho = {:.3}  <- é este que a guarda olha",
        "MÉDIA SOBRE OS EIXOS",
        todos.iter().sum::<f64>() / todos.len() as f64
    );
    println!("\nAcima de ~0,8 a peneira é fiel: o que o nível caro prefere sobrevive a ela.");

    // Qual FORMA de T1 é fiel? Medida, não escolhida — a forma certa depende do motor, e o motor
    // mudou. Custo é o número de corridas; fidelidade é a concordância com o T2.
    println!("\n-- qual forma de T1 é fiel? (média sobre 3 eixos × 2 sementes) --");
    println!(
        "| {:<12} | {:>9} | {:>10} | {:>14} |",
        "forma", "corridas", "rho x T2", "arrependimento"
    );
    println!("|--------------|-----------|------------|----------------|");
    for (temporadas, etapas) in [(12, 6), (12, 12), (15, 10), (20, 8), (30, 6)] {
        let forma = busca::Nivel::nova(temporadas, etapas);
        let mut rhos = Vec::new();
        let mut regrets = Vec::new();
        for knob in [
            Knob::RaceVariance,
            Knob::RacePaceSpread,
            Knob::OvertakingDifficulty,
            Knob::PackDensity,
        ] {
            let e = busca::Eixo {
                knob,
                valores: varredura::faixa_padrao(knob),
            };
            for s in [4242_u64, 777] {
                rhos.push(busca::concordancia_de_triagem(
                    &base,
                    &alvos,
                    &e,
                    forma,
                    busca::Nivel::T2,
                    s,
                ));
                regrets.extend(busca::arrependimento_da_triagem(
                    &base,
                    &alvos,
                    &e,
                    forma,
                    busca::Nivel::T2,
                    s,
                ));
            }
        }
        let media = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
        println!(
            "| {:<12} | {:>9} | {:>10.3} | {:>14.3} |",
            format!("{temporadas}x{etapas}"),
            forma.corridas(),
            media(&rhos),
            media(&regrets)
        );
    }
    println!(
        "\nrho é diagnóstico e vai a zero em eixo plano por construção. Quem decide é o \
         ARREPENDIMENTO:\nquanto a peneira custa em objetivo por jogar fora o ponto que o T2 \
         escolheria. 0 = não custou nada."
    );
}

#[test]
fn triagem_t1_preserva_a_ordem_do_nivel_caro() {
    // Guarda permanente da peneira. Se o T1 deixar de concordar com o T2, ele passa a descartar
    // ponto bom por sorteio e a busca inteira fica pior que uma varredura burra.
    let base = ConfigTemporada {
        pilotos: 20,
        ..ConfigTemporada::rookie()
    };
    let eixo = busca::Eixo {
        knob: Knob::RaceVariance,
        valores: varredura::faixa_padrao(Knob::RaceVariance),
    };
    // Média sobre vários eixos e sementes: um número sobre 8 pontos de um eixo só é ruidoso, e foi
    // assim que a primeira versão desta guarda ficou frágil.
    let mut regrets = Vec::new();
    for knob in [
        Knob::RaceVariance,
        Knob::PackDensity,
        Knob::RacePaceSpread,
        Knob::OvertakingDifficulty,
    ] {
        let e = busca::Eixo {
            knob,
            valores: varredura::faixa_padrao(knob),
        };
        for s in [4242_u64, 777] {
            regrets.extend(busca::arrependimento_da_triagem(
                &base,
                &Alvos::entrada(),
                &e,
                busca::Nivel::T1,
                busca::Nivel::T2,
                s,
            ));
        }
    }
    // Eixos planos dentro do ruído não entram (devolvem `None`): ali não há ponto bom a perder.
    assert!(
        !regrets.is_empty(),
        "nenhum eixo com alavanca suficiente para medir arrependimento — a guarda ficou vazia"
    );
    let medio = regrets.iter().sum::<f64>() / regrets.len() as f64;
    let pior = regrets.iter().cloned().fold(0.0_f64, f64::max);

    assert!(
        medio < 0.15,
        "a peneira T1 passou a custar objetivo: arrependimento médio {medio:.3} (máx {pior:.3}). \
         Ela está jogando fora o ponto que o T2 escolheria."
    );
}

#[test]
fn eixo_plano_nao_gera_arrependimento() {
    // A propriedade que motivou trocar a guarda: num eixo sem efeito, qualquer ponto serve, então
    // a peneira não pode ser acusada de errar. `track_difficulty` é o morto por magnitude — o eixo
    // plano de referência.
    let base = ConfigTemporada {
        pilotos: 16,
        ..ConfigTemporada::rookie()
    };
    let eixo = busca::Eixo {
        knob: Knob::TrackDifficulty,
        valores: varredura::faixa_padrao(Knob::TrackDifficulty),
    };
    let regret = busca::arrependimento_da_triagem(
        &base,
        &Alvos::entrada(),
        &eixo,
        busca::Nivel::T1,
        busca::Nivel::T2,
        4242,
    );
    assert!(
        regret.is_none(),
        "eixo plano dentro do ruído: arrependimento não é quantidade definida, e devolver um \
         número ({regret:?}) seria ruído dividido por quase-zero"
    );
}

/// Imprime o relatório da busca sobre o espaço de parâmetros atual.
#[test]
#[ignore = "gerador do relatório da busca; roda com --nocapture"]
fn imprime_busca() {
    for (rotulo, base, alvos, semente) in [
        (
            "mazda_rookie",
            ConfigTemporada::rookie(),
            Alvos::entrada(),
            2026_u64,
        ),
        ("gt3", ConfigTemporada::gt3(), Alvos::topo(), 2027),
    ] {
        let r = busca::buscar_com_duas_partidas(
            rotulo,
            ConfigTemporada {
                pilotos: 20,
                ..base
            },
            alvos,
            &busca::espaco_atual(),
            300,
            semente,
        );
        println!("{}", relatorio::tabela_busca(&r));
    }
}

/// **A varredura das constantes de POSIÇÃO NA PISTA (A1.1).**
///
/// As oito de `race::trafego` eram `const` e por isso invisíveis para esta máquina: ela varre
/// campo de contexto, e uma `const` não é campo de nada. Isso deixava a busca fechar os knobs
/// externos por cima de um conjunto interno que ninguém tinha medido — o erro de baixo ficando
/// travado embaixo de uma calibração com cara de boa.
///
/// A faixa é MULTIPLICATIVA sobre o valor de hoje (0× a 3×), porque estas têm unidade — varrer
/// `janela_ar_sujo_ms` na lista `[0 … 10]` dos multiplicadores daria uma janela de 10 ms, que é
/// o mesmo que desligá-la. O ponto `0×` é de propósito o mais informativo: se apagar a constante
/// não move nada, ela é decorativa independentemente do valor que se escolha.
///
/// A saída que interessa aqui é `ρ(grid)` — o quanto largar na frente decide a chegada. Ela
/// entrou em [`varredura::Saida`] junto com estes knobs; sem ela a tabela seria cega, porque é
/// exatamente a métrica que estas constantes existem para mover.
///
/// Separado de [`imprime_varredura_de_knobs`] de propósito: os nove de contexto já têm varredura
/// publicada, e rodá-los de novo só gasta CPU.
#[test]
#[ignore = "MUITO pesado; varredura das constantes de tráfego (A1.1)"]
fn varredura_do_trafego_mede_alavanca() {
    let mut saida = String::from("\n== VARREDURA DAS CONSTANTES DE TRÁFEGO ==\n");
    for (rotulo, base, semente) in [
        ("mazda_rookie", ConfigTemporada::rookie(), 4242_u64),
        ("gt3", ConfigTemporada::gt3(), 4243),
    ] {
        let config = ConfigTemporada {
            etapas: 12,
            pilotos: 20,
            ..base
        }
        .com_incidentes(true);

        let varreduras: Vec<_> = Knob::de_trafego()
            .into_iter()
            .map(|knob| {
                varredura::varrer(
                    rotulo,
                    &config,
                    knob,
                    &varredura::faixa_padrao(knob),
                    6,
                    semente,
                )
            })
            .collect();

        saida.push_str(&format!("\n## {rotulo}\n"));
        saida.push_str(&relatorio::matriz_de_alavanca(&varreduras));
        saida.push_str(&relatorio::tabela_varreduras(&varreduras));
        for v in &varreduras {
            saida.push_str(&relatorio::detalhe_varredura(v));
        }
    }
    println!("{saida}");
}

/// Imprime a varredura de sensibilidade de todos os knobs — a lista de mortos vs com alavanca.
#[test]
#[ignore = "MUITO pesado; gerador do relatório de sensibilidade dos knobs"]
fn imprime_varredura_de_knobs() {
    let mut saida = String::from("\n== VARREDURA DE SENSIBILIDADE ==\n");
    for (rotulo, base, semente) in [
        ("mazda_rookie", ConfigTemporada::rookie(), 2026_u64),
        ("gt3", ConfigTemporada::gt3(), 2027),
    ] {
        // Incidentes LIGADOS: agora que frequência de safety car é uma saída medida, varrer com
        // eles desligados daria SC/etapa = 0 constante em todo knob e a coluna inteira sairia
        // "morta" por construção do experimento, não por propriedade do knob.
        let config = ConfigTemporada {
            etapas: 12,
            pilotos: 20,
            ..base
        }
        .com_incidentes(true);
        let varreduras = varredura::varrer_todos(rotulo, &config, 30, semente);
        saida.push_str(&format!("\n## {rotulo}\n"));
        saida.push_str(&relatorio::matriz_de_alavanca(&varreduras));
        saida.push_str(&relatorio::tabela_varreduras(&varreduras));
        for v in &varreduras {
            saida.push_str(&relatorio::detalhe_varredura(v));
        }
    }
    println!("{saida}");
}
