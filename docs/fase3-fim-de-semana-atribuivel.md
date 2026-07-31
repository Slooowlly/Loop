# Fase 3 — o fim de semana atribuível (desenho, não implementado)

Status: **proposta aguardando liberação.** A implementação depende da função pura que
está saindo de `simulation/forma.rs` no trabalho paralelo — construir contra a
assinatura de hoje é construir contra algo que muda. Nada deste documento está no
código.

Contexto: Pacote I, fases 1 e 2 entregues (migração v55, comando `get_race_reading`,
painel "o curso da corrida" e o bloco de fatos `curso` no debrief por IA).

---

## O problema, dito com precisão

A campanha de calibração vai **aumentar** a variação que vem do fim de semana — acerto
do carro, forma do momento, afinidade com a pista. O argumento de que isso não vira
"sorte" é que essas três fontes são atribuíveis a qualidade, ao contrário de um dado.

O argumento é bom e é insuficiente, porque atribuível é uma propriedade do **modelo** e
atribuir é um ato do **jogador**. Se o acerto do fim de semana se tornar a maior fonte
de variação do resultado e continuar sem aparecer em lugar nenhum, o jogador vive
exatamente aquilo que a estatística jura que não é. Da poltrona, "ele foi mal nesta
etapa" sem causa legível é indistinguível de dado.

Vale notar o que a Fase 2 **não** resolve disso. O painel "o curso da corrida" explica o
resultado **dentro** da corrida: o box custou 3 posições, ficou 3 trechos preso, a
amarela embaralhou. Isso é mecânica visível. O fim de semana é outra camada — é o
**ponto de partida** da corrida, não o que aconteceu nela. Um piloto com acerto ruim
larga mal e corre mal o dia inteiro, e nenhum número do painel da Fase 2 diz por quê.

## Onde o jogador lê isso

Minha proposta é **dois lugares, com funções diferentes**, e explicitamente **não** um
terceiro.

### 1. Antes — na aba da próxima corrida (a Sala de Estratégia)

É o lugar principal. O fim de semana precisa ser lido **antes** da corrida, não depois,
por uma razão de design que vale mais que a conveniência: informação que só aparece
depois do resultado é indistinguível de desculpa. Se o jogo me diz "seu acerto estava
ruim" **depois** de eu terminar P12, eu não aprendi nada — eu recebi uma justificativa.
Se ele me diz antes, o P12 deixa de ser surpresa e passa a ser consequência, e eu tenho
a chance de calibrar minha expectativa (e, no futuro, de agir).

O terreno já existe: `nextrace/` e o `EngineerBriefingPanel`, que é literalmente o
engenheiro conversando com o piloto antes da largada. É onde a frase cabe sem inventar
tela nova.

### 2. Depois — uma linha no debrief do engenheiro

Papel secundário e deliberadamente magro: **fechar o loop**, não explicar o resultado.
O bloco `pre_race` do `fatos.rs` já faz exatamente esse fechamento com a manchete e o
briefing da pré-corrida — a leitura do fim de semana entra ali, no mesmo mecanismo, não num
painel próprio.

**O pós é licenciado pelo pré, e a forma da frase tem de mostrar isso.** A linha do debrief
precisa referenciar a **previsão**, não repetir o estado: não "o acerto estava ruim" (que
lido depois é desculpa), mas "a equipe te avisou no sábado que o carro não tinha vindo — e
não veio". Previsão que se cumpre ensina o mecanismo; explicação avulsa não ensina nada. Na
prática isso obriga o mesmo par (camada, faixa) a estar disponível nos dois momentos — daí a
exigência de determinismo no item 4 das dependências abaixo. Se o pré e o pós discordarem, o
loop de atribuição se desmonta na cara do jogador e o efeito é pior que não ter nada.

### 3. O que eu NÃO proponho: o dossiê do piloto

Tentador e errado. Afinidade com pista é o único dos três com cara de atributo
permanente, e colocá-la no dossiê a transforma num número que o jogador vai querer
otimizar — vira estatística de personagem, não condição de fim de semana. Forma e acerto
são efêmeros por construção e não têm o que fazer numa ficha. Se um dia a afinidade
merecer casa fixa, o lugar é o histórico de circuitos (que já existe), não o dossiê.

## O que mostrar — e o quanto revelar

Esta é a decisão de design, e a pergunta é real: mostrar demais vira planilha, mostrar
de menos mantém o problema. Minha resposta:

> **Duas camadas na tela, qualitativas, com o mecanismo nomeado e o número escondido.**

Desdobrando as três decisões que isso embute:

### Decisão 1 — três camadas, cortadas no eixo do TEMPO

**Revisada.** A primeira versão deste documento propunha duas camadas — "a pista"
(afinidade) e "o fim de semana" (forma + acerto) — justificadas pelo eixo da **agência**:
o jogador não pode agir sobre a diferença entre forma e acerto. Esse eixo estava errado, e
de um jeito que se auto-destrói: o jogador não pode agir sobre **nenhuma** das três (não
existe minijogo de acerto). Levado a sério, o critério colapsaria as três num número só,
o que é absurdo. O eixo certo é o **tempo** — que é, aliás, o princípio organizador do
próprio `forma.rs` ("camadas com escalas de tempo diferentes").

Aplicado corretamente, o eixo do tempo dá **três**, não duas:

| Na tela | Camada | Continuidade | ρ no intervalo que importa |
|---|---|---|---|
| **A pista** | afinidade | periódica — visita → visita ao mesmo circuito | **1,0** (hash de `(piloto, pista)`: idêntica todo ano) |
| **O seu momento** | forma | serial — etapa → etapa | **0,65** (o AR(1)) |
| **Este fim de semana** | acerto | nenhuma | **0** (sorteado por `(equipe, evento)`) |

O critério é: **esta camada carrega informação sobre alguma corrida futura?** Por ele,
qualquer fusão de duas mistura HISTÓRIA com RUÍDO:

- fundir forma + acerto (proposta original) mistura história serial com ruído;
- fundir afinidade + acerto mistura história *periódica* com ruído — falha simétrica.

O ρ = 0 atribuído à afinidade mede o intervalo errado. A afinidade é permanente e
vitalícia por circuito — "o Kowalczyk voa em Spa e apanha em Okayama, TODO ano", nas
palavras do módulo. Entre etapas vizinhas ela de fato não se correlaciona, mas o relógio
dela não é a rodada, é a **visita**: nesse intervalo ρ é exatamente 1, porque é
determinística. Pelo critério da informação futura, a afinidade não é a menos informativa
das três — é a **mais**, a única perfeitamente previsível.

Sobra o acerto como o único sem informação sobre qualquer corrida futura. É ele, portanto,
que precisa ficar sozinho — e não por acaso é a camada que o módulo estima ser "**a de
maior impacto das três**". A camada mais forte e sem nenhum conteúdo preditivo é
precisamente a que, invisível, se lê como sorte pura.

As três também respondem a três perguntas distintas que um piloto de verdade faz separado:
"estou numa boa fase?" (forma, com tendência), "eu vou bem aqui?" (afinidade, verificável
contra o histórico) e "o carro veio hoje?" (acerto). Fundir duas responde duas perguntas
com uma frase.

E o receio de "virar planilha", que justificava o "duas, não três", vinha dos **números** —
que a Decisão 2 já elimina. Três faixas qualitativas com o mecanismo nomeado não é
planilha; é menos do que a tela pós-corrida já mostra.

### Decisão 2 — qualitativo, não numérico

Cinco faixas nomeadas, sem número: **contra você · abaixo · neutro · a favor · muito a
favor**. Sem "+3,4 pts", sem barra de 0 a 100, sem percentil.

Três razões, em ordem de peso:

1. **Número exato convida a engenharia reversa.** Com "+3,4" na tela, o jogador
   descobre em quatro corridas que a faixa vai de −8 a +8, e a partir daí ele não está
   mais correndo: está calculando se vale a pena a etapa. É o momento em que o jogo vira
   planilha, e não tem volta.
2. **A precisão seria falsa.** O número é a soma de camadas que a campanha de calibração
   vai mexer. Exibir "+3,4" promete uma estabilidade que o valor não tem, e cada
   recalibração viraria uma mudança silenciosa de contrato com o jogador.
3. **Faixa qualitativa é o suficiente para atribuir.** Atribuir exige saber *que* houve
   uma causa e *qual* foi — não o tamanho dela em pontos. "A pista está contra você" já
   converte azar em causa, que é a inteira função deste pacote.

### Decisão 2b — a faixa é medida em σ da própria camada, nunca em pontos

Corolário obrigatório da Decisão 2, e o requisito mais fácil de errar de todo o pacote.

A campanha de calibração vai **redistribuir** as três escalas do `forma.rs` mantendo a
soma: baixar a da afinidade (hoje `AFINIDADE_ESCALA_PONTOS = 3,0`, a maior das três,
quando a análise diz que deveria ser a menor) e subir a do acerto. Com limiares
**absolutos** em pontos de skill, essa redistribuição quebraria a tela sem quebrar teste
nenhum: a afinidade cairia para metade da amplitude e passaria a marcar "neutro" quase
sempre, o acerto passaria a saturar em ±2. As duas camadas continuariam **corretas no dado
e mentindo na leitura** — e o teste de que nenhum número chega à tela continuaria
passando, porque nenhum número chega à tela.

Então a faixa é definida em **múltiplos do σ da própria camada**: `|z| < 1σ` → neutro,
`1σ ≤ |z| < 2σ` → ±1, `|z| ≥ 2σ` → ±2. Assim a faixa significa "**incomum para esta
camada**", que é o que o jogador precisa saber, e recalibrar a escala não muda o
significado de "muito a favor". O σ de cada camada é justamente o parâmetro que a campanha
ajusta, então está disponível de qualquer forma.

O guarda-corpo disso é um teste de **invariância de escala**: escalar valor e σ pelo mesmo
fator não pode mudar a faixa. Já implementado (`faixa_e_invariante_a_escala_da_camada`), e
é o teste que falharia com limiares absolutos.

### Decisão 2c — a faixa ANUNCIADA é persistida, não recomputada

Consequência direta da exigência de determinismo: se pré e pós discordarem, o loop de
atribuição se desmonta. E há dois caminhos pelos quais uma faixa recomputada divergiria da
que foi anunciada:

1. **Recalibração.** A tela pós-corrida é revisitável (`race_screens/<id>.json` persiste e
   `get_saved_race_screen` a reabre). Um patch que mude o σ de uma camada faria uma corrida
   antiga passar a mostrar faixa diferente da anunciada no sábado — pré e pós discordando
   por atualização, sem ninguém ter errado nada.
2. **Ordem de avanço da forma.** `update_driver_forma` é chamado **dentro** da simulação
   (`commands/race/simulacao.rs`), então antes da corrida `drivers.forma` guarda o estado da
   etapa *anterior*. A faixa anunciada é derivada de `proxima_forma` com semente
   determinística — reproduzível —, mas também função de motivação e confiança, que podem
   mudar no meio do fim de semana. Recomputar depois não devolve necessariamente o que foi
   anunciado.

A faixa anunciada é, portanto, **fato histórico do fim de semana, não função do estado
atual do jogo** — mesma natureza dos campos da v55. Vai no mesmo caminho, como blob
(`race_results.leitura_fds_json`, v56): lida inteira, de uma corrida só, nunca predicado, e
com aridade ("3 camadas × 2 canais") que é constante de outro módulo.

Default `'{}'` **não** desserializa numa `WeekendReading` válida, então a exibição cai em
`None` e cala. "Não anunciado" e "anunciado como morno" são coisas diferentes, e só a
primeira é honesta para uma corrida que nunca teve leitura.

### Decisão 3 — o mecanismo nomeado, sempre

O rótulo precisa dizer **de onde vem**, não só que existe. "Fim de semana difícil" é
horóscopo; "o carro não respondeu ao acerto neste circuito" é mecanismo. A diferença é
tudo: a primeira o jogador arquiva como sabor, a segunda ele usa para prever.

Concretamente, a leitura antes da corrida seria mais ou menos:

```
A PISTA          ▁▂▃▅▇  a favor
                 Você já andou bem aqui — 3 corridas, melhor resultado P4.

O FIM DE SEMANA  ▁▂▃░░  abaixo
                 O carro não veio como a equipe esperava neste traçado.
```

A linha de apoio da pista **cita o histórico real** (`historico_circuitos` já tem
largadas, melhor resultado e temporada). Isso é o que fecha o argumento da
atribuibilidade: a afirmação é verificável contra a memória do jogador, não uma alegação
do jogo sobre si mesmo.

### E o que fica escondido

O valor numérico e a decomposição por camada. Se um dia isso precisar aparecer, o lugar
é atrás do modo DEV (que já existe na tela pós-corrida com o `import.meta.env.DEV`), não
na UI de jogo. O harness de calibração já consome os números pela via dele e não precisa
da tela.

## Dependência técnica — o que preciso da função pura

Para implementar isto sem tocar em `simulation/**`:

1. Uma função **pura** que devolva as três camadas, dado piloto + pista + temporada +
   rodada, **sem** `AppHandle` e **sem** gravar nada. Hoje a esteira de modificadores
   mora dentro de um `#[tauri::command]`, o que a torna inalcançável de um comando de
   leitura.
2. As três camadas **separadas** no retorno (a tela funde duas; o dado não pode vir
   fundido — ver Decisão 1).
3. O valor em unidade **estável e documentada**, para eu mapear em faixa qualitativa. Se
   a escala for recalibrada, quero que a mudança apareça no mapeamento de faixa, num
   lugar só.
4. Determinismo para a mesma entrada: a leitura antes da corrida e o fechamento no
   debrief depois **precisam** dizer a mesma coisa, senão o loop de atribuição se
   desmonta na cara do jogador.

Nada disso pede campo novo na simulação — é a mesma informação que o motor já calcula,
alcançável de fora.

## O contrato — `WeekendReading`

Definido em `commands/career_types/corrida.rs` e já implementado do lado da apresentação.
Falta só o preenchimento.

```rust
pub struct WeekendReading {
    pub race_id: String,
    /// false = motor não forneceu → a tela não desenha NADA (regra do vazio da v55).
    pub available: bool,
    pub track_affinity: WeekendLayer,  // ρ = 1 entre visitas
    pub driver_form: WeekendLayer,     // ρ = 0,65 entre etapas — a única com trend
    pub car_setup: WeekendLayer,       // ρ = 0
}

pub struct WeekendLayer {
    /// Faixa ORDINAL em [-2, 2] no canal de RITMO. O valor bruto NÃO cruza a ponte.
    pub band: i8,
    /// A mesma faixa no canal de CLASSIFICAÇÃO, separada porque o motor é assimétrico
    /// entre canais (`MULT_AFINIDADE_QUALI`) e é isso que explica voar no sábado e não
    /// converter no domingo. A tela só cita quando DIVERGE do ritmo.
    pub qualifying_band: i8,
    /// [-1, 1]. `None` nas camadas sem autocorrelação — prometer tendência onde ρ = 0
    /// seria inventar arco a partir de ruído.
    pub trend: Option<i8>,
    /// Fato verificável já resolvido no backend (ex.: "3 corridas aqui, melhor P4").
    pub support: Option<String>,
}
```

Duas escolhas do contrato que valem defesa:

- **Faixa ordinal (`i8`), não string nem float.** Ordinal ordena naturalmente, não admite
  typo de chave e — o que importa — **não é magnitude**: o mapeamento valor bruto → faixa
  fica num lugar só (o backend), e quando a calibração mudar as escalas é lá que se mexe,
  sem tocar na tela nem nas traduções.
- **`available` explícito em vez de `Option<WeekendReading>`.** O comando responde "não
  tenho leitura" sem virar erro nem `null` ambíguo, e a tela tem um único predicado para a
  regra do vazio.

## Superfície — o que já está feito e o que falta

Feito (não depende do motor):

- DTO `WeekendReading` / `WeekendLayer` em `commands/career_types/corrida.rs`.
- `src/components/race/WeekendReadingPanel.jsx` — três camadas, medidor de 5 blocos,
  palavra da faixa, tendência só onde existe, canal de quali só quando diverge.
- Lugar na Sala de Estratégia: terceiro card de condições em `EngineerBriefingPanel`
  (junto do clima e do risco de quebra), antes da narrativa, para o engenheiro poder
  comentar em cima dela. Recebe `weekendReading` por prop e renderiza `null` sem dado,
  então a tela de hoje não muda até o fio ser ligado.
- Chaves de i18n nos dois locales; 8 casos de vitest contra dados fabricados, incluindo as
  bordas (±2), a regra do vazio e a garantia de que **nenhum número** chega à tela.

- `faixa_por_sigma(valor, sigma) -> i8` e `WeekendLayer::from_sigma(...)` — o mapeamento
  valor bruto → faixa ordinal, **em σ**, com 4 testes: limiares e sinal, invariância de
  escala, σ/valor inválidos → neutro, e separação dos canais com clamp da tendência. O valor
  bruto entra em `from_sigma` e não sai, então por construção ele nunca cruza a ponte.
- Migração **v56**: `race_results.leitura_fds_json`, a faixa anunciada como fato histórico.
  `set_race_weekend_readings` grava (na mesma transação do resultado, via `UPDATE` — a
  leitura vem de outro subsistema e não cabe na assinatura do batch), `get_race_reading` lê,
  e `RaceReadingCar::announced_weekend_reading` a expõe já desserializada. 4 testes, incluindo
  o upgrade v55 → v56 preservando o que a v55 gravou.

- Migração **v57**: `race_weekend_readings`, a leitura como fato do fim de semana que nasce
  antes do resultado. 5 testes, incluindo o que justifica a migração (gravar e ler a leitura
  sem existir nenhuma linha de `race_results`), a idempotência do preparo, e o upgrade
  v56 → v57.
- A frase de fechamento do debrief: `caso_do_anuncio(soma, assessment)` em
  `ai_news/fatos.rs`, com a matriz de tom dos quatro casos travada em teste. Lê a leitura
  anunciada da v57, nunca recomputa.

Falta (depende da função pura):

- Função `get_weekend_reading_in_base_dir` + casca + registro em `lib.rs`, no padrão de
  `get_race_reading` — só precisa chamar `from_sigma` com os valores e o σ de cada camada,
  e gravar via `set_race_weekend_readings` na primeira vez (lazy-once, igual ao clima).
- Passar `weekendReading` de `NextRaceTab` (via `useBriefingData`) para o painel.
- A linha de fechamento no bloco `pre_race` do `fatos.rs`, na forma "foi anunciado × foi
  cumprido", lendo `announced_weekend_reading` em vez de recomputar.

## Validação contra a API do motor

A API existe (`EloDaEsteira::{AfinidadeDePista, FormaDoMomento, AcertoDeFimDeSemana}`,
`Canal::{Corrida, Classificacao}`, `pretendido_de(elo, canal)`) e **casa com este DTO sem
adaptação**: três elos separados servem as três camadas, e o par de canais serve
`band`/`qualifying_band` diretamente.

### Decidido: a faixa mapeia do PRETENDIDO

Quatro razões, a última impeditiva:

1. É o que o jogador está lendo — a leitura da **equipe** sobre o fim de semana, não o
   efeito residual depois da aritmética interna. A tela descreve o mundo, não o motor.
2. O aplicado carrega o arredondamento para `u8`, que é artefato de **representação**. Uma
   faixa que se move por quantização seria variação NÃO-atribuível exibida como atribuível —
   a inversão exata que este pacote existe para impedir. E seria não-monotônica: a leitura
   tremeria sem causa que o jogador possa nomear.
3. **Não existe aplicado por camada** — as três são somadas e arredondadas uma vez só, de
   propósito. Mapear do aplicado tornaria a decomposição em três camadas impossível, e com
   ela cai o argumento inteiro de história-vs-ruído.
4. **No momento do anúncio o aplicado ainda não existe.** A faixa é dita antes da corrida; o
   aplicado só passa a existir quando a esteira soma para aquela etapa.

Registrado em `WeekendLayer::from_sigma`. Este DTO **não quer** fidelidade ao aplicado por
camada, hoje nem depois.

### Decidido, e é o que ninguém tinha perguntado: **um σ por camada, não um por canal**

Parece mais rigoroso normalizar cada canal pelo σ da sua própria distribuição. **Apaga
silenciosamente a única coisa que `qualifying_band` existe para mostrar.**

O canal de classificação da afinidade é o de corrida × `MULT_AFINIDADE_QUALI = 1,5`. Se o σ
da classificação também for 1,5× maior, os dois z saem idênticos e `qualifying_band` empata
com `band` **sempre, em todas as camadas** — campo morto. Normalizando os dois canais pelo
mesmo σ da camada (o do pretendido no canal de **corrida**), a assimetria sobrevive como
fato visível: a afinidade sai mais forte no sábado, que é a afirmação verdadeira e a que
sustenta "voou no sábado e não converteu no domingo".

`from_sigma` aceita **um** σ por construção, para tornar o erro inexprimível pela API; o
erro é modelado no teste `um_sigma_por_camada_preserva_a_assimetria_de_camada` no nível de
`faixa_por_sigma`.

**Pedido concreto ao motor:** exponha o σ da distribuição do pretendido **por elo**, do
canal `Corrida`. Se vier um σ por (elo, canal), o do canal de classificação será
deliberadamente ignorado — e é melhor não mandá-lo do que arriscar que alguém o "conserte"
para dentro.

## Diagnóstico: o determinismo do anúncio NÃO é garantido pela arquitetura atual

Verificado no código. A resposta curta é **não** — e o buraco é latente, não vivo.

### A ordem real

`commands/race/simulacao.rs` faz, nesta sequência:

1. `estado_de_forma.push(driver.forma)` — lê a coluna, que contém o estado da etapa
   **anterior**;
2. `aplicar_esteira(&base, &contextos, temporada, rodada, track_id, &estado_de_forma)` —
   a esteira avança um passo de AR internamente e devolve `esteira.estado_de_forma`;
3. só então, e só se `Playable`, `update_driver_forma` grava o estado novo.

Portanto, em **qualquer instante anterior à corrida**, `drivers.forma` vale a etapa N−1. O
valor que a corrida usa para a etapa N é
`proxima_forma(drivers.forma, semente_forma(temporada, rodada, driver_id), motivacao, confianca)`.

### Os dois modos de falha

**(a) O off-by-one-passo-de-AR.** Se o caminho pré-corrida **ler** `drivers.forma` e aplicar
`forma_em_pontos`, ele anuncia a etapa N−1 enquanto a corrida roda a N. É o erro fácil —
ler a coluna parece obviamente certo — e produz divergência de uma faixa ou de nenhuma,
que é o mais difícil de notar. Para concordar, o pré precisa **replicar o avanço**, não ler
o estado.

**(b) A deriva de motivação/confiança.** `proxima_forma` também é função de `motivacao` e
`confianca`, que são estado mutável. Se qualquer coisa mexer nelas entre o jogador abrir a
Sala de Estratégia e rodar a corrida, os dois cálculos divergem — e o anúncio fica instável
até entre duas aberturas da mesma tela, antes da corrida.

Hoje o mutador relevante é `apply_post_race_fame` (`commands/race/financas.rs`), que roda
**depois** da corrida, e o ajuste de hierarquia, que roda dentro da persistência. Ou seja: a
janela está limpa **por acidente da ordem atual de chamadas**, não por construção.

### Por que isso é estruturalmente diferente do clima

O precedente do projeto é `resolve_and_persist_race_weather`: o clima sai de
`event_seed(career_id, race_id)` — que não depende de **nada mutável** — e é persistido no
`calendar`. Por isso recomputar clima é sempre seguro, e a persistência é cache, não fonte.

A forma não está nessa classe: depende de três entradas mutáveis. A leitura do fim de
semana é segura hoje **por coincidência de ordenação**, e qualquer feature futura que toque
motivação entre etapas (treino, decisão de patrocínio, evento de moral, a própria Sala de
Estratégia ganhando ações) abre o buraco em silêncio.

### Recomendação — e ela mexe na v56

O anúncio deve **nascer uma vez**, no primeiro momento em que é preciso (lazy-once, igual ao
clima), e as duas telas lerem a mesma linha. Isso o torna seguro **por construção** em vez
de por coincidência.

Consequência de schema: a linha precisa existir **antes** do resultado, e
`race_results` não serve — a linha dele só nasce depois da corrida. O lugar é tabela
própria, no molde de `race_breakdowns` e `race_safety_cars`:

```sql
CREATE TABLE IF NOT EXISTS race_weekend_readings (
    race_id   TEXT NOT NULL,
    driver_id TEXT NOT NULL,
    leitura_json TEXT NOT NULL DEFAULT '{}',
    PRIMARY KEY (race_id, driver_id)
);
```

Isso **supersede** a coluna `race_results.leitura_fds_json` da v56. **Implementado como
v57.**

`set_race_weekend_readings` recebe uma `Connection`, não uma `Transaction` — de propósito:
a linha nasce no preparo da etapa, fora da transação do resultado. `INSERT OR REPLACE`
torna o preparo idempotente (reabrir a Sala de Estratégia reescreve, não duplica).
`get_race_weekend_reading` é o que a Sala lê para **não** recalcular; `get_race_reading`
traz a mesma linha por `LEFT JOIN` para a tela pós-corrida. Sem FK para `race_results` —
amarrá-la ao resultado recriaria o problema que a v57 resolve.

A coluna da v56 fica onde está, sem leitor nem escritor. Removê-la exigiria um
`DROP COLUMN` capaz de impedir o save de abrir se falhar em alguma build de SQLite — custo
desproporcional para uma coluna provadamente vazia (nada nunca a populou em produção,
porque o produtor da leitura ainda não existe).

### ⚠ A seta de tendência é uma promessa condicional

O harness mediu o excesso de sequência da forma com a amplitude atual: **0,02 corrida** —
estatisticamente o mesmo que uma forma sem memória nenhuma. Quem sustenta memória
*perceptível* é a amplitude, não o ρ, e a amplitude ainda está nos valores de chute
inicial.

Então hoje `driver_form.trend` descreve um mecanismo que existe no modelo e que o jogador
**não consegue sentir**. Isso é critério de aceitação da fase 1 da calibração.

**Se a fase 1 fechar sem entregar a perceptibilidade, o campo e a seta saem.** Interface
que afirma mecanismo inexistente é a mesma falha que este pacote existe para evitar — e
cometida por nós é pior, porque aí a causa ilegível não estaria só escondida, estaria
inventada. A seta fica, por ora, como aposta explícita e revogável.

### A tendência é minha, não do motor

`trend` só existe na forma e não é derivável de um pretendido de uma etapa: precisa do
estado anterior. `drivers.forma` guarda exatamente isso (o estado da etapa anterior, já que
`update_driver_forma` roda dentro da simulação), então o comando compara o estado anterior
com o da etapa e resolve a tendência sem nada novo do motor. Registrado aqui para que a
derivação pré-corrida não seja "simplificada" mais tarde.
