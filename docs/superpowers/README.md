# superpowers/ — o que sobrou, e por quê

Este diretório guardava 88 arquivos de plano e spec de implementação, de 19/03 a 30/07/2026.
**82 foram removidos na limpeza de 11/08/2026.** Sobraram os 6 abaixo.

## Por que a limpeza aconteceu

Os arquivos descreviam trabalho **já entregue** e continuavam afirmando o contrário:

- 10 specs abriam com "não implementado" ou "Status: DESIGN" sobre sistema que está no ar.
- 13 dos 14 planos tinham caixa de tarefa aberta, e 10 deles com **zero** caixas marcadas.
  Somados passavam de 300 tarefas que se liam como pendentes, com as features todas prontas.
- Os specs de março descreviam telas que deixaram de existir, como `SplashScreen` e
  `BootLogoScreen`.
- Nenhum arquivo do repositório referenciava o diretório, fora os 6 que ficaram.

Quem perguntasse "o sistema de nível do carro está pronto?" recebia um "não" daqui, com o
`car/` inteiro no ar.

**Nada se perdeu.** Tudo continua no histórico do git: `git log --diff-filter=D -- docs/superpowers/`
lista as remoções, e `git show <commit>^:<caminho>` devolve qualquer arquivo.

## Os 6 que ficaram

Ficaram porque **o código Rust em produção os cita como referência de design em doc-comment**.
Apagá-los quebraria um ponteiro vivo.

| spec | citada por | estado real hoje |
|---|---|---|
| [2026-06-20-weekly-transfer-market-design.md](specs/2026-06-20-weekly-transfer-market-design.md) | `market/transfer_window.rs` | No ar. Falta a condução na UI (F-01). |
| [2026-07-12-player-skill-tracking-design.md](specs/2026-07-12-player-skill-tracking-design.md) | `player_skill.rs` | No ar. |
| [2026-07-17-car-level-system-design.md](specs/2026-07-17-car-level-system-design.md) | 9 arquivos de `car/`, `db/queries/team_car.rs`, `market/car_maintenance.rs`, `simulation/car_build.rs` | No ar. |
| [2026-07-18-car-breakdown-system.md](specs/2026-07-18-car-breakdown-system.md) | `car/breakdown.rs`, `car/wear.rs` | No ar. |
| [2026-07-18-track-rivalry-perception-design.md](specs/2026-07-18-track-rivalry-perception-design.md) | `iracing_sdk/rivalry_perception.rs`, `commands/iracing/resultado.rs`, `corridas_salvas.rs` | No ar. |
| [2026-07-19-team-rivalry-design.md](specs/2026-07-19-team-rivalry-design.md) | `rivalry/team.rs`, `models/team_rivalry.rs` | No ar. |

Os seis levam um aviso no topo dizendo isso. **Leia qualquer um deles como a intenção original
do desenho, e nunca como estado do app.**

## Onde está o estado do app

- [DESIGN.md](../DESIGN.md): o retrato do que existe hoje, capítulo por capítulo.
- [backlog.md](../backlog.md): o que falta, com id estável.
- [roadmap.md](../roadmap.md): o porquê de cada buraco aberto.
- [divida-tecnica.md](../divida-tecnica.md): o que já fechou, com data e prova.

## Regra para o futuro

Spec de design é útil enquanto o trabalho não existe. Depois que entra no ar, ela vira arqueologia
e passa a competir com o `DESIGN.md` pela resposta à mesma pergunta.

**Ao terminar uma feature:** leve o que interessa para o `DESIGN.md` e apague o spec, ou deixe o
spec e ponha o aviso de retrato histórico no topo, com a data e a prova de que está no ar. Spec
sem aviso e sem data é o que produziu esta limpeza.
