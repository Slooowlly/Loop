//! DIÁRIO DO SPOTTER: por que ele ficou calado.
//!
//! O [`crate::radio_registro`] responde o que o spotter DISSE, e o que aconteceu com cada
//! fala depois de dita. Ele não responde a outra metade da pergunta, que é a que se faz
//! depois de uma corrida teste em que o rádio pareceu apagado: **o detector viu alguma
//! coisa e recusou, ou não viu nada?**
//!
//! São dois mundos diferentes e hoje o arquivo os confunde num silêncio só. Um detector
//! que nunca teve candidato está calibrado ou está morto, e não há como saber. Um
//! detector que teve quarenta candidatos e recusou os quarenta por 2 m de folga no limiar
//! está pedindo recalibração, e isso também não aparece.
//!
//! ## O que entra, e o que fica de fora
//!
//! Entra a RECUSA: o candidato existiu e a regra disse não, com o motivo e os números que
//! decidiram. Entra também quem perdeu a arbitragem do tique em
//! [`super::spotter::observar`], onde sete detectores disputam e um só ganha; os seis
//! perdedores voltam 16 ms depois e hoje somem sem rastro, o que esconde uma família rara
//! sendo esmagada por uma frequente durante minutos.
//!
//! Fica de fora o tique comum, em que nada foi visto. Gravar isso seria gravar a corrida
//! de novo, e a corrida já está gravada em [`super::race_capture`].
//!
//! ## Uma linha por EPISÓDIO, com o extremo dele
//!
//! A recusa se repete a 60 Hz enquanto a geometria não muda: um carro longe demais fica
//! longe demais por milhares de tiques. Uma linha por tique afogaria o que interessa.
//!
//! A unidade do arquivo é o **episódio**: o mesmo candidato recusado pelo mesmo motivo, do
//! primeiro tique ao último. Ele fica aberto em memória e vira UMA linha quando acaba, seja
//! porque o motivo mudou, porque a situação se resolveu, porque o candidato sumiu
//! ([`OCIOSO_S`]) ou porque a sessão descontinuou.
//!
//! **O que a linha carrega é o EXTREMO, e não o fim.** A primeira versão fechava na
//! transição do motivo e registrava a folga daquele instante, o que enviesa a medida para as
//! fronteiras por construção: o motivo muda justamente quando um limiar é cruzado, então a
//! folga sai perto de zero mesmo num candidato que passou longe de disparar. Medido em
//! 17/08/2026, as recusas de `boxe/perto` saíram com folga de 2 a 20 cm, todas artefato do
//! método. A pergunta real é "chegou perto de falar?", e quem a responde é a MENOR folga do
//! episódio, guardada com o detalhe daquele instante e acompanhada de quanto ele durou.
//!
//! O [`PISO_S`] cobre o caso ruim que sobra: o candidato oscilando em cima do limiar, que
//! sem ele picotaria um episódio em centenas alternando dois motivos.
//!
//! ## Nada de I/O aqui dentro
//!
//! [`nota`] é chamada de dentro dos detectores, alguns deles segurando o próprio lock, no
//! tique de 60 Hz. No caso comum ela faz um lookup em `HashMap` e atualiza dois campos: a
//! `folga` vem SOLTA justamente para o caminho quente não montar JSON, e a closure do
//! detalhe só roda quando este tique é o novo extremo. Quem escreve é [`escoar`], chamada
//! uma vez por tique no fim de [`super::spotter::observar`], fora de todo lock. Escrever
//! daqui colocaria uma chamada de sistema dentro do lock de um observador, que é o jeito
//! exato de transformar uma ferramenta de medição no motivo de o spotter engasgar.

use std::collections::HashMap;
use std::sync::Mutex;

use super::spotter_base::saltou_desde;

/// Alvo de uma nota que não fala de um carro específico: um portão de sessão, a
/// arbitragem do tique, o estado do jogador.
pub(crate) const SEM_ALVO: i32 = -1;

/// Folga de um motivo que não compara contra limiar nenhum: um portão de sessão, a
/// arbitragem do tique, a ausência de candidato.
pub(crate) const SEM_FOLGA: f64 = f64::INFINITY;

/// Quanto um episódio aberto sobrevive sem ser reafirmado antes de ser fechado.
///
/// Um candidato para de ser anotado quando some da janela, quando o jogador sai da pista ou
/// quando a corrida acaba, e nenhum desses avisa. Sem esta poda, o último episódio de cada
/// candidato ficaria aberto para sempre e nunca chegaria ao arquivo — e o último costuma ser
/// o mais interessante, porque é o que estava acontecendo quando algo mudou.
const OCIOSO_S: f64 = 1.0;

/// Piso de tempo de sessão entre duas notas do mesmo par (família, alvo).
///
/// Meio segundo deixa passar a oscilação real (um candidato que entra e sai da janela de
/// aviso a cada volta) e mata a oscilação de amostragem, que é a que não descreve nada.
const PISO_S: f64 = 0.5;

/// Teto de notas por execução do app.
///
/// Uma corrida teste de 30 minutos com dedup produz algumas centenas de linhas. Dez mil é
/// folga de mais de uma ordem de grandeza, e existe para o caso em que um detector novo
/// entra com um motivo que muda a cada tique: aí o diário para, avisa que parou, e o
/// arquivo do rádio continua legível em vez de virar um despejo de 400 MB.
const MAX_NOTAS: usize = 10_000;

/// Teto da fila pendente. Só estoura se o consumidor sumir, o que significa que o tique
/// parou de rodar; nesse caso o que importa é não crescer sem limite.
const MAX_FILA: usize = 256;

/// Corta as casas decimais que são ruído de sensor antes de o número virar texto.
///
/// Mesma razão do arredondamento da captura de corrida: `dist_m: 187.43829174` são vinte
/// bytes para uma medida cuja unidade útil é o metro, e um arquivo de diagnóstico que
/// ninguém consegue ler de relance não é lido.
pub(crate) fn arredondar(v: f64, casas: u32) -> f64 {
    let f = 10f64.powi(casas as i32);
    (v * f).round() / f
}

/// Uma recusa, pronta para virar linha no registro do rádio.
#[derive(Clone, Debug)]
pub(crate) struct Nota {
    /// Qual detector: `lateral`, `frente`, `tras`, `voltar`, `boxe`, `bandeira`, `clima`.
    pub familia: &'static str,
    /// Por que não falou. Vocabulário fechado por família, para um script agrupar sem
    /// interpretar prosa.
    pub motivo: &'static str,
    /// Índice do carro de que a nota fala, ou [`SEM_ALVO`].
    pub alvo: i32,
    /// Os números que decidiram: distância, tempo até chegar, fração de ritmo, quem ganhou
    /// o tique. É o que transforma "recusou" em "recusou por 2 m".
    pub detalhe: serde_json::Value,
}

/// Uma recusa em curso: o mesmo candidato sendo recusado pelo mesmo motivo, tique após tique.
///
/// A primeira versão do diário fechava a linha na TRANSIÇÃO do motivo, e a folga registrada
/// era a do instante da troca. Isso enviesa a medida para as fronteiras por construção: o
/// motivo muda justamente quando um limiar é cruzado, então a folga sai perto de zero mesmo
/// quando o candidato passou a corrida inteira longe de disparar. Medido em 17/08/2026: as
/// recusas de `boxe/perto` saíram com folga de 2 a 20 cm, todas artefato do método.
///
/// O que responde "chegou perto de falar?" é o EXTREMO do episódio, e não o seu fim. Por isso
/// o que fica guardado aqui é a menor folga já vista, com o detalhe daquele instante.
#[derive(Clone, Debug)]
struct Aberta {
    motivo: &'static str,
    desde_s: f64,
    ultimo_s: f64,
    tiques: u32,
    /// Menor folga já vista. `f64::INFINITY` quando o motivo não compara contra limiar
    /// nenhum (um portão de sessão, a arbitragem do tique).
    folga_min: f64,
    /// O detalhe do instante de MENOR folga, e não o do último tique.
    detalhe: serde_json::Value,
}

#[derive(Default)]
struct Estado {
    /// As recusas em curso, por par (família, alvo).
    visto: HashMap<(&'static str, i32), Aberta>,
    fila: Vec<Nota>,
    ultimo_tempo_s: Option<f64>,
    notas: usize,
    /// Já avisou que bateu no teto? O aviso sai uma vez, e não a cada nota descartada.
    teto_avisado: bool,
    /// Notas perdidas por fila cheia. Vai junto do próximo dreno, para o arquivo dizer que
    /// perdeu em vez de esconder o buraco.
    perdidas: usize,
}

/// Toda a regra do diário mora aqui, sobre um `Estado` que o chamador possui.
///
/// A alternativa era escrever tudo contra o singleton, e ela não sobrevive à suíte: os
/// testes do [`super::spotter_frente`] hoje EXERCITAM o detector, e o detector anota. Com a
/// regra amarrada ao global, cada teste de detector empurra notas para dentro do estado que
/// os testes do diário estão medindo, e as asserções passam a depender de quem o `cargo`
/// escalonou junto. `#[serial]` não resolve: ele ordena os testes marcados, e quem suja
/// aqui são os NÃO marcados.
impl Estado {
    /// **Anota uma recusa.** Barata de propósito: no caso comum, um lookup e dois campos.
    ///
    /// `folga` é quanto faltou para o limiar que recusou, em unidade da própria comparação
    /// (metros, km/h, segundos, fração). Passe [`SEM_FOLGA`] quando o motivo não compara
    /// contra limiar nenhum.
    ///
    /// Ela vem SOLTA, e não dentro do `detalhe`, exatamente para o caminho comum não montar
    /// JSON. A closure só é chamada quando este tique é o novo extremo do episódio, o que
    /// acontece uma punhado de vezes por candidato em vez de 60 vezes por segundo.
    fn anotar<F>(
        &mut self,
        tempo_s: f64,
        familia: &'static str,
        alvo: i32,
        motivo: &'static str,
        folga: f64,
        detalhe: F,
    ) where
        F: FnOnce() -> serde_json::Value,
    {
        let chave = (familia, alvo);
        match self.visto.get_mut(&chave) {
            Some(aberta) if aberta.motivo == motivo => {
                // O MESMO episódio continua. Só o extremo interessa.
                aberta.ultimo_s = tempo_s;
                aberta.tiques += 1;
                if folga < aberta.folga_min {
                    aberta.folga_min = folga;
                    aberta.detalhe = detalhe();
                }
            }
            Some(aberta) => {
                // Motivo diferente. Cedo demais é oscilação de amostragem em cima do limiar,
                // e não uma mudança do mundo: o episódio aberto continua absorvendo.
                if tempo_s - aberta.desde_s < PISO_S {
                    aberta.ultimo_s = tempo_s;
                    aberta.tiques += 1;
                    return;
                }
                self.fechar(familia, alvo);
                self.abrir(tempo_s, familia, alvo, motivo, folga, detalhe);
            }
            None => self.abrir(tempo_s, familia, alvo, motivo, folga, detalhe),
        }
    }

    fn abrir<F>(
        &mut self,
        tempo_s: f64,
        familia: &'static str,
        alvo: i32,
        motivo: &'static str,
        folga: f64,
        detalhe: F,
    ) where
        F: FnOnce() -> serde_json::Value,
    {
        self.visto.insert(
            (familia, alvo),
            Aberta {
                motivo,
                desde_s: tempo_s,
                ultimo_s: tempo_s,
                tiques: 1,
                folga_min: folga,
                detalhe: detalhe(),
            },
        );
    }

    /// **Fecha um episódio e o põe na fila.** É aqui que a recusa vira linha, e não na
    /// abertura: só no fim se sabe qual foi o instante em que ela chegou mais perto de virar
    /// fala.
    fn fechar(&mut self, familia: &'static str, alvo: i32) {
        let Some(aberta) = self.visto.remove(&(familia, alvo)) else {
            return;
        };
        if self.notas >= MAX_NOTAS {
            if !self.teto_avisado {
                self.teto_avisado = true;
                self.fila.push(Nota {
                    familia: "diario",
                    motivo: "teto",
                    alvo: SEM_ALVO,
                    detalhe: serde_json::json!({ "max": MAX_NOTAS }),
                });
            }
            return;
        }
        self.notas += 1;
        if self.fila.len() >= MAX_FILA {
            self.perdidas += 1;
            return;
        }
        let mut detalhe = aberta.detalhe;
        if let Some(obj) = detalhe.as_object_mut() {
            // `folga` passa a ser explicitamente a MENOR do episódio, e vem acompanhada de
            // quanto ele durou: uma recusa de 4 s a 0,15 km/h do corte é notícia, e a mesma
            // folga num episódio de um tique é ruído de amostragem.
            if aberta.folga_min.is_finite() {
                obj.insert("folga".into(), serde_json::json!(arredondar(aberta.folga_min, 3)));
            }
            obj.insert("durou_s".into(), serde_json::json!(arredondar(aberta.ultimo_s - aberta.desde_s, 2)));
            obj.insert("tiques".into(), serde_json::json!(aberta.tiques));
        }
        self.fila.push(Nota {
            familia,
            motivo: aberta.motivo,
            alvo,
            detalhe,
        });
    }

    /// A situação se RESOLVEU: a família ganhou o tique, o candidato virou aviso. O episódio
    /// aberto é fechado e vai para o arquivo — ele descreve o que foi recusado ANTES de a
    /// fala sair, e jogá-lo fora perderia justamente a aproximação que terminou em aviso.
    fn esquecer(&mut self, familia: &'static str, alvo: i32) {
        self.fechar(familia, alvo);
    }

    /// **O tique passou.** Roda com ou sem nota, e é ela que detecta a descontinuidade do
    /// relógio.
    ///
    /// O salto tem de ser medido no relógio do TIQUE, jamais no intervalo entre duas notas.
    /// As notas são esparsas por construção, e o dedup existe para isso: duas recusas
    /// legítimas a dez segundos de distância pareceriam um replay, a limpeza do dedup faria
    /// a segunda entrar de novo no arquivo, e o diário passaria a contar duas vezes o que
    /// aconteceu uma. A contagem do arquivo é justamente o que se lê para calibrar.
    fn tique(&mut self, tempo_s: f64) {
        if saltou_desde(self.ultimo_tempo_s, tempo_s) {
            // Sessão nova, replay, rebobinada: o mundo é outro. Os episódios abertos ainda
            // descrevem o mundo velho e vão para o arquivo antes de a memória ser limpa —
            // descartá-los perderia o retrato do instante que antecedeu a descontinuidade.
            let chaves: Vec<_> = self.visto.keys().copied().collect();
            for (familia, alvo) in chaves {
                self.fechar(familia, alvo);
            }
        }
        self.ultimo_tempo_s = Some(tempo_s);

        // A PODA. Episódio que parou de ser reafirmado acabou, e ninguém avisa quando um
        // candidato some da janela. Ver [`OCIOSO_S`].
        let vencidos: Vec<_> = self
            .visto
            .iter()
            .filter(|(_, a)| tempo_s - a.ultimo_s > OCIOSO_S)
            .map(|(k, _)| *k)
            .collect();
        for (familia, alvo) in vencidos {
            self.fechar(familia, alvo);
        }
    }

    /// Tira as notas pendentes.
    ///
    /// Devolve `Vec` vazio no caso comum, que é o que acontece na esmagadora maioria dos
    /// tiques: sem candidato novo, sem recusa nova, sem nada a escrever.
    fn drenar(&mut self) -> Vec<Nota> {
        if self.perdidas > 0 {
            let perdidas = std::mem::take(&mut self.perdidas);
            self.fila.push(Nota {
                familia: "diario",
                motivo: "fila_cheia",
                alvo: SEM_ALVO,
                detalhe: serde_json::json!({ "perdidas": perdidas }),
            });
        }
        std::mem::take(&mut self.fila)
    }
}

fn estado() -> &'static Mutex<Estado> {
    static E: std::sync::OnceLock<Mutex<Estado>> = std::sync::OnceLock::new();
    E.get_or_init(|| Mutex::new(Estado::default()))
}

fn lock() -> std::sync::MutexGuard<'static, Estado> {
    estado().lock().unwrap_or_else(|e| e.into_inner())
}

/// Anota uma recusa no diário do processo. Ver [`Estado::anotar`].
pub(crate) fn nota<F>(
    tempo_s: f64,
    familia: &'static str,
    alvo: i32,
    motivo: &'static str,
    folga: f64,
    detalhe: F,
) where
    F: FnOnce() -> serde_json::Value,
{
    lock().anotar(tempo_s, familia, alvo, motivo, folga, detalhe);
}

/// A situação se resolveu: a família ganhou o tique, o candidato virou aviso. Ver
/// [`Estado::esquecer`].
pub(crate) fn limpar(familia: &'static str, alvo: i32) {
    lock().esquecer(familia, alvo);
}

/// Escreve as notas pendentes na linha do tempo do rádio, no canal `spotter_diario`.
///
/// Canal separado e não o `spotter` de propósito: a linha do tempo do rádio é o que o
/// jogador OUVIU, e misturar recusa com fala tornaria ilegível justamente a leitura que
/// mais se faz. Mesmo arquivo, porque a junção com o `t` da sessão já vem pronta dali e
/// duplicar o mecanismo de arquivo seria duplicar a poda, o cabeçalho e a trava.
pub(crate) fn escoar(tempo_s: f64) {
    let notas = {
        let mut e = lock();
        e.tique(tempo_s);
        e.drenar()
    };
    for n in notas {
        crate::radio_registro::registrar(&crate::radio_registro::Registro {
            canal: "spotter_diario".to_string(),
            fase: "avaliada".to_string(),
            chaves: vec![n.familia.to_string()],
            desfecho: n.motivo.to_string(),
            detalhe: Some(serde_json::json!({ "alvo": n.alvo, "d": n.detalhe })),
            ..Default::default()
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Um diário próprio por teste. O singleton fica de fora de propósito: os testes dos
    /// detectores exercitam o caminho que anota, então o estado do processo chega aqui já
    /// escrito por quem o `cargo` escalonou junto.
    fn diario() -> Estado {
        Estado::default()
    }

    fn vazio() -> serde_json::Value {
        serde_json::json!({})
    }

    /// Corre o relógio do tique até `ate`, para a poda de ociosos rodar como em corrida.
    fn correr(d: &mut Estado, de: f64, ate: f64) {
        let mut t = de;
        while t < ate {
            d.tique(t);
            t += 1.0 / 60.0;
        }
    }

    #[test]
    fn o_episodio_inteiro_vira_uma_linha_so() {
        let mut d = diario();
        for n in 0..100 {
            d.anotar(10.0 + n as f64 * 0.016, "frente", 7, "longe", 50.0, vazio);
        }
        assert!(d.drenar().is_empty(), "enquanto o episódio dura, nada sai");
        // O candidato some da janela: a poda fecha o episódio.
        correr(&mut d, 11.7, 13.0);
        let notas = d.drenar();
        assert_eq!(notas.len(), 1, "cem tiques do mesmo motivo são UMA linha");
        assert_eq!(notas[0].detalhe["tiques"], 100);
    }

    /// O ponto de todo o redesenho: o que fica registrado é o instante em que a recusa
    /// chegou MAIS PERTO de virar fala, e não o instante em que o motivo mudou.
    #[test]
    fn a_folga_guardada_e_a_menor_do_episodio() {
        let mut d = diario();
        // Um carro se aproximando: a folga cai até 0,15 e volta a subir.
        for (i, folga) in [9.0, 4.0, 0.15, 3.0, 22.0].iter().enumerate() {
            let t = 10.0 + i as f64 * 0.1;
            d.anotar(t, "boxe", 9, "sem_diferenca", *folga, || {
                serde_json::json!({ "dif_kmh": 70.0 - folga })
            });
        }
        correr(&mut d, 10.5, 12.0);
        let notas = d.drenar();
        assert_eq!(notas.len(), 1);
        assert_eq!(notas[0].detalhe["folga"], 0.15, "a menor folga do episódio");
        assert_eq!(
            notas[0].detalhe["dif_kmh"], 69.85,
            "e o detalhe é o DAQUELE instante, não o do último tique"
        );
    }

    #[test]
    fn motivo_novo_fecha_o_anterior_e_abre_outro() {
        let mut d = diario();
        d.anotar(10.0, "frente", 7, "longe", 300.0, vazio);
        d.anotar(10.1, "frente", 7, "cedo", 1.0, vazio);
        assert!(d.drenar().is_empty(), "trocou cedo demais: é oscilação de amostragem");

        d.anotar(11.0, "frente", 7, "cedo", 1.0, vazio);
        let notas = d.drenar();
        assert_eq!(notas.len(), 1, "passado o piso, o episódio anterior fecha");
        assert_eq!(notas[0].motivo, "longe");
    }

    #[test]
    fn carros_diferentes_nao_se_calam() {
        let mut d = diario();
        d.anotar(10.0, "frente", 7, "longe", 5.0, vazio);
        d.anotar(10.0, "frente", 9, "longe", 5.0, vazio);
        correr(&mut d, 10.0, 11.5);
        assert_eq!(d.drenar().len(), 2, "o episódio é por alvo, não por motivo");
    }

    /// A recusa que antecede uma fala é justamente a mais informativa: ela descreve o quanto
    /// o candidato se aproximou antes de o aviso finalmente sair.
    #[test]
    fn quando_a_situacao_resolve_o_episodio_vai_para_o_arquivo() {
        let mut d = diario();
        d.anotar(10.0, "boxe", 3, "sem_diferenca", 0.6, vazio);
        d.esquecer("boxe", 3);
        let notas = d.drenar();
        assert_eq!(notas.len(), 1, "resolver não é motivo para perder o registro");
        assert_eq!(notas[0].motivo, "sem_diferenca");
    }

    #[test]
    fn o_ocioso_e_podado_e_o_episodio_fecha() {
        let mut d = diario();
        d.anotar(10.0, "frente", 7, "longe", 5.0, vazio);
        correr(&mut d, 10.0, 10.5);
        assert!(d.drenar().is_empty(), "meio segundo ainda é o mesmo episódio");
        correr(&mut d, 10.5, 11.5);
        assert_eq!(d.drenar().len(), 1, "passado o ocioso, ele fecha sozinho");
    }

    #[test]
    fn o_salto_de_sessao_fecha_o_que_estava_aberto() {
        let mut d = diario();
        d.tique(500.0);
        d.anotar(500.0, "frente", 7, "longe", 5.0, vazio);
        // Replay, rebobinada, sessão nova: o mundo é outro, e o que estava aberto descreve
        // o mundo velho — vai para o arquivo antes de a memória ser limpa.
        d.tique(3.0);
        let notas = d.drenar();
        assert_eq!(notas.len(), 1, "o episódio do mundo velho não se perde");

        d.anotar(3.0, "frente", 7, "longe", 5.0, vazio);
        correr(&mut d, 3.0, 4.5);
        assert_eq!(d.drenar().len(), 1, "e o mundo novo começa do zero");
    }

    /// O defeito que motivou separar o relógio do tique do relógio das notas.
    ///
    /// Trinta segundos entre duas recusas do mesmo par é corrida normal, e não replay. Aqui
    /// o tique corre INTERCALADO com as notas, que é como o amostrador chama os dois: sem
    /// isso o teste não exercita nem a poda nem a detecção de salto.
    #[test]
    fn nota_esparsa_nao_e_salto_de_sessao() {
        let mut d = diario();
        const DT: f64 = 1.0 / 60.0;
        for n in 0..(35 * 60) {
            let t = 10.0 + n as f64 * DT;
            d.tique(t);
            // Uma recusa aos 10 s e outra 30 s depois, com o mundo rodando entre elas.
            if n == 0 || n == 30 * 60 {
                d.anotar(t, "frente", SEM_ALVO, "perdeu_o_tique", SEM_FOLGA, vazio);
            }
        }
        assert_eq!(
            d.drenar().len(),
            2,
            "dois episódios separados por trinta segundos, e nenhum replay"
        );
    }

    /// Sem limiar não há folga, e a linha não pode inventar uma.
    #[test]
    fn motivo_sem_limiar_nao_ganha_coluna_de_folga() {
        let mut d = diario();
        d.anotar(10.0, "tras", SEM_ALVO, "campo_sem_ritmo", SEM_FOLGA, vazio);
        correr(&mut d, 10.0, 11.5);
        let notas = d.drenar();
        assert_eq!(notas.len(), 1);
        assert!(notas[0].detalhe.get("folga").is_none(), "SEM_FOLGA não vira número");
        assert!(notas[0].detalhe.get("durou_s").is_some(), "a duração vale para todos");
    }

    #[test]
    fn o_teto_avisa_uma_vez_e_para() {
        let mut d = diario();
        for n in 0..(MAX_NOTAS + 50) {
            d.anotar(n as f64, "frente", n as i32, "longe", 1.0, vazio);
            d.fechar("frente", n as i32);
            d.drenar();
        }
        assert_eq!(d.notas, MAX_NOTAS, "o teto segura a contagem");
        assert!(d.teto_avisado, "e o arquivo diz que parou de anotar");
    }

    /// A fila cheia não some calada: a linha `fila_cheia` conta o buraco.
    #[test]
    fn a_fila_cheia_deixa_o_proprio_rastro() {
        let mut d = diario();
        for n in 0..(MAX_FILA + 10) {
            d.anotar(n as f64, "frente", n as i32, "longe", 1.0, vazio);
            d.fechar("frente", n as i32);
        }
        let notas = d.drenar();
        assert_eq!(notas.len(), MAX_FILA + 1, "a fila no teto, mais a linha do buraco");
        let ultima = notas.last().expect("a fila não está vazia");
        assert_eq!(ultima.motivo, "fila_cheia");
        assert_eq!(ultima.detalhe["perdidas"], 10);
    }
}
