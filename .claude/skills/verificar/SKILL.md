---
name: verificar
description: Roda a bateria de testes certa do Loop na ordem certa, escolhendo entre as três suítes independentes (vitest, node --test estrutural, cargo) conforme o que foi tocado, e respeitando a pegadinha de que cargo exige npm run build antes. Use SEMPRE que for validar uma mudança: "roda os testes", "isso passa?", "verifica antes de commitar", "o build tá ok?", ou ao terminar qualquer edição de código antes de reportar que está pronto. Use também quando cargo test/build falhar com erro estranho de generate_context ou de asset ausente, e quando estiver em dúvida sobre qual suíte cobre o arquivo que você mexeu.
---

# Verificar uma mudança no Loop

Três suítes independentes, cada uma cobrindo uma fatia diferente. Rodar a errada
dá falso verde; rodar todas sempre é lento demais para o loop de edição. O que
segue é o mapa de qual roda quando.

## A pegadinha que custa mais tempo

```bash
npm run build
```

`tauri::generate_context!` embute os assets de `dist/` em tempo de compilação.
Sem o build do frontend, o crate Rust **não compila** — e o erro não diz
"faltou npm run build", diz algo sobre asset ou contexto que parece um problema
no Rust. Sempre que for tocar em `cargo`, o `npm run build` vem antes.

Se `dist/` já está atualizado desde a última alteração de frontend, dá para
pular; na dúvida, rode — é mais barato que perseguir o erro.

## Mapa: o que você tocou → o que rodar

| Você mexeu em | Rode |
|---|---|
| `src/**/*.jsx`, `.js` (comportamento) | `npm run test:ui` |
| layout, cores, copy em português, controles de janela | `npm run test:structure` |
| qualquer coisa no frontend | `npm run test:all` |
| `src-tauri/**` | `npm run build` → `cargo test --manifest-path src-tauri/Cargo.toml` |
| strings de UI / traduções | `npm run i18n:audit` + `npm run test:ui` |
| ponte Rust↔React (comando novo) | as duas: `test:all` e `cargo test` |

Os comandos completos:

```bash
npm run test:ui
```

```bash
npm run test:structure
```

```bash
npm run build && cargo test --manifest-path src-tauri/Cargo.toml
```

## Rodar só um caso

Perseguir uma falha específica com a suíte inteira é desperdício. Os três
formatos:

```bash
npx vitest run src/pages/tabs/MyTeamTab.test.jsx
```

```bash
node --test scripts/tests/window-controls-sizing.test.mjs
```

```bash
cargo test --manifest-path src-tauri/Cargo.toml nome_do_teste
```

`npx vitest run -t "nome do caso"` filtra por nome dentro dos arquivos.

## O que cada suíte de fato cobre

**`test:ui` (vitest, jsdom)** — comportamento de componente e store. É a única
que executa o código. Falha aqui é bug de verdade.

**`test:structure` (node --test)** — guards de estrutura e consistência visual
que leem o código-fonte **como texto**: alinhamento de layout, distribuição da
paleta de equipes, contrato dos controles de janela, acentuação da copy em
português, sanidade de encoding. Porque leem texto, a mensagem de falha costuma
ser críptica e apontar para um regex, não para o comportamento. Se ela reclamar,
a skill `guard-visual` explica o que cada guard quer.

**`cargo test`** — toda a simulação. Note que o locale do `rust-i18n` é **global
do processo**: teste que troca de idioma precisa de `#[serial]` (crate
`serial_test`), senão contamina os testes que asseveram prosa em português — e a
falha aparece de forma intermitente, em outro teste, o que é péssimo de
diagnosticar.

## Pré-commit

O hook em `.githooks/pre-commit` bloqueia commit com string de UI em português
fora de `t()` em `.jsx` no stage. Não é uma das três suítes — roda sozinho. Se
ele barrar, a saída dele diz o arquivo e a linha; a skill `nova-string` cobre o
fluxo certo. `--no-verify` existe, mas é para pular pontualmente, não para
resolver.

## Reportar o resultado

Diga o que rodou e o que aconteceu, com a saída real quando falhar. "Os testes
passam" sem dizer quais suítes rodaram esconde exatamente o caso em que a suíte
relevante não foi executada.
