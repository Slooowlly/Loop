# Dívida Técnica — iRacerApp V1

Registro de inconsistências conhecidas que não bloqueiam o jogo hoje,
mas devem ser resolvidas antes de uma refatoração maior ou release.

---

## DB-001 — Nomes divergentes entre struct `Team` e colunas do banco

**Arquivo:** `src-tauri/src/db/queries/teams.rs`

O struct `Team` usa nomes em português, mas duas colunas herdaram nomes em inglês da v1 do schema:

| Campo no struct | Coluna no banco |
|-----------------|-----------------|
| `confiabilidade` | `reliability` |
| `reputacao` | `prestige` |

O mapeamento é feito manualmente em `insert_team`, `update_team` e `team_from_row`, o que cria risco de erro silencioso se alguém adicionar um novo campo e não perceber a discrepância.

**Resolução sugerida:** migration que renomeia as colunas (`ALTER TABLE … RENAME COLUMN`) e atualiza os parâmetros das queries.

**Urgência:** Baixa — não afeta gameplay, só legibilidade e manutenção.

---

## DB-002 — Coluna legada `carreira_vitorias` redundante

**Arquivo:** `src-tauri/src/db/queries/teams.rs`

A tabela `teams` tem duas colunas que representam o mesmo dado:
- `historico_vitorias` — coluna atual, lida corretamente pelo `team_from_row`
- `carreira_vitorias` — coluna da v1 do schema, nunca removida

Em `insert_team` e `update_team`, `carreira_vitorias` recebe o mesmo valor de `historico_vitorias`:
```rust
":carreira_vitorias": team.historico_vitorias,
```

O `team_from_row` lê `historico_vitorias` com fallback para `carreira_vitorias` caso a coluna nova não exista — o que nunca acontece em bancos novos.

**Resolução sugerida:** `DROP COLUMN carreira_vitorias` numa migration futura e remover o fallback em `team_from_row`.

**Urgência:** Baixa — não afeta gameplay, apenas mantém dado duplicado no banco.

---

## DB-003 — Leitura permissiva via `placeholder_team_from_db`

**Arquivo:** `src-tauri/src/db/queries/teams.rs` — função `team_from_row`

A leitura de um `Team` do banco começa criando um objeto placeholder com valores padrão e depois sobrescreve campo a campo usando `optional_column`. Se uma coluna falhar silenciosamente (tipo errado, ausente), o campo fica com o valor padrão do placeholder sem emitir erro.

Exemplo: se `car_performance` retornar `None` por erro de tipo, o team carrega com `50.0` — valor plausível, sem aviso.

**Risco real:** Corrupção silenciosa de leitura. Um time pode ter `car_performance` errado na simulação sem nenhuma mensagem de erro.

**Resolução sugerida:** Converter para leitura explícita com `row.get("campo")?` em todos os campos obrigatórios. Apenas campos genuinamente opcionais (ex: `marca`, `classe`) devem usar `optional_column`.

**Urgência:** Média — afeta confiabilidade de dados de simulação em casos de schema desalinhado.

---

## DB-004 — `temp_pontos` e `temp_vitorias` sobrescritos com valores de `stats_*`

**Arquivo:** `src-tauri/src/db/queries/teams.rs` — funções `insert_team` e `update_team`

```rust
":temp_pontos":   team.stats_pontos as f64,
":temp_vitorias": team.stats_vitorias,
```

As colunas `temp_*` da v1 são escritas com os valores dos campos `stats_*` do struct, ignorando qualquer estado temporário independente. O struct `Team` não tem campos `temp_pontos`/`temp_vitorias` — esses eram campos do schema antigo que viraram `stats_*`.

**Risco real:** Baixo — `update_team_season_stats` sincroniza ambas corretamente. Mas `insert_team` e `update_team` fazem a escrita de forma confusa, dificultando entendimento futuro.

**Resolução sugerida:** Eliminar `temp_pontos` e `temp_vitorias` do banco (são redundantes com `stats_pontos`/`stats_vitorias`) ou documentar explicitamente a relação.

**Urgência:** Baixa.
