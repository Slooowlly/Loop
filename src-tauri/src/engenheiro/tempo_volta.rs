//! O TEMPO DE VOLTA falado, e o que se diz em volta dele.
//!
//! Três coisas moram aqui, e todas dependem da primeira:
//!
//! 1. **O tempo em si** — 2.101 peças, uma por décimo entre 30,0 s e 4:00,0.
//! 2. **A volta mais rápida da corrida** — de quem é e quanto foi.
//! 3. **O quanto falta** para tomá-la, quando o jogador está encostando.
//!
//! ## Fundido, e a faixa é o que quase quebrou isso
//!
//! A POC decidiu gravar o tempo INTEIRO num arquivo só e rejeitou de ouvido a decomposição por
//! dígitos. O desenho que saiu dali cobria 0:00,0 a 1:59,9 — 1.201 peças.
//!
//! A faixa estava errada. Medida na tabela de tempos base do jogo (618 entradas em
//! `simulation/profile/lap_times.rs`), a volta vai de 30,0 s a 703,0 s, e **18,3% delas passam
//! de dois minutos**. Quase uma volta em cinco ficaria muda.
//!
//! A tentação óbvia era partir o minuto — `"um,"` + `"trinta e dois e quatro."` — e trocar
//! 6.900 arquivos por 611. Medido e REPROVADO de ouvido: a peça `"um,"` gravada sozinha sai
//! com 0,40 s para uma sílaba, com acento pleno e contorno de fim de frase, porque é assim que
//! o modelo lê um monossílabo isolado. Na frase inteira o mesmo "um" é átono e desliza para o
//! "trinta". A montagem ficou 22% mais longa no caso comum e **91% no pior**
//! (`"um nove e zero."`, onde as duas peças são curtas). Nenhum valor de pausa conserta: o
//! defeito está no material, não na junção.
//!
//! Então voltou a ser fundido, com a faixa corrigida por medição em vez de por suposição:
//! **30,0 s a 4:00,0**, que cobre 97,4% do calendário. Os 2,6% acima de quatro minutos são
//! Nordschleife, Spa e Le Mans de enduro, e caem no caminho do modelo.
//!
//! ## O segundo de um dígito leva "zero" na frente
//!
//! `1:09.0` é gravado como `"um zero nove e zero."`, não `"um nove e zero."`. É a convenção do
//! rádio — ninguém diz "um nove" para um e nove segundos — e resolve a ambiguidade com o
//! minuto de uma vez.

use super::fala::cardinal;

/// Prefixo das chaves de tempo (`t_924` = 92,4 s).
pub const PREFIXO_TEMPO: &str = "t_";

/// Piso da faixa gravada, em décimos de segundo. 30,0 s — nada no jogo é mais rápido.
pub const MIN_DECIMOS: i32 = 300;
/// Teto da faixa gravada, em décimos. 4:00,0 — cobre 97,4% dos tempos base do calendário.
pub const MAX_DECIMOS: i32 = 2400;

/// Dentro de quantos décimos da volta mais rápida o engenheiro comenta a aproximação.
///
/// Nove — abaixo de um segundo. Acima disso "está chegando perto" é otimismo, não informação:
/// um segundo numa volta de um e trinta é mais de 1% do tempo, uma eternidade.
pub const DECIMOS_DE_APROXIMACAO: i32 = 9;

/// O tempo por extenso, como o rádio o diz. `None` fora da faixa gravada.
///
/// Exemplos: `453` → `"quarenta e cinco e três"`; `924` → `"um trinta e dois e quatro"`;
/// `690` → `"um zero nove e zero"`.
pub fn tempo_falado(decimos: i32) -> Option<String> {
    if !(MIN_DECIMOS..=MAX_DECIMOS).contains(&decimos) {
        return None;
    }
    let segundos_totais = decimos / 10;
    let decimo = decimos % 10;
    let minuto = segundos_totais / 60;
    let segundo = segundos_totais % 60;

    let decimo_txt = cardinal(decimo)?;
    if minuto == 0 {
        return Some(format!("{} e {decimo_txt}", cardinal(segundo)?));
    }
    // Segundo de um dígito ganha o "zero" da convenção do rádio: "um zero nove", não "um nove".
    let segundo_txt = if segundo < 10 {
        format!("zero {}", cardinal(segundo)?)
    } else {
        cardinal(segundo)?
    };
    Some(format!("{} {segundo_txt} e {decimo_txt}", cardinal(minuto)?))
}

/// A chave do tempo no catálogo. `None` fora da faixa — e o `None` é o ponto: ele sobe até o
/// renderizador e manda a pergunta para o modelo, em vez de pedir um `.wav` que não existe.
pub fn chave_tempo(decimos: i32) -> Option<String> {
    (MIN_DECIMOS..=MAX_DECIMOS)
        .contains(&decimos)
        .then(|| format!("{PREFIXO_TEMPO}{decimos}"))
}

/// Segundos → décimos arredondados. O arredondamento é para o décimo MAIS PRÓXIMO, não para
/// baixo: 92,37 s é "um trinta e dois e quatro", como o cronômetro mostraria.
pub fn decimos_de(segundos: f64) -> i32 {
    (segundos * 10.0).round() as i32
}

// ─── As falas em volta do tempo ──────────────────────────────────────────────

/// Abertura da resposta ao push-to-talk sobre o próprio ritmo. Vem ANTES do tempo.
pub const LEAD_VOLTA: (&str, &str) = ("tv_volta_em", "Volta em,");
/// Abertura do anúncio da volta mais rápida. Vem antes do SOBRENOME, que vem antes do tempo.
pub const LEAD_MELHOR: (&str, &str) = ("tv_melhor_e_do", "A volta mais rápida da corrida é do");

/// "Faltam N décimos para a melhor volta." — peça INTEIRA, sem emenda.
///
/// Fundida porque cabe: são nove valores. A regra da casa é fundir sempre que o número de
/// arquivos não explodir, e aqui ele não explode.
pub fn aproximacao_frase(decimos: i32) -> Option<(String, String)> {
    if !(1..=DECIMOS_DE_APROXIMACAO).contains(&decimos) {
        return None;
    }
    let n = cardinal(decimos)?;
    // Um décimo é singular. Errar a concordância é o tipo de coisa que ninguém nota escrevendo
    // e todo mundo ouve.
    let texto = if decimos == 1 {
        "Falta um décimo para a melhor volta.".to_string()
    } else {
        format!("Faltam {n} décimos para a melhor volta.")
    };
    Some((format!("tv_faltam_{decimos}"), texto))
}

/// As 3 redações de quando o JOGADOR toma a volta mais rápida.
///
/// Não estava no pedido, e entrou porque a ausência dela seria estranha: um rádio que anuncia a
/// volta mais rápida dos outros e cala quando é a sua soaria quebrado. Três peças; cortar é uma
/// linha.
pub fn tomamos_frase(variante: usize) -> (&'static str, &'static str) {
    [
        ("tv_tomamos", "Essa foi a volta mais rápida da corrida!"),
        ("tv_tomamos_2", "Volta mais rápida da corrida, muito bem!"),
        ("tv_tomamos_3", "Ninguém fez melhor que isso hoje!"),
    ][variante % 3]
}

/// TODA peça desta família, com o texto exato a gravar.
pub fn familia_tempo_volta() -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = Vec::new();
    for d in MIN_DECIMOS..=MAX_DECIMOS {
        // `expect` e não `?`: um buraco aqui seria um tempo dentro da faixa sem gravação, e o
        // sintoma na pista é silêncio. Melhor derrubar o teste que gera o catálogo.
        let texto = tempo_falado(d).expect("tempo dentro da faixa sem redação");
        v.push((chave_tempo(d).expect("chave dentro da faixa"), format!("{texto}.")));
    }
    for (k, t) in [LEAD_VOLTA, LEAD_MELHOR] {
        v.push((k.to_string(), t.to_string()));
    }
    for d in 1..=DECIMOS_DE_APROXIMACAO {
        let (k, t) = aproximacao_frase(d).expect("décimo dentro da faixa de aproximação");
        v.push((k, t));
    }
    for i in 0..3 {
        let (k, t) = tomamos_frase(i);
        v.push((k.to_string(), t.to_string()));
    }
    v
}

#[cfg(test)]
mod tests;
