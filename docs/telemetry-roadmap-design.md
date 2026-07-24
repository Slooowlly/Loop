# Telemetria de produto — o que medir em seguida

> Status: **fase 1 implementada; o resto é design proposto.** O ciclo de vida das corridas
> e o desfecho (fase 1) já estão no código — o contrato dos dois vive em
> [`telemetry-endpoint.md`](telemetry-endpoint.md), que continua sendo a fonte da verdade
> do que existe. Este documento é sobre o que vem depois, e em que ordem.
>
> A fase 1 foi implementada **fora de ordem**, antes da fase 0, porque foi pedida
> diretamente. A ordem abaixo segue valendo para o que resta.

---

## 1. As perguntas, e onde cada uma está

A telemetria nasceu para responder três perguntas que o save local não responde; uma quarta
entrou depois — se a curva de dificuldade está no ponto. Vale marcar o placar honesto antes
de propor qualquer coisa:

| Pergunta | Estado | O que falta |
|---|---|---|
| Quantas corridas estão rolando agora? | **Respondida** | — |
| Em que temporada/categoria cada jogador está? | **Respondida** | — |
| Onde o jogador para de jogar? | **Em aberto** | Tudo. Nenhum evento atual toca nisso. |
| **A dificuldade está calibrada?** | **Respondida** (fase 1) | Dados chegando; falta volume para a leitura ser confiável. |

A terceira e a quarta são as valiosas. A terceira não tem nenhuma resposta porque o evento
de corrida só existe para quem **já chegou lá**; a quarta é barata e quase pronta — falta
anexar o desfecho a uma borda que já dispara. Elas organizam a ordem deste documento.

## 2. A restrição que ordena tudo

O consentimento é pedido **depois da primeira corrida dirigida no iRacing**. Isso é uma
decisão de produto boa (a pergunta chega a alguém com contexto), mas tem uma consequência
que precisa estar escrita:

> Quem instala e desiste antes de correr **nunca consente**, e portanto é permanentemente
> invisível.

Isso não é um defeito do desenho — é o piso de qualquer esquema opt-in. Medir essa
população exigiria mandar dado antes de perguntar, que é exatamente o que o módulo promete
não fazer. **Não vale a pena.**

O que vale, e é barato: quando o consentimento chega, mandar **um evento retroativo** com
o histórico que já está no disco. O `SaveMeta` guarda `created_at`, `total_races`,
`current_season` e `last_played` de cada carreira, então no instante do "sim" dá para
reconstruir o passado inteiro daquele jogador — quando criou a carreira, quanto tempo
levou até a primeira corrida, quantas simulou antes de correr.

Ou seja: não se perde o degrau "criou a carreira e demorou para correr". Perde-se só quem
nunca correu — e esse, por definição, nunca ia aparecer.

**Decisão:** manter o gatilho onde está e fazer o backfill (fase 2).

---

## 3. Ordem de implementação

O critério é valor por esforço, com uma regra: o que **não depende de decisão nenhuma**
vem antes do que depende, para não travar.

### Fase 0 — Persistir o `reading_seconds` *(servidor, minutos)*

O app **já manda** esse campo ([`client.rs:73`](../src-tauri/src/narrative/client.rs:73)) e
o servidor **já usa** para dimensionar o texto — mas joga fora depois. Guardar é somar um
campo no `recordUsage`.

Responde: **as pessoas leem o boletim, ou pulam?** É a parte mais cara do produto (é onde
o dinheiro de IA queima) e hoje não há um único número sobre o consumo dela. Cruzado com
o `usage_by_install`, que já acumula custo por usuário/mês, sai a conta que dimensiona o
preço da assinatura: *quem custa caro é quem joga muito, ou quem gera texto e não lê?*

Nenhuma mudança no cliente. Nenhum deploy do app. É a maior razão/esforço da lista inteira,
e por isso vem primeiro.

### Fase 1 — O desfecho da corrida ✅ *(implementada)*

Hoje o `race_end` leva `status` e `duracao_s` — sabe-se *quantas* corridas acontecem, não
**como terminam**. Esta fase fecha isso e, de quebra, responde a pergunta 4.

Quase tudo já está calculado no mesmo escopo da borda que dispara:

| Campo | De onde | Custo |
|---|---|---|
| `incident_points`, `laps_completed`, `off_track`, `towed_to_pit`, `garage`, `black_flag`, `disqualified`, `worst_crash`, `restarts` | `AttemptEvidence` ([`race_monitor.rs:309`](../src-tauri/src/iracing_sdk/race_monitor.rs:309)) | cópia de campo |
| `posicao_final`, `posicao_grid`, `carros_na_classe` | `CarMeta.class_position` / `grid_class_position`, contagem de `cars_meta` | cópia de campo |
| `melhor_volta_s`, `melhor_volta_classe_s` | `CarIdxBestLapTime` ([`mod.rs:280`](../src-tauri/src/iracing_sdk/mod.rs:280)) no `player_car_idx` e no melhor da classe | cópia de campo |
| `dificuldade` | `SaveMeta.difficulty`, via `set_career_context` | uma linha, no call site que já existe |
| `carro` | YAML da sessão, ao lado de `class_id`/`car_number` | **leitura nova** (ver abaixo) |
| `track_id` | já é enviado hoje | — |

Três decisões que fazem a diferença entre um número útil e um número enganoso:

**1. Posição sozinha não significa nada.** Quinto entre 8 não é quinto entre 24; e quinto
partindo de décimo segundo é uma corrida ótima, partindo da pole é um desastre. Por isso os
três campos andam juntos — `posicao_final`, `posicao_grid` e `carros_na_classe`. Deles saem
as duas leituras que realmente calibram: **posições ganhas/perdidas** e **percentil no
grid**. Mandar só a posição final produziria uma média que mistura tudo e não sustenta
decisão nenhuma.

**2. O melhor sinal de dificuldade não é a posição — é o déficit de ritmo.** A razão entre
a melhor volta do jogador e a melhor volta da classe é imune a tamanho de grid, a
incidente, a abandono e a azar de estratégia. Duas corridas com a mesma posição final podem
ter 0,5% e 8% de déficit, e são casos de calibração opostos. É por isso que
`melhor_volta_classe_s` vai junto: sozinha, a volta do jogador não é comparável nem com ele
mesmo em outra pista.

**3. Em DNF, posição e volta não valem.** O `status` já vai no evento e precisa ser o filtro
obrigatório de qualquer média — caso contrário um abandono na volta 2 entra na estatística
como "último lugar" e afunda a leitura da dificuldade.

**Sobre o carro:** ele foi promovido da fase 4 para cá porque **a melhor volta não é
interpretável sem ele**. `track_id` sozinho não fecha a chave: 1:35 em Lime Rock é rápido
num carro e lento em outro. A chave de comparação é `(pista, carro)`, e sem as duas metades
o dado de volta não serve para nada. É a única leitura nova de YAML desta fase — o parser já
lê `class_id` e `car_number` no mesmo lugar, falta o modelo (`CarID`/`CarScreenName`).

Responde: **como as corridas terminam** e **se a dificuldade está no ponto**. Cruzando
`dificuldade × déficit de ritmo × categoria`, sai onde a curva está errada — e, mais
importante, se está errada para todo mundo ou só para quem escolheu um nível específico.

> **Consequência no consentimento:** a tela de consentimento hoje promete "início e fim da
> corrida, duração, pista, e o ano e a categoria da sua carreira". Posição, tempo de volta
> e carro **não estão nessa lista**. O texto de `telemetryConsent.sends` tem de crescer
> junto com esta fase — a lista é uma promessa, e enviar além dela quebra a promessa mesmo
> que o dado continue anônimo. Isso vale para quem já respondeu "sim" ao texto antigo.

### Fase 2 — O funil *(cliente + servidor, a maior)*

A pergunta em aberto. Dois eventos novos:

**`app_open`** — uma vez por abertura, carregando o retrato do jogador:

```json
{
  "event": "app_open",
  "install_id": "…",
  "carreiras": 2,
  "carreira_ativa_dias": 34,
  "corridas_totais": 41,
  "temporada_atual": 3,
  "dificuldade": "realista",
  "categoria": "Mazda Rookie"
}
```

Tudo isso sai do `SaveMeta` ([`app_config.rs:9`](../src-tauri/src/config/app_config.rs:9)) +
`list_saves`. Nada novo precisa ser calculado.

**`career_milestone`** — nas bordas que já existem: carreira criada
(`finalize_career_draft`), temporada fechada (`advance_season`), carreira abandonada
(mudança de `lifecycle_status`).

**`backfill`** — disparado uma única vez, no instante do consentimento, com o histórico
local descrito na seção 2.

Do lado do servidor, o `installs` já existe e ganha os campos de funil (`first_seen` já
está lá); a leitura por coorte é agrupar `installs` por semana de `first_seen` e ver
quantos chegaram a cada degrau.

Responde: **onde o jogador para.** Instalou → criou carreira → correu a primeira → chegou
à segunda temporada → sumiu. Cada degrau com uma taxa. É o que diz onde mexer no jogo.

### Fase 3 — Painel de leitura *(servidor, meio dia)*

Hoje ler a telemetria é `curl` com segredo na mão. Isso escala até uns três números; passa
disso e o dado deixa de ser consultado — e **dado que não se olha é dado que não foi
coletado**.

Uma página HTML servida pelo próprio Cloud Run, protegida pelo mesmo segredo, com: corridas
rolando agora, o funil por coorte, distribuição de temporada/categoria, custo por usuário,
desfecho das corridas. Sem dependência externa, sem build — o servidor já devolve HTML na
`/checkout/success`, é o mesmo padrão.

Vem depois da fase 2 porque é aí que o dado deixa de caber num JSON lido a olho: coorte só
é legível como gráfico.

### Fase 4 — Resto do contexto da sessão *(cliente, pequeno)*

Encolheu: pista, carro e tamanho do grid subiram para a fase 1, e com eles a pergunta
"**que conteúdo do iRacing meu público usa**" já fica respondida. O que sobra é o contorno
da sessão — tipo (prática/quali/corrida), duração prevista, condição de pista/clima.

Responde uma pergunta menor, mas real: **em que formato as pessoas correm.** Corrida de 15
minutos com grid cheio e enduro de 2h com seis carros pedem coisas diferentes do app, e hoje
as duas aparecem como uma linha idêntica.

Fica em quarto porque é o mesmo tipo de leitura nova de YAML da fase 1, sem a mesma
urgência — depois de ter pista, carro, posição e volta, isto é refinamento.

### Fase 5 — Eventos de qualidade *(cliente, pequeno, contínuo)*

Falha de IA que caiu no fallback silencioso, erro de import do iRacing, save corrompido,
falha de update.

Responde: **por que o jogador sumiu.** Sem isso, o funil da fase 2 mostra a queda mas não a
causa, e "o jogador desistiu" fica indistinguível de "o jogador quebrou". As duas conclusões
levam a trabalhos opostos.

### Fase 6 — Hash do `subsession_id` *(opcional)*

O `subsession_id` é um identificador público do iRacing: com ele dá para abrir a página
daquela corrida no site e ver quem correu. Mandar o hash em vez do número mantém o upsert
por restart e a detecção de dois jogadores na mesma subsessão (hash preserva igualdade), e
tira o identificador público do banco.

Fica por último **por decisão sua**: o acesso ao banco é só seu, e o risco não justifica
prioridade. Está listado para não se perder — não para ser feito agora.

---

## 4. Fora de escopo, por decisão

- **iRacing customer id / nome da conta.** Resolveria dedupe entre máquinas, mas é a
  fronteira entre telemetria anônima e cadastro de pessoas. Decidido: não entra.
- **Nome do piloto, da equipe, conteúdo do save.** Já prometido no cabeçalho do módulo.
- **Telemetria antes do consentimento**, para qualquer finalidade. Ver seção 2.

## 5. Regras que valem para todo evento novo

Não são sugestões — são o que já está no
[`telemetry.rs`](../src-tauri/src/telemetry.rs) e o que qualquer adição precisa manter:

1. **Nunca bloqueia.** Thread própria, timeout curto, erro engolido. O jogo não pode piscar
   porque o servidor caiu.
2. **Nunca fala sem consentimento.** `ENABLED` lido antes de tudo, inclusive nos eventos
   novos. Um evento que esqueça isso quebra a promessa inteira.
3. **Nunca inventa borda.** Todo evento pendura numa transição que o app já detecta para
   outro fim. Borda nova = risco novo em código quente.
4. **Servidor sanitiza tudo.** Número é número com teto, string tem tamanho máximo, campo
   ausente não vira `null` no banco.
5. **Tudo entra no teto diário** (`TELEMETRY_DAILY_CAP`) e ganha `expire_at`.

## 6. Custo

O Firestore libera 20 mil escritas por dia. Hoje uma corrida custa ~9 (3 eventos × 3 docs).
A fase 2 acrescenta ~3 por abertura de app. Mesmo com folga generosa, o teto só apertaria
em outra ordem de grandeza de jogadores — e quando isso acontecer, o problema é bom e o
`TELEMETRY_DAILY_CAP` já existe como freio.

O custo real desta lista não é infraestrutura: é **cada evento novo ser mais uma coisa que
pode quebrar numa thread de fundo enquanto o jogador corre**. Daí a regra 3.
