# Briefings — trabalho pronto para despachar

Cada arquivo aqui é **autocontido**: descreve um item do [roadmap.md](../roadmap.md) com
evidência, semântica do backend, o que construir e as armadilhas conhecidas. Entregue o
arquivo inteiro a uma sessão separada — não precisa de contexto desta conversa.

| # | briefing | área | tam | depende de |
|---|---|---|---|---|
| F-06 | [Backup e restauração](F06-backup-restauracao.md) | Frontend | P | nada |
| F-07 | [Espectadores e interesse de evento](F07-espectadores-interesse.md) | Rust + Frontend | M | nada (leia o R3 antes de unificar presença) |
| F-10 | [Decidir o escopo do iRacing](F10-escopo-iracing.md) | Decisão + docs | M | nada |
| D-09 | [Despacho de R1 / R2 / R4](D09-despacho-r1-r2-r4.md) | Rust | G | ver o guia |

**Podem rodar em paralelo:** F-06, F-07, F-10 e o R4. São áreas disjuntas.
**Nunca em paralelo:** R1 e R2 — tocam os mesmos arquivos de `narrative/`.

Os briefings da varredura de acoplamento ficam em
[../varredura-acoplamento/](../varredura-acoplamento/) e seguem válidos; o D-09 aqui é só
o guia de despacho deles.

## Convenções que valem para toda sessão de trabalho neste repo

- Código, comentários e UI **em português**.
- **i18n é obrigatório** e tem hook de pre-commit: string de UI em `.jsx` fora de `t()`
  bloqueia o commit. Chave em `pt-BR` **e** `en-US`, com teste de paridade.
- **Comando Tauri novo só existe depois de entrar no `generate_handler![...]`** do
  `lib.rs`.
- **`cargo build`/`cargo test` exigem `npm run build` antes** — `generate_context!` embute
  o `dist/` em tempo de compilação.
- Locale do backend é **global do processo**: teste Rust que troca idioma precisa de
  `#[serial]`.
- Há skills no repo para os fluxos comuns: `verificar`, `nova-string`, `novo-comando`,
  `nova-migracao`, `guard-visual`, `release`.
