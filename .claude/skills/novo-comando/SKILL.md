---
name: novo-comando
description: Cria um comando Tauri novo no Loop de ponta a ponta — lógica em commands/career/*, casca #[tauri::command], DTOs serde e o registro obrigatório no generate_handler! do lib.rs, mais o invoke no slice Zustand. Use SEMPRE que a tarefa envolver expor algo novo do Rust para o React: "adicionar comando", "o front precisa buscar X do backend", "criar invoke", "expor essa função pro frontend", "novo endpoint", ou quando um invoke existente estiver retornando "command not found". Também use ao mover/renomear um comando já existente, porque o registro no lib.rs quebra silenciosamente.
---

# Comando Tauri novo no Loop

A ponte Rust↔React aqui tem quatro pontos de contato e nenhum compilador que
cobre o último. Se você criar a função e a casca `#[tauri::command]` mas esquecer
de registrar no `generate_handler!`, tudo compila e o app só falha em runtime com
`command <nome> not found` — geralmente numa tela que ninguém abriu ainda. Por
isso a ordem abaixo termina no registro, e não começa por ele.

## As quatro camadas

| Camada | Arquivo | Papel |
|---|---|---|
| 1. Lógica | `src-tauri/src/commands/career/<área>.rs` | `fn *_in_base_dir(base_dir, ...)` — recebe o diretório de save explicitamente, sem `AppHandle`. É o que fica testável. |
| 2. Casca | `src-tauri/src/commands/career_commands.rs` | `#[tauri::command] pub async fn` que resolve `base_dir` e delega. Nada de lógica aqui. |
| 3. DTOs | `src-tauri/src/commands/career_types.rs` | structs serde que cruzam a ponte. |
| 4. Registro | `src-tauri/src/lib.rs` | uma linha no `generate_handler![...]`. |

O `career.rs` é só um índice — a lógica mora nos irmãos dentro de `commands/career/`
(`standings.rs`, `queries.rs`, `season_flow.rs`, `market_window.rs`, `lifecycle.rs`,
`interests.rs`, `briefing.rs`, `save_state.rs`, `vacancies.rs`, `debug.rs`).
Escolha o irmão pelo domínio; só crie arquivo novo se nenhum servir, e nesse caso
declare o `mod` no índice.

Domínios fora de carreira têm o próprio par (`race.rs`, `inbox.rs`, `iracing/`,
`overlay/`, `convocation.rs`…). O padrão de camadas é o mesmo; só troca o arquivo.

## Passo a passo

**1. Lógica.** No irmão certo de `commands/career/`, escreva a função pura:

```rust
pub fn get_teams_standings_in_base_dir(
    base_dir: &Path,
    career_id: &str,
    categoria: &str,
) -> Result<Vec<TeamStanding>, String> {
    // abre a conexão a partir de base_dir, consulta db::queries::*, monta o DTO
}
```

O erro é `String` porque é isso que a ponte serializa. Mensagens em português.

**2. DTO.** Se o retorno não é um tipo já existente, adicione a struct em
`career_types.rs` com `#[derive(Debug, Clone, Serialize, Deserialize)]` e
`#[serde(rename_all = "camelCase")]` — o React consome em camelCase. Confira o
padrão dos vizinhos antes de inventar.

**3. Casca.** Em `career_commands.rs`, importe a função no bloco `use` do topo e
escreva a casca fina:

```rust
#[tauri::command]
pub async fn get_teams_standings(
    app: AppHandle,
    career_id: String,
    category: String,
) -> Result<Vec<TeamStanding>, String> {
    let base_dir = app_data_dir(&app)?;
    get_teams_standings_in_base_dir(&base_dir, &career_id, &category)
}
```

Nomes dos parâmetros em snake_case aqui viram camelCase no `invoke` do JS — o
Tauri faz essa conversão sozinho. É a fonte mais comum de "argumento chegou
undefined": confira que o nome no JS é o snake_case convertido, não o original.

**4. Registro.** Em `lib.rs` (~linha 390), adicione a linha no
`generate_handler![...]`, mantendo o agrupamento por domínio que já existe:

```rust
commands::career_commands::get_teams_standings,
```

**Não pule esta etapa.** Se for a única coisa que você fizer nesta sessão, que
seja esta — as outras três o compilador cobra, esta não.

**5. Frontend.** O consumo é via `invoke` de `@tauri-apps/api/core`, dentro do
slice de domínio em `src/stores/career/` (`careerSlice`, `raceSlice`,
`marketSlice`, `seasonSlice`, `preRaceCacheSlice`) ou de um hook de dados em
`src/components/**/use*.js`. Escolha pelo dono do estado: se o resultado vive no
store, é slice; se é dado local de uma tela, é hook.

**6. Teste.** A função `_in_base_dir` é testável sem Tauri — é para isso que ela
existe. Os testes de comando ficam em `commands/career/tests/`. Um teste que
monta um `base_dir` temporário, roda o fluxo e assevera o DTO vale mais que
qualquer verificação manual no app.

## Fechamento

```bash
npm run build && cargo test --manifest-path src-tauri/Cargo.toml
```

O `npm run build` antes não é zelo: `tauri::generate_context!` embute `dist/` em
tempo de compilação, então sem ele o crate nem compila. Se estiver em dúvida
sobre o que rodar, use a skill `verificar`.

Antes de dizer que terminou, confirme os quatro pontos — em especial que o nome
no `generate_handler!` bate exatamente com o `pub async fn` da casca.
