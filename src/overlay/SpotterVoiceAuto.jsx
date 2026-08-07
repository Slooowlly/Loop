import { useEffect, useRef } from "react";
import { useSpotterVizinhanca, useNovosAnuncios } from "./useSpotter";
import { falar, preaquecer } from "../lib/spotterVoice";

// Voz do spotter, ao vivo. Só LÓGICA — não desenha nada.
//
// Mora na janela PRINCIPAL, não na de overlay, por causa da política de autoplay: o
// áudio precisa de um gesto do usuário para o contexto sair de "suspended", e a
// janela principal é a única em que o jogador clica. A de overlay nasce sem foco,
// clique-atravessa, e nunca recebe um clique — um contexto criado lá ficaria mudo
// esperando um gesto que não vem. Som toca com a janela minimizada, então nada se
// perde por ela estar atrás do simulador.
//
// O poll é o mesmo `iracing_spotter_vizinhanca` da faixa visual; ler duas vezes não
// custa (o comando só devolve um snapshot que o amostrador já mantém).

export default function SpotterVoiceAuto() {
  const estado = useSpotterVizinhanca({ intervalMs: 150 });
  const anuncios = useNovosAnuncios(estado);
  const aquecidoRef = useRef(false);

  // Decodifica o pacote na primeira vez que o canal dá sinal de vida. Antes disso
  // não vale ocupar a CPU do menu; depois disso, a primeira fala não pode gastar
  // milissegundos decodificando — é justamente o instante em que eles doem.
  useEffect(() => {
    if (aquecidoRef.current || !estado) return;
    aquecidoRef.current = true;
    preaquecer();
  }, [estado]);

  // Entrega TODOS os anúncios novos, na ordem em que o Rust os confirmou. Quem cede a
  // vez a quem é decisão da camada de voz — ela conhece a prioridade e o que ainda está
  // tocando. Filtrar aqui seria decidir sem essas duas informações.
  useEffect(() => {
    for (const a of anuncios) falar(a.chave);
  }, [anuncios]);

  return null;
}
