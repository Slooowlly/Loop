# iRacing AI Roster — formato de arquivo

Referência do JSON de **AI roster** do iRacing, base da integração carreira ↔ pista.
Geramos um roster com os pilotos do NOSSO app → o iRacing usa esses nomes → o SDK
(`DriverInfo.UserName`) devolve os mesmos nomes → mapeamento `CarIdx → piloto_id`
fica **automático** (casa por `driverName`/`carNumber`).

## Estrutura

Objeto raiz com uma lista `drivers`:

```json
{ "drivers": [ { /* piloto */ }, ... ] }
```

## Campos por piloto

| Campo | Tipo | Significado |
|---|---|---|
| `driverName` | string | Nome completo. **← campo-ponte com o nosso elenco** |
| `carNumber` | string | Número do carro. String (mantém zero à esquerda: "07", "940"). Bom segundo critério de match |
| `carPath` | string | Pasta do carro, ex. `mx5\\mx52016`, `toyotagr86`, `bmwm2g87` |
| `carId` | int | ID do carro no iRacing (67=MX-5, 160=GR86, 216=BMW M2 G87) |
| `carClassId` | int | ID da classe (74=MX-5, 4012=GR86, 4108=BMW M2). **Um roster pode ser multiclasse** |
| `carDesign` | string | Pintura: `padrão,cor1,cor2,cor3` (hex sem `#`; aceita `-` p/ multi: `000000-303030`; padrão pode ser vazio: `,FFF000,...`) |
| `suitDesign` | string | Macacão, mesmo formato |
| `helmetDesign` | string | Capacete, mesmo formato |
| `numberDesign` | string | Estilo do número: `fonte,?,cor1,cor2,cor3`, ex. `0,0,,,` ou `15,2,ffffff,0a0a0a,2e358f` |
| `sponsor1` | int | ID do decalque de patrocínio (opcional; ausente ⇒ 0) |
| `sponsor2` | int | ID do segundo patrocínio (opcional ⇒ 0) |
| `driverSkill` | int 0–100 | Habilidade |
| `driverAggression` | int 0–100 | Agressividade |
| `driverOptimism` | int 0–100 | Otimismo (arrisca brechas) |
| `driverSmoothness` | int 0–100 | Suavidade |
| `pitCrewSkill` | int 0–100 | Equipe de pit |
| `strategyRiskiness` | int 0–100 | Ousadia na estratégia |
| `driverAge` | int | Idade |
| `id` | string (GUID) | Identificador único do piloto no roster |
| `rowIndex` | int | Ordem no roster (0-based) |

## Notas

- **Multiclasse**: o exemplo agrupa 3 carros (MX-5 classe 74, GR86 4012, BMW M2 4108)
  no mesmo roster. A geração precisa setar `carPath`/`carId`/`carClassId` coerentes.
- Campos de design são cosméticos; pra v1 podem ser preenchidos com defaults.
- O que IMPORTA para a simulação/narrativa: `driverName`, `carNumber`, os 6 atributos
  0–100 (skill/aggression/optimism/smoothness/pitCrewSkill/strategyRiskiness) e `driverAge`.

## Mapeamento com o nosso modelo de piloto (DEFINIDO)

Identidade: `driverName` ← `driver.nome`. Números/idade/atributos abaixo (tudo
0–100; nosso `f64` → inteiro do roster, arredondar).

| Campo do roster | Fonte no nosso app |
|---|---|
| `driverName` | `driver.nome` |
| `driverSkill` | `driver.atributos.skill` |
| `driverAggression` | `driver.atributos.aggression` |
| `driverOptimism` | `driver.atributos.confianca` *(decisão do usuário)* |
| `driverSmoothness` | `driver.atributos.smoothness` |
| `driverAge` | `driver.idade` |
| `pitCrewSkill` | `team.pit_crew_quality` |
| `strategyRiskiness` | `team.pit_strategy_risk` |

Fontes: `DriverAttributes`/`Driver` em `src-tauri/src/models/driver.rs`; `Team` em
`src-tauri/src/models/team.rs` (`pit_crew_quality`, `pit_strategy_risk`).

- **Cosméticos**: usar a **cor base do time** (`team.cor_primaria`, hex com `#` →
  remover o `#` no roster). Pilotos da **mesma equipe ficam com a mesma cor**
  (`carDesign`/`suitDesign`/`helmetDesign`; padrão/cores secundárias = default).
- **Número do carro** (`carNumber`): o nosso piloto NÃO tem número — **atribuir** na
  geração (ex.: por posição de grid/campeonato, estável por piloto).
- **Multiclasse**: o exemplo é multiclasse só como referência; nem sempre usaremos.
  A geração só precisa ser capaz disso quando o campeonato pedir.

## Local em disco (resolvido)

Exemplo do usuário: `C:\Users\rodri\OneDrive\Documentos\iRacing\airosters\MAZDA TOYOTA BMW`.

A pasta é **resolvida automaticamente** em qualquer PC (OneDrive ou não, idioma
diferente, pasta movida) via Known Folder API do Windows — ver
`src-tauri/src/iracing_sdk/paths.rs` (`documents_dir` → `iracing_dir` →
`airosters_dir`/`aiseasons_dir`). Cada roster fica numa subpasta com o nome do
roster, contendo o JSON.

## Pendências (esperando o usuário)

- [ ] Formato do **aiseason** (que referencia airosters dentro).
- [ ] Confirmar a estrutura exata da subpasta do roster (nome de arquivo dentro de
      `airosters/<nome>/`).
