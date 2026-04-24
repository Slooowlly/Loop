# Window Controls Navigation Design

**Contexto:** O `WindowControlsDrawer` hoje existe somente dentro de `MainLayout`, então na prática ele aparece apenas no dashboard. O painel vertical recém-criado ainda é visual e não executa nenhuma ação.

**Objetivo:** Transformar três widgets do painel vertical em atalhos globais reais e tornar a bandeja disponível em todas as rotas do app, como acontece conceitualmente com os controles de minimizar, alternar fullscreen e fechar.

**Widgets aprovados:** O painel secundário terá, por enquanto, apenas três atalhos:
- `⚙️` navega para `/settings`
- `📂` navega para `/load-save`
- `🏠` navega para `/menu`

**Comportamento de menu:** O widget `🏠` deve herdar o comportamento do botão atual “Voltar ao menu” do dashboard. Isso significa limpar o estado da carreira com `clearCareer()` antes de navegar para `/menu`.

**Escopo global:** Para a bandeja existir de fato em todas as telas, `WindowControlsDrawer` deve subir para `src/App.jsx`, acima das rotas, e deixar de ser renderizado em `src/components/layout/MainLayout.jsx`.

**Integração visual:** Os widgets continuam no painel glassmorphism já aprovado, mas agora cada item recebe `title` nativo e comportamento de clique. O item correspondente à rota atual pode ganhar um destaque sutil para indicar contexto sem poluir a interface.

**Dashboard:** O botão “Voltar ao menu” do header do dashboard pode ser removido depois que o widget `🏠` estiver funcional, já que o app é de uso pessoal e esse atalho global substitui o papel dele.

**Validação:** Confirmar por testes de contrato que o drawer usa roteamento, define os três widgets aprovados, limpa a carreira ao ir para `/menu`, é renderizado em `App.jsx`, deixa de ser renderizado em `MainLayout.jsx` e remove o botão redundante do header. Em seguida, rodar `npm run build`.
