import { useState } from "react";
import { useTranslation } from "react-i18next";

import useCareerStore from "../../../stores/useCareerStore";
import CarreiraHeader from "./CarreiraHeader.jsx";
import HistoriaSection from "./HistoriaSection.jsx";
import MercadoSection from "./MercadoSection.jsx";
import MeuPilotoSection from "./MeuPilotoSection.jsx";
import RivaisSection from "./RivaisSection.jsx";
import TrofeusSection from "./TrofeusSection.jsx";
import { useCarreiraData } from "./useCarreiraData.js";

// A aba CARREIRA: a lente do protagonista.
//
// O diagnóstico do roadmap é que o Loop está organizado por MOMENTO da temporada e
// não por assunto — cada coisa nasceu junto do evento que a dispara, e o que ficou
// de fora foram exatamente as visões que não pertencem a nenhum evento. Esta aba é
// a correção para o assunto "eu": ficha do meu piloto (F-02), temporadas passadas
// (F-03), sala de troféus (F-04), rivais (F-05) e mercado em temporada (F-01).
//
// UMA aba, e não cinco, por dois motivos:
//
//   1. O roadmap já pedia F-03+F-04+F-05 numa tela só, porque os três comem da
//      mesma `race_history`. O mesmo argumento vale para os outros dois: F-02 e
//      F-01 leem o MESMO `get_driver_detail` do jogador. Cinco abas custariam
//      cinco buscas do mesmo payload e cinco cabeçalhos discordando entre si.
//   2. A barra de navegação tem quatro entradas. Dobrar para oito para dizer cinco
//      vezes "sobre você" trocaria um buraco de produto por um de navegação.
//
// O cabeçalho fica FORA das pílulas, sempre visível: é ele que responde a crítica
// que abriu o F-02 — hoje o jogador se vê pela mesma lente de qualquer piloto de
// IA. O `DriverDetailModal` continua existindo e continua servindo para olhar
// qualquer um, inclusive o jogador; o que ele não faz é ser um LUGAR.
//
// A ordem das pílulas vai do presente ao passado e termina no que vem: piloto (como
// eu estou), história (de onde eu vim), troféus (o que eu levei), rivais (contra
// quem), mercado (para onde). "Piloto" é a primeira porque é a razão de a aba
// existir — abrir numa pílula que não é a primeira se lê como estado errado.
const SECOES = ["piloto", "historia", "trofeus", "rivais", "mercado"];

function CarreiraTab() {
  const { t } = useTranslation();
  const careerId = useCareerStore((state) => state.careerId);
  const playerId = useCareerStore((state) => state.player?.id);
  const [secao, setSecao] = useState(SECOES[0]);
  const { detail, board, teamInterest, status, error, reload } = useCarreiraData(
    careerId,
    playerId,
  );

  if (status === "loading") {
    return (
      <div
        className="flex min-h-[320px] flex-col items-center justify-center gap-3"
        data-testid="carreira-loading"
      >
        <span className="animate-pulse text-4xl">🏎️</span>
        <p className="text-sm text-text-secondary">{t("carreiraTab.loading")}</p>
      </div>
    );
  }

  if (status === "error" || !detail) {
    return (
      <div
        className="flex min-h-[320px] flex-col items-center justify-center gap-4 px-8 text-center"
        data-testid="carreira-error"
      >
        <p className="text-sm text-status-red">{error || t("carreiraTab.errors.load")}</p>
        <button
          type="button"
          onClick={reload}
          className="rounded-xl border border-white/15 bg-[#0d1727] px-4 py-2 text-xs font-semibold text-text-secondary transition-glass hover:bg-[#14233a] hover:text-text-primary"
        >
          {t("carreiraTab.errors.retry")}
        </button>
      </div>
    );
  }

  return (
    <div className="space-y-5" data-testid="carreira-tab">
      <CarreiraHeader detail={detail} />

      <div className="flex flex-wrap gap-2" role="tablist" data-testid="carreira-secoes">
        {SECOES.map((id) => (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={secao === id}
            onClick={() => setSecao(id)}
            data-testid={`carreira-secao-${id}`}
            className={`rounded-2xl border px-4 py-2 text-xs font-semibold uppercase tracking-[0.13em] transition-glass ${
              secao === id
                ? "border-accent-primary/40 bg-accent-primary/15 text-accent-primary"
                : "border-white/[0.08] bg-black/10 text-text-muted hover:text-text-primary"
            }`}
          >
            {t(`carreiraTab.sections.${id}`)}
          </button>
        ))}
      </div>

      {/* `key` na seção → o painel remonta com o fade da casa a cada troca, o mesmo
          compasso que a troca de aba do Dashboard já tem. */}
      <div key={secao} className="tab-pane-fade">
        {secao === "piloto" ? <MeuPilotoSection detail={detail} careerId={careerId} /> : null}
        {secao === "historia" ? <HistoriaSection detail={detail} /> : null}
        {secao === "trofeus" ? <TrofeusSection detail={detail} /> : null}
        {secao === "rivais" ? <RivaisSection detail={detail} /> : null}
        {secao === "mercado" ? (
          <MercadoSection detail={detail} board={board} teamInterest={teamInterest} />
        ) : null}
      </div>
    </div>
  );
}

export default CarreiraTab;
