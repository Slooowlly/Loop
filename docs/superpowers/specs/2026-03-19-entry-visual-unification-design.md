# Entry Visual Unification Design

**Contexto:** O app agora tem uma tela de boot com a logo nova antes da splash, mas `BootLogoScreen`, `SplashScreen` e `MainMenu` ainda parecem pertencer a universos visuais diferentes. O boot usa um fundo cinematográfico, enquanto splash e menu seguem a base simples `bg-app-bg`.

**Objetivo:** Usar a logo nova como âncora visual da experiência de entrada e unificar as três telas iniciais do app numa mesma família estética.

**Direção aprovada:** A identidade visual deve partir da logo, não do tema antigo do menu. Isso significa usar a paleta e a atmosfera da logo para orientar launcher, splash e menu.

**Linguagem visual:** Fundo escuro profundo, com halos frios azulados/violeta, brilho localizado e sensação de profundidade suave. O glassmorphism continua existindo, mas mais refinado e integrado ao fundo. A entrada deve parecer premium e contínua, não três telas independentes.

**Aplicação por tela:**
- `BootLogoScreen`: permanece a tela mais limpa, centrada na logo e em sua animação.
- `SplashScreen`: herda o mesmo fundo atmosférico e traz a logo como peça visual importante, junto ao CTA principal.
- `MainMenu`: abandona a aparência provisória e passa a funcionar como hub inicial glass, com botões mais coesos com a nova identidade.

**Arquitetura recomendada:** Colocar a base visual compartilhada em classes reutilizáveis no CSS global para evitar repetição entre páginas. Cada tela reaproveita essas classes e só ajusta layout e intensidade.

**Escopo:** O fluxo de rotas permanece igual. A mudança é apenas visual e estrutural no frontend dessas três telas.

**Validação:** Confirmar por teste de contrato que boot, splash e menu usam a logo nova e classes visuais compartilhadas. Depois, rodar `npm run build`.
