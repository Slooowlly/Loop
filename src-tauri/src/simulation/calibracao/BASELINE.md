# Baseline de calibração da simulação

Medição feita **antes** de qualquer conserto no motor de corrida (pacotes B–G). É a referência
oficial contra a qual todo trabalho posterior é comparado.

Congelado em código, em [`snapshot.rs`](snapshot.rs) → `CONGELADO`. Não edite aquela constante
para fazer um teste passar: ela só muda quando alguém decide, conscientemente, que o novo
comportamento é o novo normal, e a linha do diff é a evidência dessa decisão.

## Como rodar

```bash
npm run build
```

```bash
cargo test --release --manifest-path src-tauri/Cargo.toml calibracao::tests::compara_com_congelado -- --ignored --nocapture
```

Os demais geradores (todos `#[ignore]`, todos com `--nocapture`):

| Teste | O que imprime |
|---|---|
| `imprime_baseline` | as métricas de resultado + o literal para recongelar |
| `compara_com_congelado` | **antes vs depois** — o comando a rodar depois de cada pacote |
| `imprime_decomposicao_de_variancia` | o orçamento piloto / carro / evento / corrida |
| `imprime_processo` | trocas, gaps, poder da largada |
| `imprime_varredura_de_knobs` | alavanca de cada knob existente |

Configuração dos números abaixo: 20 pilotos, 12 etapas, 84 temporadas = **1008 corridas por
linha**, grid fixo dentro da temporada, pistas variadas, 15% de etapas na chuva, sementes fixas
(2026 rookie / 2027 gt3). A mesma invocação sempre devolve os mesmos números — o diff acima fecha
em Δ = 0,000 em todas as 32 células.

Convenção: correlações e desvio de posição são medidos **só sobre quem terminou**; abandono entra
separado como `dnfs_por_etapa`.

> **Âncora**: `CONGELADO` foi medido em `d4c55e8`, com a árvore limpa. Rodar
> `compara_com_congelado` numa árvore suja mostra o efeito do trabalho ainda não commitado — é o
> uso pretendido, mas convém saber contra o que se está comparando. Na primeira execução com o
> pacote B em andamento o diff já acusou movimento na direção certa e de magnitude pequena:
> ρ(grid × chegada) −0,04 na rookie, desvio de posição +0,10 na gt3, e ρ(etapa N × N+1)
> **inalterado** — o sintoma central ainda não foi tocado.

---

# 1. Decomposição de variância — o orçamento

De onde vem a variação da posição de chegada. ANOVA de efeitos aleatórios cruzado sobre uma matriz
`piloto × evento × réplica`, com congelamento seletivo das fontes. 3840 corridas por categoria.

| Fonte | mazda_rookie | gt3 |
|---|---|---|
| **Piloto (permanente)** | **96,7%** | **48,9%** |
| **Equipe / carro (permanente)** | 0,2% | **49,6%** |
| Evento — pista | 0,3% | 0,1% |
| Evento — clima / temperatura | 1,2% | 0,7% |
| Corrida (ruído puro) | 1,6% | 0,7% |
| *(permanente total)* | *96,9%* | *98,5%* |

Aferições do método, ambas passando: a variância total medida (34,97 / 34,99 posição²) bate com a
teórica de 20 postos uniformes (33,25); e a categoria spec dá ~0% de carro, como tem que dar.

### As duas vias concordam

| | rookie | gt3 |
|---|---|---|
| Permanente via ANOVA | 0,969 | 0,985 |
| Permanente via ρ (chegadas de eventos diferentes) | 0,968 | 0,985 |
| **Divergência** | **0,001** | **0,000** |

A relação ρ² se confirma numericamente. As duas vias medem a mesma coisa por caminhos
independentes — uma decompõe soma de quadrados, a outra correlaciona resultados — e batem na
terceira casa. Isso valida o instrumento: quando o pacote B entregar afinidade, forma e acerto de
fim de semana, a leitura vai ser confiável.

### O achado: o orçamento não tem camada de evento

**97% a 98,5% da variação é permanente.** Evento (pista + clima somados) responde por 1,5% na
rookie e 0,8% na gt3. Ruído de corrida, por 1,6% e 0,7%.

Isso é o diagnóstico traduzido em números acionáveis: não existe "camada de fim de semana" na
simulação. Cada corrida é a mesma conta com um ruído mínimo por cima. É exatamente o buraco que o
pacote B (afinidade piloto×pista, forma AR(1), acerto por equipe/evento) vai preencher, e este
orçamento é a medida de quanto ele de fato preencheu.

Um alvo de referência para depois do B: numa categoria de entrada crível, evento + corrida
deveriam somar algo entre 30% e 55% — não 3,2%.

### Uma inversão que denuncia a classificação

Na rookie, ρ(grid × chegada) = **0,923** é MENOR que ρ(entre eventos) = **0,968**.

Isso não deveria acontecer no modelo: grid e chegada compartilham permanente + evento, então a
correlação do mesmo fim de semana tem que ser a maior das duas. Ela ser menor significa que **a
classificação é uma observação mais ruidosa do ritmo do que a própria corrida.** O
`qualifying_variance_multiplier` de 1,40 da rookie injeta mais ruído numa volta única do que os 5
segmentos da corrida injetam somados — e ali o ruído se cancela por média.

Consequência para o pacote F (quali e corrida carregando atributos diferentes): hoje a quali não é
"outra habilidade", é a mesma habilidade medida pior. Na gt3 a inversão não aparece (0,987 contra
0,985), porque lá a variância de quali é 0,80.

### Caveat metodológico

Piloto e carro são correlacionados no grid realista (bom piloto tende a bom carro), e nenhum
congelamento isolado separa a covariância — nivelar carros deixa a ordem quase igual, e nivelar
pilotos também. A razão piloto:carro acima é medida num grid de **encaixe independente**; as
demais fatias (evento, corrida, permanente total) vêm do grid realista e não sofrem desse
problema. O split 50/50 da gt3 é, portanto, "quanto o carro vale quando não está confundido com
quem dirige".

---

# 2. Métricas de processo

## Como a corrida acontece

| Métrica | mazda_rookie | gt3 |
|---|---|---|
| Trocas de posição (distância de Kendall) | 17,5 | 6,6 |
| **Trocas normalizadas** (0 = nenhuma, 0,5 = aleatório) | **0,09** | **0,03** |
| Posições ganhas/perdidas — média \|Δ\| | 1,46 | 0,61 |
| Posições ganhas/perdidas — p90 (a cauda) | 3,16 | 1,40 |
| **Maior ganho da corrida** | **4,11** | **1,99** |
| CV dos gaps entre carros consecutivos | 1,07 | 0,92 |
| Maior buraco / buraco mediano | 7,00 | 5,02 |
| Pelotões formados | 6,0 | 5,2 |

A leitura: **a melhor recuperação do dia, na média de 1008 corridas de rookie, é de 4 posições.**
Na gt3, de 2. O piloto médio termina 1,5 posição (rookie) ou 0,6 (gt3) longe de onde largou.

A distribuição de gaps é a única métrica de processo que não está obviamente quebrada — CV ~1,0 e
buracos de 5–7× a mediana não são uma escada regular. Mas com uma ressalva importante: sem ar sujo
não existe mecanismo que forme pelotão. O que o número está medindo é o espalhamento dos *scores*,
não carros andando juntos. Depois do pacote D esta métrica passa a significar outra coisa, e vale
remedir a partir do zero em vez de comparar com este valor.

## Poder da largada — a hipótese foi REFUTADA

O experimento: o mesmo evento, o mesmo grid de pilotos, rodado com a ordem de largada da
classificação e com a ordem **sorteada** (sem relação nenhuma com o ritmo). 800 corridas.

| Medida | mazda_rookie | gt3 |
|---|---|---|
| ρ(grid × chegada), grid da classificação | 0,937 | 0,987 |
| **ρ(grid × chegada), grid SORTEADO** | **0,153** | **0,260** |
| **ρ(skill × chegada), grid SORTEADO** | **0,885** | **0,772** |
| Trocas normalizadas com grid sorteado | 0,446 | 0,409 |

**A corrida não acaba na largada.** Largando em ordem aleatória, a posição inicial quase não
sobrevive (ρ = 0,15 / 0,26) e o ritmo reordena o pelotão inteiro (ρ = 0,89 / 0,77). As trocas
normalizadas saltam de 0,09 para 0,45 — praticamente o máximo de embaralhamento possível.

Os 2 pontos por posição de grid do `motor.rs:63` são quase irrelevantes. O que produz a ordem
congelada é o score determinístico, e a razão pela qual isso vira resultado idêntico toda etapa é
que **ultrapassar não custa nada**: um carro mais rápido atravessa o pelotão inteiro sem
resistência, todo fim de semana, na mesma ordem.

Isso redireciona o esforço com precisão: não é a largada que precisa de caos, é a ultrapassagem
que precisa de custo — o pacote D. E o teto de posições recuperáveis por corrida, hoje ilimitado,
é o parâmetro que aquele pacote vai ter que introduzir.

## Lacuna conhecida — o que não deu para medir

Duas métricas do briefing pedem posição por segmento: "trocas por segmento" e "em que segmento a
posição final estabiliza". **`RaceDriverResult` não guarda isso** — só `finish_position`. As
posições intermediárias existem em `RaceState::current_position` dentro de `race/motor.rs`, mas
são descartadas ao montar o resultado.

Bastaria um campo `posicoes_por_segmento: Vec<i32>` (5 entradas) em `RaceDriverResult`, preenchido
no laço que já calcula `state.current_position`. Está fora da fronteira deste pacote, então foi
reportado em vez de alterado. O experimento do grid sorteado responde a pergunta de fundo por
outro caminho, e argumentavelmente melhor: mede a consequência causal da posição inicial, não a
correlação temporal.

---

# 3. Knobs mortos vs knobs com alavanca

Cada knob varrido de 0 a 10 — 7× além de qualquer valor que alguma categoria use hoje.
Δρ(N,N+1) é a amplitude que a correlação entre etapas consecutivas percorre em TODA a faixa.

| Knob | Δρ rookie | veredito | Δρ gt3 | veredito |
|---|---|---|---|---|
| `race_variance_multiplier` | 0,130 | **ALAVANCA** | 0,046 | fraco |
| `pack_density_factor` | 0,089 | **ALAVANCA** | 0,005 | MORTO |
| `start_chaos_multiplier` | 0,050 | fraco | 0,009 | MORTO |
| `race_pace_spread_multiplier` | 0,046 | fraco | 0,008 | MORTO |
| `qualifying_variance_multiplier` | 0,030 | fraco | 0,044 | fraco |
| `incident_rate_multiplier` | 0,012 | fraco | 0,005 | fraco |
| `track_difficulty_multiplier` | 0,000 | **MORTO** | 0,000 | **MORTO** |
| `overtaking_difficulty_multiplier` | **0,000** | **MORTO** | **0,000** | **MORTO** |
| `rain_sensitivity` | **0,000** | **MORTO** | **0,000** | **MORTO** |

## A conclusão que decide o pacote E

> **CORREÇÃO (medida pela máquina de busca, seção 9):** a afirmação abaixo vale **por eixo** e não
> para o espaço inteiro. A combinação `race_variance = 10` × `pack_density = 10` atinge as oito
> métricas de resultado nas duas categorias. O que reprova esse ponto não é a distribuição — é o
> orçamento de variância: piloto desaba para 27,8% e corrida explode para 68,7%. Distribuição
> certa pelo motivo errado. A conclusão para o pacote E sobrevive, mas por um argumento mais
> forte: não é que os knobs não movam nada, é que só movem ruído.

**Nenhum knob existente, em nenhum valor, chega perto do alvo por eixo.** O mais forte de todos,
`race_variance_multiplier` na rookie, varrido de 0 a 10:

| valor | ρ(N,N+1) | desvio pos. | vencedores |
|---|---|---|---|
| 0,00 | 0,979 | 0,63 | 1,27 |
| 1,40 *(valor real hoje)* | 0,976 | 0,72 | 1,37 |
| 2,50 | 0,968 | 0,87 | 1,37 |
| 5,00 | 0,937 | 1,26 | 1,57 |
| **10,00** | **0,850** | **2,00** | **2,27** |
| *alvo* | *0,20–0,55* | *3,5–6,5* | *5–10* |

A 10× o valor de projeto — sete vezes o que qualquer categoria usa — ρ ainda está em 0,85 contra
um alvo de 0,55, o desvio em 2,0 contra 3,5, e o campeonato tem 2,3 vencedores contra 5.

E o caminho até lá é o errado: esses knobs são **ruído aditivo sobre um score determinístico**.
Aumentá-los não cria disputa, cria sorteio — a ordem deixa de ser previsível sem nunca passar a
ser disputada. Não existe valor de constante que produza corrida; o que falta é mecanismo.

Isso significa, concretamente, que **o pacote E (separar knobs por categoria) não deve ser feito
sobre os knobs atuais.** Separá-los por categoria distribuiria melhor uma alavanca que não existe.
O E só faz sentido depois de C e D, sobre os parâmetros novos que eles introduzirem — e aí esta
mesma varredura roda de novo, agora sobre knobs que têm o que mover.

## Os TRÊS mortos, por motivos diferentes

A guarda de [`consumo.rs`](consumo.rs) — que varre o fonte procurando quem lê cada campo do
contexto — **achou um segundo órfão na primeira execução**, não previsto no achado original:

- `overtaking_difficulty_multiplier` (Δρ = 0,000 exato): calculado pelo perfil e **nunca lido por
  lugar nenhum**. Morto por inexistência. É onde o pacote D vai encostar.
- `rain_sensitivity` (Δρ = 0,000 exato, mesmo com chuva em 100% das etapas): cadeia órfã inteira.
  `profile/resolucao.rs` calcula o valor, `context.rs` tem teste asseverando que chuva o eleva
  acima de 1,0 — e o único consumidor possível, `math::adjusted_weather_multiplier`, não é chamado
  por ninguém fora do próprio teste unitário. Quali e corrida usam
  `rain_skill_penalty(fator_chuva, rain_intensity_for(clima))` direto e ignoram a sensibilidade.
  **Na prática, a modulação de chuva por pista e por perfil de categoria não existe hoje, embora
  esteja configurada e testada.**
- `track_difficulty_multiplier` (Δρ = 0,000): este É lido por `pontuacao.rs`, mas o efeito é
  `adaptabilidade/100 × (mult−1) × 0,05` — décimos de ponto num score de 60–70. Morto por
  magnitude, não por inexistência. É o caso mais insidioso: parece conectado.

A guarda falha nos dois sentidos (órfão que passa a ser lido, consumido que para de ser lido) e a
mensagem diz o que fazer. Custa nada manter e já pagou uma vez.

## O detalhe do que já se sabia

- `track_difficulty_multiplier` (Δρ = 0,000): é LIDO por `pontuacao.rs`, mas o efeito é um bônus de
  `adaptabilidade/100 × (mult−1) × 0,05` — décimos de ponto num score de 60–70. Está morto por
  magnitude.
- `overtaking_difficulty_multiplier` (Δρ = 0,000 exato, em ambas as categorias): é calculado pelo
  perfil e **nunca lido por lugar nenhum**. Está morto por inexistência. O zero exato aqui é a
  demonstração mecânica de que o pacote D tem trabalho a fazer, não constante a ajustar — e serve
  de guarda do próprio harness (`knob_nao_lido_pela_simulacao_e_morto_por_construcao`).

---

# 4. Métricas de resultado

## mazda_rookie

| Métrica | sem incid. | com incid. | Alvo | Status |
|---|---|---|---|---|
| Spearman grid × chegada | 0,936 | 0,936 | 0,40–0,75 | ALTO |
| Vitórias do pole | 75,6% | 74,2% | 15%–35% | ALTO |
| Vencedores distintos / temporada | 1,30 | 1,43 | 5–10 | BAIXO |
| Desvio-padrão da posição de chegada | 0,71 | 0,89 | 3,5–6,5 | BAIXO |
| P(melhor do grid fora do top 5) | 1,3% | 2,9% | 15%–35% | BAIXO |
| Spearman etapa N × N+1 | 0,976 | 0,975 | 0,20–0,55 | ALTO |
| **Trocas de liderança no campeonato** | **0,23** | **0,46** | 2–7 | BAIXO |
| **Margem do campeão** | **26,5%** | **25,8%** | 2%–15% | ALTO |

Contexto: 88% (80% com incidentes) das temporadas terminam **sem nenhuma troca de liderança**.

## gt3

| Métrica | sem incid. | com incid. | Alvo | Status |
|---|---|---|---|---|
| Spearman grid × chegada | 0,986 | 0,985 | 0,60–0,88 | ALTO |
| Vitórias do pole | 88,8% | 88,2% | 30%–55% | ALTO |
| Vencedores distintos / temporada | 1,30 | 1,38 | 3–8 | BAIXO |
| Desvio-padrão da posição de chegada | 0,45 | 0,60 | 2,5–5,0 | BAIXO |
| P(melhor do grid fora do top 5) | 4,0% | 5,3% | 8%–25% | BAIXO |
| Spearman etapa N × N+1 | 0,989 | 0,989 | 0,35–0,70 | ALTO |
| **Trocas de liderança no campeonato** | **0,25** | **0,30** | 1–5 | BAIXO |
| **Margem do campeão** | **25,0%** | **24,8%** | 4%–20% | ALTO |

Contexto: 87% (82%) das temporadas sem nenhuma troca de liderança.

## As métricas de campeonato substitutas funcionaram

A etapa-de-decisão foi rebaixada a contexto: ela marca 85% em todos os quatro cenários, o que
parece saudável e não é — é aritmética da tabela de pontos (26 pela vitória, 18 pelo segundo: quem
vence tudo abre 8 por corrida, e 8k > (12−k)·26 só a partir de k = 10, ou seja 83%).

As duas substitutas não têm esse problema e denunciam na primeira leitura:

- **Trocas de liderança: 0,23 a 0,46** contra um alvo de 2–7. Em quase 9 de cada 10 temporadas o
  líder da primeira etapa é o campeão e nunca é ameaçado.
- **Margem do campeão: 25% dos pontos disponíveis** — cerca de 78 pontos, quase três vitórias de
  vantagem. Não é dinastia, é passeio.

Ambas continuam imunes ao problema da tabela de pontos: numa simulação travada a liderança troca
zero vez e a margem é gigante, independentemente de como os pontos são distribuídos.

---

# 5. Os alvos do pacote D — definidos antes de D existir

Estão em [`atrito.rs`](atrito.rs), cada faixa com o argumento ao lado. O resumo:

| Métrica | mazda_rookie | gt3 | hoje |
|---|---|---|---|
| ρ(grid × chegada), **grade sorteada** | 0,30–0,50 | 0,40–0,60 | 0,15 / 0,26 |
| ρ(skill × chegada), **grade sorteada** | 0,65–0,82 | 0,55–0,75 | 0,89 / 0,77 |
| ρ(fim do Start × chegada) | 0,40–0,75 | 0,50–0,82 | *pendente* |
| Segmento de estabilização (1–5) | 3–5 | 3–5 | *pendente* |
| Fração de células travadas atrás de mais lento | 0,08–0,22 | 0,20–0,40 | *pendente* |
| Maior comboio (segmentos consecutivos) | 1,5–3,0 | 2,5–5,0 | *pendente* |
| Recuperação máxima do dia | 5–10 | 4–8 | 4,1 / 2,0 |
| Fração do pelotão a < 1 s do da frente | 0,35–0,70 | 0,30–0,65 | derivável |
| CV dos gaps consecutivos | 1,20–2,50 | 1,40–3,00 | 1,07 / 0,92 |

## O raciocínio por categoria

**Monomarca de entrada.** Carros idênticos, pouca aerodinâmica, muito vácuo, corrida curta
(20 min, ~13 voltas). Passar É barato: sem asa, o carro de trás ganha no rebaixo em vez de perder
apoio, e 0,5 s/volta vira posição quase toda vez. Mas o tempo é curto — em ~13 voltas, quem larga
em 20º sendo o mais rápido do grid chega tipicamente entre 4º e 8º, não em 1º. Daí ρ(skill)
0,65–0,82 e ρ(grid) 0,30–0,50.

**Topo.** Aqui há uma **tensão que o briefing não menciona**, e é a razão de a separação entre as
duas categorias ser modesta em vez de dramática:

- Ar sujo empurra para MAIS persistência de grid: uma GT3 a 0,3 s/volta mais rápida frequentemente
  não converte nada, porque perde justamente onde precisaria se aproximar.
- Mas a corrida é **mais que o dobro** da rookie (45 min vs 20), e o delta de ritmo é maior — o
  carro responde por ~50% da variância permanente contra ~0% na spec. Mais tempo e mais delta
  empurram de volta para o ritmo.

Os dois efeitos se cancelam em boa parte. Por isso proponho um deslocamento de ~0,10, e não a
inversão que "GT3 é muito mais difícil de passar" sugeriria se olhasse só a aerodinâmica.

**Onde a gt3 DEVE se separar claramente é no trânsito, não na correlação final.** O comboio é o
fenômeno característico dela: `fracao_travado` 0,20–0,40 contra 0,08–0,22, e comboios de 2,5–5
segmentos contra 1,5–3. É ali que o ar sujo tem que aparecer.

## A armadilha número um do D

Os alvos pedem que a **recuperação máxima SUBA** (de 4,1 para 5–10 na rookie) enquanto a
ultrapassagem fica mais CARA. Parece contraditório e não é: hoje ninguém recupera porque ninguém
está fora de posição — o grid sai na ordem do ritmo e a corrida a confirma. Não há de onde
recuperar.

Depois do D, carros vão terminar fora de posição com frequência (preso em trem, largada ruim, ar
sujo na hora errada), e é isso que cria a recuperação. **Atrito e recuperação sobem juntos, porque
um cria a matéria-prima do outro.**

Sinal de alarme: se o D entrar e a recuperação máxima CAIR abaixo dos 4,1 / 2,0 de hoje, ele
adicionou atrito sem criar desalinhamento. Corrida mais travada, não mais disputada — o modo de
falha mais provável deste pacote.

## Dados pedidos ao D

Nenhum é derivável do que existe, e todos são subproduto natural de um modelo de posição na pista.
Pedir agora significa que D os emite por construção em vez de ser retrofitado — em
`RaceDriverResult`:

- `tentativas_ultrapassagem: i32` e `ultrapassagens_concluidas: i32`. A razão entre os dois é a
  taxa de sucesso, o parâmetro central do D: hoje ela é implicitamente 100%.
- `segmentos_em_ar_sujo: i32`. Sem isso, "fração de tempo em ar sujo" só dá para ser aproximada
  pelo retrato da bandeirada.

E `posicoes_por_segmento`, já pedido ao C. A matemática das quatro métricas de segmento está
escrita e testada contra entrada sintética em `atrito.rs`; falta uma linha no adaptador.

---

# 6. Remedição da classificação depois do pacote F

**Retratação parcial**: a inversão ρ(grid × chegada) < ρ(entre eventos) que reportei em `d4c55e8`
continua lá, e até aumentou — mas **o diagnóstico mudou de sinal**.

| | rookie | gt3 |
|---|---|---|
| ρ(grid × chegada) — antes / depois do F | 0,923 → **0,881** | 0,987 → **0,972** |
| Reprodutibilidade da CHEGADA entre eventos | 0,964 | 0,982 |
| **Reprodutibilidade do GRID entre eventos** | **0,875** | **0,949** |
| Razão grid/chegada | **0,908** | **0,966** |

A métrica nova que decide a questão é a **reprodutibilidade do grid**: ρ entre a ordem de largada
de dois eventos diferentes. Ela separa as duas explicações possíveis para um ρ(grid × chegada)
baixo, e nenhuma métrica de resultado consegue fazer isso.

- Se a quali virou **loteria**, o grid muda de evento para evento sem que nada no piloto mude, e a
  reprodutibilidade despenca.
- Se a quali virou **outro eixo estável**, ela continua reprodutível — o rápido de uma volta é
  sempre o mesmo, só não é o mais rápido de corrida.

Medido: **0,875 e 0,949**, razões de 0,91 e 0,97 contra a reprodutibilidade da chegada. É o
segundo caso, claramente. O pacote F fez o que se propôs: `ritmo_classificacao` dominando `skill`
e o trim de quali criaram um eixo próprio e estável, não ruído.

Em `d4c55e8` a interpretação "a quali é a mesma habilidade medida pior" estava certa — `skill`
dominava os dois lados. Depois do F ela está obsoleta, e a inversão passou a ser o comportamento
**esperado**: grid e chegada medem coisas diferentes, então não há razão para o ρ do mesmo fim de
semana ser o maior dos dois.

## Recomendação para `qualifying_variance_multiplier`: não mexer ainda

Hoje 1,40 (rookie) e 0,80 (topo). Recomendo **manter os dois**, por três razões:

1. **O problema não está aí.** A reprodutibilidade de 0,875/0,949 diz que a sessão não é ruidosa
   demais. O que sobra de não-reprodutível (12,5% e 5,1%) é a volta perdida e o melhor-de-N
   fazendo o trabalho deles.
2. **A alavanca é fraca.** A varredura mede Δρ de 0,030 (rookie) e 0,044 (gt3) em toda a faixa de
   0 a 10. Mexer num knob fraco antes de C e D landarem é precisamente o erro que este projeto
   existe para não repetir.
3. **O denominador vai mudar.** Depois do D, a persistência do grid passa a ser um mecanismo
   (ultrapassagem cara) em vez de uma coincidência (grid ≈ ritmo). O valor certo de variância de
   quali depende de quanto o grid vai passar a valer, e isso ainda não existe.

**Se** for para mexer depois do D, a direção defensável é **subir a da rookie** (1,40 → ~1,7),
não descer: 0,875 de reprodutibilidade ainda é um grid bastante previsível para uma categoria de
entrada com trânsito e volta única. O alvo que eu proporia para reprodutibilidade do grid é
**0,78–0,88 na rookie e 0,88–0,95 no topo** — a rookie está no teto da faixa, a gt3 está dentro.

Os outros três mecanismos do F (melhor-de-N, volta perdida, trim de quali) não podem ser avaliados
isoladamente com o instrumental atual, porque só o efeito somado deles aparece na
reprodutibilidade. Isolá-los exigiria varrer `ConfigQuali` campo a campo — que é exatamente o que
`ConfigQuali::legada()` permite, e é trabalho para quando a busca automática do item 3 existir.

---

# 7. A âncora contra o iRacing real

**Resultado da investigação: não há dado real no repositório.** Todos os testes de
`aiseason_results` e `result_bridge` usam JSON sintético montado com `serde_json::json!`. Nenhum
fixture, nenhum `.ibt`, nenhum resultado capturado.

O que existe e é bom: `iracing_sdk::aiseason_results::parse_event_result` já lê o formato oficial,
e `AiResultRow` carrega tudo que as métricas de resultado precisam — `position`,
`starting_position`, `reason_out`, `cust_id`, `display_name`.

## Por que a comparação aqui seria forte

Não é "distribuição do Loop vs distribuição genérica do iRacing". O `roster_gen::skill_curve_from`
mapeia o skill de cada piloto do Loop para o `driver_skill` do iRacing, então o grid exportado tem
**distribuição de habilidade conhecida e controlada por nós**. É um experimento de pares casados:
o mesmo grid, na mesma pista, corrido duas vezes — uma pela simulação interna, outra pelo iRacing.
Qualquer diferença é atribuível ao motor, não ao campo. É bem mais forte que comparar amostras
independentes.

## O ingestor existe

[`ancora.rs`](ancora.rs) reusa o parser deles (comparar contra um parser paralelo mediria a
diferença entre os dois parsers) e calcula as mesmas métricas do harness sobre dado real. Cinco
testes cobrem o caminho, incluindo um que monta um `aiseasons/<Season>.json` no formato REAL e
verifica ponta a ponta — ele já pagou: pegou meu próprio erro de assumir camelCase quando o JSON
do iRacing usa snake_case.

Assim que alguém soltar arquivos numa pasta:

```bash
cargo test --release --manifest-path src-tauri/Cargo.toml calibracao::ancora -- --nocapture
```

## O pedido de coleta

Está completo em `ancora::PROTOCOLO_DE_COLETA`. O resumo:

- **Formato**: `Documentos/iRacing/aiseasons/<Season>.json`, o arquivo inteiro. Já existe na
  máquina de quem joga; não precisa instrumentar nada.
- **Volume**: 12 corridas do mesmo grid (uma temporada) é o ideal; 8 dão erro-padrão de ~0,25
  posição no desvio, o suficiente para distinguir 0,71 de 3,5. Menos de 5 não serve.
- **Categorias**: `mazda_rookie` e uma de topo. Se só der uma, a rookie — carro spec isola o
  piloto.
- **Validade**: mesma temporada, mesmo roster, sem troca de piloto no meio.
- **Anotar junto**: o `driver_skill` exportado, senão a comparação perde a propriedade de pares
  casados.

**O que NÃO é ancorável**: o resultado oficial não traz posição por segmento, gaps intermediários,
tentativas de ultrapassagem nem tempo em ar sujo. As métricas de PROCESSO e as do pacote D
continuam sem padrão-ouro — só as de RESULTADO são ancoráveis. Vale saber disso antes de coletar.

## Lacuna de infraestrutura encontrada

`race_results` **não tem coluna marcando a origem do resultado**. Uma corrida que veio do iRacing
pela `result_bridge` e uma que a simulação produziu ficam indistinguíveis no save. Isso importa
porque o save de jogador é a fonte mais provável de dado real em volume — e hoje o harness não
conseguiria separar as linhas reais das simuladas nele.

Uma coluna `origem TEXT NOT NULL DEFAULT 'simulada'` (`'simulada'` | `'iracing'`), preenchida em
`commands::iracing::resultado` no caminho que já grava o oficial, resolveria. É uma migração de
uma linha. Fora da fronteira — reportado, não alterado.

---

# 8. Os alvos do pacote G — e a repartição final do orçamento

## A repartição final: a decisão de design mais importante que sobrou

Está em [`variancia.rs`](variancia.rs) → `OrcamentoAlvo`, executável e coberta por testes.

| Fonte | hoje rookie | **alvo rookie** | hoje gt3 | **alvo gt3** |
|---|---|---|---|---|
| Piloto (permanente) | 96,7% | **38–52%** | 48,9% | **22–35%** |
| Equipe / carro (permanente) | 0,2% | **0–5%** | 49,6% | **22–38%** |
| Evento (pista, clima, forma, acerto, **estratégia**) | 1,5% | **20–32%** | 0,8% | **22–34%** |
| Corrida (incidente, trânsito, azar) | 1,6% | **22–35%** | 0,7% | **12–24%** |
| *permanente total* | *96,9%* | *38–57%* | *98,5%* | *44–73%* |

**A âncora empírica** para o permanente é a correlação entre chegadas de corridas consecutivas em
séries reais: numa monomarca competitiva ela fica em 0,40–0,60, e essa correlação **é** a fração
permanente (é a mesma identidade ρ² que sustenta a segunda via da decomposição). Hoje o Loop mede
0,97. É daí que sai o alvo de 38–57%, não de intuição.

Três decisões embutidas que valem ser contestadas explicitamente:

1. **O topo concentra mais permanente que a entrada** (44–73% contra 38–57%). É design declarado —
   `car_weight_scale("gt3") == 1.30` existe para que dinastias aconteçam. Uma GT3 tão aleatória
   quanto uma monomarca de entrada seria errada.
2. **A entrada tem MAIS ruído de corrida** (22–35% contra 12–24%). Pelotão inexperiente erra mais.
   É a mesma assimetria que os perfis de categoria já tentam expressar e que hoje não chega ao
   resultado.
3. **Teto de azar**: 25% na entrada, 20% no topo. Não é uma fonte separada no ANOVA — atravessa
   `evento` e `corrida` —, é um limite de design. Variância não atribuível a qualidade
   (safety car na hora errada, batida de terceiro) é o que dá história ao campeonato, mas acima de
   um quarto do total o jogador para de sentir agência, e a carreira vira loteria. A métrica que
   vigia isso é `p_melhor_fora_top5`: se ela estourar 35% na entrada ou 25% no topo, o azar passou
   do ponto mesmo que todas as outras métricas estejam bonitas.

Quanto do `evento` deve ser estratégia? **Na entrada, quase nada de pit stop** — uma corrida de
20 minutos não tem parada — mas o safety car vale por si. Na gt3 (45 min, parada obrigatória) a
estratégia deveria ser a maior fatia isolada de `evento`. Proposta: da fatia de evento, estratégia
+ SC ocupam **~25% na entrada** (praticamente só SC) e **~45% no topo**.

## As métricas do G

Nenhuma é derivável hoje. Como no D, especificar antes é o que faz o G emitir por construção:

| Métrica | entrada | topo | por quê |
|---|---|---|---|
| Safety cars por corrida | 0,25–0,60 | 0,15–0,40 | rookie bate mais; SC é consequência de incidente |
| ρ(ordem pré-SC × chegada) nas corridas COM SC | 0,45–0,75 | 0,55–0,80 | mede o quanto o SC embaralha de fato |
| Δ vencedores distintos: corridas com SC vs sem | ≥ +1,0 | ≥ +0,8 | se o SC não muda quem ganha, ele é decoração |
| Estratégias distintas usadas no grid | 1–2 | 2–4 | rookie sem parada; gt3 tem janela real |
| Posições ganhas/perdidas atribuíveis à janela de parada | 0–1 | 2–5 | é o undercut existindo |
| Fração do grid "crucificada" (perde ≥4 posições só por timing de parada) | 0–0,05 | 0,08–0,20 | o fenômeno narrativo que o G existe para criar |

A terceira linha é a mais importante e a mais fácil de errar: **um safety car que não muda quem
ganha não é um safety car, é uma animação.** Ele tem que produzir vencedores que não venceriam.

A última é a que o briefing nomeou bem: "não é 'o dado deu ruim', é 'ele foi crucificado pelo
safety car'". Ela tem teto porque crucificação demais é a mesma loteria de antes com narrativa
melhor.

## Dados pedidos ao G

Em `RaceDriverResult`, dado cru — sem calcular métrica:

- `volta_da_parada: Vec<u32>` — as voltas em que parou (vazio = não parou).
- `posicao_antes_da_parada: Vec<i32>` e `posicao_depois: Vec<i32>` — o par que torna o undercut
  mensurável.
- `estrategia_id: String` — rótulo da estratégia escolhida (ex.: `"1-stop-cedo"`), para contar
  estratégias distintas sem inferir.

Em `RaceResult`:

- `safety_cars: Vec<u32>` — voltas de entrada de cada SC.
- `ordem_pre_safety_car: Vec<Vec<String>>` — a classificação no momento em que cada SC entrou. É o
  que permite medir o embaralhamento sem reconstruir a corrida.

## A armadilha do G

Simétrica à do D, e vale escrever antes: **o safety car não pode ser a fonte principal de trocas
de liderança.** É tentador — é o mecanismo mais barato de embaralhar e o mais legível. Mas se as
trocas de liderança do campeonato saltarem para dentro da faixa 2–7 principalmente porque o SC
sorteou, o campeonato ficou aleatório em vez de disputado, e as sete métricas de resultado não
distinguem os dois casos.

**Quem distingue é a decomposição.** Se depois do G a fatia de `corrida` estourar o teto de azar
enquanto `piloto` desaba, o G resolveu a métrica destruindo o design. Rodar a decomposição no
ponto final é obrigatório, não opcional — está na regra 6 de [CAMPANHA.md](CAMPANHA.md).

---

# Ordem de ataque sugerida

O orçamento e a varredura, lidos juntos, dão uma ordem que não é a intuitiva:

1. **D antes de C, ou junto.** O experimento do grid sorteado mostra que ultrapassar é grátis. Sem
   custo de ultrapassagem, qualquer variância adicionada em C só embaralha mais rápido — o carro
   rápido continua atravessando o pelotão inteiro, só que numa ordem diferente.
2. **B preenche o buraco de evento.** 1,5% de camada de evento é o número mais anômalo do
   orçamento. Afinidade, forma e acerto de fim de semana atacam exatamente isso, e a decomposição
   mede quanto entrou.
3. ~~**F tem um alvo concreto já identificado**~~ — **feito e remedido** (seção 6). O F entregou:
   a classificação virou eixo próprio e estável, e a recomendação é não mexer na variância dela
   ainda.
4. **E por último, e não sobre os knobs atuais.** Nenhum deles tem alavanca suficiente; separá-los
   por categoria distribuiria melhor um efeito que não existe.

# 9. A máquina de busca, rodada sobre o espaço morto

Construída antes do D e apontada para o espaço atual, cujo fracasso era garantido e conhecido — o
banco de testes ideal para o caminho de fracasso. Código em [`busca.rs`](busca.rs), plano em
[CAMPANHA.md](CAMPANHA.md).

**Ela falha, e falha bem** — mas o motivo não é o que se supunha.

## O que ela achou

Com a peneira corrigida, a descida coordenada acha um ponto que coloca **as oito métricas de
resultado dentro da faixa**, nas duas categorias:

| Métrica | rookie no ótimo | alvo | gt3 no ótimo | alvo |
|---|---|---|---|---|
| Spearman etapa N × N+1 | 0,350 | 0,20–0,55 | 0,581 | 0,35–0,70 |
| Spearman grid × chegada | 0,554 | 0,40–0,75 | 0,747 | 0,60–0,88 |
| Desvio-padrão da posição | 4,49 | 3,5–6,5 | 3,47 | 2,5–5,0 |
| Vencedores distintos | 6,05 | 5–10 | 4,51 | 3–8 |
| Vitórias do pole | 24,9% | 15–35% | 37,9% | 30–55% |
| P(melhor fora do top 5) | 30,8% | 15–35% | 18,9% | 8–25% |
| Trocas de liderança | 2,57 | 2–7 | 1,77 | 1–5 |
| Margem do campeão | 11,6% | 2–15% | 12,6% | 4–20% |

O ponto: `race_variance_multiplier = 10`, `pack_density_factor = 10`. Sete vezes o valor de
projeto, e **os dois na borda da faixa varrida**.

## Por que ela reprova assim mesmo

O portão do orçamento de variância, que a implementação promoveu de recomendação a veredito
executável:

| Fonte | rookie no ponto | alvo | gt3 no ponto | alvo |
|---|---|---|---|---|
| Piloto | **27,8%** | 38–52% | — | 22–35% |
| Evento | **3,0%** | 20–32% | **0,2%** | 22–34% |
| Corrida (ruído) | **68,7%** | 22–35% | **40,7%** | 12–24% |

O campeonato não ficou disputado — ficou **sorteado**. Dois terços da variação da posição de
chegada viraram ruído puro, e o piloto encolheu para menos de um terço. As oito métricas de
resultado não distinguem esses dois mundos: dispersão alta e vencedores variados saem tanto de
disputa quanto de loteria.

Isso é a demonstração mais forte possível de por que a decomposição de variância tem que ser
portão obrigatório e não recomendação. Sem ela, esta busca teria devolvido "convergiu, use
variance=10" — e alguém teria calibrado o jogo para ser uma roleta.

Três sinais independentes marcaram o ponto como suspeito antes mesmo do orçamento, e todos os três
estão no relatório: **ótimo na borda** em dois eixos, **partidas divergentes** (duas descidas de
pontos iniciais distantes pararam em lugares diferentes → vales múltiplos), e o próprio orçamento.

## Os dois defeitos que a implementação corrigiu

A primeira versão da busca **reportou sucesso**, e os dois erros valem registro porque são
genéricos:

1. **O veredito estava ligado ao "melhor de qualquer ponto".** Resultado: seis `ATINGIDO` vindos de
   pontos diferentes e mutuamente incompatíveis, somados num ar de sucesso. Corrigido cruzando as
   duas leituras, e daí saiu a distinção `CONFLITO` (atingível sozinha, impossível junto) vs
   `INALCANÇÁVEL` (mecanismo não existe) — dois defeitos com dois consertos diferentes.
2. **A peneira T1 tinha corte absoluto** ("nenhuma métrica fora por mais de 3×"), que descarta o
   eixo inteiro quando todo o espaço está longe do alvo — a situação inicial de qualquer
   calibração real. A busca não saía do ponto de partida. Corrigido para corte relativo: promove a
   metade melhor da própria varredura do eixo.

## As duas perguntas do briefing

**O T1 de 24 corridas tem sinal suficiente?** Não — sinal/ruído de **1,2**. Inutilizável; o ruído
(3,71) é quase do tamanho de todo o intervalo que o objetivo percorre num eixo (4,54).

O diagnóstico veio de um número inesperado: o ruído mal cai de T2 (1,89) para T3 (1,83), apesar de
quase 3× mais corridas. Se fosse ruído de amostragem teria caído com √3. **A incerteza dominante
não é quantas corridas rodaram — é quais grids foram sorteados.** Daí a correção: para baratear um
nível, corte etapas por temporada, não temporadas. T1 virou 12 × 6 = 72 corridas, sinal/ruído
**6,5**, e concordância de ordenação com T2 de **ρ = 0,929** — a peneira preserva o que o nível
caro prefere. Travado em `triagem_t1_preserva_a_ordem_do_nivel_caro`.

Consequência operacional: **comparar dois pontos exige a mesma semente**, porque grids diferentes
não são comparáveis.

**A função-objetivo tem gradiente com tudo fora da faixa?** Tem — a distância é linear na
distância, não um degrau, então não é cega. Mas há um buraco mais sutil que a preocupação original
não nomeou, e ele é real: **métricas limitadas saturam perto do limite.** Com ρ = 0,976 e alvo
0,55, cada passo de parâmetro produz um passo de métrica cada vez menor, e o gradiente afunda
abaixo do ruído justamente onde a busca começa.

Mudei o desenho: a distância é medida em escala transformada — `atanh` para correlações, `logit`
para frações, linear para contagens. ρ 0,976 → 0,850 rende um passo de objetivo 1,5× maior em
Fisher do que na escala crua, e a borda da faixa continua sendo a borda. Sem isso, a busca sobre o
espaço atual seria de fato caminhada aleatória.

---

# 10. Medição da árvore reformada — PROVISÓRIA

> **Não congelada e não congelável.** Estes números vêm da árvore de trabalho, com C, D e G
> presentes e o G a meio caminho, sem commit. Servem de leitura, não de âncora. O recongelamento
> acontece depois do commit da reforma.

## Primeiro: a régua estava errada, e o erro tinha direção

Duas falhas de instrumento no [`campo.rs`](campo.rs), consertadas antes de qualquer medição valer:

**1. A repartição dos atributos não seguia a do jogo.** `models/driver_generation.rs` separa os
atributos em correlacionados com o skill (`consistencia` ±10, `racecraft` ±8, `defesa` ±8,
`ritmo_classificacao` ±12) e sorteados em absoluto (`gestao_pneus`, `habilidade_largada`,
`adaptabilidade`, `mentalidade` 40–70; `fator_chuva`, `aggression` 30–70; `confianca` 50–70), mais
`smoothness` = 100 − `aggression` ±10. **Sete atributos saíam do talento aqui quando no jogo são
livres** — e eram exatamente os eixos que D e G fortaleceram.

E um erro na direção oposta, que o relato não previu: a `consistencia`, que o jogo AMARRA ao skill,
estava solta aqui. O teste `campo_nao_e_uma_escada_perfeita` exigia ρ < 0,75 — ele estava
asseverando o bug.

**2. O gt3 espalhava skill 38% mais que o jogo** (dp 6,1 contra 4,4). O espalhamento do talento
multiplica toda vantagem determinística, então um campo largo mede determinismo que não existe.
Alinhado por medição contra o gerador real:

| categoria | jogo (média/dp) | harness antes | harness agora | razão dp |
|---|---|---|---|---|
| mazda_rookie | 45,1 / 12,1 | 52,0 / 11,6 | 47,2 / 11,9 | 0,99 |
| gt3 | 81,1 / 4,4 | 80,9 / **6,1** | 81,7 / 4,5 | 1,02 |

O guard contra deriva é `reparticao_espelha_a_geracao_do_jogo`, que compara contra
`Driver::generate_for_category` — a fonte, não constantes copiadas. Mais
`espalhamento_do_skill_acompanha_o_jogo`, `atributos_livres_no_jogo_nao_saem_do_talento` e
`smoothness_e_o_inverso_da_agressividade`.

Ressalva: o alinhamento casa **média e desvio**, não a forma. O jogo sorteia skill quase uniforme
numa faixa; o harness usa normal com cauda. É erro de segunda ordem, e fica registrado como
aproximação conhecida em vez de fidelidade alegada.

## O efeito da correção: a régua mostrava MENOS mecanismo do que existe

A hipótese estava certa, e a direção também.

| Métrica (rookie, sem incidentes) | congelado (pré-projeto) | régua errada | **régua corrigida** | alvo |
|---|---|---|---|---|
| ρ(etapa N × N+1) | 0,976 | 0,885 | **0,83** | 0,20–0,55 |
| Desvio da posição | 0,71 | 1,72 | **2,19** | 3,5–6,5 |
| Vencedores distintos | 1,30 | ~1,8 | **2,13** | 5–10 |
| Trocas de liderança | 0,23 | — | **0,93** | 2–7 |
| Maior ganho da corrida | 4,11 | 4,71 | **5,62** | 5–10 ✓ |

Em gt3: desvio 0,45 → 1,48, ρ 0,989 → 0,92, maior ganho 1,99 → **3,17**.

**A recuperação máxima na rookie entrou no alvo** (5,62 contra 5–10). Ela não caiu em nenhum
momento — subiu nas duas medições, e a correção da régua a levou para dentro da faixa. A armadilha
nº 1 do pacote D segue sem disparar, agora com margem.

**E o sintoma central sobreviveu**, como a hipótese previa: ρ em 0,83 e 0,92 contra alvos de 0,55 e
0,70; desvio em 2,19 e 1,48 contra 3,5 e 2,5. Ele depende de skill e carro, que não eram os
atributos mal gerados. Parte do buraco era instrumento; a maior parte não era.

## A sonda de grade sorteada: quatro de quatro, confirmado com a régua certa

| Medida | pré-D | régua errada | **régua corrigida** | alvo |
|---|---|---|---|---|
| ρ(grid × chegada) sorteado — rookie | 0,153 | 0,329 | **0,436** | 0,30–0,50 ✓ |
| ρ(skill × chegada) sorteado — rookie | 0,885 | 0,733 | **0,677** | 0,65–0,82 ✓ |
| ρ(grid × chegada) sorteado — gt3 | 0,260 | 0,422 | **0,545** | 0,40–0,60 ✓ |
| ρ(skill × chegada) sorteado — gt3 | 0,772 | 0,613 | **0,642** | 0,55–0,75 ✓ |

O pacote D acertou os quatro alvos que eu especifiquei antes de ele existir, e o acerto sobrevive à
correção da régua — inclusive melhora.

## Os números abaixo são da régua ANTERIOR à correção

Ficam registrados porque a alavanca do `overtaking_difficulty` e a taxonomia dos mortos não
dependem da geração de atributos. Onde a régua importa, o valor corrigido está na seção acima.

## Os dois órfãos foram conectados — o catálogo dos mortos virou um

A guarda de [`consumo.rs`](consumo.rs) disparou sozinha e nos dois casos:

- `overtaking_difficulty_multiplier` → lido em `race/motor.rs` e `race/trafego.rs`. O pacote D
  ligou o fio. Era o órfão original, o que media alavanca **0,000 exata**.
- `rain_sensitivity` → lido em `qualifying.rs` e `race/pontuacao.rs`. Ligado pelo pacote G.

Sobra **um** morto: `track_difficulty_multiplier`, Δρ = 0,0036 na rookie e 0,0004 na gt3. É lido
por `pontuacao.rs` — a guarda de fonte não tem o que reclamar dele —, mas o efeito é
`adaptabilidade/100 × (mult−1) × 0,05`: décimos de ponto num score de 60–70. Morte por MAGNITUDE.

O teste que asseverava a morte por inexistência foi aposentado (cumpriu o papel: falhou no momento
da ligação) e substituído por dois: um que guarda o morto por magnitude, e a **guarda inversa** —
se a ligação do D ou do G se desfizer numa refatoração, a alavanca volta a zero e o fonte continua
parecendo conectado.

## A alavanca real do `overtaking_difficulty`

Era 0,000 por inexistência. Agora, na rookie:

| valor | ρ(N,N+1) | desvio pos. | vencedores |
|---|---|---|---|
| 0,00 | 0,955 | 1,01 | 1,43 |
| 0,50 | 0,946 | 1,16 | 1,63 |
| **1,00** | **0,905** | **1,57** | **1,80** |
| 1,40 | 0,885 | 1,72 | 1,80 |
| 2,50 | 0,872 | 1,87 | 2,00 |
| 10,00 | 0,849 | 2,04 | 2,23 |

Δρ = 0,108 → **ALAVANCA** na rookie; 0,035 (fraco) na gt3.

Duas leituras acionáveis:

1. **Ele satura por volta de 2,5.** De 0 a 1,0 vem metade do efeito total; de 2,5 a 10 vem quase
   nada. A faixa de varredura dele na campanha deve ser **0–3, não 0–10** — varrer até 10 gasta
   orçamento medindo platô. Os valores de perfil (~1,0–1,4) estão na região responsiva, mas
   perto do joelho.
2. **A direção é a certa, e não é a óbvia.** Mais dificuldade de ultrapassagem produz ρ MENOR e
   desvio MAIOR. Parece invertido — passar mais difícil deveria travar o resultado — e não é: o
   carro rápido fica preso no trânsito, e *quem* ele encontra depende de onde largou. Atrito gera
   variação de evento. É exatamente o mecanismo que o D existia para criar.

## O D acertou os quatro alvos da sonda de grade sorteada

O número que eu especifiquei antes de o D existir:

| Medida | congelado (pré-D) | agora | alvo | |
|---|---|---|---|---|
| ρ(grid × chegada) sorteado — rookie | 0,153 | **0,329** | 0,30–0,50 | ok |
| ρ(skill × chegada) sorteado — rookie | 0,885 | **0,733** | 0,65–0,82 | ok |
| ρ(grid × chegada) sorteado — gt3 | 0,260 | **0,422** | 0,40–0,60 | ok |
| ρ(skill × chegada) sorteado — gt3 | 0,772 | **0,613** | 0,55–0,75 | ok |

Quatro de quatro. As trocas normalizadas com grid sorteado caíram de 0,446 para 0,377 (rookie) e
de 0,409 para 0,343 (gt3): ultrapassar ficou mais caro, medido.

## A armadilha da recuperação: o alarme NÃO disparou

| | congelado | agora | alvo |
|---|---|---|---|
| Recuperação máxima — rookie | 4,11 | **4,71** | 5–10 |
| Recuperação máxima — gt3 | 1,99 | **2,23** | 4–8 |

**Atrito e recuperação subiram juntos**, que era a previsão. O modo de falha mais provável do D —
adicionar atrito sem criar desalinhamento, o que faria a recuperação CAIR — não aconteceu.

Mas as duas seguem abaixo do alvo, e por um motivo coerente: falta **desalinhamento**. O D criou o
custo de passar; quem cria carro fora de posição em volume é o G (safety car, janela de parada).
Esta é a métrica a reler quando o G fechar.

CV dos gaps: 1,07 → **1,24** na rookie (alvo 1,20–2,50, ok) e 0,92 → **1,04** na gt3 (alvo
1,40–3,00, ainda baixo). Maior buraco sobre o mediano foi de 7,0 a 10,0 na rookie — os pelotões
começaram a se formar de verdade.

## Segunda retratação sobre o método de triagem

O achado do A4 que eu chamei de "o que mais generaliza" **não generaliza**. Remedido na árvore
reformada, o perfil de ruído inverteu:

| Nível | corridas | ruído pré-reforma | ruído pós-reforma |
|---|---|---|---|
| 72 corridas | 72 | 0,60 | 2,76 |
| T2 (360) | 360 | 1,89 | 0,80 |
| T3 (1008) | 1008 | 1,83 | 0,55 |

Agora o ruído cai com o volume, como ruído de amostragem deve cair. O comportamento anterior era
**artefato de uma simulação quase determinística**: sem variância por corrida, a única variação que
sobrava era estrutural (grid a grid), e essa não some com mais corridas. O que sobrevive é a regra
operacional — comparar dois pontos exige a MESMA semente — e uma lição nova: **a forma dos níveis
tem que ser remedida a cada mudança grande do motor.** Por isso `Nivel` virou struct, não constante.

E uma segunda correção, mais desconfortável: **a peneira nunca esteve quebrada — a guarda estava.**
Eu media concordância de ordenação (Spearman T1 × T2), e ela cai a zero num eixo PLANO por
construção, qualquer que seja a qualidade das duas medições. O culpado do 0,71 era um eixo só,
`race_pace_spread`, com ρ de 0,29 — ali o objetivo não responde, e as duas medições estavam
ordenando ruído.

A quantidade certa é **arrependimento**: quanto a peneira custa em objetivo por jogar fora o ponto
que o nível caro escolheria, normalizado pela amplitude do eixo. Medido:

| forma de T1 | corridas | ρ × T2 | **arrependimento** |
|---|---|---|---|
| 12 × 6 | 72 | 0,693 | 0,050 |
| 12 × 12 | 144 | 0,670 | 0,082 |
| **15 × 10** | **150** | 0,696 | **0,000** |
| 20 × 8 | 160 | 0,807 | 0,000 |

Pelo arrependimento, até o 12 × 6 original era aceitável. Fiquei em 15 × 10 por margem — 0,05 se
compõe ao longo de duas passadas da descida — e T1 segue 2,4× mais barato que T2. Num eixo plano o
arrependimento é zero automaticamente, que é o comportamento correto e é o que a guarda antiga não
tinha.

---

# 11. Safety car: a matriz knob × saída e o diagnóstico do gatilho

## "Morto" era específico da saída medida — a crítica procedia

A varredura media Δρ e desvio de posição, duas saídas de *resultado*, e chamava
`incident_rate_multiplier` de fraco. Com frequência de safety car medida como saída própria
([`seguranca.rs`](seguranca.rs) + `Saida` em [`varredura.rs`](varredura.rs)), a matriz na rookie:

| Knob | ρ(N,N+1) | desvio | vencedores | **SC/etapa** | ρ(pré-SC) | DNF/etapa |
|---|---|---|---|---|---|---|
| `race_variance_multiplier` | **0,450** | **2,16** | **3,43** | 0,028 | **0,258** | 0,075 |
| `start_chaos_multiplier` | **0,232** | **1,18** | **1,97** | **0,242** | **0,244** | **1,14** |
| `qualifying_variance_multiplier` | **0,355** | **1,91** | **2,70** | 0,022 | **0,280** | 0,119 |
| `pack_density_factor` | **0,445** | **1,78** | **3,47** | **0,728** | **0,258** | **2,37** |
| `incident_rate_multiplier` | 0,022 | 0,051 | 0,733 | **0,558** | 0,026 | **4,10** |
| `overtaking_difficulty_multiplier` | **0,150** | 0,927 | 0,700 | 0,022 | 0,084 | 0,092 |
| `track_difficulty_multiplier` | 0,004 | 0,017 | 0,100 | 0,006 | 0,007 | 0,017 |
| `rain_sensitivity` | 0,038 | 0,230 | 0,267 | 0,011 | 0,021 | 0,047 |

**Você estava certo:** `incident_rate_multiplier` é **ALAVANCA** em SC/etapa (0,558) e em DNF/etapa
(4,10), e fraco em ρ. O veredito consolidado dele mudou de "fraco" para "ALAVANCA" — um knob só é
morto quando é morto em TODAS as colunas.

E um achado que não estava previsto: **`pack_density_factor` é a alavanca MAIS forte na frequência
de SC (0,728), mais que o próprio `incident_rate`.** Faz sentido físico — pelotão compacto é
contato — e dá um terceiro caminho para a frequência que não passa por severidade nem por gatilho.

**`track_difficulty_multiplier` é o único que sobrevive ao teste por saída: morto nas seis
colunas** (máximo 0,017). O argumento de mandá-lo para a campanha como coeficiente a redimensionar,
em vez de pacote próprio, fica reforçado por medição em vez de por opinião.

## O diagnóstico do gatilho: são os dois, em proporções diferentes por categoria

O gatilho atual (`race/estrategia.rs::traz_bandeira_amarela`) tem três linhas, e **duas exigem
DNF**. Medindo os incidentes com tipo, severidade e DNF sobre 1008 corridas:

| | mazda_rookie | gt3 |
|---|---|---|
| Incidentes por corrida | 3,710 | 2,632 |
| Incidentes **graves** por corrida | 0,242 | 0,093 |
| Qualificam pelo gatilho **atual** | 0,104 | 0,045 |
| Qualificam pelo gatilho **alargado** (sem `is_dnf`) | 0,242 | 0,093 |
| Safety cars por corrida | 0,079 | 0,033 |
| Aproveitamento do gatilho | 43% | 48% |
| **Ganho de alargar** | **2,3×** | **2,1×** |
| Conversão (SC / qualificam) | 0,76 | 0,73 |
| **Projeção alargando** | **0,184** | **0,068** |
| Alvo | 0,25–0,60 | 0,15–0,40 |
| **Fator de gravidade ainda faltante** | **1,4×** | **2,2×** |

**Rookie: gatilho estreito em primeiro lugar.** Alargar leva de 0,079 a ~0,184 — 74% do caminho até
o piso do alvo, de graça, numa linha. Sobra um ajuste modesto de gravidade (1,4×).

**GT3: os dois, com a gravidade dominando.** Alargar leva de 0,033 a ~0,068 e ainda fica 2,2× abaixo
do piso, porque só existem 0,093 incidentes graves por corrida. Alargar o predicado não cria
gravidade que não existe.

**Ordem recomendada**: alargar primeiro (é grátis e não é calibração), remedir, e só então subir
severidade ou taxa pelo fator que sobrar. E `pack_density_factor` é o candidato mais barato para
esse fator — tem a maior alavanca em SC/etapa e não mexe na tabela de severidade.

Nota metodológica: a primeira versão deste veredito era **categórica** (um corte em "0,10 graves por
corrida") e classificava rookie como "gatilho estreito" e gt3 como "faltam batidas grandes". Faca no
fio: gt3 media 0,093 contra o corte de 0,10. As duas causas contribuem nas duas categorias, e um
veredito binário escondia isso. Trocado por projeção quantitativa.

Nota sobre o experimento: a varredura passou a rodar com **incidentes ligados**. Com eles
desligados, SC/etapa seria 0 constante em todo knob e a coluna inteira sairia "morta" por
construção do experimento em vez de por propriedade do knob.

---

# Ainda não feito

- **A coleta do dado real.** O ingestor e o protocolo estão prontos (seção 7); falta alguém rodar
  uma temporada no iRacing e copiar o `.json`. É o único item da lista que não depende de código.
- **O gatilho de SC alargado** — `traz_bandeira_amarela` sem `is_dnf` nas duas linhas que o exigem. Fora da fronteira; medido e reportado (seção 11).
- **Rodar a busca sobre o espaço NOVO**, depois de C e D. A máquina existe e está validada contra
  um espaço de fracasso conhecido (seção 9); é a primeira coisa a rodar quando o D fechar.
- **Ligar as métricas de segmento** (`atrito.rs`) quando `posicoes_por_segmento` chegar. Um teste
  falha de propósito nesse momento, como lembrete.

# O que a régua cobra

Três testes `#[ignore]` **falham hoje de propósito** — são o critério de aceitação:

- `rookie_distribui_como_corrida_de_verdade`
- `gt3_distribui_como_corrida_de_verdade`
- `rookie_e_mais_caotica_que_o_topo`

Um teste leve (`nao_regride_para_determinismo_absoluto`) roda sempre e garante só que a coisa não
piora enquanto o conserto acontece. Os outros 26 testes leves cobrem o harness: a estatística, o
gerador, o congelamento seletivo, o embaralhamento de grid, a chegada dos knobs ao contexto.

```bash
cargo test --release --manifest-path src-tauri/Cargo.toml calibracao -- --ignored --nocapture
```
