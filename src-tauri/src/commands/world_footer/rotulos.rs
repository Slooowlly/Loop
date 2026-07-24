//! Rótulos e formatações da revista: tags, substantivos de recorde, ordinais e tempos.

/// Substantivo de uma métrica de recorde, no singular/plural conforme `count`
/// (i18n `world_footer.metric_noun.<id>.{one|other}`).
pub(super) fn metric_noun(id: &str, count: i32) -> String {
    let form = if count == 1 { "one" } else { "other" };
    let key = format!("world_footer.metric_noun.{id}.{form}");
    rust_i18n::t!(&key).to_string()
}

/// Mapeia a métrica persistida para o id de substantivo do recorde.
pub(super) fn metric_noun_id(metric: &str) -> &'static str {
    match metric {
        "wins" => "wins",
        "podiums" => "podiums",
        "poles" => "poles",
        "titles" => "titles",
        _ => "starts",
    }
}

/// Rótulo temático da revista (i18n `world_footer.tag.<id>`).
pub(super) fn tag_label(id: &str) -> String {
    let key = format!("world_footer.tag.{id}");
    rust_i18n::t!(&key).to_string()
}

/// Ordinal formatado no locale ativo. PT é gendered ("2º"/"2ª"); EN é "2nd".
/// Só cobre pt/en por ora (record news); estender ao adicionar locales.
pub(super) fn ord_label(n: i32, feminine: bool) -> String {
    let loc = rust_i18n::locale();
    if loc.starts_with("en") {
        let suffix = match (n % 100, n % 10) {
            (11..=13, _) => "th",
            (_, 1) => "st",
            (_, 2) => "nd",
            (_, 3) => "rd",
            _ => "th",
        };
        format!("{n}{suffix}")
    } else {
        format!("{n}{}", if feminine { "ª" } else { "º" })
    }
}

/// Formata um tempo de volta em ms para "m:ss.mmm" (ou "ss.mmm" abaixo de 1 min).
pub(super) fn format_lap_ms(ms: i32) -> String {
    let total = ms.max(0);
    let minutes = total / 60_000;
    let seconds = (total % 60_000) / 1_000;
    let millis = total % 1_000;
    if minutes > 0 {
        format!("{minutes}:{seconds:02}.{millis:03}")
    } else {
        format!("{seconds}.{millis:03}")
    }
}
