//! Testes do engenheiro. Tudo aqui é puro: nenhum SDK, nenhuma sessão, nenhum Tauri.

mod campeonato;
mod fala;
mod marco;
mod memoria;
mod momento;
mod tratamento;
mod vizinhanca;

use crate::engenheiro::fatos::dossie;
use crate::engenheiro::{classificar, Intencao};
use crate::iracing_sdk::race_monitor::{EstadoAgora, Vizinho};

// ─── Andaimes ────────────────────────────────────────────────────────────────

/// Um estado plausível de meio de corrida, do qual cada teste desvia UM campo. Escrever o
/// desvio em vez do estado inteiro é o que deixa cada teste dizer o que está testando.
pub(super) fn estado_base() -> EstadoAgora {
    EstadoAgora {
        conectado: true,
        em_corrida: true,
        verde: true,
        em_formacao: false,
        posicao: 5,
        posicao_classe: 5,
        total_carros: 24,
        volta: 8,
        voltas_totais: 20,
        voltas_restantes: 12,
        voltas_restantes_estimadas: false,
        tempo_restante_s: -1.0,
        frente: None,
        atras: None,
        ultima_volta_s: 92.8,
        melhor_volta_s: 92.4,
        // A melhor da corrida é de OUTRO e está 4 décimos à frente da nossa: é o estado que
        // deixa a resposta de ritmo render as duas frases (o tempo e o quanto falta).
        melhor_da_corrida_s: 92.0,
        melhor_da_corrida_idx: 7,
        melhor_da_corrida_e_minha: false,
        delta_melhor_s: 0.4,
        combustivel_l: 24.0,
        consumo_por_volta_l: 1.8,
        autonomia_voltas: 13.3,
        saldo_combustivel_voltas: 1.3,
        reparo_obrigatorio_s: 0.0,
        reparo_opcional_s: 0.0,
        reparos_rapidos_usados: 0,
        bandeira: String::new(),
        bandeira_preta: false,
        desclassificado: false,
        incidentes: 2,
        pista_molhada: false,
        umidade_pista: 1,
        chuva_agora: 0.0,
        declarada_molhada: false,
        temp_pista_c: 32.0,
        temp_ar_c: 24.0,
        boxes_abertos: true,
        meu_composto: -1,
        meu_pneu_voltas: -1,
        compostos_no_grid: Vec::new(),
    }
}

pub(super) fn vizinho(nome: &str, gap: f64) -> Vizinho {
    Vizinho {
        idx: 7,
        nome: nome.to_string(),
        numero: 7,
        gap_s: gap,
        ritmo_s: 93.1,
        delta_ritmo_s: -0.3,
        voltas_para_alcancar: 4.0,
        tendencia_s_por_volta: -0.3,
        no_box: false,
        volta_a_parte: false,
        composto: -1,
        pneu_voltas: -1,
    }
}

/// Todo o dossiê numa string — os testes perguntam "isto foi dito?", não em que linha.
fn texto(e: &EstadoAgora, i: Intencao) -> String {
    dossie(e, i).join("\n")
}

// ─── Intenção ────────────────────────────────────────────────────────────────

#[test]
fn perguntas_diretas_caem_na_intencao_obvia() {
    for (pergunta, esperada) in [
        ("Em que posição eu estou?", Intencao::Posicao),
        ("Qual o gap pro carro da frente?", Intencao::Frente),
        ("Quem está atrás de mim?", Intencao::Atras),
        ("Quantas voltas faltam?", Intencao::Restante),
        ("Como está o combustível?", Intencao::Combustivel),
        ("Como está meu ritmo?", Intencao::Ritmo),
        ("O carro aguenta até o fim?", Intencao::Carro),
        ("Tem bandeira amarela?", Intencao::Bandeira),
        ("Vai chover?", Intencao::Pista),
        ("Que pneu eu estou usando?", Intencao::Pneu),
    ] {
        assert_eq!(classificar(pergunta), esperada, "pergunta: {pergunta}");
    }
}

#[test]
fn acento_e_caixa_nao_mudam_a_intencao() {
    // O Scribe transcreve com acentuação correta; a tabela é sem acento. Se a
    // normalização falhar, TODA pergunta acentuada cai em Geral — e o sintoma seria um
    // engenheiro que só responde genericamente, sem erro nenhum aparecendo em log.
    assert_eq!(classificar("COMBUSTÍVEL?"), Intencao::Combustivel);
    assert_eq!(classificar("combustivel"), Intencao::Combustivel);
    assert_eq!(classificar("Qual a posição?"), Intencao::Posicao);
    assert_eq!(classificar("Está chovendo?"), Intencao::Pista);
}

#[test]
fn pergunta_aberta_e_silencio_caem_em_geral() {
    // Geral não é falha: "como estamos?" é a pergunta mais comum de um rádio real, e o
    // núcleo do dossiê responde bem a ela.
    assert_eq!(classificar("E aí, como estamos?"), Intencao::Geral);
    assert_eq!(classificar(""), Intencao::Geral);
    assert_eq!(classificar("   "), Intencao::Geral);
}

#[test]
fn pergunta_de_pneu_sobre_o_vizinho_fala_de_pneu() {
    // O caso que quebrou a primeira versão da tabela: `da frente` casava também o termo
    // solto `frente`, somava 4 contra 3 do pneu, e a resposta vinha sobre o gap.
    assert_eq!(
        classificar("O carro da frente ainda está com pneu de seco?"),
        Intencao::Pneu
    );
}

#[test]
fn termo_inequivoco_ganha_do_ambiguo() {
    // "volta" e "tempo" são de todo mundo; "combustível" é de uma intenção só. Numa
    // pergunta que mistura os dois, quem manda é o termo que não se confunde.
    assert_eq!(
        classificar("Quantas voltas ainda dá o combustível?"),
        Intencao::Combustivel
    );
}

// ─── Dossiê ──────────────────────────────────────────────────────────────────

#[test]
fn nucleo_acompanha_qualquer_pergunta() {
    // Sem posição e voltas restantes, "quanto de combustível?" não tem resposta útil —
    // o número só significa alguma coisa contra o que falta da prova.
    let t = texto(&estado_base(), Intencao::Combustivel);
    assert!(t.contains("Posição: 5º de 24"), "faltou a posição em:\n{t}");
    assert!(
        t.contains("Faltam: 12 voltas"),
        "faltou o que resta em:\n{t}"
    );
}

#[test]
fn estimativa_de_prova_por_tempo_viaja_marcada_ate_a_fala() {
    // A marca vai NA LINHA e não num campo à parte, porque é a linha que chega ao modelo.
    // Sem ela, o engenheiro anuncia "faltam 12 voltas" numa prova por tempo — precisão
    // falsa sobre um número que o próprio sim se recusa a dar.
    let mut e = estado_base();
    e.voltas_restantes_estimadas = true;
    let t = texto(&e, Intencao::Restante);
    assert!(t.contains("por volta de 12 voltas"), "sem hedge em:\n{t}");
    assert!(
        t.contains("estimativa"),
        "sem a instrução de aproximar em:\n{t}"
    );
    assert!(
        !t.contains("Faltam: 12 voltas"),
        "vazou o número seco em:\n{t}"
    );
}

#[test]
fn valor_desconhecido_some_do_dossie_em_vez_de_virar_menos_um() {
    // A regra do arquivo: um `-1` que chega ao modelo vira fato, e ele redige em cima.
    // O piloto ouviria "menos um litro" — ou, pior, uma frase confiante sobre lixo.
    let mut e = estado_base();
    e.combustivel_l = -1.0;
    e.consumo_por_volta_l = -1.0;
    e.autonomia_voltas = -1.0;
    e.saldo_combustivel_voltas = f64::NAN;
    let t = texto(&e, Intencao::Combustivel);
    assert!(!t.contains("-1"), "sentinela vazou para o dossiê:\n{t}");
    assert!(!t.contains("NaN"), "NaN vazou para o dossiê:\n{t}");
    assert!(
        !t.contains("Combustível:"),
        "afirmou combustível sem ter:\n{t}"
    );
}

#[test]
fn saldo_negativo_manda_parar_e_saldo_ausente_cala() {
    // O par que justifica o NaN lá no EstadoAgora: -1 volta de saldo é a informação mais
    // urgente da corrida, e não pode ser confundida com ausência de informação.
    let mut e = estado_base();
    e.autonomia_voltas = 11.0;
    e.saldo_combustivel_voltas = -1.0;
    let t = texto(&e, Intencao::Combustivel);
    assert!(
        t.contains("FALTA combustível"),
        "não avisou a falta em:\n{t}"
    );

    e.saldo_combustivel_voltas = f64::NAN;
    let t = texto(&e, Intencao::Combustivel);
    assert!(
        !t.contains("FALTA"),
        "inventou falta a partir de ausência:\n{t}"
    );
    assert!(!t.contains("Saldo:"), "afirmou saldo sem ter:\n{t}");
}

#[test]
fn vizinho_detalhado_so_quando_a_pergunta_e_sobre_ele() {
    let mut e = estado_base();
    e.frente = Some(vizinho("Rodrigues", 1.2));

    // Pergunta sobre outra coisa: o vizinho aparece, mas curto.
    let curto = texto(&e, Intencao::Combustivel);
    assert!(curto.contains("Rodrigues"), "sumiu o vizinho em:\n{curto}");
    assert!(
        !curto.contains("aproximando"),
        "detalhou sem ser perguntado:\n{curto}"
    );

    // Pergunta sobre ele: vem inteiro, e sem repetir a linha curta.
    let longo = texto(&e, Intencao::Frente);
    assert!(
        longo.contains("aproximando 0,3 segundos por volta"),
        "sem tendência em:\n{longo}"
    );
    assert!(
        longo.contains("encosta em 4 voltas"),
        "sem alcance em:\n{longo}"
    );
    assert_eq!(
        longo.matches("Rodrigues").count(),
        1,
        "vizinho dito duas vezes em:\n{longo}"
    );
}

#[test]
fn sinal_do_delta_de_ritmo_vira_frase_e_nao_numero_com_sinal() {
    // Interpretar sinal é exatamente a conta que um modelo erra com confiança. O dossiê
    // escreve quem é mais rápido por extenso para que não sobre nada a deduzir.
    let mut e = estado_base();
    e.frente = Some(vizinho("Rodrigues", 1.2)); // delta -0,3 = eu mais rápido
    let t = texto(&e, Intencao::Frente);
    assert!(
        t.contains("você é 0,3 segundos por volta mais rápido"),
        "sinal torto em:\n{t}"
    );

    let mut lento = vizinho("Rodrigues", 1.2);
    lento.delta_ritmo_s = 0.5;
    lento.voltas_para_alcancar = -1.0;
    e.frente = Some(lento);
    let t = texto(&e, Intencao::Frente);
    assert!(
        t.contains("ele é 0,5 segundos por volta mais rápido"),
        "sinal torto em:\n{t}"
    );
    assert!(
        !t.contains("encosta em"),
        "prometeu alcance impossível em:\n{t}"
    );
}

#[test]
fn lideranca_e_ultimo_lugar_sao_ditos_e_nao_omitidos() {
    // Sem vizinho, calar pareceria falha de dado. "Você lidera" é a resposta certa.
    let e = estado_base();
    assert!(texto(&e, Intencao::Frente).contains("você lidera"));

    // Mas "último" exige de fato ser o último. Em quinto de vinte e quatro, um vizinho
    // ausente significa que o carro de trás saiu do `cars[]` — box, garagem ou fora do
    // mundo —, não que não haja ninguém. Dizer "você é o último" ali é uma mentira que o
    // piloto desmente pelo retrovisor, e que contamina a confiança em tudo o mais.
    let atras = texto(&e, Intencao::Atras);
    assert!(
        !atras.contains("último"),
        "afirmou último em P5 de 24:\n{atras}"
    );
    assert!(
        atras.contains("não estou vendo"),
        "não admitiu a ausência:\n{atras}"
    );

    let mut ultimo = estado_base();
    ultimo.posicao = 24;
    ultimo.total_carros = 24;
    assert!(texto(&ultimo, Intencao::Atras).contains("último"));
}

#[test]
fn tendencia_insignificante_nao_vira_frase_de_movimento() {
    let mut e = estado_base();
    let mut v = vizinho("Rodrigues", 1.2);
    v.tendencia_s_por_volta = -0.01; // ruído, não aproximação
    e.frente = Some(v);
    let t = texto(&e, Intencao::Frente);
    assert!(
        !t.contains("aproximando"),
        "anunciou movimento de um centésimo em:\n{t}"
    );
}

#[test]
fn pneu_e_nomeado_e_o_grid_descrito_por_composto() {
    // O iRacing tem dois compostos, então o índice VIRA nome — 0 é seco, 1 é chuva. Não é
    // chute: é a tradução de um domínio de dois valores.
    let mut e = estado_base();
    e.meu_composto = 0;
    e.meu_pneu_voltas = 8;
    let mut v = vizinho("Rodrigues", 1.2);
    v.composto = 1;
    v.pneu_voltas = 3;
    e.frente = Some(v);
    e.compostos_no_grid = vec![(0, 8), (1, 12)];
    let t = texto(&e, Intencao::Pneu);
    assert!(
        t.contains("Seu pneu: seco, com 8 voltas"),
        "sem o seu em:\n{t}"
    );
    assert!(
        t.contains("Rodrigues: chuva, com 3 voltas"),
        "sem o dele em:\n{t}"
    );
    assert!(
        t.contains("8 de seco, 12 de chuva"),
        "grid mal descrito em:\n{t}"
    );
}

#[test]
fn categoria_sem_composto_admite_em_vez_de_calar() {
    // O MX-5 é mono-composto e o iRacing devolve o campo vazio. Calar faria o modelo
    // improvisar; dizer que o sim não informa fecha a porta.
    let mut e = estado_base();
    e.meu_composto = -1;
    let t = texto(&e, Intencao::Pneu);
    assert!(
        t.contains("não informa o composto"),
        "não admitiu a ausência em:\n{t}"
    );
}

#[test]
fn sem_telemetria_o_dossie_diz_isso_e_nada_mais() {
    // Um dossiê com posição zero e volta zero faria o modelo dizer "você está em zero".
    let mut e = estado_base();
    e.conectado = false;
    let linhas = dossie(&e, Intencao::Posicao);
    assert_eq!(linhas.len(), 1, "vazou fato sem telemetria: {linhas:?}");
    assert!(linhas[0].contains("sem telemetria"));
}

#[test]
fn posicao_na_classe_so_aparece_quando_difere_da_geral() {
    // Em prova de classe única os dois números são o mesmo, e repetir um fato em outras
    // palavras faz o modelo tratá-lo como dois — e comentar os dois.
    let mut e = estado_base();
    assert!(!texto(&e, Intencao::Posicao).contains("na sua classe"));
    e.posicao_classe = 2;
    assert!(texto(&e, Intencao::Posicao).contains("2º na sua classe"));
}

#[test]
fn dossie_completo_une_os_blocos_sem_repetir_o_nucleo() {
    // O caminho de uma ida e volta: sem transcrição não há intenção, então vai tudo.
    // O núcleo é construído por todo bloco, e sem deduplicação a posição apareceria oito
    // vezes — um modelo lendo o mesmo fato oito vezes o trata como o assunto da conversa.
    let mut e = estado_base();
    e.frente = Some(vizinho("Rodrigues", 1.2));
    let linhas = crate::engenheiro::dossie_completo(&e);
    let t = linhas.join("\n");

    assert_eq!(
        t.matches("Posição: 5º de 24").count(),
        1,
        "núcleo repetido em:\n{t}"
    );
    // E os blocos de fato estão todos lá.
    assert!(t.contains("Combustível:"), "sem combustível em:\n{t}");
    assert!(t.contains("Última volta:"), "sem ritmo em:\n{t}");
    assert!(t.contains("Carro: sem reparo"), "sem carro em:\n{t}");
    assert!(t.contains("Incidentes:"), "sem disciplina em:\n{t}");
    assert!(t.contains("Pista:"), "sem pista em:\n{t}");
    assert_eq!(
        linhas.len(),
        linhas
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        "há linha duplicada em:\n{t}"
    );
}

#[test]
fn dossie_completo_sem_telemetria_continua_calado() {
    let mut e = estado_base();
    e.conectado = false;
    assert_eq!(crate::engenheiro::dossie_completo(&e).len(), 1);
}

#[test]
fn idade_do_pneu_vira_comparacao_e_nao_dois_numeros_soltos() {
    // "Ele está com 23 voltas" não decide nada sozinho. O que decide é a diferença: se o
    // pneu dele é mais velho, vale atacar agora; se é mais novo, vale segurar.
    let mut e = estado_base();
    e.meu_composto = 0;
    e.meu_pneu_voltas = 8;
    let mut v = vizinho("Rodrigues", 1.2);
    v.composto = 0;
    v.pneu_voltas = 23;
    e.frente = Some(v);
    let t = texto(&e, Intencao::Pneu);
    assert!(
        t.contains("15 voltas MAIS VELHO"),
        "não comparou as idades em:\n{t}"
    );

    // O outro lado: pneu dele mais novo é vantagem DELE, e a frase tem de inverter.
    let mut v = vizinho("Rodrigues", 1.2);
    v.composto = 0;
    v.pneu_voltas = 2;
    e.frente = Some(v);
    let t = texto(&e, Intencao::Pneu);
    assert!(
        t.contains("6 voltas MAIS NOVO"),
        "inverteu a vantagem em:\n{t}"
    );
}

#[test]
fn descasamento_pneu_pista_antecipa_a_parada_alheia() {
    // O fato mais acionável do bloco: saber que o da frente vai TER de parar muda a
    // decisão agora — atacar ou economizar e herdar a posição.
    let mut e = estado_base();
    e.pista_molhada = true;
    e.meu_composto = 1; // eu de chuva
    e.meu_pneu_voltas = 4;
    let mut v = vizinho("Rodrigues", 1.2);
    v.composto = 0; // ele ainda de seco
    v.pneu_voltas = 18;
    e.frente = Some(v);
    let t = texto(&e, Intencao::Pneu);
    assert!(
        t.contains("está de SECO com a pista molhada"),
        "não avisou o descasamento em:\n{t}"
    );
    assert!(
        t.contains("vai precisar parar"),
        "não tirou a conclusão em:\n{t}"
    );
    // E o meu lado não deve acusar descasamento nenhum — estou certo para a pista.
    assert!(
        !t.contains("VOCÊ está"),
        "acusou descasamento inexistente em:\n{t}"
    );
}

#[test]
fn descasamento_do_proprio_jogador_e_dito_por_ultimo() {
    let mut e = estado_base();
    e.pista_molhada = true;
    e.meu_composto = 0; // eu de seco na chuva
    e.meu_pneu_voltas = 12;
    let t = texto(&e, Intencao::Pneu);
    assert!(
        t.contains("VOCÊ está de seco com a pista molhada"),
        "não avisou o meu descasamento em:\n{t}"
    );
}

// ─── O terceiro eixo da medição do Scribe ────────────────────────────────────

/// A intenção sobrevive à transcrição?
///
/// Latência a gente contorna; intenção errada dispara a fala pré-gravada ERRADA, com a voz
/// confiante do engenheiro. Este teste fecha o laço: pega o que o Scribe realmente ouviu
/// (gravado por `scripts/scribe-poc/medir.mjs`) e passa pelo classificador de verdade.
///
/// A classificação mora em Rust e é ela que decide o caminho barato. Reimplementá-la no
/// script em JS só para conferir criaria uma segunda verdade, e a divergência entre as duas
/// apareceria como roteamento errado em corrida — não como teste vermelho.
///
/// **Pula quando o arquivo não existe.** A medição depende de chave da ElevenLabs e de cota
/// da Cloud TTS; travar a suíte inteira por isso puniria quem só quer rodar os testes.
#[test]
fn intencao_sobrevive_a_transcricao() {
    let caminho = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../docs/scribe-poc/transcricoes.json"
    );
    let Ok(bruto) = std::fs::read_to_string(caminho) else {
        println!("sem transcrições em {caminho} — rode scripts/scribe-poc/medir.mjs primeiro");
        return;
    };
    let itens: Vec<serde_json::Value> =
        serde_json::from_str(&bruto).expect("transcricoes.json malformado");
    assert!(!itens.is_empty(), "transcricoes.json vazio");

    let esperada = |nome: &str| match nome {
        "Posicao" => Intencao::Posicao,
        "Frente" => Intencao::Frente,
        "Atras" => Intencao::Atras,
        "Restante" => Intencao::Restante,
        "Combustivel" => Intencao::Combustivel,
        "Ritmo" => Intencao::Ritmo,
        "Carro" => Intencao::Carro,
        "Bandeira" => Intencao::Bandeira,
        "Pista" => Intencao::Pista,
        "Pneu" => Intencao::Pneu,
        "Geral" => Intencao::Geral,
        outro => panic!("intenção desconhecida no gabarito: {outro}"),
    };

    let mut erros = Vec::new();
    for item in &itens {
        let transcrito = item["transcrito"].as_str().unwrap_or_default();
        let alvo = esperada(item["intencao_esperada"].as_str().unwrap_or_default());
        let obtida = classificar(transcrito);
        let marca = if obtida == alvo { "✓" } else { "✗" };
        println!(
            "  {marca} {:<24} {:<6} {:?} → {:?}   \"{transcrito}\"",
            item["chave"].as_str().unwrap_or_default(),
            item["sujeira"].as_str().unwrap_or_default(),
            alvo,
            obtida
        );
        if obtida != alvo {
            erros.push(format!(
                "{} ({}): esperava {alvo:?}, veio {obtida:?} de \"{transcrito}\"",
                item["chave"].as_str().unwrap_or_default(),
                item["sujeira"].as_str().unwrap_or_default()
            ));
        }
    }
    println!(
        "\n  {} de {} classificadas certo",
        itens.len() - erros.len(),
        itens.len()
    );
    assert!(
        erros.is_empty(),
        "intenções erradas:\n  {}",
        erros.join("\n  ")
    );
}

/// Despeja dossiês de exemplo para a medição do Gemini.
///
/// Existe porque medir o modelo com um prompt inventado à mão mediria a coisa errada: o que
/// se quer saber é quanto custa **este** dossiê, com este tamanho e este formato. Copiar as
/// linhas para dentro de um script JS criaria uma segunda versão do formato, que envelheceria
/// no primeiro campo novo — e a medição passaria a descrever um dossiê que não existe mais.
///
/// Só entram os casos que o [renderizador](crate::engenheiro::renderizar) **recusa**, porque
/// são exatamente esses que chegam ao modelo em produção. Medir com um caso que o acervo
/// cobre seria medir uma chamada que nunca vai acontecer.
///
/// Não é bem um teste: é um gerador de material, e por isso não afirma quase nada — só que o
/// arquivo saiu com conteúdo. Roda com `cargo test --lib dumpa_dossies_para_medicao`.
#[test]
fn dumpa_dossies_para_medicao() {
    let mut casos: Vec<(String, EstadoAgora, Intencao)> = Vec::new();

    // Ritmo: depende da biblioteca de tempos de volta, que ainda não existe.
    // O `vizinho` de teste fixa o número 7; aqui os dois lados precisam de números distintos,
    // senão o dossiê mostra dois carros diferentes com o mesmo `#7` e o modelo é convidado a
    // achar que são o mesmo.
    let atras = |nome: &str, gap: f64| {
        let mut v = vizinho(nome, gap);
        v.idx = 12;
        v.numero = 12;
        v
    };

    let mut e = estado_base();
    e.frente = Some(vizinho("Rodrigues", 1.2));
    e.atras = Some(atras("Silva", 0.8));
    casos.push(("ritmo".into(), e.clone(), Intencao::Ritmo));

    // Carro: mistura reparo obrigatório com opcional, e não tem forma fixa.
    let mut e = estado_base();
    e.reparo_obrigatorio_s = 18.5;
    e.reparo_opcional_s = 32.0;
    e.reparos_rapidos_usados = 1;
    e.frente = Some(vizinho("Rodrigues", 3.4));
    casos.push(("carro".into(), e, Intencao::Carro));

    // Geral: a pergunta aberta, e o dossiê mais longo que o modelo vai ver.
    let mut e = estado_base();
    e.frente = Some(vizinho("Rodrigues", 1.2));
    e.atras = Some(atras("Silva", 0.8));
    e.bandeira = "Bandeira amarela".to_string();
    e.meu_composto = 0;
    e.meu_pneu_voltas = 8;
    casos.push(("geral".into(), e, Intencao::Geral));

    // Frente com o vizinho uma volta à parte: o acervo não distingue tráfego de disputa.
    let mut e = estado_base();
    let mut v = vizinho("Rodrigues", 1.2);
    v.volta_a_parte = true;
    e.frente = Some(v);
    casos.push(("frente_trafego".into(), e, Intencao::Frente));

    // Prova por tempo perto do fim, com combustível apertado — o caso em que a resposta
    // precisa cruzar dois fatos, que é onde o modelo ganha do template.
    let mut e = estado_base();
    e.voltas_restantes = 8;
    e.voltas_restantes_estimadas = true;
    e.tempo_restante_s = 740.0;
    e.autonomia_voltas = 6.4;
    e.saldo_combustivel_voltas = -1.6;
    e.frente = Some(vizinho("Rodrigues", 2.1));
    casos.push(("combustivel_apertado".into(), e, Intencao::Geral));

    // O material tem de espelhar o que o comando manda de verdade, inclusive a regra de a
    // pergunta ABERTA levar o dossiê inteiro. Medir com um dossiê mais curto que o real daria
    // um número otimista de latência e um julgamento de qualidade sobre um prompt que a
    // produção nunca vai emitir.
    let saida: Vec<serde_json::Value> = casos
        .iter()
        .map(|(nome, estado, intencao)| {
            let linhas = if *intencao == Intencao::Geral {
                crate::engenheiro::dossie_completo(estado)
            } else {
                dossie(estado, *intencao)
            };
            serde_json::json!({
                "caso": nome,
                "intencao": format!("{intencao:?}"),
                "linhas": linhas,
            })
        })
        .collect();

    let caminho = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../docs/scribe-poc/dossies.json"
    );
    std::fs::write(caminho, serde_json::to_string_pretty(&saida).unwrap())
        .expect("gravar dossies.json");
    for c in &saida {
        println!(
            "  {} ({} linhas)",
            c["caso"].as_str().unwrap(),
            c["linhas"].as_array().unwrap().len()
        );
    }
    assert!(saida
        .iter()
        .all(|c| !c["linhas"].as_array().unwrap().is_empty()));
    println!("\n  {caminho}");
}
