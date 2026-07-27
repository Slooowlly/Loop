# F-10 — Decidir o escopo da integração com iRacing

**Área:** Decisão de produto + documentação · **Risco:** baixo para executar, alto para
ignorar · **Tamanho:** M · **Depende de:** nada · **Não é tarefa de código** (ainda)

Briefing autocontido. Contexto em [roadmap.md](../roadmap.md) §8.

---

## O problema em uma frase

**O documento de design descreve um app que não existe mais nessa área**, e enquanto isso
não for resolvido a maior subárea do backend não é planejável.

---

## A contradição, com evidência dos dois lados

### O que o DESIGN.md diz

[`DESIGN.md`](../DESIGN.md) §23, "Pendências conhecidas":

> **Integração real com iRacing** — `AppConfig` guarda o caminho, mas export/watchdog foram
> **removidos** do código atual (módulos `export/` e `commands/export.rs` deletados; a
> integração é expansão futura).

Ou seja: o documento diz que a área foi *desmontada* e é *futuro*.

### O que o código diz

| medida | valor |
|---|---|
| linhas em `src-tauri/src/iracing_sdk/` | **16.910** |
| arquivos `.rs` no módulo | ~30 |
| comandos `iracing_*` registrados no `lib.rs` | **46** (de 158 totais — 29% da superfície IPC) |
| componentes React dedicados | 8, em `src/components/iracing/` |

Submódulos vivos: leitura de telemetria e sessão (`imp/`, `yaml.rs`), monitor de corrida
(`race_monitor/`, com amostrador, histórico, detecção de quebras), análise de telemetria
(`telemetry_analysis/`), estratégia de pneu (`tire_strategy.rs`), clima (`weather/`),
percepção de rivalidade (`rivalry_perception.rs`), geração de grid e temporada
(`roster_gen.rs`, `season_gen.rs`, `results_gen.rs`), controle de prova
(`race_control.rs`), dificuldade adaptativa (`adaptive.rs`, `car_difficulty.rs`), ponte de
resultados (`result_bridge/`), e um módulo de **diagnóstico escrito agora**, no commit
`2c85f44`.

**Conclusão factual:** a integração não foi removida nem adiada. Ela mudou de direção.
O que era *"exportar a carreira do Loop para o iRacing"* virou *"ler o iRacing real e
trazer o resultado para dentro do Loop"* — e ninguém atualizou o documento.

---

## O segundo achado: 9 comandos órfãos

Destes 46, nove estão registrados no `invoke_handler` e **não têm uma única chamada no
frontend** (verificado por `grep` literal e por checagem de `invoke` dinâmico — não há
invocação por variável no projeto):

```
iracing_career_race_result   iracing_log_caminho        iracing_poll_race
iracing_read_session         iracing_read_telemetry     iracing_reset_race
iracing_restore_yellow_macro iracing_send_chat_macro    iracing_throw_yellow
```

Eles se separam em dois grupos com destinos provavelmente diferentes:

- **Leitura crua** (`read_telemetry`, `read_session`, `poll_race`, `log_caminho`) —
  cheiro de andaime de desenvolvimento: o encanamento que alimenta os comandos de mais
  alto nível que *são* consumidos. Podem ser legitimamente internos e não precisar estar
  expostos na ponte IPC.
- **Controle de prova** (`throw_yellow`, `restore_yellow_macro`, `send_chat_macro`,
  `reset_race`) — feature de direção de prova que parece ter sido construída e nunca
  ligada a um botão. Isso é bem diferente de andaime: é produto pronto sem porta.

Cada comando exposto e não usado é superfície de API que alguém vai manter sem saber por
quê. Mas **remover é irreversível e alguns podem estar a um botão de virar feature** —
por isso a decisão vem antes do código.

---

## O que eu quero desta sessão

Esta tarefa é **de análise e decisão**, não de implementação. O produto final é um
documento e uma decisão registrada, não um diff funcional.

### 1. Mapear o que a integração faz hoje, de verdade

Percorra `iracing_sdk/` e os 46 comandos e produza a descrição funcional honesta:
o que o Loop consegue fazer com o iRacing instalado, hoje, nesta branch. Não o que estava
planejado — o que está ligado e alcançável pelo jogador.

Comece pelos comandos **com** consumidor: eles definem a fronteira real do produto.

### 2. Classificar os 9 órfãos, um a um

Para cada um, uma das três respostas, com justificativa:
- **feature pendente** — existe, funciona, falta UI. Vira item de backlog com o que falta.
- **interno** — não deveria estar no `invoke_handler`; tirar da ponte e manter como
  função do crate.
- **morto** — remover.

Antes de marcar qualquer coisa como morta, verifique se os testes Rust dependem dela.
O briefing R5 da varredura de acoplamento documenta o padrão "caminho paralelo vivo só
pelos testes" — é exatamente o risco aqui.

### 3. Reescrever a §23 do DESIGN.md

Substituir o parágrafo de "Integração real com iRacing" por uma descrição do que existe.
Se a mudança de direção (exportar → ler) for confirmada, **diga isso explicitamente** no
documento, para o próximo leitor não repetir a confusão.

### 4. Responder a pergunta de produto

Esta é a decisão que ninguém tomou e que trava o planejamento:

> **O Loop é um simulador de carreira offline que opcionalmente lê o iRacing, ou é uma
> ferramenta de acompanhamento do iRacing que tem uma carreira simulada dentro?**

Hoje 29% da superfície IPC é iRacing, num app cujo `CLAUDE.md` se descreve como
"simulador desktop **offline** de carreira". As duas respostas são legítimas e levam a
roadmaps opostos — mas a ambiguidade cobra caro: enquanto ela durar, todo item de
roadmap dessa área é palpite.

Se você não puder decidir, **apresente o trade-off ao dono do projeto com números**, e
registre que a decisão está pendente. Não decida por omissão.

---

## Armadilhas

1. **Windows-only.** O SDK do iRacing depende de winapi; há um `imp/stub.rs` para os
   outros sistemas. Qualquer conclusão sobre "não funciona" precisa distinguir "quebrado"
   de "compilado como stub fora do Windows".
2. **Não confie em nome de arquivo.** Este repo já produziu um erro de leitura exatamente
   assim — ver a seção "Correção de uma leitura anterior" no [roadmap.md](../roadmap.md).
   Rastreie a partir do comando registrado no `lib.rs`, não do nome do módulo.
3. **Não remova nada nesta sessão.** O produto é a análise. Remoção vem depois, com a
   decisão tomada e num commit próprio.
4. **`.cargo/config.toml` é específico da máquina** — não commite mudanças nele.

## Verificação

Se o resultado for só documentação: `npm run test:structure` (há guards que leem prosa e
encoding). Se mexer em `lib.rs` para tirar comando da ponte, aí sim `npm run build`
seguido de `cargo test --manifest-path src-tauri/Cargo.toml` — nessa ordem, porque
`generate_context!` embute `dist/`.
