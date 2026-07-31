use super::*;

#[test]
fn percentil_ordena_do_pior_ao_melhor() {
    let all = vec![10.0, 20.0, 30.0, 40.0];
    assert!(percentile(40.0, &all) > percentile(10.0, &all));
    assert_eq!(percentile(40.0, &all), 1.0);
}

#[test]
fn jitter_e_deterministico_e_limitado() {
    let a = jitter("P123");
    assert_eq!(a, jitter("P123"), "mesmo id → mesmo ruído");
    assert!(a.abs() <= JITTER_RANGE / 2.0 + 1e-9);
    assert_ne!(jitter("P123"), jitter("P124"));
}

/// O CURRÍCULO tem que pesar mais que o skill oculto: um piloto com pódio precisa
/// ficar à frente de outro sem resultado, mesmo com skill bem menor. É a assimetria
/// de informação do design (§5.2) — a imprensa lê resultado, não ritmo.
#[test]
fn podio_pesa_mais_que_skill_na_percepcao() {
    let com_podio = W_PODIUM * 1.0 + W_SKILL_HINT * 40.0;
    let so_skill = W_SKILL_HINT * 50.0;
    assert!(
        com_podio > so_skill + JITTER_RANGE,
        "um pódio deve superar a vantagem de skill mesmo no pior caso de ruído"
    );
}

/// Um dossiê de mentira, só para exercitar a prosa do fallback.
fn dossie(nome: &str, equipe: Option<&str>, titulo: bool, vitoria: bool, podio: bool) -> Dossier {
    Dossier {
        id: nome.to_string(),
        nome: nome.to_string(),
        equipe: equipe.map(|s| s.to_string()),
        perception: 0.0,
        curriculo: String::new(),
        experiencia: String::new(),
        tracos: vec!["aggressive"],
        ganchos: Vec::new(),
        tem_titulo: titulo,
        tem_vitoria: vitoria,
        tem_podio: podio,
        estreante: !vitoria && !podio,
    }
}

/// A matéria de fallback é a que o jogador lê quando a IA não responde — ela não pode
/// soar preenchida por máquina. Dois guards do que já saiu errado: a mesma fôrma repetida
/// por piloto e o nome da equipe entre parênteses colado no nome.
#[test]
fn fallback_nao_repete_a_mesma_forma_por_piloto() {
    let data = PreviewData {
        facts: String::new(),
        teams: serde_json::json!({}),
        thesis: Thesis::OpenOnTalent,
        material: Material::Uniform,
        ranked: vec![
            dossie("Ana Reis", Some("Meteora"), false, true, true),
            dossie("Bruno Sá", Some("Arcen"), false, false, true),
            dossie("Caio Melo", Some("Vento"), false, false, false),
            dossie("Davi Luz", Some("Norte"), false, false, false),
            dossie("Elis Prado", None, false, false, false),
        ],
        relations: vec!["Ana Reis e Bruno Sá já dividiram equipe.".to_string()],
        opening_track: Some("Interlagos".to_string()),
        rounds: 12,
        champion: Some("Ana Reis".to_string()),
        throne_vacant: false,
        cat_label: "F3".to_string(),
        year: 2027,
    };

    let art = deterministic_article(&data);
    let body = art.body;

    assert!(!art.headline.is_empty() && !art.standfirst.is_empty());
    // Os dois primeiros são agressivos: a oração de estilo só pode aparecer uma vez.
    assert_eq!(
        body.matches("briga por cada posição").count(),
        1,
        "dois pilotos ganharam a mesma frase de estilo:\n{body}"
    );
    assert_eq!(
        body.split("\n\n").count(),
        3,
        "a matéria tem três parágrafos — cenário, topo e o resto:\n{body}"
    );
    assert!(
        !body.contains('('),
        "nome de equipe entre parênteses vira ficha técnica, não prosa:\n{body}"
    );
    for nome in ["Ana Reis", "Bruno Sá", "Caio Melo"] {
        assert!(body.contains(nome), "{nome} ficou de fora:\n{body}");
    }
    // Nenhuma frase do topo pode começar igual à outra — é o sintoma de fôrma repetida.
    let aberturas: Vec<&str> = body
        .split("\n\n")
        .nth(1)
        .unwrap()
        .split(". ")
        .map(|s| s.split(' ').next().unwrap_or(""))
        .collect();
    let unicas: std::collections::HashSet<&&str> = aberturas.iter().collect();
    assert_eq!(
        unicas.len(),
        aberturas.len(),
        "frases repetem a abertura:\n{body}"
    );
}

#[test]
fn tese_de_estreantes_vence_quando_grid_e_novato() {
    assert!(matches!(
        select_thesis(0.8, &Material::Uniform, false),
        Thesis::RookieSeason
    ));
    assert!(matches!(
        select_thesis(0.1, &Material::Uniform, true),
        Thesis::VacantThrone
    ));
    assert!(matches!(
        select_thesis(0.1, &Material::Uniform, false),
        Thesis::OpenOnTalent
    ));
}

// ── Cache e backoff da matéria ───────────────────────────────────────────────────

use super::comando::{cached_teams, recent_attempt, RETRY_BACKOFF_SECS};

fn linha_de_cache(created_at: &str) -> ai_story::AiStoryRow {
    ai_story::AiStoryRow {
        facts: String::new(),
        story: None,
        teams_json: None,
        created_at: created_at.to_string(),
    }
}

/// O template não é cacheado de propósito, então é o carimbo da tentativa que impede a
/// revista de disparar uma requisição de rede a cada abertura num save em cooldown.
#[test]
fn backoff_segura_so_a_tentativa_recente() {
    let agora = chrono::Local::now().timestamp();
    assert!(recent_attempt(&linha_de_cache(&(agora - 60).to_string())));
    assert!(!recent_attempt(&linha_de_cache(
        &(agora - RETRY_BACKOFF_SECS - 1).to_string()
    )));
    // Carimbo ilegível (linha gravada antes da coluna existir) conta como antiga.
    assert!(!recent_attempt(&linha_de_cache("")));
}

/// Um mapa vazio de cores tem que virar `None` — senão o front tenta colorir com nada.
#[test]
fn cores_do_cache_caem_para_none_quando_nao_ha_mapa() {
    let com_cores = ai_story::AiStoryRow {
        teams_json: Some(r##"{"Petrolhead United":"#ff0000"}"##.to_string()),
        ..linha_de_cache("0")
    };
    assert!(cached_teams(&com_cores).is_some());

    let vazio = ai_story::AiStoryRow {
        teams_json: Some("{}".to_string()),
        ..linha_de_cache("0")
    };
    assert!(cached_teams(&vazio).is_none());
    assert!(cached_teams(&linha_de_cache("0")).is_none());
}
