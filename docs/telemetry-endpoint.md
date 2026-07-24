# Endpoint `/telemetry` — ciclo de vida das corridas (produto)

O save local responde tudo sobre **um** jogador e nada sobre o conjunto. Esta é a
telemetria que responde as perguntas de produto: **quantas corridas estão rolando
agora**, **em que temporada e categoria cada instalação está** e quanto tempo de pista
roda por dia.

Anônima e opt-out: a chave é o `install_id` (UUID da máquina, sem vínculo com e-mail ou
conta). Nome de piloto, nome de equipe e conteúdo de save **nunca** saem do app.

**Limite que vale repetir:** isto só enxerga quem abre o iRacing. Quem simula a
temporada dentro do app continua invisível aqui.

## As duas metades

- **Cliente** — `src-tauri/src/telemetry.rs`, pendurado em bordas que o `race_monitor`
  já detectava para outros fins. Nada é inventado, nada bloqueia: cada envio sai em
  thread própria, timeout de 5s, todo erro engolido.
- **Servidor** — `index.js` do repo `iracer-news-server` (Cloud Run, mesmo serviço de
  `/race-story` e `/pre-race`). **Esse repo não é versionado**; este documento é o
  registro do contrato.

| Evento | Borda no cliente | Onde |
|---|---|---|
| `race_start` | largada verde (`prev_session_state < RACING` → `>= RACING`) | `race_monitor.rs:2441` |
| `race_ping` | a cada 30 min de corrida aberta | `race_monitor.rs:3323` |
| `race_end` | `finalize_attempt()` (bandeirada/DNF) | `race_monitor.rs:1599` |
| `race_end` | queda da conexão com o sim (`sim_closed`) | `race_monitor.rs:3346` |

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
  "event": "race_start | race_ping | race_end",
  "install_id": "<uuid>",
  "subsession_id": 12345678,
  "app_version": "0.9.1",
  "os": "windows"
}
```

Mais, quando há carreira carregada (anexado por `set_career_context()`, chamado em
`load_career_in_base_dir` — `commands/career.rs`): `ano`, `categoria`, `dificuldade`,
`temporadas_completas`, `corridas_totais`. A **dificuldade viaja em todo evento**, não só
no fim de corrida: é o eixo pelo qual o desfecho é lido — posição e ritmo só calibram a
curva se você souber em que nível aquela corrida foi disputada.

Específicos por evento:

| Evento | Campos |
|---|---|
| `race_start` | `track_id` |
| `race_ping` | `elapsed_s` |
| `race_end` | `track_id`, `duracao_s`, `status` + o desfecho (abaixo) |

### O desfecho, no `race_end`

Montado pelo `build_race_outcome()` a partir do que o `race_monitor` já acumulou para o
painel pós-corrida — nenhuma amostragem nova, nenhum custo por tick.

| Campo | Nota |
|---|---|
| `posicao_final`, `posicao_grid`, `carros_na_classe` | Andam **juntos**. Quinto entre 8 não é quinto entre 24, e quinto partindo de décimo segundo não é quinto partindo da pole. Sozinha, a posição final produz uma média que mistura tudo. |
| `melhor_volta_s`, `melhor_volta_classe_s` | A razão entre as duas é o **déficit de ritmo** — o melhor sinal de dificuldade, imune a tamanho de grid, incidente e abandono. A volta do jogador sozinha não é comparável nem com ele mesmo em outra pista. |
| `carro` | Junto com `track_id`, fecha a chave de comparação de volta. 1:35 é rápido num carro e lento noutro. |
| `voltas`, `incidentes`, `restarts` | `incidentes` é o **único** onde `0` é valor legítimo (e o mais interessante deles). |
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

**Resposta** `200 {"ok": true}`. O cliente ignora o corpo (dispara e esquece); os
códigos existem para o log. `202 {"dropped":"daily_cap"}` quando o teto diário estoura,
`400` para evento malformado, `401` sem o segredo.

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
  fato é a política de TTL do Firestore**, que precisa ser ligada à mão no Console
  (Firestore → TTL, campo `expire_at`, coleção `races`). Sem isso, nada é apagado.

## Consentimento

`AppConfig.telemetry_enabled: Option<bool>`, onde `None` (nunca perguntado) é distinto
de `Some(false)` (recusou) — é essa diferença que faz o aviso de primeira execução
aparecer. `None` é tratado como **desligado**: nada é enviado até o jogador dizer sim.

O aviso é o `components/system/TelemetryConsentGate.jsx`, montado no `App` junto dos
outros modais de sistema. Três decisões que valem registro:

- **Pergunta depois da primeira corrida DIRIGIDA no iRacing**, não na primeira abertura.
  No dia 1 o jogador não sabe o que é o Loop; a pergunta chega a um desconhecido e a
  resposta é chute. Corrida **simulada não conta**, nem reabertura de corrida antiga
  pela Home: quem nunca pisou na pista não gera evento nenhum, então perguntar a ele é
  interromper por nada. Três caminhos abrem a tela de resultado (`simulate_race_weekend`,
  `iracing_auto_import_if_ready`, `get_saved_race_screen`) e só o do meio conta — quem os
  separa é o campo `lastRaceFromIracing` do `useCareerStore`, marcado explicitamente por
  cada um. Dá para tentar inferir pela telemetria da corrida, mas ela pode vir vazia numa
  corrida real, então a marcação é explícita.
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

## O que falta

- Ligar a política de TTL do Firestore no Console (sem ela, `expire_at` não apaga nada).
