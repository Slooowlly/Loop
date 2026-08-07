import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { estaNoTauri } from "../lib/tauri";

// Vizinhança LATERAL do carro (`iracing_spotter_vizinhanca`): quem está à esquerda,
// à direita, e quando são três largos.
//
// Hoje o único consumidor é a VOZ (`SpotterVoiceAuto`). Houve uma faixa na tela com
// as três células e o valor cru do canal — instrumentação para provar que o
// `CarLeftRight` é preenchido em sessão offline com IA. Foi provado, e ela saiu: o
// spotter que funciona é o que se ouve sem tirar os olhos da pista.
//
// O poll é rápido (120 ms) e pode ser: o comando não toca no SDK nem no banco — só
// lê o snapshot que o amostrador de fundo já mantém a 60 Hz. Quem detecta é o Rust;
// aqui só se desenha. Um indicador de proximidade a 2 Hz (o ritmo da torre) chegaria
// depois da freada que ele deveria avisar.
//
// Devolve `{ bruto, vizinhanca, aoLado, tresLargos, esquerda, direita, eventos }`, ou
// null enquanto não houver leitura. `bruto` é o `CarLeftRight` sem interpretação — é
// o campo que responde "o canal está vivo nesta sessão?".

export function useSpotterVizinhanca({ intervalMs = 120, active = true } = {}) {
  const [estado, setEstado] = useState(null);

  useEffect(() => {
    if (!estaNoTauri() || !active) {
      setEstado(null);
      return undefined;
    }
    let stopped = false;
    const tick = async () => {
      try {
        const v = await invoke("iracing_spotter_vizinhanca");
        if (!stopped) setEstado(v);
      } catch {
        /* sem sessão — silencioso */
      }
    };
    tick();
    const timer = setInterval(tick, intervalMs);
    return () => {
      stopped = true;
      clearInterval(timer);
    };
  }, [intervalMs, active]);

  return estado;
}

// TODOS os anúncios confirmados desde a última leitura, em ordem. Cursor por id e
// "prime" na primeira leitura, pra não reproduzir o que aconteceu antes de a tela abrir.
//
// Devolve uma LISTA, e não o último, porque o spotter deixou de ter um assunto só. Com
// dois detectores independentes escrevendo na mesma fila — vizinhança lateral e obstáculo
// à frente —, dois eventos podem cair dentro do mesmo intervalo de poll (150 ms, contra
// uma detecção a 60 Hz). Pegar só o último descartava o outro em silêncio, e o Rust já
// tinha dado o aviso por entregue: a cadeia "nunca descarta, no máximo adia" tinha um
// furo exatamente na última ponta.
//
// Quem decide o que realmente soa é a camada de voz, que conhece prioridade e o que
// ainda está tocando. Aqui só se entrega tudo o que chegou.
export function useNovosAnuncios(estado) {
  const [eventos, setEventos] = useState([]);
  const vistoRef = useRef(-1);
  const ancoradoRef = useRef(false);

  useEffect(() => {
    const lista = estado?.eventos;
    if (!Array.isArray(lista) || lista.length === 0) return;
    const ultimo = lista[lista.length - 1];
    if (!ancoradoRef.current) {
      ancoradoRef.current = true;
      vistoRef.current = ultimo.id;
      return;
    }
    const novos = lista.filter((e) => e.id > vistoRef.current);
    if (novos.length === 0) return;
    vistoRef.current = novos[novos.length - 1].id;
    setEventos(novos);
  }, [estado]);

  return eventos;
}
