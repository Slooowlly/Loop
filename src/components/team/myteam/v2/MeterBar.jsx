// Medidor com régua: a barra é o seu valor, a marca clara é a média do grid.
//
// É o componente que responde "isso é muito?" — no v1 as barras técnicas iam de 0 a
// 100 sem referência nenhuma, então `56` de confiabilidade não queria dizer nada.
//
// `average` null é caso legítimo, não erro: folha salarial e presença pública não
// existem no payload das outras equipes, então esses dois medidores aparecem sem
// régua em vez de comparar contra um número inventado.

const TONE_BARS = {
  good: "bg-status-green",
  warn: "bg-status-yellow",
  bad: "bg-status-red",
  neutral: "bg-accent-primary",
};

const TONE_TEXT = {
  good: "text-status-green",
  warn: "text-status-yellow",
  bad: "text-status-red",
  neutral: "text-text-primary",
};

// O ícone vive numa placa própria e ASSUME o tom do medidor. Não é enfeite: quando a
// barra, o número e a placa concordam, dá para ler a linha inteira pela cor, sem
// passar pelo texto — e é isso que faz três medidores de forma idêntica pararem de
// se confundir.
//
// A placa é o COMEÇO da linha, não um adorno no meio do rótulo: ela ocupa uma coluna
// própria à esquerda e tudo — rótulo, número, barra e legenda — nasce da borda
// direita dela. É o que dá ao bloco um eixo vertical em vez de três ícones flutuando.
const TONE_TILE = {
  good: "border-status-green/25 bg-status-green/10 text-status-green",
  warn: "border-status-yellow/25 bg-status-yellow/10 text-status-yellow",
  bad: "border-status-red/25 bg-status-red/10 text-status-red",
  neutral: "border-white/10 bg-white/[0.06] text-text-secondary",
};

// `Icon` é um componente do lucide, não um emoji nem um caractere: o ícone herda a
// cor do texto ao lado e some do fluxo para leitor de tela (`aria-hidden`), porque o
// rótulo escrito já diz a mesma coisa.
function MeterBar({ label, value, percent, averagePercent = null, caption = null, tone = "neutral", testId, Icon = null }) {
  const clampedPercent = Math.max(0, Math.min(100, percent ?? 0));
  const clampedAverage = averagePercent === null ? null : Math.max(0, Math.min(100, averagePercent));

  return (
    <div data-testid={testId} className="flex items-start gap-3.5">
      {Icon ? (
        <span className={`grid h-11 w-11 shrink-0 place-items-center rounded-2xl border ${TONE_TILE[tone] ?? TONE_TILE.neutral}`}>
          <Icon size={24} strokeWidth={1.6} aria-hidden="true" />
        </span>
      ) : null}
      <div className="min-w-0 flex-1">
        <div className="flex items-center justify-between gap-3">
          <span className="text-sm text-text-secondary">{label}</span>
          <span className={`font-mono text-sm ${TONE_TEXT[tone] ?? TONE_TEXT.neutral}`}>{value}</span>
        </div>
        <div className="relative mt-2 h-2 rounded-full bg-white/10">
          <div
            className={`h-2 rounded-full ${TONE_BARS[tone] ?? TONE_BARS.neutral}`}
            style={{ width: `${Math.max(clampedPercent, clampedPercent > 0 ? 1.5 : 0)}%` }}
          />
          {clampedAverage === null ? null : (
            <span
              data-testid={testId ? `${testId}-average` : undefined}
              className="absolute top-[-3px] h-[14px] w-[2px] rounded-full bg-text-primary"
              style={{ left: `calc(${clampedAverage}% - 1px)` }}
            />
          )}
        </div>
        {caption ? <p className="mt-1.5 text-xs leading-5 text-text-muted">{caption}</p> : null}
      </div>
    </div>
  );
}

export default MeterBar;
