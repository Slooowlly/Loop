//! Guard do CONTRATO serde da oferta de quebra de contrato do jogador.
//!
//! `PlayerPoachOffer` é a única tela em que o jogador decide um poaching, e ela
//! atravessa a ponte para o React (`src/components/season/PoachAuctionModal.jsx`).
//! Renomear um campo aqui compila, serializa e quebra a tela em silêncio: o React
//! lê `offer.buyout` e recebe `undefined`.
//!
//! Este teste fixa os nomes que a tela consome. Campo novo pode entrar sem
//! cerimônia; campo existente só sai junto com o consumidor.

use super::*;

/// Campos lidos hoje pelo `PoachAuctionModal.jsx`, mais os ids que a resolução usa.
const CAMPOS_DO_CONTRATO: &[&str] = &[
    // resolução (o backend confere a oferta persistida por estes três)
    "current_contract_id",
    "current_team_id",
    "suitor_team_id",
    // números do negócio, todos na tela
    "buyout",
    "current_salary",
    "poacher_best",
    "holder_best",
    // exibição
    "suitor_name",
    "suitor_color",
    "current_team_name",
    "current_team_color",
    "bids",
];

fn oferta_de_amostra() -> PlayerPoachOffer {
    PlayerPoachOffer {
        current_contract_id: "C001".to_string(),
        current_team_id: "T001".to_string(),
        suitor_team_id: "T002".to_string(),
        buyout: 120_000.0,
        current_salary: 300_000.0,
        poacher_best: 480_000.0,
        holder_best: 340_000.0,
        suitor_name: "Equipe Assediante".to_string(),
        suitor_color: "#ff0000".to_string(),
        suitor_car_rating: 80,
        current_team_name: "Equipe Atual".to_string(),
        current_team_color: "#00ff00".to_string(),
        category_label: "GT3".to_string(),
        incumbent_name: Some("Piloto Deslocado".to_string()),
        player_fama: 60,
        bids: vec![PoachBid {
            team_name: "Equipe Atual".to_string(),
            is_poacher: false,
            salary: 300_000.0,
            label: "abertura".to_string(),
        }],
        poacher_wins: true,
    }
}

#[test]
fn oferta_de_poaching_mantem_os_campos_que_a_tela_le() {
    let json = serde_json::to_value(oferta_de_amostra()).expect("oferta serializa");
    let objeto = json.as_object().expect("oferta é um objeto JSON");

    for campo in CAMPOS_DO_CONTRATO {
        assert!(
            objeto.contains_key(*campo),
            "campo '{campo}' sumiu do PlayerPoachOffer — a tela do leilão lê esse nome \
             e passaria a receber undefined sem nenhum erro"
        );
    }
}

#[test]
fn lance_do_leilao_mantem_os_campos_que_a_tela_le() {
    let json = serde_json::to_value(oferta_de_amostra()).expect("oferta serializa");
    let primeiro = json["bids"][0]
        .as_object()
        .expect("cada lance é um objeto JSON");

    for campo in ["team_name", "is_poacher", "salary", "label"] {
        assert!(
            primeiro.contains_key(campo),
            "campo '{campo}' sumiu do PoachBid — a tabela de lances depende dele"
        );
    }
}

#[test]
fn oferta_de_poaching_faz_ida_e_volta_no_plano_da_janela() {
    // A oferta é PERSISTIDA em JSON no plano da pré-temporada e relida na decisão.
    // Um campo que serialize mas não desserialize deixaria o plano ilegível e
    // derrubaria a janela inteira, não só a tela.
    let original = oferta_de_amostra();
    let texto = serde_json::to_string(&original).expect("serializa");
    let voltou: PlayerPoachOffer = serde_json::from_str(&texto).expect("desserializa");

    assert_eq!(voltou.current_contract_id, original.current_contract_id);
    assert_eq!(voltou.suitor_team_id, original.suitor_team_id);
    assert_eq!(voltou.holder_best, original.holder_best);
    assert_eq!(voltou.bids.len(), original.bids.len());
}
