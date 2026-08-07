//! Os parâmetros FÍSICOS de cada divisão, numa tabela só e explícita.
//!
//! Esta tabela é a inversão de sentido pedida na seção 3.3 do redesign. A economia velha
//! partia de um orçamento por categoria (`finance::planning::category_finance_scale`) e
//! chamava 7% dele de "gasolina". Aqui não existe orçamento de entrada: existe quanto o
//! carro anda, quanto ele bebe, quantos jogos de pneu o fim de semana consome e quanta
//! gente viaja. O custo operacional de referência é a SOMA disso — uma consequência do
//! modelo, não uma entrada dele.
//!
//! **Nenhum número aqui foi ajustado para bater com a escala antiga.** Onde a conta física
//! diverge dela, a divergência é o resultado.
//!
//! ## A chave da tabela é a DIVISÃO, não a categoria
//!
//! `production_challenger` e `endurance` são campeonatos multi-classe: quem corre neles
//! corre com um carro de outra categoria (`MultiClassInfo::car_categoria`). Um MX-5 na
//! Production bebe como MX-5, não como "Production". Então a tabela é indexada pela
//! divisão competitiva — a mesma chave de `constants::categories::competitive_division_key`
//! — e o campeonato entra só no que é de fato do campeonato: duração da prova, taxa de
//! inscrição, noites de hotel.
//!
//! ## De onde vem cada número
//!
//! - **velocidade média**: ritmo de corrida do carro real, usado só para converter a
//!   `duracao_corrida_min` de `constants::categories` em quilômetros.
//! - **consumo**: o ponto de calibração é o próprio documento (seção 2.3): "corrida de
//!   30 min num M2: ~40 litros". 30 min a 145 km/h = 72,5 km → 0,55 l/km. Os outros
//!   carros escalam por potência a partir daí.
//! - **preço do jogo de pneu**: preço de balcão de jogo de slick da categoria.
//! - **comitiva / equipe fixa**: tamanho de operação real da categoria.
//! - **tarifa de frete**: custo por quilômetro do conjunto que a equipe move.
//! - **inscrição**: taxa de inscrição por carro por etapa da categoria.

use crate::constants::categories::{competitive_division_key, get_category_config};

// ── Preços unitários globais ─────────────────────────────────────────────────────────
//
// Não variam por categoria porque a coisa comprada é a mesma: um litro de combustível de
// competição é um litro de combustível de competição. O que varia é a QUANTIDADE, e é
// exatamente esse o ponto do modelo bottom-up.
//
// **A moeda do jogo é o DÓLAR**, não o real: `src/utils/formatters.js` formata todo
// dinheiro em `currency: "USD"` e diz isso em comentário. Todo preço desta tabela está em
// dólar e foi escrito a partir de preço de mercado em dólar — não convertido de outra
// moeda por regra de três.

/// Litro de combustível de competição. Combustível de grid (VP/Sunoco, octanagem
/// controlada, entregue em tambor na pista) sai por volta de US$13 o galão americano, o
/// que dá ~US$3,40 o litro.
pub const PRECO_LITRO_COMBUSTIVEL: f64 = 3.40;

/// Diária de hotel por pessoa da comitiva — hotel médio perto do autódromo, que é o padrão
/// de equipe de automobilismo em fim de semana de prova.
pub const DIARIA_DE_HOTEL: f64 = 120.00;

/// Ajuda de custo diária de quem trabalha o fim de semana (alimentação e deslocamento
/// local). Não é salário: a folha fixa é recorrente de temporada e mora em
/// [`crate::economia::temporada`].
pub const DIARIA_DE_PESSOAL: f64 = 110.00;

/// Passagem por pessoa: uma parte fixa (traslados, bagagem, taxa) e uma parte por
/// quilômetro. Calibrado nos dois extremos que importam — ~US$245 numa viagem continental
/// de 1.250 km e ~US$1.405 numa intercontinental de 8.500 km.
pub const PASSAGEM_PARTE_FIXA: f64 = 45.00;
pub const PASSAGEM_POR_KM: f64 = 0.16;

/// Acima deste ponto o frete deixa de ser proporcional à distância: carreta vira contêiner
/// ou carga aérea consolidada, e o preço por quilômetro despenca. Sem esta quebra uma
/// etapa intercontinental cobraria frete rodoviário por 8.500 km, o que não é uma coisa
/// que exista.
pub const FRETE_KM_SEM_DESCONTO: f64 = 1_500.0;
pub const FRETE_FATOR_LONGA_DISTANCIA: f64 = 0.45;

/// Quanto o programa de treinos e classificação cresce quando a prova cresce.
///
/// Não é 1,0 e nem perto disso. Uma prova de 24 horas tem uns três dias de pista antes da
/// largada; uma de 2 horas tem uma sessão e a classificação. O programa dobra enquanto a
/// corrida cresce doze vezes, o que dá um expoente na casa de 0,3. Com 0,35, a faixa de
/// duração que o calendário sorteia no endurance (120–360 min contra os 225 de referência)
/// move o treino de 0,80× a 1,18× — um terço de amplitude para uma prova 3× maior.
///
/// Isto substitui a versão anterior deste módulo, em que o fim de semana de endurance
/// ficava congelado na média de 225 minutos e uma prova de 6 horas cobrava treino de 3h45.
pub const EXPOENTE_TREINO_POR_DURACAO: f64 = 0.35;

/// Quanto a TAXA DE INSCRIÇÃO cresce quando a prova cresce.
///
/// Inscrição de prova longa é mais cara que a de prova curta — o organizador entrega mais
/// horas de pista, mais comissários, mais tempo de estrutura. Mas a taxa nunca é
/// proporcional à duração: boa parte dela paga o direito de estar no grid, que independe do
/// tamanho da corrida. Com 0,60, uma prova 3× mais longa cobra ~1,9× a inscrição, que é o
/// formato real das tabelas de inscrição de prova de resistência.
///
/// Nas categorias de sprint a duração é única e este expoente nunca sai de 1,0×.
pub const EXPOENTE_INSCRICAO_POR_DURACAO: f64 = 0.60;

/// Parâmetros físicos de UMA divisão competitiva.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParametrosFisicos {
    /// Ritmo médio de corrida do carro, em km/h. Só serve para virar `duracao` em km.
    pub velocidade_media_kmh: f64,
    /// Duração de REFERÊNCIA da prova, em minutos. É o ponto em que os outros parâmetros
    /// do fim de semana foram escritos — a duração concreta da etapa entra por
    /// [`crate::economia::tipos::EntradaDaEtapa::duracao_corrida_min`] e desloca o que
    /// depende dela.
    pub duracao_corrida_min: f64,
    /// Quilometragem por carro em treinos livres e classificação, NA DURAÇÃO DE
    /// REFERÊNCIA. Acontece mesmo se o carro abandonar na primeira volta da corrida.
    /// Prova mais longa traz um programa de treinos maior, mas muito menos que
    /// proporcionalmente — ver [`EXPOENTE_TREINO_POR_DURACAO`].
    pub km_treino_quali: f64,
    /// Litros por quilômetro em ritmo de corrida.
    pub consumo_l_por_km: f64,
    /// Jogos de pneu gastos em treino e classificação. São gastos independentemente do
    /// que a corrida faça — inclusive se o carro abandonar na primeira volta.
    pub jogos_de_pneu_fixos: f64,
    /// Quilômetros que um jogo de pneu aguenta EM CORRIDA. É daqui que sai o consumo de
    /// borracha da prova: `km_rodados / km_por_jogo_de_corrida`.
    ///
    /// O número é maior no endurance que no sprint, e isso não é engano: equipe de sprint
    /// põe borracha nova para uma prova curta e joga fora com vida sobrando, equipe de
    /// endurance estica o jogo por dois stints. Consumo por quilômetro MENOR no endurance.
    pub km_por_jogo_de_corrida: f64,
    pub preco_do_jogo_de_pneu: f64,
    /// Custo de revisão de motor, câmbio e freio amortizado por quilômetro rodado.
    /// É o que substitui a linha "peças" de peso fixo da fatura velha: manutenção
    /// mecânica é função de quilometragem, não de orçamento.
    pub revisao_por_km: f64,
    /// Pessoas que viajam para a etapa (inclui os pilotos).
    pub comitiva: f64,
    /// Pessoas na folha fixa da equipe. NÃO é cobrada na fatura da etapa — é recorrente
    /// de temporada. Fica declarada aqui porque é parâmetro do porte da operação e
    /// `economia::temporada` vai lê-la.
    pub equipe_fixa: f64,
    /// Custo por quilômetro de mover o conjunto de transporte da equipe.
    pub tarifa_frete_por_km: f64,
    /// Taxa de inscrição por carro por etapa.
    pub taxa_de_inscricao: f64,
    /// Noites de hotel do fim de semana.
    pub noites_de_hotel: f64,
}

impl ParametrosFisicos {
    /// Quilômetros de corrida de uma prova completa NA DURAÇÃO DE REFERÊNCIA.
    pub fn km_de_corrida(&self) -> f64 {
        self.km_de_corrida_em(self.duracao_corrida_min)
    }

    /// Quilômetros de corrida de uma prova de `duracao_min`.
    pub fn km_de_corrida_em(&self, duracao_min: f64) -> f64 {
        self.velocidade_media_kmh * duracao_min.max(0.0) / 60.0
    }

    /// Quanto esta prova é maior ou menor que a de referência.
    pub fn fator_de_duracao(&self, duracao_min: f64) -> f64 {
        if self.duracao_corrida_min <= 0.0 {
            return 1.0;
        }
        (duracao_min.max(0.0) / self.duracao_corrida_min).clamp(0.05, 20.0)
    }

    /// Quilometragem de treino e classificação de uma prova de `duracao_min`.
    ///
    /// Cresce com a prova, mas de forma fortemente amortecida
    /// ([`EXPOENTE_TREINO_POR_DURACAO`]): o programa de pista antes da largada é um
    /// formato de campeonato, não um múltiplo da duração da corrida.
    pub fn km_treino_quali_em(&self, duracao_min: f64) -> f64 {
        self.km_treino_quali
            * self
                .fator_de_duracao(duracao_min)
                .powf(EXPOENTE_TREINO_POR_DURACAO)
    }

    /// Taxa de inscrição por carro numa prova de `duracao_min`.
    ///
    /// Cresce com a prova de forma amortecida ([`EXPOENTE_INSCRICAO_POR_DURACAO`]): parte
    /// da taxa é o direito de estar no grid e não sabe quantas horas a corrida dura.
    pub fn taxa_de_inscricao_em(&self, duracao_min: f64) -> f64 {
        self.taxa_de_inscricao
            * self
                .fator_de_duracao(duracao_min)
                .powf(EXPOENTE_INSCRICAO_POR_DURACAO)
    }

    /// Voltas de uma prova de `duracao_min` numa pista de `comprimento_km`.
    pub fn voltas_em(&self, duracao_min: f64, comprimento_km: f64) -> f64 {
        if comprimento_km <= 0.0 {
            return 0.0;
        }
        self.km_de_corrida_em(duracao_min) / comprimento_km
    }

    /// Voltas de uma prova completa na pista média do jogo (4,586 km).
    ///
    /// Não usa `calendar::montagem::estimate_laps` de propósito: aquela função grampeia o
    /// resultado em 50 voltas, o que é um limite de exibição de calendário e viraria uma
    /// mentira física numa prova de 4 horas.
    pub fn voltas_de_referencia(&self, comprimento_km: f64) -> f64 {
        if comprimento_km <= 0.0 {
            return 0.0;
        }
        self.km_de_corrida() / comprimento_km
    }
}

/// A tabela. Uma linha por divisão competitiva.
///
/// Valores em dólar, como todo dinheiro do jogo.
///
/// A coluna `min` é a duração de REFERÊNCIA; `jogos fix` são os de treino e quali, e
/// `km/jogo` é a vida de um jogo em corrida.
///
/// | divisão | km/h | min | l/km | jogos fix | km/jogo | US$/jogo | comitiva | US$/km frete | inscrição |
/// |---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
/// | rookie | 132–136 | 15 | 0,25–0,28 | 0,4 | 55–57 | 1.050–1.150 | 5 | 1,00 | 175 |
/// | amador | 132–136 | 25 | 0,25–0,28 | 0,8 | 46–47 | 1.050–1.150 | 7 | 1,30 | 330 |
/// | bmw_m2 | 145 | 25 | 0,55 | 0,8 | 50 | 1.700 | 8 | 1,75 | 550 |
/// | production | 132–145 | 30 | 0,25–0,55 | 0,8 | 55–60 | 1.050–1.700 | 8 | 1,45–1,75 | 650 |
/// | gt4 | 160 | 30 | 0,60 | 0,8 | 67 | 1.900 | 12 | 3,00 | 1.500 |
/// | gt3 | 175 | 50 | 0,72 | 1,2 | 81 | 2.400 | 18 | 4,80 | 3.500 |
/// | lmp2 | 195 | 60 | 0,68 | 1,2 | 108 | 2.800 | 22 | 5,70 | 5.000 |
/// | endurance | 160–195 | 225 | 0,60–0,72 | 1,5–2,0 | 104–133 | 1.900–2.800 | 16–26 | 3,50–6,10 | 5.500–9.000 |
pub fn parametros(categoria: &str, classe: Option<&str>) -> ParametrosFisicos {
    match competitive_division_key(categoria, classe).as_str() {
        // ── Tier 0 — Rookie ──────────────────────────────────────────────────────────
        // MX-5 Cup: 181 cv, motor 2.0 aspirado. ~2min05 na pista média → 132 km/h.
        // Consumo 0,25 l/km: um carro de rua faz 8 l/100 km, um de corrida em pista
        // roda praticamente todo o tempo em plena carga — daí ~25 l/100 km.
        // Fim de semana de rookie é curto: uma prova de 15 min, quali de 15 e um treino
        // livre de 15 → 30 min fora da corrida, a 132 km/h = 66 km.
        "mazda_rookie" => ParametrosFisicos {
            velocidade_media_kmh: 132.0,
            duracao_corrida_min: 15.0,
            km_treino_quali: 66.0,
            consumo_l_por_km: 0.25,
            jogos_de_pneu_fixos: 0.4,
            km_por_jogo_de_corrida: 55.0,
            // Jogo de slick de MX-5 Cup (spec Hankook), preço de balcão.
            preco_do_jogo_de_pneu: 1_050.0,
            // Motor de MX-5 Cup: revisão de ~US$5.400 a cada ~12.000 km.
            revisao_por_km: 0.45,
            // 2 pilotos, 2 mecânicos, 1 chefe. É uma van e um reboque.
            comitiva: 5.0,
            equipe_fixa: 4.0,
            // Van + reboque aberto: combustível, pedágio e motorista.
            tarifa_frete_por_km: 1.00,
            taxa_de_inscricao: 175.0,
            noites_de_hotel: 1.0,
        },
        // GR86 de corrida: ~230 cv, um pouco mais rápido e mais sedento que o MX-5.
        "toyota_rookie" => ParametrosFisicos {
            velocidade_media_kmh: 136.0,
            duracao_corrida_min: 15.0,
            km_treino_quali: 68.0,
            consumo_l_por_km: 0.28,
            jogos_de_pneu_fixos: 0.4,
            km_por_jogo_de_corrida: 57.0,
            preco_do_jogo_de_pneu: 1_150.0,
            revisao_por_km: 0.50,
            comitiva: 5.0,
            equipe_fixa: 4.0,
            tarifa_frete_por_km: 1.00,
            taxa_de_inscricao: 175.0,
            noites_de_hotel: 1.0,
        },

        // ── Tier 1 — Amador ──────────────────────────────────────────────────────────
        // Mesmo carro do rookie, campeonato mais sério: prova de 25 min, dois treinos
        // livres + quali (~40 min fora da corrida), e o pneu já não dura o fim de semana
        // inteiro.
        "mazda_amador" => ParametrosFisicos {
            velocidade_media_kmh: 132.0,
            duracao_corrida_min: 25.0,
            km_treino_quali: 88.0,
            consumo_l_por_km: 0.25,
            jogos_de_pneu_fixos: 0.8,
            km_por_jogo_de_corrida: 46.0,
            preco_do_jogo_de_pneu: 1_050.0,
            revisao_por_km: 0.45,
            // Entra um engenheiro e mais um mecânico.
            comitiva: 7.0,
            equipe_fixa: 6.0,
            tarifa_frete_por_km: 1.30,
            taxa_de_inscricao: 330.0,
            noites_de_hotel: 2.0,
        },
        "toyota_amador" => ParametrosFisicos {
            velocidade_media_kmh: 136.0,
            duracao_corrida_min: 25.0,
            km_treino_quali: 91.0,
            consumo_l_por_km: 0.28,
            jogos_de_pneu_fixos: 0.8,
            km_por_jogo_de_corrida: 47.0,
            preco_do_jogo_de_pneu: 1_150.0,
            revisao_por_km: 0.50,
            comitiva: 7.0,
            equipe_fixa: 6.0,
            tarifa_frete_por_km: 1.30,
            taxa_de_inscricao: 330.0,
            noites_de_hotel: 2.0,
        },

        // ── Tier 2 — Pro ─────────────────────────────────────────────────────────────
        // BMW M2 CS Racing: 3.0 turbo, 365 cv. É a ÂNCORA de consumo do modelo inteiro —
        // o documento mede "corrida de 30 min num M2: ~40 litros", e 30 min a 145 km/h
        // são 72,5 km, o que dá 0,55 l/km. Todos os outros consumos escalam a partir daqui.
        "bmw_m2" => ParametrosFisicos {
            velocidade_media_kmh: 145.0,
            duracao_corrida_min: 25.0,
            km_treino_quali: 97.0,
            consumo_l_por_km: 0.55,
            jogos_de_pneu_fixos: 0.8,
            km_por_jogo_de_corrida: 50.0,
            // Slick de M2 CS Racing, jogo com 4 pneus.
            preco_do_jogo_de_pneu: 1_700.0,
            // Motor turbo custa mais para revisar e o câmbio é sequencial.
            revisao_por_km: 0.70,
            comitiva: 8.0,
            equipe_fixa: 8.0,
            // Caminhão-baú pequeno.
            tarifa_frete_por_km: 1.75,
            taxa_de_inscricao: 550.0,
            noites_de_hotel: 2.0,
        },

        // ── Tier 2 — Production Challenger (multi-classe) ────────────────────────────
        // O campeonato define a prova (30 min) e a inscrição; o CARRO define o consumo,
        // o pneu e a revisão — e o carro é o da classe.
        "production_challenger:mazda" => ParametrosFisicos {
            velocidade_media_kmh: 132.0,
            duracao_corrida_min: 30.0,
            km_treino_quali: 99.0,
            consumo_l_por_km: 0.25,
            jogos_de_pneu_fixos: 0.8,
            km_por_jogo_de_corrida: 55.0,
            preco_do_jogo_de_pneu: 1_050.0,
            revisao_por_km: 0.45,
            comitiva: 8.0,
            equipe_fixa: 7.0,
            tarifa_frete_por_km: 1.45,
            taxa_de_inscricao: 650.0,
            noites_de_hotel: 2.0,
        },
        "production_challenger:toyota" => ParametrosFisicos {
            velocidade_media_kmh: 136.0,
            duracao_corrida_min: 30.0,
            km_treino_quali: 102.0,
            consumo_l_por_km: 0.28,
            jogos_de_pneu_fixos: 0.8,
            km_por_jogo_de_corrida: 57.0,
            preco_do_jogo_de_pneu: 1_150.0,
            revisao_por_km: 0.50,
            comitiva: 8.0,
            equipe_fixa: 7.0,
            tarifa_frete_por_km: 1.45,
            taxa_de_inscricao: 650.0,
            noites_de_hotel: 2.0,
        },
        "production_challenger:bmw" => ParametrosFisicos {
            velocidade_media_kmh: 145.0,
            duracao_corrida_min: 30.0,
            km_treino_quali: 109.0,
            consumo_l_por_km: 0.55,
            jogos_de_pneu_fixos: 0.8,
            km_por_jogo_de_corrida: 60.0,
            preco_do_jogo_de_pneu: 1_700.0,
            revisao_por_km: 0.70,
            comitiva: 8.0,
            equipe_fixa: 8.0,
            tarifa_frete_por_km: 1.75,
            taxa_de_inscricao: 650.0,
            noites_de_hotel: 2.0,
        },

        // ── Tier 3 — GT4 ─────────────────────────────────────────────────────────────
        // GT4 de fábrica: ~430 cv, aerodinâmica de verdade, freio de aço grande.
        // Fim de semana com dois treinos livres longos + quali (~50 min).
        "gt4" => ParametrosFisicos {
            velocidade_media_kmh: 160.0,
            duracao_corrida_min: 30.0,
            km_treino_quali: 133.0,
            consumo_l_por_km: 0.60,
            jogos_de_pneu_fixos: 0.8,
            km_por_jogo_de_corrida: 67.0,
            // Jogo de slick GT4 (Pirelli/Michelin de campeonato).
            preco_do_jogo_de_pneu: 1_900.0,
            revisao_por_km: 1.10,
            // 2 pilotos, 4 mecânicos, engenheiro de pista, engenheiro de dados,
            // motorista do caminhão, chefe, borracheiro, hospitalidade.
            comitiva: 12.0,
            equipe_fixa: 14.0,
            // Carreta com dois carros e box montável.
            tarifa_frete_por_km: 3.00,
            taxa_de_inscricao: 1_500.0,
            noites_de_hotel: 2.0,
        },

        // ── Tier 4 — GT3 ─────────────────────────────────────────────────────────────
        // GT3: 500–550 cv, prova de 50 min. Consumo de 0,72 l/km bate com o stint real
        // de GT3 — ~120 litros para ~170 km.
        "gt3" => ParametrosFisicos {
            velocidade_media_kmh: 175.0,
            duracao_corrida_min: 50.0,
            km_treino_quali: 175.0,
            consumo_l_por_km: 0.72,
            // Três jogos: um de treino, um de quali, um de corrida.
            jogos_de_pneu_fixos: 1.2,
            km_por_jogo_de_corrida: 81.0,
            // Jogo de slick GT3 de campeonato (Pirelli DHE, ~US$2.400).
            preco_do_jogo_de_pneu: 2_400.0,
            // Motor GT3 revisado a cada ~20.000 km, mais câmbio sequencial e disco de
            // carbono — a conta por quilômetro é a mais pesada da escada de sprint.
            revisao_por_km: 1.90,
            // Operação de dois carros com engenharia dupla, dados, nutrição, imprensa.
            comitiva: 18.0,
            equipe_fixa: 24.0,
            // Duas carretas: uma de carros, uma de estrutura e motorhome.
            tarifa_frete_por_km: 4.80,
            taxa_de_inscricao: 3_500.0,
            noites_de_hotel: 3.0,
        },

        // ── Tier 5 — LMP2 (classe de referência) ─────────────────────────────────────
        // Protótipo: 600 cv, mas aerodinâmica muito mais eficiente que a de um GT3, então
        // o consumo por quilômetro é MENOR apesar da potência maior.
        "lmp2" => ParametrosFisicos {
            velocidade_media_kmh: 195.0,
            duracao_corrida_min: 60.0,
            km_treino_quali: 211.0,
            consumo_l_por_km: 0.68,
            jogos_de_pneu_fixos: 1.2,
            km_por_jogo_de_corrida: 108.0,
            preco_do_jogo_de_pneu: 2_800.0,
            revisao_por_km: 2.50,
            comitiva: 22.0,
            equipe_fixa: 30.0,
            tarifa_frete_por_km: 5.70,
            taxa_de_inscricao: 5_000.0,
            noites_de_hotel: 3.0,
        },

        // ── Tier 6 — Endurance (multi-classe) ────────────────────────────────────────
        // A prova não tem duração fixa em `constants::categories` (`duracao_corrida_min:
        // 0`): o calendário sorteia entre 120, 180, 240 e 360 minutos
        // (`calendar::montagem::resolve_race_duration`). A média aritmética desses quatro
        // é 225 minutos, e é ela que ancora a tabela — uma etapa concreta escala pela
        // distância que os carros de fato rodaram.
        //
        // O fim de semana também é maior: treinos longos, quali por classe e, em prova de
        // 4 horas, um warm-up. ~90 minutos fora da corrida.
        "endurance:gt4" => ParametrosFisicos {
            velocidade_media_kmh: 160.0,
            duracao_corrida_min: 225.0,
            km_treino_quali: 240.0,
            consumo_l_por_km: 0.60,
            // Prova longa consome jogo por stint, não por sessão.
            jogos_de_pneu_fixos: 1.5,
            km_por_jogo_de_corrida: 133.0,
            preco_do_jogo_de_pneu: 1_900.0,
            revisao_por_km: 1.10,
            // Endurance exige turnos: mecânico dobra, entra fisioterapeuta e cozinha.
            comitiva: 16.0,
            equipe_fixa: 14.0,
            tarifa_frete_por_km: 3.50,
            // Inscrição de prova longa é múltiplos de uma etapa de sprint.
            taxa_de_inscricao: 5_500.0,
            noites_de_hotel: 4.0,
        },
        "endurance:gt3" => ParametrosFisicos {
            velocidade_media_kmh: 175.0,
            duracao_corrida_min: 225.0,
            km_treino_quali: 262.0,
            consumo_l_por_km: 0.72,
            jogos_de_pneu_fixos: 2.0,
            km_por_jogo_de_corrida: 109.0,
            preco_do_jogo_de_pneu: 2_400.0,
            revisao_por_km: 1.90,
            comitiva: 22.0,
            equipe_fixa: 24.0,
            tarifa_frete_por_km: 5.20,
            taxa_de_inscricao: 7_000.0,
            noites_de_hotel: 4.0,
        },
        "endurance:lmp2" => ParametrosFisicos {
            velocidade_media_kmh: 195.0,
            duracao_corrida_min: 225.0,
            km_treino_quali: 293.0,
            consumo_l_por_km: 0.68,
            jogos_de_pneu_fixos: 2.0,
            km_por_jogo_de_corrida: 104.0,
            preco_do_jogo_de_pneu: 2_800.0,
            revisao_por_km: 2.50,
            comitiva: 26.0,
            equipe_fixa: 30.0,
            tarifa_frete_por_km: 6.10,
            taxa_de_inscricao: 9_000.0,
            noites_de_hotel: 4.0,
        },

        // Divisão desconhecida cai no meio da escada, como `category_finance_scale` já
        // fazia — nunca em zero, para não fabricar uma etapa de graça por erro de dado.
        _ => parametros("bmw_m2", None),
    }
}

/// Voltas de uma prova completa da divisão na pista média do jogo.
pub fn voltas_de_referencia(categoria: &str, classe: Option<&str>) -> f64 {
    parametros(categoria, classe)
        .voltas_de_referencia(super::tipos::PistaDaEtapa::default().comprimento_km)
}

/// Quantas etapas a temporada da categoria tem. Vem de `constants::categories`, que é a
/// única fonte de verdade do formato do campeonato — a economia não redeclara calendário.
pub fn etapas_por_temporada(categoria: &str) -> f64 {
    get_category_config(categoria)
        .map(|c| c.corridas_por_temporada as f64)
        .unwrap_or(10.0)
}

/// Todas as divisões competitivas da tabela, em ordem de escada. É o que os testes e o
/// relatório varrem — se uma divisão entrar no jogo e não entrar aqui, o teste de
/// cobertura acusa.
pub const DIVISOES: [(&str, Option<&str>); 14] = [
    ("mazda_rookie", None),
    ("toyota_rookie", None),
    ("mazda_amador", None),
    ("toyota_amador", None),
    ("bmw_m2", None),
    ("production_challenger", Some("mazda")),
    ("production_challenger", Some("toyota")),
    ("production_challenger", Some("bmw")),
    ("gt4", None),
    ("gt3", None),
    ("lmp2", None),
    ("endurance", Some("gt4")),
    ("endurance", Some("gt3")),
    ("endurance", Some("lmp2")),
];
