# Notas sobre os mapas do banco

Estas notas separam o que é melhoria de **documentação** do que seria mudança de **schema em
tempo de execução**. Nada aqui está implementado: é leitura do banco de hoje mais o que ficaria
melhor arrumado.

Regeradas em 11/08/2026 contra o schema **v63** (baseline v53). A versão anterior era de
05/05/2026, estava escrita em inglês e olhava o schema v30, antes da normalização de `teams` e
antes do colapso das migrações incrementais na baseline.

## O que os três mapas mostram

- [`database-network-diagram.mmd`](database-network-diagram.mmd): a rede ER completa. Entidades
  com os campos que importam para entender a relação, e a distinção entre `FK` (chave estrangeira
  declarada no DDL) e `SEM` (relação que só existe na lógica do app).
- [`database-core-flow.mmd`](database-core-flow.mmd): o laço principal da carreira em quatro
  tempos. Mundo (config, meta, pilotos, equipes, contratos), fim de semana (calendário, carro,
  quebra, resultado, leitura), apuração (classificação e caixa) e arquivo (licenças, snapshots,
  história).
- [`database-modules-flow.mmd`](database-modules-flow.mmd): os sistemas de apoio agrupados em
  cinco blocos. Mercado, bloco especial, carro e incidente, caixa da equipe, narrativa e recordes.

## O que mudou desde a versão de 05/05/2026

- **54 tabelas**, contra as 29 que os mapas antigos conheciam. Entraram o carro físico
  (`team_car`, `race_breakdowns`), a leitura da corrida (`race_weekend_readings`,
  `player_race_telemetry`, `race_safety_cars`), o conteúdo por IA (as quatro tabelas `ai_*`), os
  recordes (`record_milestones`, `track_lap_records`, `category_scalar_records`), a saúde
  financeira da equipe (`team_finance_history`, `team_focus`, `team_strategic_plan`,
  `team_collapse_streak`, `team_rescue_counters`, `team_ownership_events`,
  `team_promotion_history`), a janela de transferências (`transfer_window`) e o detalhe de
  rivalidade (`rivalry_episodes`, `player_nemesis`, `team_rivalries`).
- **`teams` foi normalizada.** Saíram `reliability`, `prestige`, `temp_pontos`, `temp_vitorias` e
  `carreira_vitorias`; entraram `confiabilidade`, `reputacao`, `stats_pontos`, `stats_vitorias` e
  `historico_vitorias`.
- **As migrações v1 a v52 foram colapsadas numa baseline única (v53).** Save carimbado entre 1 e
  52 é recusado com erro explícito, porque a baseline não faz backfill de dado.
- **A v62 trouxe para dentro das migrações as tabelas que nasciam em `CREATE TABLE IF NOT EXISTS`
  do lado da query.** As constantes de DDL continuam morando em `db/queries/*.rs`, e a migração as
  referencia. O schema-ouro passou a enxergar essas 11 tabelas.

## O que ficaria melhor arrumado

1. **Aposentar a tabela `races`.** `calendar` já é a tabela de evento de corrida de verdade, e
   `race_results.race_id` referencia `calendar(id)`. A `races` continua no DDL sem leitura viva.
   É o item D-02 do [backlog.md](backlog.md).
2. **Padronizar o nome das chaves.** Convivem hoje `piloto_id` e `pilot_id` (compare
   `race_results` com `injuries`), `temporada_id` e `season_id` (a `calendar` tem **as duas
   colunas**), e `equipe_id` e `team_id` (compare `contracts` com `team_car`). Cada consulta nova
   paga esse imposto uma vez.
3. **Declarar as relações que hoje são só semânticas.** As mais soltas são as de notícia,
   história, log da janela especial, arquivo de piloto e histórico de DNF. Elas aparecem como
   `SEM` no diagrama de rede.
4. **Manter separado o que é estado vivo e o que é snapshot imutável.**
   - Estado vivo: `drivers`, `teams`, `contracts`, `seasons`, `calendar`, `standings`, `team_car`.
   - História imutável: `race_results`, `race_weekend_readings`, `driver_season_archive`,
     `team_season_archive`, `history_seasons`, `retired`, `record_milestones`.
5. **Tratar mercado e bloco especial como pipeline**, que é o que eles já são no código:
   proposta ou oferta → resposta → contrato → atribuição ou inscrição → log e história.

## Ordem segura para qualquer uma dessas mudanças

1. Escrever teste que documenta o comportamento de hoje em volta de `calendar`, `race_results` e
   contrato especial.
2. Adicionar coluna ou view de compatibilidade onde o nome vai mudar.
3. Fazer o backfill do dado.
4. Mover as consultas para o nome canônico novo.
5. Só então remover o alias antigo ou a tabela legada.

> A regra do repositório continua valendo em todos os passos: o array `MIGRATIONS` é a única
> fonte de verdade da ordem, e migração já lançada nunca é editada. Crie a próxima.
