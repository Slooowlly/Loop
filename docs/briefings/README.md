# Briefings — trabalho pronto para despachar

Cada arquivo aqui é **autocontido**: descreve um item do [roadmap.md](../roadmap.md) com
evidência, semântica do backend, o que construir e as armadilhas conhecidas. Entregue o
arquivo inteiro a uma sessão separada — não precisa de contexto desta conversa.

| # | briefing | área | tam | depende de |
|---|---|---|---|---|
| F-07 | [Espectadores e interesse de evento](F07-espectadores-interesse.md) | Rust + Frontend | M | nada (leia o R3 antes de unificar presença) |
| D-09 | [Despacho de R1 / R2 / R4](D09-despacho-r1-r2-r4.md) | Rust | G | ver o guia |

**Podem rodar em paralelo:** F-07 e o R4. São áreas disjuntas.
**Nunca em paralelo:** R1 e R2, que tocam os mesmos arquivos de `narrative/`.

**Removidos na limpeza de 11/08/2026**, porque o trabalho fechou e o briefing virou pergunta
respondida:

- `F06-backup-restauracao.md`: o F-06 está **feito**. `src/components/ui/BackupsModal.jsx` é
  aberto pelo `src/pages/LoadSave.jsx`. Registro em [divida-tecnica.md](../divida-tecnica.md).
- `F10-escopo-iracing.md`: o F-10 foi **decidido** em 27/07/2026, e a saída dele é o
  [iracing-escopo.md](../iracing-escopo.md), que é o documento oficial da área. O retrato do
  que existe hoje está no [DESIGN.md](../DESIGN.md) §19.

Os dois continuam recuperáveis pelo histórico do git.

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
