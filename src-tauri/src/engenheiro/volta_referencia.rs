//! A VOLTA DE REFERÊNCIA: quanto a volta em curso está perdendo, medido por nós.
//!
//! O engenheiro só pode dizer "perdeu essa" se souber que a volta morreu. O iRacing tem uma
//! variável para isso (`LapDeltaToSessionBestLap`), e a decisão aqui foi **não usá-la**.
//!
//! Não é desconfiança genérica: é o histórico medido deste projeto com canais do SDK que
//! parecem prontos e não são. O `tire_compound` volta sempre zero fora de série multi-composto.
//! O `SessionLapsRemainEx` manda 32767 em prova por tempo. E o `f2_time` — o pior dos três —
//! volta **populado e congelado** em corrida de IA: disse 0,165 s com o carro a quarenta
//! segundos de distância. Um delta que mente é pior que delta nenhum, porque a fala sai com a
//! confiança de quem mediu.
//!
//! O que este módulo faz cabe em duas frases: guarda a melhor volta da sessão como uma curva de
//! *posição na volta → tempo decorrido*, e compara a volta em curso contra ela no mesmo ponto da
//! pista. Os dois insumos (`lap_dist_pct` e `lap_current_time`) já são lidos a 60 Hz, então isto
//! não pede nada novo do sim e funciona por construção em sessão offline com IA.

/// Em quantos pedaços a volta é dividida. Duzentos = meio ponto percentual cada.
///
/// A 60 Hz, uma volta de 90 s dá 5.400 amostras — 27 por balde. Mais baldes não trariam
/// precisão (a amostragem é o teto), e menos borrariam a comparação nas curvas lentas, que é
/// justamente onde o tempo se perde.
pub const BALDES: usize = 200;

/// Que fatia dos baldes a volta precisa ter visitado para virar referência, em porcento.
///
/// Noventa e cinco. Não é 100 porque 100 é frágil por dois motivos independentes: um quadro
/// perdido na telemetria já abre um buraco, e a própria aritmética de ponto flutuante faz uma
/// amostragem regular pular balde (medido no primeiro teste deste módulo — `3/200 * 200` dá
/// 2,9999999999999996). Cinco por cento de folga cobre os dois e continua reprovando com sobra a
/// volta interrompida no meio, que é o que este piso existe para pegar.
pub const COBERTURA_MINIMA: usize = 95;

/// Acima disto a volta não é mais recuperável, e o engenheiro pode dizer isso.
///
/// Um segundo e meio. O erro que importa evitar NÃO é ele calar demais — é ele dizer "perdeu
/// essa" numa volta que ainda dava. O piloto alivia, e aí fomos nós que custamos a volta. Três
/// décimos é ruído de pilotagem; um segundo e meio numa volta de um e trinta é 1,7% e não volta.
pub const LIMIAR_VOLTA_MORTA_S: f64 = 1.5;

/// A melhor volta da sessão, guardada como curva, e a volta em curso sendo desenhada por cima.
#[derive(Debug, Clone)]
pub struct VoltaReferencia {
    /// Tempo decorrido em cada balde da MELHOR volta. `None` até a primeira volta válida.
    referencia: Option<[f32; BALDES]>,
    /// O tempo dessa melhor volta, em segundos. 0 = ainda não há.
    melhor_s: f64,
    /// A volta em curso, sendo desenhada.
    parcial: [f32; BALDES],
    /// Que baldes a volta em curso já preencheu. Sem isto, um balde nunca visitado (o carro
    /// entrou no box no meio) valeria zero e a volta entraria como referência absurda.
    vistos: [bool; BALDES],
}

impl Default for VoltaReferencia {
    fn default() -> Self {
        VoltaReferencia::novo()
    }
}

impl VoltaReferencia {
    pub const fn novo() -> VoltaReferencia {
        VoltaReferencia {
            referencia: None,
            melhor_s: 0.0,
            parcial: [0.0; BALDES],
            vistos: [false; BALDES],
        }
    }

    /// Sessão nova. A melhor volta do classificatório não vale para a corrida — outro tanque,
    /// outro pneu, outro objetivo.
    pub fn reiniciar(&mut self) {
        *self = VoltaReferencia::novo();
    }

    /// Uma amostra da volta em curso. Chamada a cada tique de telemetria.
    ///
    /// Só o PRIMEIRO valor de cada balde conta. As passagens seguintes pelo mesmo balde na mesma
    /// volta são o carro andando devagar dentro dele, e sobrescrever faria a curva registrar o
    /// instante em que ele SAIU do trecho em vez de quando entrou.
    pub fn amostrar(&mut self, pct: f64, tempo_na_volta_s: f64) {
        if !(0.0..=1.0).contains(&pct) || tempo_na_volta_s < 0.0 {
            return;
        }
        let i = Self::balde(pct);
        if self.vistos[i] {
            return;
        }
        self.vistos[i] = true;
        self.parcial[i] = tempo_na_volta_s as f32;
    }

    /// A volta terminou com este tempo. Vira referência se for a melhor E se estiver inteira.
    ///
    /// "Inteira" é o guarda que importa: uma volta com buraco na curva produziria delta negativo
    /// gigante no trecho que faltou, e o rádio diria que estamos voando quando estamos no box.
    ///
    /// Mas o guarda não pode ser "todos os 200 baldes", e isso apareceu no primeiro teste: com
    /// aritmética de ponto flutuante, uma amostragem perfeitamente regular já pula balde. Na
    /// pista é pior — basta um quadro perdido. Então o piso é [`COBERTURA_MINIMA`] e os buracos
    /// interiores são **interpolados** entre os vizinhos, que é o único preenchimento que não
    /// inventa tempo para nenhum dos dois lados.
    pub fn fechar_volta(&mut self, tempo_da_volta_s: f64) {
        let vistos = self.vistos.iter().filter(|v| **v).count();
        let coberta = vistos * 100 >= BALDES * COBERTURA_MINIMA;
        if tempo_da_volta_s > 0.0
            && coberta
            && (self.melhor_s <= 0.0 || tempo_da_volta_s < self.melhor_s)
        {
            self.melhor_s = tempo_da_volta_s;
            self.referencia = Some(self.preencher_buracos(tempo_da_volta_s));
        }
        self.descartar_volta();
    }

    /// Interpola os baldes que a volta não visitou. Buraco no começo vira 0 (a linha), buraco no
    /// fim vira o tempo da volta — os dois extremos são conhecidos por definição.
    fn preencher_buracos(&self, tempo_da_volta_s: f64) -> [f32; BALDES] {
        let mut curva = self.parcial;
        let mut anterior_i = 0usize;
        let mut anterior_t = 0.0f32;
        for i in 0..BALDES {
            if self.vistos[i] {
                anterior_i = i;
                anterior_t = curva[i];
                continue;
            }
            // Procura o próximo balde conhecido; se não houver, o fim da volta serve de âncora.
            let (proximo_i, proximo_t) = (i + 1..BALDES)
                .find(|j| self.vistos[*j])
                .map_or((BALDES, tempo_da_volta_s as f32), |j| (j, curva[j]));
            let vao = (proximo_i - anterior_i) as f32;
            curva[i] = anterior_t + (proximo_t - anterior_t) * ((i - anterior_i) as f32 / vao);
        }
        curva
    }

    /// Joga fora a volta em curso sem considerá-la — volta de saída do box, volta abortada,
    /// bandeira. O desenho recomeça na próxima passagem.
    pub fn descartar_volta(&mut self) {
        self.parcial = [0.0; BALDES];
        self.vistos = [false; BALDES];
    }

    pub fn tem_referencia(&self) -> bool {
        self.referencia.is_some()
    }

    /// O tempo da melhor volta da sessão. 0 = ainda não há.
    pub fn melhor_s(&self) -> f64 {
        self.melhor_s
    }

    /// Quanto a volta em curso está perdendo neste ponto da pista, em segundos. Positivo = mais
    /// lenta. `None` enquanto não houver referência ou antes do primeiro balde fechado.
    pub fn delta_s(&self, pct: f64, tempo_na_volta_s: f64) -> Option<f64> {
        let referencia = self.referencia.as_ref()?;
        if !(0.0..=1.0).contains(&pct) {
            return None;
        }
        let i = Self::balde(pct);
        // O balde ZERO da referência é o instante da largada da volta, que é sempre ~0 — comparar
        // ali daria delta zero em qualquer volta e o rádio começaria toda tentativa achando que
        // está no páreo.
        if i == 0 {
            return None;
        }
        Some(tempo_na_volta_s - f64::from(referencia[i]))
    }

    /// A volta em curso já morreu? Ver [`LIMIAR_VOLTA_MORTA_S`].
    pub fn volta_morta(&self, pct: f64, tempo_na_volta_s: f64) -> bool {
        self.delta_s(pct, tempo_na_volta_s)
            .is_some_and(|d| d >= LIMIAR_VOLTA_MORTA_S)
    }

    fn balde(pct: f64) -> usize {
        ((pct * BALDES as f64) as usize).min(BALDES - 1)
    }
}

#[cfg(test)]
mod tests;
