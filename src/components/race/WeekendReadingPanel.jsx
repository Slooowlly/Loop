import { useTranslation } from "react-i18next";

// A LEITURA DO FIM DE SEMANA — o card que transforma "ele foi mal nesta etapa" em causa.
//
// A campanha de calibração vai AUMENTAR a variação que vem do fim de semana. O argumento
// de que isso não é sorte é que as fontes são atribuíveis a qualidade — mas atribuível é
// propriedade do modelo e atribuir é ato do jogador. Sem este card, a segunda metade não
// acontece, e da poltrona a variação é indistinguível de dado.
//
// TRÊS camadas, cortadas no eixo do TEMPO (o princípio organizador do `forma.rs`):
//
//   a pista        → permanente por circuito, ρ = 1 entre visitas ao mesmo lugar
//   o seu momento  → serial, ρ = 0,65 entre etapas — a ÚNICA com tendência
//   este fim de semana → evento isolado, ρ = 0, não diz nada sobre a próxima
//
// Fundir duas quaisquer mistura história com ruído. O acerto é o único sem informação
// sobre qualquer corrida futura, então é ele que fica sozinho.
//
// Mora na Sala de Estratégia, ANTES da corrida, por princípio: informação que só aparece
// depois do resultado é indistinguível de desculpa. O debrief pós-corrida fecha o que foi
// anunciado aqui — previsão que se cumpre ensina o mecanismo, explicação avulsa não
// ensina nada.
//
// Sem número na tela, nunca: faixa ordinal em [-2,2] vira palavra. Ver
// docs/fase3-fim-de-semana-atribuivel.md.

// -2..+2 → chave de i18n e cor. Um mapa só, num lugar só: quando a calibração mudar as
// escalas, o que muda é o mapeamento valor→faixa no backend, não isto.
const FAIXAS = {
  "-2": { key: "against", color: "#f87171" },
  "-1": { key: "below", color: "#f0b37a" },
  0: { key: "neutral", color: "#8b949e" },
  1: { key: "favor", color: "#58a6ff" },
  2: { key: "strongFavor", color: "#4ade80" },
};

const TENDENCIAS = { "-1": { glyph: "↘", key: "falling" }, 0: { glyph: "→", key: "steady" }, 1: { glyph: "↗", key: "rising" } };

// Medidor de 5 blocos. Sem eixo, sem escala, sem número — a posição acesa É a leitura, e
// é tudo o que atribuir exige (saber QUE houve causa e QUAL, não o tamanho em pontos).
function Medidor({ band, color }) {
  return (
    <span className="flex items-center gap-[3px]" aria-hidden="true">
      {[-2, -1, 0, 1, 2].map((slot) => (
        <span
          key={slot}
          className="inline-block rounded-[2px]"
          style={{
            width: 6,
            height: slot === band ? 14 : 8,
            background: slot === band ? color : "rgba(255,255,255,0.10)",
          }}
        />
      ))}
    </span>
  );
}

function Camada({ labelKey, layer, t }) {
  const faixa = FAIXAS[String(layer.band)] ?? FAIXAS[0];
  // Só mostramos o canal de classificação quando ele DIVERGE do ritmo. Igual, seria
  // ruído; diferente, é a explicação de voar no sábado e não converter no domingo.
  const quali =
    layer.qualifying_band !== layer.band ? FAIXAS[String(layer.qualifying_band)] ?? null : null;
  const tend = layer.trend != null ? TENDENCIAS[String(layer.trend)] : null;

  return (
    <div className="flex flex-col gap-1 py-2">
      <div className="flex items-center justify-between gap-3">
        <span className="text-[10px] uppercase tracking-widest text-gray-500 font-bold">
          {t(labelKey)}
        </span>
        <span className="flex items-center gap-2">
          <Medidor band={layer.band} color={faixa.color} />
          <span style={{ color: faixa.color }} className="text-[12px] font-semibold">
            {t(`nextRaceTab.weekendReading.bands.${faixa.key}`)}
          </span>
          {tend && (
            <span
              style={{ color: faixa.color }}
              className="text-[13px] leading-none"
              title={t(`nextRaceTab.weekendReading.trends.${tend.key}`)}
            >
              {tend.glyph}
            </span>
          )}
        </span>
      </div>
      {/* O mecanismo NOMEADO, não o efeito. "Fim de semana difícil" é horóscopo; "o carro
          não respondeu ao acerto" é mecanismo — o primeiro o jogador arquiva como sabor,
          o segundo ele usa pra prever. */}
      {layer.support && (
        <p className="text-[11px] leading-snug text-gray-400">{layer.support}</p>
      )}
      {quali && (
        <p className="text-[10px] leading-snug" style={{ color: quali.color }}>
          {t("nextRaceTab.weekendReading.qualifyingSplit", {
            band: t(`nextRaceTab.weekendReading.bands.${quali.key}`),
          })}
        </p>
      )}
    </div>
  );
}

function WeekendReadingPanel({ reading }) {
  const { t } = useTranslation();

  // Mesma regra do vazio da v55: sem leitura, nada na tela. Um card com três "neutro"
  // fabricados diria ao jogador que o fim de semana está morno quando na verdade o jogo
  // não sabe — e leitura errada é pior que leitura ausente.
  if (!reading?.available) return null;
  const { track_affinity: pista, driver_form: momento, car_setup: acerto } = reading;
  if (!pista || !momento || !acerto) return null;

  return (
    <div className="bg-[#161b22]/40 backdrop-blur-[24px] border border-white/5 shadow-[0_8px_32px_rgba(0,0,0,0.2)] rounded-3xl p-5">
      <p className="text-[11px] uppercase tracking-[0.2em] text-[#f5c76d] font-bold flex items-center mb-1">
        <span className="mr-2 text-sm">📻</span>
        {t("nextRaceTab.weekendReading.title")}
      </p>
      {/* Ordem = relógio de cada camada, do mais lento ao mais rápido. Não é estética: é
          a mesma ordem do `forma.rs`, e ela ensina implicitamente que as três têm
          permanências diferentes. */}
      <div className="divide-y divide-white/5">
        <Camada labelKey="nextRaceTab.weekendReading.layers.track" layer={pista} t={t} />
        <Camada labelKey="nextRaceTab.weekendReading.layers.form" layer={momento} t={t} />
        <Camada labelKey="nextRaceTab.weekendReading.layers.setup" layer={acerto} t={t} />
      </div>
    </div>
  );
}

export default WeekendReadingPanel;
