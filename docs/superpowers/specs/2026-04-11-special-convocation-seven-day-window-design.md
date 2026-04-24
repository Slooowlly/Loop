# Special Convocation Seven-Day Window Design

## Goal
Transformar a `Janela de Convocação` em um mercado especial vivo de `7 dias`, onde `Production` e `Endurance` são preenchidos progressivamente por propostas, aceite dos pilotos e decisões estratégicas das equipes, sem quebrar a separação entre contrato regular e contrato especial.

## Player Fantasy

### Tensão diária
- A janela especial deixa de ser uma tela estática com uma decisão instantânea.
- O jogador acompanha o mercado dia a dia.
- Ao avançar o dia, vê quais pilotos foram convocados e quais equipes ainda seguem abertas.
- A experiência desejada é de expectativa: o jogador não sabe se será chamado, quando será chamado nem se será escolhido pela equipe caso aceite.

### Contrato regular intacto
- A convocação especial continua sendo paralela ao contrato regular.
- Nenhuma proposta especial abre vaga no grid normal.
- Nenhum vínculo regular é removido ou sobrescrito durante a janela.

## Window Structure

### Duração
- A janela dura exatamente `7 dias`.
- O frontend precisa permitir `avançar dia`.
- O backend precisa persistir o dia atual da janela.

### Início do dia
- O jogador vê:
  - grid atual de `Production` e `Endurance`;
  - equipes já preenchidas;
  - equipes ainda com vaga;
  - propostas pendentes recebidas;
  - sua proposta aceita ativa do dia, se existir.

### Fechamento do dia
- O jogador não é confirmado no momento do clique.
- A resposta do jogador entra na resolução do fechamento do dia.
- Cada equipe decide se fecha com alguém, mantém a disputa viva ou segura a vaga.
- O resultado aparece apenas depois do avanço do dia.

## Offer Semantics

### Natureza da proposta
- Proposta especial é `interesse formal`, não contrato garantido.
- O piloto pode receber várias propostas ao longo da janela.
- Em um mesmo dia, o piloto só pode ter `uma` proposta marcada como `aceita ativa`.
- As demais propostas permanecem pendentes, não recusadas automaticamente.

### Persistência
- Propostas pendentes continuam disponíveis no dia seguinte.
- Elas só expiram quando:
  - a equipe fecha com outro piloto;
  - a janela termina;
  - o sistema as invalida por mudança estrutural da vaga.

### Resultado da aceitação
- Aceitar uma proposta não garante convocação.
- Se dois ou mais pilotos aceitarem a mesma equipe, a equipe escolhe no fechamento.
- O critério final pertence à equipe, não ao piloto.

## Market Dynamics

### Cobiça especial
- Cada piloto elegível recebe um valor de `cobiça especial`.
- Esse valor representa o quanto o mercado especial o deseja naquele ano.
- A cobiça pode considerar:
  - categoria de origem;
  - licença;
  - força/skill;
  - desempenho recente;
  - prestígio esportivo.

### Efeito da cobiça
- Pilotos mais cobiçados:
  - recebem mais propostas;
  - recebem propostas mais cedo;
  - tendem a decidir o mercado nos dias iniciais.
- Pilotos chamados mais tarde entram no efeito cascata:
  - vagas rejeitadas por pilotos mais fortes;
  - equipes que esperaram demais;
  - equipes mais fracas ou oportunistas.

### Curva da janela
- `Dias 1-2`: elite do mercado.
- `Dias 3-4`: cascata intermediária.
- `Dias 5-7`: fechamento tardio e vagas estratégicas remanescentes.

## Team Decision Model

### Força esportiva
- Equipes mais fortes tendem a mirar pilotos mais cobiçados.
- Equipes mais fracas tendem a depender mais do efeito cascata.

### Perfil de mercado
- Cada equipe especial recebe um perfil comportamental:
  - `agressiva`;
  - `paciente`;
  - `oportunista`;
  - `conservadora`.

### Comportamento esperado
- `Agressiva`: propõe cedo e fecha rápido.
- `Paciente`: segura vaga esperando mercado se mover.
- `Oportunista`: reage a recusas e sobras.
- `Conservadora`: só fecha quando encontra encaixe forte.

### Resolução no fechamento do dia
- Para cada vaga aberta, a equipe:
  1. decide se age ou espera;
  2. avalia os pilotos que aceitaram sua proposta;
  3. ordena candidatos por atratividade;
  4. fecha com o melhor disponível, se fizer sentido;
  5. ou mantém a vaga aberta para o dia seguinte.

## Eligibility Table

### Papel da tabela lateral
- A tela precisa mostrar uma tabela de pilotos elegíveis semelhante ao mercado normal.
- Essa tabela mostra apenas pilotos ainda `sem time especial`.
- Pilotos já convocados desaparecem da tabela e passam a existir apenas no grid das equipes.

### Filtros por campeonato
- Se `Production` estiver selecionado:
  - aparecem apenas pilotos com licença compatível para o bloco.
- Se `Endurance` estiver selecionado:
  - aparecem apenas pilotos de elite elegíveis ao campeonato.

### Organização visual
- A tabela continua mostrando:
  - nome do piloto;
  - carteira/licença;
  - categoria de origem.
- Os pilotos devem ser agrupados ou ao menos rotulados por categoria anterior para preservar leitura de carreira.

## UI Behavior

### Grid principal
- O grid completo de `Production` e `Endurance` continua sendo o protagonista da tela.
- Ao avançar o dia, o jogador precisa enxergar claramente:
  - quais equipes preencheram vaga;
  - qual piloto entrou em qual equipe;
  - quais vagas continuam abertas.

### Propostas do jogador
- As propostas do jogador continuam existindo em uma área própria.
- Essa área deixa de ser uma decisão instantânea e passa a ser um painel de gestão diária:
  - propostas pendentes;
  - proposta aceita ativa;
  - resultado após fechamento.

### Ação principal
- A CTA principal deixa de ser apenas `entrar ou não entrar`.
- O fluxo principal passa a ser:
  - revisar dia atual;
  - escolher uma proposta ativa, se houver;
  - avançar para o próximo dia;
  - chegar ao dia 7 com resultado final consolidado.

## Backend State Model

### Special window state
- O sistema precisa persistir:
  - dia atual `1..7`;
  - status da janela;
  - se a janela já foi resolvida;
  - se o jogador terminou convocado ou ficou fora.

### Candidate pool
- O pool elegível deve registrar, por piloto:
  - identidade;
  - categoria de origem;
  - licença;
  - cobiça especial;
  - status da janela (`livre`, `com_propostas`, `aceitou_time_x`, `convocado`).

### Team slots
- Cada vaga especial precisa carregar:
  - equipe;
  - categoria especial;
  - classe/carro;
  - força esportiva;
  - perfil de mercado;
  - status da vaga.

### Offers
- Cada proposta especial precisa registrar:
  - equipe;
  - piloto;
  - dia de criação;
  - status (`pendente`, `aceita_ativa`, `expirada`, `convertida_em_convocacao`, `recusada`, `perdida_no_fechamento`).

### Daily log
- O sistema precisa guardar um log diário para alimentar a UI e testes:
  - propostas emitidas;
  - propostas expiradas;
  - pilotos convocados;
  - vagas seguradas;
  - decisões importantes do mercado.

## Boundary with Normal Market

### Separação obrigatória
- `Production` e `Endurance` continuam fora da `PreSeasonView`.
- A janela especial usa estado, ofertas e grid próprios.
- Nenhuma query do mercado normal pode ler ou expor esse estado como se fosse contrato regular.

### Encerramento
- Ao final da janela:
  - convocados seguem para o bloco especial;
  - não convocados retornam ao fluxo normal sem efeito colateral;
  - contratos especiais continuam temporários e paralelos.

## Non-Goals
- Não transformar o especial em um mercado aberto completo igual ao da pré-temporada.
- Não permitir múltiplos aceites ativos por piloto no mesmo dia.
- Não misturar propostas especiais com renovação ou contratação regular.
- Não tornar a decisão das equipes puramente aleatória.

## Testing
- Validar que a janela dura exatamente `7 dias`.
- Validar que o jogador só pode ter uma proposta aceita ativa por dia.
- Validar que propostas pendentes persistem até a equipe fechar com outro piloto.
- Validar que pilotos convocados saem da tabela de elegíveis e entram no grid.
- Validar que equipes fortes e agressivas atuam mais cedo sobre pilotos mais cobiçados.
- Validar que equipes podem segurar vaga por estratégia.
- Validar que aceitar proposta não garante convocação.
- Validar que o contrato regular do jogador e dos NPCs permanece intacto durante toda a janela.
