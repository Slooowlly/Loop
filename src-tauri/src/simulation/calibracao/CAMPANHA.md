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

---

## 7. O MANIFESTO — contra o espaço que vai existir

As seções 1–6 são o método. Esta é a lista de compras: quais parâmetros, em que faixa, em que fase,
com que orçamento. Ela existe agora porque a campanha estava bloqueada por algo que o harness não
podia ver: **as constantes dos pacotes de mecanismo são `const`, resolvidas em compilação.** A busca
só alcança os multiplicadores de `profile/`, e a varredura já provou que aqueles movem ruído. Rodar a
campanha hoje daria `INALCANÇÁVEL` correto e inútil — sem nunca tocar no parâmetro que a decomposição
aponta como o déficit.

### 7.1 O déficit — e a retratação do "fator 7"

**Primeiro a retratação, porque ela invalida o argumento mais forte que eu tinha.**

Eu reportei a camada de evento em 3,1% e 3,4% contra alvo de 20–32%, e argumentei que o déficit era
robusto à hipótese que o ameaça: "3% contra 20% é fator 7, não é borda de faixa". **O número estava
medindo o caminho errado.** As três camadas do pacote B não são aplicadas dentro de
`simulation::race` — elas vivem na esteira de modificadores de `commands/race/simulacao.rs`, que soma
pontos de skill ao `SimDriver` antes de chamar a corrida. Este harness monta o grid e chama a corrida
direto, então **o pacote B inteiro estava invisível para a medição.**

Com a esteira replicada no arena (ver `arena::aplicar_esteira_de_forma`):

| Fonte | rookie s/esteira | **rookie real** | alvo | gt3 s/esteira | **gt3 real** | alvo |
|---|---|---|---|---|---|---|
| Piloto (permanente) | 79,1% | 69,4% | 38–52% | 43,0% | 38,7% | 22–35% |
| Equipe / carro | 0,1% | 2,3% | 0–5% ✅ | 48,7% | 46,7% | 22–38% |
| Evento — pista (afinidade) | 0,6% | 4,8% | — | 0,5% | 3,0% | — |
| Evento — clima + forma + acerto | 2,5% | 7,0% | — | 2,9% | 6,7% | — |
| **Evento (total)** | 3,1% | **11,8%** | **20–32%** | 3,4% | **9,7%** | **22–34%** |
| Corrida (ruído puro) | 17,7% | 16,5% | 22–35% | 4,8% | 4,9% | 12–24% |

**O déficit é fator 1,7–2,7, não fator 7.** E a consequência epistemológica é a que importa: o
argumento não sobrevive mais à hipótese que o ameaçava. Se a faixa de evento fosse 10–16% em vez de
20–32%, a rookie a 11,8% já estaria **dentro**. "As faixas estão agressivas" volta a ser uma
explicação viável para essa célula, e o `.json` do iRacing volta a ser o que decide.

O que sobrevive: **a camada de evento continua sendo a maior lacuna do orçamento**, nas duas
categorias, e continua sendo o lugar certo para a campanha começar. Só não é mais uma lacuna que
dispensa o dado real.

O que também sobrevive intacto, porque não depende dessa medição: **o permanente não precisa de
calibração.** Ele está alto como FRAÇÃO, não em valor absoluto — e ligar a esteira já o derrubou de
79,1% para 69,4% sem ninguém tocar em `driver_generation`.

### 7.2 O bracket, recalculado

Mantendo o permanente absoluto e resolvendo para o alvo, com os números do caminho certo:

| Categoria | σ da camada de EVENTO | σ do RUÍDO por corrida |
|---|---|---|
| rookie | **×1,4 – ×1,9** | ×1,4 – ×1,7 |
| gt3 | **×1,6 – ×2,2** | ×1,9 – ×2,4 |

O bracket anterior (×3,4–4,3 e ×3,6) era consequência direta do erro de medição. A faixa varrida
segue de ×1 a ×6 — agora com folga confortável em vez de mal cobrindo a predição.

**A hipótese fraca continua sendo a mesma:** o trem de carros do pacote D comprime a ordem, então
parte da variância de ritmo adicionada não chega à posição. Se for o caso, o `k` medido sai maior que
o previsto.

E há uma segunda atenuação, agora conhecida: **`SimDriver::skill` é `u8`**. A esteira arredonda o
ajuste para inteiro, então um ajuste de ±0,4 ponto desaparece. Com as escalas atuais (2,0 a 3,0
pontos) isso é perda de resolução, não anulação — mas significa que subir as escalas do `forma.rs`
briga em parte com a quantização, e que o `k` efetivo é menor que o `k` nominal. A própria esteira
registra a dívida, com a nota de que a resolução voltaria "quando a moeda virar TEMPO"; a moeda já
virou (pacote C) e o campo continua `u8`.

### 7.2b O que ligar a esteira já entregou, sem calibrar nada

| Métrica | rookie s/ | rookie c/ | gt3 s/ | gt3 c/ | alvo (rookie / gt3) |
|---|---|---|---|---|---|
| ρ(etapa N × N+1) | 0,828 | **0,782** | 0,919 | **0,862** | 0,20–0,55 / 0,35–0,70 |
| Desvio da posição | 2,19 | **2,58** | 1,48 | **2,02** | 3,5–6,5 / 2,5–5,0 |
| Vencedores distintos | 2,13 | **2,82** | 2,11 | **2,64** | 5–10 / 3–8 |
| Trocas de liderança | 0,93 | 1,05 | 0,93 | **1,56 ✅** | 2–7 / 1–5 |
| P(melhor fora do top 5) | 1,7% | 3,8% | 2,9% | 5,7% | 15–35% / 8–25% |

A gt3 ganhou uma segunda célula na faixa (`trocas_de_lideranca`), e o desvio dela chegou a 2,02
contra um piso de 2,50 — perto. Isto é o pacote B fazendo o que foi construído para fazer, e é a
primeira vez que ele aparece numa medição deste harness.

### 7.3 Os endereços

Contei **30** constantes calibráveis nos quatro blocos, não 25 — a diferença são quatro privadas do
`forma.rs` (repartições internas, não magnitudes) e uma que é estrutural e não deve entrar na busca.

#### `simulation/forma.rs` — a camada de evento. **É aqui que a campanha começa.**

| Constante | Hoje | Faixa | Fase | Contra |
|---|---|---|---|---|
| `ACERTO_ESCALA_PONTOS` | 2,5 | 2,5 – 15 | **1** | `frac_evento`, ρ(N,N+1) |
| `AFINIDADE_ESCALA_PONTOS` | 3,0 | 3,0 – 18 | **1** | `frac_evento_pista` |
| `FORMA_ESCALA_PONTOS` | 2,0 | 2,0 – 12 | **1** | `frac_evento`, sequências |
| `FORMA_RHO` | 0,65 | 0,20 – 0,85 | **1** | ρ(N,N+1) — ver 7.4 |
| `ACERTO_FRACAO_EQUIPE` | 0,70 | 0,3 – 0,9 | 1 | repartição piloto:carro do evento |
| `MULT_AFINIDADE_QUALI` | 1,5 | 1,0 – 2,5 | 5 | `reprodutibilidade_do_grid` |
| `AFINIDADE_FRACAO_IDIOSSINCRATICA` | 0,65 | — | — | legibilidade, não distribuição |
| `AFINIDADE_GANHO_ESTILO` | 1,8 | — | — | idem |
| `FORMA_PESO_ANIMO` | 0,20 | 0,0 – 0,5 | 5 | laço pódio→confiança→ritmo |
| `TETO_SIGMAS` | 2,5 | 2,0 – 4,0 | 5 | `p_melhor_fora_top5` (a cauda) |

#### `simulation/race/motor.rs` — a escala do ruído por corrida

| Constante | Hoje | Faixa | Fase | Contra |
|---|---|---|---|---|
| `VOLTAS_DE_REFERENCIA_DO_RUIDO` | 20,0 | 5 – 60 | 2 | `frac_corrida` (é a âncora de escala) |
| `CORRELACAO_DO_RUIDO_ENTRE_TRECHOS` | 0,5 | 0,0 – 0,9 | 2 | quanto do ruído sobrevive à soma |

`CORRELACAO_DO_RUIDO_ENTRE_TRECHOS` é o parâmetro mais subestimado da lista: ruído i.i.d. por
segmento se auto-cancela na soma (foi o defeito original), e é a correlação que decide quanto dele
chega ao resultado. Ele é multiplicativo com a escala, então **os dois são um par acoplado** e a
ressalva 1 da seção 3 se aplica: grade 2D, não coordenadas em série.

#### `simulation/race/trafego.rs` — o atrito (fase 1 do plano original, agora fase 3)

| Constante | Hoje | Faixa | Contra |
|---|---|---|---|
| `PROB_BASE_ULTRAPASSAGEM` | 0,35 | 0,10 – 0,70 | ρ(grid × chegada) na grade sorteada |
| `JANELA_DE_ATAQUE_MS` | 800 | 300 – 2000 | idem |
| `JANELA_AR_SUJO_MS` | 1000 | 300 – 2500 | `fracao_travado`, maior comboio |
| `PERDA_MAXIMA_AR_SUJO_PONTOS` | 3,0 | 0,5 – 8,0 | idem |
| `GAP_MINIMO_ENTRE_CARROS_MS` | 150 | 50 – 500 | recuperação máxima (**armadilha nº 1**) |
| `CUSTO_TENTATIVA_FALHA_ATACANTE_MS` | 350 | 100 – 1500 | recuperação máxima |
| `CUSTO_TENTATIVA_FALHA_DEFENSOR_MS` | 150 | 50 – 800 | idem |
| `PESO_DA_HABILIDADE_NA_ULTRAPASSAGEM` | 0,60 | 0,2 – 0,9 | ρ(skill × chegada) |
| `PESO_DA_AGRESSIVIDADE_NA_ULTRAPASSAGEM` | 0,30 | 0,0 – 0,6 | DNF/etapa |
| `DELTA_DE_RITMO_QUE_SATURA` | 6,0 | 2,0 – 15 | recuperação máxima |
| `RISCO_DE_CONTATO_NA_TENTATIVA_FALHA` | 0,06 | 0,0 – 0,07 | DNF/etapa, SC/etapa, **lesões/etapa, free agents sem vaga** |

> **O teto de `RISCO_DE_CONTATO_NA_TENTATIVA_FALHA` caiu de 0,20 para 0,07.** O contato deixou
> de ser só tempo perdido: cada um é uma rolagem de lesão de 20% em cada um dos dois carros, e
> lesão grave encerra carreira — o que empurra aposentadoria, geração de novato e sobra de
> gente sem assento. Medido num rascunho histórico de 26 temporadas:
>
> | contato | lesões/largada | aposentados | free agents |
> |---|---|---|---|
> | 0,05 | 1,18% | 353 | 20 |
> | 0,075 | 1,69% | 377–408 | 37–42 |
> | 0,10 | 2,22% | 444 | 53 |
>
> O teste `closed_system_playable_world_has_no_orphans_and_drivers_raced` trava free agents em
> ≤ 40, e **a relação não é linear** — de 0,05 para 0,075 os órfãos dobram. Acima de ~0,07 a
> busca sai procurando atrito de pista e volta com um mundo inchado. Régua:
> `simulation::race::tests::medicao`.

#### `simulation/race/estrategia.rs` — box e safety car (bloco acoplado com a fase 1)

| Constante | Hoje | Faixa | Contra |
|---|---|---|---|
| `CAOS_DO_RELANCAMENTO` | 1,0 | 0,0 – 4,0 | ρ(pré-SC × chegada) — o embaralhamento |
| `FRACAO_DO_CUSTO_SOB_SAFETY_CAR` | 0,40 | 0,1 – 0,9 | trocas de liderança |
| `CUSTO_DE_PARADA_MS` | 22 000 | 12k – 35k | trocas de liderança |
| `JANELA_DE_PARADA` | (0,35, 0,65) | largura 0,1 – 0,5 | estratégias distintas no grid |
| `FATOR_DE_COMPRESSAO_DO_PELOTAO` | 0,20 | 0,0 – 0,6 | margem do campeão |
| `GAP_MINIMO_SOB_SAFETY_CAR_MS` | 250 | 100 – 600 | guarda: ordem preservada |
| `MINUTOS_MINIMOS_PARA_PARADA` | 40 | — | **estrutural, fora da busca** |

`MINUTOS_MINIMOS_PARA_PARADA` fica de fora de propósito: ele não é uma magnitude, é a chave
liga/desliga do "rookie não para". Varrê-lo mudaria a IDENTIDADE da categoria de entrada dentro de
uma busca de distribuição, e o alvo declarado ("rookie sem parada, 1–2 estratégias no grid") é
decisão de design.

### 7.4 A ordem, refeita pela decomposição

A seção 1 ordenou por dependência de mecanismo e pôs o atrito primeiro. A decomposição reordena, e o
motivo é que o atrito **já foi entregue e já foi medido**: as quatro sondas de grade sorteada do
pacote D bateram o alvo, e a recuperação máxima ficou na faixa. Calibrar uma função de transferência
que já está no alvo, antes de ligar a fonte que está a um décimo, é otimizar o canal de um sinal que
não existe.

| Fase | Calibra | Por que aqui |
|---|---|---|
| **1** | camada de evento (`forma.rs`, 5 consts) | é o déficit de 10×; nada mais chega perto |
| **2** | ruído por corrida (`motor.rs`, par 2D) | segunda maior lacuna; independente da fase 1 |
| **3** | atrito (`trafego.rs`) | ajuste fino de uma função de transferência já no alvo |
| **4** | box e SC (`estrategia.rs`) | alimenta a MESMA camada da fase 1 — bloco acoplado |
| **5** | por categoria (E) + cauda | por último, sobre parâmetros já fixos |

**As fases 1 e 4 continuam sendo o bloco acoplado**, com a repartição-alvo da seção 3 do
[BASELINE.md](BASELINE.md). Mas agora tem um critério de repartição que sai de matemática em vez de
gosto — ver abaixo.

### 7.5 A eficiência-ρ: por que `ACERTO` vem antes de `FORMA`

O sintoma central é ρ(etapa N × N+1). Uma fonte de variância com autocorrelação `a` entre etapas
vizinhas contribui com `σ²` para a variância e `a·σ²` para a covariância. Logo a **eficiência de uma
fonte em derrubar ρ é `1 − a`**:

| Fonte | autocorrelação entre etapas vizinhas | eficiência-ρ |
|---|---|---|
| permanente (skill, carro) | 1,0 | **0** — não derruba ρ nunca |
| forma (AR(1), ρ = 0,65) | 0,65 | 0,35 |
| acerto de fim de semana | 0 (sorteio por evento) | **1,0** |
| afinidade piloto × pista | 0 entre pistas distintas | **1,0** |
| ruído por corrida | 0 | **1,0** |

Três consequências operacionais:

1. **`ACERTO_ESCALA_PONTOS` e `AFINIDADE_ESCALA_PONTOS` são 2,9× mais eficientes que
   `FORMA_ESCALA_PONTOS`** por ponto de skill gasto. A busca deve varrê-las primeiro.
2. ~~**`FORMA_ESCALA_PONTOS` pode piorar o sintoma.**~~ **REFUTADO pela medição — ver 7.8.** A
   predição era que subir a amplitude da forma com ρ = 0,65 empurraria ρ(N,N+1) para CIMA. Não
   empurra, em nenhum ρ testado. O erro na derivação: eu comparei a autocorrelação da fonte contra o
   permanente (a = 1) e concluí "parcialmente permanente, logo empurra para cima". A comparação certa
   é contra o **ρ agregado atual**. Uma fonte com autocorrelação `a` puxa o ρ do conjunto na direção
   de `a`; como ρ agregado está em 0,78–0,85, qualquer fonte com `a < 0,78` o derruba — e 0,65 < 0,78.
   `FORMA_ESCALA_PONTOS` é um lever normal, só mais fraco quanto maior o ρ.
3. **`FORMA_RHO` e `FORMA_ESCALA_PONTOS` são um par acoplado**, pela mesma razão. Grade 2D.

### A quarta consequência: a eficiência-ρ não é o critério completo

Afinidade e acerto têm o mesmo `a = 0` e a mesma eficiência-ρ na fórmula, mas **não compram a mesma
coisa**, e o que os separa é a estrutura de defasagem em outro eixo:

| Fonte | ρ entre etapas VIZINHAS | ρ entre a MESMA pista de temporadas diferentes |
|---|---|---|
| afinidade piloto × pista | 0 | **1** — `hash(driver_id, track_id)`, sem termo de temporada |
| acerto de fim de semana | 0 | **0** — `hash(temporada, rodada, equipe)` |
| forma do momento | 0,65 | ~0 — o AR(1) parte do estado, que divergiu |

Verificado no código, não suposto: `afinidade_pista(driver_id, track_id, estilo)` não recebe
temporada; `acerto_fim_de_semana(temporada, rodada, team_id, driver_id)` recebe.

Portanto: **a afinidade compra variedade DENTRO da temporada e nenhuma variedade ENTRE temporadas.**
Ela derruba ρ(N,N+1), aumenta vencedores distintos e trocas de liderança — tudo isso é real —, mas
não produz "o piloto teve um ano ruim", e duas temporadas com o mesmo calendário e o mesmo grid saem
com a mesma assinatura de quem voa onde. Acerto e forma compram as duas coisas.

Isso é o critério que justifica a repartição, e ele é **testável**: `frac_evento_pista` isola a
afinidade, porque ela é a única camada indexada por `track_id`.

### 7.5b A repartição MEDIDA — e ela diz que a afinidade está pesada demais

Descontando o que o perfil de pista já produzia sem a esteira (0,6 pp na rookie, 0,5 pp na gt3) e o
clima (2,5 pp e 2,9 pp), sobra a contribuição das três camadas do pacote B:

| Sub-fonte | rookie (pp) | fatia do B | gt3 (pp) | fatia do B | alvo proposto |
|---|---|---|---|---|---|
| afinidade piloto × pista | 4,2 | **48%** | 2,5 | **40%** | 20–30% |
| forma + acerto (agregados) | 4,5 | 52% | 3,8 | 60% | 70–80% |

**A afinidade é ~1,8× mais pesada do que a repartição pede**, nas duas categorias. E faz sentido
mecanicamente: `AFINIDADE_ESCALA_PONTOS = 3,0` é a MAIOR das três constantes, quando pelo argumento
da reprodutibilidade ela deveria ser a menor. Ela ainda ganha o multiplicador `MULT_AFINIDADE_QUALI =
1,5` no canal de classificação, que nenhuma das outras duas tem — o que explica por que ρ(grid ×
chegada) quase não se moveu ao ligar a esteira (0,89 → 0,89): a camada mais forte empurra grid e
chegada na mesma direção.

Recomendação para a fase 1, agora apoiada em medição em vez de gosto: **baixar
`AFINIDADE_ESCALA_PONTOS` enquanto sobe `ACERTO_ESCALA_PONTOS`**, mantendo a soma em variância no
bracket de ×1,4–2,2. É uma redistribuição antes de ser um aumento, e a diferença importa: aumentar as
três juntas mantém a afinidade em 48% e leva a variedade entre temporadas junto com o resto — o
sintoma "o mundo parece o mesmo todo ano" não é medido por nenhuma das oito métricas de resultado.

Repartição-alvo da camada de evento inteira, para o portão do orçamento:

| Sub-fonte | Fatia da camada de evento | Por quê |
|---|---|---|
| acerto de fim de semana | 35–50% | maior fonte, não repete, 70% dela é da equipe |
| afinidade piloto × pista | 15–25% | caráter legível; acima disso o calendário decide |
| forma do momento | 10–20% | gera sequências, que é seu valor; eficiência-ρ baixa |
| estratégia e safety car (G) | 20–30% | é a fatia do bloco acoplado da fase 4 |

**A lacuna de medição que sobra**: separar forma de acerto dentro do agregado exige congelar uma das
duas, e congelar exige que a escala seja injetável. Enquanto for `const`, a fatia de cada uma é
arbitrada; a da afinidade não é mais.

O `teto_de_azar` (25% na entrada, 20% no topo) atravessa isto: só a fatia do SC e o ruído por corrida
contam como azar. Acerto, afinidade e forma são atribuíveis a qualidade — de equipe, de casamento
piloto-pista, de momento — e por isso a campanha pode encher a camada de evento sem estourar o teto.
**Encher a camada de evento com `race_variance` estouraria**, e foi exatamente o que a busca sobre o
espaço morto tentou fazer (68,7% em corrida). O portão do orçamento existe para essa distinção.

### 7.6 Orçamento e critério de aceitação

| Fase | Consts | Grades 2D | Avaliações estimadas |
|---|---|---|---|
| 1 | 5 | 1 (`FORMA_ESCALA` × `FORMA_RHO`) | ~220 |
| 2 | 2 | 1 (o par inteiro) | ~130 |
| 3 | 11 | 0 | ~360 |
| 4 | 6 | 0, mas acoplada com a 1 | ~280 |
| 5 | 4 | 0 | ~140 |

Dentro dos 200–400 por fase da seção 2, exceto a fase 3, que fica no teto por ter 11 eixos — e é
aceitável porque ela é ajuste fino de algo já no alvo, então o risco de um eixo mal explorado é baixo.

**Critério de aceitação de cada fase**, nesta ordem, e a primeira que falhar interrompe:

1. O portão do orçamento passa no ponto final (T3 + decomposição).
2. Nenhuma métrica sai `CONFLITO` ou `INALCANÇÁVEL`.
3. O ótimo não está na borda de nenhuma faixa varrida.
4. As duas partidas distantes convergem para o mesmo vale.
5. As métricas de guarda da seção 5 não pioraram além de 10% do valor congelado.

### 7.6b A NONA métrica: excesso de emenda percebida

Alvo declarado, não consequência esperada — `assinatura::FAIXA_DE_EMENDA = (0,08, 0,20)`.

Ela mede `P(uma temporada-piloto conter 3+ resultados consecutivos acima da média do piloto)` contra
a **mesma temporada embaralhada**. Teste de permutação: o nulo destrói a ordem e preserva a
distribuição, então o excesso é a sequência que só a ordem carrega.

Por que é alvo e não esperança: a interface **já** mostra seta de tendência na forma, e o jogo hoje
entrega 0,046 de excesso. A promessa está feita na tela; a fase 1 tem que cumpri-la, e nenhuma das
oito métricas de resultado nem o portão do orçamento mede isso.

**Retratação sobre a medição anterior.** Eu havia reportado que "com a amplitude de hoje a sequência
é indistinguível de ruído — 0,02 corrida, o mesmo que ρ = 0". Era artefato da grandeza errada: com a
contagem, hoje mede +0,046 e ρ = 0 mede −0,016. Fraco, sim; igual a nenhuma memória, não. O critério
antigo ("meia corrida de excesso em 12 etapas") não era atingido por ρ nenhum — o que já era
evidência de que o critério estava errado, e não só a amplitude.

**E ela revela um conflito, que é melhor conhecer antes da fase 1:**

| Objetivo | O que quer da `FORMA_ESCALA` |
|---|---|
| ρ(N × N+1) na faixa | **menos** peso — autocorrelação 0,65 a torna a pior das três para derrubar ρ |
| excesso de emenda na faixa | **mais** peso — só a forma tem memória, só ela produz emenda |

E o bracket da fase 1 (×1,4–2,2 sobre a camada inteira) leva a forma de 2,0 para ~2,8–4,4, onde o
excesso vai de 0,046 para ~0,054 — **não chega ao piso de 0,08**. Chegar exige forma em ~6, que é
redistribuir *para dentro* dela, contra a recomendação de 7.5b. Decisão de design, não de busca; se
a busca resolver sozinha, sai `CONFLITO` e é isso que o veredito existe para dizer.

### 7.7 Os pré-requisitos do motor, em ordem

Três, e a ordem entre eles é a do adendo do pacote H.

**1. Injetabilidade das constantes.** Sem isso a busca não alcança nada do que esta seção lista.
Critério: com os padrões, os quatro cenários de `snapshot::CONGELADO` reproduzem `a905ca2` em
Δ = 0,000. `compara_com_congelado` é exatamente esse teste e já existe — é o portão de entrada da
campanha, não um relatório.

**2. A esteira vira função pura.** Bloqueante para a FASE 1 especificamente, porque a fase 1 calibra
justamente as constantes que passam por ela. Enquanto a esteira viver dentro do
`#[tauri::command]`, o harness só a alcança por espelho sem guarda, e o baseline oficial não pode
depender disso. É também quando o congelado é reescrito com a esteira ligada — uma vez só.

**3. O canal de ritmo vira `f64`.** Não bloqueia a partida da campanha, mas **distorce o resultado
dela** se ficar para depois: a fase 1 sobe amplitude contra um arredondamento para `u8`, mede uma
resposta amortecida, e compensa empurrando o parâmetro mais alto do que precisa. O valor calibrado
sairia errado por um fator que ninguém veria — e teria que ser refeito quando a resolução chegasse.

**Correção medida (7.8): o item 3 NÃO precede a campanha.** Eu argumentei que a quantização faria a
fase 1 medir uma resposta amortecida e compensar empurrando o parâmetro alto demais. Medido, o
arredondamento custa **0–2% da resposta** na faixa que a fase 1 percorre — dentro do ruído. O
argumento era meu, foi adotado, e está errado para a fase 1. O `f64` continua valendo, por outro
motivo, e pode vir depois.

---

## 7.8 A PRÉVIA da fase 1 — direção, não resposta

Rodada no espelho da esteira, que é o único lugar do projeto onde a camada de evento é ajustável
hoje. **Nada aqui é resposta da campanha e nada aqui pode ser congelado.** O espelho é cópia sem
guarda, arredonda como a esteira e vai ser descartado quando a função pura chegar. **A fase 1 roda de
novo contra a função pura, inclusive se estes números baterem** — bater não prova que o espelho está
certo, só que ele é consistente consigo mesmo.

Volume: 30 temporadas × 12 etapas (T2) por célula. O passo parametrizado da forma reduz exatamente à
função do jogo quando ρ = `FORMA_RHO`, travado em `passo_de_forma_com_rho_do_jogo_e_o_do_jogo`.

### 7.8.1 A predição da forma: REFUTADA, e a regra corrigida

ρ(etapa N × N+1), varrendo o par:

| rookie — ρ \ escala | 2,0 | 4,0 | 6,0 | 9,0 |
|---|---|---|---|---|
| 0,00 | 0,766 | 0,719 | 0,654 | **0,550** |
| 0,35 | 0,767 | 0,750 | 0,699 | 0,636 |
| 0,65 (hoje) | 0,776 | 0,756 | 0,737 | 0,714 |
| 0,85 | 0,776 | 0,765 | 0,766 | 0,751 |

| gt3 — ρ \ escala | 2,0 | 4,0 | 6,0 | 9,0 |
|---|---|---|---|---|
| 0,00 | 0,841 | 0,806 | 0,748 | 0,656 |
| 0,35 | 0,844 | 0,820 | 0,787 | 0,731 |
| 0,65 (hoje) | 0,849 | 0,843 | 0,826 | 0,796 |
| 0,85 | 0,857 | 0,853 | 0,849 | 0,837 |

**ρ cai com a escala em todas as linhas.** A predição de sinal está morta.

O que sobrevive é a **ordenação**, e ela sobrevive limpa: a resposta é monótona em ρ, e a linha
ρ = 0,85 é quase plana (−0,025 na rookie, −0,020 na gt3) contra −0,216 e −0,185 na linha ρ = 0. A
eficiência-ρ prevê exatamente essa ordem. O que ela errou foi o **ponto de inversão**.

A regra corrigida: uma fonte com autocorrelação `a` puxa o ρ agregado **na direção de `a`**, não na
direção de 1. Com ρ agregado em 0,78–0,85, uma fonte de `a = 0,65` o derruba. Ela só o levantaria se
`a` fosse maior que o agregado atual — e a linha ρ = 0,85 da gt3, onde o agregado é 0,849, é
justamente a que fica plana. O sinal previsto aparece na fronteira certa; só não existe faixa
utilizável acima dela.

**Consequência para a fase 1**: `FORMA_ESCALA_PONTOS` é um lever normal, não um parâmetro que anda
para trás. E `FORMA_RHO` é um lever **forte** que eu tinha tratado como acompanhante: sozinho ele
percorre 0,20 de ρ(N,N+1) na rookie (0,550 → 0,751 em escala 9,0), mais do que a maioria dos knobs
existentes. Ele sobe para a lista principal da fase 1.

E o número que muda a expectativa da campanha: **rookie com `FORMA_ESCALA = 9,0` e `FORMA_RHO = 0`
mede ρ = 0,550, o teto exato da faixa-alvo (0,20–0,55).** É a primeira vez que qualquer configuração
deste projeto toca o alvo do sintoma central.

### 7.8.2 Redistribuir contra escalar: empate técnico, com desempate num lugar só

Soma em variância travada por linha (a função `redistribuindo` preserva `afinidade² + acerto²`, não a
soma linear — senão as pernas não seriam comparáveis):

| rookie | σ evento | fatia afin. | ρ(N,N+1) | desvio | vencedores |
|---|---|---|---|---|---|
| hoje | 4,39 | 46,8% | 0,776 | 2,61 | 2,90 |
| escalar ×1,5 | 6,58 | 46,8% | 0,715 | 2,99 | 3,37 |
| **redistribuir ×1,5** | 6,58 | 18,7% | 0,717 | 2,97 | **3,67** |
| escalar ×2 | 8,77 | 46,8% | 0,648 | 3,36 | 3,77 |
| **redistribuir ×2** | 8,77 | 18,7% | 0,640 | 3,38 | **3,93** |

| gt3 | σ evento | fatia afin. | ρ(N,N+1) | desvio | vencedores |
|---|---|---|---|---|---|
| hoje | 4,39 | 46,8% | 0,849 | 2,10 | 2,77 |
| escalar ×1,5 | 6,58 | 46,8% | 0,794 | 2,50 | 3,07 |
| **redistribuir ×1,5** | 6,58 | 18,7% | 0,790 | 2,51 | **3,17** |
| escalar ×2 | 8,77 | 46,8% | 0,730 | 2,89 | 3,23 |
| **redistribuir ×2** | 8,77 | 18,7% | 0,728 | 2,89 | **3,30** |

Em ρ e desvio os dois movimentos são indistinguíveis (Δ ≤ 0,008, dentro do ruído de T2, que é 0,80 em
unidades da função-objetivo). **Em vencedores distintos, redistribuir ganha nas quatro comparações**,
por 0,07 a 0,30. Consistente demais em sinal para ser sorteio, pequeno demais para decidir sozinho.

A leitura: redistribuir **não custa nada** e ganha um pouco onde deveria ganhar — a afinidade é a
camada que mais concentra o embaralhamento nas mesmas mãos (o piloto que voa naquela pista é sempre o
mesmo), então tirar peso dela espalha as vitórias. E a razão principal para redistribuir continua
sendo a que **nenhuma das oito métricas mede**: variedade entre temporadas.

Recomendação mantida, agora com o custo conhecido: redistribuir é grátis, então a fase 1 deve partir
do ponto redistribuído em vez do escalado.

Nota metodológica: também não houve sinal de não-linearidade no `k` de posição por ponto de skill —
que era a hipótese fraca do bracket. Escalar ×1,5 e ×2 produz respostas quase proporcionais em desvio
(+0,38 e +0,75 na rookie), o que sugere que a linearidade local vale na faixa de interesse.

### 7.8.3 O custo da quantização: 0–2%, e isso reverte uma prioridade

| rookie | ρ det. | ρ dither | desvio det. | desvio dither |
|---|---|---|---|---|
| ×1,0 | 0,776 | 0,769 | 2,61 | 2,62 |
| ×1,4 | 0,722 | 0,731 | 2,96 | 2,90 |
| ×1,8 | 0,669 | 0,670 | 3,20 | 3,22 |
| ×2,2 | 0,622 | 0,619 | 3,50 | 3,51 |

Resposta total na faixa ×1,0 → ×2,2:

| Categoria | ρ(N,N+1) det / dither | perdido | desvio det / dither | perdido |
|---|---|---|---|---|
| rookie | −0,154 / −0,151 | **−2,0%** | +0,885 / +0,898 | **1,4%** |
| gt3 | −0,149 / −0,146 | **−2,1%** | +0,961 / +0,962 | **0,1%** |

Perda negativa significa que o determinístico respondeu *mais* — ou seja, o efeito está abaixo do
ruído de amostragem em T2. **O arredondamento não amortece a resposta da camada de evento.**

Mecanicamente é claro em retrospecto: as três camadas são somadas e arredondadas **uma vez**, com
magnitude típica de 4,4 pontos. Um erro de arredondamento de ±0,5 uniforme, de média zero, sobre
20 pilotos × 12 etapas × 30 temporadas, não enviesa a resposta a uma mudança de escala.

**Retratação, e ela reverte a ordem que eu mesmo pedi:** o item 3 (canal em `f64`) **não precisa
preceder a campanha**. O argumento que eu dei — "a fase 1 mede resposta amortecida e compensa
empurrando o parâmetro alto demais" — está medido como falso para a fase 1.

O caso do `f64` sobrevive, por um mecanismo diferente e que este espelho **não alcança**: os *outros
cinco* elos da esteira são arredondados **separadamente**, e um modificador cuja magnitude típica
fique abaixo de 0,5 ponto aplicado sobre uma base inteira é **anulado por completo, sempre** — não é
ruído de arredondamento, é perda total e determinística. É o caso do `MAX_PENALTY` do conhecimento de
pista e companhia. Medi-lo exige o delta por elo do item 5 do contrato, não este espelho.

Ordem revisada: injetabilidade → função pura → **campanha** → `f64` como pacote próprio, com o item 5
entregue junto para que a prova de equivalência consiga atribuir causa.
