import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import useCareerStore from "../../stores/useCareerStore";

// Os dois dados que o banner do cabeçalho busca por conta própria.
//
// 1. O CAMPEÃO da temporada da categoria do jogador, quando não há mais etapa pendente —
//    é o que o banner de "temporada encerrada" mostra no lugar da próxima corrida.
// 2. A PRÓXIMA CORRIDA de outra categoria, quando o jogador abre uma série diferente da
//    dele na tabela da Home. As outras categorias correm em lockstep com ele, então quase
//    sempre há uma etapa pendente na mesma janela; sem pendente (fim de temporada),
//    mostramos a última etapa sem contagem regressiva.
//
// Vivem aqui, e não no `Header`, porque são leituras de ponte — o padrão do projeto põe
// `invoke` nos slices ou em hooks `use*`, e o cabeçalho já carregava orquestração demais.

/// Dias entre duas datas YYYY-MM-DD (usa UTC para não sofrer com fuso). Para a categoria
/// do jogador a contagem já vem pronta do store; só a de OUTRA categoria é derivada aqui.
function diasEntreDatas(de, ate) {
  const ler = (valor) => {
    const m = /^(\d{4})-(\d{2})-(\d{2})/.exec(valor ?? "");
    return m ? Date.UTC(Number(m[1]), Number(m[2]) - 1, Number(m[3])) : null;
  };
  const a = ler(de);
  const b = ler(ate);
  if (a == null || b == null) return null;
  return Math.round((b - a) / 86400000);
}

export default function useBannerDaEtapa({
  activeTab,
  showRaceBriefing,
  viewingOwnCategory,
  viewedCategory,
  visibleDate,
  hasNoPendingRace,
  playerCategory,
}) {
  const careerId = useCareerStore((state) => state.careerId);
  const season = useCareerStore((state) => state.season);

  const [seasonChampion, setSeasonChampion] = useState(null);
  // { race, totalRodadas, countdownDays, pending } quando é outra categoria; `null` na do
  // jogador (aí o banner usa `nextRace` do store).
  const [categoryRace, setCategoryRace] = useState(null);

  useEffect(() => {
    let mounted = true;

    async function carregarCampeao() {
      if (!careerId || !playerCategory || !hasNoPendingRace) {
        if (mounted) setSeasonChampion(null);
        return;
      }

      try {
        const standings = await invoke("get_drivers_by_category", {
          careerId,
          category: playerCategory,
        });

        if (!mounted) return;

        setSeasonChampion(
          Array.isArray(standings)
            ? standings.find((driver) => driver?.posicao_campeonato === 1) ?? standings[0] ?? null
            : null,
        );
      } catch (error) {
        console.error("Erro ao carregar campeão da temporada para o header:", error);
        if (mounted) setSeasonChampion(null);
      }
    }

    carregarCampeao();

    return () => {
      mounted = false;
    };
  }, [careerId, playerCategory, hasNoPendingRace, season?.ano]);

  useEffect(() => {
    let mounted = true;

    if (
      !careerId ||
      viewingOwnCategory ||
      !viewedCategory ||
      activeTab !== "standings" ||
      showRaceBriefing
    ) {
      setCategoryRace(null);
      return undefined;
    }

    // `get_calendar_for_category` já devolve tudo que o banner precisa (pista, clima,
    // temperatura, data, rodada).
    invoke("get_calendar_for_category", { careerId, category: viewedCategory })
      .then((entries) => {
        if (!mounted) return;
        const list = Array.isArray(entries) ? entries : [];
        const pending = list.find((entry) => entry.status === "Pendente") ?? null;
        const race = pending ?? list[list.length - 1] ?? null;
        if (!race) {
          setCategoryRace(null);
          return;
        }
        setCategoryRace({
          race,
          totalRodadas: list.length,
          countdownDays: pending ? diasEntreDatas(visibleDate, pending.display_date) : null,
          pending: Boolean(pending),
        });
      })
      .catch(() => {
        if (mounted) setCategoryRace(null);
      });

    return () => {
      mounted = false;
    };
    // `visibleDate` + `rodada_atual` mudam a cada avanço → recalcula ao reabrir a Home.
  }, [
    careerId,
    viewedCategory,
    viewingOwnCategory,
    activeTab,
    showRaceBriefing,
    visibleDate,
    season?.ano,
    season?.rodada_atual,
  ]);

  return { seasonChampion, categoryRace };
}
