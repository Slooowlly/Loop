import { useEffect } from "react";
import useCareerStore from "../stores/useCareerStore";
import {
  useBreakdownFeed,
  useClassificacaoFeed,
  usePaceFeed,
  usePlayerWarnings,
} from "./useBreakdownFeed";
import { anunciar, pausasDoRadio } from "../lib/engenheiroVoz";

// Voz do RÁDIO DA EQUIPE, ao vivo. Só LÓGICA — não desenha nada. O card continua na janela
// de overlay; o que mora aqui é o som.
//
// Mora na janela PRINCIPAL pelo mesmo motivo que o `SpotterVoiceAuto` e o `EngenheiroPttAuto`,
// e a razão está medida: a política de autoplay exige um gesto do usuário para o contexto de
// áudio sair de "suspended", e a janela de overlay nasce sem foco e clique-atravessa — ela
// nunca recebe um clique. O engenheiro falava de lá, contra a regra que os outros dois já
// seguiam, e por isso o spotter era ouvido e ele não.
//
// A segunda razão é o RELÓGIO. A janela de overlay fica oculta enquanto o app não reconhece
// sessão ao vivo, e o navegador estrangula os timers de janela oculta: medido em 2026-08-11,
// o intervalo de 800 ms virou 20 SEGUNDOS. Um rádio que explica um acidente vinte segundos
// depois não explica nada. Aqui os mesmos hooks rodam no ritmo pedido.
//
// Som toca com a janela minimizada, então nada se perde por ela estar atrás do simulador.

export default function EngenheiroVozAuto() {
  const careerId = useCareerStore((s) => s.careerId);

  // O AVISO DO NOSSO CARRO não passa pela carreira, e é de propósito: ele fala do carro do
  // jogador — peça em risco, quebra, castigo da classificação —, tudo que o monitor sabe
  // pela telemetria. Exigir carreira aberta calava o engenheiro justamente em quem está
  // testando uma etapa, e o acidente tem de ser comentado de qualquer jeito.
  const aviso = usePlayerWarnings(true);
  // Estes dois SIM dependem da carreira: as frases nomeiam pilotos e equipes, que saem do
  // banco do save. Sem carreira não há o que dizer, e não há como dizer.
  const quebra = useBreakdownFeed(careerId);
  const ritmo = usePaceFeed(careerId);
  // A CLASSIFICAÇÃO também não passa pela carreira, e pelo mesmo motivo do aviso: nenhuma fala
  // dela nomeia piloto ou equipe. Ela é a família com o prazo mais curto do rádio — a despedida
  // termina pouco antes da linha —, então é aqui, na janela que nunca é estrangulada, que ela
  // precisa estar.
  const classificacao = useClassificacaoFeed(true);

  useEffect(() => {
    if (!aviso?.pecas?.length) return;
    anunciar(aviso.pecas, {
      pausasMs: pausasDoRadio(aviso.pecas),
      canal: "aviso",
      texto: aviso.text ?? "",
    }).catch(() => {});
  }, [aviso]);

  useEffect(() => {
    if (!quebra?.pecas?.length) return;
    anunciar(quebra.pecas, {
      pausasMs: pausasDoRadio(quebra.pecas),
      canal: "quebra",
      texto: quebra.text ?? "",
    }).catch(() => {});
  }, [quebra]);

  useEffect(() => {
    if (!ritmo?.pecas?.length) return;
    anunciar(ritmo.pecas, {
      pausasMs: pausasDoRadio(ritmo.pecas),
      canal: "ritmo",
      texto: ritmo.text ?? "",
    }).catch(() => {});
  }, [ritmo]);

  useEffect(() => {
    if (!classificacao?.pecas?.length) return;
    anunciar(classificacao.pecas, {
      pausasMs: pausasDoRadio(classificacao.pecas),
      canal: "classificacao",
      texto: classificacao.text ?? "",
    }).catch(() => {});
  }, [classificacao]);

  return null;
}
