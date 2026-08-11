import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
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
// Além do poll, o Rust EMPURRA cada anúncio no instante em que confirma (evento
// `spotter-evento`, registrado em `lib.rs`). O empurrão é o caminho normal: em corrida
// a janela do webview fica coberta pelo simulador e o navegador estrangula o
// `setInterval`, atrasando ou perdendo a fala. O poll fica como rede.
//
// Os dois caminhos passam pelo MESMO cursor por id, então o que chega em dobro é
// descartado uma vez só — quem vier segundo já não é maior que o visto.
export const EVENTO_SPOTTER = "spotter-evento";

export function useNovosAnuncios(estado) {
  const [eventos, setEventos] = useState([]);
  const vistoRef = useRef(-1);
  const ancoradoRef = useRef(false);

  // Filtra pelo cursor e entrega o que sobrou. Devolve os aceitos para quem precisa
  // saber se algo passou.
  const entregar = useCallback((lista) => {
    const novos = lista.filter((e) => e && e.id > vistoRef.current);
    if (novos.length === 0) return;
    vistoRef.current = novos[novos.length - 1].id;
    setEventos(novos);
  }, []);

  useEffect(() => {
    const lista = estado?.eventos;
    if (!Array.isArray(lista) || lista.length === 0) return;
    if (!ancoradoRef.current) {
      // 1ª leitura só ancora o cursor: nada do que aconteceu antes de a tela abrir
      // deve soar agora.
      ancoradoRef.current = true;
      vistoRef.current = lista[lista.length - 1].id;
      return;
    }
    entregar(lista);
  }, [estado, entregar]);

  useEffect(() => {
    if (!estaNoTauri()) return undefined;
    let parar = null;
    let morto = false;
    listen(EVENTO_SPOTTER, ({ payload }) => {
      // O empurrão é sempre ao vivo — ele também ANCORA, senão um anúncio que chegasse
      // antes do primeiro poll ficaria preso esperando a âncora que nunca veio.
      ancoradoRef.current = true;
      entregar([payload]);
    })
      .then((un) => {
        if (morto) un();
        else parar = un;
      })
      .catch(() => {
        /* sem ponte — o poll continua respondendo */
      });
    return () => {
      morto = true;
      if (parar) parar();
    };
  }, [entregar]);

  return eventos;
}
