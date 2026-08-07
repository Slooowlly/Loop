//! De que a pergunta trata — por palavra-chave, não por modelo.
//!
//! A classificação é feita aqui, com uma tabela de termos, e não delegada a uma segunda
//! chamada de IA. Três razões, em ordem de peso:
//!
//! 1. **Latência.** O push-to-talk já paga três serviços em série. Uma quarta chamada só
//!    para descobrir do que se trata custaria mais que toda a economia que ela traria.
//! 2. **Determinismo.** "Quanto de combustível?" tem de cair em combustível todas as
//!    vezes. Um classificador que acerta 95% erra uma pergunta a cada vinte, e o piloto
//!    não tem como saber por que a resposta veio torta.
//! 3. **Custo.** É uma tabela de strings.
//!
//! ## Por que pontuação, e não a primeira palavra que casar
//!
//! Os termos se sobrepõem de verdade em português de automobilismo. "Volta" está em
//! quanto falta e em ritmo; "seco" está em pista e em pneu; "frente" aparece numa
//! pergunta sobre pneu ("o da frente está de que pneu?"). Casar o primeiro termo faria a
//! ordem da tabela decidir o sentido da frase.
//!
//! Então cada intenção acumula peso, e a maior ganha. Termos inequívocos (`combustivel`,
//! `bandeira`) pesam 3; termos que várias intenções compartilham pesam 1. Empate ou
//! nenhum ponto caem em [`Intencao::Geral`], que não é um erro — é a pergunta aberta
//! ("e aí, como estamos?"), que é comum no rádio e merece resposta.

/// Do que a pergunta trata. `Geral` é a pergunta aberta e o destino de tudo que não casou.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Intencao {
    Posicao,
    Frente,
    Atras,
    Restante,
    Combustivel,
    Ritmo,
    Carro,
    Bandeira,
    Pista,
    Pneu,
    /// A TABELA, não a pista. É a única intenção que não se responde com telemetria — a
    /// resposta vem do save, e por isso ela existe em vez de cair em `Geral`: uma pergunta
    /// sobre o campeonato respondida com o retrato da corrida é uma resposta sobre outro
    /// assunto, dita com convicção.
    Campeonato,
    Geral,
}

/// Ordem de desempate. Não é arbitrária: é a ordem em que a informação **muda o que o
/// piloto faz agora**. Um empate entre combustível e ritmo é resolvido pelo combustível,
/// porque um decide se ele para no box e o outro decide se ele fica satisfeito.
const PRIORIDADE: [Intencao; 11] = [
    Intencao::Bandeira,
    Intencao::Carro,
    Intencao::Combustivel,
    // O CAMPEONATO ganha de posição e de ritmo no empate, e por um motivo de vocabulário:
    // quem diz "campeonato" ou "tabela" não tem outro jeito de dizer isso, enquanto
    // "posição" e "ponto" aparecem nas duas perguntas. Quem nomeou a tabela quis a tabela.
    Intencao::Campeonato,
    // Pneu vem ANTES dos vizinhos de propósito. Uma pergunta de pneu quase sempre cita
    // alguém ("o da frente está de que pneu?"), enquanto uma pergunta sobre o vizinho
    // raramente diz "pneu" — então, no empate, quem mencionou pneu quis falar de pneu.
    Intencao::Pneu,
    Intencao::Frente,
    Intencao::Atras,
    Intencao::Restante,
    Intencao::Posicao,
    Intencao::Pista,
    Intencao::Ritmo,
];

/// `(intenção, termo normalizado, peso)`.
///
/// Os termos são **radicais**, casados por substring: `alcanc` cobre "alcanço",
/// "alcançar" e "alcança" sem três entradas. Isso também é o motivo de nenhum termo ser
/// curto demais — `pa` casaria "parada", "passar" e "pane" de uma vez.
const TERMOS: &[(Intencao, &str, u32)] = &[
    // ── Campeonato ──
    // "ponto" pesa 1, e não 3, porque ele também é o ponto do gap e o ponto de frenagem —
    // sozinho ele não decide nada. "Campeonato" e "tabela" decidem.
    (Intencao::Campeonato, "campeonato", 3),
    (Intencao::Campeonato, "tabela", 3),
    (Intencao::Campeonato, "classificacao geral", 3),
    (Intencao::Campeonato, "na temporada", 3),
    (Intencao::Campeonato, "titulo", 2),
    (Intencao::Campeonato, "pontos", 1),
    // ── Posição ──
    (Intencao::Posicao, "posicao", 3),
    (Intencao::Posicao, "colocad", 3),
    (Intencao::Posicao, "colocacao", 3),
    (Intencao::Posicao, "que lugar", 3),
    (Intencao::Posicao, "em que pe", 2),
    (Intencao::Posicao, "lugar", 1),
    // ── Carro da frente ──
    (Intencao::Frente, "da frente", 3),
    (Intencao::Frente, "na frente", 3),
    (Intencao::Frente, "alcanc", 3),
    (Intencao::Frente, "gap", 2),
    (Intencao::Frente, "distancia", 2),
    (Intencao::Frente, "adiante", 2),
    // Não existe um `frente` solto aqui, e a ausência é deliberada: `da frente` CONTÉM
    // `frente`, então os dois casariam na mesma pergunta e a intenção somaria 4 onde
    // deveria somar 3. Uma pergunta de pneu sobre o carro da frente perderia para o
    // vizinho por causa de um ponto fantasma. Termo que é prefixo de outro da mesma
    // intenção conta duas vezes — vale para toda a tabela.
    // ── Carro de trás ──
    (Intencao::Atras, "atras", 3),
    (Intencao::Atras, "colado", 3),
    (Intencao::Atras, "vem vindo", 3),
    (Intencao::Atras, "pressao", 2),
    (Intencao::Atras, "me pegar", 2),
    // ── Quanto falta ──
    (Intencao::Restante, "quanto falta", 3),
    (Intencao::Restante, "quantas volta", 3),
    (Intencao::Restante, "faltam", 3),
    (Intencao::Restante, "restam", 3),
    (Intencao::Restante, "termina", 2),
    (Intencao::Restante, "acaba", 2),
    // ── Combustível ──
    (Intencao::Combustivel, "combustivel", 3),
    (Intencao::Combustivel, "gasolina", 3),
    (Intencao::Combustivel, "tanque", 3),
    (Intencao::Combustivel, "abastec", 3),
    (Intencao::Combustivel, "da pra chegar", 3),
    // ── Ritmo ──
    (Intencao::Ritmo, "ritmo", 3),
    (Intencao::Ritmo, "meu tempo", 3),
    (Intencao::Ritmo, "melhor volta", 3),
    (Intencao::Ritmo, "andando", 2),
    (Intencao::Ritmo, "rapido", 1),
    (Intencao::Ritmo, "lento", 1),
    (Intencao::Ritmo, "tempo", 1),
    // ── Carro / dano ──
    (Intencao::Carro, "dano", 3),
    (Intencao::Carro, "danific", 3),
    (Intencao::Carro, "quebrad", 3),
    (Intencao::Carro, "aguenta", 3),
    (Intencao::Carro, "reparo", 3),
    (Intencao::Carro, "conserto", 3),
    (Intencao::Carro, "asa", 2),
    (Intencao::Carro, "suspensao", 2),
    (Intencao::Carro, "motor", 2),
    (Intencao::Carro, "cambio", 2),
    (Intencao::Carro, "batida", 2),
    (Intencao::Carro, "meu carro", 2),
    // ── Bandeira / disciplina ──
    (Intencao::Bandeira, "bandeira", 3),
    (Intencao::Bandeira, "amarela", 3),
    (Intencao::Bandeira, "punic", 3),
    (Intencao::Bandeira, "penalidade", 3),
    (Intencao::Bandeira, "incidente", 3),
    (Intencao::Bandeira, "preta", 2),
    (Intencao::Bandeira, "advertencia", 2),
    // ── Pista / clima ──
    (Intencao::Pista, "chuva", 3),
    (Intencao::Pista, "chov", 3),
    (Intencao::Pista, "molhad", 3),
    (Intencao::Pista, "clima", 3),
    (Intencao::Pista, "temperatura", 3),
    (Intencao::Pista, "a pista", 2),
    // ── Pneu ──
    (Intencao::Pneu, "pneu", 3),
    (Intencao::Pneu, "composto", 3),
    (Intencao::Pneu, "borracha", 2),
    (Intencao::Pneu, "slick", 2),
];

/// Minúsculas e sem acento. O Scribe transcreve com acentuação e pontuação corretas, e a
/// tabela seria ilegível se cada termo tivesse de casar `combustível` e `combustivel`.
///
/// A troca é feita char a char e não por crate de normalização Unicode de propósito: são
/// doze vogais e um cê-cedilha, o português não precisa de mais que isso, e uma
/// dependência a mais para isso não se paga.
pub fn normalizar(texto: &str) -> String {
    texto
        .chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            outro => outro,
        })
        .collect()
}

/// A intenção da pergunta. Nunca falha: o que não casa vira [`Intencao::Geral`].
pub fn classificar(transcricao: &str) -> Intencao {
    let texto = normalizar(transcricao);
    if texto.trim().is_empty() {
        return Intencao::Geral;
    }

    let mut placar: Vec<(Intencao, u32)> = PRIORIDADE.iter().map(|i| (*i, 0)).collect();
    for (intencao, termo, peso) in TERMOS {
        if texto.contains(termo) {
            if let Some((_, p)) = placar.iter_mut().find(|(i, _)| i == intencao) {
                *p += peso;
            }
        }
    }

    // `max_by_key` devolveria o ÚLTIMO máximo, e a ordem do vetor viraria o desempate por
    // acidente. Percorrer na ordem de prioridade com `>` estrito torna o critério
    // explícito: em caso de empate vence quem aparece primeiro em `PRIORIDADE`.
    let mut vencedora = Intencao::Geral;
    let mut melhor = 0;
    for (intencao, pontos) in &placar {
        if *pontos > melhor {
            melhor = *pontos;
            vencedora = *intencao;
        }
    }
    vencedora
}
