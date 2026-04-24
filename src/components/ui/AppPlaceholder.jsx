function AppPlaceholder({ title, embedded = false }) {
  if (embedded) {
    return (
      <div className="glass-strong relative overflow-hidden rounded-[28px] px-8 py-12 text-center">
        <div className="absolute inset-0 bg-[radial-gradient(circle_at_top,rgba(111,212,255,0.10),transparent_34%),radial-gradient(circle_at_bottom,rgba(73,154,255,0.08),transparent_28%)]" />

        <div className="relative z-10 mx-auto max-w-xl">
          <p className="text-[11px] font-semibold uppercase tracking-[0.32em] text-sky-200/70">
            Em breve
          </p>
          <h2 className="mt-3 text-3xl font-semibold tracking-[0.04em] text-text-primary">
            {title}
          </h2>
          <p className="mt-4 text-sm leading-7 text-text-secondary">
            Esta area ainda esta em construcao, mas agora ja segue a identidade visual padrao do
            app para manter a navegacao consistente.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="app-shell px-4 py-6 text-text-primary sm:px-6 lg:px-10">
      <div className="app-backdrop" />

      <div className="relative mx-auto flex min-h-[calc(100vh-3rem)] max-w-5xl items-center justify-center">
        <div className="entry-panel w-full max-w-xl text-center">
          <p className="text-[11px] font-semibold uppercase tracking-[0.35em] text-sky-200/70">
            Em breve
          </p>
          <h1 className="text-3xl font-semibold tracking-[0.06em] text-slate-100">{title}</h1>
          <p className="max-w-md text-[12px] leading-6 text-slate-300/78">
            Esta tela ainda esta em desenvolvimento, mas ja usa a mesma base visual do restante do
            app para deixar tudo mais coeso.
          </p>
        </div>
      </div>
    </div>
  );
}

export default AppPlaceholder;
