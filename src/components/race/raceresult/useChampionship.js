import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

// Carrega a classificação do campeonato da categoria do jogador e, de quebra,
// o mapa nome da equipe → cor usado pelos logos.
export function useChampionship(careerId, categoria) {
  const [championship, setChampionship] = useState([]);
  const [teamColors, setTeamColors] = useState({});
  const [loadingChampionship, setLoadingChampionship] = useState(false);
  const [championshipError, setChampionshipError] = useState("");

  useEffect(() => {
    let mounted = true;
    async function fetchChampionship() {
      if (!careerId || !categoria) return;
      setLoadingChampionship(true);
      setChampionshipError("");
      try {
        const data = await invoke("get_drivers_by_category", {
          careerId,
          category: categoria,
        });
        if (mounted) {
          setChampionship(data);

          const colors = {};
          data.forEach(d => {
            if (d.equipe_nome && d.equipe_cor) {
              colors[d.equipe_nome] = d.equipe_cor;
            }
          });
          setTeamColors(colors);
        }
      } catch (error) {
        if (mounted) {
          setChampionshipError(
            typeof error === "string" ? error : "Não foi possível carregar o campeonato."
          );
        }
      } finally {
        if (mounted) setLoadingChampionship(false);
      }
    }
    fetchChampionship();
    return () => { mounted = false; };
  }, [careerId, categoria]);

  return { championship, teamColors, loadingChampionship, championshipError };
}
