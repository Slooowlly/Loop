# R1 — `narrative/` está cego para o resto do mundo (a "Etapa B" nunca foi ligada)

**Área:** Rust · **Risco:** alto (muda texto gerado por IA) · **Conflita com:** R2 — não rode em paralelo

## O que foi encontrado

Este é o ponto de maior valor da varredura. O módulo `narrative/` é o motor que
transforma o resultado de uma corrida no **contexto curado** enviado ao servidor, que
escolhe o provedor que redige. Ele decide o que é interessante; a IA só redige em cima.

E ele só enxerga a simulação. Nada mais.

### Evidência

**1. `narrative/` importa exatamente um módulo do crate.** Varredura de todos os
`crate::*::` dentro de `src-tauri/src/narrative/`: 12 ocorrências, todas
`crate::simulation::`. Zero de rivalry, evolution, market, race_eval, car.

**2. O próprio módulo documenta a lacuna.** [`narrative/mod.rs`](../../src-tauri/src/narrative/mod.rs)
abre com `#![allow(dead_code)]` e diz, textualmente, que é a "Etapa A (MVP)" e que
os beats de carreira/forma — lesão, rookie, rivalidade-arco, forma das últimas 5
corridas — "entram na Etapa B, alimentados pela base do app". A Etapa B não chegou.

**3. O ponto de entrada já existe e vem vazio.**
[`narrative/contexto.rs:22`](../../src-tauri/src/narrative/contexto.rs) declara:

```rust
/// Pano de fundo (rookie/veterano, histórico, sequência...) já renderizado.
/// Vai numa seção "Contexto" — cor pra IA usar quando ajudar, sem virar manchete.
pub context_facts: &'a [String],
```

O único callsite de `build_race_context` é
[`commands/race/noticias/persistencia.rs:57`](../../src-tauri/src/commands/race/noticias/persistencia.rs).
**Verifique o que ele passa em `context_facts`** — na varredura pareceu não incluir
nada de rivalidade/forma/lesão-arco.

**4. Os dados já estão carregados, em outro lugar.**
[`commands/ai_news/fatos.rs:293`](../../src-tauri/src/commands/ai_news/fatos.rs) tem
um bloco explicitamente chamado "rivalidade VIVIDA (nemesis + rivais) + captura do
DUELO decidido". Ele consulta `db::queries::player_nemesis::get_current_nemesis`,
`rivalry::get_pilot_rivalries`, `race_eval` e os breakdowns. Tudo isso já roda no
mesmo pós-corrida — mas alimenta o **debrief do jogador**, não o **boletim do grid**.

Também em [`commands/race/fatos.rs`](../../src-tauri/src/commands/race/fatos.rs):
`rivalry::get_pilot_rivalries` + `intensity_level(...).label()` + `race_eval::evaluate`.

## Por que importa

O boletim de notícias do grid é escrito por uma IA que recebe só "quem ganhou, quem
bateu, quem quebrou". Toda a máquina de mundo vivo que o projeto construiu —
rivalidades entre pilotos e entre construtores, vínculo piloto-equipe, moral, forma,
lesão, arco de rookie — está invisível para ela. A consequência prática é notícia
genérica: descreve o resultado, não conta a história.

E o custo de ligar é baixo, porque o campo receptor existe e os dados já são
carregados no mesmo ponto do fluxo.

## Armadilhas conhecidas

1. **Custo por token.** O contexto vai para um servidor, que reparte as chamadas entre
   DeepSeek (principal) e Gemini (pico e fallback) — ver `narrative/client.rs`. Encher
   `context_facts` sem critério aumenta o payload de toda corrida. `narrative/` tem
   um limiar de relevância e `beat_count` como proxy de densidade — a ligação
   precisa respeitar essa curadoria, não furá-la.
2. **A curadoria é o produto.** O `mod.rs` é explícito: "a inteligência de 'o que é
   interessante' mora AQUI, não na IA". Despejar fatos crus em `context_facts`
   contraria o design. O caminho provavelmente certo é criar **beats** novos com
   peso, não strings soltas.
3. **`persistencia.rs` roda dentro do fluxo pós-corrida**, possivelmente numa
   transação. Adicionar queries ali tem custo e ordem — confira o que já está aberto.
4. **i18n backend.** Prosa nova vai em `src-tauri/locales/*.yml`, com paridade
   pt-BR/en-US travada por teste. E o locale é **global do processo**: teste Rust
   que troca idioma precisa de `#[serial]`.
5. **Não confunda com R2.** O `ai_news` é o debrief do jogador (1ª pessoa). O
   `narrative` é o boletim do grid (voz de revista). A ligação é de *dados*, não de
   voz — não unifique os textos.

## O que eu quero da segunda análise

1. **Confirme a lacuna.** Leia `commands/race/noticias/persistencia.rs` e diga
   exatamente o que hoje vai em `context_facts`, `injuries` e `incidents`. Se já
   houver algo de rivalidade/forma, minha leitura está errada e quero saber.
2. **Mapeie a sobreposição com `ai_news/fatos.rs`.** Quais consultas os dois fluxos
   fariam em comum? Rodam no mesmo ponto do pós-corrida? Dá para carregar uma vez e
   servir aos dois, ou são momentos diferentes?
3. **Desenhe a Etapa B como beats, não como strings.** Proponha os `BeatKind` novos
   (rivalidade-arco, forma recente, lesão com arco, estreia/aposentadoria, vínculo
   piloto-equipe, moral de equipe) com peso relativo aos beats existentes. Leia
   `narrative/beats.rs` para calibrar na escala que já existe.
4. **Orçamento.** Estime o crescimento do payload por corrida com a proposta, e diga
   como o limiar de relevância deve ser ajustado para o texto não inchar.
5. **Ordem de implementação.** Qual beat entrega mais história por unidade de risco?
   Quero começar por um só, ver o texto gerado, e então decidir os demais.
6. **O `#![allow(dead_code)]` no `mod.rs` ainda se justifica?** Se a Etapa B for
   ligada, ele deveria sair — e o que quebra quando sair?

Não aplique nada ainda — quero ler a análise antes.
