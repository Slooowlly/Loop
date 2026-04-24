# 🏎️ iRacerApp V1

> **Um simulador de carreira nas corridas** — gerencie seu piloto, conquiste contratos, survive rivalidades e seja campeão.

Construído com **Tauri v2** (Rust) + **React** + **Vite**, o iRacerApp é uma aplicação desktop que simula uma carreira completa no automobilismo. Você começa como um piloto desconhecido, assina com uma equipe, disputa temporadas completas e acompanha cada detalhe da sua trajetória através de um dashboard moderno e imersivo.

---

## ✨ Funcionalidades

### 🏁 Simulação de Corridas
- Motor de simulação em Rust com cálculo probabilístico por desempenho, condições de pista e rivalidades
- Resultados detalhados por piloto: posição, pontos, delta de tempo, incidentes e lesões
- Histórico completo de corridas por temporada

### 📰 Sistema de Notícias
- Feed de notícias gerado automaticamente após cada corrida
- Cobertura de vitórias, rivalidades, incidentes, lesões, recordes e transferências
- Narrativa contextual baseada no histórico recente do piloto (sequências de vitórias, rebaixamentos, etc.)
- Filtros por categoria: Corridas, Mercado, Geral

### 💼 Mercado de Transferências (Pré-Temporada)
- Dashboard glassmorphism de negociação de contratos entre temporadas
- Ofertas dinâmicas com base no desempenho atual do piloto
- Visualização de equipes por tier, orçamento e interesse

### 📊 Dashboard do Piloto
- Painel central com estatísticas de carreira: vitórias, pódios, poles, campeonatos
- Radar de atributos do piloto (velocidade, consistência, pilotagem em chuva, etc.)
- Tabela de classificação da temporada em tempo real

### 🔚 Fim de Temporada
- Tela de encerramento com resumo completo da temporada
- Ranking final de pilotos e equipes
- Transição automática para a pré-temporada seguinte

### ⚙️ Configurações
- Suporte a idiomas (PT-BR / EN)
- Autosave configurável
- Integração com caminho do iRacing (expansão futura)

---

## 🧱 Stack Tecnológica

| Camada | Tecnologia |
|--------|-----------|
| Desktop Shell | [Tauri v2](https://tauri.app/) |
| Backend / Motor | Rust (SQLite via `rusqlite`) |
| Frontend | React 18 + Vite |
| Estilização | Tailwind CSS + CSS Modules |
| Banco de Dados | SQLite (persistência local) |
| Build | `npm run tauri dev` / `tauri build` |

---

## 🗂️ Estrutura do Projeto

```
iRacerApp V1/
├── src/                        # Frontend React
│   ├── components/             # Componentes por domínio (news, race, season, market...)
│   ├── pages/                  # Abas principais da UI (tabs)
│   └── utils/                  # Formatters e helpers
├── src-tauri/                  # Backend Rust / Tauri
│   └── src/
│       ├── commands/           # IPC commands expostos ao frontend
│       ├── simulation/         # Motor de simulação de corridas
│       ├── market/             # Lógica de mercado e contratos
│       ├── news/               # Gerador de notícias
│       └── db/                 # Camada de banco de dados SQLite
├── public/                     # Assets públicos
├── bandeiras/                  # Imagens de bandeiras por país
└── docs/                       # Documentação interna
```

---

## 🚀 Como Rodar

### Pré-requisitos
- [Node.js](https://nodejs.org/) 18+
- [Rust](https://rustup.rs/) (stable)
- [Tauri CLI](https://tauri.app/v2/guides/getting-started/setup/)

### Desenvolvimento
```bash
npm install
npm run tauri dev
```

### Build de Produção
```bash
npm run tauri build
```

---

## 📌 Status do Projeto

**Em desenvolvimento ativo.** As funcionalidades principais estão operacionais; expansões planejadas incluem modo multiplayer local, histórico de carreira exportável e integração real com dados do iRacing.

---

## 📄 Licença

Projeto privado — todos os direitos reservados.
