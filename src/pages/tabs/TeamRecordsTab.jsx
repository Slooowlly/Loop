import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

import i18n from "../../i18n/index.js";
import GlassCard from "../../components/ui/GlassCard";
import TeamLogoMark from "../../components/team/TeamLogoMark";
import useCareerStore from "../../stores/useCareerStore";
import { getCategoryColor } from "../../utils/categoryColors";
import { categoryLabel } from "../../utils/formatters";

// Aba de recordes de equipes.
//
// É o destino dos cards de record do dossiê: clicar em "Vitórias" na ficha de uma
// equipe abre esta lista ordenada por vitórias, no MESMO recorte em que o card
// dizia "11º de 19". Os dois números saem do mesmo agregado no backend
// (`get_team_records_ranking` reusa `aggregate_team_history`), então a posição
// aqui e o rank do card nunca divergem.
//
// Tela própria, e não um modo do Atlas. Chegaram a conviver como dois modos da
// aba Equipes e não funcionou: o Atlas é uma tela cheia, com cromo, régua de
// anos e coluna de campeonatos; isto é uma tabela em fluxo. Alternar entre as
// duas trocava a página inteira debaixo de um botão que parecia trocar uma
// visualização — e nenhuma das duas é visualização da outra. O Atlas responde
// QUANDO cada equipe passou por onde; esta responde QUANTO.
//
// Não entra no menu: ela existe como resposta a uma pergunta feita no dossiê, e
// sem essa pergunta não há métrica para ordenar nem recorte para filtrar. Por
// isso volta para a tela de onde veio em vez de para um lugar fixo.

// As colunas ordenáveis. A chave é a mesma do payload, e também a mesma do `id`
// do record no dossiê — é o que faz "clicou em pódios, chegou ordenado por
// pódios" ser uma atribuição direta, sem tabela de tradução no meio.
// `total` é o mesmo número na carreira inteira, e só as CONTAGENS o têm: uma
// taxa não se lê como fração de outra taxa.
const COLUMNS = [
  { key: "titles", label: "globalTeamRecords.col.titles", total: "total_titles" },
  { key: "wins", label: "globalTeamRecords.col.wins", total: "total_wins" },
  { key: "podiums", label: "globalTeamRecords.col.podiums", total: "total_podiums" },
  { key: "races", label: "globalTeamRecords.col.races", total: "total_races" },
  { key: "podium_rate", label: "globalTeamRecords.col.podiumRate", suffix: "%" },
  { key: "win_rate", label: "globalTeamRecords.col.winRate", suffix: "%" },
];

const SORT_KEYS = new Set(COLUMNS.map((column) => column.key));
export const DEFAULT_RECORDS_SORT = "titles";

// O filtro tem DUAS perguntas, e é a segunda que faltava.
//
// A primeira é qual categoria. A segunda é a amplitude, e são três respostas que
// não se substituem: só a categoria (a equipe contra quem estava na pista com
// ela), o grupo (escadas equivalentes juntas — o recorte dos cards do dossiê,
// para não comparar carros de mundos diferentes) ou o mundo inteiro (quem é a
// maior equipe do save, aceitando somar um título de Rookie com um de GT3).
//
// A tela já ofereceu só categorias e mentia: escolher "Mazda Rookie" trazia
// também a Mazda Championship, porque a comparação sempre foi por grupo. O
// número estava certo e o rótulo errado — a pior combinação, porque não há como
// desconfiar de um filtro que parece ter funcionado.
//
// A lista de categorias vem do backend, com o grupo de cada uma junto: é ele que
// deixa a opção "grupo" dizer o que significa AQUI ("Grupo Mazda") em vez de uma
// palavra abstrata.
const SCOPE_KINDS = ["category", "group", "world"];

// A chave de uma entrada da escada. Multiclasse é "categoria:classe"; o resto é
// só a categoria.
function entryKey(category, klass) {
  return klass ? `${category}:${klass}` : String(category ?? "");
}

export function TeamRecordsTab({
  category = null,
  teamClass = null,
  metric = null,
  highlightTeamId = null,
  onOpenTeam,
  onBack,
}) {
  const { t } = useTranslation();
  const careerId = useCareerStore((state) => state.careerId);
  // A seleção é a CHAVE da entrada, não o id da categoria: a Production vira
  // três entradas ("production_challenger:mazda" e irmãs), e um seletor por id
  // não teria como distingui-las.
  const [selectedKey, setSelectedKey] = useState(entryKey(category ?? "gt3", teamClass));
  const [selectedCategory, selectedClass] = String(selectedKey).split(":");
  // Abre em CATEGORIA, na categoria da ficha que estava aberta: a pergunta que
  // leva alguém a clicar num record é "como minha equipe se compara com as que
  // correm comigo", e quem corre com ela é a categoria, não a escada inteira.
  //
  // O preço é conhecido e aceito: o card diz "9º de 19" contando o GRUPO, então
  // a posição na primeira tela pode não ser a do card. É por isso que a nota de
  // rodapé nomeia o recorte aplicado, e que o botão do grupo fica ao lado — a
  // ponte entre os dois números está a um clique.
  const [scopeKind, setScopeKind] = useState("category");
  // A métrica clicada vira a ordenação de chegada. Sem ela, o padrão é títulos —
  // a mesma hierarquia com que o backend entrega a lista em repouso.
  const [sortKey, setSortKey] = useState(SORT_KEYS.has(metric) ? metric : DEFAULT_RECORDS_SORT);
  const [payload, setPayload] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  // Chegar de novo pelo card de outra equipe (ou de outra métrica) tem de
  // reposicionar a tela, e não deixá-la no estado da visita anterior.
  useEffect(() => {
    if (category) setSelectedKey(entryKey(category, teamClass));
    // A amplitude também reinicia: uma visita nova é uma pergunta nova, e ela
    // começa na categoria da ficha que foi aberta.
    setScopeKind("category");
  }, [category]);
  useEffect(() => {
    if (SORT_KEYS.has(metric)) setSortKey(metric);
  }, [metric]);

  // Rede de segurança para a chave não existir na escada: chegar por uma
  // multiclasse sem classe (equipe sem `classe` gravada) deixaria o seletor em
  // branco. Aí cai na primeira entrada da mesma categoria, que é uma escolha
  // arbitrária mas visível — melhor que um filtro sem rótulo.
  useEffect(() => {
    const escada = payload?.categories ?? [];
    if (!escada.length || escada.some((item) => item.key === selectedKey)) return;
    const irmao = escada.find((item) => item.id === selectedCategory);
    if (irmao) setSelectedKey(irmao.key);
  }, [payload, selectedKey, selectedCategory]);

  useEffect(() => {
    let cancelled = false;
    async function carregar() {
      if (!careerId || !selectedKey) return;
      setLoading(true);
      setError("");
      try {
        const data = await invoke("get_team_records_ranking", {
          careerId,
          category: selectedCategory,
          scope: scopeKind,
          class: selectedClass ?? null,
        });
        if (!cancelled) setPayload(data);
      } catch (err) {
        if (!cancelled) setError(String(err));
      } finally {
        if (!cancelled) setLoading(false);
      }
    }
    carregar();
    return () => {
      cancelled = true;
    };
  }, [careerId, selectedKey, scopeKind]);

  // O nome do grupo DESTA categoria, para o botão de amplitude dizer "Grupo
  // Mazda" em vez de "grupo".
  const grupoDaCategoria = (payload?.categories ?? []).find((item) => item.id === selectedCategory)?.group_label ?? "";

  const rows = useMemo(() => {
    const brutas = payload?.rows ?? [];
    // Sempre decrescente: nenhuma destas métricas fica interessante do menor para
    // o maior, e um clique que invertesse a ordem só produziria a lista de quem
    // menos venceu. O desempate por corridas separa 100% em duas provas de 100%
    // em oitenta.
    return [...brutas].sort(
      (a, b) =>
        Number(b[sortKey] ?? 0) - Number(a[sortKey] ?? 0) ||
        Number(b.races ?? 0) - Number(a.races ?? 0) ||
        String(a.team).localeCompare(String(b.team)),
    );
  }, [payload, sortKey]);

  return (
    <div className="space-y-5" data-testid="team-records-table">
      {/* Cabeçalho de tela, no mesmo formato do resto do app: sobrescrito
          pequeno, título grande e os controles à direita. É o que faz a tabela
          se anunciar como um lugar em vez de um painel solto no meio do nada. */}
      <header className="flex flex-wrap items-end justify-between gap-4 px-1">
        <div className="min-w-0">
          <p className="text-[11px] uppercase tracking-[0.24em] text-accent-primary">
            {t("globalTeamRecords.title")}
          </p>
          <h2 className="mt-1 truncate text-[26px] font-bold leading-tight text-text-primary">
            {payload?.scope
              ? t("globalTeamRecords.heading", { scope: payload.scope, count: rows.length })
              : t("globalTeamRecords.subtitle")}
          </h2>
        </div>
        <div className="flex flex-wrap items-center gap-3">
          <label className="flex items-center gap-2 text-[11px] text-text-secondary">
            {t("globalTeamRecords.categoryFilter")}
            <select
              value={selectedKey}
              onChange={(event) => setSelectedKey(event.target.value)}
              disabled={scopeKind === "world"}
              data-testid="team-records-category"
              className="rounded-lg border border-white/10 bg-[#0f1c2b] px-2.5 py-1.5 text-xs text-text-primary disabled:opacity-40"
            >
              {(payload?.categories?.length
                ? payload.categories
                : [{ key: selectedKey, label: categoryLabel(selectedCategory) }]
              ).map((item) => (
                <option key={item.key} value={item.key}>
                  {item.label}
                </option>
              ))}
            </select>
          </label>

          {/* A amplitude é o segundo filtro, e não um detalhe do primeiro: é ela
              que decide contra QUEM os números são medidos. A opção de grupo diz
              o nome do grupo desta categoria em vez da palavra "grupo" — sem
              isso, escolher entre "categoria" e "grupo" é escolher no escuro. */}
          <div className="flex overflow-hidden rounded-lg border border-white/10" data-testid="team-records-scope">
            {SCOPE_KINDS.map((id) => (
              <button
                key={id}
                type="button"
                aria-pressed={scopeKind === id}
                data-scope={id}
                data-active={scopeKind === id ? "true" : undefined}
                onClick={() => setScopeKind(id)}
                className={`px-2.5 py-1.5 text-[10px] font-semibold uppercase tracking-[0.1em] transition-glass ${
                  scopeKind === id ? "bg-white/[0.09] text-text-primary" : "text-text-muted hover:text-text-secondary"
                }`}
              >
                {id === "group" && grupoDaCategoria
                  ? grupoDaCategoria
                  : t(`globalTeamRecords.scopeKind.${id}`)}
              </button>
            ))}
          </div>
          {onBack ? (
            <button
              type="button"
              onClick={onBack}
              data-testid="team-records-back"
              className="rounded-lg border border-white/10 px-3 py-1.5 text-[11px] text-text-secondary transition-glass hover:border-white/20 hover:text-text-primary"
            >
              {t("globalTeamRecords.back")}
            </button>
          ) : null}
        </div>
      </header>

      <GlassCard className="overflow-hidden p-0">
        {error ? (
          <p className="p-4 text-xs text-status-red">{error}</p>
        ) : loading ? (
          <p className="p-4 text-xs text-text-secondary">{t("globalTeamRecords.loading")}</p>
        ) : !rows.length ? (
          <p className="p-4 text-xs text-text-secondary">{t("globalTeamRecords.empty")}</p>
        ) : (
          <div className="max-h-[calc(100vh-300px)] overflow-auto">
            <table className="min-w-full text-left text-sm" aria-label={i18n.t("globalTeamRecords.tableAria")}>
              <thead className="sticky top-0 z-10 bg-[#0b1524] text-[10px] uppercase tracking-[0.14em] text-text-muted">
                <tr>
                  <th className="px-4 py-3">#</th>
                  <th className="px-4 py-3">{t("globalTeamRecords.col.team")}</th>
                  {/* O período não ordena: ele existe para dar escala à contagem
                      ao lado, não para ser o critério da lista. */}
                  <th className="px-4 py-3">{t("globalTeamRecords.col.span")}</th>
                  {COLUMNS.map((column) => (
                    <RecordsHeader
                      key={column.key}
                      label={t(column.label)}
                      sortKey={column.key}
                      active={sortKey === column.key}
                      onSort={setSortKey}
                    />
                  ))}
                </tr>
              </thead>
              <tbody>
                {rows.map((row, index) => (
                  <RecordsRow
                    key={row.team_id}
                    row={row}
                    position={index + 1}
                    sortKey={sortKey}
                    highlighted={row.team_id === highlightTeamId}
                    onOpenTeam={onOpenTeam}
                  />
                ))}
              </tbody>
            </table>
          </div>
        )}
      </GlassCard>

      {/* A mesma explicação que o rodapé do dossiê dá, pelo mesmo motivo: o
          recorte junta escadas equivalentes, e quem não sabe disso lê os títulos
          da Mazda Championship como se fossem da Mazda Rookie. */}
      {payload?.scope ? (
        <p className="px-1 text-[11px] text-text-muted">
          {t(`globalTeamRecords.scopeNote.${payload.scope_kind ?? "group"}`, { scope: payload.scope })}
          {/* Grupo não tem tamanho fixo: o Grupo Mazda são duas categorias e o
              Grupo Production são seis, porque a Production é onde as escadas de
              entrada convergem. Nomear quem entrou é o que separa "o recorte é
              maior" de "sobrou filtro" — que é exatamente como a assimetria se
              parece sem a lista à vista. */}
          {payload.scope_kind === "group" && payload.scope_categories?.length > 1 ? (
            <span data-testid="team-records-scope-members">
              {` ${t("globalTeamRecords.scopeMembers", {
                list: payload.scope_categories.join(", "),
                count: payload.scope_categories.length,
              })}`}
              {/* A Production aparece na lista acima, e sem esta frase parece que
                  Toyota e BMW entraram junto: elas correm a mesma categoria, em
                  campeonatos separados. */}
              {payload.scope_family
                ? ` ${t("globalTeamRecords.scopeFamily", {
                    family: t(`globalTeamRecords.family.${payload.scope_family}`),
                  })}`
                : ""}
            </span>
          ) : null}
          {/* A legenda do segundo número só existe fora do mundo, onde ele
              existe. No mundo o recorte já é a carreira toda e não há par. */}
          {payload.scope_kind !== "world" ? ` ${t("globalTeamRecords.totalHint")}` : ""}
        </p>
      ) : null}
    </div>
  );
}

// Cabeçalho de coluna. Sem seta de direção porque não há direção a escolher — a
// ordem é sempre do maior para o menor; o marcador diz qual coluna manda.
function RecordsHeader({ label, sortKey, active, onSort }) {
  return (
    <th className="px-4 py-3">
      <button
        type="button"
        onClick={() => onSort(sortKey)}
        data-sort={sortKey}
        data-active={active ? "true" : undefined}
        className={`inline-flex items-center gap-1 rounded-lg text-left transition-glass hover:text-text-primary ${
          active ? "text-accent-primary" : ""
        }`}
      >
        <span>{label}</span>
        <span className={active ? "text-accent-primary" : "text-text-muted"}>{active ? "↓" : "↕"}</span>
      </button>
    </th>
  );
}

function RecordsRow({ row, position, sortKey, highlighted, onOpenTeam }) {
  const { t } = useTranslation();
  const cor = row.color || "#58a6ff";
  const periodo =
    row.first_year && row.last_year && row.first_year !== row.last_year
      ? `${row.first_year}–${row.last_year}`
      : row.first_year || row.last_year || "";
  return (
    <tr
      data-team={row.team_id}
      data-highlighted={highlighted ? "true" : undefined}
      onClick={onOpenTeam ? () => onOpenTeam(row) : undefined}
      className={`border-t border-white/[0.06] transition-glass ${onOpenTeam ? "cursor-pointer" : ""} ${
        highlighted ? "bg-accent-primary/[0.09]" : "hover:bg-white/[0.03]"
      }`}
    >
      <td className="px-4 py-2 font-mono text-xs text-text-muted">{position}</td>
      <td className="px-4 py-2">
        <div className="flex items-center gap-2.5">
          <TeamLogoMark teamName={row.team} color={cor} size="xs" testId="team-records-logo" />
          <div className="min-w-0">
            <span className="block truncate text-xs font-semibold text-text-primary">{row.team}</span>
            <span className="flex items-center gap-1.5 text-[10px] text-text-muted">
              <span
                className="h-1.5 w-1.5 shrink-0 rounded-full"
                style={{ backgroundColor: getCategoryColor(row.category_id) }}
              />
              {/* "Hoje na X", e não só "X": os números da linha são do RECORTE
                  inteiro, e a categoria aqui é onde a equipe corre AGORA — que
                  pode ser outra, porque ela subiu. Sem o "hoje", um time hoje na
                  Production com 2 títulos de Mazda Championship lia como "2
                  títulos da Production".

                  Equipe fechada continua na lista: o histórico dela é parte da
                  medida do grupo, e sumir com ela mudaria os denominadores que o
                  card do dossiê mostra. O rótulo é que avisa. */}
              {row.active
                ? t("globalTeamRecords.currentlyIn", { category: row.category })
                : t("globalTeamRecords.inactive")}
            </span>
          </div>
        </div>
      </td>
      {/* "2024–2025" ao lado de "5 corridas" é o que impede a linha de ser lida
          como uma carreira inteira. Ano único não vira intervalo — "2026–2026"
          seria ruído com cara de dado. */}
      <td className="px-4 py-2 font-mono text-[11px] text-text-muted" data-span>
        {periodo || "—"}
      </td>
      {COLUMNS.map((column) => {
        const valor = Number(row[column.key] ?? 0);
        const total = column.total ? Number(row[column.total] ?? 0) : valor;
        return (
          <td
            key={column.key}
            data-metric={column.key}
            className={`px-4 py-2 font-mono text-xs ${
              sortKey === column.key ? "font-semibold text-text-primary" : "text-text-secondary"
            }`}
          >
            {`${valor}${column.suffix ?? ""}`}
            {/* O total só aparece quando difere: igual, ele seria um "/87" ao
                lado de "87" em toda a tabela — ruído que ensina a ignorar
                justamente o número que existe para chamar atenção. */}
            {total > valor ? (
              <span
                className="ml-1 text-text-muted"
                title={t("globalTeamRecords.totalTitle", { value: total })}
                data-total={total}
              >
                {`/${total}`}
              </span>
            ) : null}
          </td>
        );
      })}
    </tr>
  );
}

export default TeamRecordsTab;
