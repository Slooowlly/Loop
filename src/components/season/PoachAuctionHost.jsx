import { useState, useEffect } from "react";
import useCareerStore from "../../stores/useCareerStore";
import PoachAuctionModal from "./PoachAuctionModal";

// Host global do leilão de quebra de contrato (Fase 2b.3): montado no nível do App,
// mostra o modal sempre que o store tiver uma `poachOffer` — venha ela do fluxo real
// (janela de mercado) ou do botão de debug, de qualquer tela. Guarda uma cópia local
// pra a tela sobreviver ao momento em que o store zera a oferta ao resolver (assim o
// desfecho aparece até o jogador clicar "Continuar").
export default function PoachAuctionHost() {
  const poachOffer = useCareerStore((s) => s.poachOffer);
  const resolvePlayerPoachOffer = useCareerStore((s) => s.resolvePlayerPoachOffer);
  const isResolvingPoach = useCareerStore((s) => s.isResolvingPoach);
  const [active, setActive] = useState(null);

  useEffect(() => {
    if (poachOffer && !active) setActive(poachOffer);
  }, [poachOffer, active]);

  if (!active) return null;
  return (
    <PoachAuctionModal
      offer={active}
      isResolving={isResolvingPoach}
      onDecide={(accept) => resolvePlayerPoachOffer(accept)}
      onClose={() => setActive(null)}
    />
  );
}
