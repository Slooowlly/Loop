// A política de CURSOR dos feeds do overlay, isolada do polling.
//
// Os três canais do rádio (quebras da grade, ritmo, avisos do nosso carro) leem listas que só
// crescem e precisam mostrar cada mensagem uma vez. A regra é sempre a mesma — ancorar na
// primeira leitura, depois avançar por id —, mas ela vivia copiada dentro de três `useEffect`,
// onde não dava para testar sem montar componente e fingir o Tauri.
//
// O defeito que motivou a extração: as três cópias ancoravam na primeira leitura NÃO VAZIA.
// Como os logs do monitor nascem vazios a cada tentativa, a âncora ficava pendente e consumia
// a PRIMEIRA fala de cada sessão — sem card, sem áudio e sem erro em lugar nenhum. Nos avisos
// do nosso carro, que são poucos e espaçados, o primeiro é muitas vezes o único.

/// Um passo do cursor sobre uma leitura do feed.
///
/// - `feed`: a lista lida agora (pode estar vazia).
/// - `seen`: maior id já exibido; `-1` quando nada foi exibido.
/// - `primed`: a âncora da primeira leitura já foi posta?
/// - `umaPorVez`: avança para a MAIS ANTIGA ainda não vista (feed de quebras) em vez de pular
///   para a mais nova. Pular descartava quebras quando duas caíam entre dois polls.
///
/// Devolve `{ primed, seen, mostrar }`, onde `mostrar` é a mensagem a exibir ou `null`.
export function passoDoCursor({ feed, seen, primed, umaPorVez = false }) {
  if (!primed) {
    // Ancora no que JÁ existia — e ancorar em "nada" é uma âncora legítima, não um adiamento.
    return {
      primed: true,
      seen: feed.length ? feed[feed.length - 1].id : -1,
      mostrar: null,
    };
  }
  if (feed.length === 0) return { primed, seen, mostrar: null };
  const proxima = umaPorVez
    ? feed.find((m) => m.id > seen)
    : [feed[feed.length - 1]].find((m) => m.id > seen);
  if (!proxima) return { primed, seen, mostrar: null };
  return { primed, seen: proxima.id, mostrar: proxima };
}
