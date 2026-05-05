# Complete Historical World Contract Design

## Contexto

A nova carreira nao deve mais criar um universo vazio. Toda carreira jogavel deve nascer de um mundo historico ja vivido, com temporadas, corridas, contratos, transferencias, aposentadorias, lesoes, equipes vencedoras e pilotos com trajetoria palpavel antes do jogador entrar.

O projeto ja possui boa parte da base:

- `historical_draft.rs` cria drafts historicos.
- `generate_historical_world` gera mundo sem jogador.
- `simulate_historical_range` simula temporadas ate o ano jogavel.
- `driver_season_archive`, `race_results`, `retired`, `contracts`, `standings` e stats de `teams` ja alimentam ficha do piloto, ficha da equipe e ranking mundial.
- `AppConfig::list_saves` ja exclui drafts e failed saves.

A lacuna nao e criar um sistema totalmente novo. A lacuna e transformar isso em contrato obrigatorio do produto, remover o caminho rapido sem historico e auditar que todos os dados essenciais foram gerados antes de liberar o save.

## Decisoes

1. **Sem carreira rapida.**
   `create_career` deixa de ser usado pela UI de nova carreira. O caminho principal passa a ser draft historico -> escolha de entrada -> finalizacao.

2. **O jogador nao existe durante a historia.**
   O mundo simula de 2000 a 2024 sem jogador. O jogador e inserido apenas em 2025, com historico pessoal zerado.

3. **Historico esportivo vem de fatos, nao de biografia falsa.**
   A primeira versao deve priorizar fatos persistidos: corridas, resultados, contratos, arquivos anuais, aposentadorias, lesoes e titulos. Textos narrativos podem ser derivados depois.

4. **Draft incompleto nao vira save jogavel.**
   Um modulo de auditoria de integridade deve bloquear a finalizacao se dados essenciais estiverem ausentes.

5. **Equipes precisam de snapshots anuais.**
   A ficha da equipe ja consegue inferir muita coisa via `race_results`, mas um mundo completo precisa de um `team_season_archive` ou equivalente para preservar categorias, posicoes, stats e contexto anual da equipe de forma explicita.

6. **Auditoria e um gate inline, nao uma etapa de usuario.**
   O modulo de auditoria deve ser uma funcao separada e testavel, mas chamada dentro do fluxo de criacao/finalizacao. O usuario nao escolhe "rodar auditoria"; um draft invalido simplesmente nao vira mundo jogavel.

7. **Draft falho nao pode deixar dados reaproveitaveis.**
   Se a geracao ou auditoria falhar, o save deve manter apenas metadados suficientes para informar o erro ao usuario. Banco, backups e artefatos gerados do draft falho devem ser removidos para impedir retomada acidental de um mundo inconsistente.

## Contrato De Dados

### Ficha Do Piloto

| Necessidade | Fonte atual | Garantia obrigatoria na geracao historica |
| --- | --- | --- |
| Identidade: nome, nacionalidade, idade, genero | `drivers` | Todo piloto ativo/aposentado consultavel deve ter identidade valida. |
| Status: ativo, livre, aposentado, lesionado | `drivers`, `contracts`, `injuries`, `retired` | Status deve ser derivavel sem fallback ambiguo. |
| Equipe, papel, salario, categoria atual | `contracts`, `teams`, `drivers` | Piloto ativo contratado deve ter contrato regular ativo consistente com equipe/categoria. |
| Historico anual, titulos, melhor temporada | `driver_season_archive` | Todo piloto veterano que disputou temporadas deve ter snapshots anuais. |
| Corridas, vitorias, podios, poles, DNFs | `race_results`, stats de `drivers` | Resultados historicos devem alimentar stats acumuladas e arquivos anuais. |
| Forma recente e ultimas corridas | `race_results`, `driver_season_archive.snapshot_json` | Pilotos com corridas recentes devem ter ultimos resultados persistidos ou reconstruiveis. |
| Mobilidade de categoria e equipes defendidas | `driver_season_archive`, `race_results`, `contracts` | Contratos e resultados precisam preservar equipe/categoria por temporada. |
| Lesoes historicas e ativas | `injuries`, inferencias de archive | Lesoes historicas devem existir em proporcao plausivel para carreiras longas; lesao ativa deve estar marcada quando aplicavel. |
| Eventos especiais | contratos especiais, resultados especiais | Participacoes especiais devem ser registradas quando simuladas. |

### Ficha Da Equipe

| Necessidade | Fonte atual | Garantia obrigatoria na geracao historica |
| --- | --- | --- |
| Identidade, fundacao, categoria atual | `teams`, timeline historica | Toda equipe deve ter ano de fundacao coerente com categoria e ano de atividade. |
| Grid atual N1/N2 e hierarquia | `teams`, `contracts` | Equipe ativa deve ter pilotos e contratos regulares consistentes. |
| Corridas, vitorias, podios, taxa de podio/vitoria | `race_results` | Equipes historicas devem ter resultados reais suficientes. |
| Titulos por categoria/ano | `standings`, futuro `team_season_archive` | Campeoes construtores devem ser recuperaveis sem depender de fallback. |
| Piloto simbolo e rivalidade | `race_results`, contratos | Historico deve permitir agregacao por piloto/equipe/rival. |
| Saude financeira e operacao | `teams`, finance state | No start jogavel, financeiro deve estar limpo/consistente, nao carregando lixo do batch historico. |
| Caminho de categoria | `race_results`, `teams`, futuro archive | Promocoes/rebaixamentos e categorias passadas devem ser persistidos. |

### Ranking Mundial De Pilotos

| Necessidade | Fonte atual | Garantia obrigatoria na geracao historica |
| --- | --- | --- |
| Indice historico por categoria | `driver_season_archive`, stats atuais | Todo piloto com carreira deve ter stats por categoria. |
| Status ativo/livre/aposentado | `drivers`, `contracts`, `retired` | Aposentados devem aparecer como aposentados mesmo se ainda existirem em `drivers`. |
| Categoria atual/final e historicas | `contracts`, `driver_season_archive`, `retired` | Categoria final de aposentado e categorias passadas devem estar preenchidas. |
| Anos de carreira, idade, salario | `drivers`, `contracts`, `retired.estatisticas` | Ativos usam contrato atual; aposentados usam snapshot historico. |
| Ano de aposentadoria e anos aposentado | `retired.temporada_aposentadoria`, `seasons` | Sempre salvar ano real, nao numero interno de temporada. |
| Lesionado | `injuries` | Lesao ativa deve ser consultavel sem texto longo no ranking. |
| Movimentacao do ranking entre corridas | `race_results`, stats atuais | Ultima corrida deve permitir reconstruir ranking anterior. |
| Exclusao de pilotos sem historico | ranking backend | Pilotos sem fatos competitivos nao entram no ranking. |

### Mercado, Evolucao E Continuidade

| Necessidade | Fonte atual | Garantia obrigatoria na geracao historica |
| --- | --- | --- |
| Pilotos livres plausiveis | `drivers`, contratos inativos/ausentes | Free agents devem ter historico e categoria/licenca plausiveis. |
| Licencas | `licenses`, categoria | Pilotos ativos devem possuir licenca necessaria. |
| Aposentadoria | `retired`, `drivers.status` | Aposentados nao podem ficar elegiveis como mercado ativo. |
| Rookies por temporada | rookies/evolution pipeline | 2025 deve ter grid completo apos aposentadorias/promocoes. |
| Promocao/rebaixamento | promotion pipeline, `teams` | Times devem chegar em 2025 em categoria coerente. |
| Meta/IDs | `meta` | IDs seguintes devem estar sincronizados apos gerar milhares de linhas. |

## Contrato De Teste Minimo

O helper `seed_complete_minimal_world` usado pelos testes de auditoria deve ter contrato explicito. Ele nao deve ser um "monte de inserts" implicito.

Ele deve criar, no minimo:

- schema completo via migracoes reais;
- `meta` com `current_season`, `current_year`, `next_driver_id`, `next_team_id`, `next_season_id`, `next_race_id`, `next_contract_id` sincronizados com os maiores IDs inseridos;
- uma temporada historica concluida antes do ano jogavel;
- uma temporada jogavel ativa no ano de entrada;
- calendario pendente para a temporada jogavel;
- pelo menos uma equipe regular ativa com N1/N2;
- dois pilotos ativos com categoria atual/licenca coerente;
- contratos regulares ativos consistentes com equipe e pilotos;
- resultados historicos suficientes para provar carreira previa;
- `driver_season_archive` para veteranos com resultados;
- pelo menos um aposentado valido quando o teste exigir validacao de aposentadoria.

Esse contrato evita que a auditoria passe porque o teste esqueceu de semear uma area inteira do mundo.

## Auditoria De Integridade Do Mundo

Criar modulo backend dedicado, por exemplo:

`src-tauri/src/world/integrity.rs`

Responsabilidade:

- verificar um draft completo antes de expor escolhas de 2025;
- gerar relatorio estruturado com erros e avisos;
- bloquear finalizacao se houver erro;
- permitir warnings nao bloqueantes para areas narrativas ainda nao obrigatorias.

Erros bloqueantes propostos:

- existe jogador antes da finalizacao;
- temporada ativa nao e 2025;
- 2025 nao possui calendario pendente;
- piloto ativo contratado sem contrato regular ativo;
- contrato ativo aponta para equipe inexistente;
- equipe ativa sem N1/N2 ou sem contratos correspondentes;
- piloto ativo sem categoria atual/licenca necessaria;
- piloto veterano com corridas mas sem `driver_season_archive`;
- `race_results` historicos ausentes ou insuficientes;
- aposentado sem ano de aposentadoria, categoria final ou estatisticas;
- aposentado com contrato regular ativo;
- ranking mundial ficaria vazio ou dominado por pilotos sem historico;
- `meta` de proximo ID menor ou igual ao maior ID usado.

Constantes de suficiencia:

- definir no modulo de auditoria uma constante nomeada para a regra minima de resultados historicos;
- primeira versao recomendada: exigir ao menos uma temporada historica com resultados por categoria ativa que existia antes do ano jogavel;
- testes curtos devem usar a mesma constante/regra, sem thresholds magicos espalhados.

Sync de IDs:

- a auditoria deve validar `next_driver_id`, `next_team_id`, `next_season_id`, `next_race_id`, `next_contract_id` contra os maiores IDs observados nas tabelas canonicas;
- o gerador de IDs ja consegue se proteger contra contador stale, mas o draft completo ainda deve falhar se sair da geracao com `meta` dessincronizado.

Warnings propostos:

- equipe com pouco historico por ter surgido tarde;
- piloto com poucas corridas por ser rookie gerado em 2024/2025;
- ausencia de noticias historicas, que e intencional nesta fase;
- dados especiais ausentes para categorias que ainda nao existiam no ano.

## Team Season Archive

Adicionar tabela canonica para snapshot anual de equipes:

`team_season_archive`

Campos minimos:

- `team_id`
- `season_number`
- `ano`
- `categoria`
- `classe`
- `posicao_campeonato`
- `pontos`
- `vitorias`
- `podios`
- `poles`
- `corridas`
- `titulos_construtores`
- `piloto_1_id`
- `piloto_2_id`
- `snapshot_json`

Uso:

- ficha da equipe pode usar `team_season_archive` como fonte primaria para temporadas, categorias e titulos;
- `race_results` continua sendo fonte de detalhe corrida-a-corrida;
- stats atuais de `teams` continuam sendo estado ativo, nao historia completa.
- persistencia deve ser idempotente por `(team_id, season_number, categoria)`: rodar o arquivamento duas vezes para a mesma temporada nao pode duplicar linhas nem somar stats duas vezes.

## Fluxo De Nova Carreira

1. Usuario informa nome, nacionalidade, idade e dificuldade.
2. Frontend chama `create_historical_career_draft`.
3. Backend cria draft com lifecycle `draft`.
4. Backend gera mundo inicial em 2000 sem jogador.
5. Backend simula 2000-2024.
6. Backend prepara 2025 pendente.
7. Backend roda auditoria de integridade.
8. Frontend mostra categorias/equipes reais de 2025.
9. Usuario escolhe categoria/equipe.
10. Backend finaliza:
    - insere jogador;
    - zera historico pessoal;
    - desloca N2;
    - cria contrato do jogador;
    - marca save como `active`.

Durante a simulacao historica, o frontend deve mostrar progresso real baseado em `draft_progress_year` quando disponivel. Se o comando atual bloquear qualquer polling util, o fluxo deve ser dividido internamente em um mecanismo start/poll/finalize antes de concluir esta fase.

Se o usuario alterar identidade relevante do piloto depois de gerar um draft, como nome, nacionalidade, idade ou dificuldade, o draft anterior deve ser descartado antes de gerar outro. Um draft historico pertence aquela identidade pendente.

## Compatibilidade

- Saves antigos continuam carregando como `active`.
- `create_career` pode permanecer para testes internos, mas nao deve ser usado pela UI normal.
- Drafts `failed` e `draft` continuam excluidos da lista normal de saves.
- Fallbacks continuam aceitaveis para saves legados.
- Para `team_season_archive`, saves legados sem a tabela populada devem usar fallback backend por `race_results`; drafts novos devem passar pela auditoria e preferir o archive.
- A auditoria bloqueante deve mirar drafts novos. Ela nao deve impedir carregamento de saves antigos que ja existem.

## Testes Obrigatorios

Backend:

- draft historico completo chega a 2025 sem jogador;
- helper `seed_complete_minimal_world` cria um mundo minimo explicitamente documentado;
- auditoria falha se remover `driver_season_archive`;
- auditoria falha se remover `race_results`;
- auditoria falha se `meta` de proximo ID estiver menor ou igual ao maior ID usado;
- auditoria falha com aposentado sem ano/categoria/stats;
- auditoria falha com equipe sem N1/N2;
- auditoria falha com contrato ativo inconsistente;
- auditoria aprova draft historico gerado normalmente;
- draft failed limpa banco/artefatos gerados e preserva erro em `meta.json`;
- finalizacao cria jogador rookie com stats zeradas;
- finalizacao desloca N2 para livre;
- listagem de saves exclui drafts/failed;
- `team_season_archive` e populado no fim de temporada;
- `archive_team_season` e idempotente;
- saves legados sem `team_season_archive` continuam retornando ficha de equipe via `race_results`;
- ficha do piloto, ficha da equipe e ranking mundial conseguem consumir draft finalizado sem campos essenciais vazios.

Frontend:

- tela de nova carreira nao oferece modo rapido;
- botao principal gera historico;
- progresso de geracao aparece com ano real quando `draft_progress_year` estiver disponivel;
- categorias/equipes sao lidas do draft;
- mudanca de identidade/dificuldade exige descartar draft;
- confirmacao chama `finalize_career_draft`;
- UI nao chama `create_career` no fluxo normal.

## Fora Do Escopo Inicial

- Biografias textuais completas.
- Noticias historicas persistidas.
- Tela de historia mundial completa.
- Checkpoint anual recuperavel para draft falho.
- Replay ou visualizador temporada a temporada.
