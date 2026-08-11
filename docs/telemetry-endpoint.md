# Endpoint `/telemetry` — ciclo de vida das corridas (produto)

O save local responde tudo sobre **um** jogador e nada sobre o conjunto. Esta é a
telemetria que responde as perguntas de produto: **quantas corridas estão rolando
agora**, **em que temporada e categoria cada instalação está** e quanto tempo de pista
roda por dia.

Anônima e opt-out: a chave é o `install_id` (UUID da máquina, sem vínculo com e-mail ou
conta). Nome de piloto, nome de equipe e conteúdo de save **nunca** saem do app.

Duas populações, duas perguntas. Quem **corre** no iRacing produz o ciclo de vida
completo da corrida (`race_start` → `race_ping` → `race_end`), com desfecho. Quem
**simula** dentro do app produz um `race_sim` e nada mais: ali a pergunta não é como foi
a corrida, é quanta gente joga assim.

## As duas metades

- **Cliente** — `src-tauri/src/telemetry.rs`, pendurado em bordas que o `race_monitor`
  já detectava para outros fins. Nada é inventado, nada bloqueia: cada envio sai em
  thread própria, com três tentativas, log em arquivo e fila em disco (ver "Entrega").
- **Servidor** — `index.js` do repo `iracer-news-server` (Cloud Run, mesmo serviço de
  `/race-story` e `/pre-race`). **Esse repo não é versionado**; este documento é o
  registro do contrato.

| Evento | Borda no cliente | Onde |
|---|---|---|
| `race_start` | largada verde (`prev_session_state < RACING` → `>= RACING`) | `race_monitor.rs:2441` |
| `race_ping` | a cada 30 min de corrida aberta | `race_monitor.rs:3323` |
| `race_end` | `finalize_attempt()` (bandeirada/DNF) | `race_monitor.rs:1599` |
| `app_start` | boot do app, no `setup()` | `lib.rs` |
| `race_end` | queda da conexão com o sim (`sim_closed`) | `race_monitor.rs:3346` |
| `race_sim` | fim de semana simulado dentro do app | `commands/race.rs`, no sucesso de `simulate_race_weekend_in_base_dir` |
| `weekend_usage` | a rodada virou (fecha o bloco de leitura anterior) | `commands/race.rs` e `commands/iracing/importacao.rs` |

### O `session_id` é o que amarra tudo

Um UUID por **abertura do app**, gerado no `init()` e carimbado em **todo** evento.
Dois eventos com o mesmo `session_id` são, por definição, a mesma vez que a pessoa abriu
o Loop.

É ele que responde, sem nenhuma heurística de janela de tempo:

- **Quantas vezes o app foi aberto** — contagem de `app_start`.
- **Quantas corridas por sessão** — `race_start` e `race_sim` agrupados por `session_id`.
- **Quantas corridas por dia** — os mesmos eventos agrupados pela data do servidor.
- **Quanto dura uma sessão** — do `app_start` ao último evento daquele `session_id`.

A alternativa seria o servidor inferir sessão por proximidade de horário, e isso erra
exatamente onde importa: quem joga duas horas seguidas com uma pausa no meio vira duas
sessões, e quem abre e fecha três vezes em dez minutos vira uma.

### O `race_sim` manda o mínimo, de propósito

Só o fato de ter simulado, mais o contexto de carreira que já viaja em todo evento.
**Nada da corrida**: nem pista, nem posição, nem resultado, nem duração. A unidade de
medida aqui é a pessoa, não a prova — o que se quer saber é quantas instalações jogam
simulando em vez de correr, e para isso o desfecho da etapa é ruído.

Só o fim de semana regular conta. O bloco especial de convocação (`simulate_special_block`)
ficou de fora: é sazonal e opcional, e contá-lo faria a mesma pessoa aparecer mais vezes
em algumas semanas do ano, sem que ela tenha jogado mais.

### Por que 30 min de ping

O ping **não é o detector**. Início e fim vêm por borda, instantâneos — uma corrida de
15 min é registrada com precisão de segundos sem nunca pingar. O ping existe só como
antídoto contra **corrida-fantasma**: PC desligado no meio, `race_end` que nunca chega.

Como a corrida mais curta do jogo é de 15 min, 30 min (o dobro) significa que o caso
comum custa **zero ping**. Um enduro de 2h pinga 3 vezes. Uma corrida órfã fica no
contador por no máximo ~35 min e expira.

## Contrato HTTP

**Request** `POST /telemetry` — header `x-app-secret: <APP_SECRET>` (mesmo dos demais).

Campos em todo evento:

```json
{
  "event": "app_start | race_start | race_ping | race_end | race_sim | weekend_usage",
  "install_id": "<uuid>",
  "session_id": "<uuid da abertura do app>",
  "subsession_id": 12345678,
  "app_version": "0.9.1",
  "os": "windows"
}
```

O `subsession_id` é o único que não vale para todos: só existe onde houve sessão de
iRacing, então `app_start`, `race_sim` e `weekend_usage` não o trazem.

Mais, quando há carreira carregada (anexado por `set_career_context()`, chamado em
`load_career_in_base_dir` — `commands/career.rs`): `ano_carreira`, `categoria`,
`dificuldade`, `temporadas_completas`, `corridas_totais`. A **dificuldade viaja em todo
evento**, não só no fim de corrida: é o eixo pelo qual o desfecho é lido — posição e
ritmo só calibram a curva se você souber em que nível aquela corrida foi disputada.

**`ano_carreira` é 1, 2, 3…**, o número da temporada ativa, e nunca o ano do calendário.
2026 não diz nada sobre onde a pessoa está na progressão, e duas carreiras começadas em
anos diferentes ficariam incomparáveis por nada. O campo antigo chamava-se `ano` e
carregava o ano civil; o servidor precisa trocar o `by_ano` do `/summary` por
`by_ano_carreira`.

Específicos por evento:

| Evento | Campos |
|---|---|
| `race_start` | `track_id` |
| `race_ping` | `elapsed_s` |
| `race_end` | `track_id`, `duracao_s`, `status`, `restarts`, `restarts_quali` + o desfecho (abaixo) |
| `race_sim` | nenhum |
| `app_start` | nenhum |
| `weekend_usage` | `temporada`, `rodada`, `categoria_rodada`, `segundos_noticias`, `segundos_briefing`, `segundos_debriefing`, `ptt_apertos`, `ptt_perguntas` |

### O `weekend_usage`, ou onde o dinheiro é gasto

Mede o tempo nas **três telas que custam geração de IA**: notícias, briefing e
debriefing. Calendário, tabela e ficha de piloto não custam nada, e medir permanência
nelas seria vigiar por vigiar.

Vai **por rodada**, e não por sessão de app: "12 minutos de notícias" sozinho não decide
nada, mas cruzado com a etapa ele responde se a matéria gerada para aquela corrida chegou
a ser lida. Uma prévia de 900 palavras que custou dois provedores e foi fechada em três
segundos é o número que justifica o evento inteiro.

**O bloco fecha quando a rodada vira**, e não no fim da corrida. A janela medida vai de
uma corrida à seguinte: o debriefing da rodada N, as notícias da semana e o briefing da
N+1 caem todos no bloco rotulado N. Fechar no `race_end` perderia justamente o
debriefing, que só é lido depois da bandeirada.

O acumulado vive em `<app_data>/telemetria-uso.json`, gravado a cada atualização. Quem lê
o debriefing e fecha o Loop não perde o tempo: o bloco vira evento no boot seguinte, com
o `atraso_s` dizendo quanto esperou.

Os dois contadores de push-to-talk medem coisas diferentes de propósito:

| Campo | Onde é contado | O que significa |
|---|---|---|
| `ptt_apertos` | borda física do botão, em `commands/ptt.rs` | inclui toque acidental e aperto cortado pelo freio do rádio |
| `ptt_perguntas` | `ptt_transcrever`, em `commands/ptt_voz.rs` | o áudio subiu: custou transcrição e vai custar geração |

A diferença entre os dois é o aperto que **não** custou nada. Se ela for grande, o freio
do rádio está trabalhando (ou o jogador está esbarrando no volante), e as duas leituras
pedem ações opostas.

### O desfecho, no `race_end`

Montado pelo `build_race_outcome()` a partir do que o `race_monitor` já acumulou para o
painel pós-corrida — nenhuma amostragem nova, nenhum custo por tick.

| Campo | Nota |
|---|---|
| `posicao_final`, `posicao_grid`, `carros_na_classe` | Andam **juntos**. Quinto entre 8 não é quinto entre 24, e quinto partindo de décimo segundo não é quinto partindo da pole. Sozinha, a posição final produz uma média que mistura tudo. |
| `melhor_volta_s`, `melhor_volta_classe_s` | A razão entre as duas é o **déficit de ritmo** — o melhor sinal de dificuldade, imune a tamanho de grid, incidente e abandono. A volta do jogador sozinha não é comparável nem com ele mesmo em outra pista. |
| `carro` | Junto com `track_id`, fecha a chave de comparação de volta. 1:35 é rápido num carro e lento noutro. |
| `voltas`, `incidentes` | `incidentes` é o **único** onde `0` é valor legítimo (e o mais interessante deles). |
| `restarts`, `restarts_quali` | Reinício de sessão, contado **por sessão** na borda em que o `race_monitor` já o detecta. Ver abaixo. |

### Reinício de sessão, e o número que estava errado

O SDK entrega os dois casos, e o `race_monitor` já detectava ambos antes desta
telemetria existir. São dois sinais diferentes:

- **A fronteira de sessão é um fato**, não uma heurística: o `SessionNum` muda, e o YAML
  do fim de semana já disse qual número é a quali e qual é a corrida
  (`qualy_session_num`, `race_session_num`).
- **O reinício é inferido** pelo relógio, em `restarted()`: o `SessionTime` cai de volta
  para perto de zero, ou o `lap_completed` regride. Só vale contra uma tentativa que já
  largou, então voltar ao box antes da largada não conta.

O que ia para o servidor até 10/08/2026 era `attempt_number - 1`. A tentativa também é
recriada a cada troca de sessão, então **um fim de semana normal (treino → quali →
corrida, sem ninguém reiniciar nada) reportava dois reinícios**. O número existia, mas
media outra coisa.

Agora são dois contadores próprios, zerados quando a subsessão muda, incrementados na
borda do reinício conforme o `SessionNum` do tick. Ficam separados de propósito: refazer a
quali é o jogador caçando uma volta boa, refazer a corrida é ele fugindo de um resultado.
Somados num campo só, viram um número que não decide nada.

No `/telemetry/report` isso aparece em `corridas.restarts`, com um terceiro número:
`corridas_com_reinicio`. O total sozinho se confunde — dez reinícios podem ser uma pessoa
teimando ou dez pessoas desistindo.
| `off_track`, `towed`, `garage`, `black_flag`, `disqualified`, `pior_batida` | Só aparecem quando verdadeiros. |

**Campo ausente ≠ zero.** Número que o cliente não conseguiu determinar é **omitido** do
payload, e o servidor recusa `0` nos campos de posição. Guardar zero faria o jogador ter
"largado da posição zero e feito uma volta de zero segundo" — e uma média contaminada por
isso não sustenta decisão nenhuma.

**Em DNF, posição e volta não valem.** O `status` é o filtro obrigatório de qualquer
média: um abandono na volta 2 entraria na estatística como "último lugar".

`status` = `finished` \| `dnf` \| `not_started` (vêm prontos do `finalize_attempt()`)
\| `sim_closed` (queda da conexão) \| `superseded` (outra subsessão abriu sem a anterior
ter fechado).

**Não existe campo `ts`.** Quem carimba a hora é o servidor — o relógio da máquina do
jogador não é confiável, e o evento chega em segundos de qualquer jeito.

**Resposta** `200 {"ok": true}`. O cliente ignora o corpo; o STATUS ele usa (ver
"Entrega"). `202 {"dropped":"daily_cap"}` quando o teto diário estoura, `400` para
evento malformado, `401` sem o segredo.

## Entrega

Medido em 08/08/2026, numa instalação de teste: cinco corridas dirigidas no iRacing,
consentimento ligado, e o servidor recebeu **um** `race_start` e nenhum `race_end`. O
timeout era de 5s contra um Cloud Run com scale-to-zero, cujo cold start passa de 20s.
E como o resultado do POST era descartado, não havia log para distinguir "saiu e falhou"
de "nunca foi disparado".

O que existe hoje no `send()`:

| Peça | Valor | Por quê |
|---|---|---|
| Timeout | 20s | O mesmo motivo dos 45s do cliente de notícias: cold start do Cloud Run. Roda em thread de fundo, então esperar não custa ao jogo. |
| Tentativas | 3, com 3s e 12s de espera | O cold start acontece uma vez. Quando a primeira paga a subida do container, a segunda o acha de pé. |
| Log | uma linha por desfecho, categoria `telemetria` | Vai para `<app_data>/logs/loop.log`, que o jogador já manda pelo botão de enviar log. Sem isso o diagnóstico é cego. |
| Fila | `<app_data>/telemetria-fila.jsonl` | O que não entrou em nenhuma tentativa espera o próximo boot. Teto de 200 eventos e 7 dias. |

**4xx não vai para a fila.** Corpo malformado ou segredo errado é recusa definitiva:
repetir não conserta e a fila só encheria de evento natimorto. `408` e `429` são exceção,
porque ali o servidor está pedindo para tentar de novo. Erro de rede, timeout e `5xx`
entram na fila.

**Drenagem** (`drenar_fila()`, chamada no boot logo depois do `init()`): uma tentativa por
evento, em ordem, e ao terceiro fracasso seguido o resto volta para a fila. Servidor fora
do ar é servidor fora do ar, e insistir nos 200 seguintes queimaria uma hora de thread.

**`atraso_s`** é o único campo novo no payload: segundos entre o evento nascer e o POST
ser aceito. Só aparece a partir de 30s. Existe porque quem carimba a hora é o servidor, e
um evento drenado amanhã chegaria datado de amanhã.

**O servidor desconta.** Todo carimbo do evento (`last_seen`, `started_at`, `ended_at`,
`expire_at`) sai de `chegada - atraso_s`, e o contador diário cai no dia em que o evento
**aconteceu**, não no dia em que chegou. Sem isso, uma corrida drenada no boot seguinte
apareceria como "rolando agora" no `/telemetry/live` por 35 minutos, e o gráfico por dia
mentiria duas vezes: faltando em ontem e sobrando em hoje.

O desconto tem teto de 7 dias, o mesmo da fila do cliente. É contra corpo forjado: um
`atraso_s` gigante escreveria `last_seen` no passado remoto e o documento sumiria de todas
as janelas de leitura sem nunca ter sido lido.

**Desligar a telemetria apaga a fila**, junto com a corrida aberta. Um evento pendente é
uma fala já engatilhada; guardá-lo seria continuar falando pelas costas de quem acabou de
pedir silêncio.

## Como o servidor guarda

| Coleção | Doc | Serve para |
|---|---|---|
| `races` | `{install_id}__{subsession_id}` | uma corrida |
| `installs` | `{install_id}` | perfil corrente: onde esse jogador está hoje |
| `telemetry_daily` | `YYYY-MM-DD` (UTC) | contadores do dia |

**O `install_id` entra na chave da corrida de propósito.** Dois jogadores do Loop podem
estar na *mesma* subsessão do iRacing; com a chave só pelo `subsession_id`, um
sobrescreveria a corrida do outro e a contagem de simultâneas ficaria menor que a
realidade.

Três decisões que blindam a contagem:

- `race_start` é **upsert**, não insert. Restart de corrida reabre a mesma subsessão em
  vez de virar duas, preserva o `started_at` original e só incrementa `restarts`. Assim
  não é preciso modelar a semântica de restart do iRacing corretamente.
- `race_end` é **idempotente**. Bandeirada e queda de conexão podem chegar as duas; o
  segundo `end` não conta de novo nem soma o tempo de pista duas vezes.
- `race_ping` de corrida que o servidor nunca viu começar **reconstrói** a corrida a
  partir do `elapsed_s` (marcada com `recovered_from_ping`). Perder um `race_start`
  (Cloud Run frio, rede caída) não pode apagar a corrida inteira do painel.

### O lado do servidor (feito em 10/08, falta o deploy)

O `index.js` do `iracer-news-server` já aceita os seis eventos. Ele **não é versionado**,
mora em `OneDrive\Área de Trabalho\Jogos\iracer-news-server`, e o que foi feito lá:

- Os três eventos novos na validação, sem exigir `subsession_id` (que só existe onde
  houve sessão de iRacing).
- `session_id` gravado em todo doc, mais a coleção `sessions`
  (`{install_id}__{session_id}`) com abertura, último evento e contagem de corridas.
- Coleção **`weekend_usage`** (`{install_id}__{temporada}__{rodada}`), somada por upsert.
  O nome não é `usage`: essa coleção já existe e guarda o **custo de IA** (docs `totals`,
  `YYYY-MM-DD`, `{install}:{mês}`, lidos pelo `/usage`). Duas semânticas na mesma coleção
  passariam despercebidas até a primeira consulta somar laranja com banana.
- Acumuladores em `installs`: `total_races`, `total_sims`, `total_sessoes`.
- `by_ano` → `by_ano_carreira` no `/summary` e no `/live`.

**Restart não conta corrida nova** em lugar nenhum: nem no contador diário, nem na
sessão, nem no total da instalação. Contar largadas faria "corridas por sessão" subir por
motivo errado. No sentido oposto, um `race_end` de corrida que o servidor nunca viu
começar **conta** na sessão: senão o número ficaria menor que a realidade justo nas
máquinas com rede ruim, que são as que mais perdem evento.

Há um teste de ponta a ponta em `teste-local/`, que sobe o `index.js` real contra um
Firestore falso em memória. Vinte e sete checagens, sem credencial e sem rede:

```bash
cd teste-local && npm install && npm test
```

### `GET /telemetry/report?days=30&min_races=3` — o dump para a IA ler

Implementado. O ponto deste endpoint é ser **lido por um modelo**, e não por um humano com
paciência: devolve um JSON único, já agregado, que cabe inteiro num prompt. Nada de
paginação, nada de evento cru, nenhum `install_id` — o que sai daqui é população, não
pessoa.

```json
{
  "janela_dias": 30,
  "instalacoes": { "total": 0, "ativas_7d": 0, "so_simulam": 0, "so_correm": 0, "ambos": 0 },
  "sessoes": {
    "total": 0, "por_instalacao_media": 0.0,
    "corridas_por_sessao_media": 0.0, "duracao_min_mediana": 0.0,
    "por_dia_da_semana": {}
  },
  "progressao": { "por_ano_carreira": {}, "por_categoria": {}, "por_dificuldade": {} },
  "corridas": {
    "dirigidas": 0, "simuladas": 0, "dnf_pct": 0.0,
    "por_pista": [
      { "track_id": 0, "carro": "", "n": 0,
        "melhor_volta_mediana_s": 0.0, "deficit_ritmo_pct": 0.0,
        "posicao_final_mediana": 0, "percentil_grid_medio": 0.0 }
    ]
  },
  "leitura": {
    "por_rodada_media": { "noticias_s": 0, "briefing_s": 0, "debriefing_s": 0 },
    "rodadas_sem_leitura_pct": 0.0,
    "ptt": { "apertos": 0, "perguntas": 0, "perguntas_por_corrida": 0.0 }
  }
}
```

O bloco `ia` é o par do `leitura`, e é o que fecha a conta de custo. Ele vem do
`usage_by_install`, que o servidor já escrevia por (instalação, mês) e agora também
quebra **por endpoint** (`calls_race_story`, `cost_race_story`, e assim por diante nos
seis escopos de geração). A chamada diz quanto se gastou; o tempo de tela diz se alguém
leu. Uma prévia de temporada que é 40% da fatura e some da tela em oito segundos aparece
sozinha no cruzamento.

Duas notas sobre esse bloco. Ele é do **mês corrente**, e não da janela em dias do resto
do relatório, porque o acumulado sempre foi mensal; o campo `ia.mes` diz qual, para
ninguém somar as duas janelas mentalmente. E ele traz média **e** mediana: com poucos
usuários, um jogador que deixou o app aberto a noite inteira leva a média sozinho, e só a
mediana mostra o custo de quem é típico.

Três regras que fazem esse relatório valer alguma coisa:

- **Só `finished` entra em mediana de volta e de posição.** Um abandono na volta 2 entra
  na estatística como último lugar e envenena a média.
- **Grupo com menos de 3 corridas fica de fora do `por_pista`**, ou uma corrida azarada
  vira "essa pista está difícil".
- **Todo número vem com o `n` do grupo.** Um modelo que recebe percentual sem tamanho de
  amostra escreve com a mesma confiança sobre 3 corridas e sobre 300, e é assim que um
  relatório automático produz decisão errada.

## Leitura

Ambas protegidas pelo mesmo segredo (header `x-app-secret` ou `?secret=`, por
comodidade no navegador).

- `GET /telemetry/live` — **quantas rolando agora**: `racing_now`, quebrado por
  categoria, ano e pista, mais as 50 corridas mais antigas em andamento. Ativa = `start`
  sem `end` **e** vista há menos de 35 min.
- `GET /telemetry/difficulty?days=30&min_races=3` — **a dificuldade está calibrada?**
  Agrupa as corridas encerradas por (`dificuldade`, `categoria`) e devolve, por grupo:
  `deficit_ritmo_pct` (mediana de `melhor_volta / melhor_volta_classe - 1`, o sinal
  principal), `percentil_grid` (posição normalizada pelo tamanho da classe),
  `posicoes_ganhas` (média de `grid - final`), `incidentes` (mediana) e `dnf_pct`. Só
  `finished` entra nas medianas; o `min_races` corta grupo pequeno demais, onde uma
  corrida azarada viraria "dificuldade errada". Ordenado pelo pior déficit.
- `GET /telemetry/summary?days=30` — **onde cada jogador está**: instalações ativas
  (24h / 7d / janela), distribuição por `categoria`, `ano`, `temporadas_completas`,
  versão e SO, mais os contadores diários da janela.

A consulta de `live` filtra **só** por `last_seen` e descarta as encerradas em memória.
É de propósito: `status == "active"` + range em `last_seen` exigiria índice composto no
Firestore, e o conjunto aqui é pequeno por definição (só o que se mexeu na última meia
hora).

## Operação

- `TELEMETRY_STALE_SECS` (2100 = 35 min) — janela do "rolando agora".
- `TELEMETRY_DAILY_CAP` (50000) — freio caso o `APP_SECRET` vaze. Não é cota de custo:
  telemetria não chama IA.
- `TELEMETRY_RETENTION_DAYS` (90) — só preenche o campo `expire_at`. **Quem apaga de
  fato é a política de TTL do Firestore**, uma por coleção. Quatro gravam `expire_at`:
  `races`, `sessions`, `weekend_usage` e `logs` (esta com `LOG_RETENTION_DAYS`). Ligar
  pela linha de comando, sem precisar do Console:

  ```bash
  gcloud firestore fields ttls update expire_at --collection-group=sessions --enable-ttl --project loop-news-500215
  ```

  Coleção nova que grave `expire_at` precisa da sua própria política: a do `races` não
  vale para as outras, e o campo sozinho não apaga nada.

## Consentimento

`AppConfig.telemetry_enabled: Option<bool>`, onde `None` (nunca perguntado) é distinto
de `Some(false)` (recusou) — é essa diferença que faz o aviso de primeira execução
aparecer. `None` é tratado como **desligado**: nada é enviado até o jogador dizer sim.

O aviso é o `components/system/TelemetryConsentGate.jsx`, montado no `App` junto dos
outros modais de sistema. Três decisões que valem registro:

- **Pergunta depois da primeira corrida**, dirigida no iRacing ou simulada, e não na
  primeira abertura. No dia 1 o jogador não sabe o que é o Loop; a pergunta chega a um
  desconhecido e a resposta é chute. **Reabertura de corrida antiga pela Home não conta**:
  ela não gera evento nenhum, então perguntar ali é interromper por nada. Três caminhos
  abrem a tela de resultado (`simulate_race_weekend`, `iracing_auto_import_if_ready`,
  `get_saved_race_screen`) e os dois primeiros contam — quem os separa é o campo
  `lastRaceOrigem` do `useCareerStore` (`"simulada"` \| `"iracing"` \| `"reaberta"`),
  marcado explicitamente por cada um. Dá para tentar inferir pela telemetria da corrida,
  mas ela pode vir vazia numa corrida real, então a marcação é explícita.

  A simulada entrou nesta lista junto com o `race_sim`, e a mudança é indissociável dele:
  o campo era um booleano `lastRaceFromIracing`, e virou três valores porque passou a
  haver diferença entre "não correu no iRacing mas gerou evento" e "não gerou nada".
  Enquanto a pergunta não alcançava quem só simula, o evento novo não mediria ninguém —
  essa população nunca era perguntada, e portanto nunca consentia.
- **Abre quando ele FECHA a tela de resultado**, não quando ela aparece. O resultado é a
  recompensa da corrida; cobrir isso com uma caixa de diálogo seria pior do que ter
  perguntado no boot.
- **Não fecha por clique no fundo nem tem "X".** Fechar sem responder deixaria o config
  em `None` e traria o aviso de volta na próxima corrida — insistir é pior que perguntar
  uma vez. Os dois botões custam o mesmo clique, então recusar é tão fácil quanto
  aceitar.

O config é lido **no momento do gatilho**, não no boot: assim quem já respondeu pelas
Configurações antes de correr não é perguntado de novo.

O toggle reversível fica em **Configurações › Geral** ("Ajudar a melhorar o Loop").

Desligar nas Configurações vale a quente (`update_config` → `telemetry::set_enabled`) e
fecha a corrida aberta **em silêncio**, sem mandar o `race_end`: o jogador pediu para
parar de falar, e isso inclui o evento de despedida. O servidor expira a órfã sozinho.

## O painel de custo contava execução de teste como usuário

Medido em 10/08/2026, ao expor o custo de IA no relatório: **1.275 "instalações" em julho
e 2.731 nos dez primeiros dias de agosto**, quase todas com uma única chamada. O Loop não
tem essa base. Eram execuções da suíte de testes.

O caminho: `simulate_race_weekend_in_base_dir` dispara o pré-aquecimento do boletim
(`spawn_prewarm_boletim`), que chama o servidor de verdade. Nos testes o `base_dir` é uma
pasta temporária nova a cada execução, então cada `cargo test` nascia com um `config.json`
novo, um `install_id` novo e virava um punhado de "instalações" no Firestore. Com custo
real nos dois provedores: US$ 2,03 somando os dois meses.

O conserto é o `rede_de_ia_bloqueada()` em [narrative/client.rs](src-tauri/src/narrative/client.rs),
no topo das cinco funções de geração. O guard fica na **saída de rede**, e não em cada
chamador, para que um caminho novo ao servidor já nasça coberto:

- `cfg!(test)` cobre a suíte do crate.
- `LOOP_SEM_IA=1` cobre o resto (harness de simulação, benchmark, agente rodando o app
  em lote).

O que isso implica para quem for ler o painel: **os números de julho e agosto de 2026 em
`usage_by_install` e `/usage/by-install` não valem**. A contagem de usuários só passa a
significar alguma coisa depois desta data, e mesmo assim a fonte honesta de "quantas
pessoas" é a coleção `installs` da telemetria, que é opt-in e nasce de gente que abriu o
jogo — não `usage_by_install`, que nasce de qualquer coisa que chame o servidor.

## O que falta

- **Distribuir uma build nova do Loop.** O servidor já aceita tudo (revisão
  `iracer-news-00060-vs7`, no ar desde 10/08/2026, sondada em produção com os seis
  eventos), mas nenhum jogador tem o cliente que os manda.
- Conferir de vez em quando se toda coleção que grava `expire_at` tem política de TTL
  (ver "Operação"). O campo sozinho não apaga nada, e a falta só aparece na conta do
  Firestore meses depois.
