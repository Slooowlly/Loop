use crate::simulation::track_profile::TrackCharacter;

// ---------------------------------------------------------------------------
// Dificuldade de pista (mapa estático para pistas com identidade reconhecida)
// ---------------------------------------------------------------------------

pub(super) fn track_difficulty_for(track_id: u32) -> f64 {
    match track_id {
        249 => 1.6, // Nordschleife
        219 => 1.5, // Mount Panorama / Bathurst
        252 => 1.5, // Nürburgring Combined 24h
        523 => 1.4, // Spa
        527 => 1.3, // Cadwell Park
        168 => 1.3, // Suzuka
        413 => 1.2, // Hungaroring (técnico/lento)
        268 => 1.2, // Le Mans (longo e exigente)
        554 => 0.9, // Charlotte Roval (mais fácil de ultrapassar)
        192 => 0.9, // Daytona Road (roval)
        // ── Venues novos (pesquisados) ──
        405 => 1.45, // Chicago Street (90°, muros, baixa aderência)
        580 => 1.35, // Adelaide (street, muros)
        589 => 1.35, // Coronado (airbase, muros de concreto)
        584 => 1.35, // St. Petersburg (bumpy, muros)
        218 => 1.30, // Gilles Villeneuve (Wall of Champions)
        521 => 1.30, // Sachsenring (waterfall cego, Omega)
        481 => 1.30, // Willow Springs (sweepers rápidos, cegos)
        539 => 1.25, // Miami (setor técnico murado)
        585 => 1.20, // Barber (ápices cegos, elevação)
        423 => 1.20, // Knockhill (ondulado, cego)
        475 => 1.15, // Aragón GP (esses cegos, comprometido)
        476 => 1.12, // Aragón National
        473 => 1.10, // Jerez
        _ => 1.0,    // baseline
    }
}

/// Calibre da dificuldade de ultrapassagem, medido e não escolhido.
///
/// Os valores por caráter nasceram numa escala em que o knob era **morto por inexistência** —
/// ninguém lia o campo, então qualquer número servia. Depois que o pacote D ligou o fio e as
/// camadas de evento passaram a tirar carro de posição, ele virou a alavanca mais forte da
/// varredura inteira: Δρ = 0,176 na gt3 e 0,183 na rookie, contra 0,046 do segundo colocado.
///
/// A varredura em cima da árvore calibrada, na rookie:
///
/// | valor | ρ(N,N+1) | desvio | vencedores |
/// |-------|----------|--------|------------|
/// | 1,00  | 0,569    | 3,62   | 4,47       |
/// | 2,50  | **0,503**| **3,90**| 4,80      |
/// | 5,00  | 0,483    | 3,99   | 4,80       |
///
/// 2,2 fica no joelho: pega quase todo o efeito (de 5,0 em diante o retorno some) sem empurrar
/// o knob para uma borda, que é o sinal de alerta nº 1 da máquina de busca.
///
/// **O preço é real e está declarado**: atrito maior faz a posição de largada persistir mais,
/// então ρ(grid × chegada) e vitórias do pole SOBEM. As métricas que faltam puxam para lados
/// opostos, e nenhum valor deste knob resolve as duas — isso é limite de mecanismo, não de
/// calibração.
const CALIBRE_DE_ULTRAPASSAGEM: f64 = 2.2;

pub(super) fn overtaking_difficulty_for(character: TrackCharacter) -> f64 {
    // As razões entre caráteres são design (Roval passa fácil, Tight é fechada) e sobrevivem
    // intactas ao calibre — o que mudou foi a escala da família, não a ordem dentro dela.
    CALIBRE_DE_ULTRAPASSAGEM
        * match character {
            TrackCharacter::Roval => 0.80,
            TrackCharacter::Flowing => 0.90,
            TrackCharacter::Technical => 1.00,
            TrackCharacter::Tight => 1.15,
        }
}
