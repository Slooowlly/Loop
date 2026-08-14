# Escopo da integração com iRacing — o que existe, o que é alcançável, o que decidir

Levantado em 2026-07-27 na branch `main-menu-redesign`, rastreando a partir da lista de
`generate_handler!` do [lib.rs](../src-tauri/src/lib.rs) — **não** por nome de módulo — e
cruzando cada comando com os `invoke` do frontend e com as telas realmente montadas.

Fecha o item **F-10** do [roadmap.md](roadmap.md) §8.

---

## 1. A contradição, resolvida

O [DESIGN.md](DESIGN.md) §23 do retrato de junho (hoje §19, reescrito) dizia que export e watchdog foram removidos e que a integração
era "expansão futura". O roadmap §8 concluiu que a direção havia se invertido — de
*exportar para o iRacing* para *ler o iRacing*.

**As duas leituras estavam erradas.** O que aconteceu foi:

- Os módulos `export/` e `commands/export.rs` foram **mesmo** deletados;
- Mas a exportação **não** morreu: reapareceu dentro do `iracing_sdk/`, como
  [`roster_gen.rs`](../src-tauri/src/iracing_sdk/roster_gen.rs) (gera o AI roster em
  `Documentos/iRacing/airosters/`) e
  [`season_gen.rs`](../src-tauri/src/iracing_sdk/season_gen.rs) (gera a AI season em
  `Documentos/iRacing/aiseasons/`);
- E ganhou uma volta: a leitura do resultado oficial e a importação para a carreira.

A integração de hoje é um **ciclo fechado**, não um mão-única:

```
carreira → exporta roster + temporada → jogador corre no iRacing
        → resultado oficial (JSON do aiseason) + sinais do monitor ao vivo
        → importa para a carreira → tela de resultado, telemetria, rivalidade, quebras
```

Esta é a lição para o próximo leitor: **nome de arquivo mente, e "módulo deletado" não
significa "feature removida"** — significa que ela mudou de casa.

---

## 2. O que o Loop faz com o iRacing hoje, de verdade

Só o que é alcançável pelo jogador numa tela montada. São **57 comandos registrados** sob
`commands::iracing::` (contados em 11/08/2026 no `generate_handler!`, os três `radio_*` incluídos);
os que sustentam o produto são estes.

### 2.1 Antes da corrida — Sala de Estratégia (`NextRaceTab`)

[`useIracingExport.js`](../src/components/race/nextrace/useIracingExport.js) faz, num
botão só: `iracing_generate_roster` → `iracing_generate_season` →
`iracing_install_yellow_macro` → `iracing_modo_janela_aplicar` (os dois últimos
best-effort, aproveitando que o sim está fechado e a escrita nos `.ini` "cola"; o modo
janela também é aplicado no boot do Loop, sem perguntar nada). Depois oferece
`iracing_focus_window` e, se o sim estiver fechado,
`iracing_launch_ui` para o jogador cair direto no iRacing.

O roster carrega a identidade da carreira: atributos do piloto viram `driverSkill` /
`driverAggression` / `driverOptimism` / `driverSmoothness`, o time vira `pitCrewSkill` /
`strategyRiskiness`, e cor/padrão de carro, macacão e capacete saem da paleta do time.
A temporada carrega o calendário com **clima por evento** (keyframes dinâmicos, versão 3).

**O export não atende ao catálogo inteiro, e isso é deliberado desde 11/08/2026.** Quem decide
é [`commands/iracing/exportavel.rs`](../src-tauri/src/commands/iracing/exportavel.rs), fonte
única: o frontend manda a CATEGORIA e não adivinha carro nenhum.

| Categoria | Export |
|---|---|
| `mazda_rookie`, `mazda_amador` | ✅ `mx5` |
| `toyota_rookie`, `toyota_amador` | ✅ `gr86` |
| `bmw_m2` | ✅ `bmwm2` |
| `gt4`, `gt3`, `lmp2` | ⛔ carro pago, substituto não autorizado |
| `production_challenger`, `endurance` | ⛔ grid de mais de uma classe, e o aiseason declara um `car_id` por temporada |

A recusa chega ao jogador com o motivo e o convite a correr a etapa pela simulação do Loop.
Antes disso as cinco recusadas caíam num `else → mx5`: o Loop gravava um AI roster e uma AI
season de Mazda MX-5 dentro da instalação do jogador, sem erro nenhum, e ele só descobria
dentro do simulador. Escolher carro pago, substituto ou "categoria só simulada" para essas
cinco é decisão de produto ainda em aberto.

A duração segue a mesma porta: `race_length_da_temporada` reduz as durações efetivas das
etapas ao único `race_length` do aiseason e recusa quando elas divergem, em vez de achatar. A
sentinela `duracao_corrida_min: 0` do `endurance` morre na cascata `duracao_efetiva` e não
alcança o arquivo.

Ainda aqui: `iracing_auto_paint_player`, que pinta o carro do jogador na cor da equipe
junto com a exportação e vincula o custid ao save. Roda sem perguntar nada, porque o
`car_<custid>.tga` é local (só ele vê essa cor) e a pintura anterior é preservada em
`.tga.loop-bak`. O interruptor mora em Configurações (`auto_paint_car`), e não depende do
Trading Paints — que escreve no mesmo arquivo, ao entrar na sessão, ou seja, depois de nós.

Briefing da corrida: `get_breakdown_forecast` e `get_grid_breakdown_risk`
([`useBriefingData.js`](../src/components/race/nextrace/useBriefingData.js) e o
`preRaceCacheSlice`).

### 2.2 Durante — sampler de fundo + overlay

O `lib.rs` sobe no setup `race_capture::init` e `race_monitor::start_watching()`: um
amostrador a ~60 Hz que consome `read_telemetry`/`read_session` **por dentro** (não pela
ponte IPC) e alimenta o monitor unificado — tentativas, batidas, DNF, quebras de peça,
incidentes, estilo de pilotagem.

[`IracingConnectedOverlay`](../src/components/iracing/IracingConnectedOverlay.jsx) está
montado no `MainLayout` (ou seja: sempre): `iracing_connected` (1 Hz),
`iracing_get_race_feedback` (versão enxuta do histórico) e `iracing_car_colors`. Dentro
dele, o `StartingCompoundsPanel`.

Race control automático: `iracing_set_auto_yellow` / `iracing_auto_yellow_enabled`
(Settings) ligam o disparo automático de bandeira amarela — o monitor chama
`race_control::throw_yellow()` sozinho quando detecta o caso. O overlay de quebras
consulta `iracing_chat_blocked`.

### 2.3 Depois — importação automática

`Dashboard` chama `iracing_focus_self_if_closed` em loop: quando o sim fecha, a janela do
Loop volta para frente sozinha. No mesmo ritmo, o
[`raceSlice`](../src/stores/career/raceSlice.js) chama `iracing_auto_import_if_ready`,
que é o coração da volta:

- lê o **resultado oficial** do JSON do aiseason (não reconstrói ao vivo);
- confere que a pista bate com a que foi exportada;
- sobrepõe o que o iRacing não sabe e o monitor sabe: batida do jogador (severidade →
  custo de conserto), DNF real de quem saiu, direção do impacto, estilo de pilotagem,
  quebras de peça, quem bateu em quem;
- persiste na carreira e devolve resultado + resumo para a tela abrir sozinha;
- aplica a **percepção de rivalidade de pista** — inclusive IA-vs-IA, alimentando o
  ledger de rivalidade do grid inteiro, não só ao redor do jogador.

Na tela de resultado: `iracing_car_colors` colore as linhas de `RaceCharts` e do
`RaceTelemetryCockpit`; `get_race_weather_timeline` desenha o clima.

### 2.4 Entre temporadas e manutenção

`iracing_apply_market_paint` (`PreSeasonView`) reaplica a pintura quando o piloto troca de
time. Em Settings: `IracingDiagnosticoPanel` (`iracing_diagnostico`, `iracing_log_ler`,
`iracing_log_revelar`, `iracing_log_enviar`), `RivalryPerceptionPanel`
(`iracing_save_race_history` / `iracing_list_saved_races` / `iracing_load_saved_race` /
`iracing_get_race_history` / `iracing_perceive_rivalries`), o chat de texto livre
(`iracing_send_chat_text`) e os armadores de quebra de teste
(`iracing_arm_test_breakdown`, `iracing_arm_test_breakdown_grid`).

### 2.5 Ressalva de plataforma

Tudo acima é Windows. Fora do Windows o `imp/stub.rs` compila no lugar do winapi e a
integração é inerte por construção — **"não funciona no Linux" não é bug**, é o desenho.

---

## 3. Os comandos sem consumidor nenhum

> **Refeito em 11/08/2026 pela fonte oficial.** A tabela original era uma varredura manual de
> 9 comandos, e a contagem manual não sobrevive a um commit. O inventário agora vive congelado em
> `SEM_CONSUMIDOR_CONHECIDO`, no guard
> [`invoke-contra-generate-handler`](../scripts/tests/invoke-contra-generate-handler.test.mjs), que
> quebra quando a lista MUDA — em qualquer direção. **A lista dele é o número; esta seção é a
> leitura dela pela ótica do iRacing.** Para conferir hoje:
>
> ```bash
> node --test scripts/tests/invoke-contra-generate-handler.test.mjs
> ```

Do inventário congelado, **13 são do módulo `commands::iracing`** (incluídos os dois `radio_log_*`,
que moram lá). Nenhum deles é chamado pelo frontend; não há `invoke` com nome montado em variável
fora do `OverlayPositionPanel`, e o guard já conta as strings soltas que casem com um comando
registrado, então "órfão" aqui é órfão de verdade.

| comando | classificação no guard | por quê |
|---|---|---|
| `iracing_process_race_result` | **API interna** | é o único da lista chamado de dentro do próprio Rust: a dificuldade adaptativa, em [`importacao.rs:138`](../src-tauri/src/commands/iracing/importacao.rs). O *registro* é que sobra; a lógica roda. Ver §6.1. |
| `iracing_read_telemetry` | **diagnóstico e bancada** | a função é consumida pelo amostrador (`race_monitor/amostrador.rs:18`), por dentro do crate. O comando é andaime. |
| `iracing_read_session` | **diagnóstico e bancada** | idem (`amostrador.rs:31`), além de `dump_session_yaml` e `player_custid`. |
| `iracing_poll_race` | **diagnóstico e bancada** | `race_monitor::poll()` é usado por dentro; só a exposição IPC sobra. |
| `iracing_estado_agora` | **diagnóstico e bancada** | casca de `race_monitor::estado_agora()` (`sessao.rs:62`); a torre e o overlay leem o estado por outro caminho. |
| `iracing_reset_race` | **diagnóstico e bancada** | zerar o monitor entre testes. `race_monitor::reset()` não tem outro chamador — sem botão, a função é inalcançável. |
| `iracing_log_caminho` | **diagnóstico e bancada** | `iracing_log_revelar` já entrega o caso de uso (abrir a pasta) e é o que a UI usa. |
| `iracing_send_chat_macro` | **diagnóstico e bancada** | o doc-comment diz "teste cru — descobrir o slot certo". Superado por `iracing_send_chat_text`, ligado em Configurações. |
| `iracing_throw_yellow` | **diagnóstico e bancada** | bandeira amarela **manual**. O automático já está ligado; falta o botão. |
| `iracing_spotter_restore` | **diagnóstico e bancada** | devolve o spotter nativo do iRacing (`spotter_control::devolver()`). O interruptor da UI é o `useSpotterNativo`, que usa `iracing_spotter_status` e `iracing_spotter_set`; a restauração explícita não tem porta. |
| `iracing_career_race_result` | **feature futura (§6)** | reconstrói o resultado a partir do monitor ao vivo, em paralelo ao `build_session_race_result`, que lê o resultado **oficial** do aiseason. ⚠️ **Divergência não resolvida:** esta seção o classificava como *morto* e a §6 manda removê-lo; o guard o rotula "feature futura, ver §6". O rótulo do guard aponta para um item que pede o corte. Decidir é do dono — ver a ressalva no fim da §6.1. |
| `radio_log_caminho` | **feature futura** | o caminho do log de rádio existe; o botão nas Configurações não. |
| `radio_log_revelar` | **feature futura** | idem, para o "revelar na pasta". |

**Saíram da lista em 11/08/2026:** `iracing_desfazer_pinturas`, `iracing_modo_janela_status` e
`iracing_modo_janela_restaurar`. Eram o item "desfazer sem botão ainda" — o Loop escreve em dois
arquivos que não são dele (a pintura `paint/<carro>/car_<custid>.tga` e a configuração gráfica
`rendererDX11*.ini`) e fazia as duas coisas sem caminho de volta. A tela existe agora:
`src/components/iracing/IracingDesfazerPanel.jsx`, montada nas Configurações
(`Settings.jsx:362`) logo abaixo do interruptor `auto_paint_car`.

**Saiu da lista em 2026-07-27:** `iracing_restore_yellow_macro`, por decisão do dono — restaurar a
macro não tem valor de produto. O original continua salvo no estado e há backup completo do
`app.ini`; o iRacing reescreve o arquivo ao fechar. Comando, registro no `lib.rs` e
`race_control::restore()` removidos.

---

## 4. O segundo achado: dois painéis inteiros desligados

O briefing pedia os órfãos por `grep` de comando. Ao subir um nível — *a tela que chama o
comando está montada?* — aparece uma camada maior:

- [`RosterGenPanel.jsx`](../src/components/iracing/RosterGenPanel.jsx) — **726 linhas**
- [`PostRacePanel.jsx`](../src/components/iracing/PostRacePanel.jsx) — **696 linhas**

Nenhum dos dois é importado em lugar nenhum de `src/` — só pelos próprios testes de contrato
(`RosterGenPanel.test.jsx`, `PostRacePanel.test.jsx`, escritos em 11/08/2026). São componentes
exportados e nunca montados. Com isso, **6 comandos** ficam inalcançáveis pelo jogador embora
tenham "consumidor" — recontados em 11/08/2026 cruzando os `invoke` dos dois painéis contra o resto
de `src/`:

`iracing_dump_session_yaml`, `iracing_preview_race_result`, `iracing_apply_player_paint`,
`iracing_player_custid`, `iracing_player_paint`, `iracing_export_rain_test`.

Os outros comandos que os painéis chamam têm consumidor vivo em outro lugar
(`iracing_generate_roster`, `iracing_generate_season`, `iracing_connected`,
`iracing_get_race_history`, `iracing_save_race_history`, `iracing_list_saved_races`,
`iracing_load_saved_race`, `list_saves`) e não contam aqui.

**Eram 7, e o sétimo saiu da lista.** `iracing_process_race_result` é a dificuldade adaptativa
(`adaptativo.rs`) — atualiza o perfil do jogador por `custid` depois de cada corrida limpa, global e
por pista. Ele só era chamado pelo `PostRacePanel`. Como o `ai_sweet_spot` **lê** esse perfil para
ancorar a curva de skill da IA na geração de roster/temporada, o efeito prático era: o perfil nunca
era escrito, a leitura devolvia sempre zero, e a IA nunca calibrava ao jogador. Não era comando
órfão; era feature paga e não entregue.

**Corrigido em 2026-07-27**: a chamada saiu da UI e entrou no import automático
(`importacao.rs`), best-effort, logo após o import bem-sucedido — que já é idempotente por
corrida. Deixou de depender de painel montado. Hoje ele aparece na §3 como **API interna**, e o que
falta é medição, não código (§6.1).

**Total, recontado em 11/08/2026: 19 dos 57 comandos iRacing (33%) não são alcançáveis por nenhuma
tela viva** — os 13 órfãos da §3 mais estes 6. Um deles, o `iracing_process_race_result`, roda de
dentro do Rust; os outros 18 não rodam por caminho nenhum. A coincidência com os 33% da varredura de
27/07 é acidente aritmético: eram 16 de 49, e mudaram as duas pontas.

---

## 5. A decisão de produto

> **O Loop é um simulador de carreira offline que opcionalmente lê o iRacing, ou é uma
> ferramenta de acompanhamento do iRacing que tem uma carreira simulada dentro?**

**Decidido em 2026-07-27 pelo dono do projeto: a segunda.** O Loop é uma **ferramenta de
iRacing com uma carreira simulada dentro**. Correr de verdade é o caminho principal; a
simulação interna preenche o que o jogador não corre.

Os números que sustentavam a decisão, em 27/07/2026: 49 de 158 comandos (31% da ponte IPC) e
16.910 linhas em `iracing_sdk/`. **Recontados em 11/08/2026: 57 de 201 comandos (28% da ponte) e
33.101 linhas em 98 arquivos de `iracing_sdk/`, mais 5.738 linhas em `commands/iracing/`.** A ponte
cresceu mais rápido que a fatia iRacing em número de comandos e o dobro dela em linhas — o peso
relativo caiu três pontos, e o absoluto dobrou. A decisão apenas alinhou o documento ao peso real
do código, e o peso real só aumentou desde então.

### O que decorre disso

1. **Os órfãos não são candidatos a corte por padrão.** As features pendentes da §3 e os dois
   painéis da §4 viram backlog de UI, não lixo. O corte que esta decisão considerou limpo eram os
   dois então classificados como **mortos** (`send_chat_macro`, `career_race_result`) e as
   exposições IPC **internas** — que sairiam do `invoke_handler` e continuariam como função do
   crate. ⚠️ Nenhum desses cortes foi executado, e o guard de 11/08 reclassificou os dois "mortos"
   (ver a nota de divergência na §3). **A decisão sobre o destino de cada órfão continua aberta e é
   do dono**; este documento registra o inventário, não o veredito.
2. **Prioridade nº 1: ligar a dificuldade adaptativa.** É a maior distância entre "pronto"
   e "entregue" no repositório inteiro.
3. **O app deixa de se descrever como "offline".** `CLAUDE.md` e `DESIGN.md` foram
   ajustados: offline continua verdade sobre *dados* (nada de servidor, tudo em SQLite
   local), mas não sobre *propósito*.
4. **Windows deixa de ser detalhe de implementação e vira requisito de produto.** O
   `stub.rs` é compatibilidade de compilação, não uma promessa de plataforma.

---

## 6. Trabalho derivado (não executado nesta sessão)

Duas ações foram executadas em 2026-07-27, por decisão do dono do projeto; o resto fica
registrado.

**Feito:**

| item | o que mudou |
|---|---|
| Dificuldade adaptativa ligada | `iracing_process_race_result` passa a ser chamado de dentro de `iracing_auto_import_if_ready` (`importacao.rs`), best-effort e em silêncio. Sai da dependência de painel montado. |
| `iracing_restore_yellow_macro` removido | comando, registro no `lib.rs` e `race_control::restore()`. O backup do `app.ini` e o valor original no estado continuam existindo — a reversão é manual e assumida. |

**Pendente:**

| # | item | tamanho |
|---|---|---|
| 1 | Dar destino ao `PostRacePanel` (696 linhas prontas) — montar ou aposentar | M |
| 2 | Dar destino ao `RosterGenPanel` (726 linhas) — é painel de debug; decidir se vira tela de Settings | M |
| 3 | Bandeira amarela manual (`iracing_throw_yellow`) + reset do monitor (`iracing_reset_race`) no diagnóstico | S |
| 4 | Remover `iracing_send_chat_macro` e `iracing_career_race_result`; tirar da ponte `read_session`, `read_telemetry`, `poll_race`, `log_caminho` | S |

O item 4 é o único que mexe em `lib.rs` — e exige `npm run build` **antes** de
`cargo test`, porque `generate_context!` embute o `dist/`.

### 6.1 Conferência de 2026-08-11

A vistoria de 10/08 voltou a marcar os inalcançáveis como pendência (A14.2). Conferido
contra o código de 11/08, **nada mudou desde 27/07** nos quatro itens acima, que dependem de
decisão. Mudou em volta: os três comandos de desfazer ganharam tela (§3), e a conta de 16 de 49 virou
**19 de 57** (§4). O que a conferência acrescenta:

- **A dificuldade adaptativa está LIGADA (wiring), e ligar não é RODAR (execução).** São duas
  perguntas diferentes, e cada uma tem sua prova:

  **Wiring** — resolvido, e é leitura de código. A chamada existe em
  [`importacao.rs:138`](../src-tauri/src/commands/iracing/importacao.rs), dentro de
  `iracing_auto_import_if_ready`, depois do import bem-sucedido. Nada mais depende de painel
  montado.

  **Execução** — em aberto até uma corrida real, e é leitura de log. Até 10/08/2026 o caminho
  nunca havia sido percorrido uma vez sequer, porque ele depende do auto-import fechar. **A receita
  é o par de linhas no `loop.log`** (`%APPDATA%\com.loop.app\logs\`):

  ```
  [import]      Corrida importada: <race_id> (pista <track_id>)
  [adaptativo]  Pista <id> · classe <c>: N IA de M carros · carro sim|não ·
                ritmo vs frente +0,42%/volta · <veredito> ·
                global 0+1=1 · pista 0+1=1 · gravado
  ```

  A segunda linha é o log explícito de sucesso, e ela sai **sempre** que o adaptativo roda — o
  sufixo é `gravado` quando a agulha se moveu e `sem mudança` quando ficou no escudo. Essa
  distinção é o que o log resolve: perfil ausente em disco é ambíguo entre "nunca rodou" e "rodou e
  não mudou nada". Falha registra `[adaptativo] Sem ajuste: <motivo>`, e a causa mais comum é o
  monitor sem histórico vivo (app reaberto entre correr e importar).

  Enquanto o par `[import]` + `[adaptativo]` não aparecer num log de corrida real, o perfil por
  `custid` continua zerado e o `ai_sweet_spot` continua ancorando em nada. É medição, não código:
  só uma corrida de verdade fecha o item, e agora o log diz exatamente o que ela mediu e por que
  decidiu.
- **Os dois painéis seguem sem consumidor.** `RosterGenPanel` e `PostRacePanel` continuam
  não importados por nenhum arquivo do frontend (conferido por `grep` fora de
  `components/iracing/`). A exportação que o jogador dispara hoje passa por
  `race/nextrace/useIracingExport.js`, que chama os MESMOS `iracing_generate_roster` e
  `iracing_generate_season`. Os dois painéis agora têm teste de contrato
  (`RosterGenPanel.test.jsx`, `PostRacePanel.test.jsx`), e os testes travam o contrato dos
  comandos — o que vale nos dois desfechos, montar ou aposentar. Aposentar deixou de custar
  a cobertura.
- **O overlay ao vivo entrou na rede.** `IracingConnectedOverlay` é a terceira tela da
  ponte e a única que fica na frente do jogador durante a corrida; ganhou
  `IracingConnectedOverlay.test.jsx`, que trava os nomes de campo do `RaceHistory` que ela
  lê. Renomear um campo no Rust deixava a tela abrir normalmente mostrando "aguardando
  dados" a corrida inteira, sem erro nenhum.
- **Os itens 3 e 4 não cabem numa frente só de `commands/iracing/`.** O item 3 precisa de
  botão em `Settings.jsx` (ou no `useFerramentasDeDebug`), e o item 4 mexe no
  `invoke_handler` do `lib.rs` — os dois fora do escopo de quem mexe nos comandos. Ficam
  registrados aqui como dependência explícita, não como esquecimento.
- **O item 4 e o guard discordam sobre dois comandos, e a discordância fica registrada.** O item
  manda remover `iracing_send_chat_macro` e `iracing_career_race_result`; o guard de 11/08 rotula o
  primeiro como "diagnóstico e bancada" e o segundo como "feature futura, ver §6" — apontando de
  volta para o item que pede o corte. Os dois textos foram escritos por passadas diferentes e
  nenhuma delas tinha autoridade para decidir. **Não foi resolvido de propósito**: cortar ou manter
  comando registrado é decisão do dono, e o guard só existe para garantir que a lista não mude sem
  que alguém decida.
