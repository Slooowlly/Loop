//! A fatura que o JOGADOR lê — a apresentação, não o modelo.
//!
//! É o pedaço visível do redesign inteiro, e é o que originou tudo: a queixa era
//! "gasolina e pneu no debriefing, é absurdo os números". O modelo por trás já é
//! honesto (`evento`, `temporada`, `receita` contam litros, jogos de pneu e pessoas);
//! o que falta é a prestação de contas.
//!
//! **Agrega, não simplifica.** O modelo interno segue com as suas ~20 linhas, cada uma
//! com quantidade física e preço unitário — ele alimenta o comportamento da IA e o
//! harness. Aqui as linhas são somadas em quatro blocos, e cada linha visível carrega
//! no `detalhe` exatamente as linhas internas que a compõem. Nada é descartado no
//! caminho: `total()` de uma linha visível é a soma dos detalhes dela, e o teste
//! garante isso.
//!
//! Forma travada pela decisão 4 da Parte 5 do redesign, com o rateio removido pela
//! decisão 10 e a peça comprada acrescentada — 4 blocos, 8 linhas de despesa (§3.7):
//!
//! ```text
//! CORRIDA      combustível · pneus · revisão mecânica · peça de reposição
//! LOGÍSTICA    frete · viagem e estadia · inscrição
//! EQUIPE       diárias do fim de semana
//! RECEITA      prêmio da etapa · bônus por resultado · bilheteria · patrocínio
//!
//! rodapé       custo fixo do ano (folha, sede, frota, seguros…)
//! ```
//!
//! ## Por que a peça entrou (e por que ela não é rodapé)
//!
//! A compra de peça sai do caixa na mesma rodada, por `technical_investment_cost`, e não
//! aparecia em lugar nenhum — a fatura reconciliava com o `event_operations_cost` e mentia
//! sobre a saída de caixa da etapa. Medido em `commands::race::despesa`: **92–100% das
//! rodadas têm compra**, e do GT4 para cima ela vale, em média, mais que a fatura visível
//! inteira (GT3 em 151%, 333% na pior rodada). Rodapé para o maior item da página é letra
//! miúda.
//!
//! A linha nova e a `revisão mecânica` não compartilham nenhuma palavra de propósito: uma é
//! o desgaste amortizado por quilômetro, a outra é a troca. Se os rótulos convergissem, o
//! jogador somaria as duas.
//!
//! ## Por que o rateio saiu (decisão 10)
//!
//! Ele estava na fatura da etapa como oitava linha e ocupava **70–84% da tela** em toda a
//! escada — medido em `economia::tests::fatura`. As sete linhas físicas que o redesign
//! passou semanas tornando honestas somavam 16–30% do que o jogador lia, e uma única linha
//! agregada carregava o resto.
//!
//! O argumento não é de proporção, é de veracidade: **folha e sede não variam por
//! corrida**. Mostrá-las por corrida é a mesma falsa precisão que este redesign existe para
//! remover — um número que muda de etapa para etapa só porque o calendário tem 14 e não 10
//! rodadas. O custo fixo vira rodapé na etapa (o ano inteiro, dito uma vez) e linha de
//! verdade no fechamento de temporada, onde ele de fato acontece.
//!
//! Efeito colateral bom: a despesa visível da etapa passa a ser exatamente o
//! `event_operations_cost` que saiu do caixa naquela rodada. A fatura reconcilia com o
//! ledger sem ajuste.
//!
//! ## As duas regras que a §3.3.3 travou
//!
//! **1. Linha aparece ou some, nunca zera.** Uma equipe de Rookie não tem simulador
//! barato — ela não tem simulador, e a linha não existe na fatura dela. O mesmo vale
//! para qualquer linha que dê zero: uma etapa em que o carro não largou não gera linha
//! de combustível de meio tanque, gera fatura sem linha de combustível. É isto que faz
//! a fatura dizer em que altura da pirâmide a equipe está: a fatura de uma GT3 tem
//! linhas que a de uma Rookie simplesmente não tem.
//!
//! **2. O rótulo e o número contam a mesma história.** Se a linha diz "combustível", o
//! valor tem que ser o que caberia no tanque. Este módulo não conserta ordem de
//! grandeza — ele só não pode estragá-la, e por isso nunca multiplica, redistribui nem
//! arredonda: ele soma. A régua de voz alta está em
//! `economia::tests::fatura` (`relatorio_da_fatura_visivel`).
//!
//! Sem dependência de Tauri, sem banco, sem I/O. Nada aqui é chamado pela simulação
//! ainda — a troca da §3.9 é etapa posterior.

use super::evento::{COMBUSTIVEL, DIARIAS, ESTADIA, FRETE, INSCRICAO, PNEUS, REVISAO, VIAGEM};
use super::receita::ReceitaDaEtapa;
use super::temporada::{self, FOLHA_DE_PILOTOS};
use super::tipos::{Bloco, FaturaDaEtapa, LinhaDaFatura, Unidade};

// ── Tokens das linhas VISÍVEIS ───────────────────────────────────────────────────────
//
// São tokens de i18n, nunca texto de UI. Alguns coincidem com os tokens do modelo
// interno (`combustivel`) porque a linha visível é a linha interna sozinha; outros são
// novos (`viagem_e_estadia`, `rateio_da_folha_fixa`) porque agregam mais de uma.
pub const V_COMBUSTIVEL: &str = "combustivel";
pub const V_PNEUS: &str = "pneus";
// ── As duas linhas de peça, que NÃO podem convergir ─────────────────────────────────
//
// São despesas diferentes e o jogador soma as duas se os rótulos se parecerem. Por isso
// nenhuma palavra é compartilhada entre elas:
//
// - `revisao_mecanica` é a revisão amortizada por quilômetro. Existe em TODA etapa, é
//   pequena e acompanha o quanto o carro rodou. É o token `revisao` do modelo físico.
// - `peca_de_reposicao` é a COMPRA — a peça que o cérebro de manutenção trocou nesta
//   rodada. Aparece em 92–100% das rodadas e, do GT4 para cima, vale em média mais que
//   toda a fatura visível junta (GT3: 151% na média, 333% na pior rodada — medido em
//   `commands::race::despesa`).
//
// O nome anterior da primeira era `desgaste_de_peca`, e "desgaste de peça" ao lado de
// "peça de reposição" é exatamente a convergência que se quer evitar: as duas falariam
// de peça, e a diferença entre gastar e comprar ficaria por conta do jogador.
pub const V_REVISAO_MECANICA: &str = "revisao_mecanica";
pub const V_PECA_DE_REPOSICAO: &str = "peca_de_reposicao";
pub const V_FRETE: &str = "frete";
pub const V_VIAGEM_E_ESTADIA: &str = "viagem_e_estadia";
pub const V_INSCRICAO: &str = "inscricao";
pub const V_DIARIAS: &str = "diarias";
/// O rodapé. Não é linha da etapa — ver a decisão 10 no topo do módulo.
pub const V_CUSTO_FIXO_DO_ANO: &str = "custo_fixo_do_ano";
pub const V_PREMIO_DA_ETAPA: &str = "premio_da_etapa";
/// O canal que `ReceitaDaEtapa::volta_mais_rapida` carrega.
///
/// O token de TELA não repete o nome do campo de propósito. Em produção esse canal é o
/// `result_bonus` do ledger — pontos, vitória, pódio e top-5 somados —, e escrever "volta
/// mais rápida" ao lado desse número seria um rótulo que o número não cumpre. É a regra 2
/// do módulo aplicada a mim mesmo.
pub const V_BONUS_POR_RESULTADO: &str = "bonus_por_resultado";
pub const V_BILHETERIA: &str = "bilheteria";
pub const V_PATROCINIO: &str = "patrocinio";

/// Quantas linhas de DESPESA a fatura pode mostrar. Uma fatura pode ter menos (linha que
/// zera some), nunca mais.
///
/// A aritmética, porque o número já mudou duas vezes:
///
/// | | linhas |
/// |---|---:|
/// | decisão 4 — a forma original | 8 |
/// | decisão 10 — o rateio vira rodapé | −1 → **7** |
/// | a peça comprada ganha linha | +1 → **8** |
///
/// O bloco de CONSERTO fica fora da conta: ele só existe depois de uma batida e tem uma
/// linha por peça danificada (até seis, em `manutencao::damage_split`), então não cabe num
/// teto fixo. Quem monta a tela trata reparo como bloco à parte, que é o que
/// `career_types::StageInvoiceDto` já faz.
pub const MAXIMO_DE_LINHAS_DE_DESPESA: usize = 8;

/// Os quatro blocos da tela. Não é o [`Bloco`] do modelo interno: aquele tem
/// `Estrutura` (o recorrente anual, que aqui vira o rateio dentro de EQUIPE) e não tem
/// receita.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlocoDaFatura {
    Corrida,
    Logistica,
    Equipe,
    Receita,
}

impl BlocoDaFatura {
    /// Token estável para i18n. Nunca é texto de UI.
    pub fn chave(self) -> &'static str {
        match self {
            BlocoDaFatura::Corrida => "corrida",
            BlocoDaFatura::Logistica => "logistica",
            BlocoDaFatura::Equipe => "equipe",
            BlocoDaFatura::Receita => "receita",
        }
    }

    /// A ordem em que os blocos aparecem na tela. Despesa primeiro, receita por último:
    /// a fatura é uma prestação de contas, e o saldo é a última coisa que se lê.
    pub fn ordem(self) -> u8 {
        match self {
            BlocoDaFatura::Corrida => 0,
            BlocoDaFatura::Logistica => 1,
            BlocoDaFatura::Equipe => 2,
            BlocoDaFatura::Receita => 3,
        }
    }

    pub fn e_receita(self) -> bool {
        matches!(self, BlocoDaFatura::Receita)
    }
}

/// Uma linha do modelo interno, preservada para o expandir. Quantidade e preço vêm
/// intactos — é o que permite ao jogador ver "148 litros × $1,42" atrás de um número.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Detalhe {
    pub chave: &'static str,
    pub quantidade: f64,
    pub unidade: Unidade,
    pub preco_unitario: f64,
}

impl Detalhe {
    pub fn total(&self) -> f64 {
        self.quantidade * self.preco_unitario
    }
}

impl From<&LinhaDaFatura> for Detalhe {
    fn from(l: &LinhaDaFatura) -> Self {
        Detalhe {
            chave: l.chave,
            quantidade: l.quantidade,
            unidade: l.unidade,
            preco_unitario: l.preco_unitario,
        }
    }
}

/// Uma linha VISÍVEL. O total é sempre derivado dos detalhes — não é um campo
/// independente justamente para não poder divergir do que está atrás dele.
#[derive(Debug, Clone, PartialEq)]
pub struct LinhaVisivel {
    pub chave: &'static str,
    pub bloco: BlocoDaFatura,
    pub detalhe: Vec<Detalhe>,
    /// Por quantas etapas o detalhe se divide. 1,0 em tudo que a etapa consumiu de
    /// fato; o número de etapas do campeonato na linha de rateio.
    ///
    /// Existe para o detalhe poder mostrar o contrato COMO ELE É — "4 pessoas ×
    /// $28.000/ano" — em vez de um preço unitário já diluído, que diria
    /// "$5.600/pessoa-ano" e seria falso: ninguém ganha isso por ano. A regra do
    /// redesign é que rótulo e número contem a mesma história, e ela vale dentro do
    /// expandir também.
    pub divisor: f64,
}

impl LinhaVisivel {
    /// A soma do que está atrás da linha, sem rateio — o valor ANUAL na linha de
    /// rateio, o valor da etapa em todas as outras.
    pub fn total_do_detalhe(&self) -> f64 {
        self.detalhe.iter().map(Detalhe::total).sum()
    }

    pub fn total(&self) -> f64 {
        self.total_do_detalhe() / self.divisor.max(1.0)
    }

    /// A linha é um rateio de algo maior que a etapa?
    pub fn e_rateio(&self) -> bool {
        self.divisor > 1.0
    }

    /// **Há o que mostrar atrás desta linha?** É o que diz à UI se abre o expandir.
    ///
    /// A pergunta certa não é "quantos detalhes tem" — é se algum deles carrega GRANDEZA
    /// FÍSICA. "Combustível $182" tem um detalhe só e é justamente o caso em que o
    /// expandir importa mais: "49,5 L × $3,67" é o que responde a "esse número é
    /// absurdo?", que é a pergunta que originou o redesign. Já "Patrocínio $37.168" também
    /// tem um detalhe só, mas ele é [`Unidade::Valor`] — dinheiro sem grandeza atrás —, e
    /// mostrá-lo escreveria a linha duas vezes.
    ///
    /// Uma versão anterior contava detalhes (`len() > 1 || e_rateio()`) e escondia o
    /// expandir de combustível, pneus e revisão. Só apareceu olhando a tela.
    pub fn tem_detalhe(&self) -> bool {
        if self.e_rateio() || self.detalhe.len() > 1 {
            return true;
        }
        self.detalhe
            .first()
            .is_some_and(|d| d.unidade != Unidade::Valor)
    }
}

/// A fatura de uma etapa como o jogador a lê.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FaturaVisivel {
    pub linhas: Vec<LinhaVisivel>,
    /// O RODAPÉ: o custo fixo do ano inteiro — folha técnica, sede, frota, seguros,
    /// simulador. **Não é linha da etapa** e não entra em [`Self::total_de_despesa`];
    /// aparece no pé da fatura como contexto e vira linha de verdade no fechamento de
    /// temporada. Ver a decisão 10 no topo do módulo.
    ///
    /// O `divisor` dela é o número de etapas do campeonato, então `total()` continua
    /// devolvendo a fatia da etapa quando o fechamento de temporada precisar dela, e
    /// `total_do_detalhe()` devolve o ano inteiro, que é o que o rodapé diz.
    ///
    /// `None` quando a divisão não tem recorrente nenhum — a regra 1 vale aqui também.
    pub custo_fixo_do_ano: Option<LinhaVisivel>,
}

impl FaturaVisivel {
    pub fn linhas_do_bloco(&self, bloco: BlocoDaFatura) -> impl Iterator<Item = &LinhaVisivel> {
        self.linhas.iter().filter(move |l| l.bloco == bloco)
    }

    pub fn total_do_bloco(&self, bloco: BlocoDaFatura) -> f64 {
        self.linhas_do_bloco(bloco).map(LinhaVisivel::total).sum()
    }

    pub fn linha(&self, chave: &str) -> Option<&LinhaVisivel> {
        self.linhas.iter().find(|l| l.chave == chave)
    }

    /// O dinheiro de uma linha, 0 se ela não existe. Atalho de teste e de relatório —
    /// **não** é o que a UI deve usar para decidir se mostra a linha: linha que não
    /// existe não vira zero na tela, ela some.
    pub fn valor(&self, chave: &str) -> f64 {
        self.linha(chave).map(LinhaVisivel::total).unwrap_or(0.0)
    }

    pub fn total_de_despesa(&self) -> f64 {
        self.linhas
            .iter()
            .filter(|l| !l.bloco.e_receita())
            .map(LinhaVisivel::total)
            .sum()
    }

    pub fn total_de_receita(&self) -> f64 {
        self.total_do_bloco(BlocoDaFatura::Receita)
    }

    /// O saldo da etapa. Positivo = a etapa se pagou.
    pub fn resultado(&self) -> f64 {
        self.total_de_receita() - self.total_de_despesa()
    }

    pub fn linhas_de_despesa(&self) -> usize {
        self.linhas.iter().filter(|l| !l.bloco.e_receita()).count()
    }

    /// O custo fixo do ANO inteiro — o número que o rodapé diz. 0 se a divisão não tem
    /// recorrente.
    pub fn custo_fixo_anual(&self) -> f64 {
        self.custo_fixo_do_ano
            .as_ref()
            .map(LinhaVisivel::total_do_detalhe)
            .unwrap_or(0.0)
    }

    /// A fatia do custo fixo que caberia a ESTA etapa. Não é cobrada aqui — é o que o
    /// fechamento de temporada distribui —, mas é o número que responde "quanto disso é
    /// meu neste fim de semana?".
    pub fn custo_fixo_por_etapa(&self) -> f64 {
        self.custo_fixo_do_ano
            .as_ref()
            .map(LinhaVisivel::total)
            .unwrap_or(0.0)
    }

    /// Reescala todos os preços unitários da DESPESA por um fator, preservando as
    /// quantidades físicas.
    ///
    /// Existe porque a produção multiplica a fatura por um modificador de conjuntura
    /// (`economy_cost_modifier`) e depois normaliza ao que de fato saiu do caixa. Aplicar
    /// isso ao TOTAL da linha quebraria o expandir — "173 L × $3,40" deixaria de dar o
    /// número ao lado, que é exatamente o defeito que este módulo existe para não
    /// cometer. Preço é o lugar certo: conjuntura mexe em preço, não em litro.
    ///
    /// Reancora cada linha de DESPESA no que de fato saiu do caixa, mexendo só no preço.
    ///
    /// A quem cabe cada coisa: o **ledger** é dono do dinheiro (é ele que registra o que a
    /// rodada debitou, com o modificador de conjuntura já dentro) e este módulo é dono da
    /// **física** (litros, jogos de pneu, pessoas-noite). A fatura da tela precisa das
    /// duas, e recalcular o dinheiro aqui criaria um segundo lugar onde a conta pode
    /// divergir — que é o defeito que este redesign inteiro existe para remover.
    ///
    /// O ajuste vai para o preço unitário e nunca para o total, pelo mesmo motivo de
    /// sempre: se o total andasse sozinho, "173 L × $3,40" deixaria de dar o número ao
    /// lado. Linha sem valor autoritativo fica como está.
    pub fn reancorar_despesa(&mut self, autoritativo: impl Fn(&str) -> Option<f64>) {
        for linha in self.linhas.iter_mut().filter(|l| !l.bloco.e_receita()) {
            let Some(alvo) = autoritativo(linha.chave) else {
                continue;
            };
            let atual = linha.total();
            if atual > 0.0 && alvo >= 0.0 {
                let fator = alvo / atual;
                for d in linha.detalhe.iter_mut() {
                    d.preco_unitario *= fator;
                }
            }
        }
        self.linhas.retain(|l| l.total() > 0.0);
    }

    /// A receita não é tocada: um modificador de CUSTO não move prêmio nem patrocínio.
    pub fn com_precos_de_despesa_escalados(mut self, fator: f64) -> Self {
        let fator = fator.max(0.0);
        for linha in self.linhas.iter_mut().filter(|l| !l.bloco.e_receita()) {
            for d in linha.detalhe.iter_mut() {
                d.preco_unitario *= fator;
            }
        }
        if let Some(rodape) = self.custo_fixo_do_ano.as_mut() {
            for d in rodape.detalhe.iter_mut() {
                d.preco_unitario *= fator;
            }
        }
        self
    }
}

/// O que a fatura visível precisa. Tudo já produzido pelos módulos do modelo — este
/// arquivo não calcula economia nenhuma, ele apresenta a que já existe.
#[derive(Debug, Clone, Copy)]
pub struct EntradaDaFatura<'a> {
    /// A fatura da etapa, de [`crate::economia::evento::fatura_da_etapa`].
    pub etapa: &'a FaturaDaEtapa,
    /// Os recorrentes do ano, de [`crate::economia::temporada::fatura_de_temporada`].
    pub temporada: &'a FaturaDaEtapa,
    /// Quantas etapas o campeonato tem — o divisor do rateio do anual.
    pub etapas_na_temporada: f64,
    /// Os quatro canais por-etapa, de [`crate::economia::receita::receita_da_etapa`].
    pub receita: ReceitaDaEtapa,
    /// A PEÇA COMPRADA nesta rodada — o `technical_investment_cost` do ledger, que no
    /// modelo físico é a compra e nada mais.
    ///
    /// Entra por fora porque não é conta física de etapa: é uma decisão do cérebro de
    /// manutenção, tomada e debitada num caminho próprio. `0.0` quando não houve troca, e
    /// aí a linha não existe (regra 1).
    ///
    /// **É o maior item da página do GT4 para cima**: 92–100% das rodadas têm compra e,
    /// na GT3, a peça vale em média 151% de toda a fatura visível — 333% na pior rodada.
    /// Foi por isso que ela deixou de ser rodapé.
    pub peca_comprada: f64,
    /// Folha ANUAL real dos contratos dos pilotos desta equipe, quando o chamador a
    /// conhece. A linha `folha_de_pilotos` de `temporada` é um valor de REFERÊNCIA de
    /// dupla mediana (existe para a âncora cobrir o mesmo escopo da tabela antiga) e
    /// não é o que a simulação debita — mostrá-la ao jogador seria escrever na tela um
    /// salário que ninguém recebe. Com `Some`, substitui a referência.
    pub folha_de_pilotos_anual: Option<f64>,
}

/// Monta a fatura visível: agrega o modelo interno em 4 blocos e no máximo
/// [`MAXIMO_DE_LINHAS_DE_DESPESA`] linhas de despesa, preservando o detalhe.
///
/// Linha que somaria zero não entra — ver a regra 1 no topo do módulo.
pub fn fatura_visivel(entrada: &EntradaDaFatura<'_>) -> FaturaVisivel {
    let mut linhas: Vec<LinhaVisivel> = Vec::with_capacity(12);

    // ── CORRIDA: o que o carro consumiu para andar ───────────────────────────────────
    // Três linhas, cada uma sozinha. O rótulo "desgaste de peça" é a revisão amortizada
    // por quilômetro — o nome de tela do que o modelo chama `revisao`.
    empurra(
        &mut linhas,
        V_COMBUSTIVEL,
        BlocoDaFatura::Corrida,
        detalhes(entrada.etapa, &[COMBUSTIVEL]),
    );
    empurra(
        &mut linhas,
        V_PNEUS,
        BlocoDaFatura::Corrida,
        detalhes(entrada.etapa, &[PNEUS]),
    );
    empurra(
        &mut linhas,
        V_REVISAO_MECANICA,
        BlocoDaFatura::Corrida,
        detalhes(entrada.etapa, &[REVISAO]),
    );
    // A peça COMPRADA. Sem detalhe físico: o ledger guarda o dinheiro da troca, não
    // quantas peças nem de que tipo — e inventar "1 × $X" para ela seria escrever uma
    // grandeza que não existe.
    empurra(
        &mut linhas,
        V_PECA_DE_REPOSICAO,
        BlocoDaFatura::Corrida,
        vec![valor_sem_grandeza(V_PECA_DE_REPOSICAO, entrada.peca_comprada)],
    );

    // ── LOGÍSTICA: o que custou levar a operação até a pista ─────────────────────────
    // Viagem e estadia viram UMA linha: são a mesma decisão (mandar N pessoas para
    // longe por M noites) e separá-las gasta duas das oito linhas com a mesma história.
    empurra(
        &mut linhas,
        V_FRETE,
        BlocoDaFatura::Logistica,
        detalhes(entrada.etapa, &[FRETE]),
    );
    empurra(
        &mut linhas,
        V_VIAGEM_E_ESTADIA,
        BlocoDaFatura::Logistica,
        detalhes(entrada.etapa, &[VIAGEM, ESTADIA]),
    );
    empurra(
        &mut linhas,
        V_INSCRICAO,
        BlocoDaFatura::Logistica,
        detalhes(entrada.etapa, &[INSCRICAO]),
    );

    // ── EQUIPE: gente ────────────────────────────────────────────────────────────────
    // Uma linha só. A folha fixa saiu daqui pela decisão 10 e virou rodapé.
    empurra(
        &mut linhas,
        V_DIARIAS,
        BlocoDaFatura::Equipe,
        detalhes(entrada.etapa, &[DIARIAS]),
    );

    // ── RECEITA ──────────────────────────────────────────────────────────────────────
    // Não tem detalhe físico: são canais, não quantidades.
    for (chave, valor) in [
        (V_PREMIO_DA_ETAPA, entrada.receita.premio_de_corrida),
        (V_BONUS_POR_RESULTADO, entrada.receita.volta_mais_rapida),
        (V_BILHETERIA, entrada.receita.bilheteria),
        (V_PATROCINIO, entrada.receita.patrocinio),
    ] {
        empurra(
            &mut linhas,
            chave,
            BlocoDaFatura::Receita,
            vec![valor_sem_grandeza(chave, valor)],
        );
    }

    linhas.sort_by_key(|l| l.bloco.ordem());

    // ── RODAPÉ: o custo fixo do ano ──────────────────────────────────────────────────
    // Fora das linhas e fora do total da etapa. O divisor guarda o calendário para quem
    // precisar da fatia (o fechamento de temporada), mas o número que o rodapé diz é o
    // ANO, porque é o ano que essa conta descreve.
    let rodape = LinhaVisivel {
        chave: V_CUSTO_FIXO_DO_ANO,
        bloco: BlocoDaFatura::Equipe,
        detalhe: rateio_do_anual(entrada),
        divisor: entrada.etapas_na_temporada.max(1.0),
    };

    FaturaVisivel {
        linhas,
        custo_fixo_do_ano: (rodape.total_do_detalhe() > 0.0).then_some(rodape),
    }
}

/// Um detalhe que é só dinheiro — sem quantidade física atrás dele.
///
/// Serve às linhas que não medem nada: os canais de receita e a peça comprada. A
/// invariante do módulo (`total da linha == soma dos detalhes`) continua valendo, e a
/// unidade [`Unidade::Valor`] diz explicitamente que ali não há grandeza. A versão
/// anterior usava `Unidade::Ano`, o que fazia a tela escrever "1 ano × $9.162" embaixo do
/// prêmio de uma corrida — a regra 2 quebrada dentro do expandir que existe para cumpri-la.
///
/// `tem_detalhe()` continua falso para estas linhas (um detalhe só, divisor 1), então a UI
/// não oferece expandir onde não há o que expandir.
fn valor_sem_grandeza(chave: &'static str, valor: f64) -> Detalhe {
    Detalhe {
        chave,
        quantidade: 1.0,
        unidade: Unidade::Valor,
        preco_unitario: valor.max(0.0),
    }
}

/// Os detalhes de uma linha visível, na ordem em que as chaves foram pedidas. Chave que
/// não existe na fatura interna simplesmente não vira detalhe.
fn detalhes(fatura: &FaturaDaEtapa, chaves: &[&str]) -> Vec<Detalhe> {
    chaves
        .iter()
        .filter_map(|c| fatura.linha(c))
        .map(Detalhe::from)
        .collect()
}

/// Os recorrentes anuais que a linha de rateio dilui.
///
/// Cada linha do bloco `Estrutura` entra com quantidade e preço ANUAIS intactos — o
/// jogador expande e vê "24 pessoas × $38.000/ano", que é o contrato como ele é. A
/// divisão pelas etapas é da linha (`divisor`), não do detalhe: diluir aqui escreveria
/// "$5.600/pessoa-ano" na tela, um salário que ninguém recebe.
///
/// A folha de PILOTOS é substituída pela folha real quando o chamador a conhece; ver
/// [`EntradaDaFatura::folha_de_pilotos_anual`].
fn rateio_do_anual(entrada: &EntradaDaFatura<'_>) -> Vec<Detalhe> {
    entrada
        .temporada
        .linhas
        .iter()
        .filter(|l| l.bloco == Bloco::Estrutura)
        .map(|l| {
            let preco = if l.chave == FOLHA_DE_PILOTOS {
                match entrada.folha_de_pilotos_anual {
                    // A folha real é o total da dupla; a quantidade da linha é o número
                    // de pilotos, então o preço unitário é o total dividido por ela.
                    Some(total) => total.max(0.0) / l.quantidade.max(1.0),
                    None => l.preco_unitario,
                }
            } else {
                l.preco_unitario
            };
            Detalhe {
                chave: l.chave,
                quantidade: l.quantidade,
                unidade: l.unidade,
                // ANUAL, não diluído: quem divide é o `divisor` da linha.
                preco_unitario: preco,
            }
        })
        .filter(|d| d.total() > 0.0)
        .collect()
}

/// Acrescenta a linha só se ela tiver dinheiro. É a regra 1 do módulo, num lugar só:
/// linha que zera não vira "0" na tela, ela some.
fn empurra(
    linhas: &mut Vec<LinhaVisivel>,
    chave: &'static str,
    bloco: BlocoDaFatura,
    detalhe: Vec<Detalhe>,
) {
    empurra_com_divisor(linhas, chave, bloco, detalhe, 1.0);
}

fn empurra_com_divisor(
    linhas: &mut Vec<LinhaVisivel>,
    chave: &'static str,
    bloco: BlocoDaFatura,
    detalhe: Vec<Detalhe>,
    divisor: f64,
) {
    let linha = LinhaVisivel {
        chave,
        bloco,
        detalhe,
        divisor: divisor.max(1.0),
    };
    if linha.total() > 0.0 {
        linhas.push(linha);
    }
}

/// A fatura de temporada de uma divisão, pronta para o rateio, com a equipe mediana.
/// Atalho para quem só quer a fatura visível típica de um degrau (relatórios e testes).
pub fn temporada_tipica(categoria: &str, classe: Option<&str>) -> FaturaDaEtapa {
    temporada::fatura_de_temporada(categoria, classe, &temporada::EquipeNaTemporada::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economia::evento::fatura_da_etapa;
    use crate::economia::tipos::EntradaDaEtapa;

    fn entrada_gt3() -> (FaturaDaEtapa, FaturaDaEtapa) {
        let etapa = fatura_da_etapa(&EntradaDaEtapa::tipica("gt3", None));
        let temporada = temporada_tipica("gt3", None);
        (etapa, temporada)
    }

    /// Compra de peça de uma rodada típica de GT3. O número não é redondo por acaso: a
    /// medição de `commands::race::despesa` põe a peça em ~151% da fatura visível da GT3,
    /// e a fatura visível típica dela fica na casa dos 55 mil.
    const PECA_TIPICA_GT3: f64 = 83_000.0;

    fn fatura(
        etapa: &FaturaDaEtapa,
        temporada: &FaturaDaEtapa,
        receita: ReceitaDaEtapa,
    ) -> FaturaVisivel {
        fatura_com_peca(etapa, temporada, receita, PECA_TIPICA_GT3)
    }

    fn fatura_com_peca(
        etapa: &FaturaDaEtapa,
        temporada: &FaturaDaEtapa,
        receita: ReceitaDaEtapa,
        peca_comprada: f64,
    ) -> FaturaVisivel {
        fatura_visivel(&EntradaDaFatura {
            etapa,
            temporada,
            etapas_na_temporada: 14.0,
            receita,
            peca_comprada,
            folha_de_pilotos_anual: None,
        })
    }

    fn receita_qualquer() -> ReceitaDaEtapa {
        ReceitaDaEtapa {
            premio_de_corrida: 120_000.0,
            volta_mais_rapida: 8_000.0,
            patrocinio: 90_000.0,
            bilheteria: 40_000.0,
        }
    }

    #[test]
    fn a_forma_travada_e_quatro_blocos_e_oito_linhas_de_despesa() {
        let (etapa, temporada) = entrada_gt3();
        let f = fatura(&etapa, &temporada, receita_qualquer());

        assert_eq!(
            f.linhas_de_despesa(),
            MAXIMO_DE_LINHAS_DE_DESPESA,
            "a GT3 típica tem as oito linhas de despesa"
        );
        for bloco in [
            BlocoDaFatura::Corrida,
            BlocoDaFatura::Logistica,
            BlocoDaFatura::Equipe,
            BlocoDaFatura::Receita,
        ] {
            assert!(
                f.linhas_do_bloco(bloco).count() > 0,
                "bloco {} vazio",
                bloco.chave()
            );
        }
        assert_eq!(f.linhas_do_bloco(BlocoDaFatura::Corrida).count(), 4);
        assert_eq!(f.linhas_do_bloco(BlocoDaFatura::Logistica).count(), 3);
        assert_eq!(f.linhas_do_bloco(BlocoDaFatura::Equipe).count(), 1);
        assert_eq!(f.linhas_do_bloco(BlocoDaFatura::Receita).count(), 4);
    }

    #[test]
    fn os_blocos_saem_na_ordem_da_tela() {
        let (etapa, temporada) = entrada_gt3();
        let f = fatura(&etapa, &temporada, receita_qualquer());
        let ordens: Vec<u8> = f.linhas.iter().map(|l| l.bloco.ordem()).collect();
        assert!(
            ordens.windows(2).all(|w| w[0] <= w[1]),
            "blocos fora de ordem: {ordens:?}"
        );
        // Receita é o último: o saldo é a última coisa que se lê.
        assert!(f.linhas.last().unwrap().bloco.e_receita());
    }

    #[test]
    fn agrega_sem_perder_nem_inventar_dinheiro() {
        // A despesa visível é EXATAMENTE o que a etapa tirou do caixa: a fatura física
        // (`event_operations_cost`) mais a peça comprada (`technical_investment_cost`).
        // Nem mais — o custo fixo do ano saiu pela decisão 10 e é rodapé — nem menos.
        let (etapa, temporada) = entrada_gt3();
        let f = fatura(&etapa, &temporada, ReceitaDaEtapa::default());

        let esperado = etapa.total() + PECA_TIPICA_GT3;
        assert!(
            (f.total_de_despesa() - esperado).abs() < 1e-6,
            "despesa visível {} ≠ etapa + peça {esperado}",
            f.total_de_despesa()
        );
        assert!((f.valor(V_PECA_DE_REPOSICAO) - PECA_TIPICA_GT3).abs() < 1e-6);
        // E o rodapé é o ano inteiro, fora do total.
        assert!((f.custo_fixo_anual() - temporada.total()).abs() < 1e-6);
        assert!((f.custo_fixo_por_etapa() - temporada.total() / 14.0).abs() < 1e-6);
        // E cada linha visível é a soma exata dos detalhes dela, dividida pelo divisor.
        for linha in &f.linhas {
            let soma: f64 = linha.detalhe.iter().map(Detalhe::total).sum();
            assert!(
                (linha.total() - soma / linha.divisor).abs() < 1e-9,
                "linha {}",
                linha.chave
            );
        }
    }

    #[test]
    fn o_detalhe_do_rodape_mostra_o_contrato_anual_nao_o_diluido() {
        // A regra "rótulo e número contam a mesma história" vale dentro do expandir:
        // a unidade diz `pessoa_ano`, então o preço tem que ser o do ANO. Escrever ali
        // o valor já dividido pelas etapas mostraria um salário que ninguém recebe.
        let (etapa, temporada) = entrada_gt3();
        let f = fatura(&etapa, &temporada, ReceitaDaEtapa::default());
        let linha = f.custo_fixo_do_ano.as_ref().expect("rodapé existe");

        // E ele não é linha da etapa: quem procurar entre as linhas não acha.
        assert!(f.linha(V_CUSTO_FIXO_DO_ANO).is_none());
        assert!(linha.e_rateio());
        assert!((linha.divisor - 14.0).abs() < 1e-9);
        // O detalhe bate com a fatura anual crua, linha a linha.
        for d in &linha.detalhe {
            let anual = temporada.linha(d.chave).expect("linha anual");
            assert!(
                (d.preco_unitario - anual.preco_unitario).abs() < 1e-6,
                "detalhe {} diluído: {} ≠ {}",
                d.chave,
                d.preco_unitario,
                anual.preco_unitario
            );
            assert!((d.quantidade - anual.quantidade).abs() < 1e-9);
        }
        // E o total da linha é o ano inteiro dividido pelas etapas.
        assert!((linha.total() - temporada.total() / 14.0).abs() < 1e-6);
    }

    #[test]
    fn viagem_e_estadia_e_uma_linha_com_as_duas_atras() {
        let (etapa, temporada) = entrada_gt3();
        let f = fatura(&etapa, &temporada, ReceitaDaEtapa::default());
        let linha = f.linha(V_VIAGEM_E_ESTADIA).expect("linha existe");

        assert!(
            linha.tem_detalhe(),
            "a UI precisa saber que dá pra expandir"
        );
        assert_eq!(linha.detalhe.len(), 2);
        assert!(
            (linha.total() - (etapa.valor(VIAGEM) + etapa.valor(ESTADIA))).abs() < 1e-9,
            "a linha agregada tem que somar as duas internas"
        );
        // E as grandezas físicas sobrevivem ao expandir.
        assert_eq!(linha.detalhe[0].unidade, Unidade::Pessoa);
        assert_eq!(linha.detalhe[1].unidade, Unidade::PessoaNoite);
    }

    #[test]
    fn a_fatura_da_rookie_nao_tem_as_linhas_que_a_gt3_tem() {
        // A regra travada na §3.3.3: a linha categórica SOME, não zera. É o que faz a
        // fatura dizer em que altura da pirâmide a equipe está.
        let rookie_etapa = fatura_da_etapa(&EntradaDaEtapa::tipica("mazda_rookie", None));
        let rookie_temp = temporada_tipica("mazda_rookie", None);
        let rookie = fatura(&rookie_etapa, &rookie_temp, ReceitaDaEtapa::default());

        let (gt3_etapa, gt3_temp) = entrada_gt3();
        let gt3 = fatura(&gt3_etapa, &gt3_temp, ReceitaDaEtapa::default());

        let chaves = |f: &FaturaVisivel| -> Vec<&'static str> {
            f.custo_fixo_do_ano
                .as_ref()
                .map(|l| l.detalhe.iter().map(|d| d.chave).collect())
                .unwrap_or_default()
        };
        let da_rookie = chaves(&rookie);
        let da_gt3 = chaves(&gt3);

        assert!(
            !da_rookie.contains(&temporada::SIMULADOR),
            "a Rookie não tem simulador barato — ela não tem simulador: {da_rookie:?}"
        );
        assert!(
            da_gt3.contains(&temporada::SIMULADOR),
            "a GT3 tem: {da_gt3:?}"
        );
        assert!(
            da_gt3.len() > da_rookie.len(),
            "a fatura do topo tem linhas que a da base não tem"
        );
    }

    #[test]
    fn nenhuma_linha_visivel_vale_zero() {
        // A invariante, para toda a escada: se está na fatura, tem dinheiro.
        for (categoria, classe) in [
            ("mazda_rookie", None),
            ("bmw_m2", None),
            ("gt4", None),
            ("gt3", None),
            ("endurance", Some("lmp2")),
        ] {
            let etapa = fatura_da_etapa(&EntradaDaEtapa::tipica(categoria, classe));
            let temporada = temporada_tipica(categoria, classe);
            let f = fatura(&etapa, &temporada, ReceitaDaEtapa::default());
            for linha in &f.linhas {
                assert!(linha.total() > 0.0, "{categoria}: linha {}", linha.chave);
                for d in &linha.detalhe {
                    assert!(d.total() > 0.0, "{categoria}: detalhe {}", d.chave);
                }
            }
            assert!(
                f.linhas_de_despesa() <= MAXIMO_DE_LINHAS_DE_DESPESA,
                "{categoria} estourou o teto de linhas"
            );
        }
    }

    #[test]
    fn canal_de_receita_zerado_some_da_fatura() {
        // Uma etapa sem volta mais rápida não mostra "volta mais rápida: 0".
        let (etapa, temporada) = entrada_gt3();
        let f = fatura(
            &etapa,
            &temporada,
            ReceitaDaEtapa {
                premio_de_corrida: 50_000.0,
                volta_mais_rapida: 0.0,
                patrocinio: 30_000.0,
                bilheteria: 10_000.0,
            },
        );
        assert!(f.linha(V_BONUS_POR_RESULTADO).is_none());
        assert_eq!(f.linhas_do_bloco(BlocoDaFatura::Receita).count(), 3);
    }

    #[test]
    fn folha_real_de_pilotos_substitui_a_referencia() {
        // A linha `folha_de_pilotos` de `temporada` é referência de dupla mediana, não
        // o que a equipe paga. Com a folha real em mãos, é ela que vai para a tela.
        let (etapa, temporada) = entrada_gt3();
        let com_referencia = fatura(&etapa, &temporada, ReceitaDaEtapa::default());

        let folha_real = 3_000_000.0;
        let com_real = fatura_visivel(&EntradaDaFatura {
            etapa: &etapa,
            temporada: &temporada,
            etapas_na_temporada: 14.0,
            receita: ReceitaDaEtapa::default(),
            peca_comprada: PECA_TIPICA_GT3,
            folha_de_pilotos_anual: Some(folha_real),
        });

        // O detalhe é ANUAL (o rateio é do divisor da linha), então bate com a folha
        // real cheia, não com a fatia da etapa.
        let folha_anual_no_detalhe = |f: &FaturaVisivel| -> f64 {
            f.custo_fixo_do_ano
                .as_ref()
                .and_then(|l| l.detalhe.iter().find(|d| d.chave == FOLHA_DE_PILOTOS))
                .map(Detalhe::total)
                .unwrap_or(0.0)
        };
        assert!((folha_anual_no_detalhe(&com_real) - folha_real).abs() < 1e-6);
        assert!(
            (folha_anual_no_detalhe(&com_referencia) - folha_real).abs() > 1.0,
            "a referência e a folha real não podem coincidir por acaso neste fixture"
        );
        // O rodapé move junto: a folha real é maior, o custo fixo do ano é maior.
        assert!(com_real.custo_fixo_anual() > com_referencia.custo_fixo_anual());
        // E a DESPESA DA ETAPA não se mexe — o contrato de piloto não é custo de corrida.
        assert!((com_real.total_de_despesa() - com_referencia.total_de_despesa()).abs() < 1e-9);
    }

    /// **O expandir segue a grandeza física, não a contagem de detalhes.** Combustível
    /// tem um detalhe só e é o caso em que o expandir mais importa; patrocínio também tem
    /// um só, e mostrá-lo escreveria a linha duas vezes.
    #[test]
    fn o_expandir_aparece_onde_ha_grandeza_fisica() {
        let (etapa, temporada) = entrada_gt3();
        let f = fatura(&etapa, &temporada, receita_qualquer());

        for chave in [V_COMBUSTIVEL, V_PNEUS, V_REVISAO_MECANICA, V_FRETE, V_DIARIAS] {
            let linha = f.linha(chave).unwrap_or_else(|| panic!("linha {chave}"));
            assert!(linha.tem_detalhe(), "{chave} perdeu o expandir");
        }
        for chave in [
            V_PECA_DE_REPOSICAO,
            V_PREMIO_DA_ETAPA,
            V_BONUS_POR_RESULTADO,
            V_BILHETERIA,
            V_PATROCINIO,
        ] {
            let linha = f.linha(chave).unwrap_or_else(|| panic!("linha {chave}"));
            assert!(
                !linha.tem_detalhe(),
                "{chave} não tem grandeza física para expandir"
            );
        }
        // E o rodapé sempre expande: o detalhe dele está numa escala diferente da linha.
        assert!(f.custo_fixo_do_ano.as_ref().expect("rodapé").tem_detalhe());
    }

    /// A regra 1 na linha nova: rodada sem troca de peça não mostra "peça: $0", mostra
    /// uma fatura sem a linha. É o que faz a fatura dizer que aquele fim de semana não
    /// consumiu nada do estoque.
    #[test]
    fn rodada_sem_troca_nao_mostra_peca_zerada() {
        let (etapa, temporada) = entrada_gt3();
        let sem = fatura_com_peca(&etapa, &temporada, ReceitaDaEtapa::default(), 0.0);

        assert!(sem.linha(V_PECA_DE_REPOSICAO).is_none());
        assert_eq!(sem.linhas_de_despesa(), MAXIMO_DE_LINHAS_DE_DESPESA - 1);
        // A revisão mecânica CONTINUA — ela existe em toda etapa, e é justamente por isso
        // que as duas não podem ter o mesmo nome.
        assert!(sem.linha(V_REVISAO_MECANICA).is_some());
    }

    /// **As duas linhas de peça não podem convergir.** O jogador soma as duas se os
    /// rótulos se parecerem, e elas são coisas diferentes: uma é desgaste amortizado por
    /// km, a outra é a compra. Este teste trava o que o código controla — que sejam duas
    /// linhas distintas, no mesmo bloco, com tokens que não compartilham radical.
    #[test]
    fn as_duas_linhas_de_peca_sao_distintas() {
        let (etapa, temporada) = entrada_gt3();
        let f = fatura(&etapa, &temporada, ReceitaDaEtapa::default());

        let revisao = f.linha(V_REVISAO_MECANICA).expect("revisão");
        let compra = f.linha(V_PECA_DE_REPOSICAO).expect("peça comprada");
        assert_ne!(revisao.chave, compra.chave);
        assert_eq!(revisao.bloco, BlocoDaFatura::Corrida);
        assert_eq!(compra.bloco, BlocoDaFatura::Corrida);

        // Nenhuma palavra em comum entre os dois tokens — é a trava contra alguém
        // renomear um dos dois para perto do outro.
        let palavras = |chave: &str| -> Vec<String> {
            chave
                .split('_')
                .filter(|p| p.len() > 2 && *p != "de")
                .map(str::to_string)
                .collect()
        };
        for p in palavras(V_REVISAO_MECANICA) {
            assert!(
                !palavras(V_PECA_DE_REPOSICAO).contains(&p),
                "os dois tokens de peça compartilham a palavra {p:?}"
            );
        }

        // E a compra é a MAIOR das duas na GT3 — é por isso que ela virou linha.
        assert!(compra.total() > revisao.total() * 10.0);
    }

    #[test]
    fn o_resultado_da_etapa_e_receita_menos_despesa() {
        let (etapa, temporada) = entrada_gt3();
        let r = receita_qualquer();
        let f = fatura(&etapa, &temporada, r);
        assert!((f.total_de_receita() - r.total()).abs() < 1e-9);
        assert!((f.resultado() - (f.total_de_receita() - f.total_de_despesa())).abs() < 1e-9);
    }

    #[test]
    fn o_calendario_nao_mexe_mais_na_fatura_da_etapa() {
        // **É a decisão 10 em um teste.** O tamanho do calendário mudava a fatura de uma
        // corrida — a mesma etapa custava mais num campeonato de 6 rodadas do que num de
        // 14, porque o rateio estava dentro dela. Isso é falsa precisão: a corrida
        // consumiu os mesmos litros nos dois casos.
        let (etapa, temporada) = entrada_gt3();
        let curto = fatura_visivel(&EntradaDaFatura {
            etapa: &etapa,
            temporada: &temporada,
            etapas_na_temporada: 6.0,
            receita: ReceitaDaEtapa::default(),
            peca_comprada: PECA_TIPICA_GT3,
            folha_de_pilotos_anual: None,
        });
        let longo = fatura(&etapa, &temporada, ReceitaDaEtapa::default());

        assert!(
            (curto.total_de_despesa() - longo.total_de_despesa()).abs() < 1e-9,
            "o calendário ainda move a fatura da etapa"
        );
        // O ano é o mesmo nos dois; só a FATIA por etapa difere, e ela é rodapé.
        assert!((curto.custo_fixo_anual() - longo.custo_fixo_anual()).abs() < 1e-6);
        assert!(curto.custo_fixo_por_etapa() > longo.custo_fixo_por_etapa());
    }

    #[test]
    fn escalar_preco_preserva_a_quantidade_fisica() {
        // O modificador de conjuntura entra no PREÇO, nunca no total: senão o expandir
        // pararia de multiplicar — "173 L × $3,40" deixaria de dar o número ao lado.
        let (etapa, temporada) = entrada_gt3();
        let base = fatura(&etapa, &temporada, receita_qualquer());
        let litros = base
            .linha(V_COMBUSTIVEL)
            .map(|l| l.detalhe[0].quantidade)
            .expect("linha de combustível");
        let escalada = base.clone().com_precos_de_despesa_escalados(1.25);

        assert!((escalada.valor(V_COMBUSTIVEL) - base.valor(V_COMBUSTIVEL) * 1.25).abs() < 1e-6);
        assert!(
            (escalada.linha(V_COMBUSTIVEL).unwrap().detalhe[0].quantidade - litros).abs() < 1e-9,
            "os litros não podem mudar com a conjuntura"
        );
        // Um modificador de CUSTO não move receita.
        assert!((escalada.total_de_receita() - base.total_de_receita()).abs() < 1e-9);
        // E o detalhe continua fechando com a linha.
        for linha in &escalada.linhas {
            let soma: f64 = linha.detalhe.iter().map(Detalhe::total).sum();
            assert!((linha.total() - soma / linha.divisor).abs() < 1e-9);
        }
    }
}
