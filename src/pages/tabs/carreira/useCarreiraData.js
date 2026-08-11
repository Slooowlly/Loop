import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import i18n from "../../../i18n/index.js";

// Dados da aba Carreira, em um lugar só.
//
// As cinco seções (piloto, história, troféus, rivais, mercado) leem quase tudo do
// MESMO payload: `get_driver_detail` do piloto do jogador já carrega trajetória,
// títulos, marcos, rivais, contrato e valor de mercado. Buscar por seção faria a
// troca de pílula custar um `invoke` e — pior — deixaria duas seções discordando
// sobre o mesmo contrato quando uma delas tivesse dado velho.
//
// O que NÃO cabe nele vem à parte e é tolerante a falha, porque nenhuma das duas
// coisas é a razão de a tela existir:
//
//   • `get_season_market_board` — os assentos abertos do mundo. Só a seção Mercado
//     usa, e ele varre todas as equipes; falhar aqui esconde a lista de vagas e
//     deixa o resto da seção (contrato, valor, quem está de olho) em pé.
//   • `get_inbox_messages` — de lá sai só `team_interest`, as equipes que cobiçam o
//     nome do jogador pela fama. É o mesmo fato que a caixa de entrada mostra na
//     Home; aqui ele vira a linha "quem está de olho em você".
export function useCarreiraData(careerId, playerId) {
  const [detail, setDetail] = useState(null);
  const [board, setBoard] = useState(null);
  const [teamInterest, setTeamInterest] = useState(null);
  const [status, setStatus] = useState("loading");
  const [error, setError] = useState("");
  // Contador de recarga: o botão "tentar de novo" da tela de erro incrementa, e o
  // efeito abaixo depende dele. Um booleano não serviria — a segunda tentativa
  // depois de uma falha não mudaria o valor e o efeito não rodaria.
  const [reloadToken, setReloadToken] = useState(0);

  const reload = useCallback(() => setReloadToken((token) => token + 1), []);

  useEffect(() => {
    let active = true;

    if (!careerId || !playerId) {
      setStatus("error");
      setError(i18n.t("carreiraTab.errors.noPlayer"));
      return undefined;
    }

    setStatus("loading");
    setError("");

    invoke("get_driver_detail", { careerId, driverId: playerId })
      .then((payload) => {
        if (!active) return;
        setDetail(payload);
        setStatus("ready");
      })
      .catch((invokeError) => {
        if (!active) return;
        setError(
          typeof invokeError === "string"
            ? invokeError
            : (invokeError?.toString?.() ?? i18n.t("carreiraTab.errors.load")),
        );
        setStatus("error");
      });

    invoke("get_season_market_board", { careerId })
      .then((payload) => {
        if (active) setBoard(payload ?? null);
      })
      .catch(() => {
        if (active) setBoard(null);
      });

    invoke("get_inbox_messages", { careerId })
      .then((payload) => {
        if (active) setTeamInterest(payload?.team_interest ?? null);
      })
      .catch(() => {
        if (active) setTeamInterest(null);
      });

    return () => {
      active = false;
    };
  }, [careerId, playerId, reloadToken]);

  return { detail, board, teamInterest, status, error, reload };
}

export default useCarreiraData;
