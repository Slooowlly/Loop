# O que falta da vistoria de 10/08/2026

Conferência ponto a ponto da [vistoria de 10/08](vistoria-2026-08.md) contra o código de
11/08/2026. A vistoria mapeou 179 pontos em 14 áreas. Desde então a árvore ganhou 194 arquivos
com diff (+7.195/-4.049) e nenhum commit novo, então boa parte do trabalho já feito está solta
no working tree.

Saldo: **52 fechados, 12 parciais, 113 abertos.**

Como a conferência foi feita: cruzei os arquivos citados em cada ponto com a lista de arquivos
modificados. Arquivo intacto significa ponto aberto. Arquivo tocado foi verificado no disco, um
a um, com a evidência anotada em cada item abaixo. Onde a evidência é indireta, o item traz a
marca `(verificar)`.

Cada pendência tem quatro linhas:

- **Onde**: arquivo e linha de hoje.
- **Hoje**: como a peça funciona neste momento.
- **Muda**: o que precisa ser feito.
- **Porque**: o risco que a mudança fecha.

O id (A1.2, A7.1) é estável e serve para riscar item conforme fechar.

---

## Placar por área

| Área | Total | Fechados | Parciais | Abertos |
|---|---|---|---|---|
| A1 Motor de corrida | 12 | 3 | 0 | 9 |
| A2 Mundo vivo entre temporadas | 16 | 6 | 1 | 9 |
| A3 iRacing SDK | 12 | 5 | 2 | 5 |
| A4 Comandos de carreira | 15 | 5 | 1 | 9 |
| A5 Comandos em tempo real | 15 | 5 | 1 | 9 |
| A6 Persistência e configuração | 10 | 4 | 1 | 5 |
| A7 Economia e carro | 12 | 1 | 0 | 11 |
| A8 Engenheiro, narrativa e notícias | 9 | 5 | 0 | 4 |
| A9 Suporte do domínio | 15 | 4 | 0 | 11 |
| A10 Frontend, estado e telas | 11 | 3 | 1 | 7 |
| A11 Componentes e overlay | 12 | 2 | 1 | 9 |
| A12 As três suítes | 11 | 3 | 2 | 6 |
| A13 Trabalho não commitado | 10 | 2 | 1 | 7 |
| A14 Dívida documentada | 17 | 4 | 1 | 12 |
| **Total** | **177** | **52** | **12** | **113** |

O total declarado na vistoria era 179. A diferença de 2 vem de itens que a vistoria contou juntos
em uma linha só; nada foi perdido.

Parcial significa que a mudança começou e não fechou: o item continua na lista, com o estado de
hoje descrito.

---

## A1. Motor de corrida

Fechados: a função-monstro do domingo, a prosa PT do contato e as buscas lineares no laço.

### A1.1 Constantes de tráfego e ultrapassagem seguem sem calibração
- **Onde**: `src-tauri/src/simulation/race/trafego.rs:55` e `:69`
- **Hoje**: `JANELA_AR_SUJO_MS = 1000.0`, `PROB_BASE_ULTRAPASSAGEM = 0.35`, mais o gap mínimo
  entre carros e os custos de tentativa falha. São os números que decidem quanto vale largar na
  frente. Nenhum passou pela máquina de busca de `simulation/calibracao/busca.rs`.
- **Muda**: rodar a varredura de knobs com essas constantes no espaço de busca e registrar o
  resultado no cabeçalho do arquivo, no padrão que `fame.rs` já usa.
- **Porque**: são o coração do realismo da corrida e hoje ninguém sabe se estão certas. Calibrar
  os knobs externos por cima de constantes internas erradas trava o erro por baixo.

### A1.2 Três tabelas paralelas por track_id sem guard cruzado
- **Onde**: `src-tauri/src/simulation/profile/lap_times.rs` (619 braços de match),
  `src-tauri/src/simulation/track_profile.rs`, `src-tauri/src/constants/tracks/`
- **Hoje**: pista nova exige editar as três. A ausência em `lap_times` cai num fallback por
  comprimento em km, que esconde o esquecimento.
- **Muda**: um teste que cruze a cobertura das três tabelas e falhe quando um track_id existir
  numa e faltar na outra.
- **Porque**: o modo de falha é silencioso. O jogo continua rodando com tempo base errado.

### A1.3 Caminhos legados vivos no motor e na quali
- **Onde**: `simulation/race/motor.rs` (6 usos de `LegadoDePontos` fora de teste),
  `simulation/qualifying.rs` (11 ocorrências de `legada`/`qual_weights_legados`)
- **Hoje**: existem só para a prova de equivalência bit-exata e para o harness. São ramos
  duplicados de pesos e sorteio que precisam de manutenção sincronizada.
- **Muda**: decidir a data de remoção agora que a prova passou, e remover.
- **Porque**: cada mudança no motor obriga a mexer no gêmeo legado, e o gêmeo não tem consumidor
  em produção.

### A1.4 Números mágicos internos dos sorteios fora do espaço de calibração `(verificar)`
- **Onde**: `simulation/incidents/sorteios.rs`, `simulation/race/pontuacao.rs`
- **Hoje**: pivô de confiabilidade, chance de pane leve, absorção de chuva e os pesos de
  atributo por segmento são literais no corpo das funções. Os dois arquivos foram tocados em
  11/08 e a mudança não foi conferida item a item.
- **Muda**: promover a constantes nomeadas e incluir no espaço de busca da calibração.
- **Porque**: a varredura de knobs não enxerga o que está inline.

### A1.5 Curvas do dossiê do jogador sem validação
- **Onde**: `src-tauri/src/player_skill.rs` (arquivo intacto)
- **Hoje**: agressividade sobe 16 pontos por incidente, adaptabilidade 5 pontos por posição
  ganha, experiência satura em 60 corridas. Tudo declarado como aproximação.
- **Muda**: medir contra a distribuição real do harness e ajustar, ou marcar no cabeçalho que a
  curva é ilustrativa.
- **Porque**: é só exibição, o mercado não consulta. O risco é o dossiê mentir para o jogador.

### A1.6 Vinte e tantas constantes de pressão travadas por decisão
- **Onde**: `src-tauri/src/simulation/pressure.rs:19` em diante (arquivo intacto)
- **Hoje**: `PACE_K = 3.0`, neutro em 0.55, multiplicadores de líder e de última corrida. Fixados
  em design, antes da esteira permitir medição.
- **Muda**: passar pelo harness agora que a esteira existe.
- **Porque**: a camada afeta simulação offline e export para o iRacing. Um swing mal dimensionado
  distorce as duas pontas do jogo.

### A1.7 Texto do catálogo de incidentes congela no idioma de origem
- **Onde**: `src-tauri/src/simulation/catalog.rs` (intacto)
- **Hoje**: a descrição vem do banco e persiste no save no idioma em que foi gerada.
- **Muda**: confirmar a paridade de idiomas do catálogo e escrever no cabeçalho que o
  congelamento é decisão, igual à opção A das lesões.
- **Porque**: sem o registro, o próximo leitor trata como bug e "conserta" quebrando saves.

### A1.8 Pesos da nota de corrida
- **Onde**: `src-tauri/src/race_eval.rs` (intacto)
- **Hoje**: base 0.52/0.30/0.18 com teto em 7.0 mais 0.3 por posição absoluta. Documentado e
  testado, com um teste de inspeção manual marcado como ignorado.
- **Muda**: só revisitar se a nota gerar reclamação de jogador.
- **Porque**: o desenho está saudável. Fica registrado para não virar achado novo na próxima
  varredura.

### A1.9 Ruído de 8 a 10% no harness Monte Carlo
- **Onde**: `src-tauri/src/sim_stats.rs` (intacto)
- **Hoje**: execuções idênticas variam entre si e o agregado esconde tendência entre temporadas.
- **Muda**: instrumentar por temporada antes de tirar conclusão de calibração dali.
- **Porque**: é o instrumento que vai fechar A1.1, A1.4 e A1.6. Instrumento sem régua confiável
  produz constante errada com cara de medida.

---

## A2. Mundo vivo entre temporadas

Fechados: a semente da semana no mercado, o doc e o `allow` do car_maintenance, o teste do
season_transition, os `allow(dead_code)` de hierarchy e public_impact, e as constantes nomeadas
da convocação.

### A2.1 Função de 524 linhas no coração da escada fechada
- **Onde**: `src-tauri/src/market/pipeline/consolidacao.rs`, `fill_remaining_vacancies_with_rookies`
- **Hoje**: um corpo só carrega a cascata inteira: cotas reabastecidas em voo, promoção de feeder,
  recrutamento profundo, fallback de pool e promoções de emergência. Cresceu 13 linhas desde a
  vistoria.
- **Muda**: quebrar em etapas nomeadas, no mesmo padrão que o resto do pipeline já usa.
- **Porque**: é a garantia de grid cheio do modelo fechado. Qualquer mexida hoje exige entender
  o arquivo inteiro.

### A2.2 Eixo de tensão N1/N2 continua inerte `(parcial)`
- **Onde**: `src-tauri/src/hierarchy/orders.rs:123`
- **Hoje**: os deltas viraram constantes nomeadas (`TENSAO_DELTA_N2_VENCE = 3.0`,
  `TENSAO_DELTA_N1_VENCE = 2.0`) e o arquivo ganhou a nota explicando como recalibrar. Os valores
  são os mesmos: o N2 precisa vencer 40% dos duelos e a medição registrou 23% a 33%.
- **Muda**: baixar `TENSAO_DELTA_N1_VENCE` até o ponto de equilíbrio cair perto da taxa medida, e
  rodar o harness para confirmar.
- **Porque**: enquanto a tensão tende a zero, o eixo existe no código e não existe no jogo. Tudo
  que for construído em cima dele nasce morto.

### A2.3 Ordem da virada de temporada travada só por comentário
- **Onde**: `src-tauri/src/evolution/pipeline/orquestracao.rs` (intacto)
- **Hoje**: `run_end_of_season_with_mode` encadeia mais de 15 passos acoplando evolution, market,
  world, finance, rivalry e promotion. As invariantes de ordem vivem em prosa.
- **Muda**: um teste que asserte a sequência, ou um tipo que torne a ordem errada impossível.
- **Porque**: a ordem já quebrou uma vez, no caso da rivalidade nascida e apagada no mesmo tique.

### A2.4 RNG do assédio ao jogador semeado só pela temporada
- **Onde**: `src-tauri/src/market/pipeline/jogador.rs:575` (intacto)
- **Hoje**: `StdRng::seed_from_u64(season ^ constante)`. O irmão `preseason/semana.rs` já foi
  corrigido e mistura a semana.
- **Muda**: misturar o mesmo discriminante que o `semana.rs` usa.
- **Porque**: decisões de momentos diferentes ficam correlacionadas e o mercado repete padrão.

### A2.5 Leilão de assédio ao jogador concentrado e com DTO sensível
- **Onde**: `src-tauri/src/market/pipeline/assedio_jogador.rs` (intacto)
- **Hoje**: `compute_player_poach_offer_inner` tem cerca de 220 linhas e o `PlayerPoachOffer`
  cruza a ponte serde para o React.
- **Muda**: extrair as etapas e cobrir o contrato com teste de shape.
- **Porque**: é a única tela em que o jogador decide um poaching. Campo trocado quebra a UI em
  silêncio.

### A2.6 `with_savepoint` triplicado
- **Onde**: `market/pipeline/comum.rs`, `promotion/pipeline.rs`, `promotion/pilots.rs`
- **Hoje**: três implementações idênticas do mesmo helper, nos módulos que rodam na mesma virada.
- **Muda**: um utilitário único em `db/`.
- **Porque**: tratamento de rollback divergente entre os três é falha de dados, não de estilo.

### A2.7 Monolitos de teste do mercado
- **Onde**: `market/pipeline/tests/mod.rs` (2.723), `market/car_maintenance/tests/mod.rs` (1.204),
  `convocation/pipeline/tests/mod.rs` (1.148)
- **Hoje**: o código de produção foi fatiado em etapas e os testes ficaram num bloco só.
- **Muda**: fatiar por tema, como `economia/tests` e `engenheiro/tests` já fazem.
- **Porque**: achar o teste de uma etapa custa caro e o arquivo cresce sem dono.

### A2.8 Estático global de instrumentação sem teto
- **Onde**: `src-tauri/src/market/pipeline.rs:29`
- **Hoje**: `EMERGENCY_PROMO_PATHS` é um `Mutex<Vec<(u8,u8)>>` global que só o harness zera.
- **Muda**: limpar por carreira, ou trocar por contador.
- **Porque**: numa carreira jogável longa o vetor cresce sem limite dentro do processo.

### A2.9 Flags de A/B por variável de ambiente em produção
- **Onde**: `promotion/pipeline.rs` (2 flags), `market/pipeline/consolidacao.rs` (5 flags)
- **Hoje**: `IRACER_PROMO_SOFT_LANDING`, `IRACER_ROOKIE_MERIT`, `IRACER_MARKET_AFFORDABILITY` e a
  família `IRACER_PROMO_DIMINISH_*` mudam regra de jogo por env var lida a cada chamada.
- **Muda**: inventário central e decisão de quais viram default definitivo.
- **Porque**: regra de jogo decidida por ambiente é irreproduzível no relato de bug do jogador.

### A2.10 `sync.rs` e `proposals.rs` sem teste próprio
- **Onde**: `src-tauri/src/market/sync.rs`, `src-tauri/src/market/proposals.rs` (intactos)
- **Hoje**: `sync_team_slots_from_active_regular_contracts` é chamada em quase todo passo do
  mercado e só tem cobertura indireta.
- **Muda**: teste direto dos dois.
- **Porque**: erro de sincronismo de vaga aparece três passos adiante, no lugar errado.

---

## A3. Integração com o iRacing

Fechados: `Attempt.status` virou enum, custid e flag de amarela saíram de `%TEMP%`,
`race_control.rs` ganhou teste, o `expect` do spotter lento saiu do laço quente e o
`allow(dead_code)` do rivalry_perception foi removido.

### A3.1 Severidade de quebra ainda circula como string `(parcial)`
- **Onde**: `iracing_sdk/race_monitor/quebras.rs:248`, `commands/ai_news/fatos.rs:772`,
  `commands/overlay/radio.rs:208`, `commands/race/importacao.rs:165,194,203`
- **Hoje**: o status da tentativa virou enum serde. A severidade da quebra continua comparada por
  literal (`severity == "dnf"`) em seis pontos de produção.
- **Muda**: mesmo tratamento do status: enum com `rename` e conversão na borda.
- **Porque**: um typo compila, serializa e falha em silêncio dos dois lados da ponte.

### A3.2 Entrega de eventos do spotter ainda por polling `(parcial)`
- **Onde**: `iracing_sdk/spotter.rs`, `commands/iracing/` (drenagem por invoke)
- **Hoje**: o sampler produz a 60 Hz e o front drena por invoke periódico. O commit de 11/08
  atacou o sintoma pelo outro lado, desligando o throttling nas três webviews do
  `tauri.conf.json`.
- **Muda**: emitir por push (`emit` do Tauri) e manter o buffer só como folga.
- **Porque**: o throttling do webview é decisão do navegador. Desligar por flag funciona hoje e
  depende de uma flag que pode sumir numa atualização do WebView2.

### A3.3 Penalidade depende de SendInput com foco de janela
- **Onde**: `iracing_sdk/imp/chat.rs`, `iracing_sdk/race_monitor/amostrador.rs`
- **Hoje**: o `!black`/`!dq` só chega se o SO aceitar o foreground. Em fullscreen exclusivo o
  envio pode falhar sem erro detectável.
- **Muda**: medir se o fallback de bandeira preta com penalidade fixa cobre todos os desfechos.
- **Porque**: a punição da quali destruída depende disso, e o furo é indetectável em runtime.

### A3.4 Noventa constantes de calibração de domínios distintos no mesmo bloco
- **Onde**: `iracing_sdk/race_monitor.rs` (85 constantes de arquivo)
- **Hoje**: batida, race control, cluster de pit, quebra na quali e limites de memória dividem o
  mesmo bloco. Parte é calibrada em teste, parte foi decidida a olho.
- **Muda**: separar por domínio e marcar no comentário o que tem medição atrás.
- **Porque**: hoje não dá para saber, olhando, se o número que você vai mexer foi medido.

### A3.5 Duplicação estrutural na família spotter
- **Onde**: `spotter_frente.rs`, `spotter_tras.rs`, `spotter_lento.rs`, `spotter_voltar.rs`,
  `spotter_bandeira.rs`, `spotter_boxe.rs`
- **Hoje**: cada irmão reimplementa singleton, fila com teto, detecção de salto de SessionTime e
  rodízio de chaves. São ~7 mil linhas onde a parte calibrada é pequena.
- **Muda**: esqueleto comum, deixando em cada arquivo só os limiares medidos.
- **Porque**: seis cópias da mesma infraestrutura significam seis lugares para corrigir o próximo
  bug de fila.

### A3.6 Mapeamento de variáveis do SDK sem guarda cruzada
- **Onde**: `iracing_sdk/imp/leitura.rs` (intacto; `read_var_inventory` existe na linha 36)
- **Hoje**: o match casa canais por nome e nome errado cai no braço `_` calado, lendo zero para
  sempre. Já aconteceu em produção com `PitRepairNeeded`.
- **Muda**: na borda de conexão, cruzar a lista curada com `read_var_inventory` e logar canal
  ausente.
- **Porque**: transforma um silêncio permanente em uma linha de diagnóstico.

### A3.7 Suíte do race_monitor num arquivo de 2.095 linhas
- **Onde**: `iracing_sdk/race_monitor/tests/mod.rs`
- **Hoje**: cresceu 42 linhas desde a vistoria. O módulo sob teste já está fatiado em 13 arquivos.
- **Muda**: fatiar espelhando os submódulos.
- **Porque**: localizar o teste de uma área exige busca textual no arquivo inteiro.

---

## A4. Camada de comandos de carreira

Fechados: a oferta de poaching passou a ser conferida contra o plano persistido, o stub
`persist_end_of_season_news` saiu, o comentário defasado de `interests.rs` foi corrigido, o merge
manual do `update_config` foi refeito e o `CLAUDE.md` foi atualizado.

### A4.1 `load_career_in_base_dir` continua grande `(parcial)`
- **Onde**: `src-tauri/src/commands/career/lifecycle.rs`, 187 linhas
- **Hoje**: caiu de ~240 para 187 linhas e ainda mistura reparo de fase, telemetria, thread de
  prewarm, interesse do evento, cota de fama, escrita de meta.json e montagem do payload.
- **Muda**: extrair os blocos puros para funções nomeadas.
- **Porque**: é onde toda preocupação nova aterrissa. O arquivo cresce por gravidade.

### A4.2 Mensagens de erro em português cru, fora do i18n
- **Onde**: 40 ocorrências em `commands/career/*.rs`, concentradas em `market_window.rs` (14),
  `lifecycle.rs` (5), `queries.rs` (5), `season_flow.rs` (5), `vacancies.rs` (4)
- **Hoje**: os erros voltam como `String` em PT sem acento ("Temporada nao encontrada."),
  enquanto o vizinho `lifecycle.rs:129` usa `t!("career.message.created")`.
- **Muda**: passar as mensagens por `rust_i18n` e acentuar.
- **Porque**: jogador em en-US recebe prosa em português, sem acento, na tela de erro.

### A4.3 Comandos de debug registrados no build de produção
- **Onde**: `src-tauri/src/lib.rs` (2 comandos de debug no handler, zero `cfg(debug_assertions)`)
- **Hoje**: `debug_prepare_market_scenario` e `debug_stamp_player_championship` escrevem SQL
  direto no save (posição forçada, fama 82.0, contrato rescindido) e estão no invoke_handler sem
  gate.
- **Muda**: envolver com `#[cfg(debug_assertions)]` ou uma flag de build.
- **Porque**: qualquer devtools aberto num build de release corrompe uma carreira real.

### A4.4 Draft histórico bloqueia o comando async
- **Onde**: `src-tauri/src/commands/historical_draft.rs`
- **Hoje**: `create_historical_career_draft_in_base_dir` roda 26 temporadas sincronamente dentro
  de um comando async, com progresso saindo por polling de meta.json.
- **Muda**: `spawn_blocking`.
- **Porque**: trabalho de minutos no runtime async segura outros comandos.

### A4.5 Helpers duplicados entre historical_draft e career
- **Onde**: `historical_draft.rs` contra `career/lifecycle.rs` e `career/save_state.rs`
- **Hoje**: `career_number_from_id`, `count_rows` e `read_save_meta` existem em duas cópias. A
  lista mágica `["production_challenger", "endurance"]` aparece em `season_flow.rs` e em
  `historical_draft.rs`.
- **Muda**: uma cópia de cada, e a lista de categorias especiais virando constante.
- **Porque**: categoria especial nova exige lembrar dos dois pontos, e esquecer um é silencioso.

### A4.6 `career/tests/mod.rs` com 5.897 linhas
- **Onde**: `src-tauri/src/commands/career/tests/mod.rs`
- **Hoje**: 93 testes de dez áreas diferentes num arquivo só, enquanto a lógica já foi fatiada em
  dez irmãos.
- **Muda**: fatiar espelhando `career/`.
- **Porque**: rodar o teste de uma área custa o arquivo inteiro.

### A4.7 Reparo de contratos em toda abertura de save
- **Onde**: `commands/career/lifecycle.rs`, `repair_regular_contract_consistency` sob
  `CAREER_OPEN_REPAIR_LOCK`
- **Hoje**: roda em cada abertura de escrita, carrega todos os contratos, equipes e pilotos, e
  serializa aberturas concorrentes.
- **Muda**: rodar sob condição (versão do save, marca de reparo já feito).
- **Porque**: o custo cresce com o mundo e é pago até por quem só ia escrever meta.json.

### A4.8 Parâmetro `category` ignorado
- **Onde**: `commands/career/lifecycle.rs`, `open_career_resources_for_category_read`
- **Hoje**: o corpo faz `let _ = category;` e delega ao read_only genérico. O chamador em
  `standings.rs` passa a categoria acreditando que ela importa.
- **Muda**: remover o parâmetro, ou implementar o comportamento que a assinatura promete.
- **Porque**: assinatura que mente induz o próximo leitor ao erro.

### A4.9 Heurística e ano fixo sem constante nomeada
- **Onde**: `commands/career/lifecycle.rs`
- **Hoje**: `is_title_decider` usa `remaining <= 2 && gap_to_leader <= 50` sem nome nem
  calibração; `Season::new(_, 1, 2024)` fixa o ano da carreira regular enquanto o draft histórico
  joga em 2026.
- **Muda**: constantes nomeadas e um único início de mundo.
- **Porque**: dois inícios de mundo divergentes por literal solto é bug esperando data.

### A4.10 Padrão de busca de vaga duplicado
- **Onde**: `commands/career/vacancies.rs`, em `generate_emergency_player_proposals` e
  `force_place_player`
- **Hoje**: os dois repetem filtrar vagas por tier e refazer a passada sem tier quando vazio,
  mudando só o predicado.
- **Muda**: um helper com o predicado como parâmetro.
- **Porque**: regra de encaixe é a que garante que o jogador não fique sem assento. Duas cópias
  divergem calado.

---

## A5. Comandos de integração em tempo real

Fechados: o auto-import passou a distinguir "aguardando" de "bloqueado" com log tipado, a tupla
de 11 campos virou a struct `ResultadoDaSessao`, as escritas engolidas do conserto de carro
ganharam tratamento, o `unwrap` da volta mais rápida saiu da torre e os limiares de previsão de
quebra passaram a ser reusados por constante nas duas telas.

### A5.1 `iracing_generate_roster` com 613 linhas
- **Onde**: `src-tauri/src/commands/iracing/roster.rs`
- **Hoje**: um comando lê o banco, monta contexto comportamental por piloto (lesão, vingança,
  nêmesis, lua de mel), resolve clima, calcula dificuldade de carro, grava três arquivos e
  instala o diretor de quebra no monitor.
- **Muda**: extrair a montagem do `DriverCtx` e do `BehaviorContext` como funções puras.
- **Porque**: é a peça que define o grid que o jogador vai enfrentar de verdade, e não tem teste
  nenhum. Função pura destrava teste unitário sem `AppHandle`.

### A5.2 `get_overlay_data` com 579 linhas `(parcial)`
- **Onde**: `src-tauri/src/commands/overlay/torre.rs:181` (cache) e a função principal
- **Hoje**: o cache por sessão foi criado (`CacheDaTorre` sob `OnceLock<Mutex<...>>`) e derrubou
  as consultas por carro a cada poll. A função continua com 579 linhas e sem teste direto; só os
  helpers de formato e ordem são testados.
- **Muda**: quebrar em etapas (resolver identidade, ordenar, montar payload) e testar a montagem.
- **Porque**: roda uma vez por segundo durante a corrida inteira, na janela que o jogador olha.

### A5.3 `commands/iracing/` praticamente sem teste
- **Onde**: 17 arquivos, e só `clima.rs` tem `#[cfg(test)]`
- **Hoje**: roster, temporada, resultado, adaptativo, previsão de quebras e pintura são validados
  apenas correndo de verdade.
- **Muda**: cobrir primeiro as partes puras, que já são testáveis hoje: `ensure_driver_numbers`,
  `ai_sweet_spot`, `story_to_weather_condition`, `sim_safe_year`.
- **Porque**: é a ponte exportar, correr e importar. É o produto.

### A5.4 Tabela de offsets por pista calibrada na escada antiga
- **Onde**: `commands/iracing/temporada.rs:165`, `track_skill_offset`, 11 entradas
- **Hoje**: os offsets foram medidos com a base de tier 10 pontos acima, rebaixada em 10/08. A
  nota que admitia a defasagem saiu do arquivo e os valores continuam os mesmos.
- **Muda**: re-medir em pista cada entrada, ou reinserir a nota de defasagem enquanto a medição
  não acontece.
- **Porque**: hoje soma offset antigo sobre baseline nova, e o arquivo já não avisa disso.

### A5.5 Acoplamento roster para temporada via post-its em arquivo
- **Onde**: `commands/iracing/adaptativo.rs`, `commands/iracing/temporada.rs`
- **Hoje**: a temporada só sai correta se o roster rodou antes na mesma pista e categoria
  (`ExportSkillBand`). A validade é checada por casamento de categoria mais pista, sem carimbo de
  tempo, e ainda resta uma escrita com `let _ =` em `temporada.rs`.
- **Muda**: carimbar o post-it com horário e recusar post-it velho; logar a falha de escrita.
- **Porque**: post-it de um fluxo interrompido produz banda de skill errada sem nenhum aviso.

### A5.6 Rivalidade IA contra IA é O(n²) dentro do import
- **Onde**: `commands/iracing/resultado.rs`
- **Hoje**: para cada carro-sonda roda `perceive_rivalries` sobre o histórico completo, com
  dedupe manual por par e todos os erros engolidos.
- **Muda**: logar as falhas e cortar o custo quadrático antes do grid de endurance crescer.
- **Porque**: hoje é aceitável em grid de 20. No endurance o custo cresce e nenhuma falha aparece.

### A5.7 Hemisfério da pista deduzido por emoji de bandeira
- **Onde**: `commands/iracing/clima.rs:37`, `track_hemisphere`
- **Hoje**: `pais.contains` sobre uma lista de emojis.
- **Muda**: campo de hemisfério no catálogo de pistas.
- **Porque**: pista nova de país do hemisfério sul fora da lista cai no norte em silêncio e
  inverte a estação inteira daquela etapa.

### A5.8 URL do servidor Cloud Run duplicada
- **Onde**: `commands/ptt_voz.rs`, `narrative/client.rs`, `telemetry.rs`, `diagnostico.rs`
- **Hoje**: o mesmo host aparece hardcoded em quatro arquivos.
- **Muda**: uma constante com os paths derivados.
- **Porque**: migrar de região ou de projeto exige caça manual, e esquecer um arquivo quebra só
  aquele recurso.

### A5.9 Export da temporada altera o calendário do save
- **Onde**: `commands/iracing/temporada.rs`
- **Hoje**: `iracing_generate_season` faz UPDATE em `calendar` (clima e temperatura) por etapa com
  `let _ =`, dentro de um comando cujo nome promete gerar arquivo.
- **Muda**: logar a falha e declarar o efeito no doc do comando.
- **Porque**: a intenção de fonte única do clima é boa. A escrita escondida num export com falha
  engolida é o que precisa mudar.

### A5.10 Máscara de bits sem constante nomeada
- **Onde**: `commands/overlay/torre.rs` (2 ocorrências de `0x0001_0000`)
- **Hoje**: a bandeira preta do jogador é lida por literal, sem apontar o enum do SDK.
- **Muda**: constante nomeada com referência ao flag do SDK.
- **Porque**: dois literais iguais em pontos distintos divergem na primeira correção.

---

## A6. Persistência e configuração

Fechados: a inferência de lesão legada por `LIKE` sobre prosa saiu, `models/temporal.rs` ganhou
teste, a régua duplicada em SQL foi removida e a coluna `unit_seed` do `team_car` entrou como
migração v62 em vez de ALTER escondido.

### A6.1 Schema ainda criado por dois canais `(parcial)`
- **Onde**: 9 arquivos de `db/queries/` com `ensure_table`, contra `db/migrations/baseline.rs`
- **Hoje**: o drift real do `team_car` foi fechado pela v62 e o cabeçalho do arquivo passou a
  explicar que o `ensure_table` reaplica o mesmo DDL. Continuam nove arquivos criando tabela fora
  do array `MIGRATIONS`, e nenhuma dessas tabelas está sob a trava do `schema_ouro`.
- **Muda**: mover o DDL restante para migração, ou estender o `schema_ouro` para cobrir as
  tabelas criadas por query.
- **Porque**: a trava de schema só enxerga o que `run_all` produz. Mudança de coluna nessas
  tabelas passa sem guard.

### A6.2 `SELECT *` em produção
- **Onde**: 35 ocorrências em `db/queries/` (drivers, seasons, calendar, contracts, teams)
- **Hoje**: a leitura é por nome de coluna, então renomear ou remover coluna só estoura em runtime,
  no save de alguém.
- **Muda**: listar as colunas nas queries de leitura.
- **Porque**: nem o compilador nem o teste de schema pegam a quebra.

### A6.3 Sete módulos de query sem nenhum teste
- **Onde**: `ai_pre_race.rs`, `ai_post_race.rs`, `ai_story.rs`, `ai_world_notes.rs`,
  `player_nemesis.rs`, `rivalry_episodes.rs`, `special_team_entries.rs`
- **Hoje**: zero `#[cfg(test)]` nos sete, e são justamente os que criam as próprias tabelas.
- **Muda**: teste de ida e volta em banco em memória para cada um.
- **Porque**: maior risco de drift com a menor cobertura do diretório.

### A6.4 `drivers.rs` mapeia 54 colunas à mão em quatro lugares
- **Onde**: `db/queries/drivers.rs` (intacto), 938 linhas
- **Hoje**: campo novo de piloto exige tocar a baseline, o INSERT nomeado, o UPDATE e o
  `driver_from_row`, além do model.
- **Muda**: gerar ou centralizar a lista de colunas.
- **Porque**: a paridade entre as quatro listas não é verificada por nada além de runtime.

### A6.5 `get_or_create_install_id` engole falha de gravação
- **Onde**: `src-tauri/src/config/app_config.rs`
- **Hoje**: `let _ = self.save()` ignora erro de disco.
- **Muda**: logar a falha.
- **Porque**: se a escrita falhar, nasce um install_id novo a cada boot e o cooldown do servidor
  de boletins reseta junto, sem ninguém entender por quê.

### A6.6 Models com lógica de mundo e acesso a banco
- **Onde**: `models/license.rs`, `models/team.rs` (intactos)
- **Hoje**: `repair_missing_licenses_for_current_categories` recebe `Connection` e escreve no
  banco de dentro de `models/`; `generate_teams_for_category` põe geração de mundo na camada de
  dados.
- **Muda**: mover para `commands/career` ou para os módulos de domínio.
- **Porque**: a exceção convida imitação, e o padrão do projeto é o oposto.

---

## A7. Economia e carro

Fechado: o gate de enduro nos dois call sites do iRacing, que agora leem a duração real da etapa
em vez da constante da categoria.

### A7.1 Sobrecusto de enduro linear e sem teto
- **Onde**: `src-tauri/src/car/breakdown.rs:527`, `enduro_surcharge`
- **Hoje**: `ENDURO_COST_K * over`, sem clamp. O alívio por parada real ganhou teto
  (`ENDURO_RELIEF_CAP`), o sobrecusto continua aberto. Em 360 min o multiplicador chega perto de
  12x e uma prova única persiste desgaste várias vezes além do fim de vida da peça.
- **Muda**: teto no sobrecusto, calibrado contra as durações reais do calendário de Endurance
  (120 a 360 min).
- **Porque**: sem teto, uma corrida longa zera qualquer decisão de manutenção seguinte. O
  comentário do próprio código só exemplifica 60 e 80 min.

### A7.2 Ralo novo dormente e regalia velha ativa
- **Onde**: `economia/desenvolvimento.rs` (sem consumidor de produção),
  `market/preseason/inicializacao.rs:232` (`apply_offseason_competitiveness_impact` ainda rodando)
- **Hoje**: o dreno do excedente foi medido no alvo pelo harness e só é chamado por teste. A
  pré-temporada continua creditando pontos de engineering e facilities sem debitar caixa.
- **Muda**: ligar o ralo no fluxo de pré-temporada e remover a regalia.
- **Porque**: é a fonte de dinheiro do nada que o próprio doc de `desenvolvimento.rs` diz
  substituir. Enquanto os dois convivem, o superávit das equipes não tem destino.

### A7.3 `cashflow.rs` mistura unidades e concentra assuntos
- **Onde**: `src-tauri/src/finance/cashflow.rs`, 1.134 linhas, 4 divisores em dólar absoluto
- **Hoje**: `cash_balance / 1_000_000` e `debt_balance / 900_000` sobreviveram à reancoragem do
  jogo em meses de operação. A força de caixa satura em categoria rica e nunca liga na base.
- **Muda**: converter os divisores para meses de operação e quebrar o arquivo.
- **Porque**: o efeito por categoria não é calibrado e o arquivo é o maior do `finance/`.

### A7.4 Estados financeiros e estratégias como string livre
- **Onde**: `finance/cashflow.rs` (3 matches por `&str`)
- **Hoje**: `financial_state_bias` e `season_strategy_bias` casam por `"elite"`, `"healthy"`,
  `"all_in"` com braço `_` neutro.
- **Muda**: enum com `from_str` explícito, no padrão que `Severity::from_key` já usa.
- **Porque**: estado novo ou typo passa sem erro e só zera o efeito.

### A7.5 Doc do módulo economia desatualizado e `allow` global
- **Onde**: `src-tauri/src/economia/mod.rs`
- **Hoje**: o doc ainda afirma que nada do módulo é chamado pela simulação, e o
  `#![allow(dead_code, unused_imports)]` de módulo inteiro continua. `finance/planning.rs`,
  `finance/state.rs` e os comandos de despesa e fatura já consomem `ancora`, `temporada`,
  `receita` e `fatura`.
- **Muda**: corrigir o doc e restringir o `allow` aos itens realmente pendentes.
- **Porque**: o `allow` de arquivo esconde import e função mortos de verdade.

### A7.6 Limiares do estilo de pilotagem sem calibração de pista
- **Onde**: `src-tauri/src/car/driving_style.rs` (intacto)
- **Hoje**: oito limiares de detecção (limitador, short-shift, frenagem forte, serra de volante,
  G de zebra) marcados no próprio arquivo como primeiro corte. Decidem multa de até +30% de
  desgaste do jogador.
- **Muda**: confrontar com captura real do SDK.
- **Porque**: é penalidade que o jogador paga em dinheiro, medida por número nunca verificado.

### A7.7 `breakdown.rs` com 1.292 linhas e proteção provisória
- **Onde**: `src-tauri/src/car/breakdown.rs`
- **Hoje**: quatro papéis no mesmo arquivo: modelo de hazard, condições de pista e clima, regras
  de enduro e a máquina ao vivo. `PLAYER_MAX_RELIEF = 0.05` traz comentário admitindo que o
  número final depende de medir o desgaste real de time pobre.
- **Muda**: quebrar em submódulos e fazer a medição pendente.
- **Porque**: cresceu 11 linhas desde a vistoria e é o arquivo que decide se o carro do jogador
  quebra.

### A7.8 `is_enduro_duration` ainda aceita a sentinela
- **Onde**: `src-tauri/src/car/breakdown.rs:509`
- **Hoje**: recebe `u16` puro. Os dois call sites que erravam foram corrigidos, e a assinatura
  continua aceitando `duracao_corrida_min = 0` sem reclamar.
- **Muda**: tipo que impeça passar a sentinela, ou `Option` explícito.
- **Porque**: o próximo call site vai errar do mesmo jeito. O bug foi consertado, a armadilha não.

### A7.9 Fallbacks de classe multi-classe em três convenções
- **Onde**: `finance/planning.rs`, `car/cost.rs`, e a divisão representativa do tier salarial
- **Hoje**: Endurance sem classe resolve para gt3 num lugar, para o teto histórico 8 (lmp2) em
  outro e para `endurance:lmp2` num terceiro.
- **Muda**: uma resposta única para a mesma pergunta.
- **Porque**: cada escolha tem justificativa local escrita, e o call site novo que esquecer a
  classe recebe uma das três ao acaso.

### A7.10 Ciclo macroeconômico idêntico em todo save
- **Onde**: `src-tauri/src/finance/economy.rs`
- **Hoje**: `season_number.rem_euclid(6)`. Recessão sempre nas temporadas múltiplas de 6, boom
  sempre na 3.
- **Muda**: sorteio semeado pelo save.
- **Porque**: todo jogador vive a mesma sequência previsível.

### A7.11 Números do carro herdados do GPRO
- **Onde**: `car/parts.rs`, `car/wear.rs`, `car/sim_bridge.rs`
- **Hoje**: `pha_per_level`, durabilidade, a tenda `level_durability_mult` e o `CAR_PERF_MAX` se
  declaram placeholder ou provisórios.
- **Muda**: medir pelo harness e substituir.
- **Porque**: são os coeficientes que definem o tradeoff desempenho contra confiabilidade do jogo
  inteiro.

---

## A8. Engenheiro, narrativa e notícias

Fechados: o helper genérico `postar_ao_servidor` unificou os cinco endpoints, o doc do acervo foi
corrigido para 3.948 peças e 373 MB, os status do ai_news viraram enum, o dossiê do rádio passou
a resolver por `rust_i18n` (46 chamadas `t!`) e o comentário de atrito da quebra deixou de
depender da igualdade exata.

### A8.1 `build_post_race_facts` com 912 linhas e sem teste
- **Onde**: `src-tauri/src/commands/ai_news/fatos.rs`, arquivo de 975 linhas, zero `#[cfg(test)]`
- **Hoje**: uma função monta 15 blocos, abre mais de 8 áreas de query, recompõe a pressão de
  título espelhando `simulation/pressure` e carrega limiares sem calibração documentada (mídia
  acima de 70 e de 87, deadzone de 0,4 no pace_delta, intensidade a partir de 2,0).
- **Muda**: extrair os blocos e cobrir a montagem inteira.
- **Porque**: os helpers `tese` e `telemetria` têm teste, a montagem não. Regressão num bloco só
  aparece lendo o texto gerado.

### A8.2 `news/mod.rs` com `allow(dead_code)` e conversão leniente
- **Onde**: `src-tauri/src/news/mod.rs`
- **Hoje**: o `allow` de arquivo continua, e `from_str` devolve Corrida ou Media para valor
  desconhecido enquanto `from_str_strict` existe ao lado.
- **Muda**: auditar quem ainda usa a leniente e decidir se o fallback silencioso é desejado.
- **Porque**: o `allow` esconde item morto de verdade.

### A8.3 `uso_tela` aceita string livre e descarta o desconhecido
- **Onde**: `src-tauri/src/telemetry/uso.rs`
- **Hoje**: o filtro silencioso é intencional, para o front não inventar métrica.
- **Muda**: uma linha no log para valor não reconhecido.
- **Porque**: tela nova com nome errado some sem erro e sem rastro.

### A8.4 Ritual repetido nos três comandos de IA
- **Onde**: `src-tauri/src/commands/ai_news/comandos.rs`
- **Hoje**: os três repetem resolução de `app_data_dir`, load do config, install_id, abertura do
  banco e o par cache mais passe em voo.
- **Muda**: extrair a preparação comum.
- **Porque**: cada endpoint novo paga o mesmo ritual e o tratamento de erro diverge.

---

## A9. Suporte do domínio

Fechados: as janelas divergentes entre gerador completo e parcial, o `LIT_TRACK_ID` que apontava
para pista inexistente (agora 554, em `constants/tracks/dados.rs:15`), a distribuição de chuva
duplicada e a divergência de faixas de skill com `models/driver.rs`.

### A9.1 Nacionalidade sem acento e congelada em pt-BR
- **Onde**: `src-tauri/src/generators/nationality.rs`, `generators/names/identidade.rs`
- **Hoje**: restam rótulos sem acentuação, e `generate_pilot_identity` persiste
  `nacionalidade_label` fixo em pt-BR.
- **Muda**: chave i18n resolvida no display, como `country_label` faz em `constants/mod.rs`.
- **Porque**: o jogador lê texto sem acento, o usuário en-US vê português, e o dado gravado ignora
  a troca de locale.

### A9.2 Duplicação entre `genesis.rs` e `historico.rs`
- **Onde**: `src-tauri/src/generators/world/` (ambos intactos)
- **Hoje**: os dois repetem o laço por categoria: geração de equipes, ordenação por skill,
  pareamento N1/N2, shuffle anti-roteiro do tier 0 e emissão de contratos, com comentário idêntico.
- **Muda**: um laço compartilhado.
- **Porque**: mudança de regra de pareamento exige tocar os dois em sincronia.

### A9.3 Substituição de pista paga ignora a posse real
- **Onde**: `src-tauri/src/constants/tracks/consultas.rs` (1 TODO)
- **Hoje**: `free_or_substitute` troca todo conteúdo pago por uma pista grátis determinística. O
  catálogo inteiro é a crava de um usuário específico, de julho de 2026.
- **Muda**: trocar pela lista de pistas que o jogador realmente possui.
- **Porque**: outro jogador com posse diferente recebe substituição errada no export.

### A9.4 Bandas de performance histórica só existem para gt3
- **Onde**: `src-tauri/src/constants/historical_timeline.rs` (intacto)
- **Hoje**: `historical_team_performance_band` devolve `None` fora de gt3, e as faixas são
  hardcoded por substring de nome real, sem registro de calibração.
- **Muda**: cobrir gt4, lmp2 e endurance, ou declarar a limitação no cabeçalho.
- **Porque**: mundo histórico nessas categorias nasce sem hierarquia de marca.

### A9.5 Ano de início de carreira usa o relógio da máquina
- **Onde**: `src-tauri/src/generators/driver_helpers.rs`, `models/driver_generation.rs`
- **Hoje**: `career_start_year_from_age` chama `current_year()` (`Local::now`).
- **Muda**: derivar do ano do mundo.
- **Porque**: em mundo histórico o início dos pilotos sai ancorado no ano civil de quem roda o
  jogo, e o mesmo save gerado em anos diferentes produz históricos diferentes.

### A9.6 Três formatos de nome de país
- **Onde**: `constants/mod.rs`, `constants/geografia.rs`
- **Hoje**: o dado cru mistura bandeira emoji, sem emoji e sem acento. `country_label` e a
  normalização de geografia mantêm cada um a sua lista de variantes.
- **Muda**: uma normalização única na entrada do dado.
- **Porque**: país novo exige lembrar de três lugares, e o esquecimento falha em silêncio.

### A9.7 Duração de quali duplicada
- **Onde**: `constants/tracks/consultas.rs`, `get_qualifying_duration` e
  `duracao_classificacao_para`
- **Hoje**: as duas implementam a mesma regra com assinaturas diferentes.
- **Muda**: uma função só.
- **Porque**: alterar o corte de 5 km numa e esquecer a outra desalinha export e simulação.

### A9.8 Strings de UI e de dado persistido em português cru
- **Onde**: `calendar/montagem.rs` (nome da etapa por `format!("Rodada {}")`),
  `constants/scoring.rs` (4 rótulos de dificuldade), `calendar/geracao.rs` (mensagem sem acento)
- **Hoje**: o nome da etapa nasce em PT e vai para o banco; `DifficultyConfig.nome` traz
  Fácil, Médio, Difícil e Lendário fora do `rust-i18n`.
- **Muda**: chave i18n no lugar do literal.
- **Porque**: tudo isso chega à tela em português no locale en-US.

### A9.9 Rivalidade de equipe duplica clamp e reusa campos de piloto
- **Onde**: `src-tauri/src/rivalry/team/motor.rs`
- **Hoje**: `AXIS_MAX`/`AXIS_MIN` são cópia de `rivalry/intensidade.rs`, e os ids de equipe viajam
  nos campos `piloto1_id`/`piloto2_id` do par normalizado, com a dependência documentada só em
  comentário.
- **Muda**: par genérico ou tipo próprio para equipe.
- **Porque**: funciona hoje e quebra na primeira refatoração do modelo de par.

### A9.10 Prova-mestra do 9D com quase 300 linhas
- **Onde**: `src-tauri/src/tests_9d.rs`, `ciclo_completo_s1_s2_s3`
- **Hoje**: o ciclo de duas temporadas inteiro num `#[test]` só.
- **Muda**: quebrar em fases nomeadas com helpers de asserção.
- **Porque**: uma falha no meio esconde as asserções seguintes.

### A9.11 `expect` em caminho de produção do calendário
- **Onde**: `calendar/geracao.rs`, `calendar/janela.rs`
- **Hoje**: `ids_iter.next().expect("race id")` e `expect` encadeado nas funções de data.
- **Muda**: erro tratável.
- **Porque**: um descompasso futuro vira panic no fluxo de criação de temporada.

---

## A10. Frontend, estado e telas

Fechados: os três stubs vazios (`useTauri.js`, `useUIStore.js`, `useNotificationStore.js`) foram
deletados, o guard `invoke-contra-generate-handler.test.mjs` fechou o contrato dos nomes de
comando, e as listas de reset do store viraram uma lista única em `helpers.js:29` com teste
próprio em `contextoDeTela.test.js`.

### A10.1 Settings.jsx ainda é a maior tela `(parcial)`
- **Onde**: `src/pages/Settings.jsx`, 705 linhas
- **Hoje**: caiu de 858 para 705 linhas, de 23 para 7 `useState` e de 18 para 4 `invoke`. O teste
  continua com 190 linhas.
- **Muda**: extrair os blocos restantes (backups, demo do overlay) para hooks e ampliar o teste.
- **Porque**: o encolhimento foi real e a cobertura não acompanhou.

### A10.2 Duas abas com versão dupla viva
- **Onde**: `src/pages/tabs/myteam/index.js:12`, `src/pages/tabs/atlas/index.js:12`
- **Hoje**: `MY_TEAM_VERSION` e `ATLAS_VERSION` fixos em 2. As v1 continuam compiladas, com testes
  grandes rodando (`MyTeamTab.test` 1.257 linhas, `GlobalTeamsTab.test` 869) e chaves de i18n em
  paridade.
- **Muda**: decidir a data de morte das v1 e remover.
- **Porque**: cerca de 2,5 mil linhas de manutenção protegendo tela desligada.

### A10.3 Cache de pré-corrida espelha a lista de comandos à mão
- **Onde**: `src/stores/career/preRaceCacheSlice.js` contra
  `src/components/race/nextrace/useBriefingData.js`
- **Hoje**: o slice pré-busca uma lista de comandos que precisa espelhar manualmente a do hook. As
  duas já divergem em contagem.
- **Muda**: uma fonte só para a lista.
- **Porque**: se a Sala de Estratégia pedir comando novo, o cache fica incompleto em silêncio e o
  flash que ele existe para evitar volta.

### A10.4 Páginas grandes sem nenhum teste
- **Onde**: `src/pages/MainMenu.jsx` (810 linhas), `src/pages/tabs/CalendarTabRedesign.jsx`,
  `src/pages/tabs/NewsMagazineTab.jsx`, `src/pages/tabs/myteam/MyTeamTabV2.jsx`
- **Hoje**: MainMenu carrega intro, lista de saves e exclusão de carreira, mais constantes mágicas
  de layout, sem guard visual nem teste.
- **Muda**: teste de comportamento para MainMenu (exclusão de carreira em primeiro lugar) e para
  a aba de calendário.
- **Porque**: exclusão de carreira é a ação mais destrutiva da UI e não tem rede.

### A10.5 `invoke` direto em componentes
- **Onde**: `src/pages/Settings.jsx`, `src/pages/NewCareer.jsx` (7), `src/pages/Dashboard.jsx`,
  `src/pages/MainMenu.jsx`
- **Hoje**: o padrão declarado no CLAUDE.md põe `invoke` nos slices ou em hooks `use*`. Essas
  quatro telas chamam inline, sem cache nem tratamento uniforme de erro.
- **Muda**: mover para hooks de dados.
- **Porque**: erro de ponte nessas telas cai em `catch` vazio e some.

### A10.6 `seasonSlice` mistura três assuntos
- **Onde**: `src/stores/career/seasonSlice.js`, 458 linhas
- **Hoje**: virada de temporada, animação do calendário e o Bloco Especial marcado como legado
  para saves pré-v33 (cerca de 215 linhas).
- **Muda**: separar o legado num slice próprio até os saves antigos poderem morrer.
- **Porque**: metade do arquivo é código que só roda para save que talvez não exista mais.

### A10.7 Strings de debug em português fora do `t()`
- **Onde**: `src/pages/Dashboard.jsx` (3 ocorrências de `DEBUG:`)
- **Hoje**: os flashes dos hotkeys Ctrl+M/L/K são texto PT hardcoded renderizado no overlay. O
  guard de i18n não pega por serem argumento de função.
- **Muda**: passar por `t()` ou marcar como texto de desenvolvimento.
- **Porque**: é texto de UI fora do padrão, visível ao jogador que apertar a tecla.

### A10.8 Utilitários puros sem teste
- **Onde**: `src/utils/calendarShared.js`, `src/lib/filaDeCarga.js`, `src/utils/trackBanners.js`
- **Hoje**: `filaDeCarga` é o semáforo que protege o acervo de voz contra
  `ERR_INSUFFICIENT_RESOURCES` e tem 44 linhas puras, sem teste.
- **Muda**: teste dos dois puros. `sfx.js` e `filtroRadio.js` seguem aceitáveis sem cobertura por
  dependerem de Web Audio.
- **Porque**: o semáforo é o que impede o navegador derrubar o rádio inteiro sob carga.

---

## A11. Componentes e overlay

Fechados: o guard `vr-overlay-contrato-dimensoes.test.mjs` fechou o contrato VR_W/VR_H entre o
JS, o `shared_frame.h` e o `vr_overlay.rs`; e `src/utils/categoryLogos.js` virou a fonte única do
brasão de categoria, que vivia copiado em cinco lugares.

### A11.1 `DriverDetailModalV2` com 5.307 linhas
- **Onde**: `src/components/driver/v2/DriverDetailModalV2.jsx`
- **Hoje**: caiu 198 linhas. Continua com dezenas de funções internas, 27 hooks e mais de 60
  símbolos importados de `curvaDeCarreira`, protegido por um teste de 4.482 linhas.
- **Muda**: extrair as peças de gráfico (balões, faixas, molduras) para módulos irmãos, como
  `detalhes/` já faz.
- **Porque**: o teste gigante protege o comportamento e trava qualquer refatoração barata.

### A11.2 `TeamHistoryDrawerV2` com 4.400 linhas
- **Onde**: `src/components/team/v2/TeamHistoryDrawerV2.jsx`
- **Hoje**: 63 funções internas atrás de um único export.
- **Muda**: extrair o miolo de rótulos e a seleção de melhores resultados, seguindo o precedente
  de `atlasV2Geometry.js` e `gridMetrics.js`.
- **Porque**: o padrão de extração já existe no próprio diretório e não foi aplicado aqui.

### A11.3 Painéis do iRacing ainda quase sem teste `(parcial)`
- **Onde**: `src/components/iracing/` (3 arquivos de teste para 10 fontes)
- **Hoje**: `RosterGenPanel` (726 linhas, 12 invokes), `PostRacePanel` (690) e
  `IracingConnectedOverlay` (624) são a ponte exportar, correr e importar.
- **Muda**: teste de contrato para os DTOs que as três consomem.
- **Porque**: mudança de DTO no Rust quebra essas telas sem nenhuma rede.

### A11.4 Árvore V1 morta ainda no repositório
- **Onde**: `src/components/race/RaceResultView.jsx` e `race/raceresult/` (zero importadores),
  `src/components/driver/DriverDetailModal.jsx`, `src/components/team/TeamHistoryDrawer.jsx`
- **Hoje**: o `RaceResultView` v1 não tem nenhum importador vivo e o subdiretório inteiro carrega
  `i18n-ignore-file`. Os outros dois seguem atrás de seletor de versão.
- **Muda**: apagar o `raceresult/` inteiro agora; decidir a data dos outros dois.
- **Porque**: é código morto que ainda escapa do guard de i18n.

### A11.5 Prosa PT em módulos `.js` escapa do auditor
- **Onde**: `src/components/season/preSeasonFormatters.js`,
  `src/components/driver/globalDriverRanking.js` (8 ocorrências)
- **Hoje**: o `i18nAudit.mjs` varre só `.jsx` por decisão documentada. `DEFAULT_FILTERS` usa
  "Todos"/"Todas" como valor-sentinela de filtro e texto de UI ao mesmo tempo.
- **Muda**: separar o sentinela do rótulo e mandar o rótulo para o i18n.
- **Porque**: acopla estado a idioma. Trocar o locale quebra o filtro.

### A11.6 Backend entrega valor de domínio em português como contrato
- **Onde**: `src/components/race/raceFactsContext.js`
- **Hoje**: `INJURY_SEVERITY_KEY` mapeia "Leve", "Moderada", "Grave" e "Critica" vindos do campo
  `lesao_ativa_tipo` do banco para chaves i18n, incluindo a grafia sem acento de "Critica".
- **Muda**: enum estável do lado Rust e tradução na borda.
- **Porque**: uma correção de acentuação no backend quebra a gravidade da lesão em silêncio.

### A11.7 Cadeia PTT e rádio do overlay sem teste
- **Onde**: `src/overlay/EngineerRadio.jsx`, `EngenheiroPttAuto.jsx`, `OverlayPositionPanel.jsx`
- **Hoje**: o diretório subiu de 4 para 8 arquivos de teste em 34 fontes, e essas três continuam
  descobertas. `OverlayPositionPanel` persiste pose em localStorage e espelha no backend, com
  poses de fábrica calibradas na mão.
- **Muda**: testar a fusão de feeds do `EngineerRadio` e a persistência de pose.
- **Porque**: é a superfície usada em corrida e em VR, onde bug aparece na pior hora.

### A11.8 Lógica pura grande sem teste
- **Onde**: `src/components/team/worldTeamChartGeometry.js` (669),
  `src/components/season/preSeasonFormatters.js` (700), `driver/v2/curvaDeCarreira.jsx` (1.121),
  `driver/v2/CurvaDeCampeonato.jsx` (842)
- **Hoje**: funções puras sem teste, na contramão dos pares `atlasV2Geometry` e `gridMetrics`, que
  têm teste do mesmo tamanho do fonte.
- **Muda**: teste espelho para as duas geometrias.
- **Porque**: são cálculos determinísticos, o tipo mais barato de cobrir e o mais caro de depurar
  na tela.

### A11.9 `Header` acumula orquestração
- **Onde**: `src/components/layout/Header.jsx`, 754 linhas
- **Hoje**: cerca de 20 seletores do store, invoke próprio de campeão e a regra do clique extra
  que desvia pelas notícias no fim do campeonato.
- **Muda**: extrair o fluxo para um hook.
- **Porque**: é lógica de fluxo de temporada morando num componente de layout.

### A11.10 Posição de fábrica do rádio espelha o `tauri.conf.json` à mão
- **Onde**: `src/overlay/EngineerRadio.jsx`
- **Hoje**: `FACTORY_POS` precisa espelhar o x/y da janela `engineer` no conf, segundo o próprio
  comentário.
- **Muda**: guard estrutural lendo os dois, no molde do que foi feito para o VR.
- **Porque**: mudança no conf sem tocar aqui desloca o padrão de fábrica de quem nunca moveu a
  janela.

---

## A12. As três suítes de teste

Fechados: o guard de nomes de `invoke` contra o `generate_handler` existe, o guard de acentuação
foi reescrito para varrer o locale inteiro em vez de 11 arquivos listados (e a reescrita revelou
29 strings sem acento que passavam com o guard verde), e os dois guards quase homônimos viraram
`career-command-shell-structure` e `career-types-module-structure`.

### A12.1 `commands/iracing/` sem teste
- Ver A5.3. É o mesmo buraco, contado nas duas áreas pela vistoria.

### A12.2 Instabilidade conhecida no historical_draft
- **Onde**: `src-tauri/src/commands/historical_draft/tests/mod.rs`
- **Hoje**: 4 falhas pré-existentes, 1 flaky e 3 harnesses sob `#[ignore]`. O teste do nascimento
  no gt3 mede quem nunca correu, que é o alvo errado.
- **Muda**: passe de saneamento separando regressão de ruído, e corrigir o alvo do teste do gt3.
- **Porque**: enquanto a área falha por padrão, nenhuma verificação da suíte é confiável.

### A12.3 Slices do career store sem teste direto `(parcial)`
- **Onde**: `src/stores/career/` (1 arquivo de teste para 7 fontes)
- **Hoje**: nasceu o `contextoDeTela.test.js` cobrindo o reset. Os `invoke` dos slices continuam
  exercitados só com mock.
- **Muda**: teste por slice, com o mock verificando os nomes de comando.
- **Porque**: o guard de invoke pega nome inexistente. Campo renomeado dentro do DTO ele não pega.

### A12.4 Guards de contrato extraem chaves de Rust por regex `(parcial)`
- **Onde**: `scripts/tests/spotter-chaves-contrato.test.mjs` (4 asserções de mínimo),
  `quebra-pecas-contrato.test.mjs` (1), `engenheiro-pecas-do-app.test.mjs`
- **Hoje**: o triângulo Rust, `.opus` e JS é fechado lendo os `.rs` como texto. Os dois primeiros
  já asseveram um mínimo de chaves encontradas.
- **Muda**: estender a mesma asserção de mínimo ao terceiro guard.
- **Porque**: regex que passa a casar zero chaves faz o guard passar vazio.

### A12.5 Componentes inteiros sem teste
- **Onde**: `src/components/wizard/` (4 fontes, 0 testes)
- **Hoje**: standings subiu para 2 testes e calendar para 1. O wizard continua zerado.
- **Muda**: teste do funil de criação de carreira.
- **Porque**: é a primeira coisa que todo jogador novo atravessa.

### A12.6 Maior guard acoplado a detalhe de implementação
- **Onde**: `scripts/tests/driver-detail-modal.test.mjs`, 827 linhas
- **Hoje**: assevera nomes de variável, classes CSS e temporização de animação cruzando 4 arquivos
  por regex.
- **Muda**: reduzir ao que é contrato de verdade.
- **Porque**: hoje é espelho do código. Qualquer refactor do drawer exige editar o teste junto.

### A12.7 Teste de componente com 4.482 linhas
- **Onde**: `src/components/driver/v2/DriverDetailModalV2.test.jsx`
- **Hoje**: carrega 16% da suíte de UI sozinho.
- **Muda**: fatiar por seção do modal, seguindo o precedente do v1.
- **Porque**: rodar um caso custa o arquivo inteiro.

### A12.8 Locale global exige `#[serial]` e nada garante a disciplina
- **Onde**: cerca de 40 arquivos com `serial_test`
- **Hoje**: a regra vive só no CLAUDE.md.
- **Muda**: guard estrutural exigindo `#[serial]` em teste que chama `set_locale`.
- **Porque**: teste novo sem a marca contamina asserções de prosa em PT de forma intermitente, e
  a falha aparece em outro arquivo.

---

## A13. Trabalho não commitado

Fechados: `telemetry.rs` foi quebrado em `telemetry/` (`fila.rs`, `entrega.rs`, `uso.rs`) e o
`testar-quali-destruida.cmd` saiu da raiz para `scripts/`.

### A13.1 A árvore continua sem commit, e cresceu
- **Onde**: raiz do repositório
- **Hoje**: 194 arquivos com diff (+7.195/-4.049) e cerca de 25 não rastreados, contra 103
  modificados na vistoria. Nenhum commit novo desde `7651564`.
- **Muda**: fatiar em commits temáticos. As frentes visíveis hoje: correções da vistoria por área,
  reescrita do guard de acentuação, cache da torre, enums do race_monitor, módulo de telemetria,
  migração v62, i18n do dossiê do rádio.
- **Porque**: é a proteção mais barata da lista inteira. A memória do projeto registra que a
  árvore do Loop não é commitada e que um `git checkout` já apagou trabalho real. Com 194
  arquivos, bisect e reversão são impossíveis.

### A13.2 Regra da quali destruída ainda armada por env var `(parcial)`
- **Onde**: `iracing_sdk/race_monitor.rs:252` e `:263`
- **Hoje**: virou `enabled || std::env::var(QUALI_WRECK_ENV).is_ok()`, então já existe o caminho
  por configuração e a env var continua como atalho.
- **Muda**: decidir o destino da env var antes do release e registrar a calibração dos tiers de
  severidade e dos segundos de reparo.
- **Porque**: os limiares que definem grave, destruído e DQ são números novos sem medição escrita
  em lugar nenhum.

### A13.3 Artefatos soltos na raiz
- **Onde**: `__curva_preview.html`, `__fita-equipes-preview.html`, `__mercado-card-preview.html`,
  `fita-mock.html`, `preview-curva-campeonato.html`, `.tabela-equipes.txt`
- **Hoje**: seis previews de UI acumulando na raiz do repositório.
- **Muda**: cada um recebe veredito: `.gitignore`, mover para `scripts/`, ou apagar.
- **Porque**: raiz de repositório é a primeira coisa que alguém lê.

### A13.4 Pintura e modo janela aplicados sem perguntar
- **Onde**: `src/components/race/nextrace/useIracingExport.js`, `commands/iracing/pintura.rs`,
  `iracing_sdk/modo_janela.rs`
- **Hoje**: os dois modais de confirmação foram deletados e o comportamento virou automático, com
  aviso único por save em localStorage. O app mexe em arquivo de configuração do iRacing do
  jogador.
- **Muda**: confirmar que existe caminho de desfazer nas Configurações e definir o comportamento
  quando o iRacing está aberto.
- **Porque**: é mudança de contrato com o jogador em cima de arquivo que não pertence ao Loop.

### A13.5 Fluxo de exportação sem teste após a reescrita
- **Onde**: `src/components/race/nextrace/useIracingExport.js` (intacto desde a vistoria),
  `src/overlay/EngenheiroPttAuto.jsx`
- **Hoje**: 161 linhas mexidas, com timer de toast fora do `dismissTimers`, e nenhum arquivo de
  teste.
- **Muda**: teste do fluxo de exportação e da limpeza dos timers.
- **Porque**: o próprio comentário admite que a limpeza errada prenderia o toast na tela.

### A13.6 `normalize_to_roster` com casos degenerados por convenção
- **Onde**: `src-tauri/src/iracing_sdk/roster_gen.rs` (intacto)
- **Hoje**: grid de um piloto ou empate exato produz banda `min+1.0` e skill 50; lista vazia
  devolve banda 0 a 1.
- **Muda**: teste explícito no consumidor, que é quem escreve `minSkill`/`maxSkill`.
- **Porque**: são escolhas razoáveis que precisam envelhecer junto nos dois lados.

### A13.7 Scrub de meta-linguagem em dupla camada
- **Onde**: `narrative/client.rs`, `commands/ai_news/fatos.rs`
- **Hoje**: uma lista de frases ASCII hardcoded, porque fatos antigos ficam persistidos no save
  com a redação velha. A ordenação por tamanho é frágil de manter na mão.
- **Muda**: migração dos fatos persistidos, e a camada sai.
- **Porque**: a lista só cresce.

### A13.8 Novos `.opus` binários entrando no git
- **Onde**: `src/assets/engenheiro/`
- **Hoje**: o acervo está em 3.948 peças e 373 MB, e cada leva nova entra no repositório.
- **Muda**: decidir se assets de voz continuam no git ou migram para artefato de release.
- **Porque**: cada clone paga o acervo inteiro.

---

## A14. Dívida documentada

Fechados: `docs/divida-tecnica.md` foi reescrito (132 linhas) e passou a declarar a divisão com o
backlog, encerrando a contradição do "nenhuma pendência". Os fechamentos de F-06, R3, três dos
quatro casos do R5, os bugs #1, #2 e #3 e os briefings F1 a F4 estão registrados. Os stubs D-03 e
D-04 foram resolvidos por remoção.

### A14.1 R4: hierarchy com estado rico e sem consequência `(parcial)`
- **Onde**: `hierarchy/orders.rs`; consumidores em `commands/race/persistencia.rs`,
  `commands/career/market_window.rs` e `db/queries/contracts/escrita.rs`
- **Hoje**: ganhou um terceiro consumidor. Tensão, duelos e status continuam sem realimentar
  mercado, narrativa ou motivação.
- **Muda**: ligar a tensão em pelo menos um consumidor de consequência, depois de A2.2.
- **Porque**: o roadmap coloca isso como bloqueador antes de encostar em hierarquia. Ligar antes
  de recalibrar propaga um eixo morto.

### A14.2 Dezesseis comandos do iRacing inalcançáveis
- **Onde**: `docs/iracing-escopo.md` §6, `docs/roadmap.md`
- **Hoje**: 9 comandos sem consumidor e 7 presos em `RosterGenPanel` e `PostRacePanel`. O mais
  caro é `iracing_process_race_result`, a dificuldade adaptativa implementada e nunca executada.
- **Muda**: dar caminho de UI aos que valem, e apagar os que não valem.
- **Porque**: o roadmap marca como primeiro item pós decisão de escopo, e nada indica execução.

### A14.3 Bug #4: `is_crash` perdeu a severidade
- **Onde**: `src-tauri/src/race_signals.rs:170`
- **Hoje**: `IncidentType::DriverError => DnfKind::Erro`, sem olhar `severity`. A regra antiga
  excluía DriverError com severidade Minor de "batida" e o caso de teste `erro_leve` foi apagado.
- **Muda**: confirmar no motor de incidentes se Minor com DNF é possível, e reintroduzir a regra
  ou registrar a decisão de removê-la.
- **Porque**: a investigação que o doc pedia segue sem resposta, e o sinal alimenta a narrativa.

### A14.4 Bug #5: duas recalibrações sem nota
- **Onde**: `event_interest/calculator.rs` (positional_bonus contínuo, 0.4 com clamp -3 a 4),
  `race_signals.rs` (`REMONTADA_MIN = 4`, antes 5 no debrief)
- **Hoje**: as duas estão vigentes e podem ser intencionais.
- **Muda**: uma nota de calibração fechando cada uma.
- **Porque**: sem o registro, a próxima varredura levanta as duas de novo como achado novo.

### A14.5 `run_market` vivo só pelos próprios testes
- **Onde**: `src-tauri/src/market/pipeline.rs:84`
- **Hoje**: nenhuma chamada fora de `tests/`. Os outros três casos do R5 já sumiram do crate.
- **Muda**: confirmar se é o motor interno de `initialize_preseason` antes de tratar como legado.
- **Porque**: é falsa cobertura. O teste exercita um caminho que a produção não percorre.

### A14.6 D-01: convocação legada
- **Onde**: `src-tauri/src/convocation/` (9 arquivos)
- **Hoje**: o diretório inteiro segue vivo.
- **Muda**: confirmar que nenhum save ativo usa as fases BlocoRegular, JanelaConvocacao,
  BlocoEspecial e PosEspecial, e remover.
- **Porque**: é o maior bloco de código de fase antiga ainda compilado.

### A14.7 D-02: tabela `races` coexistindo com `calendar`
- **Onde**: `db/migrations/baseline.rs` (CREATE TABLE races), `db/queries/races.rs` (INSERT ativo)
- **Hoje**: duas fontes de verdade para o conceito de corrida.
- **Muda**: escolher uma.
- **Porque**: qualquer consulta nova precisa saber em qual das duas confiar.

### A14.8 D-05: `advance_transfer_window` sem consumidor
- **Onde**: `src-tauri/src/lib.rs`, ausente de `src/stores/career/marketSlice.js`
- **Hoje**: zero chamadas no frontend. É a peça central do F-01, o mercado fora da janela.
- **Muda**: ligar quando o F-01 for feito, ou remover.
- **Porque**: comando registrado e inalcançável é dívida que se acumula em silêncio.

### A14.9 Bug #6 parte 2: estimativa de voltas pela melhor volta do campo
- **Onde**: `src-tauri/src/commands/overlay/torre.rs`
- **Hoje**: dividir tempo restante pela melhor volta absoluta subestima sistematicamente o total.
  A parte 1 caiu: o gate por prova cronometrada está certo, porque `SessionLapsRemainEx` vem
  sentinelado.
- **Muda**: usar a mediana das últimas voltas do líder.
- **Porque**: o número aparece na torre que o jogador olha durante a corrida.

### A14.10 R1: `allow(dead_code)` do narrative
- **Onde**: `src-tauri/src/narrative/mod.rs:1`
- **Hoje**: a Etapa B foi ligada (o `persistencia.rs` monta injury_facts, context_facts e
  career_beats) e o `allow` de arquivo continua na linha 1.
- **Muda**: remover o `allow` e tratar o que aparecer.
- **Porque**: era exatamente o item que o briefing mandava reavaliar depois da ligação.

### A14.11 D-06, D-07 e D-08: TODOs de design e migração
- **Onde**: `constants/tracks/consultas.rs` (posse de pistas, ver A9.3),
  `calendar/generator.rs` (2 venues ausentes do banco), `models/driver.rs` (TODO de migração)
- **Hoje**: os três intactos.
- **Muda**: veredito em cada um.
- **Porque**: TODO sem dono envelhece até virar arqueologia.

### A14.12 Árvore de diretórios vazia dentro do crate
- **Onde**: `src-tauri/src/src-tauri/src/evolution/pipeline/tests/`
- **Hoje**: existe.
- **Muda**: `rmdir`. O README da varredura já autoriza a remoção direta.
- **Porque**: custo de um comando.

### A14.13 F-01, F-02 e F-07: buracos de produto
- **Onde**: `docs/backlog.md`, `docs/roadmap.md`
- **Hoje**: mercado fora da janela, ficha do próprio piloto e UI de espectadores seguem abertos,
  com backend pronto nos três. F-03, F-04 e F-05 continuam abertos e o roadmap manda tratá-los
  juntos numa tela só.
- **Muda**: trabalho de frontend, na ordem que o roadmap sugere.
- **Porque**: hoje o jogador se vê pela mesma lente de qualquer piloto de IA.

---

## O que a vistoria nunca leu

Esta parte não estava na conta dos 179. São áreas do repositório que nenhum dos 14 leitores abriu,
levantadas em 11/08/2026. Não são achados: são pontos cegos.

- **Comandos Tauri fora da carreira**, cerca de 20 mil linhas de produção em 20 módulos:
  `career_detail` (5.800), `career_team_dossier` (4.450), `global_driver_rankings` (2.793),
  `season_preview` (1.084), `global_team_history` (944), `world_footer` (873), `save` (562, o
  backend dos backups do F-06), `convocation` (388), mais `tts_poc`, `vr_overlay`, `vr_layer`,
  `engenheiro`, `transfer_market`, `overlay_window`, `race_history`, `inbox`, `volante`,
  `calendar`, `debug_capture`. A vistoria só os citou como agregados do `career_commands.rs`.
- **O layer VR nativo**: `vr-overlay/src/overlay_layer.cpp` (931 linhas) e `shared_frame.h` (82).
  É o outro lado do contrato de dimensões que a área 11 marcou como Alta.
- **`radio_registro.rs`** (503 linhas), sem menção em nenhuma área.
- **Ferramental**: 21 scripts `.mjs`, 6.762 linhas, incluindo `release.mjs` (assina e publica),
  `engenheiro-pack` e `spotter-pack` (montam o acervo de voz), `gen-track-countries` (gera código
  que o front consome) e `build-vr-layer`. A área 12 leu apenas `scripts/tests/`.
- **Build e CI**: `build.rs`, `tauri.conf.json`, `ci.yml`, `.githooks/pre-commit`,
  `.cargo/config.toml`, `vite.config.js`.
- **Docs**: 136 arquivos `.md` em `docs/`, dos quais a área 14 conferiu 5. `DESIGN.md`,
  `iracing-escopo.md`, `iracing-dados-disponiveis.md` e `i18n-translation-spec.md` ficaram de fora.

---

## Ordem de ataque revisada

1. **Commitar.** A13.1. São 194 arquivos de trabalho real sem rede. Nada nesta lista protege
   tanto por tão pouco.
2. **Os dois bugs de dinheiro.** A7.2 (o ralo dormente com a regalia viva) e A7.1 (sobrecusto de
   enduro sem teto). São os únicos achados abertos que corrompem o estado do save em vez de só
   incomodar.
3. **A4.3**, o gate dos comandos de debug. Uma linha de `cfg`, e fecha a porta de corromper
   carreira real.
4. **Cobrir `commands/iracing/`** (A5.3), começando pelas funções puras. É o caminho principal do
   jogo e a única área grande com zero teste.
5. **Fechar os contratos que sobraram**: A3.1 (severidade como string) e A11.6 (gravidade de lesão
   em PT como contrato). O trabalho de enums já começou e parou no meio.
6. **Calibrar**: A2.2 (eixo N1/N2, já com a receita escrita no arquivo), A1.1 (tráfego),
   A1.6 (pressão), A7.6 (estilo de pilotagem).
7. **Decidir os cortes**: A10.2 e A11.4 (v1 do frontend), A1.3 (legados do motor), A2.9 (env
   flags), A14.6 (convocação legada).
8. **Ler o que nunca foi lido**, a seção acima. Os 20 mil linhas de comandos são o maior ponto
   cego, e `career_team_dossier` é o maior arquivo sem pasta de testes do conjunto.


