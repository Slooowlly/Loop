//! O esqueleto que a família de spotters compartilha.
//!
//! Os seis irmãos ([`super::spotter_frente`], [`super::spotter_tras`],
//! [`super::spotter_lento`], [`super::spotter_voltar`], [`super::spotter_bandeira`],
//! [`super::spotter_boxe`]) somam milhares de linhas, e a parte que os torna DIFERENTES é
//! pequena: cada um tem meia dúzia de limiares medidos numa captura de corrida. O resto —
//! o singleton do observador, o teto da fila de episódios, a detecção de salto do
//! `SessionTime`, a distância adiante na volta, os valores de superfície do SDK — era a
//! mesma coisa escrita seis vezes.
//!
//! Seis cópias de uma infraestrutura significam seis lugares para corrigir o próximo bug
//! de fila, e é assim que dois irmãos passam a divergir sem ninguém decidir que
//! divergissem. O que mora aqui é o que NÃO é calibração; o que ficou em cada arquivo é o
//! que a medição daquele módulo definiu.
//!
//! Nada aqui tem estado. O singleton é gerado por macro DENTRO de cada módulo, então cada
//! irmão continua com o próprio `OnceLock` — o compartilhamento é de código, não de dados.

// ─── Contrato do SDK ─────────────────────────────────────────────────────────
// Valores publicados pelo iRacing. Não são calibração: trocar um número aqui não desafina
// o spotter, faz ele ler outra coisa.

/// Quantos índices de carro o SDK publica em `cars[]`. Teto dos vetores por carro.
pub(crate) const MAX_CARROS: usize = 64;

/// `CarIdxTrackSurface`: fora do mundo (garagem, guincho, ainda não entrou).
pub(crate) const SUP_FORA_DO_MUNDO: i32 = -1;
/// `CarIdxTrackSurface`: fora da pista (grama, brita, escapatória).
pub(crate) const SUP_FORA_DA_PISTA: i32 = 0;
/// `CarIdxTrackSurface`: parado na própria caixa.
pub(crate) const SUP_NA_CAIXA: i32 = 1;
/// `CarIdxTrackSurface`: na via de boxes.
pub(crate) const SUP_ENTRANDO_BOX: i32 = 2;
/// `CarIdxTrackSurface`: no asfalto.
pub(crate) const SUP_NA_PISTA: i32 = 3;
/// `SessionState`: corrida em andamento.
pub(crate) const ESTADO_CORRIDA: i32 = 4;

// ─── Continuidade do relógio ─────────────────────────────────────────────────

/// Salto de `session_time` acima do qual o mundo não é mais o mesmo: replay, rebobinada,
/// sessão nova. Todo irmão zera o próprio estado nessa borda, porque a alternativa é
/// interpretar a descontinuidade como física — um carro que "andou" meia volta em um
/// tique, um episódio que dura o replay inteiro.
///
/// Cinco segundos são folga grande de propósito. O custo de zerar sem precisar é um
/// punhado de tiques sem fala; o de NÃO zerar é uma medição inventada entrando no
/// histórico de calibração.
pub(crate) const SALTO_MAX_S: f64 = 5.0;

/// Sumiu de `cars[]` por mais que isto: guincho, garagem, saiu do mundo.
///
/// O caso que originou a regra está medido no [`super::spotter_frente`]: dois carros
/// rebocados em Okayama produziram episódios de 172 e 173 s, porque um carro guinchado
/// simplesmente para de aparecer no vetor e o estado aberto sobre ele nunca fechava.
pub(crate) const AUSENCIA_MAX_S: f64 = 1.0;

/// Janela da velocidade derivada da variação de `LapDistPct`.
///
/// O SDK não publica velocidade dos OUTROS carros — só a do jogador. A dos demais sai da
/// distância percorrida na janela, e o tamanho dela é um equilíbrio: curta demais e o
/// ruído de amostragem vira aceleração fantasma; longa demais e uma freada some na média.
pub(crate) const JANELA_VEL_S: f64 = 0.25;

// ─── Geometria da volta ──────────────────────────────────────────────────────

/// Metros de `de_pct` até `para_pct` ANDANDO PARA A FRENTE, fechando o círculo da volta.
///
/// Sempre positivo. É a conta que responde "quanto falta para ele me alcançar" sem que a
/// linha de chegada entre no meio: quem está em 0,98 e persegue quem está em 0,02 está a
/// 4% da pista atrás, não a 96% na frente.
pub(crate) fn adiante(de_pct: f64, para_pct: f64, comprimento_m: f64) -> f64 {
    let mut d = para_pct - de_pct;
    if d < 0.0 {
        d += 1.0;
    }
    d * comprimento_m
}

/// O relógio da sessão andou para trás ou pulou mais que [`SALTO_MAX_S`]?
pub(crate) fn saltou(anterior_s: f64, agora_s: f64) -> bool {
    agora_s < anterior_s || agora_s - anterior_s > SALTO_MAX_S
}

/// O mesmo, para quem guarda o instante anterior como `Option` — o primeiro tique de uma
/// sessão não é salto, é a ausência de referência.
pub(crate) fn saltou_desde(anterior_s: Option<f64>, agora_s: f64) -> bool {
    anterior_s.is_some_and(|ant| saltou(ant, agora_s))
}

// ─── Fila de episódios ───────────────────────────────────────────────────────

/// Guarda mais um episódio encerrado, descartando os mais antigos acima do teto.
///
/// Os episódios existem para CALIBRAÇÃO: eles vão para o `loop.log` e é deles que saem os
/// números medidos de cada irmão. O teto é o que impede uma sessão de quatro horas de
/// virar consumo de memória sem limite — e é sempre o mais VELHO que sai, porque a última
/// hora de corrida descreve melhor o que está sendo calibrado do que a primeira.
pub(crate) fn empurrar_com_teto<T>(fila: &mut Vec<T>, item: T, teto: usize) {
    fila.push(item);
    while fila.len() > teto {
        fila.remove(0);
    }
}

// ─── Singleton do observador ─────────────────────────────────────────────────

/// Declara o `observador()`/`lock()` de um irmão da família.
///
/// Cada spotter tem UM observador de processo, alimentado pelo amostrador a 60 Hz e lido
/// pelo front a 2 Hz. O `unwrap_or_else(into_inner)` é deliberado: um pânico dentro do
/// lock envenenaria o mutex e emudeceria o spotter pelo resto da sessão, e um spotter mudo
/// é pior do que um spotter que segue com o estado que tinha.
///
/// A macro é declarada em cada módulo (e não uma função genérica aqui) porque o `static`
/// precisa ser monomórfico — cada irmão fica com o próprio `OnceLock`, sem estado
/// compartilhado entre eles.
macro_rules! spotter_singleton {
    ($tipo:ty, $criar:expr) => {
        fn observador() -> &'static std::sync::Mutex<$tipo> {
            static OBS: std::sync::OnceLock<std::sync::Mutex<$tipo>> = std::sync::OnceLock::new();
            OBS.get_or_init(|| std::sync::Mutex::new($criar))
        }

        fn lock() -> std::sync::MutexGuard<'static, $tipo> {
            observador().lock().unwrap_or_else(|e| e.into_inner())
        }
    };
}

pub(crate) use spotter_singleton;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adiante_fecha_o_circulo_da_volta() {
        // Quem está em 0,98 perseguindo quem está em 0,02 está 4% atrás, não 96% à frente.
        assert!((adiante(0.98, 0.02, 1000.0) - 40.0).abs() < 1e-9);
        assert!((adiante(0.10, 0.60, 1000.0) - 500.0).abs() < 1e-9);
        assert_eq!(adiante(0.5, 0.5, 1000.0), 0.0);
    }

    #[test]
    fn salto_pega_rebobinada_e_avanco() {
        assert!(saltou(100.0, 99.0), "relógio andou para trás");
        assert!(saltou(100.0, 100.0 + SALTO_MAX_S + 0.1), "pulou adiante");
        assert!(!saltou(100.0, 100.016), "um tique normal não é salto");
        assert!(!saltou(100.0, 100.0 + SALTO_MAX_S), "o limite não estoura");
    }

    #[test]
    fn o_primeiro_tique_da_sessao_nao_e_salto() {
        assert!(!saltou_desde(None, 5000.0));
        assert!(saltou_desde(Some(5000.0), 10.0));
    }

    #[test]
    fn a_fila_descarta_o_mais_velho() {
        let mut fila: Vec<i32> = Vec::new();
        for n in 0..5 {
            empurrar_com_teto(&mut fila, n, 3);
        }
        assert_eq!(fila, vec![2, 3, 4], "o teto tem de comer pela frente");
    }
}
