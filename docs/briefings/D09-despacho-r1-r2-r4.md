# D-09 — Guia de despacho: R1, R2 e R4

**Área:** Rust · **Risco:** varia por item · **Tamanho:** G no conjunto

Este arquivo **não substitui** os briefings — eles já existem, são autocontidos e são o
que você deve entregar à sessão de trabalho:

- [R1 — `narrative/` cego, a Etapa B nunca ligada](../varredura-acoplamento/R1-narrative-etapa-b.md)
- [R2 — três motores de tese](../varredura-acoplamento/R2-tres-teses.md) + [**a segunda análise, já feita**](../varredura-acoplamento/R2-analise.md)
- [R4 — `hierarchy/` com estado rico e sem consumidor](../varredura-acoplamento/R4-hierarchy-sem-consumidor.md)

O que este guia acrescenta: **em que estado cada um está hoje** (o R2 andou desde que os
briefings foram escritos) e **em que ordem despachar**.

---

## Regra dura, antes de qualquer coisa

**R1 e R2 tocam os mesmos arquivos — nunca rode os dois em paralelo.** O R1 vai criar
beats novos em `narrative/`; a fatia P1 do R2 vai extrair predicados usados por
`narrative/tese.rs` e `narrative/beats.rs`. Duas sessões mexendo ali ao mesmo tempo
produzem conflito garantido.

R4 é independente dos dois e pode rodar em paralelo com qualquer um.

---

## R2 — o mais adiantado, e o que achou um bug de verdade

**Estado: segunda análise concluída. P0 já implementado.**

A suspeita original do briefing era "há três motores de tese duplicados, unifique". A
análise ([`R2-analise.md`](../varredura-acoplamento/R2-analise.md), 241 linhas) devolveu
um veredito mais afiado, e é o que importa:

> A duplicação das **teses** é aceitável e deve ficar. A duplicação dos **sinais** não é —
> e não é teórica: há um defeito real de escala de unidade que já produz o debrief e o
> boletim classificando o mesmo piloto na mesma corrida de formas contraditórias.

Concretamente, o mesmo DNF pode ser "batida" para o boletim e "mecânico" para o debrief,
porque um lê `IncidentType` e o outro roda regex sobre a string do motivo. E "remontada"
tem **quatro** limiares diferentes no código: `gained >= 5` (debrief), `gained >= 8 &&
finish <= 6` (boletim/tese), `gained > 0` (boletim/beat), `gained >= 4` (`race_eval`).

**O P0 já foi feito** — `build_merit_field` em
[`commands/race/merito.rs`](../../src-tauri/src/commands/race/merito.rs) virou a
construção única do campo de mérito, no commit `2c85f44`.

**O que resta:**
- **P1 (fazer, ~4–6h):** extrair `dnf_kind` e os predicados nomeados; um limiar por
  conceito. **Toca `narrative/` — coordene com o R1.**
- **P2 (opcional, ~1h):** frontend lê o `assessment` persistido em vez de recalcular
  `dismal`. Não toca `narrative/`; pode ir a qualquer momento.

**Não fazer** (a análise é explícita): um `RaceFacts` único para os três motores, e
qualquer fusão das vozes. As três teses estão certas em existir — o debrief é 1ª pessoa,
o boletim é voz de revista, a prévia não tem resultado nenhum.

---

## R1 — o de maior valor de produto, e o mais delicado

**Estado: briefing escrito, segunda análise não feita.**

O `narrative/` é o motor que decide o que é interessante numa corrida antes de mandar o
contexto para a IA redigir. Ele importa **exatamente um** módulo do crate: 12 ocorrências
de `crate::*::`, todas `crate::simulation::`. Zero de rivalry, evolution, market,
race_eval, car.

O resultado prático: o boletim do grid é escrito por uma IA que recebe só "quem ganhou,
quem bateu, quem quebrou". Toda a máquina de mundo vivo — rivalidades, vínculo
piloto-equipe, moral, forma, lesão, arco de rookie — está invisível para ela. A notícia
descreve o resultado; não conta a história.

O que torna isso barato: **o campo receptor já existe** (`context_facts` em
`narrative/contexto.rs:22`) e **os dados já são carregados no mesmo ponto do fluxo** —
`commands/ai_news/fatos.rs:293` tem um bloco de rivalidade vivida que consulta nemesis,
rivalidades e `race_eval`, mas alimenta o debrief do jogador, não o boletim.

**Por que é delicado:** o briefing pede que a Etapa B seja desenhada como **beats com
peso**, não como strings soltas em `context_facts`. A curadoria é o produto — o próprio
`mod.rs` diz que "a inteligência de 'o que é interessante' mora AQUI, não na IA".
Despejar fatos crus contraria o design e ainda infla o payload por token de toda corrida.

**Despache pedindo a análise primeiro.** O briefing termina com seis perguntas e um
"não aplique nada ainda" — respeite isso.

---

## R4 — o que pode terminar em "corte o módulo"

**Estado: briefing escrito, segunda análise não feita. Independente de R1 e R2.**

`hierarchy/` modela a política interna da equipe: N1, N2, o duelo, a tensão acumulada, o
gatilho de inversão. Onze funções públicas em `orders.rs`, mais três em `transition.rs`.
Consumidores externos: **dois** — um de lógica (`process_hierarchy_for_category`, por
corrida) e um de sanidade (`validate_and_normalize_team_hierarchies`).

O estado que ele produz a cada corrida quase não realimenta o mundo. O mercado não lê
`tensao`, `status` nem `n2_win_rate`: um N2 que vence o duelo interno a temporada inteira
não pede saída, não fica mais caro, não vira alvo de assédio. A narrativa não vê
hierarquia nenhuma (ver R1) — e "briga interna na equipe X" é uma das manchetes mais
naturais do automobilismo.

**Duas advertências que mudam o resultado da análise:**

1. **Ligar hierarquia ao mercado é design de equilíbrio, não refactor.** Mexe na economia
   de contratos. O briefing pede análise de risco de realimentação positiva — hierarquia →
   moral → simulação → resultado → hierarquia já é um ciclo fechado, e adicionar mercado e
   motivação fecha mais.
2. **"Cortar o módulo" é uma resposta legítima** e o briefing pede explicitamente que ela
   esteja na mesa. Se o sistema não tem consequência e ninguém o vê, simplificar pode
   valer mais que ligar.

A primeira pergunta do briefing é justamente se o frontend já exibe hierarquia em algum
lugar (`MyTeamTab`? `DriverCard`?) — se exibir, o sistema já tem valor como informação e
a conversa muda inteira.

---

## Ordem sugerida

| # | item | por quê |
|---|---|---|
| 1 | **R2 / P1** | Já tem análise pronta e um bug confirmado em produção. Menor incerteza, valor imediato. Faça antes do R1 para não conflitar. |
| 2 | **R1** (análise) | Maior valor de produto do conjunto. Só depois que o P1 do R2 assentar `narrative/`. |
| 3 | **R4** (análise) | Independente — pode rodar em paralelo com qualquer um dos dois. Despache quando houver banca. |

R2/P2 (frontend) é avulso e cabe em qualquer buraco.

---

## Aviso de método, válido para os três

O README da varredura é explícito: a contagem de referências foi feita por `grep`
excluindo o diretório do próprio módulo, e isso **gera falsos positivos** — uma função
re-exportada e consumida por um irmão aparece como "sem chamador".

Este repositório já pagou por esse erro em outra forma: a primeira varredura de roadmap
concluiu que "o mercado não tem tela" porque leu um arquivo de 9 linhas em vez de rastrear
os comandos Tauri. Havia ~2700 linhas de UI de mercado. O registro está em
[roadmap.md](../roadmap.md), seção "Correção de uma leitura anterior".

**Cada um dos três briefings termina pedindo confirmação da própria evidência antes de
virar código. Isso não é formalidade.**
