//! Formatação de número para EXIBIÇÃO, resolvida no locale ativo.
//!
//! O separador decimal é o ponto em en-US e a vírgula em pt-BR. Enquanto cada tela
//! formatava por conta própria, o resultado era um `format!("{:.1}")` cru (ponto em
//! toda parte, inclusive na tela em português) ou um `.replace('.', ",")` fixo
//! (vírgula em toda parte, inclusive na tela em inglês). Os dois estão errados na
//! metade dos casos, e nenhum falha em voz alta.
//!
//! A regra é a mesma de `world_footer::rotulos::ord_label`: quem escolhe a forma é o
//! locale ATIVO, no instante de montar o texto. Nada disso pode ser persistido — o
//! save guarda número, e a string localizada nasce e morre na borda.

/// O separador decimal do locale ativo.
///
/// Só distingue "en" do resto porque são os dois locales do jogo, e o resto do mundo
/// que o Loop cobre (pt-BR) usa vírgula. Estender ao adicionar locale novo.
fn separador_decimal() -> char {
    if rust_i18n::locale().starts_with("en") {
        '.'
    } else {
        ','
    }
}

/// Um decimal com `decimals` casas, no separador do locale ativo.
///
/// `formatar_decimal(2.34, 1)` devolve "2.3" em en-US e "2,3" em pt-BR.
pub fn formatar_decimal(value: f64, decimals: usize) -> String {
    let bruto = format!("{value:.decimals$}");
    match separador_decimal() {
        '.' => bruto,
        outro => bruto.replace('.', &outro.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `#[serial]`: o locale é estado global do processo, e um teste que troca de
    /// idioma contamina qualquer outro que asseverar prosa em PT.
    #[test]
    #[serial_test::serial]
    fn o_separador_decimal_segue_o_locale_ativo() {
        let anterior = rust_i18n::locale().to_string();

        rust_i18n::set_locale("pt-BR");
        assert_eq!(formatar_decimal(2.34, 1), "2,3");
        assert_eq!(formatar_decimal(12.0, 2), "12,00");
        assert_eq!(formatar_decimal(-4.28, 1), "-4,3");

        rust_i18n::set_locale("en-US");
        assert_eq!(formatar_decimal(2.34, 1), "2.3");
        assert_eq!(formatar_decimal(12.0, 2), "12.00");
        assert_eq!(formatar_decimal(-4.28, 1), "-4.3");

        // Zero casa não deixa separador nenhum para trocar, nos dois locales.
        rust_i18n::set_locale("pt-BR");
        assert_eq!(formatar_decimal(9.6, 0), "10");

        rust_i18n::set_locale(&anterior);
    }
}
