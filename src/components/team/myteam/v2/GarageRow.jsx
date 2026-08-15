// A linha da folha: rótulo à esquerda, o meio livre para a régua ou para o texto de
// contexto, e o número encostado na direita.
//
// As três colunas são as MESMAS em todos os blocos da aba. É esse alinhamento que
// faz a folha parecer folha: descendo a tela, todo número cai na mesma coluna, e
// comparar salário com custo por ponto vira leitura vertical em vez de caça. Se
// cada bloco escolhesse a própria largura de rótulo, a grade sumia e sobrava um
// monte de linha parecida.
//
// A grade é constante exportada, e não classe repetida em cada arquivo, porque o
// alinhamento só existe enquanto os quatro blocos usarem o mesmo valor.
//
// 116px na coluna do rótulo não é arredondamento: cada linha é uma grade própria, e
// largura `auto` faria cada uma medir o próprio rótulo — a coluna do meio começaria
// num lugar diferente a cada linha e a folha perderia justamente o alinhamento que
// ela existe para ter. O valor é medido, não estimado: 108px cortava
// "Confiabilidade" em 4px, que é o rótulo mais longo sem espaço para quebrar.
// Rótulo de duas palavras quebra em duas linhas e não estoura a coluna.
export const GRADE_DA_LINHA = "grid grid-cols-[116px_minmax(0,1fr)_auto] items-center gap-x-3 gap-y-1";

// A régua da linha: barra reta com filete de base, e um traço claro na média.
//
// Ela substituiu a barra de canto arredondado de 8px de altura. Reta e fina, ela
// lê como escala de instrumento; arredondada, lê como barra de progresso de
// download — e o dado aqui nunca é progresso rumo a 100.
export function GarageRule({ percent, averagePercent = null, barClass = "bg-accent-primary", markerTestId = undefined }) {
  const preenchida = Math.max(0, Math.min(100, percent ?? 0));
  const media = averagePercent === null || averagePercent === undefined ? null : Math.max(0, Math.min(100, averagePercent));

  return (
    <div className="relative h-[9px] border-b border-white/[0.14]">
      <div
        className={`absolute inset-y-0 left-0 ${barClass}`}
        style={{ width: `${Math.max(preenchida, preenchida > 0 ? 1.5 : 0)}%` }}
      />
      {media === null ? null : (
        <span
          data-testid={markerTestId}
          className="absolute -top-[3px] bottom-[-3px] w-px bg-text-primary"
          style={{ left: `calc(${media}% - 0.5px)` }}
        />
      )}
    </div>
  );
}

// `caption` cai na segunda linha da mesma grade, começando na coluna do meio: a
// legenda nasce alinhada com a régua que ela explica, e não com a margem do bloco.
function GarageRow({
  label,
  children = null,
  value = null,
  valueTone = "text-text-primary",
  caption = null,
  testId = undefined,
  divided = true,
}) {
  return (
    <div
      data-testid={testId}
      className={`${GRADE_DA_LINHA} py-1.5 ${divided ? "border-b border-dotted border-white/[0.10]" : ""}`}
    >
      <span className="text-[10px] uppercase leading-4 tracking-[0.12em] text-text-muted">{label}</span>
      <div className="min-w-0 text-[11px] text-text-secondary">{children}</div>
      {value === null ? (
        <span />
      ) : (
        <span className={`whitespace-nowrap font-garage text-[13px] tabular-nums ${valueTone}`}>{value}</span>
      )}
      {caption ? <div className="col-span-2 col-start-2 text-[11px] leading-4 text-text-muted">{caption}</div> : null}
    </div>
  );
}

export default GarageRow;
