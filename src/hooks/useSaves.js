import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

// A lista de carreiras salvas e a exclusão de uma delas.
//
// Mora num hook porque duas telas leem a mesma coisa (o menu principal e a decisão de
// destino do botão de Configurações), e porque a EXCLUSÃO é a ação mais destrutiva da
// interface: quando ela vivia inline no `MainMenu`, o `catch` era vazio e o fluxo seguia
// igual — o diálogo fechava, a lista recarregava e a carreira que não foi apagada
// reaparecia como se nada tivesse acontecido. Aqui a falha vira estado, e quem desenha
// decide o que mostrar.

/// Mais recente primeiro. `last_played` ausente vai para o fim em vez de virar `Invalid Date`.
function porUltimaPartida(lista) {
  return [...lista].sort(
    (a, b) => new Date(b.last_played || 0) - new Date(a.last_played || 0),
  );
}

export default function useSaves({ carregarAoMontar = true } = {}) {
  const [saves, setSaves] = useState([]);
  const [carregando, setCarregando] = useState(carregarAoMontar);
  const [erro, setErro] = useState("");

  /// Rebusca a lista e a devolve — quem chama pode decidir na hora, sem esperar o render.
  const recarregar = useCallback(async () => {
    setCarregando(true);
    try {
      const lista = await invoke("list_saves");
      const ordenada = porUltimaPartida(Array.isArray(lista) ? lista : []);
      setSaves(ordenada);
      setErro("");
      return ordenada;
    } catch (falha) {
      setErro(String(falha));
      // Devolve o que já estava na tela: uma falha de leitura não é motivo para o menu
      // dizer que o jogador não tem nenhuma carreira.
      return null;
    } finally {
      setCarregando(false);
    }
  }, []);

  /// Exclui e recarrega. Devolve `true` só quando o backend confirmou o apagamento.
  const excluir = useCallback(
    async (careerId) => {
      if (!careerId) return false;
      try {
        await invoke("delete_career", { careerId });
      } catch (falha) {
        setErro(String(falha));
        return false;
      }
      setErro("");
      await recarregar();
      return true;
    },
    [recarregar],
  );

  const limparErro = useCallback(() => setErro(""), []);

  useEffect(() => {
    if (carregarAoMontar) void recarregar();
  }, [carregarAoMontar, recarregar]);

  return { saves, carregando, erro, recarregar, excluir, limparErro };
}
