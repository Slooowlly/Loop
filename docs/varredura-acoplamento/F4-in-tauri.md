# F4 — `IN_TAURI` redefinido em 11 arquivos

**Área:** frontend · **Risco:** baixo, mas espalhado · **Conflita com:** nada

## O que foi encontrado

A detecção de "estou rodando dentro do shell Tauri ou no vite puro" é redefinida
localmente em 11 arquivos:

- `src/main.jsx`
- `src/lib/updater.js`
- `src/components/iracing/StartingCompoundsPanel.jsx`
- `src/overlay/EngineerRadio.jsx`
- `src/overlay/EngineerVrWriter.jsx`
- `src/overlay/OverlayLiveView.jsx`
- `src/overlay/OverlayMonitorAuto.jsx`
- `src/overlay/OverlayPositionPanel.jsx`
- `src/overlay/OverlayVrWriter.jsx`
- `src/overlay/useBreakdownFeed.js`
- `src/overlay/useOverlayData.js`

Não existe definição canônica exportada em `src/utils/` nem em `src/lib/`.

## Por que importa

`IN_TAURI` é o gate que decide se um `invoke` acontece ou se o código cai em mock /
no-op. Onze detecções independentes significam onze oportunidades de divergir —
e o modo de falha é sutil: no `npm run dev` (só frontend, sem shell Tauri) um
componente com a detecção "errada" tenta invocar e explode, ou pior, silencia um
erro que apareceria no app real.

Nove das onze cópias estão no overlay, que é justamente a parte mais difícil de
testar manualmente (janela transparente, always-on-top, sobre o iRacing).

## Armadilhas conhecidas

1. **As detecções podem não ser equivalentes.** Existem várias formas de detectar
   (`window.__TAURI__`, `window.__TAURI_INTERNALS__`, try/catch no import de
   `@tauri-apps/api`). Versões diferentes do Tauri v2 mudaram o global exposto —
   algumas dessas cópias podem ser de épocas diferentes e uma delas pode estar
   simplesmente errada hoje.
2. **Overlay é outra webview.** Confirme que a detecção vale igual nas três janelas
   declaradas em `tauri.conf.json` (principal, `overlay`, `engineer`).
3. Avaliação em tempo de import vs. em tempo de chamada muda comportamento em teste
   (jsdom com mock de `window`). Vários testes em `src/**/*.test.jsx` mockam
   `invoke` — um módulo compartilhado avaliado no import pode furar esses mocks.

## O que eu quero da segunda análise

1. **Cole as 11 definições lado a lado.** São iguais? Quantas variantes distintas
   existem de fato?
2. **Qual está correta para o Tauri v2 que o projeto usa?** Cheque a versão em
   `package.json`/`Cargo.toml` e a API oficial. Se alguma das 11 estiver errada,
   isso é um bug real e vira prioridade acima do refactor.
3. **Import-time ou call-time?** Recomende, considerando os testes que mockam
   `invoke`. Se for import-time, liste quais testes precisam de ajuste.
4. **Onde mora.** `src/lib/` parece o lugar (já tem `updater.js`), mas confirme que
   o overlay pode importar de lá sem arrastar dependência pesada.
5. **Plano de migração**, com `npm run test:ui` entre passos, e uma nota sobre como
   verificar o overlay — que a suíte de testes não cobre bem.

Não aplique nada ainda — quero ler a análise antes.
