# F-06 — Backup e restauração: o backend inteiro existe e é inalcançável

**Área:** Frontend (React) · **Risco:** baixo · **Tamanho:** P · **Depende de:** nada

Briefing autocontido. Leia o [roadmap.md](../roadmap.md) §1 para o contexto de por que
este item é o primeiro da fila.

---

## O que foi encontrado

O Loop **cria backups automaticamente e nunca deixa o jogador chegar neles.**

Não é um sistema pela metade: é um sistema completo, testado, rodando em produção, com
zero superfície de UI. Um jogador com 5 temporadas jogadas tem 5 arquivos `.db` no disco
que não pode listar nem restaurar por dentro do jogo.

### O backup é criado sozinho, no fim de cada temporada

[`commands/career/season_flow.rs:78`](../../src-tauri/src/commands/career/season_flow.rs#L78):

```rust
// Backup canônico de fim de temporada — antes de qualquer mutação da próxima.
// Falha aqui bloqueia o pipeline: melhor abortar do que avançar sem rede de segurança.
crate::commands::save::backup_season_internal(&db_path, &career_dir, season.numero as u32, &meta_path)
    .map_err(|e| format!("Falha ao criar backup de fim de temporada: {e}"))?;
```

Ou seja: avançar de temporada **sempre** produz um backup, e falhar em criá-lo aborta o
avanço. A rede de segurança já está armada.

### Os três comandos estão registrados e nenhum é chamado

Confirmado por `grep` em todo o `src/`: **zero** chamadas. O `LoadSave.jsx` não menciona
backup em lugar nenhum.

| comando | assinatura | o que faz |
|---|---|---|
| `create_season_backup` | `(career_id: String, season_number: u32) -> BackupInfo` | força um backup manual da temporada indicada |
| `list_backups` | `(career_id: String) -> Vec<BackupInfo>` | lista os backups existentes, ordenados por `season_number` |
| `restore_backup` | `(career_id: String, season_number: u32) -> ()` | restaura o estado daquela temporada |

Implementação em [`commands/save/comandos.rs`](../../src-tauri/src/commands/save/comandos.rs),
com a lógica em `backup.rs` e `restore.rs` no mesmo diretório.

### O DTO que a UI vai receber

`BackupInfo` (já serializado para o frontend):

```rust
season_number: u32       // número da temporada
file_name:     String    // "temporada_003.db"
file_path:     String    // caminho absoluto
size_kb:       u64       // tamanho em KB
modified_at:   String    // "2026-07-27T14:32:11" (hora local, não UTC)
```

`list_backups` devolve `Ok(vec![])` — não erro — quando o diretório `backups/` não existe.
Carreira nova é lista vazia, não falha.

---

## Semântica que a UI precisa respeitar

Estas quatro regras vêm da leitura do código e **não são negociáveis pela UI**:

**1. Um backup por temporada, e o novo sobrescreve o antigo.**
O nome do arquivo é derivado do número da temporada
(`season_backup_filename` → `temporada_{n:03}.db`), e `replace_backup_file` **apaga o
anterior** antes de renomear o novo por cima. Chamar `create_season_backup` na temporada
3 destrói o backup anterior da temporada 3. A UI precisa dizer isso antes de confirmar.

**2. O restore já cria uma cópia de segurança do estado atual — mas só uma.**
[`restore.rs:28`](../../src-tauri/src/commands/save/restore.rs#L28) copia o banco vigente
para `career.db.bak` antes de sobrescrever. Isso protege o jogador de um restore
acidental. Mas é **um único slot**: dois restores seguidos e o estado original se perde.
Não venda isso na UI como "dá pra desfazer sempre".

**3. Restaurar volta o save inteiro, não só o banco.**
O backup carrega um snapshot de arquivos auxiliares (`meta.json`, `race_results.json`,
`resume_context.json`, `briefing_phrase_history.json`, `preseason_plan.json`). O restore
os traz de volta **e apaga os que não existiam no snapshot**. É uma viagem no tempo
completa, não um merge. A UI deve comunicar perda de progresso, não "carregar um ponto".

**4. Backup antigo sem snapshot é reconstruído, e perde coisa.**
Se o diretório `.files` não existir (backup gerado por versão anterior),
`rebuild_meta_from_restored_db` remonta o `meta.json` lendo o banco. Funciona, mas
`last_saved` volta `None` e a dificuldade cai no default `"medio"`. Um backup restaurado
pode ter dificuldade diferente da original — vale um aviso se o snapshot estiver ausente.

---

## O que construir

Um painel de backups. **Uma tela, não um fluxo.**

**Onde:** a decisão é sua, mas há duas opções com trade-off real.
- No `LoadSave.jsx`, dentro do cartão de cada carreira — é onde o jogador já pensa em
  "meus saves", e o `career_id` está à mão. Recomendado.
- No `Settings.jsx`, como seção de dados — mais fácil de achar para quem procura,
  mas desconectado de qual carreira.

**O que mostra:** a lista de `list_backups`, uma linha por temporada, com número, data
(`modified_at`) e tamanho (`size_kb`). Lista vazia é estado normal em carreira nova —
trate com uma frase, não com erro.

**Duas ações:**
- *Criar backup agora* → `create_season_backup` com a temporada corrente. Confirmar
  antes, avisando que sobrescreve o backup existente daquela temporada (regra 1).
- *Restaurar* → `restore_backup`. **Confirmação obrigatória e explícita**, deixando claro
  que todo o progresso desde aquela temporada se perde (regra 3). Depois do restore, o
  estado em memória do `useCareerStore` está obsoleto: recarregue a carreira ou volte o
  jogador ao menu. Não deixe o dashboard mostrando dados do save antigo.

**i18n:** obrigatório, com chave em `pt-BR` e `en-US` — há hook de pre-commit e teste de
paridade. Use a skill `nova-string` do repo.

---

## Armadilhas

1. **`career_id` é string, e o backend faz `parse_career_number`.** Passe o mesmo formato
   que o resto do app já usa (`career_1`), não o número cru.
2. **Restaurar durante uma corrida em andamento não foi testado.** O código faz
   `checkpoint_wal` e troca o arquivo do banco; o que acontece com uma simulação a meio
   caminho é desconhecido. O caminho seguro é bloquear o restore fora do estado ocioso —
   e vale confirmar com quem conhece o fluxo antes de liberar.
3. **Não invente política de retenção.** É tentador limitar a N backups ou apagar antigos.
   Isso é mudança de backend com risco de perda de dados. Fora do escopo deste item.

## Verificação

`npm run test:ui` e `npm run test:structure`. Não deve precisar de `cargo` — o backend
não muda. Se você acabar mexendo em Rust, esse item saiu do escopo; pare e reavalie.
