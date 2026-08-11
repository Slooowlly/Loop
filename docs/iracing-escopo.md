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

Só o que é alcançável pelo jogador numa tela montada. 53 comandos registrados; os que
sustentam o produto são estes.

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

## 3. Os 9 comandos sem consumidor nenhum

Nenhum deles é chamado pelo frontend nem pelos testes Rust (verificado por `grep` em todo
o crate; não há `invoke` dinâmico por variável no projeto). O risco "caminho paralelo vivo
só pelos testes" do briefing R5 **não** se aplica aqui.

| comando | classificação | por quê |
|---|---|---|
| `iracing_read_telemetry` | **interno** | a função é consumida pelo sampler (`race_monitor/amostrador.rs:18`). O *comando* é andaime; a função fica. |
| `iracing_read_session` | **interno** | idem (`amostrador.rs:31`), além de `dump_session_yaml` e `player_custid`. |
| `iracing_poll_race` | **interno** | `race_monitor::poll()` é usado por `overlay/torre.rs`, `resultado.rs` e `corridas_salvas.rs`. Só a exposição IPC sobra. |
| `iracing_log_caminho` | **interno** | `iracing_log_revelar` já entrega o caso de uso (abrir a pasta) e é o que a UI usa. |
| `iracing_send_chat_macro` | **morto** | o próprio doc-comment diz "teste cru — descobrir o slot certo". Superado por `iracing_send_chat_text`, que é o caminho parametrizado e **está** ligado em Settings. |
| `iracing_career_race_result` | **morto** | reconstrói o resultado a partir do monitor ao vivo. Superado por `build_session_race_result`, que lê o resultado **oficial** do aiseason. Manter os dois é manter duas verdades sobre a mesma corrida. |
| `iracing_reset_race` | **feature pendente** | zerar o monitor entre testes. `race_monitor::reset()` não tem outro chamador — sem este botão, a função é inalcançável. Cabe no painel de diagnóstico. |
| `iracing_restore_yellow_macro` | ~~feature pendente~~ → **removido em 2026-07-27** | decisão do dono: restaurar a macro não tem valor de produto. O original continua salvo no estado e há backup completo do `app.ini`; quem quiser desfazer faz à mão, e o iRacing reescreve o arquivo ao fechar. Comando, registro no `lib.rs` e `race_control::restore()` removidos. |
| `iracing_throw_yellow` | **feature pendente** | bandeira amarela **manual**. O automático já está ligado; falta o botão. Direção de prova pronta, sem porta. |

---

## 4. O segundo achado: dois painéis inteiros desligados

O briefing pedia os órfãos por `grep` de comando. Ao subir um nível — *a tela que chama o
comando está montada?* — aparece uma camada maior:

- [`RosterGenPanel.jsx`](../src/components/iracing/RosterGenPanel.jsx) — **723 linhas**
- [`PostRacePanel.jsx`](../src/components/iracing/PostRacePanel.jsx) — **697 linhas**

Nenhum dos dois é importado em lugar nenhum. São componentes exportados e nunca montados.
Com isso, mais 7 comandos ficam inalcançáveis pelo jogador embora tenham "consumidor":

`iracing_dump_session_yaml`, `iracing_preview_race_result`, `iracing_apply_player_paint`,
`iracing_player_custid`, `iracing_player_paint`, `iracing_export_rain_test`,
`iracing_process_race_result`.

**O caro da lista era `iracing_process_race_result`**: ele é a dificuldade adaptativa
(`adaptativo.rs`) — atualiza o perfil do jogador por `custid` depois de cada corrida
limpa, global e por pista. Ele só era chamado pelo `PostRacePanel`. Como o `ai_sweet_spot`
(`temporada.rs:66`) **lê** esse perfil para ancorar a curva de skill da IA na geração de
roster/temporada, o efeito prático era: o perfil nunca era escrito, a leitura devolvia
sempre zero, e a IA nunca calibrava ao jogador. Não era comando órfão; era feature paga e
não entregue.

**Corrigido em 2026-07-27**: a chamada saiu da UI e entrou no import automático
(`importacao.rs`), best-effort, logo após o import bem-sucedido — que já é idempotente por
corrida. Deixou de depender de painel montado.

**Total: 16 dos 49 comandos iRacing (33%) são inalcançáveis pelo jogador.**

---

## 5. A decisão de produto

> **O Loop é um simulador de carreira offline que opcionalmente lê o iRacing, ou é uma
> ferramenta de acompanhamento do iRacing que tem uma carreira simulada dentro?**

**Decidido em 2026-07-27 pelo dono do projeto: a segunda.** O Loop é uma **ferramenta de
iRacing com uma carreira simulada dentro**. Correr de verdade é o caminho principal; a
simulação interna preenche o que o jogador não corre.

Os números que sustentam a decisão: 49 de 158 comandos (31% da ponte IPC) e 16.910 linhas
em `iracing_sdk/` já dizem isso na prática. A decisão apenas alinha o documento ao peso
real do código.

### O que decorre disso

1. **Os órfãos não são candidatos a corte por padrão.** As três "features pendentes" da
   §3 e os dois painéis da §4 viram backlog de UI, não lixo. O único corte limpo são os
   dois **mortos** (`send_chat_macro`, `career_race_result`) e as quatro exposições IPC
   **internas** — que saem do `invoke_handler` e continuam como função do crate.
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
| 1 | Dar destino ao `PostRacePanel` (697 linhas prontas) — montar ou aposentar | M |
| 2 | Dar destino ao `RosterGenPanel` (723 linhas) — é painel de debug; decidir se vira tela de Settings | M |
| 3 | Bandeira amarela manual (`iracing_throw_yellow`) + reset do monitor (`iracing_reset_race`) no diagnóstico | S |
| 4 | Remover `iracing_send_chat_macro` e `iracing_career_race_result`; tirar da ponte `read_session`, `read_telemetry`, `poll_race`, `log_caminho` | S |

O item 4 é o único que mexe em `lib.rs` — e exige `npm run build` **antes** de
`cargo test`, porque `generate_context!` embute o `dist/`.

### 6.1 Conferência de 2026-08-11

A vistoria de 10/08 voltou a marcar os 16 inalcançáveis como pendência (A14.2). Conferido
contra o código de 11/08, **nada mudou desde 27/07**: os quatro itens acima continuam
abertos e a conta de 16 continua valendo. O que a conferência acrescenta:

- **A dificuldade adaptativa está ligada, mas ligar não é rodar.** A chamada existe
  (`importacao.rs:138`, com `crate::diagnostico::linha("adaptativo", ...)` no erro), e até
  10/08/2026 ela nunca havia sido executada uma vez sequer — o caminho depende do
  auto-import fechar, e é o par de linhas `[import]` seguido de `[adaptativo]` no
  `loop.log` que prova o ciclo. Enquanto esse par não aparecer num log de corrida real, o
  perfil por `custid` continua zerado e o `ai_sweet_spot` continua ancorando em nada. É
  medição, não código: só uma corrida de verdade fecha o item.
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
