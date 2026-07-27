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
| F1 | [Helpers de pista e clima duplicados](F1-helpers-pista-clima.md) | Frontend | — |
| F2 | [`formatLap` e paleta de gráficos](F2-graficos-corrida.md) | Frontend | — |
| F3 | [`getReadableTeamColor` triplicado](F3-cor-legivel-equipe.md) | Frontend | — |
| F4 | [`IN_TAURI` em 11 arquivos](F4-in-tauri.md) | Frontend | — |
| R1 | [`narrative/` cego — Etapa B nunca ligada](R1-narrative-etapa-b.md) | Rust | R2 |
| R2 | [Três motores de tese](R2-tres-teses.md) | Rust / Frontend | R1 |
| R3 | [`public_presence` vs `market/visibility`](R3-tiers-duplicados.md) | Rust | — |
| R4 | [`hierarchy` — estado rico sem consumidor](R4-hierarchy-sem-consumidor.md) | Rust | — |
| R5 | [Caminhos paralelos vivos só pelos testes](R5-caminhos-paralelos.md) | Rust | — |

Os briefings marcados como conflitantes tocam os mesmos arquivos — **não rode R1 e R2
em paralelo**. Os demais são independentes entre si.

## Apêndice — sujeira trivial (não precisa de briefing)

`src-tauri/src/src-tauri/src/evolution/pipeline/tests/` é uma árvore de diretórios
vazia, artefato de um comando rodado com o path duplicado. Nenhum arquivo dentro,
nenhuma referência no crate. Pode ser removida direto.

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
