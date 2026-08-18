# Loop

Jogo desktop de carreira no automobilismo, construído em volta do iRacing.

O jogador controla **um piloto** subindo uma pirâmide de 9 categorias, do Mazda Rookie ao
Endurance. Ao redor dele o Loop mantém um mundo vivo de 204 pilotos de IA em 102 equipes, que
correm, evoluem, trocam de equipe, se lesionam, criam rivalidades e se aposentam sozinhos.

**O caminho principal é correr a etapa de verdade.** O Loop exporta o grid e o calendário como AI
roster e AI season do iRacing, o jogador corre a prova dentro do simulador, e o resultado oficial
volta para a carreira. A simulação interna preenche o que ele não corre: as outras 8 categorias,
as etapas puladas e o mundo inteiro entre uma corrida e outra.

**"Offline" vale para os dados, e não para o propósito.** Não há servidor, conta nem login: a
carreira inteira vive num SQLite na máquina do jogador.

## Alvo: Windows

Não é preferência. O SDK do iRacing e a winapi são Windows-only, e fora do Windows a integração
compila como stub inerte. O CI roda em `windows-latest`.

## Stack

| Camada | Tecnologia |
|---|---|
| Shell desktop | Tauri v2 |
| Simulação e domínio | Rust, com SQLite via `rusqlite` |
| Frontend | React 18 + Vite |
| Estilo | Tailwind CSS |
| i18n | i18next no frontend, `rust-i18n` no backend |
| Overlay em corrida | Camada OpenXR em C++ (`vr-overlay/`) |

Toda a simulação vive em Rust. O React desenha e dispara `invoke`, sem camada de abstração da
ponte no meio.

## Como rodar

### Pré-requisitos

- **Node.js 24 ou maior.** O `package-lock.json` é escrito pelo npm 11, e o npm 10 que vem com o
  Node 20 recusa o lock.
- Rust stable.
- Windows, pelos motivos acima.

### Desenvolvimento

```bash
npm install
npm run tauri dev
```

`npm install` também instala o hook de pre-commit que faz valer a regra de i18n.

Só o frontend, em http://localhost:1420:

```bash
npm run dev
```

### Build

```bash
npm run build
```

O `npm run build` gera o `dist/`, e ele é **pré-requisito do lado Rust**: o
`tauri::generate_context!` embute os assets de `dist/` em tempo de compilação, então sem ele o
crate não compila.

## Testes

São três suítes independentes.

```bash
npm run test:ui          # vitest, jsdom, src/**/*.test.{js,jsx}
npm run test:structure   # guards estruturais e visuais (scripts/rodar-guards.mjs)
npm run test:all         # as duas de JS
```

```bash
cd src-tauri; cargo test
```

Sempre de dentro de `src-tauri/`, nunca com `--manifest-path` a partir da raiz: o cargo procura o
`.cargo/config.toml` a partir do diretório atual, e da raiz ele ignora o `target-dir` configurado
e recompila tudo num diretório novo.

Os **guards estruturais** são a parte menos óbvia da suíte. Eles leem o código-fonte como texto e
travam decisões de projeto que um teste de comportamento não pega: alinhamento de layout, paleta
de cores de equipe, contratos dos controles de janela, teto de `unwrap` no Rust de produção,
diretivas obrigatórias da CSP, e a proibição de falha engolida sem rastro no caminho de corrida.
Ao mexer em layout ou em paleta, espere que essa suíte reclame.

## Estrutura

```
Loop/
├── src/                     Frontend React
│   ├── components/          Por domínio: race, market, driver, team, standings, ui, wizard...
│   ├── pages/               Telas (MainMenu, Dashboard, NewCareer, LoadSave, Settings)
│   │   └── tabs/            As abas dentro do Dashboard
│   ├── stores/              Zustand: useCareerStore como hub, slices em stores/career/
│   ├── overlay/             Overlay de corrida (torre, rádio do engenheiro)
│   ├── i18n/                i18next, um namespace por área
│   └── assets/              Bandeiras, áudio do engenheiro
├── src-tauri/               Backend Rust / Tauri
│   └── src/
│       ├── commands/        Os comandos expostos na ponte
│       ├── simulation/      Motor de corrida: quali, corrida, incidentes, pontuação
│       ├── market/          Mercado entre temporadas: propostas, renovação, assédio
│       ├── evolution/       Crescimento e declínio por idade, licenças, rookies
│       ├── promotion/       Promoção e rebaixamento na escada de categorias
│       ├── iracing_sdk/     Telemetria e sessão do iRacing real
│       ├── engenheiro.rs    Rádio do engenheiro em corrida
│       ├── narrative/       Geração de notícias determinística
│       ├── constants/       Categorias, pistas, carros, pontuação, equipes
│       └── db/              Conexão, migrações versionadas e queries por área
├── vr-overlay/              Camada OpenXR em C++, para uso em VR
├── scripts/                 Release, auditoria de i18n e os guards estruturais
└── docs/                    Documentação interna
```

## Convenções que quebram o build se ignoradas

**O código, os comentários e a UI são em português.** Mantenha o padrão ao escrever código novo.

**i18n é obrigatório e tem hook.** Um pre-commit bloqueia strings de UI em português fora de `t()`
nos arquivos `.jsx` e `.js` no stage. O passivo que já existia está congelado em
`scripts/i18nBaseline.mjs`, frase por frase, e entrada nova nunca se acrescenta a ele para liberar
commit. As exceções intencionais são `{/* i18n-ignore */}` na linha ou na linha acima, e
`// i18n-ignore-file` no arquivo.

**Comando novo só existe depois de registrado** no `invoke_handler` do `lib.rs`. Um guard cobra
que todo `invoke("...")` do frontend exista nessa lista.

**Migração nunca se edita depois de lançada.** O array `MIGRATIONS` em `db/migrations.rs` é a
única fonte da ordem: adicionar é uma linha nele mais o bump do `CURRENT_VERSION`.

**A versão tem fonte única.** O `package.json` manda, e o `tauri.conf.json` e o `Cargo.toml`
espelham. Use `node scripts/release.mjs` para bumpar, que sincroniza os três, assina e publica.

## Documentação

| Documento | O que traz |
|---|---|
| [CLAUDE.md](CLAUDE.md) | O guia curto de quem vai mexer no código |
| [docs/DESIGN.md](docs/DESIGN.md) | Retrato completo do domínio |
| [docs/iracing-escopo.md](docs/iracing-escopo.md) | O que a integração com o iRacing se propõe a fazer |
| [docs/iracing-dados-disponiveis.md](docs/iracing-dados-disponiveis.md) | O que a telemetria entrega, o que não entrega e as armadilhas medidas |
| [docs/roadmap.md](docs/roadmap.md) | O que falta e em que ordem |
| [docs/divida-tecnica.md](docs/divida-tecnica.md) | Dívida técnica, com o que já fechou e a data |
| [docs/i18n-translation-spec.md](docs/i18n-translation-spec.md) | A fila de tradução e o estado de cada aba |

## Licença

Projeto privado, todos os direitos reservados.
