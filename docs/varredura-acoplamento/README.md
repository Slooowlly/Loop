# Varredura de acoplamento — 2026-07-24

Levantamento feito após a sequência de refatorações que quebrou `mod.rs` gordos em
módulos irmãos (narrative, rivalry, calendar, constants). Com o código mais fatiado,
ficou visível o que está duplicado e o que deveria estar ligado e não está.

**Cada arquivo aqui é um briefing autocontido** para ser entregue a uma sessão
separada. Todos terminam pedindo uma segunda análise detalhada — a varredura
original foi rasa por design (grep + leitura pontual), e várias conclusões precisam
de confirmação antes de virar código.

## Índice

| # | Briefing | Área | Conflita com |
|---|---|---|---|
| F1 a F4 | Helpers de pista e clima, `formatLap` e paleta de gráficos, `getReadableTeamColor`, `IN_TAURI` | Frontend | — |
| R1 | [`narrative/` cego — Etapa B nunca ligada](R1-narrative-etapa-b.md) | Rust | R2 |
| R2 | [Três motores de tese](R2-tres-teses.md), com [a segunda análise e o veredito](R2-analise.md) | Rust / Frontend | R1 |
| R3 | [`public_presence` vs `market/visibility`](R3-tiers-duplicados.md) | Rust | — |
| R4 | [`hierarchy` — estado rico sem consumidor](R4-hierarchy-sem-consumidor.md) | Rust | — |
| R5 | [Caminhos paralelos vivos só pelos testes](R5-caminhos-paralelos.md) | Rust | — |

Os briefings marcados como conflitantes tocam os mesmos arquivos — **não rode R1 e R2
em paralelo**. Os demais são independentes entre si.

Os quatro briefings de frontend foram resolvidos no commit `2c85f44` e os arquivos saíram
daqui; ficam na linha acima só como registro do que a varredura cobriu.

## Estado em 11/08/2026

Os cinco briefings de Rust foram reconferidos contra o código atual, na mesma sessão
justamente para não ter duas frentes editando `narrative/`. Cada arquivo abre agora com um
bloco "Situação em 11/08/2026" com a classificação afirmação por afirmação.

| # | Estado | Em uma linha |
|---|---|---|
| R1 | **PARCIAL** | A parte técnica fechou (mortos removidos, `allow` fora, contratos honestos). Promover forma/lesão/marcos a beat é **design**. |
| R2 | **RESOLVIDO** no Rust | Campo de mérito único e `race_signals.rs` em produção. O item de frontend (`dismal` recalculado em JS) fica **opcional**. |
| R3 | **RESOLVIDO — fechado** | O enum de tier de equipe não existe mais, e o comentário obsoleto foi corrigido. Não recriar. |
| R4 | **PARCIAL** | A premissa "sem consumidor" caiu: há quatro consumidores, incluindo o mercado via `contract.papel`. Código morto removido; a falta de consequência nova é **design**, o eixo de tensão parado é **calibração**. |
| R5 | **RESOLVIDO** | Três funções legadas já não existem; `run_market` fica, com `cfg` e contrato escritos. |

A árvore vazia do apêndice de sujeira trivial foi removida em 11/08/2026. A nota de método
continua valendo — a contagem por `grep` gerou falso positivo de novo, desta vez no R4.

## Apêndice — sujeira trivial (não precisa de briefing)

`src-tauri/src/src-tauri/src/evolution/pipeline/tests/` era uma árvore de diretórios
vazia, artefato de um comando rodado com o path duplicado. Nenhum arquivo dentro,
nenhuma referência no crate. **Removida em 11/08/2026.**

## Nota de método (importante para quem receber estes briefings)

A varredura usou contagem de referências via `grep` excluindo o diretório do próprio
módulo. Isso **gera falsos positivos**: uma função re-exportada e consumida por um
irmão do mesmo módulo aparece como "sem chamador".

Já caí nessa uma vez durante a varredura — `narrative::build_beats` e
`narrative::select_race_thesis` pareciam mortos, mas são chamados por
[`narrative/contexto.rs:39`](../../src-tauri/src/narrative/contexto.rs) e
[`:95`](../../src-tauri/src/narrative/contexto.rs). Toda alegação de "não tem
chamador" nestes briefings foi conferida à mão, mas confira de novo antes de apagar
qualquer coisa.
