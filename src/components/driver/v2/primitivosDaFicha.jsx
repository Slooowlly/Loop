import { useTranslation } from "react-i18next";
import { Award, Crown, Flag, Medal } from "lucide-react";

import Tooltip from "../../ui/Tooltip";
import { ordinal } from "../../../i18n/format.js";

// Os tijolos da ficha do piloto: rótulo de bloco, casca de seção, chip do
// cabeçalho, linha de tabela, marcas de rank, barra de motivação, legenda de
// medalha e o ícone de métrica.
//
// Moram aqui, e não em `../../ui/`, porque nenhum deles é genérico: cada um
// carrega uma decisão de composição DESTA ficha (o rótulo centralizado, a linha
// que não pode ganhar altura, o rank sem denominador). Fora daqui só são usados
// pelas seções irmãs — a ficha é quem os compõe.

const METRIC_ICONS = {
  corridas: Flag,
  vitorias: Crown,
  podios: Medal,
  titulos: Award,
};

// Rótulo de bloco. Caixa de frase, sem tracking largo. NÃO reintroduza caixa
// alta em rótulo pequeno aqui — a combinação de versalete, tracking e cinza
// apagado num corpo de 10px obriga a soletrar, e a hierarquia não depende dela.
//
// Centralizado, e centralizado AQUI: a ficha é uma pilha de cartões de largura
// cheia, e um rótulo encostado no canto esquerdo de cada um deles puxava o olho
// para fora do conteúdo a cada bloco. Quem paga por isso são as linhas que
// pareiam o rótulo com um contador ou um botão — elas precisam do
// `justify-center` (ou tirar o controle do fluxo) para o eixo bater.
export function BlockLabel({ children }) {
  return (
    <span className="block text-center text-xs font-semibold text-text-secondary">{children}</span>
  );
}

// Casca de seção para os blocos importados de ../detalhes/ que esperam um
// `SectionComponent` com título próprio.
export function Block({ title, children }) {
  return (
    <section className="mb-4 last:mb-0">
      <BlockLabel>{title}</BlockLabel>
      <div className="mt-2">{children}</div>
    </section>
  );
}

// Chip do cabeçalho: licença, status, "sem equipe". Borda fina e fundo quase
// preto para ficarem legíveis sem virar botões — nenhum deles é clicável.
export function HeroBadge({ children }) {
  return (
    <span className="rounded-full border border-white/15 bg-[#08111f] px-2.5 py-1 text-xs text-text-secondary">
      {children}
    </span>
  );
}

// `hint` é a referência do mundo, colada no valor em corpo menor: "1,5%" não
// responde se ele é confiável, "1,5% média 4,2%" responde. Fica ao lado e não
// embaixo para não empurrar a altura da linha — os cards do grid precisam
// terminar alinhados.
export function DataRow({
  label,
  value,
  hint = null,
  recorde = null,
  valueClassName = "text-text-primary",
}) {
  return (
    <div className="flex items-baseline justify-between gap-3 border-b border-white/[0.06] py-2 last:border-b-0 last:pb-0">
      <span className="min-w-0 truncate text-xs text-text-secondary">{label}</span>
      <span className="flex shrink-0 items-baseline gap-1.5">
        {hint ? <span className="text-[10px] text-text-muted">{hint}</span> : null}
        {recorde ? <RankMarks recorde={recorde} /> : null}
        <span className={`text-right text-[13px] font-medium ${valueClassName}`}>{value}</span>
      </span>
    </div>
  );
}

// A posição do piloto naquele número, nas duas populações.
//
// Só os dois ordinais, sem denominador: eles são os mesmos em todas as linhas, e
// repetir "de 12 no grid · de 503 no mundo" vinte vezes quebrava cada linha em
// duas e dobrava a altura dos nove cards. Os totais vão UMA vez, na legenda do
// botão; o número exato de cada linha (que varia — grid médio só conta quem tem
// largada registrada) fica no `title`.
//
// Entra ANTES do valor, e não depois, para a coluna dos números continuar
// alinhada à direita — é ela que dá o ritmo do card.
export function RankMarks({ recorde }) {
  const { t } = useTranslation();
  const grid =
    Number.isFinite(recorde.grid) && recorde.grid_total > 1
      ? { chave: "grid", rank: recorde.grid, total: recorde.grid_total }
      : null;
  const mundo =
    Number.isFinite(recorde.mundo) && recorde.mundo_total > 1
      ? { chave: "mundo", rank: recorde.mundo, total: recorde.mundo_total }
      : null;
  if (!grid && !mundo) return null;

  return (
    <span className="flex shrink-0 items-baseline gap-1 text-[10px]" data-testid="dossier-rank">
      {grid ? (
        <Tooltip
          texto={t("driverDetail.history.rankGrid", {
            rank: ordinal(grid.rank),
            total: grid.total,
          })}
        >
          <span className="font-medium text-[color:var(--team)]">{ordinal(grid.rank)}</span>
        </Tooltip>
      ) : null}
      {grid && mundo ? <span className="text-text-muted">·</span> : null}
      {mundo ? (
        <Tooltip
          texto={t("driverDetail.history.rankWorld", {
            rank: ordinal(mundo.rank),
            total: mundo.total,
          })}
        >
          <span className="text-text-muted">{ordinal(mundo.rank)}</span>
        </Tooltip>
      ) : null}
    </span>
  );
}

// Motivação deitada: rótulo, trilho curto e o número, tudo na mesma linha.
//
// Em pé — rótulo e número numa linha, barra embaixo — ela precisava de uma
// coluna própria no cabeçalho e reservava uma faixa de 248px que nada mais
// usava. Deitada, a barra vira o que sempre foi: um adjetivo do número ao lado,
// não um gráfico. 56px bastam para diferenciar 30% de 100% em leitura
// periférica, que é tudo que se pede dela aqui.
export function MotivationBar({ value }) {
  const { t } = useTranslation();
  const normalized = Number.isFinite(value) ? value : 0;
  const color = normalized >= 70 ? "#3fb950" : normalized >= 40 ? "#d29922" : "#f85149";
  return (
    <div className="flex items-center gap-2" data-testid="driver-detail-motivation">
      <span className="whitespace-nowrap text-xs text-text-secondary">
        {t("driverDetail.motivation.label")}
      </span>
      <div className="h-1 w-14 shrink-0 overflow-hidden rounded-full bg-white/10">
        <div
          className="h-full rounded-full transition-all duration-700"
          style={{ width: `${normalized}%`, backgroundColor: color }}
        />
      </div>
      <span className="font-mono text-xs tabular-nums" style={{ color }}>
        {normalized}%
      </span>
    </div>
  );
}

export function MedalKey({ color, label }) {
  return (
    <span className="flex items-center gap-1.5">
      <span className="h-2 w-2 rounded-sm" style={{ backgroundColor: color }} />
      {label}
    </span>
  );
}

export function MetricIcon({ name, size = 15 }) {
  const Icon = METRIC_ICONS[name];
  if (!Icon) return null;
  return <Icon size={size} strokeWidth={1.5} aria-hidden="true" className="shrink-0" />;
}
