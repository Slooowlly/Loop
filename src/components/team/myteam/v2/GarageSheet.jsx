// A moldura da aba Minha Equipe: uma FOLHA, não um cartão.
//
// A aba inteira era feita de `GlassCard` — canto de 24 a 28px, vidro, sombra de
// 50px e respiro de 24px por bloco. Cinco desses empilhados na mesma tela leem
// como painel de produto web, e o custo real é de leitura: cada bloco flutua
// sozinho, então o olho não tem nenhuma linha vertical para descer. Números
// alinhados dentro de um cartão não alinham com os do cartão vizinho.
//
// Aqui a moldura recua: canto pequeno, borda de 1px, fundo chapado e nenhum
// deslocamento. O que separa bloco de bloco é filete, e é o filete que devolve a
// grade — a mesma coluna de números atravessa a folha de cima abaixo.
//
// `accentColor` desenha a barra da equipe na borda esquerda. É o único lugar da
// folha em que a cor entra por identidade; no resto dela a cor é sinal (verde,
// amarelo, vermelho) e nada mais.
function GarageSheet({ children, className = "", accentColor = null, testId = undefined, ...props }) {
  return (
    <section
      {...props}
      data-testid={testId}
      className={`relative overflow-hidden rounded-lg border border-white/[0.08] bg-app-bg/60 shadow-[0_8px_28px_rgba(0,0,0,0.30)] ${className}`}
    >
      {accentColor ? (
        <span
          aria-hidden="true"
          className="absolute inset-y-0 left-0 w-[3px]"
          style={{ backgroundColor: accentColor }}
        />
      ) : null}
      {children}
    </section>
  );
}

// O cabeçalho de um bloco da folha: rótulo à esquerda, nota de contexto à direita,
// filete embaixo. Sempre no mesmo tamanho e no mesmo espaçamento, para os blocos
// começarem na mesma altura quando ficam lado a lado.
//
// `value` é o veredito do bloco — o número que responde sozinho o que ele diz. Ele
// mora aqui, no alto à direita, e não no meio do conteúdo: é a mesma posição do
// caixa na faixa de comando e da colocação no cartão do piloto, então a tela inteira
// passa a ter uma coluna só de resposta, na mesma borda.
export function SheetHeader({ children, aside = null, value = null, valueTone = "text-text-primary" }) {
  return (
    <div className="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1 border-b border-white/[0.08] px-4 py-2.5">
      <p className="flex items-center gap-2 text-[10px] uppercase tracking-[0.22em] text-text-muted">{children}</p>
      <div className="flex items-baseline gap-3">
        {aside ? <p className="text-[10px] uppercase tracking-[0.16em] text-text-muted">{aside}</p> : null}
        {value === null ? null : (
          <p className={`font-garage text-[22px] font-semibold leading-none tabular-nums ${valueTone}`}>{value}</p>
        )}
      </div>
    </div>
  );
}

export default GarageSheet;
