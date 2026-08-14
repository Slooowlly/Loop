// Moldura comum das cinco seções da aba Carreira.
//
// Existe para o mesmo motivo que a aba existe: as seções falam do MESMO piloto, e
// se cada uma inventasse a própria caixa a tela leria como cinco telas grudadas.
// São só duas peças — o cartão com título e o número grande —, e nenhuma delas
// carrega texto: quem passa a prosa é a seção, sempre por `t()`.

// Cartão de seção: título discreto em cima, conteúdo dentro.
//
// `acao` é o canto direito do título (um seletor, um contador) e some quando não
// vem — sem ele o título ficaria com um vão à direita que se lê como falta.
export function Bloco({ titulo, acao = null, children, testId = undefined }) {
  return (
    <section
      data-testid={testId}
      className="rounded-2xl border border-white/[0.08] bg-black/15 px-4 py-3.5"
    >
      <div className="mb-3 flex items-baseline justify-between gap-4">
        <h3 className="text-[11px] font-semibold uppercase tracking-[0.16em] text-text-muted">
          {titulo}
        </h3>
        {acao}
      </div>
      {children}
    </section>
  );
}

// O número que a seção veio dizer, com o rótulo embaixo dele.
//
// Rótulo EMBAIXO, e não em cima: numa fileira de quatro, os números precisam
// alinhar entre si na primeira linha do olho, e rótulos de comprimentos
// diferentes acima deles desalinhavam a fileira inteira.
export function Numero({ valor, rotulo, tom = "text-text-primary", nota = null }) {
  return (
    <div className="min-w-0">
      <div
        className={`font-mono text-[26px] font-semibold leading-none tracking-[-0.03em] ${tom}`}
        style={{ fontVariantNumeric: "tabular-nums" }}
      >
        {valor}
      </div>
      <div className="mt-1.5 truncate text-[10px] font-semibold uppercase tracking-[0.16em] text-text-muted">
        {rotulo}
      </div>
      {nota ? <div className="mt-1 truncate text-[11px] text-text-secondary">{nota}</div> : null}
    </div>
  );
}

// Aviso de seção vazia. Uma frase, sem moldura própria: a caixa do `Bloco` já
// está desenhada em volta, e uma segunda borda dentro dela viraria buraco.
export function Vazio({ children }) {
  return <p className="py-1.5 text-sm leading-relaxed text-text-secondary">{children}</p>;
}
