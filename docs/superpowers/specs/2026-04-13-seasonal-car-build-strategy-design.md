# Seasonal Car Build Strategy Design

**Goal**

Adicionar um sistema sazonal de construcao do carro em que cada equipe escolhe, na offseason, um perfil tecnico para a proxima temporada. O `car_performance` continua sendo a base do carro, enquanto o perfil escolhido gera um ajuste de pista que afeta apenas o carro e entra no quali/corrida via `effective_car_performance`.

**Context**

- Hoje as pistas ja possuem `track_character` e multiplicadores de stress em [src-tauri/src/simulation/track_profile.rs](/c:/Users/rodri/OneDrive/Área%20de%20Trabalho/Jogos/iRacerApp%20V1/src-tauri/src/simulation/track_profile.rs:17), mas ainda nao expressam pesos especificos de `aceleracao`, `potencia` e `dirigibilidade`.
- As equipes ja possuem `car_performance`, `confiabilidade` e `budget` em [src-tauri/src/models/team.rs](/c:/Users/rodri/OneDrive/Área%20de%20Trabalho/Jogos/iRacerApp%20V1/src-tauri/src/models/team.rs:77), e esse `car_performance` ja entra diretamente no score de quali/corrida.
- A pre-temporada ja simula o futuro em [src-tauri/src/market/preseason.rs](/c:/Users/rodri/OneDrive/Área%20de%20Trabalho/Jogos/iRacerApp%20V1/src-tauri/src/market/preseason.rs:224), o que a torna o lugar natural para a IA decidir o perfil do carro da temporada seguinte.

**Core Decisions**

- O perfil do carro nao e fixo da equipe. Ele e uma decisao de preseason e pode mudar a cada temporada.
- A IA escolhe o perfil olhando para o calendario da temporada seguinte, inclusive considerando contexto de promocao/rebaixamento.
- `car_performance` permanece como forca base do carro.
- O perfil do carro gera apenas um ajuste contextual por pista, forte o bastante para ser sentido e alterar duelos relevantes.
- O sistema usa perfis discretos para manter legibilidade e balanceamento sob controle.
- O perfil `balanceado` e mais caro. Perfis extremos sao mais baratos e, portanto, mais provaveis em equipes pobres.

**Track Attributes**

Cada pista em `track_profile.rs` passa a expor tambem:

- `acceleration_weight`
- `power_weight`
- `handling_weight`

Os pesos somam `100` e seguem a tabela aprovada durante o brainstorming. O catalogo de pistas continua centralizado em `track_profile.rs`, mantendo os pesos perto do `track_character` e dos multiplicadores de stress.

**Team Car Build Profiles**

As equipes passam a escolher um `car_build_profile` discreto por temporada:

- `Balanced`: `34 / 33 / 33`
- `AccelerationIntermediate`: `47 / 26.5 / 26.5`
- `PowerIntermediate`: `26.5 / 47 / 26.5`
- `HandlingIntermediate`: `26.5 / 26.5 / 47`
- `AccelerationExtreme`: `60 / 20 / 20`
- `PowerExtreme`: `20 / 60 / 20`
- `HandlingExtreme`: `20 / 20 / 60`

Cada perfil tambem recebe uma faixa de custo estrategico:

- `Balanced`: mais caro e premium
- `Intermediate`: custo medio
- `Extreme`: custo mais baixo

Os percentuais nao precisam ser persistidos separadamente. O enum do perfil e a fonte unica de verdade; helpers resolvem os pesos derivados.

**Simulation Formula**

O ajuste de pista usa sobreposicao direta entre perfil do carro e pesos da pista:

```text
team_match = dot(team_profile_weights, track_weights)
balanced_match = dot([34, 33, 33], track_weights)
advantage = team_match - balanced_match
track_delta = clamp(advantage / 2.5, -6.0, +6.0)
effective_car_performance = base_car_performance + track_delta
```

Consequencias desejadas:

- equipe balanceada e a referencia neutra em qualquer pista
- especializacao correta pode compensar uma diferenca relevante de tier
- especializacao errada pode punir bastante o carro
- o teto do efeito continua limitado por `clamp`

O piloto nao recebe nenhum buff/nerf novo. O impacto da pista acontece apenas no carro, via `effective_car_performance`.

**How It Enters Qualifying And Race**

- O backend calcula `effective_car_performance` antes de construir o `SimDriver`.
- `SimDriver` passa a carregar o valor efetivo para aquele evento.
- `qualifying.rs` e `race.rs` continuam praticamente iguais, porque ja usam `driver.car_performance`.
- O sistema de incidentes, confiabilidade e atributos do piloto permanece intacto.

Isso preserva o pipeline atual e minimiza regressao na simulacao.

**Offseason AI Decision**

A IA avalia todos os 7 perfis e escolhe o de maior score para a temporada seguinte. O score combina:

- `calendar_fit`: soma do encaixe do perfil nas pistas da proxima temporada
- `strategy_bias`: contexto competitivo da equipe
- `budget_bias`: viabilidade financeira do perfil
- `car_strength_bias`: incentivo a consistencia ou aposta com base no `car_performance` atual
- `movement_bias`: ajuste pelo risco/chance de promocao ou rebaixamento

Regras comportamentais aprovadas:

- equipes fortes e candidatas ao titulo tendem ao `Balanced`
- equipes seguras priorizam consistencia
- equipes em risco de rebaixamento aceitam mais variancia
- equipes mirando promocao podem assumir mais risco se o calendario futuro favorecer um foco especifico
- o calendario da proxima categoria tambem pesa quando promocao/rebaixamento e provavel

**Budget And Cost Strategy**

O custo do perfil deve funcionar de forma dupla:

- existe barreira de acesso por budget
- escolher o perfil tambem representa gasto/penalidade real de offseason

Mas o budget nao pode decidir sozinho. Ele entra como fator importante, nao como bloqueio absoluto. A escolha final precisa continuar refletindo `calendario + contexto competitivo + budget`.

**Balance Safeguards**

Para evitar distorcoes:

- `Balanced` e sempre a baseline neutra
- `track_delta` fica travado em `[-6.0, +6.0]`
- os perfis sao discretos e fechados; nao ha distribuicoes livres
- extremos devem aparecer mais em equipes pobres, nao em todo o grid
- a IA precisa considerar seguranca competitiva e nao apenas o melhor fit matematico

Essas travas reduzem os tres riscos principais levantados no brainstorming:

- explosao numerica do sistema
- escolhas previsiveis demais
- custo dominando todas as decisoes

**Persistence And Visibility**

- `Team` passa a persistir o `car_build_profile`
- a migration precisa preencher saves legados com `Balanced`
- `TeamSummary` pode expor o perfil atual para a UI
- a `MyTeamTab` pode mostrar o perfil do carro como informacao de temporada, sem precisar expor ainda todos os pesos internos

**Testing**

Cobertura minima esperada:

- `track_profile.rs`: pesos corretos por pista e fallback
- `team.rs` / `teams.rs`: persistencia e fallback do novo perfil
- `simulation`: testes de `effective_car_performance` para match correto, errado e balanceado
- `preseason.rs`: IA escolhendo perfis diferentes conforme calendario, budget e contexto competitivo
- `career.rs` / `career_types.rs`: serializacao do perfil para a UI, se exposto nesta fase

**Rollout**

Implementacao em fases:

1. adicionar pesos das pistas e helpers de calculo
2. persistir perfis de construcao do carro nas equipes
3. integrar `effective_car_performance` na simulacao
4. ensinar a IA da preseason a escolher o perfil da temporada
5. expor o perfil minimo para observabilidade na UI

