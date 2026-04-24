# Mechanical News Detail Design

## Goal

Fazer com que noticias de incidentes mecanicos usem o detalhe exato gerado pelo motor de simulacao, em vez de texto generico, e ampliar a cobertura editorial para incidentes mecanicos sem DNF e corridas anormalmente caoticas.

## Key Decisions

- `IncidentResult.description` passa a ser a fonte factual primaria para incidentes mecanicos.
- O `news/generator` nao deve reconsultar catalogo nem recriar a causa do problema; ele deve reutilizar o fato ja escolhido pela simulacao.
- `Mechanical + DNF` continua elegivel para noticia individual.
- `Mechanical` sem DNF passa a ser elegivel para noticia individual com prioridade editorial menor do que colisao critica, colisao com DNF e erro com DNF.
- Corridas com volume anormal de incidentes ganham uma noticia-resumo adicional, com consolidacao de DNFs e danos relevantes.
- O resumo usa apenas fatos ja presentes em `race_result.race_results[].incidents`, preservando contexto como dano latente apos colisao.

## Architecture

- `src-tauri/src/news/generator.rs`
  - Ajusta a prioridade editorial para incluir `Mechanical` sem DNF.
  - Faz noticias mecanicas individuais usarem o `incident.description` como corpo factual.
  - Adiciona um construtor de noticia-resumo para corridas caoticas.
- `src-tauri/src/news/flavour/templates/incidents.rs`
  - Mantem o papel editorial dos titulos, sem ser a fonte de causalidade mecanica.
- `src-tauri/src/simulation/incidents.rs`
  - Permanece como origem dos fatos mecanicos; nenhuma duplicacao de logica.
- `src-tauri/src/simulation/race.rs`
  - Ja entrega o conjunto de incidentes necessario para o resumo editorial.

## Editorial Rules

- Noticia individual mecanica:
  - titulo continua editorial;
  - corpo menciona o problema exato registrado em `incident.description`;
  - se houver `damage_origin_segment`, o texto preserva o contexto de dano consequente.
- Noticia-resumo de corrida caotica:
  - entra quando a corrida ultrapassa um limiar anormal de incidentes, DNFs ou combinacao de DNF e danos relevantes;
  - cita apenas os casos mais importantes para evitar texto excessivo;
  - mistura eliminacoes e danos sem DNF num unico item.

## Selection Model

- Ordem editorial proposta:
  - colisao critica;
  - colisao com DNF;
  - erro de pilotagem com DNF;
  - problema mecanico com DNF;
  - colisao major sem DNF;
  - problema mecanico sem DNF;
  - erro de pilotagem major sem DNF.
- A noticia-resumo nao substitui a noticia principal da corrida; ela complementa o feed quando a prova foge do normal.

## Risks

- Se toda falha mecanica sem DNF virar manchete individual, o feed pode ficar ruidoso.
- Se o resumo listar incidentes demais, a noticia perde legibilidade.
- Reaproveitar `incident.description` exige cuidado para nao duplicar prefixos editoriais ou produzir textos redundantes.

## Testing Strategy

- Validar que noticia mecanica individual usa o detalhe exato de `incident.description`.
- Validar que problema mecanico sem DNF passa a ser elegivel no ranking editorial.
- Validar que colisao critica continua vencendo incidentes mecanicos menores.
- Validar que corrida caotica gera noticia-resumo com fatos de DNF e dano sem DNF.
- Validar que dano latente continua aparecendo com contexto de origem quando aplicavel.
