import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { estaNoTauri } from "../lib/tauri";
import * as microfone from "../lib/microfone";
import * as voz from "../lib/engenheiroVoz";
import { criarOrquestrador } from "../lib/pttEngenheiro";
import { estaEmTeste, GATILHO_STORE, lerGatilhoSalvo, lerMicSalvo } from "../lib/pttConfig";
import useCareerStore from "../stores/useCareerStore";

// Push-to-talk do engenheiro, ao vivo. Só LÓGICA — não desenha nada.
//
// Mora na janela PRINCIPAL pelo mesmo motivo que o `SpotterVoiceAuto`: a política de
// autoplay exige um gesto do usuário para o contexto de áudio sair de "suspended", e a
// principal é a única janela em que o jogador clica. A de overlay nasce sem foco e
// clique-atravessa — um contexto criado lá ficaria mudo esperando um gesto que não vem.
//
// O microfone abre e fecha com a SESSÃO, não com o app. Enquanto está aberto o Windows
// mostra o indicador de microfone em uso, e isso é correto: o jogador armou de propósito.
// Deixá-lo aberto no menu principal seria uma luz acesa sem motivo; abri-lo só no toque
// do botão custaria de 100 a 500 ms de `getUserMedia` justo quando ele começa a falar.

/// De quanto em quanto se pergunta "estou numa sessão?". Dois segundos: a única decisão
/// que depende disso é armar o microfone, e ela não tem pressa nenhuma.
const POLL_SESSAO_MS = 2000;

export default function EngenheiroPttAuto() {
  const orqRef = useRef(null);
  // Só a trava de "há um `armar()` em voo". Se o microfone ESTÁ armado quem responde é o
  // módulo, que confere a faixa; uma cópia aqui envelhece na primeira faixa que morre.
  const armandoRef = useRef(false);
  const aquecidoRef = useRef(false);

  // O orquestrador é criado uma vez e sobrevive a re-renderizações: ele guarda a geração
  // da pergunta em curso, e recriá-lo no meio de uma resposta perderia o cancelamento.
  // Lido por REF, e não capturado: o orquestrador é criado uma vez e sobrevive a
  // re-renderizações, então um valor copiado na criação ficaria preso no `null` de antes
  // de a carreira carregar — e o engenheiro nunca mencionaria o campeonato.
  const careerId = useCareerStore((s) => s.careerId);
  const careerIdRef = useRef(careerId);
  careerIdRef.current = careerId;

  if (!orqRef.current) {
    orqRef.current = criarOrquestrador({
      microfone,
      voz,
      invoke,
      careerId: () => careerIdRef.current || "",
    });
  }

  // A VOZ PRÓPRIA: o sobrenome do jogador, sintetizado uma vez e guardado no save dele.
  //
  // Uma chamada por carreira carregada, e não por sessão de corrida: o Rust responde do disco
  // depois da primeira vez, e a peça precisa estar decodificada ANTES da primeira pergunta —
  // quem pergunta na volta 1 não pode esperar um render.
  //
  // Falha em silêncio de propósito. Sem rede, sem save ou antes das três corridas o comando
  // devolve `null` e o engenheiro continua falando; o que se perde é o nome na frente da
  // frase, e a próxima abertura do app tenta de novo.
  useEffect(() => {
    if (!estaNoTauri() || !careerId) return undefined;
    let parado = false;
    invoke("engenheiro_voz_propria", { careerId })
      .then((p) => {
        if (parado || !p) return;
        return voz.registrarPecaPropria(p.chave, p.audio_b64, p.mime);
      })
      .catch(() => {});
    return () => {
      parado = true;
    };
  }, [careerId]);

  // OS MOMENTOS SEM PRESSA. Volta de formação e bandeirada: o único instante do rádio em
  // que nada disputa o canal, e o único em que os segundos do modelo não custam nada.
  //
  // Vai por `anunciarRemoto`, e não por `falarRemoto`: ninguém perguntou, então esta fala
  // ESPERA a vez como qualquer outra notícia. O trinco de uma vez por corrida vive no Rust,
  // que é quem sabe a subsessão.
  useEffect(() => {
    if (!estaNoTauri()) return undefined;
    let parado = false;
    const tick = async () => {
      let oc;
      try {
        oc = await invoke("engenheiro_ocasiao", { careerId: careerIdRef.current || "" });
      } catch {
        return;
      }
      if (parado || !oc?.linhas?.length) return;
      try {
        const falada = await invoke("ptt_responder", {
          transcricao: "",
          intencao: "geral",
          linhas: oc.linhas,
          ocasiao: oc.ocasiao,
          tratamento: oc.tratamento ?? null,
        });
        if (!parado) voz.anunciarRemoto(falada.audio_b64, falada.mime);
      } catch {
        // O trinco já fechou lá no Rust, de propósito: uma ocasião perdida custa uma fala,
        // e um trinco que só fechasse no sucesso custaria uma ida ao servidor por poll
        // enquanto ele estivesse fora do ar.
      }
    };
    const id = setInterval(tick, POLL_SESSAO_MS);
    return () => {
      parado = true;
      clearInterval(id);
    };
  }, []);

  // O PORTÃO DO MOMENTO. Quem decide o que é quente é o Rust, que tem a telemetria; este
  // componente só liga os dois, e é ele porque já é o dono do ciclo de vida do rádio na
  // janela principal.
  //
  // Vale só para a fila de ANÚNCIOS. A resposta ao push-to-talk e a fala do spotter não
  // passam por aqui — a primeira porque o piloto perguntou, a segunda porque é segurança.
  useEffect(() => {
    if (!estaNoTauri()) return undefined;
    voz.definirPortao(() => invoke("engenheiro_momento_quente").catch(() => null));
    return () => voz.definirPortao(null);
  }, []);

  // Reempurra a ligação salva ao backend. O vigia vive no Rust e não conhece o
  // localStorage; sem isto, o botão só voltaria a funcionar depois de abrir as
  // configurações — que é o último lugar em que o jogador vai olhar.
  useEffect(() => {
    if (!estaNoTauri()) return undefined;
    const empurrar = () => {
      invoke("ptt_set_gatilho", { gatilho: lerGatilhoSalvo() }).catch(() => {});
    };
    empurrar();
    // A associação pode mudar noutra tela; `storage` cobre outra janela, e o evento
    // próprio cobre a mesma.
    const aoTrocar = (e) => {
      if (!e || e.key === GATILHO_STORE) empurrar();
    };
    window.addEventListener("storage", aoTrocar);
    window.addEventListener("loop:ptt-gatilho", aoTrocar);
    return () => {
      window.removeEventListener("storage", aoTrocar);
      window.removeEventListener("loop:ptt-gatilho", aoTrocar);
    };
  }, []);

  // Os dois eventos do vigia. `apertar`/`soltar` são assíncronos e o `listen` não espera
  // por eles — de propósito: a máquina de estados já resolve a ordem sozinha, e segurar o
  // canal de eventos do Tauri numa gravação inteira seria pior.
  useEffect(() => {
    if (!estaNoTauri()) return undefined;
    const limpezas = [];
    // `listen` é assíncrono e a limpeza do efeito é síncrona: sem esta bandeira, uma
    // desmontagem que acontece antes de o ouvinte existir deixa `limpezas` vazio e o
    // ouvinte órfão — vivo e sem ninguém para removê-lo. Em dev o StrictMode monta duas
    // vezes de propósito, e o resultado era cada toque no botão virar duas perguntas.
    let morto = false;
    const registrar = (fn) => {
      if (morto) fn();
      else limpezas.push(fn);
    };
    (async () => {
      try {
        // O bloco de teste das configurações usa o MESMO botão. Enquanto ele está aberto,
        // quem responde é ele: senão, segurar o botão para conferir o microfone dispararia
        // uma pergunta de verdade ao engenheiro por cima do teste.
        registrar(await listen("ptt-apertou", () => !estaEmTeste() && orqRef.current.apertar()));
        registrar(await listen("ptt-soltou", () => !estaEmTeste() && orqRef.current.soltar()));
      } catch {
        /* sem eventos: o push-to-talk fica pela interface */
      }
    })();
    return () => {
      morto = true;
      limpezas.forEach((fn) => fn && fn());
    };
  }, []);

  // Sessão aberta = microfone armado + servidor aquecido.
  //
  // O portão é `iracing_connected`, e não a vizinhança do spotter. Aquele comando devolve
  // uma ESTRUTURA, e `Boolean(estrutura)` é `true` em toda leitura — com o simulador
  // aberto, fechado ou nunca instalado. O portão não abria nem fechava: dava sempre
  // aberto. E é essa constante que deixa o engenheiro surdo em corrida — o microfone abria
  // UMA vez, na primeira volta do laço, que acontece no menu principal segundos depois do
  // boot. É o pior instante possível para abri-lo: o rig
  // ainda não está ligado, então o dispositivo ESCOLHIDO costuma não existir e a captura
  // cai no padrão do Windows, exatamente o defeito que passar o `deviceId` foi posto para
  // evitar. E, como o portão nunca mais mudava de valor, não havia segunda tentativa: a
  // corrida inteira era ouvida pela placa errada, ou por uma faixa já morta.
  //
  // Quem sabe se há microfone é o módulo do microfone, que confere a FAIXA. Um `useRef`
  // espelhando isso aqui era a segunda fonte de verdade que travava o rearme.
  useEffect(() => {
    if (!estaNoTauri()) return undefined;
    let parado = false;

    const tick = async () => {
      // A bancada das configurações abre e fecha o mesmo microfone. Enquanto ela está
      // aberta, quem manda é ela: desarmar por baixo a cada 2 s quebraria a única tela em
      // que o jogador consegue provar que o hardware responde.
      if (estaEmTeste()) return;

      let emSessao = false;
      try {
        emSessao = Boolean(await invoke("iracing_connected"));
      } catch {
        emSessao = false;
      }
      if (parado) return;

      if (!emSessao) {
        // Só na BORDA de fechar. `cancelar()` cala o rádio, e chamá-lo a cada volta do
        // laço cortaria as falas que o engenheiro dá fora de sessão.
        if (microfone.estaArmado()) {
          orqRef.current.cancelar();
          microfone.desarmar();
        }
        return;
      }

      // Aqui NÃO se aquece o acervo. Ele tem 3.943 peças e 324 MB, e este bloco roda de
      // novo a cada 2 s enquanto o microfone falhar ao armar. Aquecer daqui multiplicava o
      // acervo inteiro por tentativa e afogava o carregamento das falas do spotter. Quem
      // aquece agora é `tocarSequencia`, com a lista da resposta.
      if (!aquecidoRef.current) {
        aquecidoRef.current = true;
        // Tira o Cloud Run do zero AGORA. O aquecimento que já existia dispara nas voltas
        // finais, para o debrief; a primeira pergunta ao engenheiro pode vir na volta 1, e
        // seria ela a pagar os 20 a 40 s de cold start.
        invoke("ptt_aquecer").catch(() => {});
      }

      // Armado e vivo: nada a fazer. `armar()` sozinho já é barato nesse caso, e a
      // pergunta é feita aqui só para a trava de concorrência não ser paga à toa.
      if (microfone.estaArmado() || armandoRef.current) return;
      // `getUserMedia` leva de 100 a 500 ms e o laço bate a cada 2 s — sem a trava, um
      // dispositivo lento renderia duas aberturas concorrentes do mesmo microfone.
      armandoRef.current = true;
      try {
        // COM o dispositivo escolhido. Sem isto a corrida abria o padrão do Windows, que
        // num rig de VR costuma ser o áudio virtual do headset — o engenheiro ficava
        // ouvindo a placa errada e nada indicava por quê.
        await microfone.armar({ deviceId: lerMicSalvo() });
      } catch {
        // Sem microfone o resto do encanamento não tem o que fazer. O erro aparece nas
        // configurações, onde há espaço para explicá-lo; aqui a próxima volta do laço
        // tenta de novo, que é o que resolve o dispositivo que ainda estava subindo.
      } finally {
        armandoRef.current = false;
      }
    };

    tick();
    const timer = setInterval(tick, POLL_SESSAO_MS);
    return () => {
      parado = true;
      clearInterval(timer);
      microfone.desarmar();
    };
  }, []);

  return null;
}
