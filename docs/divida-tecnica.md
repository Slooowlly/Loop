# Divida Tecnica - Loop

Registro de inconsistencias conhecidas que nao bloqueiam o jogo hoje,
mas devem ser resolvidas antes de uma refatoracao maior ou release.

---

## Itens resolvidos

### DB-001 a DB-004 - Schema de `teams` normalizado

**Status:** Resolvido na migration v28.

Foram removidas as colunas legadas da tabela `teams`:
- `reliability`
- `prestige`
- `temp_pontos`
- `temp_vitorias`
- `carreira_vitorias`

As queries agora usam os nomes atuais do dominio:
- `confiabilidade`
- `reputacao`
- `stats_pontos`
- `stats_vitorias`
- `historico_vitorias`

A leitura de `Team` tambem deixou de usar placeholder permissivo. Campos obrigatorios agora sao lidos de forma explicita, fazendo a consulta falhar caso o schema esteja desalinhado em vez de carregar valores padrao silenciosamente.

---

## Pendencias conhecidas

Nenhuma pendencia registrada no momento.
