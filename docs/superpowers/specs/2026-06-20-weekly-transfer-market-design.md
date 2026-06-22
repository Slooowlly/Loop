# Design — Janela de Transferências Semanal (mercado realista)

**Status:** design aprovado nas decisões grandes; pendente de detalhamento numérico e implementação.
**Data:** 2026-06-20.
**Substitui:** o casamento guloso vaga-por-vaga de `market/pipeline.rs::run_market` (loop sequencial: primeira equipe que oferta + piloto aceita, fecha).

---

## 1. Objetivo

Trocar o mercado instantâneo e guloso por uma **janela de transferências realista**: multi-semana, leilão de dois lados, guerra de lances dentro do orçamento, e **suspense interativo** pro jogador (aceita uma oferta → espera → foi escolhido ou virou "resto").

## 2. Decisões travadas (usuário)

1. **Totalmente interativo** — o jogador avança semana a semana, vê ofertas, aceita, vê resultado.
2. **Aceita só UMA e arrisca** — o piloto aceita a melhor oferta, recusa as outras; se a equipe escolher outro, ele vira "resto". Suspense máximo.
3. **Até esvaziar** — a janela roda até uma semana passar sem nenhuma assinatura nova (mercado estável); duração variável.
4. **Shortlist de 2-3 alvos por vaga** — a equipe persegue 2-3 pilotos ao mesmo tempo e escolhe entre os que aceitam (fonte do "escolheu ele ou outro").
5. **Rede de segurança por skill** — ao fechar a janela, craque sempre acha vaga (equipe abre exceção); só mediano/fraco pode ficar de fora 1 temporada.

## 3. O ciclo de uma SEMANA

```
1. OFERTAS    Cada equipe com vaga atualiza sua shortlist (2-3 alvos por vaga) e manda
              OFERTA (salário) a cada alvo. Alvo perdido na semana passada → SOBE o lance.
2. RESPOSTAS  Cada piloto livre/"resto" pontua TODAS as ofertas recebidas e ACEITA UMA
              (a melhor) se cruzar o limiar dele; recusa o resto. Senão, segura (espera).
              [JOGADOR: a UI mostra as ofertas; ele escolhe aceitar uma ou esperar.]
3. RESULTADOS Cada equipe assina o nº 1 (por preferência da equipe) entre os que aceitaram
              a oferta DELA. Os outros que aceitaram → "resto" (recusaram outras à toa).
              [JOGADOR: vê se a equipe que ele aceitou o escolheu ou pegou outro.]
4. ROLLOVER   Vagas abertas + "resto" + ainda-livres seguem pra próxima semana.
→ FECHA quando uma semana inteira passa SEM assinatura nova.
   Pós-fecho: rookies/promoções de feeder preenchem vagas; pilotos sem vaga → rede de
   segurança por skill (craques colocados em vaga/exceção; medianos/fracos podem ficar
   de fora 1 temporada → reentram na próxima janela).
```

É **dois lados** de verdade:
- Piloto cobiçado recebe ofertas de **várias equipes** → escolhe uma → as outras o perderam e **sobem o lance** semana seguinte.
- Equipe com **vários alvos** → escolhe um → os outros viram "resto".

## 4. Guerra de lances (salário livre, dentro do orçamento)

- **Teto de salário da vaga** = f(finanças da equipe) — máximo que a equipe paga por aquele assento (orçamento − compromissos já assumidos).
- **Oferta inicial a um alvo** = "valor de mercado" do piloto = f(skill, tier, idade, mídia), clampado a [piso, teto da vaga].
- **Escalada:** a cada semana que a equipe NÃO conquista um alvo que ainda quer, sobe a oferta àquele alvo um passo (ex.: +8-12%) rumo ao teto. Para no teto.
- **Competição:** duas equipes no mesmo piloto → ambas escalam → o piloto recebe ofertas crescentes e aceita a melhor quando cruza o limiar.
- **Limiar do piloto:** salário compensa carro/tier piores ATÉ UM PONTO — há um "piso de dignidade" (um craque não vai pra lanterna por qualquer salário). O jogador vive isso recebendo: segurar pra ganhar mais arrisca a equipe fechar com outro ou recuar.

## 5. As duas decisões da semana (algoritmo)

**Preferência da EQUIPE sobre pilotos** (quem entra na shortlist e quem assina entre os que aceitam): reaproveita o `candidate_score` atual (skill 0.4 + consistência 0.2 + visibilidade 0.2 + idade + mídia), mais aptidão (fit) e affordability. Shortlist = top 2-3 elegíveis.

**Preferência do PILOTO sobre ofertas** (qual aceita): adapta o `evaluate_proposal`, comparando MÚLTIPLAS ofertas e escolhendo a de maior score acima do limiar. **Pesos novos** — o piloto confia mais no HISTÓRICO demonstrado que na promessa não-verificável do carro:
- **prestígio histórico da equipe ~22** (NOVO — substitui a "reputação" estática; ver §5b)
- **carro ~18** (REDUZIDO de 30 — o piloto não sabe se o carro é o que prometem)
- tier 25 · papel 10-15 · salário 15 · ± personalidade
- **bônus de slam** (categoria-alvo, §8) · **bônus de marca** (same-brand tiers 0-1, §6d)

## 5b. Prestígio competitivo da equipe (histórico de 10 anos)

O piloto trusta o que a equipe DEMONSTROU, não a promessa do carro. Prestígio = competitividade no **campeonato de construtores** nas **últimas 10 temporadas** (dado em `team_season_archive`):
- **Título** vale muito; **pódio (2º/3º)** vale médio; resto ~0.
- **Peso por recência:** ano atual conta cheio, decaindo até ~10% há 10 anos (3º recente conta; 3º há 20 anos é ignorado).
- Normaliza pra 0-100 → entra no score do piloto (peso ~22).

**Portão de elite (SUAVE, decisão do user):** craque (skill ≥ 80) **desconta forte** uma equipe sem histórico competitivo recente (sem nenhum pódio nos últimos 10 anos), mas um **carro excelente + salário gordo ainda podem convencer** — não é recusa dura.

## 6. Rede de segurança no fecho da janela

Duas redes distintas:

**6a. IA — por skill.** Pilotos-IA sem vaga, ordenados por skill:
- **Craque** (skill alto): alguma equipe abre exceção / vaga de baixo → sempre corre.
- **Mediano/fraco:** se não sobrou vaga digna, fica **de fora 1 temporada** (sem contrato), reentra na próxima janela. Abre espaço pra rookies. Limiar de "craque" a calibrar (~80+ ou top-N por categoria).

**6b. JOGADOR — garantia de porta (na PRÓPRIA categoria).** O jogador NUNCA é trancado fora do grid contra a vontade. Na última semana, se não fechou nada, o sistema **garante ≥1 vaga pra ele NA CATEGORIA QUE ELE QUER CORRER** — renovação com o time atual, ou a melhor vaga que sobrar **na própria categoria**. NUNCA uma sobra de outra categoria que ele nem queria (decisão do user). Ele pega ou recusa, mas a porta digna existe.

## 6c. Zona de acesso — promoção de não-campeões (narrativo)

Hoje só o **campeão** sobe (e por equipe). No mercado novo, a **shortlist de uma vaga da divisão de cima** passa a considerar os melhores da divisão de baixo, dando chance a quem NÃO foi 1º:

- **Pool de candidatos à promoção** = **top 4 do campeonato** da divisão de baixo **+ qualquer piloto com skill alto** (um talento que foi mal no campeonato ainda é cobiçado).
- **Garantia (decisão do user):** o **pódio (1º, 2º, 3º)** da divisão de baixo recebe **ao menos UMA proposta** pra subir — mesmo que de um time fraco. Fechar ou não é com eles + o leilão.
- O **4º e os talentos** são **elegíveis** (entram nas shortlists), sem garantia.
- Convive com a promoção por equipe: o time campeão sobe carregando seus pilotos; E vagas de cima podem ser preenchidas por esses pilotos em ascensão.

**Efeito narrativo:** vários rivais da divisão de baixo podem subir junto com o jogador (outros não) → a divisão nova tem caras conhecidos e novos. Quebra o problema de "só o 1º tinha chance".

## 6d. Escada de marca (afinidade Mazda→Mazda)

A IA **respeita a escada da própria marca**: campeão da Mazda Rookie quer subir pra **Mazda Cup** (`mazda_amador`), não Toyota Cup. Só cruza de marca **quando não sobrou vaga na própria marca**.
- **Afinidade forte** no casamento: oferta da MESMA marca leva um bônus grande no score do piloto-IA; vaga de uma marca prioriza pilotos da mesma marca na shortlist. Cross-brand só como fallback (sem same-brand disponível).
- **Jogador — estrutura de ofertas (decisão do user):** recebe **até 3 ofertas da própria categoria + 1 da alternativa**. O nº de ofertas da própria varia (1-3, conforme o interesse das equipes — nem sempre 3 times o querem). A oferta **cross-brand só aparece na SEMANA 1** (1º lance) como tentação cedo; não pegou, sumiu. O fallback (§6b) é SEMPRE na própria categoria — a alternativa nunca é "o que sobrou".
- Atua só nos **tiers 0-1** (split Mazda/Toyota). De BMW/Production/GT4 pra cima as marcas se fundem → afinidade não se aplica. Marca derivada do id da categoria (`mazda_*` vs `toyota_*`).

## 6e. Rivais seguem o jogador (entourage narrativo)

Quando o jogador troca de categoria, **garante que 1-2 ex-competidores apareçam na divisão nova dele** — mantém caras conhecidos enquanto ele sobe.
- **Quem (decisão do user):** mistura de **intensidade de rivalidade** (sistema de rivalidade existente) **+ skill** (quem brigou muito com o jogador E tem nível pra subir).
- **Garantia (decisão do user):** 1-2 são garantidos (não só tendência). Implementação: viés forte desses pilotos pra categoria nova do jogador (entram nas shortlists de lá + preferem aquela categoria); se o mercado orgânico não colocar 1-2, **força** a colocação no fecho pra honrar a garantia.
- É um viés NARRATIVO só em torno do jogador (a IA-vs-IA segue as regras normais).

## 6f. Categorias especiais NA JANELA (Production / Endurance) — decisão do user

⚠️ **CRÍTICO:** o sistema de especiais foi alterado a ponto da **janela de convocação NÃO existir mais**. Logo, a janela de transferências **PRECISA preencher as vagas de Production e Endurance** — senão fica um buraco (bug: ninguém preenche essas categorias).
- Vagas de Production/Endurance entram na janela **com classe** (Production: mazda/toyota/bmw; Endurance: gt4/gt3/lmp2). O casamento respeita a classe (vaga de classe X só casa com piloto que vai correr a classe X).
- O **slam-chasing** de Production/Endurance passa a funcionar pela própria janela (piloto assina com times de classes diferentes ao longo das temporadas → fecha o slam multiclasse).
- **IMPLEMENTAÇÃO:** verificar/alinhar com o estado ATUAL do código de especiais (`convocation/*`, `uses_regular_contracts`, `is_especial`) antes de construir — o sistema antigo de convocação pode ter resíduos a remover/adaptar.

## 7. Arquitetura

O mercado deixa de ser uma função instantânea e vira **estado persistido e avançado em passos**:

- **Estado novo (tabelas):** `transfer_window` (temporada, semana atual, status open/closed), `market_offers` (oferta: equipe, piloto, vaga, salário, semana, status pending/accepted/signed/dumped/withdrawn), e estado por piloto (free/accepted-pending/signed/resto).
- **Comandos novos (Tauri):** `get_transfer_window_state`, `player_respond_offer(offer_id, accept)`, `advance_transfer_week`.
- **Fases de IA** (ofertas / respostas-da-IA / confirmação) rodam a cada avanço de semana; a **resposta do jogador** é interativa entre avanços.
- **Integração na pré-temporada:** em vez de `run_market` resolver tudo, ele **inicializa a janela**. O jogador avança semanas até fechar; depois o resto da pré-temporada (rookies, finalize) roda.
- **Avanço (decisão do user):** o jogador avança **TODA semana manualmente** (sem auto-pular) — acompanha tudo de perto, máxima imersão.
- **UI nova:** tela "Janela de Transferências" — ofertas do jogador (equipe, categoria, papel, salário, dica de "você é 1 de N na shortlist"), botão aceitar/esperar, o avanço de semana, e o **feed do mercado** (ver abaixo).

**Feed do mercado — ESCOPADO ao horizonte do jogador (decisão do user):** mostra só o que ele "conhece" — nada de grandes nomes de GT4/GT3 se ele está longe de lá.
- **Rivais do jogador** (onde quer que assinem).
- **Pilotos favoritados** pelo jogador → ⚠️ DEPENDÊNCIA: precisa implementar um sistema de "favoritar piloto" (ainda não existe; o user quer).
- **Grandes nomes / contratações da SUA categoria atual e da PRÓXIMA** (ex.: venceu a Rookie → vê as contratações da Cup, pra onde ele vai).
- **NÃO** mostra categorias distantes (2+ tiers acima) — ele não conhece ninguém lá.

## 8. Integração com o slam-chasing (já implementado)

O mercado novo **absorve** o passe prioritário do slam. Não precisa de passe separado:
- O cérebro `slam_ambition::decide` continua dando a **categoria-alvo** do piloto ambicioso.
- Isso vira um **bônus forte** no score de ofertas do piloto (§5, preferência do piloto): ele prefere/aceita ofertas na categoria-alvo.
- Como ele é agente livre e super-qualificado, equipes da categoria-alvo o colocam na shortlist → ele cai lá naturalmente, via o leilão de dois lados.
- A **renovação-consciente** (slam-chaser recusa renovar p/ trocar de categoria) continua valendo — ele entra na janela como agente livre.
- Resultado: o passe prioritário (`apply_slam_priority_pass`) pode ser **removido** quando a janela entrar; sua função vira o bônus de categoria-alvo no score.

## 9. Fluxo do jogador (suspense)

```
Semana N (tela):
  - Vê ofertas recebidas (equipe, categoria, papel, salário; lance subiu vs semana passada?).
  - Escolhe: ACEITAR uma  |  ESPERAR (recusa todas, aposta em oferta melhor).
  → avança →
  RESULTADO:
    - Assinou? → fechou, sai da janela.
    - Não escolhido? → "resto" → semana N+1 com novas ofertas (talvez lances maiores).
  Janela fecha → assinado, ou rede de segurança (craque acha vaga; senão fica de fora).
```

## 10. Fases de implementação (proposta)

1. **Fase 1 — Motor IA-only.** Janela semanal + leilão de dois lados + lances, SEM jogador (jogador resolvido pela IA temporariamente). Validar por simulação multi-temporada (grids plausíveis, sem craque desempregado, lances coerentes). Remove o loop guloso.
2. **Fase 2 — Interatividade + estado persistido.** Tabelas, comandos, avanço de semana, resposta do jogador.
3. **Fase 3 — UI + feed + polish.** Tela da janela, suspense visual, integração do slam (bônus de categoria-alvo; remover o passe prioritário).

## 11. Calibração — NÚMEROS FECHADOS (score do piloto: tier 25 + prestígio histórico 22 + carro 18 + salário 15 + papel 10-15; aceita ≥ ~50)

**Leilão / lances:**
- Passo do lance: a cada semana que a equipe não conquista o alvo, fecha **30% da folga até o teto**.
- Teto do lance: salário que o **orçamento da equipe** comporta (menos compromissos já assumidos).
- Shortlist por vaga: **3 nas categorias de cima (tiers 3+) / 2 nas de baixo (tiers 0-2)**.

**Aceitação do piloto (pesos do score):**
- **Prestígio histórico da equipe: 22** — competitividade dos **últimos 10 anos** (título alto, pódio médio, com recência), normalizado 0-100. Substitui a reputação estática. O que o piloto mais confia.
- **Carro: 18** (reduzido de 30 — promessa não-verificável).
- tier 25 · salário 15 · papel 10-15 · ± personalidade.
- **Portão de elite SUAVE:** craque (skill ≥ 80) desconta forte time sem pódio nos últimos 10 anos, mas carro excelente + salário gordo ainda convencem.
- Piso de dignidade: piloto-IA **recusa cair 2+ tiers abaixo** do seu nível de skill, por qualquer salário (jogador tem a garantia de porta §6b).

**Afinidade de marca (tiers 0-1):**
- IA: regra **dura** — só considera cross-brand (Mazda↔Toyota) se **não há** oferta same-brand.
- Jogador: **até 3 ofertas da própria categoria + 1 da alternativa** (varia 1-3 próprias); a cross-brand só na **semana 1**; fallback sempre na própria categoria.

**Zona de acesso (§6c):**
- Pool elegível à promoção: **top 4 do campeonato** da divisão de baixo **+ qualquer skill ≥ 72**.
- Garantia: **pódio (1º/2º/3º)** recebe ≥1 proposta pra subir.

**Redes de segurança:**
- "Craque" (IA sempre acha vaga): **skill ≥ 80**. Abaixo → pode ficar de fora 1 temporada.
- Jogador: garante **≥1 vaga** na última semana (a melhor que sobrar).

**Entourage — rivais seguem o jogador (§6e):**
- **1-2 garantidos** na divisão nova do jogador.
- Escolha por score = **60% rivalidade + 40% skill**.

## 12. Questões resolvidas (as 4 finais)

- **Especiais (Production/Endurance):** a janela TRATA elas, por classe (§6f) — a convocação não existe mais, sem isso seria bug.
- **Backward-compat:** sem problema — mercado antigo era atômico, nenhum save fica "no meio". Migração cria as tabelas (vazias); a janela ativa no próximo mercado. Nada a decidir, só executar.
- **Semanas:** até esvaziar (1 semana sem assinatura fecha) + teto duro ~10; jogador avança TODA semana manualmente (§7).
- **Feed:** moderado e ESCOPADO ao horizonte do jogador — rivais, favoritados, e grandes nomes da categoria atual + próxima; nada de categorias distantes (§7).

## 13. Dependências / pré-requisitos

- **Sistema de "favoritar piloto"** (NÃO existe ainda; user quer implementar) — alimenta o feed do mercado (§7). Construir antes ou junto da Fase 3 (UI/feed).
- **Verificar o estado atual do código de especiais** (`convocation/*`, `is_especial`, `uses_regular_contracts`) antes da Fase 1 — alinhar a janela com o que sobrou do sistema de convocação (§6f).
```
