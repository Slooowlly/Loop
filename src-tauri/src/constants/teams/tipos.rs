#![allow(dead_code)]

pub struct TeamTemplate {
    pub nome: &'static str,
    pub nome_curto: &'static str,
    pub categoria: &'static str,
    pub cor_primaria: &'static str,
    pub cor_secundaria: &'static str,
    pub pais_sede: &'static str,
    pub marca: Option<&'static str>,
    pub classe: Option<&'static str>,
    pub car_performance_base: f64,
    pub budget_base: f64,
    pub reputacao_base: f64,
}
