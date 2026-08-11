# "O Que Esperar" — Design da aba de expectativas de pré-temporada

> Status: **design travado, implementação pendente de aprovação.** Este documento é a
> fonte da verdade. O contrato técnico do endpoint vive em
> [`season-preview-endpoint.md`](season-preview-endpoint.md) e deve ser reescrito para
> casar com este design (a v1 do código usa 2ª pessoa e números crus — **será refeita**).

---

## 1. Visão e propósito

"O Que Esperar" é a **matéria de abertura da temporada**: a edição que a revista publica
*antes da primeira corrida*, quando ainda não há resultado nenhum para noticiar. Ela
substitui o antigo "livro fechado" — a revista está **sempre aberta**.

Não é um placeholder nem um resumo de planilha. É uma **peça jornalística de
antecipação**: apresenta o elenco do ano, aponta os favoritos, levanta as promessas e as
incógnitas, contextualiza o que mudou desde o título anterior e arma a expectativa para a
etapa de abertura. É a aba que faz o jogador **entrar na temporada já com um enredo na
cabeça** — quem observar, de quem esperar o quê, qual é a história a se provar.

Para ser uma aba *importante*, ela precisa de três coisas: **substância** (texto de
revista, não três frases), **cobertura por piloto** (o leitor conhece o grid nome a nome)
e **voz** (uma publicação com opinião editorial, não um relatório).

---

## 2. Princípios editoriais (regras invioláveis)

Estas regras valem para **todo o texto gerado** — IA ou fallback determinístico.

1. **3ª pessoa, voz de revista. SEMPRE.** É uma matéria publicada, escrita por um
   redator para um leitor genérico. **NUNCA se dirige ao jogador.** Proibido "você",
   "seu", "sua", "seu carro", "sua equipe", "SEU CENÁRIO". O piloto do jogador entra
   **pelo nome, como qualquer outro** — no máximo com um leve gancho de contexto ("o
   piloto da casa", "a aposta da Racing Academy Red"), jamais em 2ª pessoa.

2. **Descrição, não planilha. Nenhum número cru no corpo do texto.** Proibido citar
   nível de carro ("1/10"), valor de atributo ("potencial 50"), salário em cifras
   ("$ 13.318"), skill, mídia, idade exata como estatística. Todo dado vira **qualidade**:
   *material equalizado*, *o mais valorizado do grid*, *a maior promessa*, *estreante*,
   *já sabe vencer*. (Números continuam OK **na UI** — a coluna lateral de favoritos é
   visualização de dados; a proibição é para a **prosa**.)

3. **Ancorado em dados reais.** O texto só afirma o que os fatos sustentam. **Não inventa**
   resultados, patrocinadores, rivalidades, números ou histórico. Um piloto sem vitórias
   não "vem de uma sequência de triunfos". A cor é livre; os fatos são sagrados.

4. **Uma tese dominante + camadas.** Como os briefings pré-corrida
   (`nextRaceThesis.js`), a matéria elege **um enredo central** para a temporada (ex.:
   "grid equalizado, decisão no talento" / "temporada de estreantes" / "o campeão foi
   embora, trono vago") e organiza o resto como apoio. Sem tese, vira lista.

5. **Cobertura de grid, não só do topo.** A matéria fala dos favoritos **e** das
   promessas/azarões. O leitor deve sair conhecendo mais que dois nomes.

---

## 3. Ciclo de vida

- **Quando aparece:** enquanto não há nenhuma corrida concluída na categoria do jogador
  (`editions.length === 0`). É o estado de pré-temporada.
- **Quando some:** ao concluir a 1ª etapa, quando entram as edições de corrida normais.
- **Regeneração:** uma matéria **por temporada+categoria**, cacheada (chave
  `season-preview:{season_id}:{category}`). Virada de temporada → nova matéria. Reabrir a
  aba não regenera (sem custo, sem cooldown).
- **Promoção de categoria:** se o jogador sobe de divisão entre temporadas, a matéria da
  nova temporada é sobre a **nova** categoria (novo elenco, novos favoritos).

---

## 4. Anatomia da matéria (estrutura de seções)

A matéria tem **manchete + linha-fina + corpo em 4 blocos**. O corpo é substancial:
**alvo de 4 a 6 parágrafos, ~450–600 palavras**. Cada bloco tem um papel; a IA recebe os
fatos já organizados por bloco (ver §6) e o prompt exige que todos apareçam.

1. **Chamada (manchete + linha-fina + lead).**
   A tese da temporada em uma manchete de revista + subtítulo. O parágrafo de abertura
   estabelece o enredo central e o clima do ano.

2. **A briga pela ponta (os favoritos).**
   Os 2–4 nomes de topo, com **expectativa individual de cada um**: por que é favorito, o
   que se espera dele, qual é a sua história (a promessa que precisa cumprir, o veterano
   que quer confirmar, o vice do ano passado com sede de título). Este é o coração da
   matéria — é aqui que o leitor "conhece" os protagonistas.

3. **Promessas e incógnitas (o segundo pelotão).**
   Estreantes de olho grande, pilotos em ascensão, azarões, jovens de equipe pequena.
   Quem pode surpreender e por quê. Dá profundidade ao grid além do topo.

4. **Pano de fundo + a largada.**
   O que mudou desde o último título: o **campeão em título** (ficou para defender? subiu
   de divisão? deixou o trono vago?), o **equilíbrio de material** (grid parelho vs.
   equipes com pacote superior — sempre descritivo), o **nome mais valorizado do mercado**,
   e as **relações que o grid carrega** (§5.5) — velhos conhecidos, ex-parceiros que se
   reencontram, uma rivalidade herdada. Fecha com a **etapa de abertura** como o primeiro
   veredito e o formato do calendário.

> Opcional (fase 2): um bloco **"storylines"** — 2 a 4 ganchos curtos em bullet ("O
> retorno de X", "A dobradinha da equipe Y", "O jejum de Z") renderizados como destaques
> visuais na página.

---

## 5. Camada de tradução — de número a qualidade, com assimetria de informação

Este é o núcleo do redesign, e ele tem **duas ideias**:

1. **Número vira qualidade.** O backend nunca manda skill/nível/salário crus; traduz cada
   sinal para um **token qualitativo** (relativo ao grid, via percentil), e o texto
   descreve o token.
2. **O jornalista não é onisciente.** Existe uma linha entre o que a imprensa **vê** e o
   que ela só **intui**. O redator escreve da perspectiva pública: ele conhece resultados,
   reputação e o **estilo de pilotagem** visível na pista, mas **não** enxerga o "ritmo
   bruto" (skill) como um número. A expectativa dele nasce dos **sinais públicos** — por
   isso um pódio recente pesa mais na percepção que um talento oculto ainda não provado.

### 5.1 O que é PÚBLICO (o jornalista descreve com segurança)

| Sinal            | Fonte                                              | Vira (token qualitativo) |
|------------------|----------------------------------------------------|--------------------------|
| **Currículo / resultados** | `stats_carreira.titulos/vitorias/podios`   | títulos → *já coleciona troféus*; vitórias → *sabe vencer*; pódios → *já provou o pódio*; nada → *ainda busca o primeiro grande resultado* |
| **Experiência**  | `stats_carreira.corridas`, `corridas_na_categoria` | 0 → *estreante absoluto*; poucas → *em ascensão*; muitas → *piloto estabelecido*; muitíssimas → *veterano calejado* |
| **Reputação / fama** | `atributos.midia`, `carisma`                   | topo → *nome de público / queridinho da mídia*; alto → *já tem torcida*; baixo → não citado |
| **Estilo de pilotagem** (§5.3) | `atributos.aggression`, `smoothness`, `confianca` (os **exportados ao iRacing**) | *agressivo* / *cauteloso*; *suave e preciso* / *bruto, no limite*; *ousado* / *comedido* |
| **Idade**        | `idade`                                            | *jovem promessa* / *piloto no auge* / *veterano* (usar com parcimônia) |
| **Valor de mercado** | `contratos.salario_anual`, ranqueado           | topo → *a contratação mais cara / o nome mais valorizado do grid*; resto → não citado |
| **Material do carro** | `team_car.display_level` por equipe           | todos iguais → *grid equalizado, decisão no talento*; desigual → *larga com o melhor pacote* / *material mais modesto* (nomear a equipe, **nunca o número**) |
| **Relações do grid** (§5.5) | `contracts.get_former_teammates`, `rivalries`, dupla atual | *ex-parceiros reencontrados em equipes rivais*; *dupla de equipe*; *rivalidade de longa data* |
| **Título**       | campeão anterior (`PreviousChampions`)             | ficou → *volta para defender a coroa*; subiu → *deixou o trono vago ao subir de divisão*; inexistente → *categoria em refundação* |

### 5.2 O que é OCULTO (o jornalista só intui — e pode errar)

| Sinal | Fonte | Como aparece |
|-------|-------|--------------|
| **Ritmo bruto / potencial** | `atributos.skill` (o `driverSkill` exportado) | **NUNCA** afirmado como fato nem citado como número. Só vaza como **impressão hedgeada**: "aponta como o mais rápido no papel", "dizem os bastidores", "resta ver na pista". No rookie o skill costuma decidir — mas isso é segredo do paddock, não certeza do jornalista. |

**Modelo de percepção (como a imprensa ordena os favoritos).** A ordem dos `FAVORITOS`
**não** é o ranking de skill. É uma **percepção pública**, calculada assim (pesos
decrescentes): **resultados/currículo ≫ reputação/fama > experiência > estilo marcante**,
e o **skill entra só como um empurrão fraco e ruidoso**. Consequências desejadas:

- **Giulia**, já com um pódio, é percebida como favorita **acima** de um piloto de skill
  parecido mas sem resultado. O currículo fala mais alto que o talento oculto.
- Um **estreante de skill altíssimo** (que "no papel" deveria ganhar) é apresentado como
  **incógnita / aposta**, não como favorito consolidado — a imprensa ainda não tem prova.
- No **arranque de temporada**, quando ninguém tem resultado, a percepção desaba para
  fama/experiência + ruído → tudo genuinamente **em aberto** ("temporada de estreantes").
- Isso cria **ironia dramática**: o jogador sabe (pelo resto do jogo) o skill real; a
  matéria, não. Às vezes a imprensa subestima quem vai brilhar — e acerta as contas depois.

### 5.3 Traços de atributo (foco no que é exportado ao iRacing)

Dos 18 atributos, os que **se manifestam na pista via a IA do iRacing** são
`skill → driverSkill`, `aggression → driverAggression`, `smoothness → driverSmoothness`,
`confianca → driverOptimism`, `idade → driverAge` (mais pit crew/estratégia da equipe). O
estilo de um piloto é basicamente o vetor **agressão × suavidade × ousadia** — e isso é
**observável**, logo descritível com confiança:

| Atributo (exportado) | Extremo alto | Extremo baixo |
|----------------------|--------------|---------------|
| **aggression**       | *agressivo, briga por cada posição, dá o bote* | *cauteloso, jogo posicional, evita o contato* |
| **smoothness**       | *suave e preciso, poupa carro e pneu*          | *bruto, anda no fio da navalha, castiga o equipamento* |
| **confianca** (optimism) | *ousado, tenta o improvável*               | *comedido, calculista, não força a sorte* |

Regras dos traços:
- **Só o que se destaca.** Por piloto, no máximo **1–2 traços** — os mais extremos no grid
  (percentil alto/baixo). Piloto "mediano" em tudo não ganha traço (evita ruído).
- **Estilo ≠ ritmo.** Traço descreve **como** o piloto corre (público), nunca **quão
  rápido** ele é (oculto). "Agressivo" não implica "rápido".
- Reservado para **alguns** pilotos (os de personalidade marcante), não todos — dá cor sem
  virar ficha técnica.

### 5.4 Regras gerais da tradução

- **Percentis, não limiares fixos** — a régua se adapta a rookie, GT3, protótipo.
- **Um piloto = um mini-dossiê** (percepção + currículo + 0–2 traços), matéria-prima
  suficiente por nome sem excesso.
- **Sinais fracos somem.** Salário uniforme (rookie) → sem "mais valorizado". Material
  uniforme → "grid equalizado" citado uma vez, não por piloto.

### 5.5 Relações do grid — quem já correu com quem (player-agnostic)

Uma temporada não é um punhado de pilotos avulsos: é gente que **já se cruzou**. Esta
camada varre o grid inteiro procurando **pares de pilotos com história em comum** e os
transforma em enredo — **sem privilegiar o jogador** (as relações dele são só um caso
particular, tratadas como as dos outros).

Tipos de relação (fonte → token), do mais forte ao mais fraco:

| Relação | Fonte | Vira |
|---------|-------|------|
| **Ex-parceiros reencontrados** | `get_former_teammates`, filtrado a quem está no grid da categoria **e não é mais companheiro atual** | *X e Y, ex-companheiros na [equipe], agora em equipes rivais* |
| **Rivalidade estabelecida** | `rivalries` / `rivalry_episodes` (intensidade acima do gate), ambos no grid | *X e Y trazem uma rivalidade de [tempo]* |
| **Reencontro de dupla** | ex-companheiros que **voltaram** à mesma equipe | *X e Y reeditam a antiga parceria na [equipe]* |
| **Dupla atual** | mesmo contrato ativo | *X e Y formam a dupla da [equipe]* |

Como funciona (implementação):
1. Monte o **conjunto do grid** (pilotos da categoria). Só pares em que **os dois** estão
   nele entram — o leitor precisa poder acompanhar os dois nesta temporada.
2. Para cada piloto, `get_former_teammates` → intersecção com o grid → classifique:
   companheiro **atual** (mesma equipe hoje) vs **ex** (equipes diferentes hoje).
3. Cruze com `rivalries` para marcar pares que também são rivais (ex-parceiros que viraram
   rivais é o enredo mais suculento).
4. **Dedup por par não-ordenado** (X-Y = Y-X) e **cap em ~2–3 relações** — as mais fortes
   (rivalidade > ex-parceiros rivais > ex-parceiros > dupla atual). Relação fraca/isolada
   não entra; grid sem história compartilhada simplesmente não gera o bloco.

Gates de ruído: exigir **sobreposição real** de contrato no passado (a query já garante),
rivalidade **acima do limiar** de intensidade, e no máximo **uma** relação por piloto para
não repetir o mesmo nome em três ganchos.

---

## 6. Contrato de fatos (o que o backend manda ao servidor)

O bundle deixa de ser uma lista solta e passa a ser **estruturado por bloco**, já em
linguagem qualitativa. Formato de referência (PT; gerado no idioma ativo via rust-i18n):

Cada piloto é uma linha com campos separados por `|`, na ordem:
`nome | equipe | percepção | currículo | experiência | [traço de estilo] | [gancho]`.
São **cinco linhas em `FAVORITOS` e cinco em `PROMESSAS`** — dez dossiês, ou o grid
inteiro quando ele for menor que isso. A **ordem das
linhas em `FAVORITOS` é a percepção pública** (§5.2), **não** o skill. O skill oculto,
quando entra, vem só no bloco `INTUIÇÃO` como impressão hedgeada — nunca colado ao piloto.

```
TEMPORADA: Mazda Rookie, temporada 27 (2026). Calendário curto, 5 etapas.
ABERTURA: Motorsport Arena Oschersleben.
TÍTULO: o campeão anterior subiu de divisão — trono da categoria vago.
MATERIAL: grid equalizado (nenhuma equipe larga com pacote superior).
TESE SUGERIDA: temporada aberta, decisão no talento.

FAVORITOS (ordem = percepção pública):
- Martin Laurent | Track Day Heroes | favorito da imprensa | já sabe vencer | suave e preciso
- Nathaniel Turner | Northgate | cotado | já sabe vencer | agressivo, dá o bote
- Giulia Bianchi | Track Day Heroes | em alta | já provou o pódio | ousada
- Rodrigo Carvalho | Racing Academy Red | aposta cara, ainda a provar | ainda sem pódio | o nome mais valorizado do grid

PROMESSAS / INCÓGNITAS:
- Ramiro Ruiz | Racing Academy Red | já provou o pódio | dupla de Carvalho
- estreantes do grid | vários | sem histórico, pura expectativa

RELAÇÕES (histórias que o grid carrega):
- Nathaniel Turner e Jon Dahl foram companheiros na Northgate; hoje seguem juntos.
- Giulia Bianchi e Martin Laurent dividem a garagem da Track Day Heroes.
- (ex.: se houvesse) Fulano e Beltrano, ex-parceiros, reencontram-se em equipes rivais.

INTUIÇÃO (bastidor, hedgear — NÃO afirmar): nos números internos, Carvalho aparece como o
mais rápido no papel; resta a pista confirmar.

GRID: 12 pilotos, majoritariamente jovens em início de carreira.
```

- **Sem números.** Tudo já é token qualitativo. Sinal sem token relevante não entra.
- **`FAVORITOS` ordenado por percepção**, não por skill — repare que Carvalho (maior skill
  real) aparece **por último** entre os favoritos, como "aposta a provar", enquanto
  Laurent/Turner (com vitórias) e Giulia (com pódio) vêm à frente. É a assimetria do §5.2.
- **`INTUIÇÃO`** carrega o skill oculto como um sussurro de bastidor que o prompt deve
  hedgear ("no papel", "resta ver") ou até omitir — nunca vira fato.
- **`RELAÇÕES`** traz quem já correu junto (§5.5): ex-parceiros, duplas, rivalidades — do
  grid inteiro, **não** centrado no jogador. Vira storyline ("velhos conhecidos",
  "reencontro", "parceria de novo").
- **Blocos nomeados** (`TESE`, `FAVORITOS`, `PROMESSAS`, `RELAÇÕES`, `INTUIÇÃO`, `MATERIAL`,
  `TÍTULO`, `ABERTURA`) mapeiam direto nas seções do §4.
- **O piloto do jogador é só mais uma linha.** Sem marcação de "jogador" — no máximo um
  `| piloto da casa` como gancho opcional.

---

## 7. Persona e prompt do servidor

**Persona.** Redator-chefe de uma revista de automobilismo escrevendo a **prévia da
temporada**. Tom: informado, opinativo, com fôlego editorial — antecipação, não
retrospecto. Publicação séria, não fofoca.

**Regras duras no prompt (repetir explicitamente):**
- Escreva em **3ª pessoa**. **NUNCA** se dirija ao leitor ou a nenhum piloto em 2ª pessoa
  ("você/seu"). É proibido.
- **NÃO** cite números: nada de níveis, notas, salários em cifras, idades como estatística,
  potencial numérico. Descreva em palavras.
- **NÃO** invente fatos além dos fornecidos (resultados, patrocinadores, rivalidades).
- **Você é um jornalista, não onisciente.** A hierarquia de favoritos que você recebe é a
  **percepção pública** (baseada em resultados e reputação) — respeite essa ordem. Sobre
  quem é *de fato mais rápido*, **hedgeie**: "no papel", "promete", "resta ver na pista",
  "os bastidores apontam". Nunca declare com certeza que um piloto é mais rápido que outro.
- **Descreva o estilo com confiança, o ritmo com cautela.** Traços de pilotagem
  (agressivo, suave, ousado) são observáveis — pode afirmar. Potencial/ritmo é intuição —
  sempre incerto. Um estreante de grande aposta é uma **incógnita empolgante**, não um
  campeão anunciado.
- **Use o bloco `INTUIÇÃO` com pinça:** no máximo uma menção hedgeada, ou omita. Ele nunca
  vira manchete nem afirmação.
- **Teça as `RELAÇÕES` como enredo**, quando houver: "velhos conhecidos", ex-parceiros que
  se reencontram, uma rivalidade que a categoria herda. Trate-as como história do **grid**,
  em 3ª pessoa — não como algo que envolve "você". No máximo 1–2, sem forçar.
- Estruture em **manchete + linha-fina + 4 blocos** (§4). O bundle traz **dez dossiês**
  (5 em `FAVORITOS`, 5 em `PROMESSAS`) e **todos** devem aparecer no texto: cada um dos
  cinco favoritos ganha um **tratamento próprio** (uma ou duas frases — o que ele já fez,
  como pilota, o que se espera dele), e cada nome de `PROMESSAS` ganha ao menos **uma
  frase própria**. Uma lista de nomes despejados numa frase só não cumpre isso.
- Quando um piloto tiver traço de estilo, teça-o na descrição dele — e **não repita a
  mesma construção** de um piloto para o outro ("Fulano chega como X: Y" dez vezes é o
  fracasso que esta regra existe para evitar). Varie a forma da frase a cada nome.
- Comprimento-alvo: **450–600 palavras** — o payload manda o intervalo em `target_words`,
  e ele é a autoridade. Já foi 700–900, mas no playtest a matéria ficou longa demais para
  ser lida; abaixo de ~400, com dez pilotos a cobrir, vira legenda de foto. O intervalo
  atual dá 1–2 frases por favorito e uma frase por promessa, com fôlego de revista.
- Feche pela **etapa de abertura**.

**Formato de resposta** (JSON):
```json
{
  "headline": "Manchete de revista",
  "standfirst": "Linha-fina (subtítulo de uma frase)",
  "body": "Corpo em parágrafos, separados por linha em branco."
}
```
(A v1 devolve só `story`; o redesign adiciona `headline`/`standfirst` para a diagramação
do §8. Fallback: se vierem vazios, o front usa a 1ª frase do corpo como manchete.)

---

## 8. Layout na revista (UI)

Reaproveita o spread de duas páginas já existente (`NewsMagazineTab`), com hierarquia de
revista de verdade:

- **Página esquerda (a matéria):** manchete (fonte display/Kardust) → linha-fina → corpo
  em colunas. É o foco. Nomes de piloto e equipe coloridos (já implementado via
  `renderBulletinParagraph` + `teams`).
- **Página direita (o dossiê visual):** foto da pista de abertura + **grid de favoritos**.
  Aqui **números são permitidos** (é data-viz, não prosa): a lista pode mostrar potencial,
  experiência ("estreante" / "3 vitórias"), a marca da equipe. É o complemento factual da
  matéria.
- **Rodapé:** "Edição de Pré-Temporada · Temporada {ano}" + "Do mundo do Grid".

Fase 2: cards de piloto clicáveis (abrem o `DriverDetailModal`), bloco de "storylines"
com destaques, selo de "material equalizado / desigual" como ícone.

---

## 9. Fallback determinístico

Quando a IA está indisponível (endpoint fora, cooldown, offline), a aba **não** cai num
placeholder pobre nem em planilha. Um **montador determinístico** produz uma matéria
enxuta a partir dos **mesmos tokens qualitativos** do §6, respeitando **todas** as regras
editoriais (3ª pessoa, sem números, descritivo). Fica mais curto e mais template que a
versão de IA, mas ainda lê como revista. Assim a aba nunca "quebra o personagem".

(Isso inverte a v1, cujo placeholder era genérico. O fallback passa a ser um cidadão de
primeira classe.)

---

## 10. O que torna a aba "importante" (critérios de qualidade)

- **Cobertura:** o leitor termina conhecendo ≥5 nomes do grid, com expectativa de cada.
- **Enredo:** há uma tese clara da temporada, não uma lista.
- **Voz:** lê como uma publicação com opinião, em 3ª pessoa, sem uma vírgula de "você".
- **Fidelidade:** nada afirmado sem lastro nos dados; zero números crus na prosa.
- **Continuidade:** conversa com o resto do mundo vivo (campeão que subiu, rivalidade
  herdada, dupla de equipe) — não é um texto isolado.
- **Cadência:** uma matéria por temporada, sempre fresca, sempre a primeira coisa que o
  jogador lê ao entrar no ano.

---

## 11. Extensões futuras

- **Power rankings de meio de temporada** (reaproveita a camada de tradução com forma
  recente).
- **Dossiê profundo por piloto** ao clicar num nome.
- **Storylines persistentes** que a temporada "cobra" no debrief final ("a promessa se
  confirmou?").
- **Capa temática** por categoria/ano.
- **Imagens** de pilotos/equipes quando houver arte.

---

## 12. Deltas de implementação (a partir da v1)

1. **`commands/season_preview.rs`** — reescrever `build_season_preview_facts` para emitir
   o bundle estruturado do §6 (tokens qualitativos, blocos nomeados). Remover 2ª pessoa e
   números crus. Adicionar a **camada de tradução** (percentis de potencial, tiers de
   experiência/currículo, paridade de material, top de mercado).
2. **i18n `season_preview.*`** — trocar as chaves de "linha com número" por chaves de
   **token qualitativo**; remover `player_line*`/`SEU CENÁRIO` (2ª pessoa).
3. **`narrative/client.rs`** — `fetch_season_preview` passa a devolver
   `headline/standfirst/body` (ou manter `story` e derivar). Ajustar o struct de resposta.
4. **Servidor `/season-preview`** — prompt do §7 (persona + regras duras + formato).
5. **Front `NewsMagazineTab`** — renderizar manchete/linha-fina separadas; a coluna de
   favoritos pode exibir experiência além do potencial.
6. **Fallback determinístico** (§9) — montador em Rust a partir dos tokens.
