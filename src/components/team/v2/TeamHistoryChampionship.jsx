import { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  EVOLUTION_VIEW_RUN,
  EVOLUTION_VIEW_SEASONS,
  RUN_MODE_POINTS,
  RUN_MODE_POSITION,
  guardarModoEvolucao,
  guardarVistaEvolucao,
  lerModoEvolucao,
  lerVistaEvolucao,
} from "./evolutionPreferences.js";
import { campanhaTemDados, curvaTemDados } from "./teamHistoryV2Logic";
import { ChampionshipRun } from "./TeamHistoryChampionshipRun.jsx";
import { ChampionshipCurve } from "./TeamHistoryChampionshipCurve.jsx";

// O seletor de vistas do campeonato — o único ponto que o drawer importa desta
// área. Os dois gráficos moram em [TeamHistoryChampionshipRun.jsx] e
// [TeamHistoryChampionshipCurve.jsx], separados em 11/08/2026.

// As duas vistas do mesmo assunto — onde a equipe TERMINOU cada campeonato, e
// COMO o campeonato de agora está sendo disputado.
//
// O panorama entre temporadas é o padrão porque é a pergunta que o dossiê de uma
// equipe responde primeiro: quem é essa equipe ao longo dos anos. A campanha é o
// zoom no ano corrente, e zoom vem depois do panorama — a mesma ordem que a fita
// de forma recente segue logo abaixo.
//
// O seletor entra no cabeçalho do gráfico, à esquerda, colado no rótulo: é ali
// que ele fica no MESMO lugar nas duas vistas. À direita cada vista tem o que é
// dela (a pílula do pódio, o modo do eixo), e o seletor pularia de posição a cada
// troca.
export function ChampionshipEvolution({ run, seasons, rodadaAcesa = null, onAcenderRodada = null }) {
  const { t } = useTranslation();
  // As duas escolhas sobrevivem ao desmonte do bloco — ver evolutionPreferences.js.
  // Comparar equipes é o uso principal do gráfico, e o caminho até a próxima
  // equipe passa por trocar de aba ou fechar o dossiê: sem persistir, o gráfico
  // voltava para "entre campeonatos" bem no meio da comparação.
  const [vista, setVistaState] = useState(lerVistaEvolucao);
  // A métrica é escolhida UMA vez e vale nas duas escalas de tempo. Ela morava
  // dentro da campanha, então trocar de vista fazia aparecer um segundo seletor
  // e uma pílula do nada — e o toggle parecia levar a dois blocos diferentes em
  // vez de a duas vistas do mesmo. São dois eixos de escolha independentes:
  // QUANDO (entre campeonatos · campeonato atual) e O QUÊ (colocação · pontos).
  const [modo, setModoState] = useState(lerModoEvolucao);
  // Só o CLIQUE grava. A vista efetiva pode divergir da escolhida quando a
  // equipe da vez não tem campanha (abaixo), e essa queda é circunstância da
  // equipe — não deve reescrever o que o jogador pediu.
  const setVista = (id) => {
    guardarVistaEvolucao(id);
    setVistaState(id);
  };
  const setModo = (id) => {
    guardarModoEvolucao(id);
    setModoState(id);
  };
  const temCampanha = campanhaTemDados(run);
  const temTemporadas = curvaTemDados(seasons);
  if (!temCampanha && !temTemporadas) return null;

  // A vista escolhida pode ficar sem dado sem que ninguém clique em nada: as
  // setas do dossiê trocam de equipe sem desmontar a tela, e a próxima pode não
  // ter campanha. Derivar em vez de guardar em efeito evita o quadro em branco
  // de um frame.
  const efetiva = temCampanha && temTemporadas ? vista : temCampanha ? EVOLUTION_VIEW_RUN : EVOLUTION_VIEW_SEASONS;

  // Com uma vista só o seletor não aparece: um segmentado de um botão é ruído
  // que promete uma escolha inexistente.
  const seletor =
    temCampanha && temTemporadas ? (
      <div className="flex overflow-hidden rounded-lg border border-white/10" data-testid="team-history-evolution-view">
        {[EVOLUTION_VIEW_SEASONS, EVOLUTION_VIEW_RUN].map((id) => (
          <button
            key={id}
            type="button"
            onClick={() => setVista(id)}
            data-view={id}
            data-active={efetiva === id ? "true" : undefined}
            className={`px-2 py-1 text-[10px] font-semibold transition-glass ${
              efetiva === id ? "bg-white/[0.09] text-text-primary" : "text-text-muted hover:text-text-secondary"
            }`}
          >
            {t(`myTeamTab.history.sport.evolutionView.${id}`)}
          </button>
        ))}
      </div>
    ) : null;

  // O seletor de métrica é o MESMO objeto nas duas vistas — mesma posição, mesmo
  // desenho, mesma escolha preservada ao trocar de escala. É o que faz as duas
  // vistas lerem como um sistema, e não como dois blocos que se substituem.
  const seletorModo = (
    <div className="flex overflow-hidden rounded-lg border border-white/10" data-testid="team-history-run-mode">
      {[RUN_MODE_POSITION, RUN_MODE_POINTS].map((id) => (
        <button
          key={id}
          type="button"
          onClick={() => setModo(id)}
          data-mode={id}
          data-active={modo === id ? "true" : undefined}
          className={`px-2 py-1 text-[10px] font-semibold transition-colors duration-150 ${
            modo === id ? "bg-white/[0.09] text-text-primary" : "text-text-muted hover:text-text-secondary"
          }`}
        >
          {t(`myTeamTab.history.sport.runMode.${id}`)}
        </button>
      ))}
    </div>
  );

  return efetiva === EVOLUTION_VIEW_RUN ? (
    <ChampionshipRun
      run={run}
      seletor={seletor}
      seletorModo={seletorModo}
      modo={modo}
      rodadaAcesa={rodadaAcesa}
      onAcenderRodada={onAcenderRodada}
    />
  ) : (
    <ChampionshipCurve seasons={seasons} seletor={seletor} seletorModo={seletorModo} modo={modo} />
  );
}
