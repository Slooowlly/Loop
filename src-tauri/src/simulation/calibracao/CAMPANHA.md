# Plano da campanha de calibração

Documento, não código. O código vem depois do D, quando existir o que buscar.

Escrito porque improvisar a calibração de um espaço grande é exatamente o que produziu cinco
corridas com o mesmo resultado. Depois de C e D o espaço de parâmetros fica bem maior que o de
hoje, e "pessoa mexe na constante, roda, olha" não escala nem é auditável.

---

## 1. A ordem, e por que ela importa mais que o algoritmo

O erro caro numa campanha de calibração não é escolher mal o passo — é **calibrar contra alvo
móvel**: ajustar A contra uma métrica que B ainda vai mexer, e ter que refazer tudo. A ordem sai
de quais parâmetros são independentes e quais interagem.

O grafo de dependência, lido do orçamento de variância:

```
   [B] afinidade/forma/acerto  ──┐
                                 ├──►  camada de EVENTO (hoje 1,5% / 0,8%)
   [G] estratégia/safety car  ──┘

   [C] moeda vira tempo  ────────►  escala do RUÍDO (hoje 1,6% / 0,7%)

   [D] ar sujo/ultrapassagem  ───►  ATRITO — não é fonte de variância,
                                    é o que converte (ou não) ritmo em posição
```

**D é diferente dos outros e vem primeiro.** B, C e G adicionam variância; D muda a função de
transferência entre ritmo e resultado. Calibrar qualquer fonte de variância antes de D é medir
quanto de variância chega ao resultado por um canal que vai ser reescrito. A prova está no
baseline: com grade sorteada, ρ(skill × chegada) = 0,89 — o ritmo atravessa o pelotão sem
resistência, então hoje toda variância adicionada vira embaralhamento direto.

**Ordem proposta:**

| Fase | Calibra | Contra que métrica | Por que aqui |
|---|---|---|---|
| 0 | nada — só medir | todas | linha de base pós-C/D, congelada |
| 1 | atrito do D | ρ(grid/skill) na grade sorteada, `fracao_travado`, comboio | é a função de transferência; tudo depois depende dela |
| 2 | ruído do C | `frac_corrida` no orçamento, desvio de posição | escala com a distância; independente do atrito |
| 3 | evento do B | `frac_evento`, ρ(etapas consecutivas) | precisa do atrito fixo para saber quanto do evento chega ao resultado |
| 4 | estratégia do G | `frac_evento` (fatia de estratégia), trocas de liderança | interage com B pela mesma camada — calibrar junto, não em série |
| 5 | knobs por categoria (E) | separação rookie × topo | por último, sobre os parâmetros novos |

**As fases 3 e 4 interagem** e são a única coordenação difícil: B e G alimentam a MESMA camada do
orçamento. Calibrar B até o alvo de evento e depois adicionar G estoura o alvo. O jeito certo é
tratá-las como um bloco com um orçamento repartido — a repartição-alvo está na seção 3 do
[BASELINE.md](BASELINE.md#7-os-alvos-do-pacote-g) e é decisão de design, não de busca.

---

## 2. O orçamento de avaliações

O custo real, medido: **1008 corridas ≈ 0,25 s** em release. Uma medição completa (4 cenários) ≈
1 s. A decomposição de variância, que é a cara, ≈ 4 s por categoria.

Isso muda o cálculo em relação ao que se poderia supor: o gargalo **não é CPU**, é o número de
pontos que um humano consegue auditar. Uma busca que testa 5 000 combinações e devolve a melhor é
inauditável — ninguém sabe dizer por que aquele ponto ganhou, e o resultado vira outra constante
mágica, só que achada por máquina.

**Orçamento proposto por fase: 200–400 avaliações.** Não porque não caiba mais, mas porque acima
disso o resultado deixa de ser explicável. A restrição é epistemológica, não computacional.

### Triagem em três níveis — corrigida pela medição

| Nível | Volume | O que decide |
|---|---|---|
| **T1 — peneira** | **12 temporadas × 6 etapas (72 corridas)** | promove a metade melhor da varredura do eixo |
| **T2 — trabalho** | 30 × 12 (360 corridas) | ordena candidatos; é onde a descida coordenada vive |
| **T3 — veredito** | 84 × 12 (1008) + decomposição | só no finalista; é o número que entra no relatório |

A forma do T1 foi medida duas vezes, e a segunda derrubou a conclusão da primeira. As duas versões
ficam registradas porque o erro é instrutivo.

**Primeira medição, contra a simulação pré-reforma.** O plano original (3 × 8 = 24 corridas) dava
sinal/ruído de 1,2, e o ruído quase não caía de T2 (1,89) para T3 (1,83) apesar de 3× mais
corridas. Concluí que a incerteza dominante era **qual grid foi sorteado**, e portanto que se devia
cortar etapas em vez de temporadas.

**Segunda medição, na árvore reformada.** A tabela inverteu:

| Nível | corridas | ruído pré-reforma | ruído pós-reforma |
|---|---|---|---|
| 72 corridas | 72 | 0,60 | 2,76 |
| T2 | 360 | 1,89 | 0,80 |
| T3 | 1008 | 1,83 | 0,55 |

Agora o ruído cai com o volume, como ruído de amostragem deve cair. O comportamento anterior era
**artefato de uma simulação quase determinística** — sem variância por corrida, a única variação
que sobrava era estrutural. Assim que a corrida ganhou física, mais corridas voltaram a comprar
precisão. **A forma dos níveis tem que ser remedida a cada mudança grande do motor**, e por isso
`Nivel` é struct e não constante.

Sobrevive a regra operacional: **comparar dois pontos exige a MESMA semente**, porque grids
diferentes não são comparáveis. O `Avaliador` fixa a semente de propósito.

### A guarda da peneira: arrependimento, não concordância de ordenação

A guarda que eu escrevi primeiro media Spearman entre os totais de T1 e T2 ao longo de um eixo, e
**estava errada**: Spearman entre duas medições de uma função PLANA é ~0 por construção, qualquer
que seja a qualidade das duas. Nenhuma forma de T1 passava de 0,81, e o culpado era um eixo só —
`race_pace_spread`, com ρ de 0,29 — onde o objetivo não responde. As duas medições estavam
ordenando ruído, o que não é defeito da peneira.

O que a triagem precisa garantir não é ordenar bem: é **não jogar fora o ponto que o nível caro
escolheria.**

```
arrependimento = (melhor T2 entre os PROMOVIDOS − melhor T2 do eixo) / amplitude T2
```

| forma de T1 | corridas | ρ × T2 | **arrependimento** |
|---|---|---|---|
| 12 × 6 | 72 | 0,693 | 0,050 |
| 12 × 12 | 144 | 0,670 | 0,082 |
| **15 × 10** | **150** | 0,696 | **0,000** |
| 20 × 8 | 160 | 0,807 | 0,000 |
| 30 × 6 | 180 | 0,762 | 0,000 |

Pelo arrependimento a peneira nunca esteve quebrada — nem no 12 × 6. Fiquei em 15 × 10 por margem
(0,05 se compõe ao longo de duas passadas) mantendo T1 2,4× mais barato que T2. Num eixo plano o
arrependimento é zero automaticamente, que é o comportamento correto.

Travado em `triagem_t1_preserva_a_ordem_do_nivel_caro` (média sobre 4 eixos × 2 sementes) e
`eixo_plano_nao_gera_arrependimento`.

**Regra de promoção**, também corrigida: T1 → T2 promove a **metade melhor da própria varredura do
eixo** (mínimo 3). O corte ABSOLUTO do plano original ("nenhuma métrica fora por mais de 3×")
descartava o eixo inteiro quando todo o espaço está longe do alvo — que é exatamente a situação
inicial de qualquer calibração real, e foi o que a primeira implementação fez: a busca não saiu do
ponto de partida.

**Comparar pontos exige a MESMA semente.** Como a incerteza dominante é o sorteio dos grids, dois
pontos medidos com sementes diferentes não são comparáveis. O `Avaliador` fixa a semente para
todas as avaliações de uma busca de propósito.

---

## 3. Descida coordenada, com as duas ressalvas

Uma coordenada por vez, varrendo a faixa padrão, fixando o melhor, passando à próxima; duas ou
três passadas. É suficiente e é auditável: cada passo tem um "por quê" de uma linha.

**Ressalva 1 — a ordem das coordenadas importa quando elas interagem.** Descida coordenada
assume separabilidade aproximada. Onde não vale (atrito × variância: com ultrapassagem cara, mais
variância vira mais trânsito em vez de mais troca), a saída é agrupar as coordenadas acopladas
num par e varrer a grade 2D dele. Um par de 8×8 são 64 avaliações em T2 — 6 segundos. Cabe.

**Ressalva 2 — o ponto inicial contamina.** Rodar duas descidas de pontos iniciais distantes e
comparar. Se convergirem para lugares diferentes, o espaço tem vales múltiplos e o relatório tem
que dizer isso em vez de apresentar um vencedor.

### A função-objetivo — e por que ela mudou de escala

Distância normalizada às faixas, somada. Para cada métrica com alvo `[min, max]`:

```
d = 0                        se min ≤ x ≤ max
d = (min − x) / (max − min)  se x < min
d = (x − max) / (max − min)  se x > max
```

Normalizar pela largura da faixa é o que impede uma métrica de escala grande (desvio de posição,
0–20) de dominar uma de escala pequena (correlações, 0–1). Somar em vez de tirar média mantém o
custo de deixar uma métrica de fora.

**A correção:** essa distância é medida numa escala TRANSFORMADA, não na crua. A versão crua tem
gradiente fora da faixa — é linear na distância, não um degrau — mas tem um buraco mais sutil, e
ele é fatal exatamente na situação inicial de qualquer calibração real:

**métricas limitadas saturam perto do limite.** ρ entre etapas consecutivas está em 0,976 contra
alvo de 0,55. Como ρ é limitado por 1, mexer um knob de 1,4 para 10 move ρ em 0,13, e perto de 1
cada passo de parâmetro produz um passo de métrica cada vez menor. Na escala crua o gradiente
**existe mas encolhe** justamente onde a busca começa, e afunda abaixo do ruído de amostragem. Aí
sim a busca vira caminhada aleatória.

Por isso:

| Tipo de métrica | Escala | Efeito |
|---|---|---|
| correlações (−1, 1) | `atanh` (Fisher) | ρ 0,976 → z 2,19; ρ 0,55 → z 0,62. A distância passa de 0,43 para 1,57 |
| frações (0, 1) | `logit` | mesma razão, perto de 0 e de 1 |
| contagens e posições | linear | não têm teto, não saturam |

Isso não muda ONDE está o alvo — a faixa transformada tem as mesmas bordas —, só a métrica de
distância até ele. Travado em `escala_de_correlacao_restaura_gradiente_na_saturacao`.

**Pesos**: nenhum, por padrão. Peso é onde julgamento entra disfarçado de matemática. Se uma
métrica precisar valer mais, isso deve estar na LARGURA da faixa — uma faixa estreita já pesa
mais, e a razão fica escrita no comentário do alvo em vez de escondida num vetor de pesos.

---

## 4. Reportar "inalcançável" — o requisito que não pode se perder

Foi o achado mais valioso da varredura: **nenhum knob existente, em nenhum valor, chega perto do
alvo.** Uma busca ingênua que devolve só o melhor ponto encontrado esconde isso — ela sempre
devolve algo, e "o melhor de 400 pontos ruins" lido sem contexto vira "calibrado".

O relatório da busca tem que ter, obrigatoriamente:

1. **Distância do melhor ponto, por métrica**, não só a soma. Uma soma baixa pode esconder uma
   métrica catastroficamente fora.
2. **Veredito por métrica**: `ATINGIDO` / `PARCIAL` (dentro de 1,5× da faixa) / `INALCANÇÁVEL`
   (nenhum ponto do espaço chegou a 1,5×).
3. **Se o ótimo está na BORDA da faixa varrida**, dizer. Ótimo na borda significa que a faixa foi
   apertada demais ou que o parâmetro está saturando — e nos dois casos o número devolvido é
   suspeito.
4. **O melhor valor alcançado de cada métrica em QUALQUER ponto**, mesmo pontos que perderam no
   agregado. É isso que responde "esta métrica é alcançável de todo?", que é uma pergunta
   diferente de "o melhor ponto a atinge?".
5. **Falha explícita, não silenciosa**: se qualquer métrica sair `INALCANÇÁVEL`, a busca termina
   com veredito de falha e diz qual mecanismo falta, não com "melhor ponto encontrado: ...".

O modelo é o que a varredura já faz hoje ao classificar `MORTO` / `fraco` / `ALAVANCA`. Aquilo
foi útil porque o veredito é categórico e legível; o número sozinho não seria.

### A taxonomia do veredito, corrigida na implementação

A primeira versão errou exatamente aqui: ligou o veredito ao "melhor valor em qualquer ponto", e o
resultado foi um relatório com seis `ATINGIDO` que vinham de **pontos diferentes e mutuamente
incompatíveis**. Somados, davam ar de sucesso a um espaço que não entrega nada junto. O veredito
cruza as duas leituras:

| Veredito | Condição | Conserto |
|---|---|---|
| `ATINGIDO` | dentro da faixa **no ponto ótimo**, junto com todas as outras | nada |
| `PARCIAL` | fora no ótimo, mas a ≤1,5× | ajuste fino |
| `CONFLITO` | **atingível sozinha, impossível em conjunto** — a alavanca existe e está sendo gasta contra outra métrica | rever alvos ou desacoplar; mexer mais nas constantes não resolve |
| `INALCANÇÁVEL` | nenhum ponto chegou a 1,5×, nem isoladamente | construir mecanismo |

`CONFLITO` e `INALCANÇÁVEL` são defeitos diferentes com consertos diferentes, e confundi-los custa
o pacote errado. É a mesma lição da taxonomia dos três knobs mortos.

### O sexto requisito, descoberto na implementação: o portão do orçamento

Não estava no plano e é o mais importante de todos. Rodada sobre o espaço atual, a busca **atinge
as oito métricas de resultado** com `race_variance = 10` e `pack_density = 10`. Todas dentro da
faixa. E o resultado é lixo: a decomposição no ponto final dá piloto em 27,8% (alvo 38–52%) e
corrida em 68,7% (alvo 22–35%). O campeonato não ficou disputado — ficou sorteado.

**Métrica de resultado não distingue os dois casos.** Dispersão alta e vencedores variados saem
tanto de disputa quanto de loteria. Por isso a decomposição de variância roda SEMPRE no ponto
final, e o veredito é falha quando o orçamento sai da faixa, mesmo com as oito métricas verdes. É
a regra 6 da seção 6 promovida de recomendação a portão executável.

---

## 5. Contra o que cada parâmetro responde

Preenchível de verdade só depois de C e D existirem, mas o esqueleto é este — e o próprio ato de
não conseguir preencher uma linha já é diagnóstico (é como o `overtaking_difficulty_multiplier`
foi pego):

| Parâmetro | Métrica primária | Métrica de guarda (não pode piorar) |
|---|---|---|
| custo de ultrapassagem (D) | ρ(grid × chegada) na grade sorteada | recuperação máxima — a armadilha nº 1 |
| janela de ar sujo (D) | `fracao_travado`, maior comboio | CV dos gaps |
| ruído por distância (C) | `frac_corrida` no orçamento | vencedores distintos |
| amplitude de forma (B) | `frac_evento`, ρ(etapas consecutivas) | reprodutibilidade do grid |
| taxa de safety car (G) | trocas de liderança | ρ(grid × chegada) — SC demais vira loteria |

A coluna de guarda é a que impede o modo de falha clássico da calibração: acertar a métrica-alvo
destruindo outra. Toda avaliação reporta as duas.

---

## 6. O que a campanha NÃO deve fazer

- **Não calibrar contra o baseline atual.** Ele é o retrato do defeito. O alvo são as faixas.
- **Não mexer em knob com veredito `MORTO` ou `fraco`.** A varredura já disse que não há alavanca;
  ajustá-los produz a ilusão de progresso mais cara que existe.
- **Não recongelar `snapshot::CONGELADO` durante a busca.** O congelado é a âncora histórica; ele
  muda por decisão, num commit próprio, e nunca como efeito colateral de uma calibração.
- **Não aceitar um resultado sem rodar a decomposição de variância no ponto final.** As métricas
  de resultado podem ficar certas com o orçamento errado — a distribuição bate e a razão de ela
  bater é outra. É o único jeito de pegar isso.
