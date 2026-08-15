import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { Eye } from "lucide-react";

import { bestEffort } from "../../../utils/bestEffort";
import { formatSalaryAnnual } from "../../../utils/formatters";
import { getVividTeamColor } from "../../../utils/teamColors";
import { formatCategoryLabel, formatContractRole } from "../detalhes/formatadores.js";

// Os dois blocos da aba Mercado que só existem para o JOGADOR.
//
// Eles vêm da seção Mercado da antiga aba Carreira (`pages/tabs/carreira/`,
// apagada em 14/08/2026). Das quatro coisas que aquela seção mostrava, contrato e
// valor de mercado já eram o `MarketSection` desta ficha, palavra por palavra. As
// duas que sobraram são estas, e nenhuma delas cabia lá porque nenhuma sai do
// `get_driver_detail`: são as únicas respostas da aba que dependem do MUNDO, e não
// do piloto.
//
// A pergunta que elas respondem é a do meio da temporada, numa terça-feira
// qualquer: quem está de olho em mim, e que cadeira abriu por aí. Fora da janela de
// pré-temporada o mercado só chega ao jogador em eventos pontuais — assédio, oferta
// especial, interesse — que exigem resposta na hora e somem.
//
// Só para o jogador, e a razão não é de escopo, é de significado: "elegível" é
// elegível PARA ELE (a licença dele, a faixa de tier dele), e o interesse ativo é o
// que cobiça o nome dele. Numa ficha de piloto de IA os dois blocos diriam a
// verdade sobre outra pessoa.
export function MercadoDoJogador({ careerId }) {
  const { t } = useTranslation();
  const { board, teamInterest } = useMercadoDoJogador(careerId);

  return (
    <>
      <QuemEstaDeOlho teamInterest={teamInterest} t={t} />
      <VagasAbertas board={board} t={t} />
    </>
  );
}

// As duas buscas, tolerantes a falha e independentes uma da outra.
//
// Nenhuma das duas é a razão de a aba existir — o termômetro e a situação
// contratual são, e eles já estão desenhados quando isto chega. Por isso a falha
// não derruba a aba nem pinta erro: o bloco fica no estado vazio, que é o mesmo que
// o jogador vê quando de fato não há ninguém de olho nem vaga aberta.
//
// `bestEffort` e não `.catch(() => {})`: o estado vazio e a falha silenciosa se
// parecem na tela, e sem a linha no `loop.log` não há como distinguir os dois
// depois. `get_season_market_board` varre todas as equipes do mundo, que é o
// caminho mais caro dos dois e o candidato natural a falhar num save maduro.
function useMercadoDoJogador(careerId) {
  const [board, setBoard] = useState(null);
  const [teamInterest, setTeamInterest] = useState(null);

  useEffect(() => {
    let ativo = true;
    if (!careerId) return undefined;

    bestEffort(invoke("get_season_market_board", { careerId }), "get_season_market_board").then(
      (payload) => {
        if (ativo) setBoard(payload ?? null);
      },
    );

    bestEffort(invoke("get_inbox_messages", { careerId }), "get_inbox_messages/team_interest").then(
      (payload) => {
        if (ativo) setTeamInterest(payload?.team_interest ?? null);
      },
    );

    return () => {
      ativo = false;
    };
  }, [careerId]);

  return { board, teamInterest };
}

// Quem está de olho: o interesse ATIVO pela fama, o mesmo fato que a caixa de
// entrada da Home mostra. Lá ele passa como mensagem; aqui ele fica como estado.
function QuemEstaDeOlho({ teamInterest, t }) {
  const equipes = Array.isArray(teamInterest?.teams) ? teamInterest.teams : [];

  return (
    <Cartao titulo={t("driverDetail.market.player.watching")} testId="driver-detail-watching">
      {equipes.length ? (
        <>
          <p className="text-[11px] leading-relaxed text-text-secondary">
            {t("driverDetail.market.player.watchingIntro", { count: equipes.length })}
          </p>
          <ul className="mt-2.5 grid gap-1.5 sm:grid-cols-2">
            {equipes.map((equipe, indice) => (
              <li
                key={`${equipe.team_name}-${indice}`}
                className="flex items-center gap-2 rounded-lg bg-white/[0.04] px-2.5 py-1.5"
              >
                <Eye
                  size={13}
                  strokeWidth={1.8}
                  aria-hidden="true"
                  className="shrink-0 text-accent-primary"
                />
                <span className="min-w-0 flex-1 truncate text-xs text-text-primary">
                  {equipe.team_name}
                </span>
                <span className="shrink-0 text-[10px] uppercase tracking-[0.12em] text-text-muted">
                  {formatCategoryLabel(equipe.category)}
                </span>
              </li>
            ))}
          </ul>
        </>
      ) : (
        // Interesse ativo nasce da FAMA, então "ninguém de olho" não é bug: é o
        // estado de quem ainda não tem apelo comercial. A frase diz isso, em vez de
        // deixar a caixa vazia sugerindo dado que não carregou.
        <p className="text-[11px] leading-relaxed text-text-secondary">
          {t("driverDetail.market.player.noWatchers")}
        </p>
      )}
    </Cartao>
  );
}

// As cadeiras vazias do mundo, com o veredito de elegibilidade já resolvido pelo
// backend — a MESMA regra de licença e faixa de tier da proposta emergencial, e por
// isso ela não é recalculada aqui.
//
// A vaga inelegível continua na lista de propósito: o jogador tem o direito de ver
// a cadeira que abriu na categoria de cima e saber que ela não é para ele ainda.
function VagasAbertas({ board, t }) {
  const vagas = Array.isArray(board?.vagas) ? board.vagas : [];

  return (
    <Cartao
      titulo={t("driverDetail.market.player.openSeats")}
      testId="driver-detail-open-seats"
      acao={
        board ? (
          <span className="shrink-0 text-[11px] text-text-secondary">
            {t("driverDetail.market.player.seatsCount", {
              eligible: board.vagas_elegiveis ?? 0,
              total: vagas.length,
            })}
          </span>
        ) : null
      }
    >
      {vagas.length ? (
        <ul className="grid gap-1.5">
          {vagas.map((vaga) => (
            <Vaga key={`${vaga.team_id}-${vaga.papel}`} vaga={vaga} t={t} />
          ))}
        </ul>
      ) : (
        <p className="text-[11px] leading-relaxed text-text-secondary">
          {t("driverDetail.market.player.noSeats")}
        </p>
      )}
    </Cartao>
  );
}

function Vaga({ vaga, t }) {
  const elegivel = vaga.licenca_ok && vaga.tier_ok;

  return (
    <li
      data-elegivel={elegivel ? "true" : "false"}
      className={`flex flex-wrap items-center gap-x-3 gap-y-1 rounded-lg px-2.5 py-2 ${
        elegivel ? "bg-accent-primary/[0.08]" : "bg-white/[0.04]"
      }`}
    >
      <span
        aria-hidden="true"
        className="h-5 w-1 shrink-0 rounded-full"
        style={{ backgroundColor: getVividTeamColor(vaga.team_color || "") }}
      />
      <span className="min-w-0 flex-1 truncate text-xs font-medium text-text-primary">
        {vaga.team_name}
      </span>
      <span className="shrink-0 text-[11px] text-text-secondary">
        {formatCategoryLabel(vaga.classe ? `${vaga.categoria}:${vaga.classe}` : vaga.categoria)}
      </span>
      <span className="shrink-0 text-[11px] font-semibold text-text-secondary">
        {formatContractRole(vaga.papel)}
      </span>
      <span className="shrink-0 font-mono text-[11px] tabular-nums text-text-muted">
        {t("driverDetail.market.player.car", { value: vaga.car_performance_rating })}
      </span>
      {/* Salário só vem nas elegíveis: estimar oferta para um assento que o jogador
          não pode ocupar seria inventar uma proposta que o mercado nunca faria. No
          lugar dele vai o MOTIVO da recusa, que é o que ele precisa para saber o
          que lhe falta.

          Número cheio com o sufixo anual, e não a forma compacta que a aba Carreira
          usava: é o MESMO tipo de número das barras de custo logo acima, e as duas
          grafias na mesma aba fariam o jogador comparar `$1.3M` com `$1,300,000/ano`
          de cabeça para descobrir que são iguais. A linha é densa, e o preço disso é
          uma quebra a mais em janela estreita — o `flex-wrap` já a acomoda. */}
      <span className="shrink-0 text-[11px]">
        {Number.isFinite(vaga.salario_estimado) ? (
          <span className="font-mono font-semibold tabular-nums text-accent-primary">
            {formatSalaryAnnual(vaga.salario_estimado)}
          </span>
        ) : (
          <span className="text-text-muted">
            {vaga.licenca_ok
              ? t("driverDetail.market.player.outOfTier")
              : t("driverDetail.market.player.noLicense")}
          </span>
        )}
      </span>
    </li>
  );
}

// A moldura dos dois blocos, no mesmo vocabulário que o resto da aba Mercado já
// usa: o fundo `#0f1c2b` do termômetro e da situação contratual, e o título em
// `text-xs font-semibold text-text-secondary`. Uma caixa própria faria os dois se
// lerem como outra tela colada no fim da aba.
function Cartao({ titulo, acao = null, testId, children }) {
  return (
    <div className="mt-3 rounded-xl bg-[#0f1c2b] px-4 py-3.5" data-testid={testId}>
      <div className="mb-2 flex items-baseline justify-between gap-3">
        <span className="text-xs font-semibold text-text-secondary">{titulo}</span>
        {acao}
      </div>
      {children}
    </div>
  );
}

export default MercadoDoJogador;
