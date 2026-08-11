# Vistoria do código, 10/08/2026

Varredura de reconhecimento sobre todos os subsistemas do Loop, feita com 14 leitores independentes, um por área. Este documento é o mapa do que merece revisão, com prioridade e motivo, para nada passar batido. A revisão em si fica para depois, frente a frente.

Saldo: 179 pontos de revisão (38 Alta, 84 Média, 57 Baixa) em 14 áreas.

Como ler cada área: o resumo diz o que o subsistema faz, o bloco de testes diz onde a cobertura segura e onde não segura, e a lista traz os pontos. Alta significa risco real de bug, contrato frágil ou decisão pendente que trava outras. Média é manutenção que evita bug futuro. Baixa é higiene.

A última área (dívida já documentada) confere os docs de dívida contra o código de hoje, para separar achado novo do que já estava registrado.


## Motor de corrida (simulation/, race_eval, race_signals, player_skill, sim_stats)

Transforma grid mais contexto em resultado de fim de semana: classificação como melhor de N tentativas, corrida em 5 segmentos com ruído AR(1), ar sujo, ultrapassagem como tentativa, trem de carros, safety car, paradas planejadas, incidentes sorteados, quebras de peça pré-roladas, lesões e pontuação. Uma esteira de modificadores puros (conhecimento de pista, adaptação de categoria, forma em três camadas, pressão de título, rivalidade) ajusta cada piloto antes de entrar no motor. Na saída, race_eval traduz o resultado do jogador em significado de carreira e race_signals é a definição única dos sinais narrativos. calibracao/ e sim_stats/ são réguas de medição, sem efeito no jogo.

Componentes:

- simulation/engine.rs (~135 linhas): Orquestra o fim de semana: quali, corrida, volta rápida, pontos
- simulation/qualifying.rs (~1275 linhas): Sábado: melhor de N tentativas, volta perdida, pesos por caráter de pista; carrega caminho legado para prova de equivalência
- simulation/race/ (motor, pontuacao, trafego, tipos, estrategia, resultados, danos) (~2252 linhas): O domingo: laço de 5 segmentos, ruído correlacionado, ar sujo, ultrapassagem, trem, safety car, paradas e fechamento do resultado
- simulation/incidents/ (segmento, sorteios, risco, tipos) (~788 linhas): Sorteio de pane, erro e colisão por segmento, dano latente pós-contato
- simulation/injuries.rs (~273 linhas): Lesões derivadas de incidentes, nomes via rust-i18n congelados no save
- simulation/scoring.rs (~124 linhas): Pontos por posição e bônus de volta mais rápida
- simulation/esteira.rs (~591 linhas): Esteira de modificadores pré-corrida como função pura, com medição da perda por quantização u8
- simulation/forma.rs (~569 linhas): Três camadas intermediárias: afinidade piloto x pista por hash, forma AR(1) persistida, acerto de fim de semana por equipe
- simulation/pressure.rs (~675 linhas): Clutch/choke por contexto de título, pressão de evento, rivalidade e motivação
- simulation/track_knowledge.rs + category_adaptation.rs (~295 linhas): Penalidades de aprendizagem de pista e de categoria, fonte única com o export pro iRacing
- simulation/profile/ (lap_times, base, resolucao, pista) (~1124 linhas): Perfil de simulação por categoria x pista; tabela hardcoded de 618 tempos base
- simulation/track_profile.rs + car_build.rs + math.rs + context.rs + catalog.rs (~1943 linhas): Dados canônicos de pista, shape do carro, curvas de chuva, DTOs SimDriver/SimulationContext e catálogo narrativo de incidentes
- simulation/calibracao/ (~5600 linhas): Régua: campo sintético, arena, métricas, decomposição de variância, varredura e busca de knobs, alvos e relatório
- race_eval.rs (~438 linhas): Avaliação pós-corrida do jogador: potencial, meta, nota 0-10 e frases via rust-i18n
- race_signals.rs (~304 linhas): Definição única de remontada, colapso, caos e natureza do DNF, um limiar por conceito
- player_skill.rs (~891 linhas): Dossiê de atributos do jogador inferido do desempenho real, só exibição
- sim_stats/ (cfg(test)) (~2500 linhas): Harness Monte Carlo: roda carreiras inteiras e agrega lesões, evolução, aposentadorias e promoções

Testes: Cobertura forte e acima da média do projeto: cerca de 9,9 mil linhas de teste na área. Suítes dedicadas em race/tests (2.760 linhas, incluindo medição), incidents/tests (799), calibracao/tests (2.551), forma/tests (787), esteira/tests (655), mais testes inline em qualifying (~875), race_eval, race_signals, player_skill, pressure, scoring, math, context, catalog, injuries, car_build, category_adaptation, track_knowledge e track_profile. Buracos: race/motor.rs, race/danos.rs e race/resultados.rs dependem só da suíte central de race/tests, sem testes de unidade próprios das funções internas (atraso sob safety car, execução de parada, dano latente); calibracao/busca.rs (1.001 linhas) idem via tests/mod.rs central. Os testes que asseveram prosa PT usam #[serial] corretamente. O sim_stats é coletor, sempre passa, e não protege invariantes.

Pontos de revisão:

- [Alta] simulate_race_com_modo concentra a corrida inteira numa função de ~560 linhas
  - Arquivos: src-tauri/src/simulation/race/motor.rs:281
  - Motivo: Um único laço de segmentos acumula incidentes, quebras, ar sujo, ruído, ultrapassagem, trem, paradas, safety car e retratos de tráfego, com estado mutável compartilhado (states, ordem_de_entrada, gap_de_entrada, ritmo_limpo, fechamento_no_trecho) atravessando todas as fases. Qualquer mudança local exige reler o laço todo. Extrair cada fase para função nomeada preservaria a sequência de sorteios do rng se feito com cuidado.
- [Alta] Constantes de tráfego e ultrapassagem declaradas provisórias e nunca calibradas
  - Arquivos: src-tauri/src/simulation/race/trafego.rs:33
  - Motivo: O próprio bloco diz que nenhuma foi calibrada (JANELA_AR_SUJO_MS=1000, PROB_BASE_ULTRAPASSAGEM=0.35, GAP_MINIMO_ENTRE_CARROS_MS=150, custos de tentativa falha, pesos de rivalidade). A máquina de busca existe em calibracao/busca.rs e o fechamento está pendente. Enquanto isso, essas constantes ditam quanto vale largar na frente.
- [Alta] Prosa PT hardcoded vira dado de classificação de DNF
  - Arquivos: src-tauri/src/simulation/race/motor.rs:622, src-tauri/src/race_signals.rs:72
  - Motivo: O contato na ultrapassagem grava descrição em português fixo ("bate ao tentar passar") fora do rust-i18n, persistida no save, e race_signals reclassifica o abandono por listas de palavras-chave em PT e EN sobre esse texto. Mudar a frase degrada dnf_kind para Desconhecido em silêncio. O incidente estruturado tem precedência quando disponível, então o buraco é o caminho de save antigo e de texto livre.
- [Média] Três tabelas paralelas indexadas por track_id sem guard cruzado
  - Arquivos: src-tauri/src/simulation/profile/lap_times.rs:14, src-tauri/src/simulation/track_profile.rs:56, src-tauri/src/constants/tracks/
  - Motivo: 618 combinações de tempo base num match de 845 linhas, mais o match de identidade esportiva em track_profile.rs e as constantes de pista. Adicionar pista exige editar os três; a ausência em lap_times cai num fallback por comprimento_km que esconde o esquecimento. Um teste que cruze a cobertura das três tabelas pegaria isso.
- [Média] Caminhos legados vivos dentro do motor e da quali
  - Arquivos: src-tauri/src/simulation/race/motor.rs:216, src-tauri/src/simulation/qualifying.rs:201
  - Motivo: ModoDoMotor::LegadoDePontos e ConfigQuali::legada/qual_weights_legados existem só para a prova de equivalência bit-exata e para o harness. São ramos duplicados dos pesos e do sorteio que precisam de manutenção sincronizada. Vale decidir o prazo de remoção agora que a prova passou.
- [Média] Buscas lineares por id repetidas dentro do laço de segmentos
  - Arquivos: src-tauri/src/simulation/race/motor.rs:302,381,408,451,475,576
  - Motivo: drivers.iter().find e states.iter_mut().find rodam por carro por segmento, dando custo quadrático no tamanho do grid. Com 30 carros é barato; no harness e na calibração, que rodam milhares de corridas, soma. Um índice HashMap montado uma vez por corrida elimina.
- [Média] Números mágicos internos dos sorteios ficam fora do espaço de calibração
  - Arquivos: src-tauri/src/simulation/incidents/sorteios.rs:25, src-tauri/src/simulation/race/pontuacao.rs:26
  - Motivo: Pivô de confiabilidade 70 com escala /25*0.70, 15% de chance de pane leve, absorção de chuva 0.80 e os pesos de atributo por segmento são valores fixos que a varredura de knobs de calibracao/ não enxerga. Se a busca fechar os knobs externos com esses internos errados, o erro fica travado por baixo.
- [Média] Curvas do dossiê do jogador são chutes sem validação
  - Arquivos: src-tauri/src/player_skill.rs:344
  - Motivo: aggression = 15 + 16 por incidente, adaptabilidade a 5 pontos por posição de melhora, experiência saturando em 60 corridas: heurísticas documentadas como aproximação e sem medição contra a distribuição real. É só exibição, o mercado não consulta, então o risco é o dossiê mentir para o jogador.
- [Média] Vinte e tantas constantes de pressão travadas por decisão e não por medição
  - Arquivos: src-tauri/src/simulation/pressure.rs:17
  - Motivo: NEUTRAL=0.55, PACE_K=3.0, multiplicadores x2 de líder e x3 de última corrida, CHASER_NEUTRAL_SHIFT=0.07 foram fixados em design e a esteira agora permite medi-los pelo harness. A camada afeta simulação offline e export, então um swing mal dimensionado distorce as duas pontas.
- [Baixa] Textos do catálogo de incidentes congelam no idioma de origem
  - Arquivos: src-tauri/src/simulation/catalog.rs:232
  - Motivo: select_and_render renderiza texto vindo do banco e a descrição persiste no save no idioma em que foi gerada, mesma opção A das lesões. Confirmar que o catálogo tem paridade de idiomas e que a UI aceita o congelamento como decisão, não como bug.
- [Baixa] Nota de corrida com pesos finos declarados como decisão do usuário
  - Arquivos: src-tauri/src/race_eval.rs:184
  - Motivo: Base 0.52/0.30/0.18 com teto 7.0 + 0.3 por posição absoluta: documentado, testado e com teste de inspeção manual ignorado. Só revisitar se a nota gerar reclamação de jogador, o desenho está saudável.
- [Baixa] Harness Monte Carlo com ruído de 8 a 10% entre execuções idênticas
  - Arquivos: src-tauri/src/sim_stats.rs:1, src-tauri/src/sim_stats/experimento/coleta.rs
  - Motivo: Todo o sim_stats é cfg(test) e cheio de expect, aceitável para coletor. O risco é de método: o agregado esconde tendência entre temporadas, então conclusões de calibração tiradas dele precisam de instrumentação por temporada antes de virarem mudança de constante.


## Mundo vivo entre temporadas (market, promotion, hierarchy, evolution, convocation, world, event_interest, fame)

É o subsistema que faz o mundo evoluir sozinho na virada de temporada. A orquestração central vive em evolution/pipeline/orquestracao.rs (run_end_of_season_with_mode), que roda numa única transação: standings e licenças, evolução dos pilotos (crescimento, declínio, motivação, aposentadoria), arquivamento de piloto e equipe, reputação, moral, vínculos e rivalidades, prêmios e ciclo de falência, promoção e rebaixamento de equipes em 3 blocos estruturais, criação da temporada seguinte com calendários e abertura da pré-temporada interativa. A pré-temporada avança semana a semana (market/preseason) com pré-passes de contrato, leilão de transferências (transfer_window), assédio, propostas ao jogador e a escada fechada que preenche vagas com promoções de baixo e rookies só na base. Convocation roda o bloco especial durante a temporada, e fame e event_interest alimentam salário, visibilidade e mídia dos dois lados.

Componentes:

- evolution/pipeline (orquestracao, pilotos, financas, transicao) (~2000 linhas): Orquestra a virada de temporada inteira numa transação: encadeia standings, evolução, arquivos, finanças, promoção e a abertura da pré-temporada. É o índice de encadeamento de todos os outros módulos da área.
- evolution (growth, decline, motivation, retirement, rookies, licenses, injury, standings, season_transition) (~3782 linhas): Crescimento e declínio por idade, motivação com dois passes (durante e pós-offseason), aposentadoria com paciência por talento e idade, geração de rookies sob demanda e calendários da temporada nova.
- market/pipeline (+ consolidacao, jogador, assedio, assedio_jogador, janela, vagas, contratacao, slam) (~7300 linhas): O mercado da entressafra em estágios (Contratos, Movimentos, Completo): renovação, rebaixamento por mérito, poaching IA e do jogador, janela de leilão e a cascata da escada fechada que garante grid cheio.
- market/preseason (~2400 linhas): Pré-temporada interativa semanal: plano persistido em JSON, pré-passes na saída da semana 1, mercado aberto em semana variável, feed de eventos e assentos reservados ao jogador.
- market/transfer_window (~1360 linhas): Motor puro do leilão de dois lados (ofertas, respostas, resultados, rollover), sem DB; o wiring fica em pipeline/janela.
- market (renewal, team_ai, driver_ai, poaching, slam_ambition, bond, visibility, evaluation, proposals, sync) (~3200 linhas): As decisões de IA do mercado: renovar ou dispensar, montar propostas, multa e leilão de poaching, meta de slam do piloto, vínculo piloto-equipe e visibilidade esportiva.
- market/car_maintenance (+ pit_strategy) (~2500 linhas): Cérebro de manutenção do carro por corrida (trocar, esticar, subir nível dentro do caixa) usado por IA e jogador; pit_strategy dá o teto de equipe de box por categoria.
- promotion (pipeline, block1/2/3, effects, pilots, standings) (~3453 linhas): Promoção e rebaixamento de equipes na escada fechada, em 3 blocos de pares estruturais, com soft-landing do carro do promovido, deltas de atributos e resolução da situação dos pilotos que sobem ou descem.
- hierarchy (orders, transition) (~1716 linhas): Hierarquia interna N1/N2: duelo por rodada, eixo de tensão, promoção interna e a transição entre temporadas (preservação parcial ou reset) com invariante validada no fechamento da janela.
- convocation (pipeline, eligibility, scoring, quotas, player_offers, special_window) (~4994 linhas): O bloco especial sazonal: coleta candidatos por 4 fontes, pontua e cota o grid, gera calendários especiais e conduz a janela de revelação com ofertas ao jogador.
- world (team_archive, integrity) (~1696 linhas): Arquivamento por temporada das equipes e consolidação do histórico de carreira (base do elite_score); auditoria de integridade usada pelo draft histórico.
- event_interest (calculator, public_impact) (~1396 linhas): Interesse esperado e realizado de cada evento (score por categoria, fase, rodada, contexto competitivo) e o impacto de mídia pós-corrida por piloto.
- fame.rs (~642 linhas): Fama e carisma: deltas por resultado, saturação com joelho, piso pessoal por conquistas, decaimento e a ponte comercial (unidades, interesse ativo de equipes, prêmio salarial).

Testes: Cobertura forte no geral. Suítes dedicadas: market/pipeline/tests (2.723 linhas), market/car_maintenance/tests (1.204), convocation/pipeline/tests (1.148), market/preseason/tests (973), evolution/pipeline/tests (953), market/transfer_window/tests (305). Quase todos os arquivos unitários têm módulo de teste inline (promotion completa, hierarchy, world, fame, event_interest, as unidades de evolution e as decisões de market). Higiene de erro excelente: todos os unwrap e expect da área estão confinados a código de teste, zero em caminho de produção. Buracos: evolution/season_transition.rs (calendários da temporada nova) sem teste direto, market/sync.rs e market/proposals.rs só com cobertura indireta, e os submódulos de convocation/special_window testados apenas de raspão pelos testes do pipeline de convocação.

Pontos de revisão:

- [Alta] Função de 511 linhas no coração da escada fechada
  - Arquivos: src-tauri/src/market/pipeline/consolidacao.rs:41
  - Motivo: fill_remaining_vacancies_with_rookies concentra num corpo só a cascata inteira: cotas reabastecidas em voo, promoção de feeder, recrutamento profundo, fallback de pool e promoções de emergência. É a garantia de grid cheio do modelo fechado e qualquer mexida exige entender o arquivo inteiro. Candidata a quebrar em etapas nomeadas como o resto do pipeline já faz.
- [Alta] Eixo de tensão N1/N2 com deltas sem calibração
  - Arquivos: src-tauri/src/hierarchy/orders.rs:157
  - Motivo: Os deltas fixos (+3 vitória do N2, -2 do N1, -1 de decaimento, +10/+15 de eventos) implicam que o N2 precisa vencer 40% dos duelos para o eixo se sustentar, e a medição do harness registrou 23% a 33%. A tensão tende a zero e o sistema fica inerte na prática. Recalibrar os deltas contra a taxa medida antes de construir qualquer coisa em cima.
- [Alta] Ordem da virada de temporada travada só por comentários
  - Arquivos: src-tauri/src/evolution/pipeline/orquestracao.rs:23
  - Motivo: run_end_of_season_with_mode encadeia mais de 15 passos acoplando evolution, market, world, finance, rivalry e promotion, com invariantes de ordem documentadas em prosa (o caso da rivalidade nascida e apagada no mesmo tique prova que a ordem já quebrou uma vez). Falta um teste que asserte a sequência ou um tipo que a torne impossível de violar.
- [Média] RNG semeado só com o número da temporada
  - Arquivos: src-tauri/src/market/preseason/semana.rs:22, src-tauri/src/market/pipeline/jogador.rs:575
  - Motivo: advance_week recria StdRng::seed_from_u64(season) a cada semana, então toda semana consome a mesma sequência aleatória; jogador.rs faz o mesmo com um xor constante. Decisões de semanas diferentes ficam correlacionadas. Misturar a semana na seed preserva o determinismo e remove a correlação.
- [Média] Leilão de assédio ao jogador concentrado e com DTO sensível
  - Arquivos: src-tauri/src/market/pipeline/assedio_jogador.rs:153
  - Motivo: compute_player_poach_offer_inner tem ~220 linhas e resolve_player_poach ~145; o PlayerPoachOffer cruza a ponte serde para o React e é a única tela em que o jogador decide um poaching, então mudança de campo quebra a UI em silêncio. Vale extrair etapas e cobrir o contrato com teste de shape.
- [Média] Constantes do event_interest sem trilha de calibração
  - Arquivos: src-tauri/src/event_interest/calculator.rs:18, src-tauri/src/event_interest/calculator.rs:82, src-tauri/src/event_interest/calculator.rs:179
  - Motivo: Escala x450 para display, multiplicadores 0.20/0.35/0.25 de pressão/mídia/motivação, cortes de tier em 85/65/45/25 e o score-base por categoria vivem sem const nomeada nem referência de medição, em contraste com fame.rs e retirement.rs que documentam a origem de cada número.
- [Média] Pesos de pontuação da convocação inline
  - Arquivos: src-tauri/src/convocation/scoring.rs:18
  - Motivo: Os pesos 0.45/0.25/0.10, a base 20.0 e a normalização pontos/200.0 estão espalhados nos corpos das funções por fonte de convocação, sem const nomeada e sem nota de calibração. Reequilibrar o grid especial hoje exige caçar literais.
- [Média] Docs defasados e allow(dead_code) no car_maintenance
  - Arquivos: src-tauri/src/market/car_maintenance.rs:1
  - Motivo: O cabeçalho diz que o wiring do tick pós-corrida vem no chunk 4 e que o legado car_build_strategy convive até o chunk 5; o wiring já existe (commands/race/despesa.rs:513, commands/career/lifecycle.rs:62, commands/iracing/roster.rs:627) e o legado só aparece citado no próprio doc. O allow(dead_code) de arquivo agora esconde código morto de verdade.
- [Média] with_savepoint triplicado
  - Arquivos: src-tauri/src/market/pipeline/comum.rs:7, src-tauri/src/promotion/pipeline.rs:92, src-tauri/src/promotion/pilots.rs:14
  - Motivo: Três implementações idênticas do mesmo helper de savepoint em módulos que rodam na mesma virada. Um utilitário em db/ elimina a divergência futura de tratamento de rollback.
- [Média] season_transition sem teste direto
  - Arquivos: src-tauri/src/evolution/season_transition.rs
  - Motivo: 318 linhas que criam a temporada nova e geram todos os calendários, sem módulo de teste próprio; a cobertura vem só de testes de integração do pipeline. Um erro de calendário aqui contamina a temporada inteira antes de qualquer corrida.
- [Média] Monolitos de teste dificultam navegação
  - Arquivos: src-tauri/src/market/pipeline/tests/mod.rs, src-tauri/src/market/car_maintenance/tests/mod.rs, src-tauri/src/convocation/pipeline/tests/mod.rs
  - Motivo: 2.723, 1.204 e 1.148 linhas num arquivo cada. O código de produção foi fatiado em etapas e os testes ficaram num bloco só; achar o teste de uma etapa custa caro e o arquivo tende a crescer sem dono.
- [Baixa] Estático global de instrumentação e comentário que mente
  - Arquivos: src-tauri/src/market/pipeline.rs:26, src-tauri/src/market/pipeline.rs:77
  - Motivo: EMERGENCY_PROMO_PATHS é um Mutex<Vec> global que só o harness sim_stats zera, então cresce sem teto numa carreira jogável longa; e o comentário da linha 77 pede para ligar os incrementos que já estão ligados em consolidacao.rs:504 e 538.
- [Baixa] allow(dead_code) de arquivo em três módulos
  - Arquivos: src-tauri/src/hierarchy/orders.rs:6, src-tauri/src/hierarchy/transition.rs:11, src-tauri/src/event_interest/public_impact.rs:1
  - Motivo: O atributo no topo do arquivo silencia o compilador para o módulo inteiro, então funções que perderem o último caller nunca serão apontadas. Trocar por allow pontual nos itens realmente pendentes.
- [Baixa] Flags de A/B por variável de ambiente em caminho de produção
  - Arquivos: src-tauri/src/promotion/pipeline.rs:29, src-tauri/src/market/pipeline/consolidacao.rs:14
  - Motivo: IRACER_PROMO_SOFT_LANDING, IRACER_ROOKIE_MERIT, IRACER_MARKET_AFFORDABILITY e a família IRACER_PROMO_DIMINISH_* mudam regra de jogo por env var lida a cada chamada. Útil para o Monte Carlo; merece um inventário central e decisão de quais viram default definitivo.
- [Baixa] sync.rs e proposals.rs sem teste próprio
  - Arquivos: src-tauri/src/market/sync.rs, src-tauri/src/market/proposals.rs
  - Motivo: sync_team_slots_from_active_regular_contracts é chamada em quase todo passo do mercado e proposals.rs define os DTOs do report; ambos só têm cobertura indireta pelos testes do pipeline.


## Integração com o iRacing real (src-tauri/src/iracing_sdk, 31,2k linhas)

O módulo fala com o iRacing rodando na máquina: abre o mapeamento de memória do SDK na mão (winapi, sem crate intermediária), extrai o YAML de sessão e um snapshot curado de telemetria, e alimenta um amostrador de fundo a 60 Hz que sustenta o monitor de corrida (tentativas, batidas, quebras, histórico), a família de spotters calibrados por corridas gravadas e o gravador sempre ligado. Na outra direção, gera os artefatos que o iRacing consome (AI roster, AI season, clima em keyframes, pinturas), injeta comandos de chat via SendInput para penalidades, e traz o resultado oficial de volta para a carreira pelo result_bridge. Fora do Windows tudo compila como stub que devolve Unsupported. A robustez de desconexão é tratada por bordas: sim fechado vira DNF da tentativa ativa, fecha a captura, cai para polling de 1 Hz e arma a janela de recuperação de foco.

Componentes:

- imp/ (leitura, janela, chat, diagnostico, util, stub) (~1181 linhas): Binding winapi: abre a shared memory com segunda chance de namespace, distingue sim fechado de acesso negado, escolhe o buffer mais recente, casa ~80 variáveis por nome, acha e foca a janela do sim, digita chat via SendInput e cruza sinais num diagnóstico com veredito
- race_monitor/ + race_monitor.rs (~8300 linhas): Monitor unificado a 60 Hz: tentativa como container de batidas e evidências, pontuação de impacto calibrada, sistema de quebra com fila de comandos !black/!dq, race control com recomendação de amarela, histórico volta a volta, estado agora para o engenheiro falado e recuperação de foco quando o sim fecha
- Família spotter (spotter, frente, tras, lento, voltar, bandeira, boxe, clima, control) (~7148 linhas): Detectores com histerese sobre CarLeftRight e CarIdx*: vizinhança lateral, obstáculo à frente, carro lento, bandeiras, box e clima; todos os limiares saíram de corridas gravadas; spotter_control silencia o spotter nativo no app.ini e o devolve
- telemetry_analysis/ (~2333 linhas): Pós-corrida fase 2, lógica pura: ritmo, consistência, rival, momentos narrativos, setores, combustível e o dossiê de habilidade, tudo tolerante a dados parciais
- behavior/ (~2141 linhas): Atitude do dia por corrida no export: sinais em três tiers somados à base com maleabilidade e compostura por mentalidade, só mexendo nos atributos secundários da IA
- result_bridge/ (~1566 linhas): Ponte da corrida real para o RaceResult que a carreira consome: identidade carro x piloto, agregações do histórico, resultado da sessão ao vivo e do JSON oficial do aiseason
- weather/ (~1421 linhas): História do clima do fim de semana por pista e estação, penalidade de skill da IA na chuva e conversão em keyframes da timeline dinâmica do iRacing
- Export (roster_gen, season_gen, results_gen, paint_gen, car_difficulty) (~1832 linhas): Gera roster.json e aiseason JSON com aparência por time, número e sponsors; inverte nível de carro em dificuldade de IA porque os carros do iRacing são spec
- race_capture.rs (~658 linhas): Gravador sempre ligado: JSONL gzip com telemetria crua, YAML e resumo do histórico, subamostragem de cars[] e teto de disco por rotação de capturas
- Análises puras (adaptive, tire_strategy, rivalry_perception) (~1999 linhas): Dificuldade adaptativa por ritmo verde, estratégia de pneu inferida das paradas e percepção de rivalidade a partir do trace de campo
- Resultados oficiais (session_results, aiseason_results) (~570 linhas): Parsers do ResultsPositions do YAML de sessão e do JSON persistido do aiseason, a fonte autoritativa quando o jogador sai cedo
- Configuração do sim (modo_janela, race_control, paths, custid) (~777 linhas): Põe o sim em borderless nos rendererDX11*.ini, instala a macro de amarela no app.ini com backup, resolve Documentos via Known Folder API e persiste o custid do jogador
- Fachada e tipos (api, tipos, yaml, constantes) (~671 linhas): Funções públicas que o resto do app chama, DTOs serde da telemetria e parsers rasos do YAML por varredura de linha

Testes: Cobertura forte para um módulo que fala com hardware alheio: 423 #[test] no total, com suítes dedicadas em race_monitor/tests (78 testes), behavior/tests, weather/tests, telemetry_analysis/tests, result_bridge/tests e race_monitor/voltas/tests, mais testes inline em todos os spotters (calibrados contra corridas gravadas de Lime Rock e Okayama), modo_janela, spotter_control, race_capture, adaptive, tire_strategy e yaml. Os buracos: race_control.rs e custid.rs têm zero testes; imp/* é winapi puro e só se valida com o sim aberto, o que é aceitável; o amostrador (thread de fundo com toda a orquestração de bordas de conexão) não tem teste direto, e a lógica de DNF por sim fechado vive dentro dele; e o match de extract_telemetry, o ponto onde nome errado lê zero em silêncio, só é validado indiretamente por capturas reais.

Pontos de revisão:

- [Alta] Contrato stringly-typed cruzando para o React: status, ended_by e severidade como String
  - Arquivos: src-tauri/src/iracing_sdk/race_monitor/tipos.rs:9-60, src-tauri/src/iracing_sdk/race_monitor.rs:206-227
  - Motivo: Attempt.status é String com valores mágicos (active, finished, dnf, not_started), ended_by idem, e a severidade circula como string em português (grave, destruído) comparada por índice no array SEVERITIES; há 23 comparações literais fora de testes e a regra de quali wreck compara QUALI_WRECK_PENALTY_SEV == "grave" direto. Um typo compila, serializa e falha em silêncio dos dois lados da ponte. Enums serde com rename resolveriam o risco inteiro.
- [Alta] Preferências e identidade persistidas em %TEMP%
  - Arquivos: src-tauri/src/iracing_sdk/race_monitor.rs:774, src-tauri/src/iracing_sdk/custid.rs:10
  - Motivo: O flag de auto amarela (loop_auto_yellow.flag) e o custid do jogador (loop_player_custid.txt) moram em std::env::temp_dir(). A limpeza de disco do Windows apaga %TEMP% e o custid é a identidade que casa o jogador no pós-corrida; a perda é silenciosa e o sintoma aparece longe da causa. Deveriam morar no diretório de config do app junto do resto.
- [Alta] Entrega de eventos do spotter por polling do front, vulnerável ao throttling de janela coberta
  - Arquivos: src-tauri/src/iracing_sdk/spotter.rs:50-54, src-tauri/src/iracing_sdk/race_monitor/api.rs
  - Motivo: O sampler produz eventos a 60 Hz e o front os drena por invoke periódico. Com a janela do webview coberta pelo sim o navegador estrangula os timers e há perda medida de eventos que constam no loop.log (memória do projeto confirma que a perda é do front). Emitir os eventos por push (emit do Tauri) tiraria o elo frágil; hoje o buffer de 80 eventos é a única folga.
- [Média] Penalidade depende de SendInput com foco de janela, com furo conhecido em fullscreen exclusivo
  - Arquivos: src-tauri/src/iracing_sdk/imp/chat.rs:110-134, src-tauri/src/iracing_sdk/race_monitor/amostrador.rs:83-101
  - Motivo: O comando !black/!dq só chega se o SO aceitar o foreground; o próprio comentário documenta o limite: com o sim já em foco e em fullscreen exclusivo o SendInput pode não chegar sem erro detectável. A mitigação (modo_janela força borderless) cobre o caso comum e o aviso âmbar cobre a recusa de foco, e o caso indetectável segue existindo. Vale medir se o fallback de bandeira preta com penalidade fixa (QUALI_WRECK_FALLBACK_PENALTY_S) cobre todos os desfechos.
- [Média] race_monitor.rs concentra ~90 constantes de calibração de domínios distintos
  - Arquivos: src-tauri/src/iracing_sdk/race_monitor.rs:52-237
  - Motivo: Batida, race control, cluster de pit, quebra na quali e limites de memória dividem o mesmo bloco de constantes. Parte é calibrada em teste (pontuação de batida), parte é a olho sem medição registrada (DANGER_GAP 0.10, YELLOW_MIN_STOP_SECS 2.0, START_GRACE_SECS 8.0, DANGER_CARS_MIN 1). Separar por domínio e marcar o que tem medição atrás evitaria mexida cega no que foi medido.
- [Média] Duplicação estrutural na família spotter
  - Arquivos: src-tauri/src/iracing_sdk/spotter_frente.rs, spotter_tras.rs, spotter_lento.rs, spotter_voltar.rs, spotter_bandeira.rs, spotter_boxe.rs
  - Motivo: Cada irmão reimplementa o mesmo esqueleto: singleton Mutex/OnceLock, VecDeque de eventos com teto, lock(), observar(), detecção de salto de SessionTime e rodízio de chaves de fala. São ~7k linhas onde a parte calibrada (os limiares) é pequena e a infraestrutura se repete seis vezes; um esqueleto comum reduziria a superfície sem tocar nos números medidos.
- [Média] Mapeamento de variáveis do SDK sem guarda cruzada com o inventário
  - Arquivos: src-tauri/src/iracing_sdk/imp/leitura.rs:188-358
  - Motivo: O match gigante casa canais por nome e um nome errado cai no _ => {} calado, lendo zero para sempre; já aconteceu em produção com PitRepairNeeded vs PitRepairLeft, corrigido e documentado no próprio código. O inventário read_var_inventory existe e vai para a captura; cruzar a lista curada com o inventário na borda de conexão e logar canal curado ausente transformaria o silêncio em diagnóstico.
- [Média] Parsers de YAML por prefixo de linha sem âncora de seção
  - Arquivos: src-tauri/src/iracing_sdk/yaml.rs, src-tauri/src/iracing_sdk/race_monitor/sessao.rs, src-tauri/src/iracing_sdk/session_results.rs
  - Motivo: parse_track_id e parecidos devolvem a primeira ocorrência do prefixo no arquivo inteiro; um campo homônimo em outra seção do YAML do sim passaria a ser lido no lugar errado sem erro. A abordagem está espalhada por três arquivos com estilos levemente diferentes. Funciona nas builds vistas e merece ao menos ancorar na seção esperada nos campos ambíguos.
- [Média] race_control.rs sem nenhum teste
  - Arquivos: src-tauri/src/iracing_sdk/race_control.rs
  - Motivo: Edita o app.ini do jogador (backup, busca do slot AutoChatStr por texto com fallback para o slot 7, troca pela macro !y$). Uma regressão no parsing corrompe configuração do usuário fora do app e nada acusaria; o vizinho spotter_control.rs faz manipulação parecida e tem testes de reescrita, o padrão já existe para copiar.
- [Baixa] Suíte do race_monitor num arquivo único de 1994 linhas
  - Arquivos: src-tauri/src/iracing_sdk/race_monitor/tests/mod.rs
  - Motivo: 78 testes de tentativas, quebras, sessão e clima no mesmo mod.rs. O módulo sob teste já foi fatiado em 13 arquivos; a suíte ficou monolítica e localizar o teste de uma área exige busca textual. Fatiar espelhando os submódulos.
- [Baixa] expect de invariante em caminho de produção do spotter lento
  - Arquivos: src-tauri/src/iracing_sdk/spotter_lento.rs:743
  - Motivo: episodio.take().expect("acabou de existir") roda dentro do tick do sampler; o catch_unwind do amostrador segura o processo e o tick morre com log genérico. O invariante parece verdadeiro hoje; um if let com continue custaria nada e tiraria o panic do loop quente. Os demais unwrap do módulo estão em testes ou guardados por checagem de tamanho.
- [Baixa] rivalry_perception com allow(dead_code) no topo
  - Arquivos: src-tauri/src/iracing_sdk/rivalry_perception.rs:1
  - Motivo: 691 linhas de camada de percepção ainda não ligada ao motor de rivalidade, com o lint de código morto desligado no arquivo inteiro. Enquanto a aplicação não chega, o allow esconde qualquer função que ficar órfã de verdade; restringir o allow aos itens de fato pendentes manteria o aviso útil.


## Camada de comandos de carreira (src-tauri/src/commands: career/, career_commands.rs, career_types/, historical_draft, config.rs, window.rs, mod.rs, lib.rs)

É a fachada Tauri da carreira: toda a lógica vive em funções *_in_base_dir puras em relação ao AppHandle (career/ e historical_draft.rs), e career_commands.rs é a casca fina que resolve o base_dir e delega. O padrão é consistente em toda a área: abertura de save via open_career_resources (com reparo de contratos sob mutex global), erros como Result&lt;_, String&gt;, DTOs serde isolados em career_types/. Cobre criação e carga de save, avanço de temporada, janela de mercado e poaching, classificações, consultas de leitura, tela de campeão, draft histórico de 26 temporadas e utilitários de janela e config. O lib.rs registra 198 comandos no invoke_handler, todos no mesmo formato commands::modulo::funcao.

Componentes:

- career.rs (índice) (~186 linhas): Declara os submódulos de career/, concentra os ~60 imports compartilhados via use super::* e faz os re-exports com visibilidade calibrada (pub(crate), cfg(test))
- career/lifecycle.rs (~875 linhas): Criação, carga, exclusão e listagem de saves; open_career_resources e o reparo de consistência de contratos regulares que roda em toda abertura
- career/season_flow.rs (~274 linhas): Avanço de temporada com máquina de fases (backup canônico antes do fechamento) e skip de todas as corridas pendentes pelo jogador sem equipe
- career/market_window.rs (~778 linhas): Janela de mercado e pré-temporada: avanço de semana, propostas ao jogador, quebra de contrato (poaching) e o fecho com preenchimento de vagas
- career/standings.rs (~642 linhas): Classificações de equipes, categorias especiais e o fallback de resultados recentes; ordenação pela temporada anterior antes da primeira etapa
- career/queries.rs (~887 linhas): Consultas de leitura: pilotos por categoria, calendário, notícias, dossiê do jogador, resolução de contrato/equipe/papel e montagem de TeamSummary
- career/champion.rs (~1587 linhas): Payload da tela Campeão da Temporada montado só de race_results (recordes, prêmios, construtores), com constantes nomeadas e 15 testes inline
- career/vacancies.rs (~514 linhas): Normalização de lineup, propostas emergenciais ao jogador, encaixe forçado e reposição de assentos vazios
- career/interests.rs (~162 linhas): Seleção de Nemesis e Rivais com pisos e histerese em constantes documentadas
- career/briefing.rs (~241 linhas): Resumos pré-corrida: histórico da pista, rival primário e histórias do fim de semana
- career/save_state.rs (~244 linhas): Leitura e escrita de meta.json, resume context e histórico de frases do briefing
- career/debug.rs (~378 linhas): Comandos de debug: cenários de mercado, pulo para a final, dry-run de poaching e carimbo de campeonato, com SQL direto no save
- career_commands.rs (~603 linhas): Casca #[tauri::command] uniforme: resolve app_data_dir e delega; agrega também career_detail, career_team_dossier, global_driver_rankings, global_team_history e transfer_market
- career_types/ (9 arquivos) (~3223 linhas): DTOs serde puros da ponte Rust/React, fachada em career_types.rs; piloto.rs (1019) e equipe.rs (933) concentram 98 structs/enums
- historical_draft.rs (~1118 linhas): Draft de carreira histórica: simula 2000 a 2025 dentro do comando, poda órfãos do backstory, audita o mundo e finaliza inserindo o jogador como N2
- historical_draft/tests/mod.rs (~1747 linhas): 28 testes do draft histórico, com instabilidade conhecida registrada na memória do projeto
- career/tests/mod.rs (~5897 linhas): 93 testes de integração da área inteira num único arquivo
- config.rs (~57 linhas): get_config e update_config com merge manual campo a campo do AppConfig
- window.rs (~75 linhas): Controles da janela sem decoração: minimizar, arrastar, maximizar, tela cheia
- mod.rs + lib.rs (~32 linhas): 32 módulos de comandos declarados; invoke_handler com 198 comandos registrados um a um, padrão uniforme

Testes: Cobertura forte no núcleo: career/tests/mod.rs tem 93 testes de integração cobrindo lifecycle (criação, carga, reparo de contratos, concorrência), market_window (aceite, recusa, propostas emergenciais, fecho da janela), standings, queries, briefing e season_flow; interests aparece nesses testes (46 menções a nemesis/rival). champion.rs traz 15 testes inline dos recordes e prêmios. historical_draft tem 28 testes com 4 falhas pré-existentes e 1 flaky conhecidas. lib.rs testa o debounce e a persistência de janela. Buracos: debug.rs sem nenhum teste direto (e escreve SQL cru), config.rs sem teste do merge manual, window.rs sem teste (aceitável, é casca do Tauri), e career_types/ depende só dos testes de serialização indiretos. unwrap/expect praticamente ausentes em caminho de produção: um único expect justificado em historical_draft.rs:828, o resto vive em blocos de teste.

Pontos de revisão:

- [Alta] resolve_player_poach_offer confia na oferta vinda do front
  - Arquivos: src-tauri/src/commands/career/market_window.rs:117
  - Motivo: O comando recebe o PlayerPoachOffer inteiro serializado pela UI e o aplica no banco via resolve_player_poach sem conferir contra o plan.player_poach_offer persistido. Um front desatualizado ou adulterado grava salário e equipe que o backend nunca emitiu. Contrato frágil na ponte Rust/React.
- [Alta] load_career_in_base_dir é função gigante com efeitos colaterais
  - Arquivos: src-tauri/src/commands/career/lifecycle.rs:140
  - Motivo: Cerca de 240 linhas misturando reparo de fase da temporada, telemetria, spawn de thread de prewarm, cálculo de interesse do evento, cota de fama, escrita de meta.json e config, e montagem do payload. Cada preocupação nova aterrissa aqui. Extrair blocos puros deixaria o fluxo testável por partes.
- [Média] Mensagens de erro user-facing fora do i18n e sem acento
  - Arquivos: src-tauri/src/commands/career/*.rs, src-tauri/src/commands/historical_draft.rs:509
  - Motivo: Toda a camada devolve erros como String em PT hardcoded ("Temporada ativa nao encontrada.", "Save nao encontrado.") sem rust_i18n e sem acentuação, enquanto lifecycle.rs:129 usa t!("career.message.created"). historical_draft.rs:509 devolve mensagem de sucesso hardcoded. Jogador em en-US recebe prosa em português.
- [Média] Comandos de debug registrados no build de produção
  - Arquivos: src-tauri/src/lib.rs:481, src-tauri/src/commands/career/debug.rs
  - Motivo: debug_prepare_market_scenario, debug_stamp_player_championship, debug_skip_to_season_finale e afins escrevem SQL direto no save (posição forçada, fama 82.0, contrato rescindido) e estão no invoke_handler sem gate de cfg(debug_assertions) ou flag. Qualquer devtools aberto pode corromper uma carreira real.
- [Média] Draft histórico simula 26 temporadas bloqueando o comando async
  - Arquivos: src-tauri/src/commands/historical_draft.rs:40
  - Motivo: create_historical_career_draft_in_base_dir roda a simulação 2000 a 2025 sincronamente dentro de um comando async; o progresso sai por polling de meta.json (update_draft_progress). Trabalho de minutos no runtime async sem spawn_blocking pode segurar outros comandos.
- [Média] Duplicação de helpers entre historical_draft.rs e career/
  - Arquivos: src-tauri/src/commands/historical_draft.rs:368,513,831 vs career/lifecycle.rs:467,868 e career/save_state.rs:5
  - Motivo: career_number_from_id, count_rows e read_save_meta existem em duas cópias que podem divergir. A lista mágica de categorias especiais ["production_challenger", "endurance"] também aparece em season_flow.rs:207 e historical_draft.rs:926; uma categoria especial nova exige lembrar dos dois pontos.
- [Média] career/tests/mod.rs monolítico e testes instáveis do draft
  - Arquivos: src-tauri/src/commands/career/tests/mod.rs, src-tauri/src/commands/historical_draft/tests/mod.rs
  - Motivo: 5.897 linhas num único arquivo para 93 testes dificultam localizar e fatiar execução; o padrão dos irmãos (overlay/tests, iracing) já é diretório. O suite do historical_draft carrega 4 falhas pré-existentes e 1 flaky documentadas na memória do projeto, o que polui qualquer verificação da área.
- [Média] update_config perde campo novo silenciosamente
  - Arquivos: src-tauri/src/commands/config.rs:16
  - Motivo: O merge é manual campo a campo (language, autosave_enabled, auto_paint_car, telemetry, chaves de overlay). Um campo novo de AppConfig que não entrar nessa lista é aceito do front e descartado ao salvar, sem erro. Não há teste cobrindo o merge.
- [Baixa] Reparo de contratos em toda abertura de save sob mutex global
  - Arquivos: src-tauri/src/commands/career/lifecycle.rs:661,686
  - Motivo: repair_regular_contract_consistency roda em cada open_career_resources de escrita, carrega todos os contratos, equipes e pilotos e serializa aberturas concorrentes via CAREER_OPEN_REPAIR_LOCK. O custo cresce com o mundo (200+ pilotos) e é pago até por comandos que só precisavam escrever meta.json.
- [Baixa] Parâmetro category ignorado em open_career_resources_for_category_read
  - Arquivos: src-tauri/src/commands/career/lifecycle.rs:623
  - Motivo: O corpo faz let _ = category; e delega ao read_only genérico. A assinatura promete comportamento por categoria que não existe, e o chamador em standings.rs:241 passa a categoria acreditando que ela importa.
- [Baixa] persist_end_of_season_news é stub morto ainda chamado
  - Arquivos: src-tauri/src/commands/career/season_flow.rs:231,90
  - Motivo: A função devolve Ok(()) com todos os argumentos ignorados e segue sendo chamada e embrulhada em warn_if_noncritical no advance_season. Ou implementa ou remove a chamada.
- [Baixa] Doc de select_player_interests contradiz o código
  - Arquivos: src-tauri/src/commands/career/interests.rs:42
  - Motivo: O comentário diz "Sem histerese ainda" e o próprio corpo aplica NEMESIS_HYSTERESIS_MARGIN nas linhas 135 a 148, com persistência do reinante em get_player_interests_in_base_dir. Comentário defasado num arquivo que serve de referência de calibração.
- [Baixa] Números mágicos de heurística e ano fixo na criação
  - Arquivos: src-tauri/src/commands/career/lifecycle.rs:48,239
  - Motivo: is_title_decider usa remaining <= 2 && gap_to_leader <= 50 sem constante nomeada nem calibração registrada; Season::new(_, 1, 2024) fixa o ano da carreira regular enquanto o draft histórico joga em 2026 (PLAYABLE_START_YEAR), dois inícios de mundo divergentes por literais soltos.
- [Baixa] Padrão de busca de vaga duplicado em vacancies.rs
  - Arquivos: src-tauri/src/commands/career/vacancies.rs:246,328
  - Motivo: generate_emergency_player_proposals e force_place_player repetem o mesmo bloco de filtrar vagas por tier e refazer a passada sem tier quando vazio, só mudando o predicado. Um helper com o predicado como parâmetro elimina a cópia.
- [Baixa] CLAUDE.md defasado sobre o tamanho do invoke_handler
  - Arquivos: CLAUDE.md, src-tauri/src/lib.rs:468
  - Motivo: O doc fala em ~150 entradas; o invoke_handler tem 198 comandos hoje. Detalhe pequeno, mas é o número que orienta quem registra comando novo.


## Comandos de integração em tempo real (commands/iracing, commands/overlay, commands/race, ptt)

É a ponte entre a carreira e o iRacing real. Exporta o grid e o calendário como AI roster e AI season (roster.rs, temporada.rs), acompanha a sessão ao vivo pela telemetria (torre de tempos, avisos de quebra do engenheiro), importa o resultado oficial de volta para a carreira (resultado.rs, importacao.rs dos dois lados) e fecha o ciclo com dificuldade adaptativa por custid, previsão de quebras por Monte Carlo, pintura automática do carro e o push-to-talk de voz com o engenheiro. A simulação offline (race/simulacao.rs) usa as mesmas fontes determinísticas de clima e quebra para os dois mundos baterem.

Componentes:

- src-tauri/src/commands/iracing/roster.rs (~704 linhas): Geração do AI roster: números fixos por piloto, contexto comportamental (forma, lesão, rivalidade, vingança), dificuldade por carro e instalação do diretor de quebra ao vivo
- src-tauri/src/commands/iracing/temporada.rs (~626 linhas): Geração da AI season (calendário do iRacing), escada de dificuldade por tier, offsets calibrados por pista e a banda de skill que ancora o esticão do iRacing
- src-tauri/src/commands/iracing/resultado.rs (~694 linhas): Ponte resultado do iRacing para a carreira: build_session_race_result (setup compartilhado de preview e import), quebras resolvidas para driver_id e percepção de rivalidades de pista
- src-tauri/src/commands/iracing/importacao.rs (~152 linhas): Gatilho automático de import: poller que detecta o resultado pronto, importa, aplica rivalidade, clima e ajuste adaptativo, e persiste a tela pós-corrida
- src-tauri/src/commands/iracing/clima.rs (~341 linhas): Fonte única do clima determinístico por etapa (semente carreira+corrida), conversões para o Sistema de Quebra e timeline para a UI
- src-tauri/src/commands/iracing/adaptativo.rs (~288 linhas): Perfil adaptativo de dificuldade por custid (global + por pista), post-its de carro e banda do export, processamento idempotente da última corrida
- src-tauri/src/commands/iracing/previsao_quebras.rs (~280 linhas): Previsão de risco de quebra por Monte Carlo: card do jogador na Sala de Estratégia e marcador de risco por equipe na tabela
- src-tauri/src/commands/iracing/pintura.rs (~310 linhas): Pintura do carro do jogador na cor do time (tga custom paint), com backup do arquivo original e vínculo do custid ao save
- src-tauri/src/commands/iracing/modo_janela.rs (~15 linhas): Casca fina do ajuste dos renderer.ini para janela sem borda (pré-requisito do overlay)
- src-tauri/src/commands/overlay/torre.rs (~799 linhas): Torre de tempos ao vivo: cruza telemetria, YAML da sessão e banco da carreira a cada poll, ordena por classe e monta o payload da janela de overlay
- src-tauri/src/commands/overlay/formato.rs (~269 linhas): Helpers puros da torre: parse do YAML de sessão, sentinelas de tempo/volta do iRacing, contagem de voltas e diagnóstico em log
- src-tauri/src/commands/overlay/avisos.rs (~152 linhas): Avisos pessoais do jogador na voz do engenheiro (peça em risco, desfecho de quebra, quali destruída) e banner de chat bloqueado
- src-tauri/src/commands/race/simulacao.rs (~826 linhas): Simulação offline de uma etapa: pré-roll de quebras sobre o desgaste real dos carros, esteira de modificadores e varredura das outras categorias da semana
- src-tauri/src/commands/race/importacao.rs (~499 linhas): Entrada do resultado real na carreira: filtro de carros fantasmas, rescore de classe, DNF de quebra, conserto do carro do jogador, fatura e boletim
- src-tauri/src/commands/ptt.rs (~183 linhas): Vigia do gatilho físico do push-to-talk (tecla ou botão de volante via GetAsyncKeyState/joyGetPosEx), emitindo ptt-apertou/ptt-soltou
- src-tauri/src/commands/ptt_voz.rs (~247 linhas): Relé HTTP do PTT: transcrição, resposta redigida+sintetizada e síntese de peça de voz, tudo contra o servidor Cloud Run

Testes: Cobertura muito desigual. O lado overlay tem 28 testes unitários em overlay/tests/mod.rs cobrindo bem os helpers puros (formato: best_positive_lap, contagem de voltas, parse do YAML, sentinelas; ordem: critérios por modo de sessão); o comando get_overlay_data em si (o join com o banco e a montagem do payload) fica sem teste. ptt.rs e ptt_voz.rs têm testes inline pequenos e pertinentes (gatilho, tradução de status HTTP). race/tests cobre despesa e medição financeira, fora deste recorte; simulacao.rs e importacao.rs de race são exercitados indiretamente pelos testes de career. O buraco grande é commands/iracing/ inteiro: zero testes para roster, temporada, resultado, clima, adaptativo, previsão de quebras e pintura, justamente onde moram os contratos mais frágeis (números fixos, post-its, bandas de skill, casamento de resultado).

Pontos de revisão:

- [Alta] iracing_generate_roster é uma função de ~620 linhas que faz tudo
  - Arquivos: src-tauri/src/commands/iracing/roster.rs:82
  - Motivo: Um único comando lê o banco, monta contexto comportamental por piloto (lesão, vingança, nêmesis, lua de mel), resolve clima, calcula dificuldade de carro, grava três arquivos e instala o diretor de quebra no monitor. Qualquer mudança em um eixo obriga a reler a função inteira, e nada disso tem teste. Extrair a montagem do DriverCtx e do BehaviorContext em funções puras destravaria teste unitário.
- [Alta] Auto-import engole erro real como se fosse 'ainda não está pronto'
  - Arquivos: src-tauri/src/commands/iracing/importacao.rs:45
  - Motivo: O match faz Err(_) => return Ok(None). Falha de verdade (banco corrompido, aiseason inválido, post-it apontando para arquivo apagado) fica indistinguível de resultado ainda ausente: o poller do front repete em silêncio para sempre e o jogador nunca vê o motivo. Vale separar as falhas de 'ainda não' das falhas de estado quebrado, ao menos no log de diagnóstico.
- [Alta] get_overlay_data abre banco e faz consultas por carro a cada poll
  - Arquivos: src-tauri/src/commands/overlay/torre.rs:129
  - Motivo: A função roda ~1x/s pelo front e a cada chamada reabre o career.db, relê o mapa de números do disco e o closure resolve dispara 3 consultas por carro (piloto, contrato, equipe) para um grid de 20+. São dezenas de queries por segundo durante a corrida inteira, dentro de uma função de ~670 linhas sem teste direto (só os helpers de formato/ordem têm). Cache por sessão do lado carreira derrubaria quase todo o custo.
- [Alta] build_session_race_result devolve tupla posicional de 11 elementos
  - Arquivos: src-tauri/src/commands/iracing/resultado.rs:131, src-tauri/src/commands/iracing/importacao.rs:33
  - Motivo: O contrato entre o setup compartilhado e os dois consumidores (preview e auto-import) é destructuring posicional de 11 campos, dois deles String sem tipo próprio (severidade, direção do impacto). Inserir um campo no meio recompila mas troca significado em silêncio se dois vizinhos tiverem o mesmo tipo. Uma struct nomeada elimina a classe de erro.
- [Média] track_skill_offset calibrada na escada antiga de dificuldade
  - Arquivos: src-tauri/src/commands/iracing/temporada.rs:154
  - Motivo: A tabela de offsets por pista foi medida com a base de tier 10 pontos acima (rebaixada em 10/08/2026, comentário nas linhas 36-38 e 152-153 admite a defasagem). Os valores atuais somam offset novo sobre baseline nova sem revalidação em pista. Cada corrida em pista já calibrada merece re-medição, e a tabela cresce por edição manual de código.
- [Média] Acoplamento temporal roster->temporada via post-its em arquivo
  - Arquivos: src-tauri/src/commands/iracing/adaptativo.rs:110, src-tauri/src/commands/iracing/temporada.rs:523, src-tauri/src/commands/iracing/temporada.rs:606
  - Motivo: A temporada só sai correta se o roster rodou antes na mesma pista/categoria (ExportSkillBand), e o import só funciona se o pointer do export existir. As escritas desses post-its são let _ = (falha silenciosa), e a validade é checada por casamento de categoria+pista, sem carimbo de tempo. Post-it velho de um fluxo interrompido produz banda errada sem aviso.
- [Média] import_iracing_race_result concentra 440 linhas e perde débito em silêncio
  - Arquivos: src-tauri/src/commands/race/importacao.rs:63, src-tauri/src/commands/race/importacao.rs:442
  - Motivo: A função mistura filtro de fantasmas, rescore, DNF de quebra, persistência, conserto, fatura, boletim e simulação das outras categorias. No bloco do conserto, upsert_team_car e update_team são let _ =: se a escrita falhar, o dano do carro e o débito do caixa somem sem log, com o resultado já persistido. O warn_if_side_effect_fails usado no resto do arquivo caberia aqui.
- [Média] Rivalidade IA contra IA é O(n²) de percepções dentro do import
  - Arquivos: src-tauri/src/commands/iracing/resultado.rs:635
  - Motivo: Para cada carro-sonda roda perceive_rivalries sobre o histórico completo, com dedupe manual por par e todos os erros engolidos (Err(_) => continue, let _ =). Roda uma vez por import, o custo hoje é aceitável em grid de 20; cresce quadrático com o grid do endurance e nenhuma falha aparece em log.
- [Média] commands/iracing/ inteiro sem nenhum teste
  - Arquivos: src-tauri/src/commands/iracing/
  - Motivo: Nenhum arquivo do diretório tem #[cfg(test)] e não existe pasta tests. Roster, temporada, resultado, clima e adaptativo são a ponte crítica com o iRacing e contêm a lógica mais fina do subsistema (bandas de skill, casamento de números, post-its); tudo é validado só correndo de verdade. As partes puras (ensure_driver_numbers, ai_sweet_spot, story_to_weather_condition, sim_safe_year) são testáveis hoje sem refatoração.
- [Média] Limiares de previsão de quebra declaradamente sem calibração
  - Arquivos: src-tauri/src/commands/iracing/previsao_quebras.rs:168
  - Motivo: DNF_VERMELHO=0.03, CUSTO_LARANJA=0.08 e IDAS_LARANJA=0.50 decidem a cor do card e o comentário diz 'ainda por afinar na pista'. O marcador da tabela (linhas 270-273) repete os mesmos números inline em vez de reusar as constantes: ajustar um lado e esquecer o outro dessincroniza card e tabela.
- [Baixa] Hemisfério da pista deduzido por emoji de bandeira em string
  - Arquivos: src-tauri/src/commands/iracing/clima.rs:9
  - Motivo: track_hemisphere faz pais.contains sobre uma lista de 9 emojis. Pista nova de país do hemisfério sul fora da lista cai no norte em silêncio e inverte a estação do clima inteiro daquela etapa. Um campo de hemisfério no catálogo de pistas mataria a heurística.
- [Baixa] URL do servidor Cloud Run duplicada em 9 pontos do código
  - Arquivos: src-tauri/src/commands/ptt_voz.rs:26
  - Motivo: O mesmo host aparece hardcoded em ptt_voz.rs, narrative/client.rs (6 constantes), telemetry.rs e diagnostico.rs. Migrar de região ou de projeto exige caça manual; uma constante única com os paths derivados resolve.
- [Baixa] Export da temporada altera o calendário do save como efeito colateral
  - Arquivos: src-tauri/src/commands/iracing/temporada.rs:360
  - Motivo: iracing_generate_season faz UPDATE em calendar (clima e temperatura) por etapa com let _ = no meio de um comando cujo nome promete gerar um arquivo. A intenção (fonte única de clima) é boa; o efeito de escrita escondido num export e a falha engolida merecem ao menos log e menção no doc do comando.
- [Baixa] Máscara de bits e unwrap soltos na torre
  - Arquivos: src-tauri/src/commands/overlay/torre.rs:296, src-tauri/src/commands/overlay/torre.rs:654
  - Motivo: A black flag do jogador é lida com o literal 0x0001_0000 sem constante nomeada apontando o enum do SDK. O partial_cmp().unwrap() da volta mais rápida está protegido pelo filtro de tempos positivos e ainda assim é o único unwrap em caminho de produção da área: um NaN vindo do SDK derrubaria o comando do overlay.


## Persistência e configuração (src-tauri/src/db, src-tauri/src/models, src-tauri/src/config, commands/config.rs)

Camada de dados do save: SQLite local por carreira com WAL, busy_timeout de 15 s e transação BEGIN IMMEDIATE. O schema nasce de uma baseline colapsada (43 tabelas, 39 índices) registrada como migração v53, mais 8 migrações incrementais até a v61; saves v1 a v52 são recusados com mensagem explícita e sem tocar no arquivo. As queries são organizadas em 26 módulos por domínio, os models são structs serde em português que cruzam a ponte para o React, e a configuração do app vive em config.json (AppConfig) e meta.json por save (SaveMeta).

Componentes:

- db/migrations.rs (~1197 linhas): Registro declarativo das 9 migrações vivas (baseline v53 + v54 a v61), helpers de versão na tabela meta, recusa de saves pré-baseline e suíte de testes de upgrade caminho a caminho
- db/migrations/baseline.rs (~789 linhas): DDL colapsado das 53 migrações originais: 43 CREATE TABLE e 39 CREATE INDEX, tudo IF NOT EXISTS
- db/migrations/seed_incidentes.rs (~758 linhas): Seed do catálogo de 54 incidentes aplicado pela baseline
- db/migrations/schema_ouro.rs (~179 linhas): Trava de teste: descreve o schema produzido pelas migrações de forma semântica (PRAGMA table_info/index_info) e compara com fixture versionado
- db/connection.rs (~112 linhas): Database e DbError, PRAGMAs obrigatórios (WAL, foreign_keys, busy_timeout), backup com wal_checkpoint e transação com BEGIN IMMEDIATE
- db/queries/ (26 módulos) (~13400 linhas): Uma área de query por domínio; as maiores são drivers.rs (938), injuries.rs (864), news.rs (674) e races.rs (640); calendar, contracts, race_history e teams são pastas com submódulos leitura/escrita/mapeamento e tests/mod.rs próprios
- models/ (~4813 linhas): Structs de domínio serde (Driver, Team, Contract, Season, Injury, Rivalry) e enums por área em models/enums/; temporal.rs guarda a régua season_week x week_of_year; license.rs e team.rs carregam também lógica de mundo e acesso a banco
- config/app_config.rs (~416 linhas): AppConfig (espelho do config.json, com defaults defensivos por função para chaves novas) e SaveMeta (espelho do meta.json por carreira), mais helpers de caminho de save
- commands/config.rs (~57 linhas): Casca Tauri get_config/update_config com merge manual campo a campo e aplicação a quente de idioma, telemetria e chaves de overlay

Testes: O núcleo é bem coberto: migrations.rs testa cada caminho de upgrade (v54 a v61) com dados legados sobrevivendo, guarda a ordem do array e chega a asseverar EXPLAIN QUERY PLAN dos índices novos; schema_ouro trava o schema semanticamente contra fixture versionado; calendar, contracts, race_history e teams têm tests/mod.rs por área (557 a 717 linhas cada); drivers, injuries, news, favorites e meta têm testes inline, incluindo casos de dado corrompido no banco; app_config.rs cobre config parcial legado e filtro de lifecycle dos saves. Os buracos: os quatro módulos ai_*, player_nemesis, rivalry_episodes e special_team_entries sem teste algum (e são os que criam tabela fora das migrações), models/temporal.rs sem teste no módulo apesar da regra crítica da semana 48, e commands/config.rs sem teste da lógica de merge.

Pontos de revisão:

- [Alta] Schema criado por dois canais paralelos, com drift real já instalado
  - Arquivos: src-tauri/src/db/queries/team_car.rs:18, src-tauri/src/db/queries/milestones.rs:14, src-tauri/src/db/queries/ai_pre_race.rs:12, src-tauri/src/db/queries/ai_post_race.rs:12, src-tauri/src/db/queries/ai_story.rs:14, src-tauri/src/db/queries/ai_world_notes.rs:15, src-tauri/src/db/queries/player_nemesis.rs:15, src-tauri/src/db/queries/race_breakdowns.rs:34, src-tauri/src/db/queries/rivalry_episodes.rs:16, src-tauri/src/db/migrations/baseline.rs:647
  - Motivo: Nove arquivos de query criam ~12 tabelas via ensure_table idempotente fora do array MIGRATIONS. team_car.rs duplica textualmente o DDL da baseline e ainda adiciona a coluna unit_seed por ALTER guardado que a baseline desconhece: duas fontes de verdade que já divergem. A trava schema_ouro só enxerga o que run_all produz, então nada dessas tabelas está sob a trava, e uma mudança de coluna nelas passa sem nenhum guard.
- [Alta] Inferência de lesões legadas por LIKE sobre prosa de UI
  - Arquivos: src-tauri/src/db/queries/injuries.rs:365
  - Motivo: count_legacy_inferred_injuries_by_severity_for_pilot classifica severidade com dez padrões LIKE ('%colis%', '%batid%', '%capot%'...) sobre dnf_reason, que é texto gerado e o backend é bilíngue via rust-i18n. Um save com dnf_reason gravado em en-US zera a inferência em silêncio. Os mesmos padrões se repetem em três blocos CASE dentro da mesma query, e o comentário da migração v60 registra que esse caminho roda uma vez por piloto no ranking mundial.
- [Média] race_results sem índice por equipe_id
  - Arquivos: src-tauri/src/db/queries/teams/recordes.rs:34, src-tauri/src/db/migrations/baseline.rs:301
  - Motivo: A maior tabela do save (26.800 linhas num save de terceira temporada, pelo próprio comentário da v60) tem índice único (race_id, piloto_id) e o índice por piloto_id da v60. Consultas agregadas por equipe (get_category_wins_by_team, dobradinhas, dossiê) varrem a tabela inteira. A história da v60 (2,5 s caindo para 25 ms) mostra que esse padrão de índice faltante já custou caro uma vez.
- [Média] Régua season_week duplicada em SQL e sem teste no módulo canônico
  - Arquivos: src-tauri/src/db/queries/calendar/temporal.rs:142, src-tauri/src/models/temporal.rs:22
  - Motivo: models/temporal.rs define os conversores com a exceção documentada da semana 48 ('terra de ninguém', erro explícito) e calendar/temporal.rs embute COALESCE(season_week, week_of_year + 4) direto no SQL, aritmética que ignora a exceção. São duas implementações da mesma regra crítica, e temporal.rs é o único model de lógica sem bloco de teste próprio.
- [Média] SELECT * em seis arquivos de produção
  - Arquivos: src-tauri/src/db/queries/drivers.rs:100, src-tauri/src/db/queries/seasons.rs, src-tauri/src/db/queries/calendar/consultas.rs, src-tauri/src/db/queries/contracts/leitura.rs, src-tauri/src/db/queries/contracts/especial.rs, src-tauri/src/db/queries/teams/leitura.rs
  - Motivo: 35 ocorrências de SELECT * fora de teste. A leitura é por nome de coluna (driver_from_row usa row.get("...")), então renomear ou remover coluna só estoura em runtime, no save de alguém, sem aviso de compilação nem de teste de schema.
- [Média] Sete módulos de query sem nenhum teste
  - Arquivos: src-tauri/src/db/queries/ai_pre_race.rs, src-tauri/src/db/queries/ai_post_race.rs, src-tauri/src/db/queries/ai_story.rs, src-tauri/src/db/queries/ai_world_notes.rs, src-tauri/src/db/queries/player_nemesis.rs, src-tauri/src/db/queries/rivalry_episodes.rs, src-tauri/src/db/queries/special_team_entries.rs
  - Motivo: Nenhum bloco #[cfg(test)] nesses sete módulos. São justamente os que criam as próprias tabelas fora das migrações, ou seja, os de maior risco de drift de schema são os de menor cobertura.
- [Média] drivers.rs mapeia 54 colunas à mão em quatro lugares
  - Arquivos: src-tauri/src/db/queries/drivers.rs:9, src-tauri/src/db/migrations/baseline.rs:95
  - Motivo: Maior arquivo de query (938 linhas). Campo novo de piloto exige tocar a baseline, o INSERT nomeado, o UPDATE e o driver_from_row, além do model. A lista de 54 parâmetros nomeados é escrita duas vezes e nada verifica a paridade entre elas além de runtime.
- [Baixa] update_config faz merge manual campo a campo
  - Arquivos: src-tauri/src/commands/config.rs:16
  - Motivo: Cada chave nova de AppConfig exige lembrar de adicionar a linha de merge no update_config; chave esquecida é aceita do front e descartada em silêncio ao salvar pela tela de Configurações. A exceção do spotter_takeover está documentada no lugar, o que confirma que a regra é frágil o bastante para precisar de aviso.
- [Baixa] get_or_create_install_id engole falha de gravação
  - Arquivos: src-tauri/src/config/app_config.rs:212
  - Motivo: let _ = self.save() ignora erro de disco: se a escrita do config.json falhar, um install_id novo nasce a cada boot e o cooldown do servidor de boletins de IA reseta junto.
- [Baixa] models com lógica de mundo e acesso a banco
  - Arquivos: src-tauri/src/models/license.rs:217, src-tauri/src/models/team.rs:364
  - Motivo: repair_missing_licenses_for_current_categories recebe Connection e escreve no banco de dentro de models/, e generate_teams_for_category coloca geração de mundo na camada de dados. O padrão do projeto põe esse tipo de lógica em commands/career ou nos módulos de domínio, e a exceção convida imitação.


## economia e carro

O subsistema cobre o dinheiro e o carro do jogo. economia/ é o modelo novo bottom-up: fatura física da etapa (litros, pneus, frete), recorrentes anuais por divisão, cinco canais de receita e o ralo de investimento do excedente. finance/ é a camada legada por equipe (cashflow da rodada, saúde em meses de operação, salário, prêmio, venda de equipe falida, reputação, moral, foco e planos estratégicos), em reancoragem progressiva sobre economia/. car/ modela as 11 peças (vetor PHA, curva de custo com teto suave, desgaste com identidade de unidade, pré-roll de quebra com clima e pista, batida, estilo de pilotagem). volante.rs lê botões de volante via winmm para o recentro de VR.

Componentes:

- src-tauri/src/car/breakdown.rs (~1281 linhas): Cérebro do sistema de quebra: hazard em dois regimes, multiplicadores de pista e clima, regras de enduro, LiveBreakdown/BreakdownDirector para o disparo ao vivo e forecast pré-corrida
- src-tauri/src/economia/fatura.rs (~1048 linhas): Apresentação da fatura para o jogador: agrega as ~20 linhas físicas em quatro blocos com detalhe por quantidade e preço unitário
- src-tauri/src/finance/cashflow.rs (~978 linhas): Fluxo de caixa da rodada: amortização de dívida, bilheteria por fama, bônus, impacto de offseason na estrutura e biases de estado e estratégia
- src-tauri/src/finance/planning.rs (~670 linhas): Escala financeira por divisão (caixa e custo operacional), plano financeiro e budget_index; delega a âncora para economia/temporada
- src-tauri/src/finance/state.rs (~597 linhas): Saúde financeira em meses de operação, com as seis faixas (elite a colapso) calibradas no harness
- src-tauri/src/car/wear.rs (~590 linhas): Ciclo de vida da peça: desgaste por corrida e por volta, identidade de unidade (unit_seed), tenda de durabilidade por nível, confiabilidade do time
- src-tauri/src/car/breakdown_sim.rs (~577 linhas): Harness Monte Carlo do sistema de quebra, só em teste
- src-tauri/src/economia/ancora.rs (~538 linhas): Tabela física por divisão competitiva: consumo, pneu, frete, comitiva, inscrição, com preços unitários globais em dólar
- src-tauri/src/economia/temporada.rs (~538 linhas): Recorrentes anuais (escalares e categóricos) e o custo operacional anual de referência, a âncora do redesign
- src-tauri/src/car/driving_style.rs (~531 linhas): Estilo de pilotagem do jogador via telemetria: acumulador de sinais e fator assimétrico de desgaste por peça (0.75 a 1.30)
- src-tauri/src/economia/receita.rs (~508 linhas): Os cinco canais de receita declarados como total de temporada, com expoente de calendário derivado do critério 6
- src-tauri/src/car/crash.rs (~432 linhas): Dano por batida do jogador (G-force vira dano em peça e custo) e desgaste de contato de disputa para a grade toda
- src-tauri/src/finance/rescue.rs (~421 linhas): Venda de equipe cronicamente falida: 2 meses de caixa, regressão de estrutura, reputação e carro, identidade preservada
- src-tauri/src/car/cost.rs (~375 linhas): Curva de custo de peça: crescimento geométrico, parede acima do teto da categoria, teto de desenvolvimento e escala derivada do custo operacional
- src-tauri/src/economia/desenvolvimento.rs (~353 linhas): O ralo: equipe investe o excedente acima de 9 meses de reserva e recebe estrutura com retorno decrescente e depreciação
- src-tauri/src/finance/strategy.rs (~398 linhas): Planos estratégicos de 3 temporadas por equipe e categorias premium de dinastia
- src-tauri/src/car/parts.rs (~234 linhas): As 11 peças: durabilidade em corridas, viés PHA por nível e custo relativo, derivados do modelo do GPRO
- src-tauri/src/economia/evento.rs (~241 linhas): Fatura física de uma etapa: km reais, litros, jogos de pneu, frete com desconto de longa distância
- src-tauri/src/car/sim_bridge.rs (~106 linhas): Ponte carro para simulação: magnitude vira car_performance, PHA vira pesos de shape contra a pista
- src-tauri/src/volante.rs (~164 linhas): Leitura de botões de volante via joyGetPosEx (winmm) com binding manual; stub inerte fora do Windows
- src-tauri/src/finance/{salary,prize,morale,focus,reputation,events,economy}.rs (~1265 linhas): Satélites do finance: base salarial, prêmio de fim de temporada, moral, foco com histerese, reputação viva, juros de dívida e ciclo macroeconômico

Testes: Cobertura forte no conjunto. economia/ tem suíte dedicada em economia/tests/ (auditoria, fatura, faixas, forma, temporada, relatorio, duracao, hipotese, legado, divida, ~2,1k linhas) cobrindo os arquivos que não têm teste inline (ancora, evento, temporada, tipos). car/ tem teste inline em wear (18), cost (10), crash (11), driving_style (8), parts (6), seed (5), sim_bridge (4), mais car/breakdown/tests/mod.rs com 55 testes, o harness Monte Carlo breakdown_sim.rs e a medição ignorada medicao.rs (roda com --ignored, é régua e não asserção). finance/ tem teste inline em todos os arquivos (4 a 19 por arquivo), vários contra banco em memória. volante.rs tem dois smoke tests honestos sobre a limitação de CI sem volante. Lacunas: breakdown.rs não tem teste que exercite o gate de enduro pelos call sites reais do iracing/ (é onde o bug da sentinela vive); os limiares do driving_style nunca foram confrontados com telemetria capturada; unwrap/expect/panic aparecem só em código de teste, caminho de produção limpo.

Pontos de revisão:

- [Alta] Gate de enduro furado pela sentinela duracao_corrida_min=0
  - Arquivos: src-tauri/src/car/breakdown.rs:509, src-tauri/src/commands/iracing/previsao_quebras.rs:78, src-tauri/src/commands/iracing/roster.rs:652
  - Motivo: is_enduro_duration recebe a duração da config da categoria nesses dois call sites, e no Endurance essa constante vale 0 (o calendário sorteia 120 a 360 min). Resultado: is_enduro=false, e o disparo ao vivo e o forecast tratam uma prova de 6 horas como sprint, com DNF cheio e sem a rampa de fim. commands/race/despesa.rs:147 documenta a armadilha e usa a duração do CalendarEntry; os dois call sites do iracing/ seguem lendo a config. O contrato do gate merece um tipo que impeça passar a sentinela.
- [Alta] Sobrecusto de enduro linear e sem teto
  - Arquivos: src-tauri/src/car/breakdown.rs:84, src-tauri/src/car/breakdown.rs:524
  - Motivo: enduro_surcharge = ENDURO_COST_K × over sem clamp. Em 360 min o multiplicador de desgaste pós-alívio chega a ~12x, e uma prova única persiste desgaste várias vezes além do fim de vida da peça, o que zera qualquer decisão de manutenção seguinte. O comentário do próprio código exemplifica só 60 e 80 min; as durações reais do calendário de Endurance (120 a 360) nunca aparecem na calibração.
- [Alta] Ralo novo dormente enquanto a regalia velha segue ativa
  - Arquivos: src-tauri/src/economia/desenvolvimento.rs, src-tauri/src/finance/cashflow.rs:412, src-tauri/src/market/preseason/inicializacao.rs:232
  - Motivo: economia::desenvolvimento (o dreno do excedente, medido 9/9 no alvo pelo harness) não tem nenhum consumidor fora de economia/. Enquanto isso apply_offseason_competitiveness_impact continua rodando na pré-temporada e credita pontos de engineering e facilities sem debitar caixa, exatamente a fonte de dinheiro do nada que o doc de desenvolvimento.rs diz substituir. O superávit das equipes segue sem destino em produção.
- [Média] cashflow.rs mistura unidades e concentra assuntos demais
  - Arquivos: src-tauri/src/finance/cashflow.rs:365, src-tauri/src/finance/cashflow.rs:371
  - Motivo: Divisores em dólar absoluto (cash_balance/1_000_000, debt_balance/900_000) sobreviveram à reancoragem do jogo em meses de operação: a força de caixa satura em categorias ricas e nunca liga em categoria de base, com efeito por categoria não calibrado. O arquivo tem 978 linhas juntando amortização de dívida, bilheteria, offseason e biases, é o maior do finance/ e o ponto natural de próxima decomposição.
- [Média] Estados financeiros e estratégias como strings livres com fallback silencioso
  - Arquivos: src-tauri/src/finance/cashflow.rs:453, src-tauri/src/finance/cashflow.rs:488
  - Motivo: financial_state_bias e season_strategy_bias casam por &str ("elite", "healthy", "all_in") com braço _ neutro. Um typo ou um estado novo (as seis faixas de state.rs) passa sem erro e só zera o efeito. Um enum com from_str explícito fecharia o contrato, no mesmo padrão que Severity::from_key já usa em breakdown.rs.
- [Média] Doc de economia/mod.rs desatualizado e allow global escondendo código morto
  - Arquivos: src-tauri/src/economia/mod.rs:3, src-tauri/src/economia/mod.rs:36
  - Motivo: O doc afirma que nada do módulo é chamado pela simulação. finance/planning.rs, finance/state.rs, commands/race/despesa.rs e commands/race/fatura.rs já consomem ancora, temporada, receita e fatura. O #![allow(dead_code, unused_imports)] de módulo inteiro, justificado pela fase pré-integração, agora mascara import e função mortos de verdade.
- [Média] Limiares do estilo de pilotagem sem calibração de pista
  - Arquivos: src-tauri/src/car/driving_style.rs:16, src-tauri/src/car/driving_style.rs:47
  - Motivo: O próprio arquivo marca os oito limiares de detecção (limitador, short-shift, frenagem forte, serra de volante, G de zebra) como primeiro corte a calibrar com telemetria real. Eles decidem multa de até +30% de desgaste do jogador, e nenhum teste os confronta com captura real do SDK.
- [Média] breakdown.rs gigante e com proteção do jogador declarada provisória
  - Arquivos: src-tauri/src/car/breakdown.rs:558, src-tauri/src/car/breakdown.rs:1161
  - Motivo: 1.281 linhas somando quatro papéis: modelo de hazard, condições (pista e clima), regras de enduro e a máquina ao vivo (LiveBreakdown, BreakdownDirector, forecast). PLAYER_MAX_RELIEF=0.05 traz comentário admitindo que o número final depende de medir o desgaste real de time pobre no wiring, medição que ainda não aparece. Candidato a quebrar em submódulos como o resto do projeto faz.
- [Baixa] Fallbacks de classe multi-classe espalhados em três convenções
  - Arquivos: src-tauri/src/finance/planning.rs:84, src-tauri/src/car/cost.rs:79
  - Motivo: Endurance sem classe resolve para gt3 no planning, para o teto histórico 8 (lmp2) no cost e para endurance:lmp2 na divisão representativa do tier salarial. Cada escolha tem justificativa local escrita, e a dispersão cria três respostas diferentes para a mesma pergunta quando um call site novo esquecer a classe.
- [Baixa] Ciclo macroeconômico determinístico e idêntico em todo save
  - Arquivos: src-tauri/src/finance/economy.rs:24
  - Motivo: global_economic_health_for_season usa season_number.rem_euclid(6): recessão sempre nas temporadas múltiplas de 6, boom sempre na 3. Todo jogador vive a mesma sequência previsível; um sorteio semeado pelo save daria variedade sem perder reprodutibilidade.
- [Baixa] Números do carro herdados do GPRO e tenda de durabilidade sem medição citada
  - Arquivos: src-tauri/src/car/parts.rs:99, src-tauri/src/car/wear.rs:83, src-tauri/src/car/sim_bridge.rs:16
  - Motivo: pha_per_level e durabilidade vêm do modelo do GPRO com a nota de que os absolutos são placeholder; level_durability_mult é uma tabela fixa (0.60 a 1.15 a 0.50) sem referência a harness; CAR_PERF_MAX e o mapeamento linear do sim_bridge se declaram provisórios (chunk 8). São os coeficientes que definem o tradeoff desempenho x confiabilidade do jogo inteiro.


## Engenheiro falado, narrativa e notícias

O subsistema cobre a voz do engenheiro no rádio e toda a prosa gerada do jogo. O engenheiro responde ao push-to-talk por dois caminhos: peças de voz gravadas montadas por catálogo (fala.rs enumera cada chave com o texto exato a gravar, quebra.rs monta falas por colagem de sobrenome, aposto e trecho) e, quando o acervo não cobre, um dossiê de fatos em português (fatos.rs, responder.rs) que sobe ao servidor Cloud Run para o modelo redigir. narrative/ e commands/ai_news/ montam bundles de fatos curados (boletim, prévia, debrief, prévia de temporada, rodapé) e chamam o mesmo servidor via narrative/client.rs, que esconde os dois provedores (DeepSeek fora do pico, Gemini no pico e como fallback); em qualquer falha o front cai no template determinístico e nenhuma tela quebra. telemetry.rs é a telemetria de produto (anônima, opt-out, com fila de reenvio em disco) e diagnostico.rs o log rotativo com envio por ticket.

Componentes:

- engenheiro/fala.rs (~910 linhas): Renderizador do caminho gravado: famílias de peças (posição, restante, gap, pneu, combustível, estáticas) e o catálogo que é fonte única para gerar e tocar; tudo-ou-nada, peça faltando manda a pergunta ao modelo
- engenheiro/quebra.rs (~700 linhas): Fala de quebra de terceiro montada por colagem (abertura, sobrenome, aposto, trecho, coda), precedência de vínculo e degradação para a forma pela equipe
- engenheiro/fatos.rs (~564 linhas): O dossiê: EstadoAgora virado em linhas de fato sem sentinela, com núcleo fixo, blocos por intenção e reordenação do urgente
- engenheiro/peca_propria.rs (~354 linhas): Avisos e desfechos sobre o carro do jogador em 2ª pessoa, frases inteiras gravadas, mais o conselho de poupar e as falas da quali destruída
- engenheiro/responder.rs (~188 linhas): Composição das fontes (telemetria, save, memória da conversa, vocativo) para a resposta gravada e para o dossiê
- engenheiro/campeonato.rs, vizinhanca.rs, memoria.rs, tratamento.rs, tempo_volta.rs, classificacao.rs, ritmo.rs, momento.rs, marco.rs, volta_referencia.rs, nomes.rs, intencao.rs (~2100 linhas): Famílias e regras auxiliares: tabela do save no rádio, vizinho nomeado, delta desde a última resposta, vocativo, tempos de volta falados, janela da classificação, comentários espontâneos, silêncio situacional, marcos de carreira, referência de volta própria, pools de nomes e classificação de intenção por palavra-chave
- engenheiro/medicao_radio.rs (~315 linhas): Harness de medição (cfg test) que despeja a linha do tempo do rádio em JSON para análise externa
- narrative/client.rs (~625 linhas): Única saída de rede para o servidor de IA: cinco endpoints, timeout de 45 s por causa do cold start, guard que bloqueia a suíte de testes, limpeza de vazamento de meta-linguagem e apara de frase cortada
- narrative/beats.rs, tese.rs, contexto.rs, incidentes.rs, consulta.rs (~730 linhas): Curadoria determinística do boletim: beats com peso, limiar de relevância, tese dominante e renderização do contexto curado
- narrative/em_voo.rs (~124 linhas): Trava por chave que impede prefetch e tela de pagarem duas gerações do mesmo texto; passe com liberação no Drop
- news/mod.rs (~157 linhas): DTOs de notícia (NewsItem, NewsType, NewsImportance) com conversões leniente e estrita
- commands/ai_news/fatos.rs (~975 linhas): Montagem do fact bundle pós-corrida: 15 blocos (resultado, curso, anúncio, lesão, pressão, rivais, quebras, telemetria, arco) organizados em eixo, apoio e pano de fundo
- commands/ai_news/comandos.rs (~398 linhas): Cascas #[tauri::command] com spawn_blocking, cache por race_id/news_id, passe em voo e mapeamento de erro para status string
- commands/ai_news/telemetria.rs, tese.rs, arco.rs, engajamento.rs, tipos.rs (~796 linhas): Leitura do race trace para fatos, tese do debrief, memória entre etapas, gate de engajamento da prévia e DTOs de retorno
- telemetry.rs (~1071 linhas): Telemetria de produto anônima e opt-out: eventos de corrida, uso por rodada, entrega com retry e fila de reenvio em JSON Lines
- diagnostico.rs (~289 linhas): Log rotativo em arquivo, linha_unica para o caminho quente, redação do USERPROFILE e envio por ticket sob clique do jogador

Testes: Cobertura forte e deliberada no núcleo do engenheiro: engenheiro/tests/ tem o teste de alcance que varre estados, renderiza e exige que toda chave emitida exista no catálogo (o modo de falha que ele fecha é o engenheiro mudo sem erro), mais suítes por família (fala, campeonato, memoria, momento, tratamento, vizinhanca, marco) e testes nos submódulos quebra, ritmo, classificacao, tempo_volta e volta_referencia; medicao_radio.rs é harness de medição, sem asserção. narrative/ tem 561 linhas de teste e o guard de client.rs que prova que a suíte nunca chama o servidor de IA (o incidente dos 2.731 falsos usuários no Firestore). commands/ai_news/tests cobre tese, telemetria e caso_do_anuncio. telemetry.rs testa a fila em disco no próprio arquivo, diagnostico.rs testa o no-op sem init. Os buracos: build_post_race_facts não tem teste de ponta a ponta (só os helpers), as cascas Tauri de comandos.rs dependem de AppHandle e ficam sem teste, e o vocabulário de status strings que cruza para o React não tem guard de paridade.

Pontos de revisão:

- [Alta] build_post_race_facts é uma função de ~910 linhas sem teste próprio
  - Arquivos: src-tauri/src/commands/ai_news/fatos.rs:64
  - Motivo: Uma única função monta 15 blocos, abre 8+ áreas de query (teams, calendar, seasons, nemesis, injuries, drivers, race_breakdowns, ai_pre_race), recompõe a pressão de título espelhando simulation/pressure e carrega números mágicos sem calibração documentada (midia > 70 e > 87, deadzone de pace_delta 0,4, intensity >= 2,0). Os helpers tese e telemetria têm teste, a montagem inteira não tem nenhum, e uma regressão num bloco só aparece lendo o texto gerado.
- [Média] Cinco fetch_* duplicam o mesmo esqueleto de requisição
  - Arquivos: src-tauri/src/narrative/client.rs:238, 302, 371, 437, 502
  - Motivo: Gate de teste, builder do client, POST com segredo, match de status e limpeza (remover_vazamento_meta, aparar_frase_incompleta) repetem-se em cinco funções. Endpoint novo copiado de um exemplar errado nasce sem a limpeza de meta-linguagem ou sem o guard de rede, e o defeito é silencioso. Um helper genérico parametrizado por URL e tipo de resposta elimina a classe de erro.
- [Média] Doc do acervo afirma 1.056 peças; o acervo medido tem 3.943 e 324 MB
  - Arquivos: src-tauri/src/engenheiro/fala.rs:52
  - Motivo: O cabeçalho sustenta o argumento de que fundir frases é barato com um número quase 4x menor que o real (a família de tempo_volta sozinha tem 2.101 peças). Decisões de escala e de custo de regeração já foram tomadas em cima desse comentário. Atualizar o número e a conta de caracteres da Cloud TTS.
- [Média] Contrato Rust para React por strings de status ad hoc
  - Arquivos: src-tauri/src/commands/ai_news/tipos.rs, src-tauri/src/commands/ai_news/comandos.rs
  - Motivo: Os três DTOs devolvem status como string livre (ok, cached, unavailable, rate_limited, error, engagement_template) sem enum nem teste que trave o vocabulário. Um typo de um lado ou um case novo esquecido do outro degrada em silêncio para o template. Vale um tipo serde com variantes fixas ou ao menos constantes compartilhadas com teste.
- [Média] Dossiê do rádio com prosa hardcoded em português fora do rust_i18n
  - Arquivos: src-tauri/src/engenheiro/fatos.rs
  - Motivo: Todas as linhas do dossiê (Situação, Posição, Faltam, Saldo, avisos de pneu) são literais em PT, enquanto commands/ai_news/fatos.rs resolve tudo por rust_i18n. Jogador em en-US recebe fatos em português no caminho gerado do push-to-talk. Se a decisão é o rádio ser monolíngue por causa do acervo de voz, ela merece estar escrita no cabeçalho do módulo.
- [Baixa] Comentário de atrito só dispara no cruzamento exato do limiar
  - Arquivos: src-tauri/src/engenheiro/quebra.rs:181
  - Motivo: A coda sai quando abandonos_ate_aqui == 4 exatamente. Uma fala dupla (montar_duplo) não carrega coda e pode fazer a contagem pular de 3 para 5, silenciando o comentário na corrida inteira; contagem zerada do chamador (demo, import) tem o mesmo efeito. Conferir se o chamador garante incremento de um em um.
- [Baixa] Ritual repetido de base_dir, config e banco nos três comandos de IA
  - Arquivos: src-tauri/src/commands/ai_news/comandos.rs
  - Motivo: enrich_race_news_ai, pre_race_briefing_ai e post_race_debrief_ai repetem a resolução de app_data_dir, load do config, install_id, abertura do banco e o par cache, passe em voo, releitura. Extrair a preparação comum reduz o custo de cada endpoint novo e evita divergência no tratamento de erro.
- [Baixa] news/mod.rs com allow(dead_code) e conversão leniente convivendo com a estrita
  - Arquivos: src-tauri/src/news/mod.rs:1
  - Motivo: O allow(dead_code) de arquivo esconde item morto de verdade, e from_str devolve Corrida ou Media para valor desconhecido enquanto from_str_strict existe ao lado. Vale auditar quais call sites ainda usam a leniente e se o fallback silencioso é desejado.
- [Baixa] uso_tela aceita string livre do front e ignora valor desconhecido
  - Arquivos: src-tauri/src/telemetry.rs:628
  - Motivo: O filtro silencioso é intencional para o front não inventar métrica, e tem o efeito colateral de que uma tela nova com nome errado some sem erro nem linha de diagnóstico. Uma linha no log para valor não reconhecido tornaria o descarte visível.


## Módulos de suporte do domínio (generators, constants, calendar, rivalry, public_presence, common, tests_9d)

São os alicerces de dados e geração do mundo do Loop: catálogos estáticos de pistas (107 eventos com ids reais do iRacing), equipes, carros e categorias; geradores de identidade (nomes por nacionalidade, ids sequenciais, mundo inicial e histórico); o pipeline de calendário 9D (pools temáticos curados, seleção com slots narrativos, janelas de semana e persistência); o sistema de rivalidade entre pilotos e equipes com dois eixos e gatilhos calibrados por medição; e a presença pública e atração de bilheteria das equipes. tests_9d.rs fecha a área com provas de integração end-to-end da fase 9D.

Componentes:

- constants/tracks (dados, consultas, tipos, tests) (~1412 linhas): Catálogo hardcoded dos 107 eventos de pista que o jogador possui, com track_id real do iRacing (fonte: WeekendInfo.TrackID, jul/2026), grupo de chuva, flag gratuita e consultas (substituição determinística de pista paga por grátis, duração de quali).
- constants/teams (dados, consultas, tipos, tests) (~1878 linhas): Templates hardcoded das 60+ equipes por categoria (nome, cores, país-sede, marca, classe, performance/budget/reputação base) e consultas por categoria, classe endurance e marca.
- constants/categories.rs (~652 linhas): As 9 categorias da escada com toda a configuração de grid, corridas, duração, multi-classe e licenças; inclui o sentinela duracao_corrida_min=0 do Endurance.
- constants/geografia.rs (~293 linhas): Coordenadas de referência por país (onde a corrida acontece, ponto declarado) e haversine para o frete da logística; normaliza as três variantes legadas de nome de país.
- constants/historical_timeline.rs (~256 linhas): Anos de início por categoria/classe, ano de fundação de marcas reais (Ferrari 1929 etc.) e bandas de performance histórica por marca, hoje só para gt3.
- constants/{scoring,skill_ranges,timeline,cars,mod}.rs (~688 linhas): Tabelas de pontos e conversões score-tempo (documentadas como equivalência do modelo antigo), faixas de skill por tier, semanas do mercado/temporada, catálogo de carros com car_id do iRacing e labels i18n de tier/país.
- generators/names + nationality (~1385 linhas): 23 pools de nomes por nacionalidade (dados por continente), sorteio ponderado de nacionalidade (pesos somam 118), gênero (5% F) e identidade única com fallback determinístico anti-colisão.
- generators/ids.rs (~204 linhas): Sequenciador de ids legíveis (P001, T001, R001...) persistido na tabela meta, com recuperação de contador defasado contra as tabelas reais.
- generators/world (genesis, historico, tipos, tests) (~1200 linhas): Montagem do mundo do ano 1 (jogador encaixado na equipe escolhida) e do mundo histórico (fundações, bandas de performance, feeders), com pareamento N1/N2 por skill e shuffle anti-roteiro nas categorias spec.
- calendar/generator.rs (~748 linhas): Fundação declarativa do calendário temático: famílias de campeonato, regiões, pools curados de track_ids por categoria (GT4/GT3/LMP2/Endurance com palcos reais), pistas fortes para slots narrativos e rotação um-layout-por-venue por temporada.
- calendar/{selecao,montagem,geracao,janela,entry}.rs (~1142 linhas): Pipeline de geração: seleção com reserva de slots narrativos e resolução de conflitos entre categorias irmãs, montagem da etapa (clima, temperatura placeholder, voltas estimadas, corrida noturna garantida), janelas temporais e datas visuais.
- calendar/full_season (completo, parcial, tests) (~1056 linhas): Gerador 9D unificado: 74 entradas nas 9 divisões com janelas de semana escalonadas por prestígio (completo) e gerador parcial de migração v34 com regra de degradação documentada.
- rivalry (gatilhos, eventos, intensidade, noticias, decaimento, tests) (~2263 linhas): Rivalidade entre pilotos com dois eixos (histórico e recente), gatilhos de hierarquia, campeonato e colisão com peso de contexto calibrado por medição em mundos simulados, thresholds semânticos e notícia de escalada via i18n.
- rivalry/team (~1271 linhas): Gêmeo da rivalidade para equipes: motor de upsert nos mesmos dois eixos, gatilhos de campeonato, pista, mercado, derby e rivalidade herdada, com decaimento próprio.
- public_presence (team, atracao, medicao) (~1736 linhas): Presença pública da equipe (0.7/0.3 sobre a mídia do lineup), atração de bilheteria (presença x competitividade + vínculo local + história) e harness de medição da distribuição de fama (cfg(test), ignored, imprime em vez de assertar).
- common/time.rs (~10 linhas): Timestamp e ano corrente a partir do relógio local.
- tests_9d.rs (~947 linhas): Integração end-to-end da fase 9D: invariantes pós-criação (74 entradas, sw 10 a 51, zero LMP2 no calendário, zero tabelas legadas), imutabilidade do calendário, ciclo completo S1 a S3, jogador em endurance/lmp2 e recusa de save pré-baseline sem tocar no arquivo.

Testes: Cobertura forte nos módulos de lógica: calendar tem 1.641 linhas de teste (suíte geral de 943 + full_season completo 472 e parcial 226) cobrindo janelas, conflitos e determinismo; rivalry tem 1.530 (1.106 de piloto + 424 de equipe); generators/world tem 562 linhas de teste do pareamento e do mundo histórico. Os catálogos estáticos têm guards úteis: teams/tests (470) e tracks/tests (109, incluindo unicidade de ids, contagem exata de 107 eventos e amostra de ids corrigidos contra o iRacing); categories, geografia, scoring, skill_ranges, historical_timeline e nationality têm testes inline. tests_9d.rs prova a integração end-to-end com limpeza de diretórios temporários. Lacunas: driver_helpers.rs (roll_carisma e career_start_year_from_age) sem teste direto; timeline.rs sem teste algum das constantes de semana que o mercado e a temporada inteira consomem; cars.rs com um único teste; calendar/montagem depende da suíte geral e a regra da corrida noturna passa sem ninguém verificar que a pista iluminada existe no catálogo, que é exatamente onde o bug do id 556 se escondeu; public_presence/medicao.rs é harness de medição (imprime, roda com --ignored), então a distribuição de fama tem régua e tem zero asserção de regressão.

Pontos de revisão:

- [Alta] Janelas ditas canônicas divergem entre o gerador completo e o parcial
  - Arquivos: src-tauri/src/calendar/full_season/parcial.rs:5-28, src-tauri/src/calendar/full_season/completo.rs:33-43
  - Motivo: parcial.rs declara production woy_end=47 (sw 51) e endurance woy_end=45 (sw 49) como o mesmo offset do gerador completo; o array FULL_SEASON_WINDOWS do completo tem production terminando em woy 44 (sw 48) e endurance em woy 46 (sw 50). Um dos dois mente sobre o cânone, e o parcial roda em migração de save real (v34).
- [Alta] LIT_TRACK_ID aponta para pista inexistente e mata a preferência da corrida noturna
  - Arquivos: src-tauri/src/calendar/montagem.rs:20
  - Motivo: A constante vale 556 com comentário Charlotte Roval; no catálogo o Charlotte Roval é track_id 554 e o id 556 tem zero ocorrências em tracks/dados.rs. O find nunca casa, a etapa noturna cai sempre no sorteio aleatório do miolo e a intenção de preferir pista iluminada é código morto silencioso.
- [Média] Faixas de skill duplicadas e divergentes entre skill_ranges.rs e driver.rs
  - Arquivos: src-tauri/src/constants/skill_ranges.rs:5-13
  - Motivo: O próprio arquivo documenta que driver.rs usa tier_base_range com valores diferentes (tier 0: 20-48 contra spec 25-70, etc.) e nomes de dificuldade diferentes, com a promessa de resolver quando driver.rs consumir este módulo. A promessa está parada e duas fontes de verdade de balanceamento convivem.
- [Média] Nacionalidade em português sem acento e congelada em pt-BR no save
  - Arquivos: src-tauri/src/generators/nationality.rs:5-213, src-tauri/src/generators/names/identidade.rs:99
  - Motivo: nome_pt traz Britanico, Alemao, Frances, Holandes, Japones, Chines, Alema sem acentuação, e generate_pilot_identity persiste nacionalidade_label fixo em pt-BR. O jogador lê texto sem acento e o usuário en-US vê rótulo em português; o dado gravado ignora a troca de locale. O padrão do projeto é label por chave i18n resolvida no display, como country_label faz em constants/mod.rs.
- [Média] Duplicação estrutural entre genesis.rs e historico.rs
  - Arquivos: src-tauri/src/generators/world/genesis.rs:75-140, src-tauri/src/generators/world/historico.rs:42-130
  - Motivo: Os dois geradores repetem o mesmo laço por categoria: geração de equipes, sorteio e ordenação de pilotos por skill, pareamento N1/N2, shuffle anti-roteiro do tier 0 (comentário idêntico nas linhas 136 e 91) e emissão de contratos. Uma mudança de regra de pareamento exige tocar os dois em sincronia.
- [Média] TODO de design aberto: substituição de pista paga ignora a posse real do jogador
  - Arquivos: src-tauri/src/constants/tracks/consultas.rs:30
  - Motivo: free_or_substitute troca todo conteúdo pago por uma free determinística; o TODO(design final) pede trocar pela lista de pistas que o jogador realmente possui. O catálogo inteiro já é a crava de um usuário específico (jul/2026), então outro jogador com posse diferente recebe substituições erradas no export.
- [Média] Bandas de performance histórica só existem para gt3 e vivem em números mágicos por marca
  - Arquivos: src-tauri/src/constants/historical_timeline.rs:71-108
  - Motivo: historical_team_performance_band devolve None para qualquer categoria fora de gt3, e as faixas (Mercedes 14.8-16.0, Acura 4.5-8.5 etc.) são hardcoded por substring de nome real, sem registro de calibração. Mundos históricos em gt4, lmp2 e endurance nascem sem hierarquia de marca.
- [Média] Ano de início de carreira usa o relógio real da máquina
  - Arquivos: src-tauri/src/generators/driver_helpers.rs:62-64, src-tauri/src/models/driver_generation.rs:130
  - Motivo: career_start_year_from_age chama current_year() de common/time.rs (Local::now). Em mundo histórico com start_year no passado, o ano de início dos pilotos sai ancorado no ano civil de quem roda o jogo, e o mesmo save gerado em anos diferentes produz históricos diferentes.
- [Média] Três formatos de nome de país convivem e a normalização está espalhada
  - Arquivos: src-tauri/src/constants/mod.rs:31-56, src-tauri/src/constants/geografia.rs:26-27
  - Motivo: O dado cru de teams e tracks mistura bandeira emoji, sem emoji e sem acento (Japao, Suica, Franca); country_label e a normalização de geografia mantêm cada um a sua lista de variantes. País novo exige lembrar de três lugares, e um esquecimento falha silencioso (fallback = valor cru ou distância None).
- [Baixa] Duração de quali duplicada em duas funções irmãs
  - Arquivos: src-tauri/src/constants/tracks/consultas.rs:92-114
  - Motivo: get_qualifying_duration e duracao_classificacao_para implementam a mesma regra (ids longos 20, acima de 5 km 18, senão 15) com assinaturas diferentes. Alterar o corte de 5.0 km em uma e esquecer a outra desalinha export e simulação.
- [Baixa] Distribuição de intensidade de chuva duplicada fora da constante oficial
  - Arquivos: src-tauri/src/calendar/montagem.rs:117-131, src-tauri/src/constants/scoring.rs:98-102
  - Motivo: random_weather hardcoda os cortes 0.40/0.80 que reproduzem RAIN_INTENSITY_DISTRIBUTION de scoring.rs sem referenciá-la. Recalibrar a distribuição num lugar deixa o outro para trás.
- [Baixa] Strings de UI e de dado persistido em português cru no backend
  - Arquivos: src-tauri/src/calendar/montagem.rs:92, src-tauri/src/constants/scoring.rs:74-92, src-tauri/src/calendar/geracao.rs:34
  - Motivo: O nome da etapa nasce como format!("Rodada {} - {}") e vai para o banco; DifficultyConfig.nome traz Fácil/Médio/Difícil/Lendário fora do rust-i18n; e a mensagem de erro do LMP2 sai sem acento ("LMP2 e uma classe..."). Tudo que cruza para a tela em en-US aparece em português.
- [Baixa] Gêmeo de rivalidade de equipe duplica clamp e reusa normalize_pair com semântica de piloto
  - Arquivos: src-tauri/src/rivalry/team/motor.rs:14-21,64-67
  - Motivo: AXIS_MAX, AXIS_MIN e clamp são cópias de rivalry/intensidade.rs, e os ids de equipe viajam nos campos piloto1_id/piloto2_id do par normalizado, dependência documentada só por comentário. Funciona hoje e quebra fácil numa refatoração do modelo de par.
- [Baixa] Prova-mestra do 9D é uma função de teste de quase 300 linhas
  - Arquivos: src-tauri/src/tests_9d.rs:478-760
  - Motivo: ciclo_completo_s1_s2_s3 concentra o ciclo de duas temporadas inteiro num único #[test]; uma falha no meio esconde as asserções seguintes e o diagnóstico exige ler o teste todo. Vale quebrar em fases nomeadas com helpers de asserção.
- [Baixa] expect em caminho de produção do calendário
  - Arquivos: src-tauri/src/calendar/geracao.rs:186, src-tauri/src/calendar/janela.rs:111-124
  - Motivo: O gerador consome ids com ids_iter.next().expect("race id") e as funções de data usam expect encadeado. Os invariantes hoje seguram (contagem de ids pré-alocada, meses válidos por construção), e um descompasso futuro vira panic no fluxo de criação de temporada em vez de erro tratável.


## frontend, estado e telas (src/stores, src/pages, src/pages/tabs, src/hooks, src/utils, src/lib)

Camada React do Loop: um store Zustand único (useCareerStore) composto por cinco slices de domínio que compartilham o mesmo (set, get) e concentram a maioria dos invoke para o backend Rust. As páginas são as telas de fora da carreira (MainMenu, NewCareer, LoadSave, Settings) e o Dashboard, que orquestra as abas de pages/tabs, os pollers do iRacing e três hotkeys de debug. utils/ guarda formatação, cores, país de pista e a regra de pouso pós-corrida; lib/ guarda a cadeia de áudio do rádio (voz do engenheiro, spotter, push-to-talk, filtro e fila de carga do acervo de 3.943 peças Opus).

Componentes:

- src/stores/useCareerStore.js + career/ (5 slices, state, helpers) (~1613 linhas): Hub de estado: composição dos slices carreira/corrida/mercado/temporada/cache pré-corrida sobre um initialState único; helpers puros e fetches compartilhados
- src/pages/Dashboard.jsx (~492 linhas): Orquestrador das abas, das telas de sobreposição (resultado, mercado, convocação), do poller de import do iRacing e dos hotkeys de debug Ctrl+M/L/K
- src/pages/Settings.jsx (~858 linhas): Tela de configurações: idioma, spotter, macro de amarela, captura de corrida, demo do overlay, backups; 18 invoke inline num único componente
- src/pages/MainMenu.jsx (~810 linhas): Menu principal com intro cinematográfica, lista de saves, exclusão de carreira e ícones SVG inline
- src/pages/NewCareer.jsx (~784 linhas): Wizard de criação de carreira sobre o draft do backend (get/update/finalize_career_draft) com poll de progresso
- src/pages/tabs/ (StandingsTab, NextRaceTab, CalendarTabRedesign, GlobalDriversTab, TeamRecordsTab, NewsMagazineTab) (~2178 linhas): Abas do Dashboard: classificação, sala de estratégia, calendário, pilotos globais, recordes de equipe, revista de notícias
- src/pages/tabs/myteam/ e atlas/ + versões v1 na raiz de tabs/ (~1038 linhas): Duas abas com seletor de versão (MY_TEAM_VERSION e ATLAS_VERSION fixos em 2); as v1 seguem vivas como rollback com testes e i18n próprios
- src/pages/tabs/nextRace*.js + inboxMessages.js (~1025 linhas): Módulos puros de texto da sala de estratégia (contexto, tese, briefing, editorial, inbox), cada um com teste comportamental e de i18n
- src/lib/ (~4211 linhas): Cadeia de áudio do rádio: engenheiroVoz (761), pttEngenheiro (456), spotterVoice (394), microfone (321), filtroRadio, volumeRadio, filaDeCarga, detecção do shell Tauri e updater
- src/utils/ (~2985 linhas): Formatadores, cores de equipe e categoria, país/imagem/banner de pista (trackCountries gerado do Rust), pouso pós-corrida, SFX procedurais
- src/hooks/ (~219 linhas): useTempoDeTela, useDeferredLoading, useExitToMenu e o stub useTauri.js de uma linha

Testes: Forte no núcleo: o store composto tem useCareerStore.test.js com 1016 linhas; as abas principais têm testes maiores que o próprio código (StandingsTab 1656, MyTeamTab 1257, GlobalDriversTab 899, GlobalTeamsTab 869, NextRaceTab 616, TeamRecordsTab 470); os módulos puros nextRace* e inboxMessages têm cada um teste comportamental e teste de i18n dedicado; utils e lib cobrem quase tudo (formatters, teamColors, postRaceLanding, trackImages, weather, pttEngenheiro, engenheiroVoz, spotterVoice, microfone, volumeRadio, pttConfig, pttFreio, pausasDoRadio). Os buracos: MainMenu.jsx (810 linhas) sem nenhum teste, CalendarTabRedesign e NewsMagazineTab sem teste, MyTeamTabV2 sem teste direto (o teste gordo protege a v1 desligada), Settings com teste raso (190 linhas para 858), e os utilitários sfx, calendarShared, trackBanners, filtroRadio, filaDeCarga e updater sem cobertura. A suíte estrutural de scripts/tests cobre contratos de PTT, spotter, controles de janela e paleta; falta um guard para os nomes de comando do invoke.

Pontos de revisão:

- [Alta] Settings.jsx é um monólito de 858 linhas com 23 useState e 18 invoke inline
  - Arquivos: src/pages/Settings.jsx:21
  - Motivo: Um único componente concentra spotter, macro de amarela, captura, demo do overlay, config e backups, falando com ~12 comandos Tauri diferentes sem nenhum hook extraído. O teste (Settings.test.jsx, 190 linhas) cobre fração pequena disso. Qualquer mexida arrisca o resto da tela.
- [Alta] Listas de reset do estado duplicadas à mão em quatro lugares, com omissões reais
  - Arquivos: src/stores/career/careerSlice.js:22, src/stores/career/helpers.js:170, src/stores/career/seasonSlice.js:31, src/stores/career/marketSlice.js:259
  - Motivo: loadCareer, advanceSeason, finalizePreseason e buildResumeUiState repetem blocos de ~15 chaves de reset com diferenças silenciosas: os ramos end_of_season e final de buildResumeUiState e o fallback de erro em careerSlice.js:64 omitem preseasonFreeAgents (e o fallback omite também playerSpecialOffers e acceptedSpecialOffer), então valor velho sobrevive à troca de contexto. Chave nova no estado exige lembrar de quatro listas.
- [Média] Contrato Rust↔React por string sem guard estrutural
  - Arquivos: scripts/tests/, src-tauri/src/lib.rs
  - Motivo: 86 arquivos do frontend chamam invoke com o nome do comando em string e nenhum teste confere se esses nomes existem no generate_handler (~150 entradas). Um typo ou um comando removido só aparece em runtime, como erro silencioso nos catch vazios. A suíte scripts/tests já faz guards de contrato para PTT, spotter e window controls; este é o contrato mais largo e está descoberto.
- [Média] Duas abas com versão dupla viva (v1 + v2) e seletor congelado em 2
  - Arquivos: src/pages/tabs/myteam/index.js, src/pages/tabs/atlas/index.js, src/pages/tabs/MyTeamTab.jsx, src/pages/tabs/GlobalTeamsTab.jsx
  - Motivo: MY_TEAM_VERSION e ATLAS_VERSION estão fixos em 2, e as v1 (129 + 320 linhas) continuam compiladas, com testes enormes rodando (MyTeamTab.test 1257 linhas, GlobalTeamsTab.test 869) e chaves de i18n mantidas em paridade. Vale decidir a data de morte das v1 para cortar ~2,5k linhas de manutenção.
- [Média] Slice de cache importa código da camada de páginas e espelha fetch à mão
  - Arquivos: src/stores/career/preRaceCacheSlice.js:3, src/components/race/nextrace/useBriefingData.js:74
  - Motivo: O store depende de pages/tabs/nextRaceContext (inversão de camada, funciona por o módulo ser puro). E a lista de comandos pré-buscados (get_drivers_by_category, get_teams_standings, get_briefing_phrase_history, get_breakdown_forecast) precisa espelhar manualmente a de useBriefingData; se a Sala de Estratégia passar a pedir um comando novo, o cache fica incompleto em silêncio e o flash que ele existe para evitar volta.
- [Média] Três stubs TODO vazios: useTauri.js, useUIStore.js, useNotificationStore.js
  - Arquivos: src/hooks/useTauri.js:1, src/stores/useUIStore.js:1, src/stores/useNotificationStore.js:1
  - Motivo: Os três dizem "TODO: Implementar". useTauri.js tem 1 linha e os dois stores criam Zustand vazio sem nenhum consumidor fora de stores/. Código morto que confunde quem chega (o CLAUDE.md os descreve como triviais em vez de vazios); remover ou implementar.
- [Média] Páginas grandes sem nenhum teste: MainMenu e CalendarTabRedesign
  - Arquivos: src/pages/MainMenu.jsx, src/pages/tabs/CalendarTabRedesign.jsx, src/pages/tabs/NewsMagazineTab.jsx, src/pages/tabs/myteam/MyTeamTabV2.jsx
  - Motivo: MainMenu tem 810 linhas (intro, saves, exclusão de carreira) e zero teste; CalendarTabRedesign (356) e NewsMagazineTab (155) idem; MyTeamTabV2 não tem teste direto (a v1 tem 1257 linhas de teste que protegem a versão desligada). MainMenu ainda carrega constantes mágicas de layout (POS panelX 565, yOffset -85 na linha 49) sem guard visual.
- [Média] invoke direto em componentes fora do padrão declarado no CLAUDE.md
  - Arquivos: src/pages/Settings.jsx, src/pages/NewCareer.jsx, src/pages/Dashboard.jsx:122, src/pages/MainMenu.jsx:300
  - Motivo: O padrão do projeto põe invoke nos slices ou em hooks use* de dados. Settings (18 chamadas), NewCareer (7), Dashboard (iracing_focus_self_if_closed dentro de setInterval) e MainMenu (list_saves, delete_career) chamam inline no corpo do componente, sem cache nem tratamento uniforme de erro.
- [Baixa] seasonSlice mistura três assuntos e metade é legado 9D
  - Arquivos: src/stores/career/seasonSlice.js:135
  - Motivo: 464 linhas juntando virada de temporada, animação do calendário (startCalendarAdvance, ~110 linhas) e o Bloco Especial marcado como legado para saves pré-v33 (~215 linhas, das linhas 135 a 349). Quando os saves antigos puderem ser considerados mortos, metade do arquivo sai; até lá, vale ao menos separar o legado num slice próprio.
- [Baixa] Strings de debug em português visíveis ao jogador fora do t()
  - Arquivos: src/pages/Dashboard.jsx:174, src/pages/Dashboard.jsx:212, src/pages/Dashboard.jsx:242
  - Motivo: Os flashes dos hotkeys Ctrl+M/L/K ("DEBUG: pulando corridas...") são texto pt hardcoded renderizado no overlay. O guard de i18n não os pega por serem argumento de função e não texto JSX. Impacto restrito a atalhos de debug, e ainda assim é texto de UI fora do padrão.
- [Baixa] Utilitários sem teste, alguns puros e fáceis de cobrir
  - Arquivos: src/utils/sfx.js, src/utils/calendarShared.js, src/utils/trackBanners.js, src/lib/filtroRadio.js, src/lib/filaDeCarga.js, src/lib/updater.js
  - Motivo: sfx (302 linhas de Web Audio) e filtroRadio (171) são difíceis em jsdom e aceitáveis sem teste. filaDeCarga (44, semáforo puro que protege o acervo de 3.943 peças contra ERR_INSUFFICIENT_RESOURCES) e calendarShared (137) são puros e testáveis hoje.
- [Baixa] Números de calibração de UX espalhados e sem fonte comum
  - Arquivos: src/stores/career/helpers.js:104, src/utils/postRaceLanding.js:22, src/pages/Dashboard.jsx:29, src/pages/Dashboard.jsx:116
  - Motivo: Duração da animação do calendário (1500/3000 ms, limiares 3/14 dias), NEWS_READ_MS 15000 com limites 3/2 pulos, feedback de chegada 1000 ms e os intervalos dos pollers (4000 e 1500 ms) vivem cada um no seu arquivo. Todos estão bem comentados no local; centralizar só se virarem ajuste de produto.


## frontend, componentes e overlay (src/components, src/overlay, src/i18n, src/dev)

Camada de apresentação do Loop: componentes React por domínio (race, season, driver, team, iracing, standings, news, calendar, layout, ui, system, wizard) que desenham o que o Rust simula, com os invoke concentrados em hooks use*.js locais das telas. O src/overlay contém as duas janelas transparentes sobre o iRacing (torre de tempos em canvas compartilhado entre monitor e VR, rádio do engenheiro com PTT e feed de quebras). O src/i18n guarda a infra i18next com pt-BR como locale-base, paridade pt/en garantida por teste e um auditor que bloqueia string PT crua em .jsx. O src/dev são páginas de bancada de TTS e PTT que somem do build de produção.

Componentes:

- src/components/driver/v2/DriverDetailModalV2.jsx (~5505 linhas): Ficha do piloto v2: modal centralizado com seções, curvas SVG, tooltips e portal; ~91 funções internas num arquivo só
- src/components/team/v2/TeamHistoryDrawerV2.jsx (~4525 linhas): Dossiê e história da equipe v2: 63 funções internas, um único export
- src/overlay/towerCanvas.js (~1297 linhas): Renderizador único da torre de tempos em canvas, serve o overlay de monitor e o buffer de VR (supersample 2x, 512x1024)
- src/components/race/RaceResultViewV2.jsx (~1216 linhas): Tela de pós-corrida v2, lê EventRepercussionSummary do backend
- src/components/driver/v2/curvaDeCarreira.jsx (~1121 linhas): Gráfico SVG da curva de carreira do piloto, geometria própria
- src/components/season/SeasonChampionOverlay.jsx (~847 linhas): Pop-up de campeão da temporada com gráfico SVG e CSS dedicado
- src/components/layout/Header.jsx (~754 linhas): Cabeçalho do Dashboard: ~20 seletores do store, orquestra avanço de calendário, convocação e desvio pelas notícias de fim de ano
- src/components/iracing/RosterGenPanel.jsx (~726 linhas): Exportação de AI roster e AI season para o iRacing, 12 pontos de invoke
- src/components/iracing/PostRacePanel.jsx (~690 linhas): Importação do resultado oficial da corrida real de volta para a carreira
- src/overlay/OverlayPositionPanel.jsx (~682 linhas): Posicionamento dos quads de VR (torre e rádio): pose em localStorage empurrada para a memória compartilhada via comandos vr_overlay_*/vr_engineer_*
- src/components/season/PreSeasonView.jsx (~635 linhas): Mercado da pré-temporada; mistura painéis v1 e v2 atrás de LayoutSwitch em runtime
- src/overlay/EngineerRadio.jsx (~464 linhas): Card do rádio do engenheiro: funde feeds de quebra, ritmo e avisos, dispara fala e persiste posição da janela
- src/overlay/useOverlayData.js (~125 linhas): Poll único de get_overlay_data na janela principal com barramento local e relé por evento para a janela de overlay
- src/components/race/nextrace/useBriefingData.js (~275 linhas): Maior hook de dados: monta o briefing pré-corrida com standings, previsão de quebras e cache de pré-corrida
- src/i18n/ (~10103 linhas): Infra i18next: dois common.json de 4.468 linhas, format.js, e três testes-guarda (cobertura, paridade de chaves, i18n geral)
- src/dev/ (~1507 linhas): Bancadas de TTS e PTT roteadas só com import.meta.env.DEV em App.jsx

Testes: Cobertura muito desigual. Bem servidos: driver/v2 (teste de 4.482 linhas do modal), team/v2 e myteam/v2 (atlasV2Geometry e gridMetrics com teste espelho do tamanho do fonte), season (PreSeasonView 1.185, SeasonChampionOverlay, ConvocationView), layout (Header, WindowControlsDrawer, guard de i18n do chrome), ui/Tooltip e os módulos puros do overlay (towerRows, towerAnimation, tireCompounds, towerCanvas com smoke de 177 linhas para 1.297 de fonte). O i18n tem três guardas dedicados (cobertura de strings, paridade pt/en, formatação) mais a suíte estrutural de scripts/tests. Zerados: standings (0 de 9), calendar (0 de 7), wizard (0 de 4), news (1 de 9), iracing (1 de 10, justamente a área que fecha o loop com o jogo real), todos os hooks de dados use*.js (useBriefingData, useCalendarData, useMagazineData, useIracingExport, usePreRaceAi) e a cadeia PTT/rádio/posição de VR do overlay. Os arquivos V1 mortos ou semivivos carregam testes próprios que gastam manutenção sem proteger tela em produção.

Pontos de revisão:

- [Alta] DriverDetailModalV2 é um arquivo-deus de 5.505 linhas
  - Arquivos: src/components/driver/v2/DriverDetailModalV2.jsx
  - Motivo: ~91 funções e componentes internos, 27 useState/useEffect e mais de 60 símbolos importados de curvaDeCarreira num único arquivo. O teste de 4.482 linhas protege o comportamento e ao mesmo tempo trava qualquer refatoração barata. As peças de gráfico (balões, faixas, molduras) já têm nomes próprios e pedem extração para módulos irmãos como as de detalhes/.
- [Alta] TeamHistoryDrawerV2 com 4.525 linhas e um export só
  - Arquivos: src/components/team/v2/TeamHistoryDrawerV2.jsx:90
  - Motivo: 63 funções internas atrás de um único export. O padrão do próprio projeto (atlasV2Geometry.js, gridMetrics.js extraídos e testados à parte) mostra o caminho; o miolo de rótulos e seleção de melhores resultados (linhas ~2651 e ~4430) é lógica pura extraível.
- [Alta] Contrato VR_W/VR_H com o Rust guardado só por comentário
  - Arquivos: src/overlay/towerCanvas.js:44
  - Motivo: O comentário exige que VR_W/VR_H casem com IRACER_OVERLAY_W/H em shared_frame.h e W/H em vr_overlay.rs. Nenhum teste (nem towerCanvas.test.js nem a suíte estrutural de scripts/tests) assevera essa igualdade lendo os dois lados. Um drift produz frame de VR corrompido em silêncio. Um guard estrutural que leia os três arquivos resolve barato.
- [Alta] Painéis de iracing/ sem teste no coração do loop principal
  - Arquivos: src/components/iracing/RosterGenPanel.jsx, src/components/iracing/PostRacePanel.jsx, src/components/iracing/IracingConnectedOverlay.jsx
  - Motivo: RosterGenPanel (726 linhas, 12 invokes), PostRacePanel (690, 5 invokes) e IracingConnectedOverlay (624, 5 invokes) são a ponte exportar-correr-importar que define o produto e o único teste do diretório é o de PttEngenheiroSettings. Qualquer mudança de DTO no Rust quebra essas telas sem rede.
- [Média] Árvore V1 morta ou semiviva duplicando manutenção
  - Arquivos: src/components/race/RaceResultView.jsx, src/components/race/raceresult/ (13 arquivos), src/components/driver/DriverDetailModal.jsx, src/components/team/TeamHistoryDrawer.jsx, src/components/season/preseason/ (painéis v1)
  - Motivo: RaceResultView.jsx e todo o raceresult/ não têm importador vivo (Dashboard usa RaceResultViewV2 direto) e o subdiretório inteiro carrega i18n-ignore-file, ou seja, código morto que ainda escapa do guard de i18n. DriverDetailModal v1 (749+594 linhas) e TeamHistoryDrawer v1 (1.120) seguem vivos atrás de seletores de versão em driver/index.js e team/history/index.js, e PreSeasonView mistura painéis v1 e v2 em runtime. Decidir o corte do v1 elimina milhares de linhas de superfície.
- [Média] Prosa PT em módulos .js escapa do auditor de i18n
  - Arquivos: src/components/season/preSeasonFormatters.js:11, src/components/driver/globalDriverRanking.js:18
  - Motivo: O i18nAudit.mjs varre só .jsx por decisão documentada (scripts/i18nAudit.mjs:15). Vazamentos concretos: CATEGORIES com label "Todas" e SUBCAT_LABELS ("Mazda Cup Principal") em preSeasonFormatters.js chegam à tela em PT no locale en-US; DEFAULT_FILTERS de globalDriverRanking usa "Todos"/"Todas" como valor-sentinela de filtro e texto de UI ao mesmo tempo, acoplando estado a idioma.
- [Média] Backend entrega valores de domínio em português como contrato
  - Arquivos: src/components/race/raceFactsContext.js:14
  - Motivo: INJURY_SEVERITY_KEY mapeia "Leve"/"Moderada"/"Grave"/"Critica" vindos do campo lesao_ativa_tipo do banco para chaves i18n. O contrato Rust-React depende de string PT literal (incluindo a grafia sem acento de "Critica"); uma correção de acentuação no backend quebra a gravidade da lesão em silêncio.
- [Média] Mapa categoria para logo triplicado
  - Arquivos: src/overlay/towerCanvas.js:17, src/components/team/v2/atlasCategoryLogos.js:6, src/components/race/raceresult/constants.js
  - Motivo: CATEGORY_META do towerCanvas, atlasCategoryLogos (cujo próprio comentário admite ser quase igual ao de raceresult/constants.js) e o mapa do raceresult são três cópias do mesmo dicionário categoria/nome/arquivo de logo. Categoria nova exige lembrar dos três; unificar num utils resolve.
- [Média] standings/ e calendar/ com zero testes
  - Arquivos: src/components/standings/standingsLadder.js, src/components/standings/DriverStandingsTable.jsx, src/components/calendar/useCalendarData.js
  - Motivo: standings tem 9 fontes (standingsLadder.js com 208 linhas de lógica pura de escada) e calendar tem 7 (useCalendarData com 202 linhas derivando mapas por data e estatísticas), tudo sem nenhum .test. São módulos puros e baratos de testar.
- [Média] Cadeia PTT e rádio do overlay inteira sem teste
  - Arquivos: src/overlay/EngineerRadio.jsx, src/overlay/EngenheiroPttAuto.jsx, src/overlay/OverlayPositionPanel.jsx
  - Motivo: EngineerRadio (fusão de três feeds via useMaisRecente com comparação por referência), EngenheiroPttAuto (7 invokes) e OverlayPositionPanel (682 linhas, pose persistida em localStorage e espelhada no backend, com poses de fábrica calibradas na mão como y 0.61, z -1.29, pitch 30) não têm nenhum teste. A memória do projeto já registra um episódio de spotter mudo nessa região; os módulos puros vizinhos (towerRows, towerAnimation) mostram que dá para testar.
- [Média] Lógica pura grande sem teste em team e season
  - Arquivos: src/components/team/worldTeamChartGeometry.js, src/components/season/preSeasonFormatters.js, src/components/driver/v2/curvaDeCarreira.jsx, src/components/driver/v2/CurvaDeCampeonato.jsx
  - Motivo: worldTeamChartGeometry (669 linhas) e preSeasonFormatters (700) são funções puras sem .test, na contramão dos pares atlasV2Geometry e gridMetrics que têm teste espelho do mesmo tamanho do fonte. As duas curvas SVG do driver/v2 (1.121 e 842 linhas) só são exercitadas indiretamente pelo teste do modal.
- [Baixa] Header acumula orquestração de avanço além de layout
  - Arquivos: src/components/layout/Header.jsx
  - Motivo: 754 linhas com ~20 seletores do store, invoke próprio de campeão e a regra do clique extra que desvia pelas notícias no fim do campeonato. É lógica de fluxo de temporada morando num componente de layout; o teste de 346 linhas cobre parte, e extrair o fluxo para um hook deixaria o cabeçalho trivial.
- [Baixa] Posição de fábrica do rádio espelha tauri.conf.json na mão
  - Arquivos: src/overlay/EngineerRadio.jsx:20
  - Motivo: FACTORY_POS x 653, y 195 precisa espelhar o x/y da janela engineer em tauri.conf.json segundo o próprio comentário. Mudança no conf sem tocar aqui desloca o padrão de fábrica de quem nunca moveu a janela.


## As três suítes de teste do Loop (guards estruturais em scripts/tests, vitest em src, cargo test em src-tauri)

O projeto tem três suítes independentes: 29 guards estruturais node --test que leem código-fonte como texto (3.188 linhas), 89 arquivos vitest em jsdom (27.669 linhas) com i18n real inicializado por src/test/setup.js, e 3.062 funções #[test] no Rust, organizadas em pastas tests/ por módulo e em blocos inline #[cfg(test)] espalhados por mais de 200 arquivos. O CI (ci.yml, windows-latest) roda test:all e cargo test a cada push; o pre-commit roda só o i18nAudit. Os guards estruturais são a peça mais original: fecham contratos entre camadas que não se conhecem (Rust emite chave, script gera .opus, JS decide pausa) e vigiam regressão visual e de acentuação sem screenshot.

Componentes:

- scripts/tests/*.test.mjs (29 guards) (~3188 linhas): Guards estruturais node --test que leem .jsx/.rs/.json como texto: contratos Rust↔JS↔acervo de áudio (spotter-chaves, quebra-pecas, engenheiro-pecas, ptt-contrato-ponte), consistência visual (team-palette, dashboard-alignment, window-controls x5), acentuação de copy (portuguese-copy-accents), sanidade de encoding em src, src-tauri e .claude (text-encoding-sanity)
- scripts/tests/driver-detail-modal.test.mjs (~827 linhas): Maior guard: assevera camadas, animação de saída e wiring do drawer de piloto cruzando 4 arquivos fonte por regex
- suíte vitest (89 arquivos em src/**) (~27669 linhas): Testes de componente e de utilitário em jsdom; setup.js (6 linhas) inicializa o i18next real; inclui i18nCoverage.test.js (mesmo checker do pre-commit) e localeParity.test.js (paridade pt-BR/en-US)
- src/components/driver/v2/DriverDetailModalV2.test.jsx (~4482 linhas): Maior arquivo vitest do projeto, 16% do volume da suíte de UI num componente só
- src-tauri: 3.062 funções #[test]: Duas formas de organização: pastas tests/ por módulo (career, overlay, historical_draft, behavior, economia, engenheiro, race, save, season_preview, world_footer, db/queries parcial) e blocos inline #[cfg(test)] em ~230 arquivos (finance, promotion, evolution, simulation, spotter, db/queries maioria)
- src-tauri/src/commands/career/tests/mod.rs (~5897 linhas): Monólito de teste da lógica de carreira; a lógica foi fatiada em 10 irmãos por área e os testes ficaram num arquivo só
- src-tauri/src/commands/historical_draft/tests/mod.rs (~1747 linhas): Testes do draft histórico mais 3 harnesses de medição sob #[ignore] (deflação da escada, absorção de lesionados, rivalidade entre companheiros); área com instabilidade conhecida
- src-tauri/src/iracing_sdk/behavior/tests/mod.rs (~753 linhas): Testes dos sinais de comportamento tier 1 a 3 do SDK
- .github/workflows/ci.yml + .githooks/pre-commit: CI roda as três suítes em windows-latest (cargo test exige npm run build antes); o pre-commit roda apenas o auditor de i18n em .jsx no stage

Testes: Densa onde o domínio é puro: simulation, finance, promotion, evolution, market, rivalry, spotter e a maioria de db/queries têm teste inline; economia e engenheiro têm pastas tests/ bem fatiadas; a UI tem 89 arquivos vitest com i18n real e os 29 guards pegam contrato e regressão visual que teste de comportamento não vê. Buracos concretos: commands/iracing/ inteiro (o caminho principal do jogo), os slices Zustand de src/stores/career/, components/standings e wizard, MainMenu, CalendarTabRedesign, NewsMagazineTab, 19 dos 23 arquivos de src/overlay, e sete queries de db fora do padrão do diretório. iracing_sdk/imp/ é gap aceitável por depender do SDK real. Instabilidade conhecida concentrada em historical_draft (4 falhas pré-existentes, 1 flaky, 3 harnesses sob ignore) e no risco de contaminação de locale sem #[serial]. O pre-commit cobre só i18n; guards e vitest dependem do CI para rodar.

Pontos de revisão:

- [Alta] Camada de comandos do iRacing sem nenhum teste
  - Arquivos: src-tauri/src/commands/iracing/ (17 arquivos: importacao.rs, resultado.rs, roster.rs, temporada.rs, clima.rs, pintura.rs, previsao_quebras.rs, adaptativo.rs, modo_janela.rs e outros)
  - Motivo: O caminho principal do jogo (exportar grid, importar resultado oficial) atravessa essa pasta e ela não tem tests/ nem #[cfg(test)] em arquivo algum. Os geradores puros por baixo (roster_gen, season_gen, results_gen) têm teste inline, a orquestração dos comandos fica descoberta. iracing_sdk/imp/* depende de hardware e é gap aceitável; a camada de comandos é testável com base_dir.
- [Alta] Monólito de 5.897 linhas em career/tests
  - Arquivos: src-tauri/src/commands/career/tests/mod.rs
  - Motivo: A lógica de carreira foi fatiada em standings, queries, season_flow, market_window, lifecycle, interests, briefing, save_state, vacancies e debug; os testes de tudo isso vivem num arquivo único. Modelos melhores já existem no próprio repo: economia/tests e engenheiro/tests são fatiados por tema.
- [Alta] Instabilidade conhecida no historical_draft
  - Arquivos: src-tauri/src/commands/historical_draft/tests/mod.rs
  - Motivo: Área com 4 falhas pré-existentes e 1 teste flaky documentados; além disso 3 harnesses lentos sob #[ignore] só rodam sob demanda, então a calibração que eles medem envelhece em silêncio. Há ainda o teste do nascimento no gt3 que mede o alvo errado (mede quem nunca correu). Merece um passe de saneamento com registro do que é regressão e do que é ruído.
- [Média] Guard de acentuação protege só 11 arquivos hardcoded
  - Arquivos: scripts/tests/portuguese-copy-accents.test.mjs
  - Motivo: A lista FILES_AND_FORBIDDEN_COPY nomeia 11 arquivos e fragmentos específicos; tela nova nasce fora do guard e o teste quebra com readFile se um arquivo listado for movido. O text-encoding-sanity já varre diretórios inteiros e mostra o padrão melhor: varrer src/ e procurar padrões de mojibake e palavras sem acento genéricas.
- [Média] Slices do career store sem teste direto
  - Arquivos: src/stores/career/ (7 arquivos, 0 testes; useCareerStore.test.js:1 testa só a composição)
  - Motivo: Os invoke que cruzam a ponte Rust↔React moram nos slices e só são exercitados com mock do @tauri-apps/api. Um campo renomeado num DTO serde passa no vitest e no cargo test e quebra em produção. Vale um guard estrutural que confira os nomes de comando invocados contra o generate_handler de lib.rs.
- [Média] Guards de contrato extraem chaves de Rust por regex
  - Arquivos: scripts/tests/spotter-chaves-contrato.test.mjs, scripts/tests/quebra-pecas-contrato.test.mjs, scripts/tests/engenheiro-pecas-do-app.test.mjs
  - Motivo: O triângulo Rust → .opus → JS é fechado lendo os .rs como texto. Renomear spotter_frente.rs ou mudar a forma de declarar a chave no Rust fura a extração; se a regex passa a casar zero chaves o guard pode passar vazio. Conferir que cada extração encontrou um mínimo de chaves tornaria a falha ruidosa. spotter_lento.rs está excluído de propósito e documentado, esse cuidado é o padrão a seguir.
- [Média] Overlay com 23 fontes e 4 testes
  - Arquivos: src/overlay/ (testes só em tireCompounds, towerAnimation, towerRows, towerCanvas)
  - Motivo: As janelas overlay e engineer são a superfície usada em corrida e VR, onde bug aparece na pior hora. O preview da torre não redesenha no HMR, o que desestimula verificação manual e aumenta o valor de teste automatizado aqui.
- [Média] Componentes de UI inteiros sem teste
  - Arquivos: src/components/standings/ (9 arquivos), src/components/wizard/ (4), src/components/iracing/ (10 fontes, 1 teste), src/pages/MainMenu.jsx, src/pages/tabs/CalendarTabRedesign.jsx, src/pages/tabs/NewsMagazineTab.jsx
  - Motivo: Diretórios com zero vitest enquanto vizinhos têm cobertura densa. O wizard é o funil de criação de carreira e a aba de calendário redesenhada é recente, os dois pontos onde regressão custa mais.
- [Média] Maior guard acoplado a detalhe de implementação
  - Arquivos: scripts/tests/driver-detail-modal.test.mjs (827 linhas)
  - Motivo: Assevera nomes de variável (selectedDriverId), classes CSS e temporização de animação cruzando 4 arquivos por regex. É 4x maior que o segundo guard e qualquer refactor do drawer exige editar o teste em paralelo, o que o transforma em espelho do código em vez de contrato.
- [Baixa] Queries de IA e de nêmesis sem teste
  - Arquivos: src-tauri/src/db/queries/ai_post_race.rs, ai_pre_race.rs, ai_story.rs, ai_world_notes.rs, player_nemesis.rs, rivalry_episodes.rs, special_team_entries.rs, db/connection.rs
  - Motivo: db/queries tem teste inline na maioria dos arquivos e pastas tests/ para calendar, contracts, race_history e teams; esses sete ficaram de fora do padrão do próprio diretório.
- [Baixa] Dois guards quase homônimos para comandos de carreira
  - Arquivos: scripts/tests/career-command-structure.test.mjs:26, scripts/tests/career-commands-structure.test.mjs:15
  - Motivo: career-command-structure confere o módulo de tipos e career-commands-structure confere a casca de comandos; o nome com um s de diferença convida a editar o arquivo errado. Fundir os dois ou renomear para o que cada um vigia.
- [Baixa] Teste de componente com 4.482 linhas
  - Arquivos: src/components/driver/v2/DriverDetailModalV2.test.jsx
  - Motivo: Um único arquivo carrega 16% da suíte de UI. Fatiar por seção do modal seguiria o precedente já aberto por DriverDetailModalSections.test.jsx no v1 e reduziria o custo de rodar um caso só.
- [Baixa] Locale global exige serial e nada garante a disciplina
  - Arquivos: src-tauri/src/engenheiro/tests/, src-tauri/src/commands/ai_news/tests/mod.rs e ~40 arquivos com serial_test
  - Motivo: O rust-i18n é global do processo; teste novo que troca idioma sem #[serial] contamina asserções de prosa em PT de forma intermitente. A regra vive só no CLAUDE.md; um guard estrutural que exija serial em teste que chama set_locale fecharia a porta.


## Trabalho não commitado (working tree de C:\dev\Loop)

O working tree carrega 103 arquivos modificados (+6.176/-1.067 linhas) e cerca de 25 não rastreados, distribuídos em pelo menos oito frentes independentes: telemetria de produto com fila de reenvio em disco e medição de tempo de tela, novo ciclo de vida de voltas no race_monitor (corrige o atraso do LapLastLapTime), sistema de castigo por quali destruída com falas novas do engenheiro, banda de skill do roster com mapeamento identidade, torre de classificação no overlay, exportação para o iRacing sem prompts (pintura e modo janela automáticos), poda do season preview com scrub de meta-linguagem, e draft histórico com identidade editável. Cada frente tem doc atualizada junto (telemetry-endpoint.md, season-preview-*.md, engenheiro-catalogo.*), o que indica trabalho deliberado. São claramente vários commits temáticos esperando para nascer, no padrão dos commits recentes do repositório.

Componentes:

- src-tauri/src/telemetry.rs (~1071 linhas): Telemetria de produto reescrita: fila de reenvio em disco (JSONL), retries com esperas [3,12]s, timeout 20s, session_id por abertura, eventos app_start e race_simulated, acumulador de uso (tempo de tela, PTT, virada de rodada). Cresceu +809 linhas motivado por medição de 08/08/2026: 5 corridas produziram 1 race_start e 0 race_end no servidor (Cloud Run frio com timeout de 5s).
- src-tauri/src/commands/telemetria.rs (~23 linhas): Comando novo (não rastreado): ponte do front para uso_tela, aceita só as três telas que custam geração de IA (noticias, briefing, debriefing).
- src/hooks/useTempoDeTela.js (~69 linhas): Hook novo: cronômetro de permanência nas telas pagas, reporta no unmount e a cada minuto para não perder a leitura de quem fecha o app na tela.
- src-tauri/src/iracing_sdk/race_monitor/voltas.rs (~231 linhas): Módulo novo: ciclo de vida de uma volta. Só aceita tempo quando o SDK publica valor NOVO após a virada (o LapLastLapTime chega 0,067 a 0,433s atrasado); fecha por cronômetro próprio (erro medido de 0,010s) quando o oficial não vem. Tem tests.rs próprio e +1.028 linhas em race_monitor/tests/mod.rs.
- src-tauri/src/iracing_sdk/race_monitor/observacao.rs (~927 linhas): Ganhou +410 linhas: máquina de castigo por quali destruída (tiers de estrago por severidade e segundos de reparo, punição ao vivo, despacho latched do EOL/DQ que espera o YAML popular o número do carro). Regra armada por variável de ambiente.
- src-tauri/src/car/breakdown.rs (~700 linhas): reroll_luck(salt) para o reinício de sessão não repetir a mesma quebra, e advance_lap_at_cfg(allow_break) que desliga o sorteio de falha na quali e na carência de largada mantendo o desgaste.
- src-tauri/src/iracing_sdk/roster_gen.rs (~937 linhas): normalize_to_roster + SkillBand: a temporada passa a escrever minSkill/maxSkill dos próprios alvos, tornando o esticão do iRacing a identidade. Corrige o apagamento dos nudges (pressão, forma, chuva) nos extremos do grid e o empate no teto de 100.
- src-tauri/src/narrative/client.rs (~625 linhas): Season preview encolhe de 700-900 para 450-600 palavras (playtest: ninguém terminava de ler) e ganha remover_vazamento_meta, scrub de 9 frases de quarta parede que saves antigos ainda enviam persistidas.
- src/components/race/nextrace/useIracingExport.js (~191 linhas): Exportação sem prompts: pintura aplicada automaticamente com aviso único por save (PAINT_NOTICE_KEY), modo janela sincronizado no boot pelo lib.rs. Os modais NextRacePaintPrompt.jsx e NextRaceWindowModePrompt.jsx foram deletados.
- src/overlay/towerCanvas.js (~1297 linhas): Torre do overlay com modo classificação: tempo restante da sessão e intervalo para a melhor volta da classe, alimentado por overlay/torre.rs e formato.rs no Rust (+295 linhas somadas).
- src-tauri/src/commands/historical_draft.rs (~600 linhas): update_career_draft_identity (trocar nome/nacionalidade/idade sem descartar 26 temporadas simuladas) e purge_existing_drafts (contrato de um draft por vez, limpa drafts failed órfãos). NewCareer.jsx reduz WORLD_SHAPING_FIELDS a só dificuldade.
- src/lib/spotterVoice.js (~300 linhas): Corrige corrida de prioridade no await da decodificação: fala de prioridade menor no mesmo lote de poll deixava de atropelar a maior; agora há registro do que está decodificando com seq e prioridade.

Testes: Cobertura acompanhou o grosso do trabalho: race_monitor/tests/mod.rs ganhou 1.028 linhas e voltas/ tem tests.rs próprio construído sobre captura real de 53.551 quadros; telemetry.rs tem mod tests para fila e acumulado (ida e volta em disco, linha corrompida, poda por idade); overlay/tests +177, towerCanvas.test.js +61, weather/tests +117, result_bridge/tests +141, historical_draft/tests +131; e nasceram spotterVoice.test.js, microfone.test.js, useTempoDeTela.test.js, RaceTelemetryCockpit.test.jsx e nextRaceContext.i18n.test.js. Buracos: useIracingExport.js e EngenheiroPttAuto.jsx mudaram bastante sem teste; em telemetry.rs as funções de rede (entregar, postar, drenar_fila com fracassos seguidos) só têm a parte de disco coberta; e os limiares do castigo de quali em observacao.rs dependem dos testes do race_monitor para se sustentar, sem calibração documentada dos números em si.

Pontos de revisão:

- [Alta] Oito frentes num único working tree sem nenhum commit
  - Arquivos: raiz do repositório (103 modificados + 25 não rastreados)
  - Motivo: Telemetria, voltas, quali destruída, roster, torre, exportação, narrativa e draft são mudanças independentes entre si. Um commit único torna bisect e reversão impossíveis, e a memória do projeto registra que a árvore do Loop não é commitada e que git checkout já apagou trabalho real. Fatiar em commits temáticos é a proteção mais barata disponível agora.
- [Alta] Regra da quali destruída armada por variável de ambiente
  - Arquivos: src-tauri/src/iracing_sdk/race_monitor.rs, src-tauri/src/iracing_sdk/race_monitor/observacao.rs
  - Motivo: Feature flag improvisado via env var decide se o castigo EOL/DQ existe. Precisa de decisão de destino antes de release: virar config do jogador, virar padrão, ou sair. Os limiares de severidade e de segundos de reparo que definem os tiers (grave, destruído, DQ) são números novos sem calibração registrada em doc.
- [Alta] telemetry.rs virou monólito de 1.071 linhas com estado global
  - Arquivos: src-tauri/src/telemetry.rs
  - Motivo: Fila em disco, acumulador de uso, entrega com retry e seis estáticos (OnceLock/Mutex/AtomicBool) num arquivo só. As constantes de entrega (timeout 20s, esperas [3,12]s, fila de 200, idade de 7 dias) foram calibradas numa única medição de 08/08; merecem revisita com dados de mais instalações. Candidato natural a módulo com submódulos fila/uso/entrega.
- [Média] Artefatos soltos na raiz do repositório sem destino
  - Arquivos: __curva_preview.html, __fita-equipes-preview.html, __mercado-card-preview.html, fita-mock.html, preview-curva-campeonato.html, .tabela-equipes.txt, testar-quali-destruida.cmd, docs/release-notes-0.14.0.txt
  - Motivo: Previews de UI e script de teste manual acumulando na raiz. Cada um precisa de veredito: .gitignore, mover para scripts/ ou docs/, ou apagar. testar-quali-destruida.cmd em particular documenta como armar a env var da quali e se perderia junto com o conhecimento.
- [Média] Pintura e modo janela aplicados sem perguntar ao jogador
  - Arquivos: src/components/race/nextrace/useIracingExport.js, src-tauri/src/lib.rs, src-tauri/src/iracing_sdk/modo_janela.rs, src-tauri/src/commands/iracing/pintura.rs
  - Motivo: Dois modais de confirmação foram deletados e o comportamento virou automático (modo janela sincronizado no boot, pintura em toda exportação com aviso único via localStorage). Mudança de contrato com o jogador que mexe em arquivo de config do iRacing dele; vale confirmar o caminho de desfazer nas Configurações e o comportamento quando o iRacing está aberto.
- [Média] Fluxo de exportação sem teste após reescrita
  - Arquivos: src/components/race/nextrace/useIracingExport.js, src/overlay/EngenheiroPttAuto.jsx
  - Motivo: useIracingExport teve 161 linhas mexidas com timer de toast fora do dismissTimers (comentário admite que a limpeza errada prenderia o toast na tela) e não tem arquivo de teste. EngenheiroPttAuto mudou 90 linhas na mesma condição.
- [Média] normalize_to_roster com casos degenerados decididos por convenção
  - Arquivos: src-tauri/src/iracing_sdk/roster_gen.rs, src-tauri/src/commands/iracing/temporada.rs
  - Motivo: Grid de um piloto ou empate exato produz banda min+1.0 e skill 50; lista vazia devolve banda 0-1. São escolhas razoáveis que merecem teste explícito no consumidor (temporada.rs escreve o minSkill/maxSkill) para o contrato dos dois lados envelhecer junto.
- [Baixa] Scrub de meta-linguagem em dupla camada tende a crescer
  - Arquivos: src-tauri/src/narrative/client.rs, src-tauri/src/commands/ai_news/fatos.rs
  - Motivo: FRASES_META lista 9 frases ASCII hardcoded porque fatos antigos ficam persistidos no save com a redação velha. Curativo consciente e documentado; a lista só cresce e a ordenação por tamanho é frágil de manter na mão. Uma migração dos fatos persistidos eliminaria a camada.
- [Baixa] Flags de throttling adicionadas nas três webviews
  - Arquivos: src-tauri/tauri.conf.json
  - Motivo: IntensiveWakeUpThrottling desligado e disable-background-timer-throttling nas três janelas, resposta direta ao caso do spotter mudo com janela coberta. Vale medir consumo em segundo plano nas máquinas dos jogadores e registrar a motivação em doc, senão a flag some numa limpeza futura.
- [Baixa] Cinco novos .opus binários entrando no git
  - Arquivos: src/assets/engenheiro/meu_quali_*.opus, docs/engenheiro-catalogo.json
  - Motivo: O acervo do engenheiro já soma 3.943 peças e 324 MB segundo a memória do projeto. Cada leva nova de binários no repositório agrava o clone; vale decidir se assets de voz continuam no git ou migram para artefato de release.


## Dívida já documentada (docs/divida-tecnica.md, docs/backlog.md, docs/roadmap.md, docs/varredura-bugs-2026-07.md, docs/varredura-acoplamento/)

O corpus de dívida do Loop vive em cinco lugares: divida-tecnica.md registra o resolvido, backlog.md lista ids estáveis (F-xx features, D-xx dívida, P-xx pontas soltas), roadmap.md guarda o raciocínio, varredura-bugs-2026-07.md traz 6 achados a confirmar e varredura-acoplamento/ traz 9 briefings (F1-F4 frontend, R1-R5 Rust). Conferi cada item contra o código de hoje (2026-08-10): boa parte foi resolvida depois dos docs serem escritos e nada foi registrado de volta, então os documentos mentem sobre o estado atual. O saldo real aberto: R4 (hierarchy sem consumidor), run_market vivo só pelos testes (resto do R5), bug #4 (is_crash perdeu a severidade sem registro), bug #5 (recalibrações sem nota), parte 2 do bug #6, D-01 a D-08 quase todos, e o backlog derivado do iracing-escopo §6.

Componentes:

- docs/divida-tecnica.md (~34 linhas): Registro oficial de dívida. Hoje só contém DB-001..004 (resolvidos na migration v28) e declara 'nenhuma pendência', o que contradiz o próprio backlog.md com D-01 a D-09 abertos.
- docs/backlog.md (~80 linhas): Lista única com ids estáveis: F-01..F-10 (features), D-01..D-09 (dívida), P-01..P-04 (pontas soltas). Levantada em 2026-07-27. Ordem sugerida: F-06, depois F-03+F-04+F-05, F-02, F-01, F-07.
- docs/roadmap.md (~262 linhas): O raciocínio por trás do backlog: diagnóstico de que o app é organizado por momento da temporada e faltam visões permanentes. Contém a correção do falso achado 'mercado não tem tela' e a decisão de escopo do iRacing (2026-07-27).
- docs/varredura-bugs-2026-07.md (~497 linhas): Seis achados sobre o diff da branch main-menu-redesign, cada um com critério de refutação. Inclui a lista do que a varredura inocentou (parsers YAML, gate in_race_session, towerAnimation, player_repercussion e outros).
- docs/varredura-acoplamento/README.md (~47 linhas): Índice dos 9 briefings da varredura de 2026-07-24, aviso de falsos positivos do método (grep excluindo o próprio módulo) e apêndice da árvore vazia src-tauri/src/src-tauri/.
- docs/varredura-acoplamento/F1..F4: Briefings frontend: helpers de pista/clima duplicados, formatLap e paleta de gráficos, getReadableTeamColor triplicado, IN_TAURI em 11 arquivos. Os quatro foram resolvidos no commit 2c85f44 segundo o backlog.
- docs/varredura-acoplamento/R1..R5: Briefings Rust: narrative cego (Etapa B), três motores de tese, tiers duplicados, hierarchy sem consumidor, caminhos vivos só pelos testes. R1 e R2 tocam os mesmos arquivos e o doc proíbe rodar em paralelo.

Testes: Os docs registram o estado das suítes na época da varredura: vitest 465/465, estrutural 28/28, cargo 1886/1887 (a falha era o bug #1, hoje corrigido no teste). Lacunas de teste apontadas pelos próprios docs: o bug #5 pede um teste de cenário de referência para a calibração de repercussão que aparentemente nunca foi criado; o R3 pedia um teste espelho de limiares que ficou dispensável com a remoção do enum; o R5 denunciava falsa cobertura (testes de integração exercitando caminhos que a produção nunca percorre), saneada em três dos quatro casos, com run_market ainda coberto só pelos próprios testes de pipeline. A área de docs em si não tem guard automatizado de consistência com o código, e é exatamente aí que ela envelheceu: cinco fechamentos reais sem registro.

Pontos de revisão:

- [Alta] Documentação de dívida defasada contra o código
  - Arquivos: docs/backlog.md, docs/divida-tecnica.md, docs/roadmap.md, docs/varredura-bugs-2026-07.md
  - Motivo: Conferi hoje: F-06 está feito (src/components/ui/BackupsModal.jsx consome create_season_backup, list_backups e restore_backup), R3 está fechado, três dos quatro casos do R5 sumiram, e os bugs #1, #2 e #3 da varredura foram corrigidos. Nenhum desses fechamentos foi registrado nos docs. divida-tecnica.md diz 'nenhuma pendência' enquanto o backlog lista D-01 a D-09. Quem for separar achado novo de registrado vai trabalhar sobre um mapa errado.
- [Alta] R4: hierarchy com estado rico e sem consequência (aberto)
  - Arquivos: src-tauri/src/hierarchy/orders.rs, src-tauri/src/commands/race/persistencia.rs:222, src-tauri/src/commands/career/market_window.rs:362
  - Motivo: Verificado hoje: seguem só os dois consumidores mapeados no briefing (um de lógica, um de sanidade). Tensão, duelos e status não realimentam mercado, narrativa nem motivação. O roadmap o coloca como bloqueador antes de encostar em hierarquia. Único briefing R sem nenhum avanço aparente.
- [Alta] Backlog derivado do escopo iRacing: 16 comandos inalcançáveis (aberto)
  - Arquivos: docs/iracing-escopo.md (§6), docs/roadmap.md:209-214
  - Motivo: O roadmap registra 9 comandos sem consumidor e 7 presos em RosterGenPanel e PostRacePanel (1.420 linhas nunca importadas). O item mais caro: iracing_process_race_result é a dificuldade adaptativa implementada e nunca executada. O roadmap marca como primeiro item pós decisão de escopo e nada indica execução.
- [Média] Bug #4: is_crash perdeu a severidade sem registro de decisão (aberto, sem dono)
  - Arquivos: src-tauri/src/race_signals.rs:149, src-tauri/src/narrative/beats.rs
  - Motivo: Confirmado hoje: DriverError vira DnfKind::Erro sem olhar severity. A regra antiga excluía DriverError+Minor de 'batida' e o caso de teste erro_leve foi apagado. O doc pedia confirmar se a combinação Minor+DNF é possível no motor de incidentes; essa investigação continua sem resposta registrada.
- [Média] Bug #5: duas recalibrações vigentes sem nota de calibração (aberto, sem dono)
  - Arquivos: src-tauri/src/event_interest/calculator.rs:235, src-tauri/src/race_signals.rs:24
  - Motivo: Confirmado hoje: positional_bonus contínuo (0.4 com clamp -3..4, remontada de 5 posições vale metade do que valia) e REMONTADA_MIN=4 (antes 5 no debrief). O doc aceita que pode ser intencional e exige um registro de calibração para fechar; esse registro não existe.
- [Média] R5 restante: run_market vivo só pelos próprios testes (aberto)
  - Arquivos: src-tauri/src/market/pipeline.rs:86, src-tauri/src/market/pipeline/tests/mod.rs
  - Motivo: Verificado hoje: run_full_race, simulate_race legado e process_segment_incidents legado sumiram do crate (três dos quatro casos resolvidos). run_market permanece com 8 chamadas, todas em tests/mod.rs. O briefing já avisava que pode ser o motor interno de initialize_preseason e pede confirmação antes de tratar como legado.
- [Média] R2: unificação de sinais iniciada e briefing nunca fechado (parcial)
  - Arquivos: docs/varredura-acoplamento/R2-tres-teses.md, src-tauri/src/race_signals.rs, src-tauri/src/commands/ai_news/tese.rs, src-tauri/src/narrative/tese.rs
  - Motivo: race_signals.rs nasceu depois do briefing e já unifica remontada e tipo de DNF (os bugs #4 e #5 são efeitos colaterais dessa unificação). A análise pedida (tabela de sinais, cenários de incoerência, veredito de fazer ou não) nunca foi devolvida, então a unificação avança sem o crivo que o próprio doc exigia.
- [Média] D-01: código legado de convocação (aberto)
  - Arquivos: src-tauri/src/convocation/
  - Motivo: O diretório existe com eligibility.rs, mod.rs e pipeline. O backlog pede confirmar que nenhum save ativo usa as fases BlocoRegular/JanelaConvocacao/BlocoEspecial/PosEspecial e remover.
- [Média] D-02: tabela races coexistindo com calendar (aberto)
  - Arquivos: src-tauri/src/db/migrations/baseline.rs:261, src-tauri/src/db/queries/races.rs:361
  - Motivo: Verificado hoje: CREATE TABLE races no baseline e INSERT ativo em queries/races.rs. Duas fontes de verdade para o conceito de corrida, como o backlog descreve.
- [Média] D-05: advance_transfer_window segue sem consumidor (aberto)
  - Arquivos: src-tauri/src/lib.rs, src/stores/career/marketSlice.js
  - Motivo: Verificado hoje: nenhuma chamada no frontend. É a peça central do F-01 (mercado fora da janela). Os demais órfãos citados (get_driver, get_race_results_by_category, create_career) ficaram sem verificação nesta vistoria por timeout de busca e merecem o mesmo grep.
- [Média] F-01, F-02, F-07: buracos de produto registrados e abertos
  - Arquivos: docs/backlog.md, docs/roadmap.md §3, §4, §5
  - Motivo: Mercado fora da janela (sem painel de contrato e vagas no meio do ano), ficha do próprio piloto (o jogador se vê pela lente do DriverDetailModal de qualquer piloto) e UI de espectadores (§17.1 do DESIGN). Backend pronto nos três, trabalho de frontend. F-03, F-04 e F-05 (aba de História, troféus, rivalidades) seguem abertos e o roadmap manda tratá-los juntos numa tela só.
- [Baixa] Bug #6 parte 2: estimativa de voltas usa a melhor volta do campo (aberto)
  - Arquivos: src-tauri/src/commands/overlay/torre.rs:711-725
  - Motivo: A parte 1 caiu: SessionLapsRemainEx vem sentinelado (32767) em prova por tempo, medição já registrada na memória do projeto, então o gate !timed está certo. A parte 2 segue: dividir tempo restante pela melhor volta absoluta subestima sistematicamente o total, e o doc sugere mediana das últimas voltas do líder.
- [Baixa] R1: Etapa B aparentemente ligada, resta o allow(dead_code) (parcial)
  - Arquivos: src-tauri/src/narrative/mod.rs:1, src-tauri/src/narrative/beats.rs, src-tauri/src/commands/race/noticias/persistencia.rs:46
  - Motivo: O texto 'Etapa A (MVP)' e a menção à Etapa B sumiram do módulo, e persistencia.rs agora monta injury_facts, context_facts e career_beats (beats de carreira existem em beats.rs e contexto.rs). O #![allow(dead_code)] segue na linha 1, exatamente o item que o briefing mandava reavaliar após a ligação.
- [Baixa] D-03 e D-04: stores stub e useTauri.js morto (abertos)
  - Arquivos: src/stores/useUIStore.js, src/stores/useNotificationStore.js, src/hooks/useTauri.js
  - Motivo: Verificado hoje: os três arquivos existem e mantêm o TODO. Decisão pendente de implementar ou apagar, custo baixo.
- [Baixa] D-06, D-07 e D-08: TODOs de design e migração intactos (abertos)
  - Arquivos: src-tauri/src/constants/tracks/consultas.rs:30, src-tauri/src/calendar/generator.rs, src-tauri/src/models/driver.rs:22
  - Motivo: Verificado hoje: TODO(design final) das pistas possuídas, 2 TODOs de venues ausentes do banco no generator, e TODO(migration) afirmando cobertura do Módulo 10 sem verificação registrada.
- [Baixa] F-08 sem dono e P-04 aguardando os F-xx
  - Arquivos: docs/backlog.md, src/i18n/locales/pt-BR/common.json, src/i18n/locales/en-US/common.json
  - Motivo: F-08 (outras categorias) está fora da fila até alguém responder o que mostraria além das abas globais, pergunta de design sem dono. P-04 deixou chaves i18n órfãs (marketTab.*, driversTab.*) de propósito; se algum F-xx for cancelado, a limpeza fica sem gatilho.
- [Baixa] Árvore de diretórios vazia do apêndice segue no crate (aberto)
  - Arquivos: src-tauri/src/src-tauri/src/evolution/pipeline/tests/
  - Motivo: Verificado hoje: a árvore existe. O README da varredura autoriza remoção direta, sem briefing. Custo de um rmdir.
- [Baixa] Itens fechados que merecem só o registro de fechamento
  - Arquivos: src-tauri/src/commands/race/manutencao.rs:89, src/stores/career/raceSlice.js:128, src-tauri/src/commands/career/tests/mod.rs:3778, src-tauri/src/public_presence/team.rs, docs/varredura-acoplamento/F1..F4
  - Motivo: Confirmados resolvidos no código: bug #2 (fatura agora ancora em season_number e round desta rodada), bug #3 (dismissResult zera lastRaceMaintenance), bug #1 (teste de vacancies reescrito), R3 (enum de tier removido de public_presence, que devolve f64 contínuo com comentário novo explicando a decisão, e o comentário obsoleto de visibility.rs sumiu), F1 a F4 (commit 2c85f44), DB-001..004 (migration v28), P-01 e P-03 (commit 2c85f44 e limpeza de placeholders), F-10 (decisão de escopo 2026-07-27). Falta unicamente atualizar os docs.


## Sugestão de ordem de ataque

1. Fatiar o trabalho não commitado em commits temáticos. São 8 frentes independentes em 103 arquivos modificados; enquanto não vira commit, qualquer acidente de checkout apaga trabalho real. É a proteção mais barata da lista inteira.
2. Verificar os bugs prováveis achados na vistoria: LIT_TRACK_ID 556 inexistente no catálogo, janelas de calendário divergentes entre gerador completo e parcial, gate de enduro furado nos dois call sites do iracing/, auto-import engolindo erro real como "ainda não pronto", custid e flag de amarela em %TEMP%, seed do mercado repetida a cada semana, e a regalia de pré-temporada creditando pontos sem debitar caixa enquanto o ralo novo segue sem consumidor.
3. Fechar os contratos da ponte Rust/React: enums serde no lugar das strings mágicas (status do race_monitor, ai_news, severidade), struct nomeada no lugar da tupla de 11 campos, validação da oferta de poaching contra o plano persistido, e dois guards estruturais novos (nomes de invoke contra o generate_handler, VR_W/VR_H contra shared_frame.h).
4. Cobrir os caminhos críticos sem teste: commands/iracing/ inteiro, get_overlay_data, slices do career store, race_control.rs, useIracingExport, standings/ e calendar/ do frontend.
5. Rodar a calibração pendente pelo harness: constantes de tráfego e ultrapassagem, pressão, deltas do eixo N1/N2, sobrecusto de enduro, limiares de previsão de quebra e do estilo de pilotagem.
6. Tomar as decisões de corte: data de morte das árvores v1 do frontend, remoção dos caminhos legados do motor e da quali, destino das env flags de A/B, gate para os comandos de debug no build de produção, D-01 (convocação legada).
7. Atualizar os docs de dívida: cinco fechamentos reais sem registro, e divida-tecnica.md dizendo "nenhuma pendência" com D-01 a D-08 abertos no backlog.
8. Performance de fundo: índice de race_results por equipe, cache por sessão na torre do overlay, índice por id no laço de segmentos do motor.
