//! Mensagens de erro da camada de carreira, uma função por mensagem.
//!
//! Antes cada `Err`/`ok_or_else` carregava a prosa em português cru e sem acento, então
//! o jogador em en-US recebia texto em português na tela de erro. Aqui a mensagem passa
//! pelo `rust_i18n` (chaves em `locales/*.yml`, sob `career.error`), e a acentuação é
//! responsabilidade do arquivo de locale, não do código.
//!
//! São funções e não constantes porque o locale é resolvido na HORA da falha: o mesmo
//! processo pode ter trocado de idioma depois do boot.
//!
//! **O que NÃO passa por aqui, de propósito:** os ~250 `format!("Falha ao …: {e}")` dos
//! `map_err` de `career/*.rs`. Aqueles carregam o erro cru do rusqlite/serde grudado no
//! fim, então já são texto misto e servem ao diagnóstico, não ao jogador. Traduzir a
//! metade em português de uma frase que termina em `SqliteFailure(...)` não deixaria a
//! tela mais legível em en-US; o que resolve aquele caso é a mensagem de domínio que a
//! tela mostra no lugar, e é ela que mora aqui.

macro_rules! career_error {
    ($(#[$doc:meta])* $name:ident => $key:literal) => {
        $(#[$doc])*
        pub(crate) fn $name() -> String {
            rust_i18n::t!(concat!("career.error.", $key)).to_string()
        }
    };
}

career_error!(
    /// O diretório do save não existe.
    save_not_found => "save_not_found"
);
career_error!(
    /// O diretório existe mas o `career.db` não.
    db_not_found => "db_not_found"
);
career_error!(
    /// `career_id` fora do formato `career_NNN`.
    invalid_career_id => "invalid_career_id"
);
career_error!(
    /// Nenhuma temporada com status ativo no save.
    active_season_not_found => "active_season_not_found"
);
career_error!(
    /// A temporada existe mas nenhuma etapa foi disputada ainda.
    season_not_started => "season_not_started"
);
career_error!(
    /// Não há plano de pré-temporada gravado no save.
    preseason_plan_not_found => "preseason_plan_not_found"
);
career_error!(
    /// A pré-temporada ainda tem semanas por avançar.
    preseason_not_finished => "preseason_not_finished"
);
career_error!(
    /// A proposta citada não existe na temporada.
    proposal_not_found => "proposal_not_found"
);
career_error!(
    /// A proposta existe, mas é de outro piloto.
    proposal_not_player => "proposal_not_player"
);
career_error!(
    /// A proposta já foi aceita ou recusada.
    proposal_not_pending => "proposal_not_pending"
);
career_error!(
    /// A equipe referida pela proposta desapareceu do mundo.
    proposal_team_not_found => "proposal_team_not_found"
);
career_error!(
    /// Equipe ausente na validação da vaga escolhida.
    team_not_found_for_seat => "team_not_found_for_seat"
);
career_error!(
    /// Equipe ausente ao atualizar o plano de pré-temporada.
    team_not_found_for_plan => "team_not_found_for_plan"
);
career_error!(
    /// Equipe ausente ao normalizar os contratos regulares.
    team_not_found_for_contracts => "team_not_found_for_contracts"
);
career_error!(
    /// Equipe ausente ao encaixar o jogador numa vaga.
    team_not_found_for_placement => "team_not_found_for_placement"
);
career_error!(
    /// Equipe ausente ao recalcular a hierarquia.
    team_not_found_for_hierarchy => "team_not_found_for_hierarchy"
);
career_error!(
    /// Equipe ausente ao repor um piloto que saiu.
    team_not_found_for_replacement => "team_not_found_for_replacement"
);
career_error!(
    /// O gerador não devolveu nenhum rookie para a reposição de emergência.
    rookie_generation_failed => "rookie_generation_failed"
);
career_error!(
    /// O mundo foi gerado sem a equipe do jogador — estado impossível.
    player_team_missing_after_world => "player_team_missing_after_world"
);
career_error!(
    /// O jogador não está em nenhuma categoria (agente livre sem histórico).
    player_without_category => "player_without_category"
);
career_error!(
    /// Nome do piloto vazio.
    name_required => "name_required"
);
career_error!(
    /// Nome do piloto acima do limite de caracteres.
    name_too_long => "name_too_long"
);
career_error!(
    /// Nacionalidade fora da lista suportada.
    invalid_nationality => "invalid_nationality"
);
career_error!(
    /// Categoria de estreia diferente das duas rookie.
    invalid_starting_category => "invalid_starting_category"
);
career_error!(
    /// Índice de equipe fora da faixa da categoria de estreia.
    invalid_team_index => "invalid_team_index"
);
career_error!(
    /// Dificuldade fora da lista suportada.
    invalid_difficulty => "invalid_difficulty"
);
career_error!(
    /// Idade do piloto fora da faixa aceita.
    invalid_age => "invalid_age"
);

#[cfg(test)]
mod tests {
    use super::*;

    /// Toda mensagem resolve nos dois locales, com acento em PT e sem sobrar chave crua.
    /// `#[serial]` porque o locale do `rust_i18n` é global do processo.
    #[test]
    #[serial_test::serial]
    fn mensagens_resolvem_nos_dois_locales() {
        let todas: [fn() -> String; 27] = [
            save_not_found,
            db_not_found,
            invalid_career_id,
            active_season_not_found,
            season_not_started,
            preseason_plan_not_found,
            preseason_not_finished,
            proposal_not_found,
            proposal_not_player,
            proposal_not_pending,
            proposal_team_not_found,
            team_not_found_for_seat,
            team_not_found_for_plan,
            team_not_found_for_contracts,
            team_not_found_for_placement,
            team_not_found_for_hierarchy,
            team_not_found_for_replacement,
            rookie_generation_failed,
            player_team_missing_after_world,
            player_without_category,
            name_required,
            name_too_long,
            invalid_nationality,
            invalid_starting_category,
            invalid_team_index,
            invalid_difficulty,
            invalid_age,
        ];

        for locale in ["pt-BR", "en-US"] {
            rust_i18n::set_locale(locale);
            for mensagem in todas {
                let texto = mensagem();
                assert!(
                    !texto.is_empty() && !texto.starts_with("career.error."),
                    "mensagem sem tradução em {locale}: {texto}"
                );
            }
        }

        rust_i18n::set_locale("pt-BR");
        assert_eq!(save_not_found(), "Save não encontrado.");
        assert_eq!(
            active_season_not_found(),
            "Temporada ativa não encontrada."
        );
        rust_i18n::set_locale("en-US");
        assert_eq!(save_not_found(), "Save not found.");

        rust_i18n::set_locale("pt-BR"); // restaura
    }
}
