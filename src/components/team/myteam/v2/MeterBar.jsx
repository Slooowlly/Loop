import GarageRow, { GarageRule } from "./GarageRow";

// Medidor com régua: a barra é o seu valor, o traço claro é a média do grid.
//
// É o componente que responde "isso é muito?" — no v1 as barras técnicas iam de 0 a
// 100 sem referência nenhuma, então `56` de confiabilidade não queria dizer nada.
//
// `average` null é caso legítimo, não erro: folha salarial e presença pública não
// existem no payload das outras equipes, então esses dois medidores aparecem sem
// régua em vez de comparar contra um número inventado.
//
// A forma é a linha da folha (`GarageRow`), a mesma de salário e de custo por
// ponto: medidor e número solto ficam na mesma grade e a coluna da direita alinha
// de ponta a ponta do bloco.

const TONE_BARS = {
  good: "bg-status-green/80",
  warn: "bg-status-yellow/80",
  bad: "bg-status-red/80",
  neutral: "bg-accent-primary/80",
};

const TONE_TEXT = {
  good: "text-status-green",
  warn: "text-status-yellow",
  bad: "text-status-red",
  neutral: "text-text-primary",
};

// O ícone perdeu a placa de 44px que assumia o tom do medidor. Ela existia para
// distinguir três barras de forma idêntica pelo canto do olho, e cobrava caro por
// isso: era o objeto mais pesado da linha, empurrava rótulo e régua para a direita
// e repetia em cor o que a própria barra já dizia.
//
// Aqui o ícone volta ao tamanho de marca do rótulo, na cor apagada dele. A
// distinção entre medidores continua existindo pelo texto do rótulo, que é o que
// se lê de fato, e a cor volta a ser só sinal: verde, amarelo, vermelho na régua e
// no número.
//
// `Icon` é um componente do lucide, não um emoji nem um caractere: ele herda a cor
// do rótulo ao lado e some do fluxo do leitor de tela (`aria-hidden`), porque o
// rótulo escrito já diz a mesma coisa.
function MeterBar({ label, value, percent, averagePercent = null, caption = null, tone = "neutral", testId, Icon = null, divided = true }) {
  return (
    <GarageRow
      testId={testId}
      divided={divided}
      label={
        <span className="inline-flex items-center gap-1.5">
          {Icon ? <Icon size={13} strokeWidth={1.8} aria-hidden="true" className="shrink-0" /> : null}
          {label}
        </span>
      }
      value={value}
      valueTone={TONE_TEXT[tone] ?? TONE_TEXT.neutral}
      caption={caption}
    >
      <GarageRule
        percent={percent}
        averagePercent={averagePercent}
        barClass={TONE_BARS[tone] ?? TONE_BARS.neutral}
        markerTestId={testId ? `${testId}-average` : undefined}
      />
    </GarageRow>
  );
}

export default MeterBar;
