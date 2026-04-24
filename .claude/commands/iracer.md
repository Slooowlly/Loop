# iRacing Career Simulator — Skill File

Documento de referência técnica para a IA desenvolvedora.
Garante consistência, padrões e alinhamento ao design doc.

---

## 1. IDENTIDADE DO PROJETO

| Campo | Valor |
|-------|-------|
| Nome | iRacing Career Simulator |
| Tipo | App desktop offline, single-player |
| Stack | React + Tailwind CSS + Tauri 2 + Rust + SQLite |
| Build | Tauri gera .exe standalone para Windows |
| Dados | SQLite local (rusqlite com feature "bundled") |
| Exportação | JSON para integração com iRacing |
| Gráficos | Recharts |
| State mgmt | Zustand |
| Roteamento | React Router DOM |

---

## 2. ESTRUTURA DO PROJETO

```
src/                          → Frontend React
  components/ui/              → Componentes reutilizáveis (Button, Card, etc.)
  components/layout/          → Layout (Header, Sidebar, MainLayout)
  components/driver/          → Componentes de piloto
  components/team/            → Componentes de equipe
  components/race/            → Componentes de corrida
  components/charts/          → Gráficos Recharts
  pages/                      → Páginas (SplashScreen, Dashboard, etc.)
  pages/tabs/                 → 8 tabs do dashboard
  pages/history/              → Sub-páginas de história
  stores/                     → Zustand stores
  hooks/                      → Custom hooks
  utils/                      → Utilitários

src-tauri/src/                → Backend Rust
  commands/                   → Tauri commands (IPC com frontend)
  db/                         → Conexão SQLite + migrações
  db/queries/                 → Queries organizadas por domínio
  models/                     → Structs de dados (Driver, Team, Race, etc.)
  simulation/                 → Motor de simulação de corrida
  evolution/                  → Crescimento/declínio entre temporadas
  market/                     → Mercado de transferências
  promotion/                  → Promoção/rebaixamento
  hierarchy/                  → Hierarquia N1/N2 de equipe
  calendar/                   → Geração de calendário
  export/                     → Exportação para iRacing (JSON)
  news/                       → Sistema de notícias
  generators/                 → Geração de nomes, IDs, nacionalidades
  constants/                  → Dados estáticos (categorias, pistas, carros)
  config/                     → Configuração do app (config.json)
```

---

## 3. CONVENÇÕES DE CÓDIGO

### Rust

| Aspecto | Convenção |
|---------|-----------|
| Nomes de struct | PascalCase (`DriverData`, `TeamPerformance`) |
| Nomes de campo | snake_case (`car_performance`, `piloto_id`) |
| Nomes de função | snake_case (`calculate_score`, `next_id`) |
| Nomes de módulo | snake_case (arquivo = módulo) |
| Enums | PascalCase variantes (`DriverStatus::Ativo`) |
| Enums no SQLite | Armazenados como TEXT via `as_str()` / `from_str()` |
| Erros | Usar `thiserror` para tipos de erro. Implementar `Serialize` para retorno Tauri |
| Serialização | `serde` com `Serialize, Deserialize` em todas as structs públicas |
| Campos JSON no SQLite | Armazenados como TEXT, parse com `serde_json` ao ler |
| Testes | Módulo `#[cfg(test)]` com banco in-memory |
| Comentários | Em português nos comentários de negócio, inglês nos técnicos |

### React / JavaScript

| Aspecto | Convenção |
|---------|-----------|
| Componentes | PascalCase, um por arquivo (`DriverCard.jsx`) |
| Hooks | camelCase com prefixo `use` (`useTauri.js`) |
| Stores | camelCase com prefixo `use` (`useCareerStore.js`) |
| Utils | camelCase (`formatters.js`) |
| Props | camelCase (`driverName`, `teamId`) |
| Chamadas Tauri | Sempre via `invoke()` de `@tauri-apps/api/core` |
| Async | Todas as chamadas ao backend são `async/await` |
| CSS | Tailwind classes inline. Sem CSS modules. |
| Componentes | Functional components com hooks (sem classes) |

---

## 4. PADRÃO DE COMUNICAÇÃO FRONTEND ↔ BACKEND

```
Frontend (React)          ←→  Backend (Rust)
     │                              │
     │  invoke("command_name",      │
     │         { param1, param2 })  │
     │  ──────────────────────────→ │
     │                              │  #[tauri::command]
     │                              │  fn command_name(param1, param2)
     │                              │    → Result<ResponseType, String>
     │  ←────────────────────────── │
     │  receives JSON (auto serde)  │
```

**Regras:**
- Comandos Tauri retornam `Result<T, String>` onde T é Serializable
- Erros são `String` (Tauri serializa automático)
- Parâmetros complexos: usar struct com `Deserialize`
- Respostas complexas: usar struct com `Serialize`
- Nomes dos commands: snake_case no Rust, camelCase no JS (Tauri converte automaticamente)

**Exemplo de padrão:**
```
Rust: fn create_career(request: NewCareerRequest) -> Result<NewCareerResponse, String>
JS:   invoke("create_career", { request: { player_name: "João", ... } })
```

---

## 5. PADRÃO DE DADOS NO SQLITE

### Tipos de coluna usados

| Tipo no design | Tipo SQLite | Notas |
|----------------|-------------|-------|
| string | TEXT | |
| int | INTEGER | |
| float / REAL | REAL | |
| bool | INTEGER | 0 = false, 1 = true |
| enum | TEXT | Armazenado como string (`as_str()`) |
| array/object | TEXT | JSON serializado |
| date/datetime | TEXT | Formato ISO: "2024-01-20T14:30:00" |
| nullable | Sem NOT NULL | Checa com Option<T> no Rust |

### PRAGMAs obrigatórios (todo banco)
```sql
PRAGMA journal_mode=WAL;
PRAGMA synchronous=NORMAL;
PRAGMA foreign_keys=ON;
PRAGMA busy_timeout=5000;
```

### Padrão de IDs
| Entidade | Prefixo | Formato | Exemplo |
|----------|---------|---------|---------|
| Piloto | P | P + 3 dígitos | P001, P042 |
| Equipe | T | T + 3 dígitos | T001, T015 |
| Temporada | S | S + 3 dígitos | S001 |
| Corrida | R | R + 3 dígitos | R001 |
| Contrato | C | C + 3 dígitos | C001 |
| Notícia | N | N + 3 dígitos | N001 |
| Rivalidade | RV | RV + 3 dígitos | RV001 |

IDs são sequenciais, controlados pela tabela `meta` (chave `next_driver_id`, etc).

---

## 6. PADRÃO DE ERROS

### Rust
```
Cada módulo pode ter seu próprio tipo de erro via thiserror.
Nos commands Tauri: converter para String com .map_err(|e| e.to_string())
Nunca usar .unwrap() em código de produção — sempre Result.
Panic é bug, não fluxo normal.
```

### Frontend
```
Toda chamada invoke() em try/catch.
Erros mostrados via Toast (store de notificações).
Nunca falhar silenciosamente.
```

---

## 7. REGRAS DE NEGÓCIO CRÍTICAS

Estas regras NUNCA devem ser violadas. Se em dúvida, consulte o design doc.

### Tamanhos fixos de categoria (invariante)
```
mazda_rookie:          6 equipes × 2 pilotos = 12
toyota_rookie:         6 equipes × 2 pilotos = 12
mazda_amador:         10 equipes × 2 pilotos = 20
toyota_amador:        10 equipes × 2 pilotos = 20
bmw_m2:              10 equipes × 2 pilotos = 20
production_challenger: 15 equipes (5+5+5) × 2 = 30
gt4:                  10 equipes × 2 pilotos = 20
gt3:                  14 equipes × 2 pilotos = 28
endurance:            17 equipes (6+6+5) × 2 = 34

TOTAL: 98 equipes, 196 pilotos
```
Após QUALQUER operação de mercado/promoção/rebaixamento, verificar que todas as categorias mantêm exatamente esses números.

### Sistema de tags (NUNCA mostrar números ao jogador)
```
Atributos 0-100 NUNCA aparecem como número na UI.
≤25: tag de defeito (vermelho/laranja)
26-74: invisível (não mostra nada)
≥75: tag de qualidade (verde/azul/roxo)
Exceção: motivação aparece como barra de progresso.
```

### Crescimento sem potencial fixo
```
Nenhum piloto tem "potencial máximo" no banco.
Crescimento é calculado por: resultados + idade + categoria + diminishing returns.
Não existe campo "potencial" em nenhuma tabela.
```

### Jogador = mesma struct que IA
```
O piloto do jogador usa a mesma struct Driver que a IA.
Campo is_jogador = true diferencia comportamento, não estrutura.
Sem campos exclusivos do jogador.
O jogador NÃO tem personalidade atribuída.
```

---

## 8. PADRÃO VISUAL (FRONTEND)

### Tema
```
SEMPRE dark theme. Não existe light theme na v1.0.
Background: #0E0E10
Cards: #1C1C1E com borda #21262d
Accent: #58a6ff (azul)
Sucesso: #3fb950 (verde)
Erro: #f85149 (vermelho)
Texto principal: #e6edf3
Texto secundário: #7d8590
```

### Componentes base (Tailwind)
```
Cards: classe "glass-card" (bg-app-card/80 backdrop-blur-md border border-border rounded-lg)
Botões: 4 variantes (Primary=azul, Secondary=cinza, Success=verde, Danger=vermelho)
Fontes: Segoe UI para texto, Consolas para números
```

### Animações
```
Transições: duration-200 para hovers
Cards: entrada staggered (delay por index)
Toasts: canto superior direito, auto-dismiss
```

---

## 9. ORDEM DE IMPLEMENTAÇÃO

```
FASE 1 — FUNDAÇÃO
  ✅ Passo 1:  Estrutura de pastas
  → Passo 2:  Módulos 03+08 (Banco SQLite, schema, IDs)
    Passo 3:  Módulo 05 (Constantes: categorias, pistas, carros)
    Passo 4:  Módulo 04 (Config app completa)

FASE 2 — MODELOS DE DADOS
    Passo 5:  Módulos 09+10 (Model Driver completo)
    Passo 6:  Módulos 16-18 (Model Team completo)
    Passo 7:  Módulo 07 (Geração de nomes e nacionalidades)

FASE 3 — CRIAÇÃO DE CARREIRA
    Passo 8:  Módulo 53 (Wizard Nova Carreira — backend: popular banco)
    Passo 9:  Módulo 53 (Wizard Nova Carreira — frontend: UI multi-step)
    Passo 10: Módulo 48 (Geração de calendário)

FASE 4 — SIMULAÇÃO
    Passo 11: Módulos 19-21 (Contexto + Classificação)
    Passo 12: Módulos 22-24 (Corrida 5 segmentos + degradação)
    Passo 13: Módulos 25-27 (Incidentes + lesões + clima)
    Passo 14: Módulo 28 (Pontuação + standings)

FASE 5 — INTERFACE PRINCIPAL
    Passo 15: Módulo 62 (Componentes UI base)
    Passo 16: Módulo 54 (Layout principal + navegação)
    Passo 17: Módulo 55 (Tab Pilotos/Standings)
    Passo 18: Módulo 56-57 (Tab Race Weekend)
    Passo 19: Módulo 59 (Fichas piloto/equipe)

FASE 6 — ENTRE-TEMPORADAS
    Passo 20: Módulos 29-31 (Evolução + motivação + crescimento)
    Passo 21: Módulos 32-34 (Experiência + lesões + aposentadoria)
    Passo 22: Módulos 35-40 (Mercado de transferências)
    Passo 23: Módulos 06+41 (Promoção/rebaixamento)
    Passo 24: Módulo 42 (Hierarquia N1/N2)
    Passo 25: Módulo 58 (Pipeline fim de temporada)

FASE 7 — EXPORTAÇÃO E INTEGRAÇÃO
    Passo 26: Módulos 44-47 (Exportação iRacing)
    Passo 27: Módulo 50 (Config sessão iRacing)
    Passo 28: Módulo 57 (Importação resultado iRacing + watchdog)

FASE 8 — POLISH
    Passo 29: Módulo 43 (Sistema de notícias)
    Passo 30: Módulo 61 (História + troféus)
    Passo 31: Módulo 02 (Splash, saves, loading)
    Passo 32: Tabs restantes (MyTeam, Prediction, Market, News, Other, Profile)
```

---

## 10. CHECKLIST POR PASSO

```
[ ] Compila sem erros (cargo build + npm run build)
[ ] Funcionalidade testável (botão ou comando que executa)
[ ] Nenhum .unwrap() em código de produção
[ ] Structs com Serialize/Deserialize onde necessário
[ ] mod.rs atualizado para exportar novos módulos
[ ] lib.rs atualizado com novos commands (se houver)
[ ] Não quebrou nada dos passos anteriores
```

---

## 11. O QUE NÃO FAZER

```
❌ Não criar potencial máximo de piloto
❌ Não mostrar números de atributos na UI (usar tags)
❌ Não usar .unwrap() em produção
❌ Não criar sistema de reservas (não existe)
❌ Não pular categorias na progressão
❌ Não criar campos exclusivos do jogador na struct Driver
❌ Não hardcodar paths (usar app_data_dir do Tauri)
❌ Não criar light theme
❌ Não importar saves do app Python antigo
❌ Não usar CSS modules (só Tailwind inline)
❌ Não criar componentes de classe React (só functional)
```
