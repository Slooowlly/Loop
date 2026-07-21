# Decisões finais de balanceamento do sistema de quebra

**Data:** 2026-07-21  
**Status:** aprovado pelo usuário  
**Escopo:** clima, proteção do jogador e manutenção parcial em enduros

## 1. Contexto

O primeiro pacote de balanceamento já transformou a antiga curva de falha em uma rampa e reduziu o esvaziamento do grid. O estado atual abre a janela de risco em `0.87`, coloca a parede em `1.13`, limita o multiplicador combinado de pista e clima a `1.5`, impede que a parede promova automaticamente uma falha grave a DNF e reduz a parcela-base de DNF das peças estruturais.

Três decisões de produto permaneceram abertas:

1. quão relevante deve ser o clima para um carro saudável;
2. quanta proteção indireta o jogador deve receber em uma equipe fraca;
3. como uma parada real deve aliviar o risco mecânico de um enduro.

Este documento fecha as três decisões. Ele não redesenha a separação entre desgaste econômico e risco ao vivo.

## 2. Decisões aprovadas

### 2.1 Clima: risco sério, mas sobrevivível

Condições brutais devem ameaçar também um carro saudável, sem voltar a transformar a corrida em uma loteria de abandono coletivo.

O valor final de `CONDITIONS_MAX_MULT` será escolhido por medição. A implementação deve testar pelo menos `1.60`, `1.65` e `1.70` e pode ampliar a busca se nenhum candidato atingir os critérios da seção 7. O valor escolhido será o menor que produza o efeito climático desejado sem exceder os limites de DNF.

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

Uma parada é genuína quando o carro permanece parado na própria caixa por pelo menos 4 segundos. Passagem pelo pit lane, drive-through e oscilações breves de telemetria não contam.

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

### `LiveBreakdown`

Continua sendo o dono do desgaste vivo por peça. O estado já guarda o desgaste de entrada em `entered`; a manutenção parcial deve usar esse valor como piso.

`service_pit()` deixa de zerar peças gastas e passa a aplicar a fórmula da seção 2.3. A operação não deve fazer nada em peças já quebradas nem ressuscitar um carro em DNF.

### `BreakdownDirector`

Expõe uma operação para aplicar a manutenção a um carro identificado pelo número do iRacing. O diretor continua responsável por deduplicação e por localizar o `LiveBreakdown` correto.

A lista pré-programada `service_laps` pode continuar atendendo simulações e previsões determinísticas, mas o fluxo ao vivo não dependerá dela para descobrir paradas reais.

### Monitor do iRacing

O detector existente de permanência na caixa é a autoridade sobre paradas genuínas. Ao confirmar a saída da caixa com pelo menos 4 segundos parado, o monitor aplica uma única manutenção ao carro, antes de processar sua próxima volta.

O limiar de 4 segundos deve ser compartilhado com o cálculo econômico pós-corrida, eliminando definições duplicadas de parada genuína.

## 5. Fluxo de dados

1. O carro larga com desgaste persistente carregado do banco.
2. Somente o carro do jogador recebe o alívio de entrada de até 6%.
3. A cada volta, `LiveBreakdown` acumula desgaste usando o multiplicador combinado de pista e clima, já limitado pela constante calibrada.
4. Em enduro, o detector de pit cronometra a permanência na caixa de cada carro.
5. Na saída de uma parada genuína, o monitor solicita ao diretor a manutenção parcial daquele carro.
6. O diretor atualiza o estado vivo, sem alterar o carro persistido.
7. A volta seguinte parte do desgaste parcialmente aliviado.
8. Após a corrida, a economia calcula e persiste o desgaste pela regra econômica existente; o pit continua reduzindo somente o sobrecusto de enduro nessa camada.

## 6. Previsão pré-corrida

O benefício de uma futura parada não é garantido no momento da previsão. Por isso, a previsão de enduro deve ser conservadora e calcular o risco antes de qualquer manutenção futura.

A interface deve deixar claro que uma parada preventiva pode reduzir o risco mostrado. Este ciclo não deve prometer uma volta exata de serviço que o jogador ainda não executou.

## 7. Critérios de aceitação

### Clima e grid

| Cenário | Alvo |
|---|---:|
| Carro saudável, clima neutro | quebra relevante próxima de 0% |
| Carro saudável, clima brutal | 5–10% de quebra relevante |
| Carro saudável, clima brutal | DNF menor ou igual a 2% |
| Time pobre esticando, clima neutro | DNF aproximadamente 10–15% |
| Grid em clima brutal | DNF total abaixo de 30% |

“Quebra relevante” significa penalidade grave ou DNF; falha leve não conta para esse alvo.

### Proteção do jogador

| Cenário | Alvo |
|---|---:|
| Jogador em equipe fraca | redução relativa de risco próxima de 20–25% |
| Jogador em equipe forte | diferença desprezível para IA equivalente |

### Enduro

| Cenário | Alvo |
|---|---:|
| Uma parada preventiva genuína | redução relevante contra o mesmo carro sem parada |
| Peça que largou gasta | nunca fica abaixo do desgaste de largada |
| Sprint | parada não produz manutenção preventiva |
| Drive-through ou parada menor que 4 s | nenhum benefício |

Os 60% de alívio constituem o primeiro candidato. O valor só deve mudar se o harness mostrar benefício imperceptível ou proteção excessiva.

## 8. Tratamento de bordas

- Uma oscilação transitória de `InPitStall` não pode contar como parada.
- Uma mesma permanência na caixa aplica manutenção uma única vez.
- Carro sem mapeamento válido no diretor é ignorado com segurança.
- Parada após DNF não altera o estado.
- Peça já marcada como quebrada não recebe manutenção.
- O serviço só é habilitado quando a corrida está classificada como enduro.
- A manutenção viva não grava diretamente no banco e não reduz o custo persistente já definido pela economia.

## 9. Estratégia de testes

### Testes determinísticos

- fórmula parcial nos dois exemplos numéricos aprovados;
- piso absoluto no desgaste de largada;
- peça abaixo de `SERVICE_WEAR_FLOOR` inalterada;
- múltiplas paradas sem atravessar o piso;
- peça quebrada e carro em DNF inalterados;
- sprint sem manutenção preventiva;
- parada curta e passagem pelo pit sem benefício;
- uma aplicação por parada genuína;
- `PLAYER_MAX_RELIEF = 0.06` escalando de equipe fraca a forte;
- mesma fonte de teto climático no risco e na economia.

### Harness Monte Carlo

- varrer candidatos de teto climático e registrar quebra qualquer, quebra relevante e DNF;
- comparar perfis saudável, limítrofe, pobre esticando e pobre degradado;
- comparar clima neutro e brutal;
- comparar enduro sem parada e com uma parada preventiva;
- confirmar todos os critérios da seção 7;
- conservar a semente fixa e a determinismo do relatório.

### Regressão

Toda a suíte do módulo de quebra deve permanecer verde. As demais suítes diretamente afetadas pelo monitor e pela manutenção econômica também devem ser executadas.

## 10. Fora de escopo

- separar clima de desgaste e aplicá-lo diretamente ao hazard;
- recalibrar toda a economia de peças;
- criar estratégia automática nova de pit para a IA;
- persistir manutenção preventiva como rejuvenescimento da peça;
- mudar `RISK_OPEN`, `HARD_WALL`, pesos de severidade ou `ENDURO_DNF_SCALE`, salvo se uma regressão demonstrar que os critérios aprovados são matematicamente incompatíveis.
