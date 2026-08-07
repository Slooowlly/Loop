//! Entrada e saída da economia — structs PUROS.
//!
//! Nada aqui conhece `rusqlite`, `AppHandle` ou o schema do save. Quem chama monta a
//! entrada a partir do que já tem em mãos (equipe, pista, resultado da corrida) e recebe
//! de volta uma fatura itemizada. Isso é o que permite o harness rodar o modelo novo e o
//! velho lado a lado sem banco nenhum.
//!
//! A diferença de fundo para a fatura antiga (hoje congelada em
//! `commands::race::tests::despesa_legada`) está na SAÍDA: lá uma
//! linha era só um `f64` — o peso de um orçamento abstrato vestindo o nome de um objeto
//! físico. Aqui toda linha carrega `quantidade`, `unidade` e `preco_unitario`, e o `total`
//! é o produto dos dois. O rótulo e o número contam a mesma história porque o número É a
//! conta que o rótulo descreve.

/// Bloco da fatura — o agrupamento que a tela do pós-corrida mostra.
///
/// São três porque o quarto bloco do desenho (RECEITA) não é despesa e mora em
/// `economia::receita`, que ainda não existe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bloco {
    /// O que o carro consumiu para andar: combustível, pneus, desgaste mecânico.
    Corrida,
    /// O que custou levar a operação até a pista: frete, viagem, estadia, inscrição.
    Logistica,
    /// O que custou manter gente na etapa: diárias do fim de semana.
    Equipe,
    /// O que custa manter a operação de pé o ano inteiro, tenha corrida ou não: folha
    /// técnica, sede, frota, seguros e os contratos que só existem acima de certo degrau.
    /// Não aparece na fatura da etapa — é o bloco de [`crate::economia::temporada`].
    Estrutura,
}

impl Bloco {
    /// Token estável para i18n e para o ledger. Nunca é texto de UI.
    pub fn chave(self) -> &'static str {
        match self {
            Bloco::Corrida => "corrida",
            Bloco::Logistica => "logistica",
            Bloco::Equipe => "equipe",
            Bloco::Estrutura => "estrutura",
        }
    }
}

/// Em que a `quantidade` de uma linha está medida.
///
/// Existe para que um teste possa afirmar a grandeza física, não só o dinheiro: "a linha
/// de combustível pediu entre 30 e 90 litros" é uma asserção que a fatura antiga não
/// permitia fazer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unidade {
    Litro,
    JogoDePneu,
    /// Quilômetro rodado pelos carros (revisão mecânica amortizada).
    Km,
    /// Quilômetro faturado pela transportadora (ida e volta, já degressivo).
    KmDeFrete,
    Pessoa,
    PessoaNoite,
    PessoaDia,
    /// Uma inscrição por carro.
    Carro,
    /// Uma pessoa na folha por um ano.
    PessoaAno,
    /// Um ano de um contrato ou de um ativo — a unidade dos recorrentes de temporada.
    Ano,
    /// **Sem grandeza física.** É o valor pelo valor: um canal de receita, uma compra
    /// avulsa. Existe para essas linhas não terem que mentir uma unidade — escrever
    /// "1 ano × $9.162" embaixo do prêmio de uma corrida é a regra 2 do redesign sendo
    /// quebrada dentro do próprio expandir que existe para cumpri-la.
    Valor,
}

impl Unidade {
    pub fn chave(self) -> &'static str {
        match self {
            Unidade::Litro => "litro",
            Unidade::JogoDePneu => "jogo_de_pneu",
            Unidade::Km => "km",
            Unidade::KmDeFrete => "km_de_frete",
            Unidade::Pessoa => "pessoa",
            Unidade::PessoaNoite => "pessoa_noite",
            Unidade::PessoaDia => "pessoa_dia",
            Unidade::Carro => "carro",
            Unidade::PessoaAno => "pessoa_ano",
            Unidade::Ano => "ano",
            Unidade::Valor => "valor",
        }
    }
}

/// Uma linha da fatura. `total` é sempre `quantidade × preco_unitario` — a construção
/// passa por [`LinhaDaFatura::nova`], que não deixa os três se contradizerem.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinhaDaFatura {
    /// Token da linha (`combustivel`, `pneus`, `frete`, ...).
    pub chave: &'static str,
    pub bloco: Bloco,
    pub quantidade: f64,
    pub unidade: Unidade,
    pub preco_unitario: f64,
}

impl LinhaDaFatura {
    pub fn nova(
        chave: &'static str,
        bloco: Bloco,
        quantidade: f64,
        unidade: Unidade,
        preco_unitario: f64,
    ) -> Self {
        Self {
            chave,
            bloco,
            quantidade: quantidade.max(0.0),
            unidade,
            preco_unitario: preco_unitario.max(0.0),
        }
    }

    /// O dinheiro da linha. Não é um campo justamente para não poder divergir da conta.
    pub fn total(&self) -> f64 {
        self.quantidade * self.preco_unitario
    }
}

/// A fatura de UMA etapa para UMA equipe.
#[derive(Debug, Clone, PartialEq)]
pub struct FaturaDaEtapa {
    pub linhas: Vec<LinhaDaFatura>,
}

impl FaturaDaEtapa {
    pub fn total(&self) -> f64 {
        self.linhas.iter().map(LinhaDaFatura::total).sum()
    }

    pub fn total_do_bloco(&self, bloco: Bloco) -> f64 {
        self.linhas
            .iter()
            .filter(|l| l.bloco == bloco)
            .map(LinhaDaFatura::total)
            .sum()
    }

    /// Busca uma linha por token. `None` quando a etapa não gerou aquela linha.
    pub fn linha(&self, chave: &str) -> Option<&LinhaDaFatura> {
        self.linhas.iter().find(|l| l.chave == chave)
    }

    /// Atalho de teste e de relatório: o dinheiro de uma linha, 0 se ela não existe.
    pub fn valor(&self, chave: &str) -> f64 {
        self.linha(chave).map(LinhaDaFatura::total).unwrap_or(0.0)
    }
}

/// A equipe do ponto de vista da etapa. Só o que muda a conta física.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EquipeNaEtapa {
    /// Quantos carros a equipe leva. No Loop é `pilotos_por_equipe`, mas entra explícito
    /// porque uma equipe pode largar com um carro só (piloto lesionado, carro destruído).
    pub carros_inscritos: u32,
    /// Instalações (0–100). Estrutura maior viaja com mais volume: mexe no frete.
    pub instalacoes: f64,
    /// Qualidade do pit crew (0–100) — proxy do tamanho da comitiva que a equipe leva.
    pub qualidade_pit_crew: f64,
}

impl Default for EquipeNaEtapa {
    /// A equipe mediana: dois carros, estrutura e crew no meio da escala. É o ponto em
    /// que a tabela de âncoras foi escrita, então é também o ponto de calibração.
    fn default() -> Self {
        Self {
            carros_inscritos: 2,
            instalacoes: 50.0,
            qualidade_pit_crew: 50.0,
        }
    }
}

/// A pista da etapa. Comprimento vem de `constants::tracks`; a distância é a viagem que a
/// equipe precisa fazer para chegar lá.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PistaDaEtapa {
    pub comprimento_km: f64,
    /// Distância SÓ DE IDA entre a sede da equipe e a pista. Ida e volta é conta do
    /// módulo, não do chamador.
    pub distancia_da_sede_km: f64,
}

/// Distâncias de referência quando o chamador não tem a geografia em mãos. Batem, em
/// ordem de grandeza, com o que o `travel_factor` da fatura antiga classificava como
/// casa / mesmo continente / outro continente.
pub const DISTANCIA_CASA_KM: f64 = 220.0;
pub const DISTANCIA_CONTINENTAL_KM: f64 = 1_250.0;
pub const DISTANCIA_INTERCONTINENTAL_KM: f64 = 8_500.0;

impl Default for PistaDaEtapa {
    /// Pista média do jogo: 4,586 km é a média aritmética dos `comprimento_km` das 107
    /// pistas de `constants/tracks/dados.rs`. Distância continental.
    fn default() -> Self {
        Self {
            comprimento_km: 4.586,
            distancia_da_sede_km: DISTANCIA_CONTINENTAL_KM,
        }
    }
}

/// O que os carros da equipe efetivamente fizeram na corrida. É o que faz a etapa não
/// custar igual toda vez.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DesempenhoNaEtapa {
    /// Média de voltas completadas pelos carros da equipe.
    pub voltas_completadas: f64,
    /// Distância da prova em voltas (o que o vencedor completou).
    pub voltas_da_prova: f64,
    /// Desgaste final médio dos pneus (0..1). Pneu castigado significa mais borracha
    /// gasta no fim de semana.
    pub desgaste_final_pneus: f64,
}

impl DesempenhoNaEtapa {
    /// Fração da prova que a equipe completou (0..1). Abandono cedo queima menos.
    pub fn fracao_completada(&self) -> f64 {
        if self.voltas_da_prova <= 0.0 {
            return 1.0;
        }
        (self.voltas_completadas / self.voltas_da_prova).clamp(0.0, 1.0)
    }
}

/// Tudo que a fatura de uma etapa precisa saber. Sem `Connection`, sem `AppHandle`.
#[derive(Debug, Clone, PartialEq)]
pub struct EntradaDaEtapa {
    /// Id do campeonato (`gt3`, `endurance`, ...).
    pub categoria: String,
    /// Classe dentro do campeonato multi-classe (`gt3` dentro de `endurance`). `None`
    /// nas categorias de classe única.
    pub classe: Option<String>,
    /// Duração REAL da prova, em minutos.
    ///
    /// Nas categorias de sprint isto é sempre a `duracao_corrida_min` de
    /// `constants::categories`. No endurance não é: aquela constante vale 0 e o calendário
    /// sorteia entre 120, 180, 240 e 360 minutos
    /// (`calendar::montagem::resolve_race_duration`). Uma prova de 6 horas gasta mais
    /// borracha e traz um programa de treinos maior que uma de 2 — congelar isso na média
    /// da categoria, como a primeira versão deste módulo fazia, apagava a diferença entre
    /// as duas etapas mais diferentes do calendário.
    pub duracao_corrida_min: f64,
    pub equipe: EquipeNaEtapa,
    pub pista: PistaDaEtapa,
    pub corrida: DesempenhoNaEtapa,
}

impl EntradaDaEtapa {
    /// Uma etapa TÍPICA: equipe mediana, pista média, distância continental, prova
    /// completa na duração de referência da divisão, com desgaste normal. O ponto de
    /// referência dos testes e do relatório.
    pub fn tipica(categoria: &str, classe: Option<&str>) -> Self {
        let duracao = super::ancora::parametros(categoria, classe).duracao_corrida_min;
        Self::tipica_com_duracao(categoria, classe, duracao)
    }

    /// A mesma etapa típica, numa prova de duração escolhida. É por aqui que uma etapa de
    /// 6 horas do endurance passa a custar diferente de uma de 2.
    pub fn tipica_com_duracao(categoria: &str, classe: Option<&str>, duracao_min: f64) -> Self {
        let pista = PistaDaEtapa::default();
        let voltas = super::ancora::parametros(categoria, classe)
            .voltas_em(duracao_min, pista.comprimento_km);
        Self {
            categoria: categoria.to_string(),
            classe: classe.map(str::to_string),
            duracao_corrida_min: duracao_min,
            equipe: EquipeNaEtapa::default(),
            pista,
            corrida: DesempenhoNaEtapa {
                voltas_completadas: voltas,
                voltas_da_prova: voltas,
                // Desgaste com que um carro volta de uma prova normal — o mesmo 0,65 que
                // a fatura antiga usava como ponto médio.
                desgaste_final_pneus: 0.65,
            },
        }
    }
}
