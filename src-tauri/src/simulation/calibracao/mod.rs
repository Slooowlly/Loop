//! **Calibração da simulação** — a régua, não o motor.
//!
//! Este módulo não simula nada e não conserta nada. Ele MEDE: gera campos sintéticos
//! reprodutíveis, roda temporadas inteiras por cima da simulação real e extrai a distribuição
//! agregada dos resultados, comparando-a com faixas do que uma corrida crível produziria.
//!
//! A razão de existir é o erro que criou o problema que ele mede: constantes de simulação foram
//! ajustadas por sensação, sem nada medindo o agregado. Enquanto a régua não existia, "as
//! corridas estão sempre iguais" era uma reclamação; com ela, é uma falha de teste.
//!
//! ## Peças
//!
//! - [`campo`] — gerador de grid sintético com distribuição de cauda, parametrizável e semeado.
//! - [`arena`] — roda eventos, temporadas e campanhas pelo mesmo caminho da carreira jogável.
//! - [`metricas`] — as métricas de RESULTADO: dizem se a distribuição final melhorou.
//! - [`variancia`] — a decomposição do orçamento (piloto / carro / evento / corrida): diz o que
//!   ajustar. É a peça que transforma calibração em decisão informada.
//! - [`processo`] — as métricas de COMO a corrida acontece (trocas, gaps, poder da largada).
//! - [`pressao`] — a régua da camada de clutch/choke. É a única que fecha o CICLO DE PONTOS
//!   (etapa → classificação → situação de título → esteira da etapa seguinte), porque sem ele a
//!   pressão de campeonato não tem entrada e as constantes de `simulation::pressure` ficam sem
//!   como ser medidas.
//! - [`atrito`] — as métricas e os ALVOS do pacote D (ar sujo e ultrapassagem), definidos antes
//!   de D existir para que ele seja construído contra alvo em vez de ajustado depois.
//! - [`consumo`] — guarda contra knob calculado e nunca lido, generalizando o achado do
//!   `overtaking_difficulty_multiplier`.
//! - [`ancora`] — comparação contra resultado REAL do iRacing, o único padrão-ouro disponível.
//!   Sem dado no repositório hoje; o ingestor existe e o protocolo de coleta está escrito.
//! - [`varredura`] — quais knobs têm alavanca real e quais são decorativos.
//! - [`busca`] — a máquina de calibração: descida coordenada contra as faixas, com o caminho de
//!   fracasso explícito. Validada contra o espaço de parâmetros atual, onde o fracasso é
//!   conhecido.
//! - [`snapshot`] — o baseline congelado, para diff antes/depois de cada pacote.
//! - [`alvos`] — as faixas-alvo, com o argumento de cada uma escrito ao lado.
//! - [`relatorio`] — as tabelas em texto.
//!
//! ## Como usar
//!
//! ```ignore
//! use crate::simulation::calibracao::{arena::ConfigTemporada, alvos::Alvos, arena, relatorio};
//!
//! let m = arena::medir("rookie", &ConfigTemporada::rookie(), 40, 7);
//! println!("{}", relatorio::tabela(&m, &Alvos::entrada()));
//! ```
//!
//! O baseline medido antes de qualquer conserto está em `BASELINE.md`, ao lado deste arquivo.
//!
//! Os testes pesados (~1000 corridas por asserção) estão marcados `#[ignore]` para não pesar no
//! CI. Rode-os com:
//!
//! ```text
//! cargo test --manifest-path src-tauri/Cargo.toml calibracao -- --ignored --nocapture
//! ```

#![allow(dead_code)]

pub mod alvos;
pub mod ancora;
pub mod arena;
pub mod assinatura;
pub mod atrito;
pub mod busca;
pub mod campo;
pub mod consumo;
pub mod metricas;
pub mod pressao;
pub mod previa;
pub mod processo;
pub mod relatorio;
pub mod seguranca;
pub mod snapshot;
pub mod variancia;
pub mod varredura;

#[cfg(test)]
mod tests;
