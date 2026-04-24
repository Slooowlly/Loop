import { formatSalary } from "../../utils/formatters";

function PerformanceGroup({ title, items }) {
  return (
    <div className="glass-light rounded-xl p-4">
      <div className="mb-3 text-[10px] font-bold uppercase tracking-[0.22em] text-[#7d8590]">
        {title}
      </div>
      <div className="grid grid-cols-2 gap-3">
        {items.map((item) => (
          <div key={item.label} className="rounded-lg border border-white/6 bg-black/10 p-2.5">
            <div className="text-lg font-bold text-[#e6edf3]">{formatStatValue(item.value)}</div>
            <div className="text-[10px] uppercase tracking-[0.16em] text-[#7d8590]">
              {item.label}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function FormChip({ entry }) {
  const isDnf = entry?.dnf;
  const label = isDnf ? "DNF" : `P${entry?.chegada ?? "-"}`;
  const tone = isDnf
    ? "bg-[#f85149]/15 text-[#f85149]"
    : entry?.chegada <= 3
      ? "bg-[#d29922]/18 text-[#d29922]"
      : entry?.chegada <= 10
        ? "bg-[#3fb950]/15 text-[#3fb950]"
        : "bg-white/8 text-[#c9d1d9]";

  return (
    <div className="glass-light rounded-xl px-3 py-2 text-center">
      <div className={["rounded-lg px-2 py-1 text-sm font-bold", tone].join(" ")}>{label}</div>
      <div className="mt-2 text-[10px] uppercase tracking-[0.16em] text-[#7d8590]">
        R{entry?.rodada}
      </div>
    </div>
  );
}

function TimelineItem({ item }) {
  return (
    <div className="relative pl-5">
      <span className="absolute left-0 top-1.5 h-2.5 w-2.5 rounded-full bg-[#58a6ff]" />
      <div className="text-xs font-semibold uppercase tracking-[0.16em] text-[#7d8590]">
        {item.tipo}
      </div>
      <div className="mt-1 text-sm font-semibold text-[#e6edf3]">{item.titulo}</div>
      <div className="mt-1 text-xs text-[#7d8590]">{item.descricao}</div>
    </div>
  );
}

function formatStatValue(value) {
  if (value === null || value === undefined) return "-";
  return value;
}

function formatAverage(value) {
  if (value === null || value === undefined) return "-";
  return value.toFixed(1);
}

export function formatMoment(momento) {
  const map = {
    forte: { label: "Em alta", color: "text-[#3fb950]" },
    estavel: { label: "Estável", color: "text-[#d29922]" },
    em_baixa: { label: "Em baixa", color: "text-[#f85149]" },
    sem_dados: { label: "Sem dados", color: "text-[#7d8590]" },
  };

  return map[momento] || map.sem_dados;
}

export function PerformanceSection({ SectionComponent, title, stats }) {
  return (
    <SectionComponent title={title}>
      <div className="grid gap-3 lg:grid-cols-[1.35fr_0.65fr]">
        <PerformanceGroup
          title="Corrida"
          items={[
            { label: "Vitórias", value: stats?.vitorias },
            { label: "Pódios", value: stats?.podios },
            { label: "Top 10", value: stats?.top_10 },
            { label: "Fora Top 10", value: stats?.fora_top_10 },
          ]}
        />
        <PerformanceGroup
          title="Ritmo"
          items={[{ label: "Voltas rápidas", value: stats?.voltas_rapidas }]}
        />
        <PerformanceGroup
          title="Resistência"
          items={[
            { label: "Corridas", value: stats?.corridas },
            { label: "DNFs", value: stats?.dnfs },
          ]}
        />
      </div>
    </SectionComponent>
  );
}

export function FormSection({ SectionComponent, detail, moment }) {
  return (
    <SectionComponent title="Forma Atual">
      <div className="grid gap-4 lg:grid-cols-[1.2fr_0.8fr]">
        <div>
          <div className="mb-2 text-[10px] font-bold uppercase tracking-[0.18em] text-[#7d8590]">
            Últimas 5 corridas
          </div>
          <div className="grid grid-cols-5 gap-2">
            {detail.forma.ultimas_5?.length ? (
              detail.forma.ultimas_5.map((entry) => (
                <FormChip key={`form-${entry.rodada}`} entry={entry} />
              ))
            ) : (
              <div className="col-span-5 glass-light rounded-xl p-3 text-xs text-[#7d8590]">
                Sem corridas suficientes para medir o momento.
              </div>
            )}
          </div>
        </div>

        <div className="glass-light rounded-xl p-4">
          <div className="mb-2 text-[10px] font-bold uppercase tracking-[0.18em] text-[#7d8590]">
            Leitura rápida
          </div>
          <div className="grid gap-3">
            <div>
              <div className="text-[10px] uppercase tracking-[0.16em] text-[#7d8590]">
                Média de chegada
              </div>
              <div className="mt-1 text-2xl font-bold text-[#e6edf3]">
                {formatAverage(detail.forma.media_chegada)}
              </div>
            </div>
            <div className="flex items-center justify-between">
              <div>
                <div className="text-[10px] uppercase tracking-[0.16em] text-[#7d8590]">
                  Tendência
                </div>
                <div className="mt-1 text-2xl font-bold text-[#e6edf3]">
                  {detail.forma.tendencia}
                </div>
              </div>
              <div className={["text-sm font-semibold", moment.color].join(" ")}>{moment.label}</div>
            </div>
          </div>
        </div>
      </div>
    </SectionComponent>
  );
}

export function CareerSection({ SectionComponent, detail, trajetoria }) {
  return (
    <>
      <PerformanceSection SectionComponent={SectionComponent} title="Carreira" stats={detail.performance?.carreira} />

      <SectionComponent title="Trajetória">
        <div className="grid gap-4 lg:grid-cols-[0.9fr_1.1fr]">
          <div className="glass-light rounded-xl p-4">
            <div className="grid gap-3">
              <div>
                <div className="text-[10px] uppercase tracking-[0.16em] text-[#7d8590]">
                  Ano de estreia
                </div>
                <div className="mt-1 text-lg font-bold text-[#e6edf3]">
                  {trajetoria?.ano_estreia ?? "-"}
                </div>
              </div>
              <div>
                <div className="text-[10px] uppercase tracking-[0.16em] text-[#7d8590]">
                  Equipe de estreia
                </div>
                <div className="mt-1 text-sm text-[#e6edf3]">
                  {trajetoria?.equipe_estreia || "Não identificada"}
                </div>
              </div>
              <div>
                <div className="text-[10px] uppercase tracking-[0.16em] text-[#7d8590]">
                  Vivência atual
                </div>
                <div className="mt-1 text-sm text-[#e6edf3]">
                  {trajetoria?.temporadas_na_categoria ?? 0} temporada(s) ·{" "}
                  {trajetoria?.corridas_na_categoria ?? 0} corrida(s) na categoria
                </div>
              </div>
              <div className="rounded-xl border border-[#d29922]/18 bg-[#d29922]/7 p-3">
                <div className="text-[10px] uppercase tracking-[0.16em] text-[#d29922]">
                  Campeonatos
                </div>
                <div className="mt-2 flex items-center justify-between gap-3">
                  <div className="text-2xl font-bold text-[#e6edf3]">
                    {trajetoria?.titulos ?? 0}
                  </div>
                  <div
                    className={[
                      "rounded-full px-2.5 py-1 text-[10px] font-bold uppercase tracking-[0.18em]",
                      trajetoria?.foi_campeao
                        ? "bg-[#d29922]/20 text-[#d29922]"
                        : "bg-white/8 text-[#7d8590]",
                    ].join(" ")}
                  >
                    {trajetoria?.foi_campeao ? "Campeão" : "Sem título"}
                  </div>
                </div>
                <div className="mt-2 text-xs text-[#7d8590]">
                  {trajetoria?.foi_campeao
                    ? "Já venceu ao menos uma categoria na carreira."
                    : "Ainda busca o primeiro título da carreira."}
                </div>
              </div>
            </div>
          </div>

          <div className="glass-light rounded-xl p-4">
            <div className="mb-3 text-[10px] font-bold uppercase tracking-[0.18em] text-[#7d8590]">
              Linha do tempo
            </div>
            <div className="grid gap-4 border-l border-[#21262d] pl-4">
              {trajetoria?.marcos?.length ? (
                trajetoria.marcos.map((item, index) => (
                  <TimelineItem key={`${item.tipo}-${index}`} item={item} />
                ))
              ) : (
                <p className="text-xs text-[#7d8590]">Sem marcos visíveis por enquanto.</p>
              )}
            </div>
          </div>
        </div>
      </SectionComponent>
    </>
  );
}

export function MarketSection({ SectionComponent, detail, market }) {
  return (
    <>
      {detail.contrato_mercado ? (
        <SectionComponent title="Contrato e Mercado">
          <div className="grid gap-4">
            {detail.contrato_mercado.contrato ? (
              <div className="glass-light rounded-xl p-4">
                <div className="mb-3 flex items-center gap-2">
                  <span className="text-sm">📝</span>
                  <span className="text-sm font-medium text-[#e6edf3]">
                    {detail.contrato_mercado.contrato.equipe_nome}
                  </span>
                </div>
                <div className="grid gap-x-4 gap-y-2 text-sm sm:grid-cols-2">
                  <div>
                    <span className="text-[#7d8590]">Papel: </span>
                    <span className="font-medium text-[#e6edf3]">
                      {detail.contrato_mercado.contrato.papel}
                    </span>
                  </div>
                  <div>
                    <span className="text-[#7d8590]">Salário: </span>
                    <span className="font-medium text-[#e6edf3]">
                      {formatSalary(detail.contrato_mercado.contrato.salario_anual)}
                    </span>
                  </div>
                  <div>
                    <span className="text-[#7d8590]">Duração: </span>
                    <span className="text-[#e6edf3]">
                      Temp {detail.contrato_mercado.contrato.temporada_inicio} - Temp{" "}
                      {detail.contrato_mercado.contrato.temporada_fim}
                    </span>
                  </div>
                  <div>
                    <span className="text-[#7d8590]">Restante: </span>
                    <span className="font-medium text-[#e6edf3]">
                      {detail.contrato_mercado.contrato.anos_restantes} ano
                      {detail.contrato_mercado.contrato.anos_restantes !== 1 ? "s" : ""}
                    </span>
                  </div>
                </div>
              </div>
            ) : null}

            {market ? (
              <div className="glass-light rounded-xl p-4">
                <div className="mb-2 text-[10px] font-bold uppercase tracking-[0.18em] text-[#7d8590]">
                  Mercado
                </div>
                <div className="grid gap-2 text-sm text-[#e6edf3] sm:grid-cols-3">
                  <div>Valor: {formatSalary(market.valor_mercado)}</div>
                  <div>Faixa salarial: {formatSalary(market.salario_estimado)}</div>
                  <div>Chance de troca: {market.chance_transferencia ?? "-"}%</div>
                </div>
              </div>
            ) : (
              <div className="glass-light rounded-xl p-4 text-sm text-[#7d8590]">
                Sem sinais fortes de mercado no momento.
              </div>
            )}
          </div>
        </SectionComponent>
      ) : null}

      {detail.relacionamentos ? (
        <SectionComponent title="Relacionamentos">
          <div className="glass-light rounded-xl p-4 text-sm text-[#e6edf3]">
            Rival principal: {detail.relacionamentos.rival_principal || "-"}
          </div>
        </SectionComponent>
      ) : null}

      {detail.reputacao ? (
        <SectionComponent title="Reputação">
          <div className="glass-light rounded-xl p-4 text-sm text-[#e6edf3]">
            Popularidade: {formatStatValue(detail.reputacao.popularidade)}
          </div>
        </SectionComponent>
      ) : null}

      {detail.saude ? (
        <SectionComponent title="Saúde" className="mb-0">
          <div className="glass-light rounded-xl p-4 text-sm text-[#e6edf3]">
            Saúde geral: {formatStatValue(detail.saude.saude_geral)}
          </div>
        </SectionComponent>
      ) : null}
    </>
  );
}
