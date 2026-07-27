---
name: nova-migracao
description: Adiciona uma migração de schema no SQLite do Loop com segurança — uma linha no array MIGRATIONS, bump do CURRENT_VERSION, e a regra dura de nunca editar migração já lançada. Use SEMPRE que a tarefa mexer no schema do banco: "adicionar coluna", "nova tabela", "campo novo no piloto/equipe/temporada", "preciso guardar isso no save", "alterar o banco", "criar índice", ou quando um save existente estiver quebrando por coluna ausente. Use também antes de editar db/migrations.rs ou db/migrations/baseline.rs por qualquer motivo, porque editar a baseline no lugar errado corrompe saves em campo silenciosamente.
---

# Migração de schema no Loop

O banco é SQLite local, um arquivo por save. Um erro aqui não quebra o build —
quebra o save de quem já está jogando, e não tem como desfazer remotamente. Daí a
cautela desproporcional ao tamanho da mudança.

## Como o versionamento funciona hoje

`src-tauri/src/db/migrations.rs` tem duas coisas que precisam andar juntas:

```rust
const CURRENT_VERSION: u32 = 53;

const MIGRATIONS: &[(u32, fn(&Connection) -> Result<(), DbError>)] =
    &[(53, migrate_baseline)];
```

O array `MIGRATIONS` é a **única** fonte de verdade da ordem e do conjunto.
`run_all` aplica tudo num banco novo; `run_pending` aplica só o que falta num
banco existente, comparando com a versão gravada.

As 53 migrações incrementais originais foram colapsadas numa baseline única
(`db/migrations/baseline.rs`, o `BASELINE_DDL`), registrada na versão 53. Um banco
novo já nasce carimbado como 53. Consequência prática: **a baseline só sabe criar
banco do zero, não migra save antigo.**

## A regra que não se negocia

Migração já lançada é imutável. Se a v53 está num build que alguém instalou, o
banco daquela pessoa já foi carimbado como 53 e `run_pending` nunca mais vai
rodar a v53. Editar o `BASELINE_DDL` para "corrigir" alguma coisa faz o schema
divergir entre quem instalou antes e quem instalou depois — e o sintoma aparece
semanas depois, como uma query que falha só na máquina de um.

Então: alterou schema, **cria a próxima versão**. Sempre.

A única exceção honesta é quando a versão ainda não saiu de nenhum build
publicado e você mesmo a escreveu nesta sessão. Se houver qualquer dúvida sobre
isso, trate como lançada.

## Passo a passo

**1. Escreva a função de migração** em `db/migrations.rs`, ao lado de
`migrate_baseline`:

```rust
/// v54 — guarda o número de treinos livres disputados por fim de semana.
fn migrate_v54_treinos_livres(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(
        "ALTER TABLE race_weekend ADD COLUMN treinos_livres INTEGER NOT NULL DEFAULT 0;",
    )?;
    Ok(())
}
```

Escreva o DDL de forma idempotente sempre que a sintaxe permitir
(`CREATE TABLE IF NOT EXISTS`, `CREATE INDEX IF NOT EXISTS`) — a baseline inteira
segue esse princípio, e isso torna reaplicação inofensiva. `ALTER TABLE ADD
COLUMN` não tem `IF NOT EXISTS` no SQLite; toda coluna nova precisa de
`NOT NULL DEFAULT <algo>` ou de ser nullable, senão a tabela existente não aceita.

**2. Registre no array e bumpe a versão** — as duas coisas, no mesmo commit:

```rust
const CURRENT_VERSION: u32 = 54;

const MIGRATIONS: &[(u32, fn(&Connection) -> Result<(), DbError>)] = &[
    (53, migrate_baseline),
    (54, migrate_v54_treinos_livres),
];
```

Bumpar um sem o outro é o bug clássico: o banco fica carimbado numa versão que
não corresponde ao que foi aplicado.

**3. Espelhe na baseline.** Um banco novo roda a v53 e depois a v54, então o
schema final bate — mas só enquanto a cadeia toda existir. Se a intenção é que a
coluna faça parte do schema-base, avalie com o usuário se vale colapsar depois;
não colapse por conta própria dentro da mesma mudança.

**4. Ajuste o schema-ouro.** `db/migrations/schema_ouro.rs` (só em `#[cfg(test)]`)
descreve o schema normalizado e compara com um fixture versionado. Qualquer
mudança de schema faz esse teste falhar — isso é o teste funcionando. Atualize o
fixture conscientemente, olhando o diff: ele é a última linha de defesa contra
schema mudado sem querer.

**5. Queries.** O acesso fica em `db/queries/*` (uma por área de domínio).
Coluna nova sem query que a leia é peso morto; coluna lida por query que ainda
não foi atualizada é `no such column` em runtime.

## Fechamento

```bash
npm run build && cargo test --manifest-path src-tauri/Cargo.toml
```

Um teste que cria um banco do zero e assevera `column_exists` para as colunas
novas segue o padrão que já existe no fim de `migrations.rs` — copie a forma.

E vale testar o caminho de upgrade também, não só o banco novo: abrir um save
gerado antes da mudança é o cenário que de fato queima o jogador.
