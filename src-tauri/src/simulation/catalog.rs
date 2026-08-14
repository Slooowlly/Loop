//! **Catálogo de incidentes** — o texto que dá cara ao que quebrou no carro.
//!
//! O banco guarda CHAVE, não frase: `dnf_key`, `non_dnf_key` e `description_key` carregam
//! `breakdown.<id>.{dnf|warn|part}`, e o texto sai de `locales/*.yml` no locale ATIVO, na
//! hora de apresentar. A prosa em português saiu do schema na migração v65
//! (`db::migrations::incident_catalog_chaves`).
//!
//! Chave que não resolve não vira frase inventada. Ela cai, nesta ordem, no texto que a v65
//! preservou em `incident_catalog_texto_legado` — que só existe para id fora dos 54 — e, na
//! falta dele, na própria chave, que é feia de propósito: é sinal de linha faltando no
//! locale, e a guarda `todo_id_do_catalogo_tem_texto_nos_dois_locales` existe para isso
//! nunca chegar ao jogador.
//!
//! ## O texto CONGELA no idioma em que a corrida foi disputada — e isso é decisão
//!
//! O render acontece uma vez, na hora do incidente, e o resultado vai para o save junto com a
//! corrida. Trocar o idioma do jogo depois **não retraduz o histórico**: as corridas antigas
//! seguem no idioma em que foram corridas, as novas saem no idioma novo.
//!
//! Isto é a **opção A**, a mesma já adotada nas lesões, e está escrito aqui porque sem o
//! registro o próximo leitor trata como bug. As três razões, na ordem em que pesam:
//!
//! 1. **O save guarda prosa, não chave.** O incidente é gravado como texto renderizado, com o
//!    nome do piloto já substituído. Retraduzir exigiria guardar `(id, parâmetros)` em vez da
//!    frase e recompor tudo na leitura — mudança de contrato de save, não de texto.
//! 2. **O histórico é um registro do que aconteceu.** Uma manchete de 2029 lida em 2031 é um
//!    documento da temporada de 2029; retraduzi-la seria reescrever o passado, não exibi-lo.
//! 3. **O id sobrevive.** `SelectedEntry::catalog_id` vai junto no save, então **a porta não
//!    está fechada**: quem um dia quiser retraduzir o histórico tem por onde — a decisão é de
//!    não fazer agora, não de impedir.
//!
//! ## Paridade — conferida em 11/08/2026
//!
//! Os 54 ids semeados por `db::migrations::seed_incidentes` existem nos DOIS locales, sem sobra
//! de nenhum lado, e com exatamente os sufixos que o `severity_context` de cada um exige. Antes
//! da v65 um id faltando no `en-US` caía calado no `dnf_template` do banco, que estava em
//! português, e o jogador em inglês lia português sem nada indicar falha. Hoje o mesmo buraco
//! aparece como a chave crua na tela — barulhento, e ainda assim tarde demais. É a guarda
//! `todo_id_do_catalogo_tem_texto_nos_dois_locales` que impede os dois.

#![allow(dead_code)]

use std::collections::HashMap;

use rand::Rng;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::db::connection::DbError;

// ── Enums ─────────────────────────────────────────────────────────────────────

/// Classe de veículo — determina quais entries do catálogo são elegíveis.
///
/// [`VehicleClass::Unknown`] é a **ausência declarada**: a categoria não tem classe única
/// (multiclasse) ou não é conhecida. Nenhuma linha do `incident_catalog` carrega esse valor,
/// então filtrar por ele não devolve candidato e o incidente sai com o texto genérico do
/// motor. É de propósito: flavor text errado mente sobre o carro, e não ter flavor text não.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VehicleClass {
    StreetBased,
    RaceSpec,
    Prototype,
    Unknown,
}

/// Filtro de formato de corrida no catálogo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RaceFormatFilter {
    Sprint,
    Endurance,
    Both,
}

/// Fonte do incidente no catálogo — ortogonal ao IncidentType do motor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncidentSource {
    Mechanical,
    DriverError,
    PostCollision,
    Operational,
}

/// Quando a entry é elegível para seleção.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerType {
    /// Roll normal do motor (mecânico espontâneo, erro de pilotagem).
    Spontaneous,
    /// Só após colisão Minor/Major não-DNF.
    PostCollision,
    /// Só após DriverError Minor (rodada) — agravamento para stall.
    PostSpinStall,
}

/// Se a entry se aplica a DNF, non-DNF ou ambos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeverityContext {
    DnfOnly,
    NonDnfOnly,
    Both,
}

// ── Structs ───────────────────────────────────────────────────────────────────

/// Prosa que a v65 preservou para um id que não está entre os 54 do seed.
///
/// Existe para não inventar tradução de texto que ninguém escreveu: o que o save já mostrava
/// continua sendo mostrado, exatamente como estava. Para as 54 entradas do catálogo isto é
/// sempre `None` — elas têm linha nos dois locales.
#[derive(Debug, Clone, Default)]
pub struct TextoLegado {
    pub dnf: Option<String>,
    pub non_dnf: Option<String>,
    pub description: Option<String>,
}

pub struct CatalogEntry {
    pub id: String,
    pub vehicle_class: VehicleClass,
    pub race_format: RaceFormatFilter,
    pub incident_source: IncidentSource,
    pub trigger_type: TriggerType,
    pub severity_context: SeverityContext,
    pub weight_sprint: u32,
    pub weight_endurance: u32,
    /// Chave i18n do texto de DNF. Vazia quando a entrada é `NonDnfOnly`.
    pub dnf_key: String,
    /// Chave i18n do texto de não-DNF. `None` quando a entrada é `DnfOnly`.
    pub non_dnf_key: Option<String>,
    /// Chave i18n da descrição curta — a peça que quebrou.
    pub description_key: String,
    pub legado: Option<TextoLegado>,
}

pub struct SelectedEntry {
    pub catalog_id: String,
    pub rendered_text: String,
    pub description_short: String,
}

pub struct IncidentCatalog {
    entries: Vec<CatalogEntry>,
}

// ── Parsing helpers ───────────────────────────────────────────────────────────
//
// Os cinco RECUSAM valor desconhecido, em vez de cair num padrão. O braço `_ =>` que estava
// aqui até 12/08/2026 transformava erro de digitação no banco em comportamento: uma linha com
// `vehicle_class = 'RaceSpecc'` virava `StreetBased` e passava a concorrer com as entradas de
// carro de rua, mudando a distribuição do sorteio sem nada acusar. Agora a linha malformada
// derruba o carregamento com o id e a coluna no texto do erro.

fn campo_invalido(id: &str, coluna: &str, valor: &str) -> DbError {
    DbError::InvalidData(format!(
        "incident_catalog: linha '{id}' tem {coluna} inválido: '{valor}'"
    ))
}

fn parse_vehicle_class(id: &str, s: &str) -> Result<VehicleClass, DbError> {
    match s {
        "StreetBased" => Ok(VehicleClass::StreetBased),
        "RaceSpec" => Ok(VehicleClass::RaceSpec),
        "Prototype" => Ok(VehicleClass::Prototype),
        // `Unknown` é a ausência resolvida em tempo de execução, não um valor de catálogo.
        _ => Err(campo_invalido(id, "vehicle_class", s)),
    }
}

fn parse_race_format(id: &str, s: &str) -> Result<RaceFormatFilter, DbError> {
    match s {
        "Sprint" => Ok(RaceFormatFilter::Sprint),
        "Endurance" => Ok(RaceFormatFilter::Endurance),
        "Both" => Ok(RaceFormatFilter::Both),
        _ => Err(campo_invalido(id, "race_format", s)),
    }
}

fn parse_incident_source(id: &str, s: &str) -> Result<IncidentSource, DbError> {
    match s {
        "Mechanical" => Ok(IncidentSource::Mechanical),
        "DriverError" => Ok(IncidentSource::DriverError),
        "PostCollision" => Ok(IncidentSource::PostCollision),
        "Operational" => Ok(IncidentSource::Operational),
        _ => Err(campo_invalido(id, "incident_source", s)),
    }
}

fn parse_trigger_type(id: &str, s: &str) -> Result<TriggerType, DbError> {
    match s {
        "Spontaneous" => Ok(TriggerType::Spontaneous),
        "PostCollision" => Ok(TriggerType::PostCollision),
        "PostSpinStall" => Ok(TriggerType::PostSpinStall),
        _ => Err(campo_invalido(id, "trigger_type", s)),
    }
}

/// Peso do sorteio. `0` é legítimo (a entrada fica fora daquele formato); negativo não é, e o
/// `as u32` cru que estava aqui virava um peso astronômico que engolia o sorteio inteiro.
fn parse_peso(id: &str, coluna: &str, valor: i64) -> Result<u32, DbError> {
    u32::try_from(valor).map_err(|_| campo_invalido(id, coluna, &valor.to_string()))
}

fn parse_severity_context(id: &str, s: &str) -> Result<SeverityContext, DbError> {
    match s {
        "Both" => Ok(SeverityContext::Both),
        "DnfOnly" => Ok(SeverityContext::DnfOnly),
        "NonDnfOnly" => Ok(SeverityContext::NonDnfOnly),
        _ => Err(campo_invalido(id, "severity_context", s)),
    }
}

// ── Filter helpers ────────────────────────────────────────────────────────────

fn format_matches(filter: RaceFormatFilter, is_endurance: bool) -> bool {
    match filter {
        RaceFormatFilter::Both => true,
        RaceFormatFilter::Sprint => !is_endurance,
        RaceFormatFilter::Endurance => is_endurance,
    }
}

fn severity_matches(ctx: SeverityContext, is_dnf: bool) -> bool {
    match ctx {
        SeverityContext::Both => true,
        SeverityContext::DnfOnly => is_dnf,
        SeverityContext::NonDnfOnly => !is_dnf,
    }
}

// ── vehicle_class_from_category ───────────────────────────────────────────────

/// Resolve a classe de veículo a partir do `category_id`.
///
/// Aceita a forma **com classe** (`"endurance:gt3"`), a mesma convenção de
/// [`crate::models::team::Team::categoria_com_classe`]: quando a classe vem junto, é ela que
/// decide, e o Endurance resolve corretamente carro por carro.
///
/// Sem classe, uma categoria multiclasse só resolve quando as classes dela **concordam**:
/// `production_challenger` é mazda/toyota/bmw, três carros de rua, então `StreetBased` ali
/// não é escolha arbitrária. `endurance` é gt4/gt3/lmp2 — `RaceSpec` e `Prototype` no mesmo
/// grid — e não tem resposta única: devolve [`VehicleClass::Unknown`].
///
/// Até 12/08/2026 o braço final era `_ => StreetBased`. Com ele, `lmp2` (protótipo) e
/// `endurance` (multiclasse) recebiam flavor text de carro de rua, e a mentira não tinha
/// sintoma: o jogador lia sobre o câmbio do MX-5 num protótipo. `Unknown` não casa com
/// nenhuma linha do catálogo, então o incidente sai com o texto genérico do motor.
pub fn vehicle_class_from_category(category_id: &str) -> VehicleClass {
    // Sufixo vazio (`"gt3:"`) é a categoria crua: o `:` cai fora, e não vira classe.
    let (base, classe) = match category_id.split_once(':') {
        Some((base, classe)) => (base, Some(classe).filter(|c| !c.is_empty())),
        None => (category_id, None),
    };

    if let Some(classe) = classe {
        return classe_de_carro_unico(classe);
    }
    match classe_de_carro_unico(base) {
        VehicleClass::Unknown => classe_unanime_da_multiclasse(base),
        classe => classe,
    }
}

/// A classe de uma categoria de carro ÚNICO — o `match` por identidade, sem braço de padrão.
/// Multiclasse cai em `Unknown` aqui de propósito: quem as resolve é
/// [`classe_unanime_da_multiclasse`].
fn classe_de_carro_unico(category_id: &str) -> VehicleClass {
    match category_id {
        "mazda_rookie" | "toyota_rookie" | "mazda_amador" | "toyota_amador" | "bmw_m2" => {
            VehicleClass::StreetBased
        }
        "gt4" | "gt3" => VehicleClass::RaceSpec,
        "lmp2" => VehicleClass::Prototype,
        _ => VehicleClass::Unknown,
    }
}

/// A classe de uma categoria MULTICLASSE, e só quando todas as classes dela concordam.
///
/// Lê `constants::categories` em vez de repetir a lista: classe nova numa categoria
/// multiclasse entra por lá e chega aqui sozinha. Qualquer divergência, ou uma classe que o
/// `match` acima não conhece, devolve `Unknown` — a recusa é o resultado correto.
fn classe_unanime_da_multiclasse(category_id: &str) -> VehicleClass {
    let Some(config) = crate::constants::categories::get_category(category_id) else {
        return VehicleClass::Unknown;
    };
    let mut unanime: Option<VehicleClass> = None;
    for classe in config.classes {
        let atual = classe_de_carro_unico(classe.car_categoria);
        if atual == VehicleClass::Unknown {
            return VehicleClass::Unknown;
        }
        match unanime {
            Some(anterior) if anterior != atual => return VehicleClass::Unknown,
            _ => unanime = Some(atual),
        }
    }
    unanime.unwrap_or(VehicleClass::Unknown)
}

// ── IncidentCatalog ───────────────────────────────────────────────────────────

impl IncidentCatalog {
    /// Carrega todas as entries da tabela `incident_catalog`.
    ///
    /// ## `ORDER BY id` é contrato, não estilo
    ///
    /// A seleção ponderada de [`Self::select_and_render`] caminha os candidatos somando peso
    /// até estourar o sorteio, então **a ordem da lista decide qual entrada sai** para um dado
    /// número do RNG. Sem `ORDER BY`, quem decidia era a ordem física das linhas no SQLite —
    /// que é ordem de `rowid`, muda com `VACUUM`, com re-seed e com a ordem em que a migração
    /// inseriu. Dois saves com o MESMO catálogo lógico e a MESMA semente podiam sortear
    /// incidentes diferentes. O `id` é a chave estável, e é por ele que se ordena.
    ///
    /// **Isto quebra a reprodutibilidade de sequências semeadas antigas** (12/08/2026): uma
    /// corrida re-simulada com a semente de antes pode escolher outra entrada do catálogo. Os
    /// pesos e as probabilidades não mudaram — só a ordem em que os candidatos são percorridos.
    /// Nada foi ajustado para compensar, de propósito.
    ///
    /// Linha malformada **derruba o carregamento** com o id e a coluna no erro. Ver o bloco de
    /// parsers.
    pub fn load(conn: &Connection) -> Result<Self, DbError> {
        let legado = carregar_texto_legado(conn)?;

        let mut stmt = conn.prepare(
            "SELECT id, vehicle_class, race_format, incident_source, trigger_type,
                    severity_context, weight_sprint, weight_endurance,
                    dnf_key, non_dnf_key, description_key
             FROM incident_catalog
             ORDER BY id",
        )?;

        // Uma linha crua por vez: o `query_map` só sabe devolver `rusqlite::Error`, e o erro de
        // campo inválido é nosso. As strings saem daqui e viram enum logo abaixo, com `?`.
        type LinhaCrua = (
            String,
            String,
            String,
            String,
            String,
            String,
            i64,
            i64,
            String,
            Option<String>,
            String,
        );
        let linhas = stmt.query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
                row.get(10)?,
            ))
        })?;

        let mut entries = Vec::new();
        for linha in linhas {
            let (
                id,
                vehicle_class,
                race_format,
                incident_source,
                trigger_type,
                severity_context,
                weight_sprint,
                weight_endurance,
                dnf_key,
                non_dnf_key,
                description_key,
            ): LinhaCrua = linha?;
            let legado_da_entry = legado.get(&id).cloned();
            entries.push(CatalogEntry {
                vehicle_class: parse_vehicle_class(&id, &vehicle_class)?,
                race_format: parse_race_format(&id, &race_format)?,
                incident_source: parse_incident_source(&id, &incident_source)?,
                trigger_type: parse_trigger_type(&id, &trigger_type)?,
                severity_context: parse_severity_context(&id, &severity_context)?,
                weight_sprint: parse_peso(&id, "weight_sprint", weight_sprint)?,
                weight_endurance: parse_peso(&id, "weight_endurance", weight_endurance)?,
                id,
                dnf_key,
                non_dnf_key,
                description_key,
                legado: legado_da_entry,
            });
        }

        Ok(Self { entries })
    }

    /// Catálogo vazio — para testes que não se importam com flavor text.
    /// Com catálogo vazio, `select_and_render` retorna `None`,
    /// `catalog_id` fica `None`, e o comportamento existente é preservado.
    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Retorna entries que satisfazem todos os critérios.
    pub fn filter(
        &self,
        vehicle_class: VehicleClass,
        is_endurance: bool,
        incident_source: IncidentSource,
        trigger_type: TriggerType,
        is_dnf: bool,
    ) -> Vec<&CatalogEntry> {
        self.entries
            .iter()
            .filter(|e| {
                e.vehicle_class == vehicle_class
                    && format_matches(e.race_format, is_endurance)
                    && e.incident_source == incident_source
                    && e.trigger_type == trigger_type
                    && severity_matches(e.severity_context, is_dnf)
                    && weight_for(e, is_endurance) > 0
            })
            .collect()
    }

    /// Seleciona uma entry por peso ponderado e renderiza o template.
    /// Retorna `None` se nenhuma entry elegível existe (catálogo vazio ou sem match).
    pub fn select_and_render(
        &self,
        vehicle_class: VehicleClass,
        is_endurance: bool,
        incident_source: IncidentSource,
        trigger_type: TriggerType,
        is_dnf: bool,
        driver_name: &str,
        rng: &mut impl Rng,
    ) -> Option<SelectedEntry> {
        let candidates = self.filter(
            vehicle_class,
            is_endurance,
            incident_source,
            trigger_type,
            is_dnf,
        );
        if candidates.is_empty() {
            return None;
        }

        let total_weight: u32 = candidates.iter().map(|e| weight_for(e, is_endurance)).sum();
        if total_weight == 0 {
            return None;
        }

        let mut pick = rng.gen_range(0..total_weight);
        let chosen = candidates.iter().find(|e| {
            let w = weight_for(e, is_endurance);
            if pick < w {
                true
            } else {
                pick -= w;
                false
            }
        })?;

        // Texto no locale ATIVO, resolvido pela chave que o banco guarda. A ausência de uma
        // das variantes (`NonDnfOnly` sem DNF, `DnfOnly` sem não-DNF) só é alcançável se o
        // filtro for burlado; nesse caso a outra variante serve. `{driver}` entra aqui.
        let chave = if is_dnf {
            if chosen.dnf_key.is_empty() {
                chosen.non_dnf_key.clone().unwrap_or_default()
            } else {
                chosen.dnf_key.clone()
            }
        } else {
            chosen
                .non_dnf_key
                .clone()
                .unwrap_or_else(|| chosen.dnf_key.clone())
        };
        let template = resolver_no_locale(&chave)
            .or_else(|| {
                chosen.legado.as_ref().and_then(|l| {
                    if is_dnf {
                        l.dnf.clone().or_else(|| l.non_dnf.clone())
                    } else {
                        l.non_dnf.clone().or_else(|| l.dnf.clone())
                    }
                })
            })
            .unwrap_or_else(|| chave.clone());
        let rendered_text = template.replace("{driver}", driver_name);

        let description_short = resolver_no_locale(&chosen.description_key)
            .or_else(|| chosen.legado.as_ref().and_then(|l| l.description.clone()))
            .unwrap_or_else(|| chosen.description_key.clone());

        Some(SelectedEntry {
            catalog_id: chosen.id.clone(),
            rendered_text,
            description_short,
        })
    }
}

/// Resolve uma chave no locale ativo. `None` quando a chave é vazia (a entrada não tem
/// aquela variante) ou quando falta no locale — `rust_i18n::t!` devolve a PRÓPRIA chave
/// nesse caso, e é assim que se percebe a linha faltando.
fn resolver_no_locale(chave: &str) -> Option<String> {
    if chave.is_empty() {
        return None;
    }
    let texto = rust_i18n::t!(chave).to_string();
    if texto == chave {
        None
    } else {
        Some(texto)
    }
}

/// Prosa preservada pela v65, indexada por id.
///
/// **Tabela ausente** devolve mapa vazio em vez de erro, e isso continua: banco de teste
/// montado à mão e save que ainda não passou pela v65 não têm essa tabela, e nenhum dos dois é
/// motivo para o catálogo inteiro deixar de carregar. A ausência é reconhecida no `prepare`,
/// que é o único ponto em que ela aparece.
///
/// **Linha malformada** dentro de uma tabela que existe é outra coisa, e agora sobe como erro:
/// o `filter_map(ok)` que estava aqui descartava a linha calada, e a entrada perdia o texto de
/// fallback sem que nada indicasse o buraco.
fn carregar_texto_legado(conn: &Connection) -> Result<HashMap<String, TextoLegado>, DbError> {
    let Ok(mut stmt) = conn.prepare(
        "SELECT id, dnf_template, non_dnf_template, description_short
         FROM incident_catalog_texto_legado",
    ) else {
        return Ok(HashMap::new());
    };
    let linhas = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            TextoLegado {
                dnf: row.get(1)?,
                non_dnf: row.get(2)?,
                description: row.get(3)?,
            },
        ))
    })?;
    let mut mapa = HashMap::new();
    for linha in linhas {
        let (id, texto) = linha?;
        mapa.insert(id, texto);
    }
    Ok(mapa)
}

fn weight_for(entry: &CatalogEntry, is_endurance: bool) -> u32 {
    if is_endurance {
        entry.weight_endurance
    } else {
        entry.weight_sprint
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use super::*;

    /// Cenário de quebra real (`breakdown.<id>`) resolve nos dois locales, mantém o
    /// placeholder `{driver}` (substituído no render) e não vaza `%{...}`. `#[serial]`
    /// porque troca o locale global.
    #[test]
    #[serial_test::serial]
    fn breakdown_scenario_resolve_nos_dois_locales() {
        rust_i18n::set_locale("pt-BR");
        let pt = rust_i18n::t!("breakdown.SB_S_MEC_01.dnf").to_string();
        assert!(
            pt.contains("{driver}") && pt.contains("câmbio") && !pt.contains("%{"),
            "{pt}"
        );

        rust_i18n::set_locale("en-US");
        let en = rust_i18n::t!("breakdown.SB_S_MEC_01.dnf").to_string();
        assert!(
            en.contains("{driver}") && en.contains("gearbox") && !en.contains("%{"),
            "{en}"
        );
        assert_ne!(pt, en);

        rust_i18n::set_locale("pt-BR"); // restaura
    }

    /// **TODA chave semeada tem texto nos dois locales.** O teste acima confere UM id; este
    /// confere as chaves das 54 entradas, e é o que sustenta a decisão de tirar a prosa do
    /// banco: sem linha no locale não há mais nada para onde cair.
    ///
    /// A lista sai do próprio seed (`ENTRADAS`), não de uma cópia — cenário novo entra na
    /// guarda no mesmo commit em que entra no catálogo. E são as TRÊS chaves, não só `.part`:
    /// a presença de `.dnf`/`.warn` é derivada do `severity_context` pela mesma função que o
    /// banco usa, então cobrar as três não dá mais falso positivo.
    ///
    /// `#[serial]` porque o locale é global do processo.
    #[test]
    #[serial_test::serial]
    fn todo_id_do_catalogo_tem_texto_nos_dois_locales() {
        use crate::db::migrations::seed_incidentes::{chaves_de_texto, todas_as_entradas};

        assert_eq!(
            todas_as_entradas().count(),
            70,
            "o catálogo mudou de tamanho — confira a guarda antes de mudar o número"
        );

        let anterior = rust_i18n::locale().to_string();
        let mut faltando = Vec::new();
        for locale in ["pt-BR", "en-US"] {
            rust_i18n::set_locale(locale);
            for entrada in todas_as_entradas() {
                let (dnf, non_dnf, curta) = chaves_de_texto(entrada.0, entrada.5);
                let chaves = [Some(dnf).filter(|c| !c.is_empty()), non_dnf, Some(curta)];
                for chave in chaves.into_iter().flatten() {
                    if rust_i18n::t!(&chave) == chave {
                        faltando.push(format!("{locale}: {chave}"));
                    }
                }
            }
        }
        rust_i18n::set_locale(&anterior);

        assert!(
            faltando.is_empty(),
            "chave do catálogo sem texto no locale (o jogador leria a chave crua na tela):\n{}",
            faltando.join("\n")
        );
    }

    /// Entry de teste. As "chaves" aqui são frases soltas de propósito: chave que não existe
    /// em locale nenhum é renderizada como ela mesma, então o fixture fica legível e ainda
    /// exercita a substituição de `{driver}`.
    fn make_entry(
        id: &str,
        vehicle_class: VehicleClass,
        race_format: RaceFormatFilter,
        incident_source: IncidentSource,
        trigger_type: TriggerType,
        severity_context: SeverityContext,
        weight_sprint: u32,
        weight_endurance: u32,
        dnf_key: &str,
        non_dnf_key: Option<&str>,
    ) -> CatalogEntry {
        CatalogEntry {
            id: id.to_string(),
            vehicle_class,
            race_format,
            incident_source,
            trigger_type,
            severity_context,
            weight_sprint,
            weight_endurance,
            dnf_key: dnf_key.to_string(),
            non_dnf_key: non_dnf_key.map(|s| s.to_string()),
            description_key: format!("{} short", id),
            legado: None,
        }
    }

    #[test]
    fn test_filter_returns_matching_entries() {
        let catalog = IncidentCatalog {
            entries: vec![
                make_entry(
                    "SB_S",
                    VehicleClass::StreetBased,
                    RaceFormatFilter::Sprint,
                    IncidentSource::Mechanical,
                    TriggerType::Spontaneous,
                    SeverityContext::Both,
                    100,
                    0,
                    "dnf {driver}",
                    Some("non {driver}"),
                ),
                make_entry(
                    "RS_S",
                    VehicleClass::RaceSpec,
                    RaceFormatFilter::Sprint,
                    IncidentSource::Mechanical,
                    TriggerType::Spontaneous,
                    SeverityContext::Both,
                    100,
                    0,
                    "dnf {driver}",
                    Some("non {driver}"),
                ),
                make_entry(
                    "SB_E",
                    VehicleClass::StreetBased,
                    RaceFormatFilter::Endurance,
                    IncidentSource::Mechanical,
                    TriggerType::Spontaneous,
                    SeverityContext::Both,
                    0,
                    100,
                    "dnf {driver}",
                    Some("non {driver}"),
                ),
            ],
        };

        let result = catalog.filter(
            VehicleClass::StreetBased,
            false,
            IncidentSource::Mechanical,
            TriggerType::Spontaneous,
            true,
        );
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "SB_S");
    }

    #[test]
    fn test_filter_both_format_matches_sprint_and_endurance() {
        let catalog = IncidentCatalog {
            entries: vec![make_entry(
                "BOTH",
                VehicleClass::StreetBased,
                RaceFormatFilter::Both,
                IncidentSource::PostCollision,
                TriggerType::PostCollision,
                SeverityContext::Both,
                100,
                100,
                "{driver} dnf",
                None,
            )],
        };

        let sprint = catalog.filter(
            VehicleClass::StreetBased,
            false,
            IncidentSource::PostCollision,
            TriggerType::PostCollision,
            true,
        );
        let endurance = catalog.filter(
            VehicleClass::StreetBased,
            true,
            IncidentSource::PostCollision,
            TriggerType::PostCollision,
            true,
        );
        assert_eq!(sprint.len(), 1);
        assert_eq!(endurance.len(), 1);
    }

    #[test]
    fn test_weighted_selection_excludes_zero_weight() {
        let catalog = IncidentCatalog {
            entries: vec![
                make_entry(
                    "GOOD",
                    VehicleClass::StreetBased,
                    RaceFormatFilter::Sprint,
                    IncidentSource::Mechanical,
                    TriggerType::Spontaneous,
                    SeverityContext::Both,
                    100,
                    0,
                    "{driver} dnf",
                    None,
                ),
                make_entry(
                    "ZERO",
                    VehicleClass::StreetBased,
                    RaceFormatFilter::Sprint,
                    IncidentSource::Mechanical,
                    TriggerType::Spontaneous,
                    SeverityContext::Both,
                    0,
                    0,
                    "{driver} dnf",
                    None,
                ),
            ],
        };
        let mut rng = StdRng::seed_from_u64(42);

        for _ in 0..20 {
            let result = catalog.select_and_render(
                VehicleClass::StreetBased,
                false,
                IncidentSource::Mechanical,
                TriggerType::Spontaneous,
                true,
                "Piloto",
                &mut rng,
            );
            assert!(result.is_some());
            assert_eq!(result.unwrap().catalog_id, "GOOD");
        }
    }

    #[test]
    fn test_select_and_render_substitutes_driver_name() {
        let catalog = IncidentCatalog {
            entries: vec![make_entry(
                "E1",
                VehicleClass::StreetBased,
                RaceFormatFilter::Sprint,
                IncidentSource::Mechanical,
                TriggerType::Spontaneous,
                SeverityContext::Both,
                100,
                0,
                "{driver} abandona com problema no câmbio",
                None,
            )],
        };
        let mut rng = StdRng::seed_from_u64(1);

        let result = catalog.select_and_render(
            VehicleClass::StreetBased,
            false,
            IncidentSource::Mechanical,
            TriggerType::Spontaneous,
            true,
            "Senna",
            &mut rng,
        );
        assert!(result.is_some());
        let sel = result.unwrap();
        assert_eq!(sel.rendered_text, "Senna abandona com problema no câmbio");
        assert_eq!(sel.catalog_id, "E1");
    }

    #[test]
    fn test_select_and_render_non_dnf_uses_non_dnf_key() {
        let catalog = IncidentCatalog {
            entries: vec![make_entry(
                "E1",
                VehicleClass::StreetBased,
                RaceFormatFilter::Sprint,
                IncidentSource::Mechanical,
                TriggerType::Spontaneous,
                SeverityContext::Both,
                100,
                0,
                "{driver} abandona",
                Some("{driver} perdeu ritmo"),
            )],
        };
        let mut rng = StdRng::seed_from_u64(1);

        let result = catalog.select_and_render(
            VehicleClass::StreetBased,
            false,
            IncidentSource::Mechanical,
            TriggerType::Spontaneous,
            false,
            "Prost",
            &mut rng,
        );
        assert_eq!(result.unwrap().rendered_text, "Prost perdeu ritmo");
    }

    /// Banco de verdade, com as migrações aplicadas, reduzido a UMA entrada para a seleção
    /// ficar determinística.
    fn save_com_uma_entrada(id: &str) -> Connection {
        let conn = Connection::open_in_memory().expect("banco em memória");
        crate::db::migrations::run_all(&conn).expect("migrações");
        conn.execute("DELETE FROM incident_catalog WHERE id <> ?1", [id])
            .expect("reduzir catálogo");
        conn
    }

    /// **O MESMO save renderiza nos dois idiomas.** É o que a v65 comprou: o banco guarda
    /// chave, então trocar o locale muda o texto do incidente sem tocar em nada gravado.
    ///
    /// O que NÃO muda é o histórico já escrito — incidente antigo foi gravado como prosa
    /// renderizada e continua no idioma da corrida. A decisão está no topo do módulo.
    #[test]
    #[serial_test::serial]
    fn troca_de_locale_muda_o_texto_do_mesmo_save() {
        let conn = save_com_uma_entrada("SB_S_MEC_01");
        let catalog = IncidentCatalog::load(&conn).expect("catálogo");
        let anterior = rust_i18n::locale().to_string();

        let render = |locale: &str| {
            rust_i18n::set_locale(locale);
            let mut rng = StdRng::seed_from_u64(7);
            catalog
                .select_and_render(
                    VehicleClass::StreetBased,
                    false,
                    IncidentSource::Mechanical,
                    TriggerType::Spontaneous,
                    true,
                    "Senna",
                    &mut rng,
                )
                .expect("entrada elegível")
        };

        let pt = render("pt-BR");
        let en = render("en-US");
        rust_i18n::set_locale(&anterior);

        assert_eq!(pt.catalog_id, "SB_S_MEC_01");
        assert_eq!(en.catalog_id, "SB_S_MEC_01");
        assert_ne!(pt.rendered_text, en.rendered_text);
        assert_ne!(pt.description_short, en.description_short);
        assert!(pt.rendered_text.contains("Senna") && pt.rendered_text.contains("câmbio"));
        assert!(en.rendered_text.contains("Senna") && en.rendered_text.contains("gearbox"));
        // Chave crua na tela seria o sintoma de linha faltando no locale.
        for texto in [
            &pt.rendered_text,
            &en.rendered_text,
            &pt.description_short,
            &en.description_short,
        ] {
            assert!(!texto.contains("breakdown."), "chave crua vazou: {texto}");
        }
    }

    /// **Id que o catálogo não conhece continua legível, sem tradução inventada.** A prosa
    /// preservada pela v65 é o texto exibido, nos dois locales, porque é o único texto que
    /// existe para aquela entrada.
    #[test]
    #[serial_test::serial]
    fn texto_legado_de_id_desconhecido_ainda_renderiza() {
        let conn = save_com_uma_entrada("SB_S_MEC_01");
        conn.execute_batch(
            "DELETE FROM incident_catalog;
             INSERT INTO incident_catalog
               (id, vehicle_class, race_format, incident_source, trigger_type,
                severity_context, weight_sprint, weight_endurance,
                dnf_key, non_dnf_key, description_key)
             VALUES ('XX_CASEIRO_99','StreetBased','Sprint','Mechanical','Spontaneous','Both',
                     50, 0,
                     'breakdown.XX_CASEIRO_99.dnf',
                     'breakdown.XX_CASEIRO_99.warn',
                     'breakdown.XX_CASEIRO_99.part');
             INSERT INTO incident_catalog_texto_legado
               (id, dnf_template, non_dnf_template, description_short, migrado_em)
             VALUES ('XX_CASEIRO_99',
                     '{driver} abandona com a bomba d''água estourada',
                     '{driver} perdeu pressão de água',
                     'Bomba d''água',
                     '2026-08-11T00:00:00-03:00');",
        )
        .expect("entrada caseira");

        let catalog = IncidentCatalog::load(&conn).expect("catálogo");
        let anterior = rust_i18n::locale().to_string();
        rust_i18n::set_locale("en-US");
        let mut rng = StdRng::seed_from_u64(3);
        let sel = catalog
            .select_and_render(
                VehicleClass::StreetBased,
                false,
                IncidentSource::Mechanical,
                TriggerType::Spontaneous,
                true,
                "Prost",
                &mut rng,
            )
            .expect("entrada elegível");
        rust_i18n::set_locale(&anterior);

        assert_eq!(
            sel.rendered_text,
            "Prost abandona com a bomba d'água estourada"
        );
        assert_eq!(sel.description_short, "Bomba d'água");
    }

    /// **Mesma semente + mesmo catálogo lógico = mesma sequência, doa a ordem física que der.**
    ///
    /// A seleção ponderada caminha os candidatos somando peso, então a ordem da lista escolhe
    /// a entrada. Enquanto o `SELECT` não tinha `ORDER BY`, quem escolhia era o `rowid` — e
    /// dois saves com o mesmo catálogo podiam divergir depois de um `VACUUM` ou de um re-seed
    /// em outra ordem. Aqui o segundo banco tem a tabela reescrita de trás para frente.
    #[test]
    fn a_ordem_fisica_do_sqlite_nao_muda_a_selecao() {
        let direto = Connection::open_in_memory().expect("banco em memória");
        crate::db::migrations::run_all(&direto).expect("migrações");

        let invertido = Connection::open_in_memory().expect("banco em memória");
        crate::db::migrations::run_all(&invertido).expect("migrações");
        invertido
            .execute_batch(
                "CREATE TEMP TABLE copia AS SELECT * FROM incident_catalog;
                 DELETE FROM incident_catalog;
                 INSERT INTO incident_catalog SELECT * FROM copia ORDER BY id DESC;
                 DROP TABLE copia;",
            )
            .expect("reescrever o catálogo na ordem inversa");

        let a = IncidentCatalog::load(&direto).expect("catálogo");
        let b = IncidentCatalog::load(&invertido).expect("catálogo");

        let ids_a: Vec<&str> = a.entries.iter().map(|e| e.id.as_str()).collect();
        let ids_b: Vec<&str> = b.entries.iter().map(|e| e.id.as_str()).collect();
        assert!(!ids_a.is_empty(), "o catálogo semeado veio vazio");
        assert_eq!(
            ids_a, ids_b,
            "a ordem de carregamento seguiu a ordem física"
        );

        let mut ordenado = ids_a.clone();
        ordenado.sort_unstable();
        assert_eq!(ids_a, ordenado, "o carregamento não está ordenado por id");

        let sequencia = |catalog: &IncidentCatalog| {
            let mut rng = StdRng::seed_from_u64(20_260_812);
            (0..200)
                .map(|_| {
                    catalog
                        .select_and_render(
                            VehicleClass::StreetBased,
                            false,
                            IncidentSource::Mechanical,
                            TriggerType::Spontaneous,
                            true,
                            "Senna",
                            &mut rng,
                        )
                        .expect("entrada elegível")
                        .catalog_id
                })
                .collect::<Vec<_>>()
        };
        let seq_a = sequencia(&a);
        let distintos: std::collections::HashSet<&String> = seq_a.iter().collect();
        assert!(
            distintos.len() > 1,
            "o sorteio caiu sempre na mesma entrada — o teste não prova nada"
        );
        assert_eq!(seq_a, sequencia(&b));
    }

    /// **Linha malformada derruba o carregamento, e o erro diz qual e onde.** O
    /// `filter_map(|r| r.ok())` que estava aqui descartava a linha calada: o catálogo carregava
    /// menor, a distribuição do sorteio mudava e nada acusava.
    #[test]
    fn linha_malformada_derruba_o_carregamento_com_diagnostico() {
        let casos: &[(&str, &str)] = &[
            (
                "UPDATE incident_catalog SET vehicle_class = 'RaceSpecc' WHERE id = 'SB_S_MEC_01'",
                "vehicle_class",
            ),
            (
                "UPDATE incident_catalog SET trigger_type = '' WHERE id = 'SB_S_MEC_01'",
                "trigger_type",
            ),
            // Peso negativo virava um `u32` astronômico no `as` cru e engolia o sorteio.
            (
                "UPDATE incident_catalog SET weight_sprint = -1 WHERE id = 'SB_S_MEC_01'",
                "weight_sprint",
            ),
        ];

        for (sql, coluna) in casos {
            let conn = save_com_uma_entrada("SB_S_MEC_01");
            conn.execute(sql, []).expect("estragar a linha");
            let Err(erro) = IncidentCatalog::load(&conn) else {
                panic!("linha com {coluna} malformado deveria derrubar o carregamento");
            };
            let texto = erro.to_string();
            assert!(
                texto.contains("SB_S_MEC_01") && texto.contains(coluna),
                "o erro não diz qual linha nem qual coluna: {texto}"
            );
        }
    }

    #[test]
    fn test_empty_catalog_returns_none() {
        let catalog = IncidentCatalog::empty();
        let mut rng = StdRng::seed_from_u64(1);

        let result = catalog.select_and_render(
            VehicleClass::StreetBased,
            false,
            IncidentSource::Mechanical,
            TriggerType::Spontaneous,
            true,
            "Piloto",
            &mut rng,
        );
        assert!(result.is_none());
    }

    /// **O catálogo de categorias inteiro, uma a uma.** A tabela é escrita à mão de propósito:
    /// categoria nova entra por aqui declarando a classe dela, ou declarando que não tem uma.
    #[test]
    fn toda_categoria_resolve_ou_recusa_explicitamente() {
        use crate::constants::categories::{get_category, CATEGORIES};

        let esperado: &[(&str, VehicleClass)] = &[
            ("mazda_rookie", VehicleClass::StreetBased),
            ("toyota_rookie", VehicleClass::StreetBased),
            ("mazda_amador", VehicleClass::StreetBased),
            ("toyota_amador", VehicleClass::StreetBased),
            ("bmw_m2", VehicleClass::StreetBased),
            // Multiclasse de três carros de rua: as três classes concordam, então a resposta
            // não é escolha arbitrária.
            ("production_challenger", VehicleClass::StreetBased),
            ("gt4", VehicleClass::RaceSpec),
            ("gt3", VehicleClass::RaceSpec),
            ("lmp2", VehicleClass::Prototype),
            // GT4 + GT3 (RaceSpec) e LMP2 (Prototype) no mesmo grid: sem classe não há
            // resposta, e inventar uma é o bug que este item fechou.
            ("endurance", VehicleClass::Unknown),
        ];

        for (id, classe) in esperado {
            assert!(
                get_category(id).is_some(),
                "'{id}' saiu de constants::categories — atualize a tabela deste teste"
            );
            assert_eq!(
                vehicle_class_from_category(id),
                *classe,
                "classe de veículo de '{id}'"
            );
        }

        for categoria in CATEGORIES.iter() {
            assert!(
                esperado.iter().any(|(id, _)| *id == categoria.id),
                "categoria '{}' não está na tabela deste teste: declare a classe dela (ou \
                 Unknown, com o motivo) em vehicle_class_from_category",
                categoria.id
            );
        }
    }

    /// **O LMP2 tem flavor text PRÓPRIO, e não o genérico.**
    ///
    /// A correção de classe de 12/08/2026 tirou o LMP2 de `StreetBased` — ele deixou de ler
    /// sobre o câmbio do MX-5 — e o deixou em `Prototype`, classe que não tinha uma única linha
    /// no catálogo. O sintoma trocou de mentira por silêncio: `select_and_render` devolvia
    /// `None` e o incidente saía com o texto genérico do motor. Este caso cobre as duas metades.
    ///
    /// O `StreetBased` entra junto de propósito: é a prova de que a classe importa mesmo — sem
    /// ele, um catálogo que ignorasse o filtro passaria igual.
    #[test]
    #[serial_test::serial]
    fn o_prototipo_e_o_carro_de_rua_recebem_textos_de_classes_diferentes() {
        let conn = Connection::open_in_memory().expect("banco em memória");
        crate::db::migrations::run_all(&conn).expect("migrações");
        let catalog = IncidentCatalog::load(&conn).expect("catálogo");

        assert_eq!(
            vehicle_class_from_category("lmp2"),
            VehicleClass::Prototype,
            "o LMP2 deixou de resolver como protótipo"
        );

        let anterior = rust_i18n::locale().to_string();
        rust_i18n::set_locale("pt-BR");

        let render = |classe: VehicleClass, semente: u64| {
            let mut rng = StdRng::seed_from_u64(semente);
            catalog.select_and_render(
                classe,
                false,
                IncidentSource::Mechanical,
                TriggerType::Spontaneous,
                true,
                "Senna",
                &mut rng,
            )
        };

        // Várias sementes: uma só poderia cair sempre na mesma entrada e não provar cobertura.
        let mut ids_proto = std::collections::HashSet::new();
        for semente in 0..40u64 {
            let sel = render(VehicleClass::Prototype, semente)
                .expect("protótipo sem candidato — o LMP2 voltou ao texto genérico");
            assert!(
                sel.catalog_id.starts_with("PR_"),
                "o protótipo casou com uma entrada de outra classe: {}",
                sel.catalog_id
            );
            assert!(
                !sel.rendered_text.contains("breakdown."),
                "chave crua vazou: {}",
                sel.rendered_text
            );
            ids_proto.insert(sel.catalog_id);
        }
        assert!(
            ids_proto.len() > 1,
            "o sorteio caiu sempre na mesma entrada — o teste não prova cobertura"
        );

        // Street-based continua respondendo pela lista dela, e o texto é OUTRO.
        let rua = render(VehicleClass::StreetBased, 7).expect("carro de rua sem candidato");
        assert!(rua.catalog_id.starts_with("SB_"), "{}", rua.catalog_id);
        let proto = render(VehicleClass::Prototype, 7).expect("protótipo sem candidato");
        assert_ne!(rua.rendered_text, proto.rendered_text);

        // E o enduro do protótipo, que tem lista própria de pesos.
        let mut rng = StdRng::seed_from_u64(11);
        let enduro = catalog
            .select_and_render(
                VehicleClass::Prototype,
                true,
                IncidentSource::Mechanical,
                TriggerType::Spontaneous,
                true,
                "Senna",
                &mut rng,
            )
            .expect("protótipo sem candidato de enduro");
        assert!(
            enduro.catalog_id.starts_with("PR_"),
            "{}",
            enduro.catalog_id
        );

        rust_i18n::set_locale(&anterior);
    }

    /// **`Unknown` continua sem candidato — o fallback genérico é só para o desconhecido.**
    ///
    /// O grid multiclasse do Endurance sem classe declarada não tem resposta única, e é essa
    /// ausência que o texto genérico serve. Acrescentar o protótipo não pode ter transformado
    /// `Unknown` num sinônimo de "protótipo".
    #[test]
    fn o_generico_segue_valendo_so_para_a_classe_desconhecida() {
        let conn = Connection::open_in_memory().expect("banco em memória");
        crate::db::migrations::run_all(&conn).expect("migrações");
        let catalog = IncidentCatalog::load(&conn).expect("catálogo");
        let mut rng = StdRng::seed_from_u64(3);

        assert_eq!(
            vehicle_class_from_category("endurance"),
            VehicleClass::Unknown
        );
        assert!(catalog
            .select_and_render(
                VehicleClass::Unknown,
                false,
                IncidentSource::Mechanical,
                TriggerType::Spontaneous,
                true,
                "Senna",
                &mut rng,
            )
            .is_none());
    }

    /// A forma `categoria:classe` resolve pela CLASSE — é o que faz o grid multiclasse do
    /// Endurance responder carro por carro.
    #[test]
    fn categoria_com_classe_resolve_pela_classe() {
        assert_eq!(
            vehicle_class_from_category("endurance:gt4"),
            VehicleClass::RaceSpec
        );
        assert_eq!(
            vehicle_class_from_category("endurance:gt3"),
            VehicleClass::RaceSpec
        );
        assert_eq!(
            vehicle_class_from_category("endurance:lmp2"),
            VehicleClass::Prototype
        );
        assert_eq!(
            vehicle_class_from_category("production_challenger:bmw"),
            // "bmw" é o nome da CLASSE, não o id da categoria do carro (`bmw_m2`). Sem
            // correspondência, a recusa é a resposta certa.
            VehicleClass::Unknown
        );
        // Sufixo vazio é a categoria crua, não uma classe.
        assert_eq!(vehicle_class_from_category("gt3:"), VehicleClass::RaceSpec);
    }

    /// Categoria que ninguém conhece devolve `Unknown`, e `Unknown` não casa com nenhuma
    /// entrada do catálogo — o incidente sai com o texto genérico do motor em vez de com
    /// flavor text de um carro que não é aquele.
    #[test]
    fn categoria_desconhecida_recusa_em_vez_de_chutar() {
        assert_eq!(
            vehicle_class_from_category("formula_x"),
            VehicleClass::Unknown
        );
        assert_eq!(vehicle_class_from_category(""), VehicleClass::Unknown);

        let catalog = IncidentCatalog {
            entries: vec![make_entry(
                "SB",
                VehicleClass::StreetBased,
                RaceFormatFilter::Sprint,
                IncidentSource::Mechanical,
                TriggerType::Spontaneous,
                SeverityContext::Both,
                100,
                0,
                "{driver} dnf",
                None,
            )],
        };
        let mut rng = StdRng::seed_from_u64(9);
        assert!(catalog
            .select_and_render(
                VehicleClass::Unknown,
                false,
                IncidentSource::Mechanical,
                TriggerType::Spontaneous,
                true,
                "Piquet",
                &mut rng,
            )
            .is_none());
    }

    #[test]
    fn test_severity_context_dnf_only_excludes_non_dnf() {
        let catalog = IncidentCatalog {
            entries: vec![make_entry(
                "DNF_ONLY",
                VehicleClass::StreetBased,
                RaceFormatFilter::Sprint,
                IncidentSource::Operational,
                TriggerType::PostSpinStall,
                SeverityContext::DnfOnly,
                100,
                0,
                "{driver} rodou e não religou",
                None,
            )],
        };

        let dnf_result = catalog.filter(
            VehicleClass::StreetBased,
            false,
            IncidentSource::Operational,
            TriggerType::PostSpinStall,
            true,
        );
        let non_dnf_result = catalog.filter(
            VehicleClass::StreetBased,
            false,
            IncidentSource::Operational,
            TriggerType::PostSpinStall,
            false,
        );

        assert_eq!(dnf_result.len(), 1);
        assert_eq!(non_dnf_result.len(), 0);
    }
}
