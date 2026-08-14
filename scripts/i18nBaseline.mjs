// Passivo de texto em português nos módulos `.js`, congelado string por string.
//
// POR QUE EXISTE: o auditor passou a varrer `.js` em 11/08/2026. Ligar a varredura sem baseline
// bloquearia o commit seguinte por prosa escrita meses antes, e uma saída de centenas de linhas
// não se lê — vira `--no-verify` de hábito, e aí o guard inteiro morre. Com o baseline, o
// passivo fica visível e parado, e o texto NOVO falha na hora.
//
// O QUE ELE NÃO É: não é allowlist de arquivo. A chave é o caminho, o valor é a lista das
// frases exatas que já estavam lá. Frase nova no mesmo arquivo continua sendo apontada.
//
// COMO ESVAZIAR: traduza a frase (`t()`/`i18n.t` + chave nos dois common.json) e APAGUE a linha
// daqui. Entrada que o auditor não acha mais é reportada como órfã por `npm run i18n:audit`,
// então a lista não incha nem apodrece em silêncio.
//
// O QUE **NÃO** ENTRA AQUI: exceção intencional e permanente (dado de exemplo, fixture de
// debug, harness de POC, token que o Rust manda no payload) se marca no arquivo, com
// `// i18n-ignore-file` ou `// i18n-ignore` na linha. Esta lista é só para DÍVIDA — texto que
// se pretende traduzir um dia.
//
// O texto é o NORMALIZADO pelo auditor: interpolação de template vira espaço e espaço repetido
// colapsa (ver `normalizar` em i18nAudit.mjs). Por isso `Volante ${x} · botão ${y}` aparece
// aqui como "Volante · botão".

export const BASELINE_JS = {
  // Rótulo do posto do piloto no dossiê. Migra junto com o resto de `detalhes/`.
  "src/components/driver/detalhes/formatadores.js": ["Piloto N1", "Piloto N2"],

  // Nome do AI roster gravado no export para o iRacing. Sai da nossa tela, mas aparece na do
  // iRacing — traduzir mexe no arquivo exportado, então é decisão do dono do export.
  "src/components/race/nextrace/useIracingExport.js": ["Carreira"],

  // Rótulos do resumo de pré-temporada (o que aconteceu com cada piloto na janela).
  "src/components/season/preSeasonFormatters.js": [
    "Contratação",
    "Proposta recebida",
    "Saiu da equipe",
    "Promoção",
    "Já correu com você",
  ],

  // Erros do microfone do engenheiro. São lidos pelo jogador no overlay, não no console.
  "src/lib/microfone.js": [
    "Este webview não expõe captura de áudio.",
    "Este webview não sabe gravar áudio (sem MediaRecorder).",
    "Microfone não está armado.",
    "Falha ao gravar:",
    "Permissão de microfone negada. O Windows pode estar bloqueando o app em Privacidade → Microfone.",
    "Nenhum microfone encontrado — ou o dispositivo escolhido foi desconectado.",
    "O microfone existe mas outro programa está com ele preso.",
    "Falha ao abrir o microfone:",
  ],

  // Como o jogador lê a associação de tecla/botão na tela de configuração do PTT.
  "src/lib/pttConfig.js": ["Volante · botão"],

  // Estados do push-to-talk do engenheiro, mostrados na linha de status do overlay.
  "src/lib/pttEngenheiro.js": [
    "sem detalhe",
    "rádio mudo por mais s",
    "Microfone não está armado.",
    "s de silêncio — nada foi enviado",
    "Transcrição vazia — nada foi entendido.",
    "Nenhum fato disponível — sem corrida e sem dossiê de demonstração.",
    "· FORÇADO",
    "peças gravadas + empurrão",
    "peças gravadas",
    "frase de espera tocada",
    "voz do modelo · render ms · rms",
    "voz do modelo · SEM NIVELAMENTO",
    '" " (escrita, sem Scribe)',
  ],

  // Texto desenhado no canvas do rádio, sobre a tela do iRacing.
  "src/overlay/radioCanvas.js": ["SEU CARRO · ATENÇÃO", "RÁDIO DA EQUIPE"],

  // Último recurso quando a falha não trouxe mensagem nenhuma; chega à tela como erro.
  "src/utils/bestEffort.js": ["falha sem mensagem"],

  // ARQUIVO GERADO por scripts/gen-track-countries.mjs, a partir de constants/tracks/dados.rs.
  // O gentílico sai do gerador, então a tradução tem de nascer lá (ou virar chave de locale
  // por código de país) — editar este arquivo à mão é reverter no próximo `npm run` que
  // regenerar. Só os países com acento aparecem; o resto não é distinguível de inglês.
  "src/utils/trackCountries.js": [
    "🇦🇺 Austrália",
    "🇲🇽 México",
    "🇮🇹 Itália",
    "🇨🇦 Canadá",
    "🇫🇷 França",
    "🇧🇪 Bélgica",
    "🇯🇵 Japão",
    "🇦🇹 Áustria",
  ],
};
