//! O AVISO SOBRE O NOSSO CARRO: a peça do jogador entrou na janela de risco.
//!
//! Irmã da [`quebra`](super::quebra) e o oposto dela em quase tudo, o que vale escrever porque
//! é a razão de serem dois módulos:
//!
//! | | quebra | aviso próprio |
//! |---|---|---|
//! | de quem fala | de um terceiro | de você |
//! | pessoa | 3ª ("Cooper está com…") | 2ª ("no seu motor") |
//! | montagem | por peças, porque nomeia alguém | frase INTEIRA, sempre |
//! | quantas por corrida | ~33 | no máximo 11 (uma por peça) |
//!
//! A montagem por peças não se justifica aqui. Ela existe lá porque o sujeito é um de 355
//! sobrenomes; aqui o sujeito é sempre "você", então a fala inteira cabe numa gravação só — e
//! a gravação inteira é sempre melhor que a colada, medido na POC. São 12 peças × 3 redações.
//!
//! ## A fala de POUPAR é o cruzamento de dois sinais
//!
//! O engenheiro só manda economizar carro quando as duas coisas valem ao mesmo tempo: uma peça
//! NOSSA na janela de risco E a corrida já derrubando gente. Separadas, nenhuma das duas
//! justifica o conselho — peça gasta sozinha é o desgaste normal de fim de stint, e grid
//! quebrando sozinho não é problema nosso. Juntas, dizem que a pista está cobrando mais caro
//! do que o previsto, e aí forçar menos é a decisão certa.
//!
//! Sai UMA vez por corrida, e por isso: ela pede uma mudança de pilotagem. Repetir vira
//! ruído, e ruído é o que faz o piloto desligar o rádio.

/// Prefixo das chaves desta família no catálogo do engenheiro.
pub const PREFIXO: &str = "meu_";

/// As 11 peças mais o genérico, na ordem do catálogo. Espelha
/// [`quebra::PECAS`](super::quebra::PECAS) — a mesma lista, outra pessoa gramatical.
pub const PECAS: [&str; 12] = super::quebra::PECAS;

/// Peça com preposição na 2ª pessoa ("no seu motor", "na sua suspensão").
pub fn part_com_seu(part_key: &str) -> &'static str {
    match part_key {
        "engine" => "no seu motor",
        "gearbox" => "no seu câmbio",
        "brakes" => "nos seus freios",
        "suspension" => "na sua suspensão",
        "cooling" => "no seu arrefecimento",
        "front_wing" => "na sua asa dianteira",
        "rear_wing" => "na sua asa traseira",
        "sidepods" => "nas suas laterais",
        "underbody" => "no seu assoalho",
        "chassis" => "no seu chassi",
        "electronics" => "na sua parte elétrica",
        _ => "no seu carro",
    }
}

/// As 3 redações do aviso, com a peça já embutida.
///
/// SEM número de propósito. O engenheiro não lê um sensor de desgaste — ele ouve, sente,
/// desconfia. Dizer "sua suspensão está em noventa e seis por cento" entregaria ao jogador uma
/// leitura que o personagem não tem, e transformaria uma decisão de pilotagem numa conta.
///
/// Frase única, sem pontuação no meio: MEDIDO no acervo, texto com travessão ou vírgula sai do
/// modelo com 0,35 s de silêncio dentro da fala, contra 0,01 s da oração corrida.
pub fn aviso_frase(part_key: &str, variante: usize) -> String {
    let onde = part_com_seu(part_key);
    match variante % 3 {
        0 => format!("Estou ouvindo algo estranho {onde}"),
        1 => format!("Não gostei de um barulho {onde}"),
        _ => format!("Tem algo esquisito acontecendo {onde}"),
    }
}

/// A chave do aviso no catálogo: `meu_engine_0`.
pub fn chave_aviso(part_key: &str, variante: usize) -> String {
    let peca = if PECAS.contains(&part_key) { part_key } else { "outra" };
    format!("{PREFIXO}{peca}_{}", variante % 3)
}

/// O DESFECHO: a peça não avisou mais, ela largou. A quebra do NOSSO carro, em 2ª pessoa.
///
/// Esta família nasceu de uma medição, e a medição achou um defeito de pessoa gramatical. O carro
/// do jogador entra no `breakdown_log` como qualquer outro, e o rádio da grade montava a fala
/// dele igual à dos outros — só que o nome do jogador não sai de pool nenhum, então ela caía na
/// forma pela equipe:
///
/// > *"O piloto um da Racing Academy Red foi retirado da corrida com problemas no motor."*
///
/// O piloto um da Racing Academy Red **é ele**. Acontecia 1,3 vez por corrida.
///
/// **A peça é nomeada, e isso custou 108 gravações.** A alternativa barata era falar só o
/// desfecho ("Acabou por hoje") em 9 peças, com o argumento de que o aviso anterior já tinha dito
/// qual peça era. Não vale: o aviso só sai se a peça cruzou a janela de risco, e quebra sem aviso
/// prévio existe — nesses casos o jogador ficaria sem saber o que largou, justo no momento em que
/// a corrida dele acabou.
///
/// Frase única, sem pontuação interna, pela mesma regra medida do resto do módulo.
pub fn desfecho_frase(part_key: &str, severidade: &str, variante: usize) -> String {
    let onde = part_com_seu(part_key);
    let redacoes: [&str; 3] = match severidade {
        // LEVE: perdeu alguma coisa e segue. O tom é de contrariedade, não de fim.
        "light" => [
            "Perdemos alguma coisa {} mas dá pra seguir",
            "Deu uma avaria {} e vamos andar assim mesmo",
            "Tem coisa solta {} mas o carro anda",
        ],
        // GRAVE: tem que entrar no box, e a instrução vem junto — é o que o piloto precisa fazer.
        "heavy" => [
            "Deu problema sério {} e vamos ter que parar",
            "Entra no box agora com essa quebra {}",
            "Temos avaria grave {} então entra no box",
        ],
        // ABANDONO: acabou. Sem instrução, porque não há o que fazer.
        _ => [
            "Acabou por hoje com problema {}",
            "Fim de prova com pane {}",
            "Não dá mais pra seguir com esse problema {}",
        ],
    };
    redacoes[variante % 3].replace("{}", onde)
}

/// A chave do desfecho no catálogo: `meu_dnf_engine_0`. A ordem (severidade, peça) espelha
/// [`quebra::chave_trecho`](super::quebra::chave_trecho) de propósito — são as duas caras da
/// mesma quebra, e ler as duas listas lado a lado tem que ser fácil.
pub fn chave_desfecho(part_key: &str, severidade: &str, variante: usize) -> String {
    let peca = if PECAS.contains(&part_key) { part_key } else { "outra" };
    let sev = if SEVERIDADES.contains(&severidade) { severidade } else { "dnf" };
    format!("{PREFIXO}{sev}_{peca}_{}", variante % 3)
}

/// As três severidades, na ordem do catálogo.
pub const SEVERIDADES: [&str; 3] = ["light", "heavy", "dnf"];

/// As 3 redações do conselho de POUPAR o carro. Ver o cabeçalho do módulo para quando sai.
///
/// São as ÚNICAS peças do acervo com duas frases, e de propósito: o conselho tem diagnóstico e
/// instrução, e separá-los em duas peças criaria uma emenda onde a pausa natural entre frases
/// já resolve. O gerador vai acusar silêncio interno nelas — aqui é o respiro certo, não o
/// defeito que a família de quebra combate.
pub fn poupar_frase(variante: usize) -> (&'static str, &'static str) {
    [
        (
            "meu_poupar",
            "A pista está comendo os carros hoje. Pega mais leve.",
        ),
        (
            "meu_poupar_2",
            "Muita gente quebrando. Force menos o carro.",
        ),
        (
            "meu_poupar_3",
            "Esse traçado destrói peça. Alivia pra chegar ao fim.",
        ),
    ][variante % 3]
}

/// TODA peça que esta família pode emitir, com o texto exato a gravar.
pub fn familia_peca_propria() -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = Vec::new();
    for peca in PECAS {
        for i in 0..3 {
            v.push((chave_aviso(peca, i), format!("{}.", aviso_frase(peca, i))));
        }
    }
    for sev in SEVERIDADES {
        for peca in PECAS {
            for i in 0..3 {
                v.push((
                    chave_desfecho(peca, sev, i),
                    format!("{}.", desfecho_frase(peca, sev, i)),
                ));
            }
        }
    }
    for i in 0..3 {
        let (k, t) = poupar_frase(i);
        v.push((k.to_string(), t.to_string()));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn o_catalogo_cobre_toda_peca_e_variacao() {
        let v = familia_peca_propria();
        assert_eq!(v.len(), 12 * 3 + 12 * 3 * 3 + 3);
        let chaves: HashSet<&String> = v.iter().map(|(k, _)| k).collect();
        assert_eq!(chaves.len(), v.len(), "chave repetida");
        for peca in PECAS {
            for i in 0..3 {
                assert!(chaves.contains(&chave_aviso(peca, i)), "falta {peca}/{i}");
            }
        }
    }

    #[test]
    fn o_desfecho_existe_para_toda_peca_e_severidade() {
        let chaves: HashSet<String> = familia_peca_propria().into_iter().map(|(k, _)| k).collect();
        for sev in SEVERIDADES {
            for peca in PECAS {
                for i in 0..3 {
                    let k = chave_desfecho(peca, sev, i);
                    assert!(chaves.contains(&k), "falta {k}");
                }
            }
        }
    }

    #[test]
    fn o_desfecho_fala_na_segunda_pessoa_e_nomeia_a_peca() {
        // O defeito que esta família existe para consertar: o carro do jogador contado como se
        // fosse de outro. Se alguma redação perder o "seu"/"sua", o defeito volta calado.
        for sev in SEVERIDADES {
            for peca in PECAS {
                for i in 0..3 {
                    let t = desfecho_frase(peca, sev, i);
                    assert!(
                        t.contains("seu") || t.contains("sua"),
                        "{peca}/{sev}/{i} saiu em 3ª pessoa: {t:?}",
                    );
                    assert!(
                        !t.contains(',') && !t.contains('—') && !t.contains(';'),
                        "{peca}/{sev}/{i} tem pausa interna: {t:?}",
                    );
                }
            }
        }
    }

    #[test]
    fn severidade_desconhecida_cai_no_abandono() {
        // Errar para o lado do abandono é o barato: dizer "acabou" numa quebra leve confunde uma
        // vez; dizer "dá pra seguir" num abandono manda o piloto insistir num carro morto.
        assert_eq!(chave_desfecho("engine", "sei_la", 0), "meu_dnf_engine_0");
    }

    #[test]
    fn peca_desconhecida_cai_na_chave_generica() {
        // O defeito silencioso: manter a chave da peça inexistente pediria `meu_turbo_0.wav`,
        // que ninguém gravou — e `falarPecas` pula peça ausente sem erro nenhum.
        assert_eq!(chave_aviso("turbo", 0), "meu_outra_0");
        assert!(aviso_frase("turbo", 0).ends_with("no seu carro"));
    }

    #[test]
    fn o_aviso_nunca_diz_o_numero_do_desgaste() {
        // O engenheiro OUVE, não lê sensor. Um número aqui entregaria ao jogador uma leitura
        // que o personagem não tem — e trocaria uma decisão de pilotagem por uma conta.
        for (chave, texto) in familia_peca_propria() {
            assert!(
                !texto.chars().any(|c| c.is_ascii_digit()),
                "{chave} tem número: {texto:?}",
            );
        }
    }

    #[test]
    fn o_aviso_de_peca_nao_tem_pausa_interna() {
        // Estas são frases INTEIRAS, tocadas sozinhas — não há emenda para a pausa somar. Mas a
        // regra vale igual: 0,35 s de silêncio no meio de uma fala de rádio soa como falha de
        // transmissão, colada ou não.
        for peca in PECAS {
            for i in 0..3 {
                let t = aviso_frase(peca, i);
                assert!(
                    !t.contains(',') && !t.contains('—') && !t.contains(';'),
                    "{peca}/{i}: {t:?}",
                );
            }
        }
    }

    #[test]
    fn a_pessoa_gramatical_e_a_segunda() {
        // Trocar de pessoa aqui é o defeito que o jogador percebe primeiro: o aviso sobre o
        // carro DELE contado em terceira pessoa soa como se fosse de outro carro.
        for peca in PECAS {
            assert!(
                part_com_seu(peca).contains("seu") || part_com_seu(peca).contains("sua"),
                "{peca} não está na 2ª pessoa",
            );
        }
    }
}
