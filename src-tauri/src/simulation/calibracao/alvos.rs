//! As faixas-alvo: o que a distribuição PRECISA parecer para a corrida ser crível.
//!
//! Cada faixa é uma afirmação discutível, e por isso vem comentada com o porquê. Elas não são
//! verdade revelada — são a proposta contra a qual o conserto da simulação vai ser julgado, e o
//! lugar certo para discordar é aqui, editando o número e o comentário juntos.
//!
//! Duas faixas de propósito DIFERENTES por categoria, porque a hipótese do pacote é que rookie e
//! topo deveriam ser caóticas por motivos opostos: na rookie o carro é spec e o caos vem do erro
//! humano (consistência baixa, largada, batida); no topo o pelotão é metronômico e o caos teria
//! que vir do carro, da pista e da estratégia. Hoje as duas medem quase igual — o que já é, por
//! si só, o sintoma.

/// Intervalo fechado aceitável para uma métrica.
#[derive(Debug, Clone, Copy)]
pub struct Faixa {
    pub min: f64,
    pub max: f64,
}

impl Faixa {
    pub const fn nova(min: f64, max: f64) -> Self {
        Self { min, max }
    }

    pub fn contem(&self, valor: f64) -> bool {
        valor.is_finite() && valor >= self.min && valor <= self.max
    }

    /// `"ok"` / `"BAIXO"` / `"ALTO"` — usado no relatório.
    pub fn veredito(&self, valor: f64) -> &'static str {
        if !valor.is_finite() {
            "?"
        } else if valor < self.min {
            "BAIXO"
        } else if valor > self.max {
            "ALTO"
        } else {
            "ok"
        }
    }
}

/// O conjunto completo de faixas de uma categoria.
#[derive(Debug, Clone)]
pub struct Alvos {
    pub spearman_grid_chegada: Faixa,
    pub pct_vitorias_do_pole: Faixa,
    pub vencedores_distintos: Faixa,
    pub desvio_posicao: Faixa,
    pub p_melhor_fora_top5: Faixa,
    pub spearman_etapas_consecutivas: Faixa,
    /// Trocas de liderança do campeonato na temporada. Substituiu a etapa-de-decisão, que era
    /// falso verde: aquela passa numa simulação travada por aritmética da tabela de pontos, esta
    /// é zero quando o campeonato não existe.
    pub trocas_de_lideranca: Faixa,
    /// Margem do campeão sobre o segundo, em fração dos pontos disponíveis a um piloto.
    pub margem_do_campeao: Faixa,
}

impl Alvos {
    /// Categoria de entrada (mazda_rookie): carro spec, pelotão inexperiente, muito erro.
    ///
    /// Divergências assumidas em relação aos alvos de partida do briefing:
    /// - **pole→vitória**: o briefing pede 35–50%. Esse número é de F1 (20 carros, ultrapassagem
    ///   cara, carro dominante). Numa monomarca de entrada com carro idêntico e pelotão
    ///   irregular, a pole vale muito menos: 15–35% é o que se vê em MX-5 cup de verdade.
    /// - **Spearman grid×chegada**: 0,60–0,80 é apertado demais para categoria de entrada.
    ///   Abaixamos o piso: um campo spec de novatos embaralha mais que isso.
    pub fn entrada() -> Self {
        Self {
            spearman_grid_chegada: Faixa::nova(0.40, 0.75),
            pct_vitorias_do_pole: Faixa::nova(0.15, 0.35),
            // 12 etapas: uma rookie saudável tem meia dúzia de nomes ganhando.
            vencedores_distintos: Faixa::nova(5.0, 10.0),
            desvio_posicao: Faixa::nova(3.5, 6.5),
            p_melhor_fora_top5: Faixa::nova(0.15, 0.35),
            // Ligada por construção ao Spearman de grid. Grid e chegada são duas observações do
            // MESMO fim de semana: compartilham o permanente E a camada de evento. Etapas
            // consecutivas compartilham só o permanente. Logo esta TEM que ser materialmente
            // menor que a de grid, e o 0,55–0,75 do briefing era impossível de satisfazer junto
            // com o alvo de grid × chegada.
            spearman_etapas_consecutivas: Faixa::nova(0.20, 0.55),
            // 12 etapas numa monomarca disputada: a ponta troca várias vezes.
            trocas_de_lideranca: Faixa::nova(2.0, 7.0),
            margem_do_campeao: Faixa::nova(0.02, 0.15),
        }
    }

    /// Categoria de topo (gt3): pelotão forte, consistente e profissional — a ordem PODE ser mais
    /// estável, e a diferença de carro é parte do design (`car_weight_scale("gt3") == 1.30`).
    ///
    /// Divergência assumida: o briefing pede 5–10 vencedores distintos como alvo único. Exigir
    /// isso no topo brigaria com o próprio design do jogo, que quer dinastias — um time com
    /// carro claramente melhor DEVE ganhar muito. Aqui a faixa é 3–8: estabilidade sim,
    /// resultado carimbado não.
    pub fn topo() -> Self {
        Self {
            spearman_grid_chegada: Faixa::nova(0.60, 0.88),
            pct_vitorias_do_pole: Faixa::nova(0.30, 0.55),
            vencedores_distintos: Faixa::nova(3.0, 8.0),
            desvio_posicao: Faixa::nova(2.5, 5.0),
            p_melhor_fora_top5: Faixa::nova(0.08, 0.25),
            spearman_etapas_consecutivas: Faixa::nova(0.35, 0.70),
            // No topo a concentração é design (dinastias), então menos trocas — mas nunca zero.
            trocas_de_lideranca: Faixa::nova(1.0, 5.0),
            // 12 etapas dão ~312 pontos a um piloto. 20% de margem = ~62 pontos = quase 2,5
            // vitórias de vantagem: já é temporada dominante. Acima disso não é dinastia, é
            // passeio, e o teto tem que refletir isso mesmo que o topo deva concentrar.
            margem_do_campeao: Faixa::nova(0.04, 0.20),
        }
    }
}
