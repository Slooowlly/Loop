# Decisões finais de balanceamento do sistema de quebra

**Data:** 2026-07-21  
**Status:** aprovado pelo usuário  
**Escopo:** clima, proteção do jogador, manutenção parcial em enduros e escala de DNF no sprint

## 1. Contexto

O primeiro pacote de balanceamento já transformou a antiga curva de falha em uma rampa e reduziu o esvaziamento do grid. O estado atual abre a janela de risco em `0.87`, coloca a parede em `1.13`, limita o multiplicador combinado de pista e clima a `1.5`, impede que a parede promova automaticamente uma falha grave a DNF e reduz a parcela-base de DNF das peças estruturais.

Três decisões de produto permaneceram abertas, e a validação posterior revelou um quarto ajuste necessário:

1. quão relevante deve ser o clima para um carro saudável;
2. quanta proteção indireta o jogador deve receber em uma equipe fraca;
3. como uma parada real deve aliviar o risco mecânico de um enduro.
4. como reduzir o DNF do perfil pobre neutro sem apagar as quebras graves.

Este documento fecha as quatro decisões. Ele não redesenha a separação entre desgaste econômico e risco ao vivo.

## 2. Decisões aprovadas

### 2.1 Clima: risco sério, mas sobrevivível

Condições brutais devem ameaçar também um carro saudável, sem voltar a transformar a corrida em uma loteria de abandono coletivo.

O valor final de `CONDITIONS_MAX_MULT` será escolhido pela matriz reproduzível da seção 7. A implementação deve testar `1.60`, `1.65` e `1.70` nessa ordem. Se nenhum candidato passar, deve testar `1.75` e `1.80`; `1.80` é o limite deste design. O valor escolhido será o primeiro que satisfaça simultaneamente todos os critérios climáticos, de DNF e de desgaste econômico. Se nenhum candidato passar, a implementação deve parar e solicitar nova decisão de produto, sem ampliar o intervalo ou alterar outros dials.

`conditions_mult()` continua sendo a única fonte do teto. O mesmo multiplicador alimenta o risco ao vivo e o desgaste aplicado pela economia.

### 2.2 Proteção do jogador: máximo de 6%

`PLAYER_MAX_RELIEF` passa de `0.05` para `0.06`.

A proteção continua aplicada ao desgaste de entrada, antes do sorteio da corrida, e escala com a fraqueza da equipe:

```text
fraqueza = 1 - clamp(pit_crew_quality, 0, 100) / 100
alivio = 0.06 * fraqueza
desgaste_protegido = desgaste * (1 - alivio)
```

Equipes fortes recebem alívio próximo de zero. A IA nunca recebe essa proteção.

### 2.3 Enduro: manutenção parcial em parada genuína

Uma parada real de enduro deve remover parte do desgaste adquirido durante a corrida, mas nunca apagar o desgaste com que a peça largou.

Para o serviço mecânico, uma parada é genuína quando o carro permanece parado na própria caixa por pelo menos 4 segundos. Passagem pelo pit lane, drive-through e oscilações breves de telemetria não contam. Esse limiar é específico dos consumidores de manutenção mecânica e economia; ele não substitui o limiar global de 2,5 segundos usado para registrar paradas e inferir estratégia de pneus.

O primeiro valor de calibração será `PIT_SERVICE_RELIEF = 0.60`. Somente peças com desgaste atual igual ou superior a `SERVICE_WEAR_FLOOR = 0.60` recebem manutenção.

Para cada peça elegível:

```text
ganho_na_corrida = max(0, desgaste_atual - desgaste_de_largada)
desgaste_novo = desgaste_atual - ganho_na_corrida * PIT_SERVICE_RELIEF
desgaste_novo = max(desgaste_novo, desgaste_de_largada)
```

Exemplos:

- uma peça larga com `0.40` e chega ao pit com `0.90`: sai com `0.60`;
- uma peça larga com `0.80` e chega ao pit com `1.00`: sai com `0.88`;
- uma peça larga com `0.80` e chega ao pit com `0.85`: permanece acima de `0.80`;
- uma peça abaixo do piso de serviço não muda.

Paradas posteriores reaplicam a mesma fórmula sobre o desgaste atual, sempre respeitando o desgaste de largada como piso absoluto.

### 2.4 Sprint: DNF retido em escala global

O perfil `pobre_esticando` produz atualmente 38,2% de DNF no sprint neutro. O alvo aprovado é 20–25%, sem reduzir a frequência de quebras graves.

Um novo `SPRINT_DNF_SCALE` atua como filtro final de severidade somente em sprint. Quando o sorteio-base produz `Dnf`, uma rolagem determinística e decorrelacionada mantém o DNF com essa probabilidade; caso contrário, o resultado vira `Heavy`. O filtro vale para todas as peças e igualmente para jogador e IA. Ele não altera `Light`, `Heavy`, hazard, parede ou pesos por peça.

O primeiro candidato é `0.60`. A matriz da seção 7 define a busca e o critério de parada.

## 3. Abordagem escolhida

Foi escolhida a abordagem de dials calibrados e manutenção parcial. Ela preserva o modelo atual e mantém cada decisão observável em uma constante ou fórmula curta.

Foram descartadas neste ciclo:

- regras adaptativas que alterariam clima ou manutenção conforme a saúde do carro, por serem mais opacas e difíceis de calibrar;
- a separação estrutural entre desgaste econômico, estresse climático e fadiga de enduro, por ampliar o escopo e exigir nova calibração do sistema inteiro;
- a troca completa de peças no pit, porque transformaria uma parada comum em renovação gratuita de componentes antigos;
- retirar todo efeito mecânico do pit, porque eliminaria uma escolha estratégica valiosa dos enduros.

## 4. Componentes e responsabilidades

### `conditions_mult()`

Mantém o teto climático em uma fonte única. A calibração altera somente `CONDITIONS_MAX_MULT`.

### `player_protected_car()`

Mantém a fórmula atual e passa a usar `PLAYER_MAX_RELIEF = 0.06`.

### `sample_severity()`

Aplica `SPRINT_DNF_SCALE` como filtro final quando `is_enduro = false`. A rolagem de retenção deve ser determinística e decorrelacionada da rolagem que escolheu a severidade-base. O caminho de enduro continua usando `ENDURO_DNF_SCALE` e não recebe o filtro de sprint.

### `LiveBreakdown`

Continua sendo o dono do desgaste vivo por peça. O estado já guarda o desgaste de entrada em `entered`; a manutenção parcial deve usar esse valor como piso.

`service_pit()` deixa de zerar peças gastas e passa a aplicar a fórmula da seção 2.3. A própria operação verifica `is_enduro` e vira no-op em sprint, independentemente do chamador. Ela também não deve fazer nada em peças já quebradas nem ressuscitar um carro em DNF.

### `BreakdownDirector`

Expõe `service_car(car_number)` para localizar o `LiveBreakdown` correto e solicitar a manutenção. Essa operação não é idempotente e não deduplica paradas; o monitor só pode chamá-la uma vez para cada transição confirmada de saída da caixa.

A lista pré-programada `service_laps` pode continuar atendendo simulações e previsões determinísticas, mas o fluxo ao vivo não dependerá dela para descobrir paradas reais.

### Monitor do iRacing

O detector existente de permanência na caixa é a autoridade sobre paradas. A transição `InPitStall → fora da caixa` fecha exatamente um evento e fornece seu `stationary_secs`. O histórico continua aceitando eventos a partir de 2,5 segundos para a estratégia de pneus. Quando o mesmo evento tem pelo menos 4 segundos, o monitor chama `service_car` exatamente uma vez, antes de processar a próxima volta. O monitor não precisa conhecer o gate de enduro: `LiveBreakdown::service_pit()` recebe a chamada e garante o no-op em sprint. O evento fechado — e não uma leitura posterior do histórico — é a fronteira de deduplicação.

Uma constante de domínio para os 4 segundos deve ser compartilhada pelo serviço mecânico ao vivo e pelo cálculo econômico pós-corrida. `MIN_PIT_STALL_DWELL_SECS = 2.5` permanece próprio do detector e da estratégia de pneus.

## 5. Fluxo de dados

1. O carro larga com desgaste persistente carregado do banco.
2. Somente o carro do jogador recebe o alívio de entrada de até 6%.
3. A cada volta, `LiveBreakdown` acumula desgaste usando o multiplicador combinado de pista e clima, já limitado pela constante calibrada.
4. O detector de pit cronometra a permanência na caixa de cada carro em qualquer corrida.
5. Na saída da caixa, o monitor registra a parada para pneus se ela durou pelo menos 2,5 segundos e solicita manutenção parcial se ela durou pelo menos 4 segundos; em sprint, o estado vivo ignora a solicitação.
6. O diretor atualiza o estado vivo, sem alterar o carro persistido.
7. A volta seguinte parte do desgaste parcialmente aliviado.
8. Após a corrida, a economia calcula e persiste o desgaste pela regra econômica existente; o pit continua reduzindo somente o sobrecusto de enduro nessa camada.

## 6. Previsão pré-corrida

O benefício de uma futura parada não é garantido no momento da previsão. Por isso, a previsão de enduro calcula o risco da distância planejada sem aplicar manutenção futura.

`RaceBreakdownCtx` deve carregar `laps`, obtido diretamente de `calendar.voltas` da próxima etapa. Tanto a previsão do jogador quanto a previsão do grid deixam de usar as 18 voltas fixas e passam `ctx.laps` ao Monte Carlo. `is_enduro` continua controlando a rampa e a severidade. Com a distância real e `service_laps = []`, o resultado é explicitamente o cenário sem parada preventiva.

`BreakdownForecastView` ganha `preventive_service_available: bool` e `forecast_laps: u32`. O primeiro vale `true` somente em enduro disponível; o segundo informa a distância simulada. No estado indisponível, `preventive_service_available` é `false`, `forecast_laps` é `0` e o card continua oculto.

Quando `preventive_service_available` for verdadeiro, o detalhe expandido de `BreakdownRiskButton` mostra, abaixo da lista de peças, uma nota traduzida: “Previsão sem manutenção futura; uma parada preventiva pode reduzir o risco.” Devem ser adicionadas chaves equivalentes em `pt-BR` e `en-US`. O texto não promete uma volta nem uma redução exata.

## 7. Critérios de aceitação

### Matriz fixa de calibração

Todas as porcentagens são estimadas com `N = 20_000`, semente-base `0x00C0_FFEE` e a mesma derivação determinística de sementes já usada por `analise_taxa_quebra`. Limites são inclusivos, exceto onde a tabela usa `<`.

O sprint de calibração usa 18 voltas, `is_enduro = false`, pit crew 50, nenhuma parada e os cenários já definidos pelo harness:

- neutro: `TRACK_NEUTRO` + `WEATHER_NEUTRO`;
- brutal: `TRACK_POWER` + `WEATHER_BRUTAL`.

Os perfis são fórmulas, não dados externos. Para cada peça, `limiar(pt) = 1 - wear_per_race(pt)`:

- `rico_saudavel`: todas as peças entram com `limiar(pt) / 2`;
- `rico_limitrofe`: todas entram com `max(limiar(pt) - 0.02, 0)`;
- `pobre_esticando`: peças com durabilidade menor ou igual a 3 entram com `limiar(pt)`; as demais, com `limiar(pt) / 2`;
- `pobre_degradado`: peças com durabilidade menor ou igual a 3 entram com `0.98`; as demais, com `limiar(pt) - 0.02`.

“Grid brutal” é a média aritmética das probabilidades de DNF desses quatro perfis, com peso de 25% para cada um. Essa composição sintética existe apenas como benchmark reproduzível; ela não pretende reproduzir a distribuição de um save.

“Quebra relevante” significa pelo menos um evento `Heavy` ou `Dnf` na corrida. “Próxima de 0%” significa menor ou igual a 0,5%.

### Ordem de calibração

1. Fixar `CONDITIONS_MAX_MULT = 1.5` e escolher `SPRINT_DNF_SCALE`.
2. Fixar a escala de DNF escolhida e selecionar `CONDITIONS_MAX_MULT`.
3. Fixar ambos e validar `PLAYER_MAX_RELIEF = 0.06`.
4. Calibrar `PIT_SERVICE_RELIEF` somente na matriz de enduro.

Para `SPRINT_DNF_SCALE`, testar `0.50`, `0.55`, `0.60`, `0.65` e `0.70`. São válidos os candidatos que coloquem o DNF de `pobre_esticando` neutro entre 20% e 25% e mantenham sua quebra relevante a no máximo 0,5 ponto percentual do baseline sem filtro. Escolher o candidato válido mais próximo de `0.60`; em empate, escolher o menor. Se nenhum passar, interromper a calibração e solicitar nova decisão de produto — não ampliar o intervalo nem alterar hazard ou pesos por peça automaticamente.

### Clima e grid

| Cenário | Alvo |
|---|---:|
| `rico_saudavel`, clima neutro | quebra relevante ≤ 0,5% |
| `rico_saudavel`, clima brutal | quebra relevante entre 5% e 10% |
| `rico_saudavel`, clima brutal | DNF ≤ 2% |
| `pobre_esticando`, clima neutro | DNF entre 20% e 25% |
| Grid em clima brutal | DNF total abaixo de 30% |

O candidato também precisa preservar a economia: em clima neutro, os 11 multiplicadores de `conditions_wear_mults` devem permanecer iguais aos do teto `1.5` dentro de `1e-9`; no cenário brutal, a média aritmética dos 11 multiplicadores não pode crescer mais de 8% em relação ao teto `1.5`; e nenhum multiplicador individual pode exceder o candidato.

### Proteção do jogador

| Cenário | Alvo |
|---|---:|
| `pobre_esticando`, pit crew 0 | quebra relevante do carro protegido 20–25% menor que a do mesmo carro sem proteção |
| Qualquer perfil, pit crew 100 | diferença absoluta ≤ 0,1 ponto percentual para o mesmo carro sem proteção |

### Enduro

| Cenário | Alvo |
|---|---:|
| Uma parada preventiva genuína | quebra relevante 20–60% menor que no mesmo carro sem parada |
| Peça que largou gasta | nunca fica abaixo do desgaste de largada |
| Sprint | parada não produz manutenção preventiva |
| Drive-through ou parada menor que 4 s | nenhum benefício |

O benchmark de enduro usa 40 voltas, `is_enduro = true`, pit crew 50, cenário neutro e os perfis `rico_saudavel` e `pobre_esticando`. Compara `service_laps = []` a `service_laps = [20]`, com as mesmas 20.000 sementes em ambos os lados.

Para `PIT_SERVICE_RELIEF`, testar de `0.00` a `1.00` em passos de `0.05`. São válidos os candidatos que coloquem os dois perfis na faixa de redução de 20–60%. Escolher o válido mais próximo de `0.60`; em empate, escolher o menor. Se nenhum candidato for válido para ambos os perfis, interromper a calibração e solicitar nova decisão de produto. A implementação não amplia o intervalo nem muda outros dials para forçar o resultado.

## 8. Tratamento de bordas

- Uma oscilação transitória de `InPitStall` não pode contar como parada.
- Uma mesma transição confirmada de saída da caixa aplica manutenção uma única vez; o monitor é responsável por essa garantia.
- Carro sem mapeamento válido no diretor é ignorado com segurança.
- Parada após DNF não altera o estado.
- Peça já marcada como quebrada não recebe manutenção.
- `LiveBreakdown::service_pit()` garante internamente que o serviço só é habilitado quando a corrida está classificada como enduro.
- A manutenção viva não grava diretamente no banco e não reduz o custo persistente já definido pela economia.

## 9. Estratégia de testes

### Testes determinísticos

- fórmula parcial nos dois exemplos numéricos aprovados;
- piso absoluto no desgaste de largada;
- peça abaixo de `SERVICE_WEAR_FLOOR` inalterada;
- múltiplas paradas sem atravessar o piso;
- peça quebrada e carro em DNF inalterados;
- sprint sem manutenção preventiva;
- passagem pelo pit, parada abaixo de 2,5 segundos e parada entre 2,5 e 4 segundos sem benefício mecânico;
- parada entre 2,5 e 4 segundos ainda disponível para a estratégia de pneus;
- uma aplicação por transição genuína de saída da caixa;
- `PLAYER_MAX_RELIEF = 0.06` escalando de equipe fraca a forte;
- `SPRINT_DNF_SCALE` preservando e rebaixando DNF de forma determinística;
- filtro de sprint sem efeito sobre `Light`, `Heavy` ou o caminho de enduro;
- mesma fonte de teto climático no risco e na economia.

### Harness Monte Carlo

- varrer primeiro os candidatos de `SPRINT_DNF_SCALE` e depois os de teto climático;
- registrar quebra qualquer, quebra relevante e DNF;
- comparar perfis saudável, limítrofe, pobre esticando e pobre degradado;
- comparar clima neutro e brutal;
- comparar enduro sem parada e com uma parada preventiva;
- medir a variação média dos 11 multiplicadores econômicos contra o teto `1.5`;
- confirmar todos os critérios da seção 7;
- conservar a semente fixa e o determinismo do relatório;
- salvar no documento de implementação os candidatos testados, os valores escolhidos e a saída completa da matriz como evidência.

### Regressão

Toda a suíte do módulo de quebra deve permanecer verde. As demais suítes diretamente afetadas pelo monitor e pela manutenção econômica também devem ser executadas.

## 10. Fora de escopo

- separar clima de desgaste e aplicá-lo diretamente ao hazard;
- recalibrar toda a economia de peças;
- criar estratégia automática nova de pit para a IA;
- persistir manutenção preventiva como rejuvenescimento da peça;
- mudar `RISK_OPEN`, `HARD_WALL`, pesos de severidade, hazard ou `ENDURO_DNF_SCALE`.
