# FT-3 Promotion/Relegation Design

**Context**

FT-3 adiciona promoção/rebaixamento de equipes ao pipeline de fim de temporada do simulador. O sistema precisa preservar tamanhos fixos por categoria, respeitar licenças de pilotos e manter compatibilidade com FT-2, onde o mercado persiste dados em `market` e `market_proposals`.

**Approved Approach**

- Promoção/rebaixamento roda depois de evolução e aposentadorias.
- Mercado roda depois da promoção, para preencher vagas abertas pelas movimentações.
- A nova temporada continua sendo criada antes do mercado, porque o mercado persiste propostas e estado por `temporada_id`.
- O pipeline usa standings da temporada recém-finalizada para decidir promoções; não usa o estado mutado do banco como fonte de classificação.

**Modules**

- `promotion/standings.rs`: standings de construtores por categoria e por classe.
- `promotion/block1.rs`: `rookie <-> amador`.
- `promotion/block2.rs`: `amador/bmw <-> production`.
- `promotion/block3.rs`: `gt4/gt3 <-> endurance`.
- `promotion/effects.rs`: deltas de atributos de equipe por promoção/rebaixamento.
- `promotion/pilots.rs`: efeitos sobre pilotos e checagem de licença.
- `promotion/pipeline.rs`: orquestra blocos, aplica movimentos, resolve pilotos e valida invariantes.

**Data Flow**

1. `run_end_of_season` finaliza standings e licenças.
2. Evolução e aposentadorias são aplicadas.
3. `run_promotion_relegation(conn, finished_season_number, rng)` usa standings da temporada encerrada para gerar movimentos.
4. Movimentos atualizam categoria/classe das equipes.
5. Situação dos pilotos é resolvida:
   - promovido com licença sobe com a equipe;
   - sem licença fica livre;
   - jogador sem licença fica livre, permanecendo elegível na categoria atual;
   - rebaixados sempre descem com a equipe.
6. Deltas de atributos das equipes são aplicados.
7. O pipeline cria a nova temporada.
8. `run_market(conn, new_season_number, rng)` preenche vagas já considerando as equipes promovidas/rebaixadas.

**Key Rules**

- Temporada 1 não tem promoção/rebaixamento.
- Production e Endurance usam standings por `classe`.
- O bloco 2 deve excluir equipes já rebaixadas do amador no bloco 1 do conjunto elegível a subir.
- LMP2 em `endurance` nunca se move.
- Invariantes por categoria devem ser verificados ao final.

**Compatibility Notes**

- Nenhuma migration ou struct de model será alterada.
- Checagem de licença consultará diretamente a tabela `licenses`.
- Pequenos helpers em queries podem ser adicionados se simplificarem leitura/escrita.

**Testing Strategy**

- TDD por módulo, começando por standings.
- Testes unitários para blocos e efeitos.
- Testes de pipeline cobrindo temporada 1, ordem promoção antes de mercado e manutenção dos tamanhos fixos.
