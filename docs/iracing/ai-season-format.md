# iRacing AI Season — formato de arquivo

Fica em `Documentos/iRacing/aiseasons/<nome>.json` (resolvido por
`iracing_sdk::paths::aiseasons_dir`). Uma "AI season" amarra um **roster** a um
**calendário de eventos** (pistas) + regras de corrida/clima.

## Campos de topo (principais)

| Campo | Significado |
|---|---|
| `rosterName` | Nome do **AI roster** vinculado (casa com a pasta em `airosters/`). Aqui: `"MAZDA TOYOTA BMW"` |
| `name` | Nome da season (ex. `"Pistas"`) |
| `aiCarClassId` | Classe principal (74 = MX-5) |
| `aiCarClassIds` | Todas as classes (multiclasse): `[74, 4012, 4108]` |
| `carId` / `userCarClassId` | Carro/classe do usuário (67 / 74) |
| `carSettings[]` | Por `car_id`: `max_pct_fuel_fill`, `max_dry_tire_sets` |
| `category_id` | Categoria iRacing — **5 = Sports Car** |
| `max_drivers` | Tamanho do grid (37) |
| `minSkill` / `maxSkill` | Faixa de skill da IA (25–50) |
| `gridPosition` | Posição de largada do usuário |
| `points_system_id` | Sistema de pontos |
| `multiclassType` | Tipo de multiclasse (2) |
| `subsessions` | Tipos de subsessão: `[5, 6]` (quali, corrida) |
| Regras de corrida | `race_length_type`, `race_length`, `race_laps`, `qualify_laps`, `qualify_length`, `practice_length`, `restarts`, `rolling_starts`, `full_course_cautions`, `incident_limit`, `damage_model`, etc. |
| `weather` | Objeto de clima (type, temp, umidade, vento, céu, `simulated_start_time`, etc.) |
| `track_state` | Borracha de pista por sessão (`leave_marbles`, `*_rubber`) |
| `events[]` | **O calendário** — ver abaixo |

## `events[]` — o calendário (cada item = uma etapa)

| Campo | Significado |
|---|---|
| `trackId` | **ID da pista no iRacing** (o "código" da pista) |
| `paceCar` | `{ category_id, car_id, is_oval, is_dirt, car_name, car_class_id, order }` — dá pra derivar **oval vs road** por evento |
| `num_opt_laps` | Voltas opcionais |
| `rolling_starts` | Largada lançada (presente nos ovais) |
| `subsessions` | `[5, 6]` |
| `eventId` | GUID do evento |

## Uso: base do conteúdo grátis (Production)

Os `trackId` desta season são o **pool de pistas grátis** para os carros grátis
(Mazda MX-5, Toyota GR86, BMW M2). Materializado em
[`free-content.json`](free-content.json): 3 carros/classes + 33 pistas
(2 ovais: 556, 15; 31 road), categoria Sports Car (5).

## Nomes das pistas — RESOLVIDO

A ordem do `events[]` casa 1:1 com a lista da UI do iRacing (confirmado: os 2 ovais
caem na posição exata — 556 Charlotte Oval, 15 Concord Speedway — e grupos de layout
têm trackIds consecutivos, ex. Oulton Park 180–186). A tabela `trackId → nome` está
preenchida em [`free-content.json`](free-content.json). Em runtime ainda dá pra
confirmar/atualizar pelo YAML (`WeekendInfo.TrackID` + `TrackDisplayName`).
