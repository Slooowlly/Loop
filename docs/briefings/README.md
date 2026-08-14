# Briefings — trabalho pronto para despachar

Cada arquivo aqui é **autocontido**: descreve um item do [roadmap.md](../roadmap.md) com
evidência, semântica do backend, o que construir e as armadilhas conhecidas. Entregue o
arquivo inteiro a uma sessão separada — não precisa de contexto desta conversa.

| # | briefing | área | tam | depende de |
|---|---|---|---|---|
| D-09 | [Despacho de R1 / R2 / R4](D09-despacho-r1-r2-r4.md) | Rust | G | ver o guia |

**Nunca em paralelo:** R1 e R2, que tocam os mesmos arquivos de `narrative/`.

**Removidos na limpeza de 11/08/2026**, porque o trabalho fechou e o briefing virou pergunta
respondida:

- `F06-backup-restauracao.md`: o F-06 está **feito**. `src/components/ui/BackupsModal.jsx` é
  aberto pelo `src/pages/LoadSave.jsx`. Registro em [divida-tecnica.md](../divida-tecnica.md).
- `F10-escopo-iracing.md`: o F-10 foi **decidido** em 27/07/2026, e a saída dele é o
  [iracing-escopo.md](../iracing-escopo.md), que é o documento oficial da área. O retrato do
  que existe hoje está no [DESIGN.md](../DESIGN.md) §19.
- `F07-espectadores-interesse.md`: o F-07 está **feito**. O retrato do ciclo fechado (card de
  interesse esperado na Sala de Estratégia, repercussão no resultado, presença pública na
  Minha Equipe) está no [DESIGN.md](../DESIGN.md) §17.1. ⚠️ **Vale ler o que o briefing errou**,
  porque é o erro de método mais comum deste repo: ele descrevia como pendente a fatia P1
  (repercussão pós-corrida) e a P2 (presença pública da equipe), e as duas já estavam
  implementadas quando foi despachado. Só a terceira coisa faltava, e ela não era nenhuma das
  duas fatias — era a exibição rica do interesse ESPERADO, que o `DESIGN.md` §17.1 pedia e o
  briefing não listou como trabalho. Briefing envelhece; confira cada fatia contra o código
  antes de escrever uma linha.

Os três continuam recuperáveis pelo histórico do git.

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
