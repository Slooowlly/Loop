---
name: release
description: Publica uma versão do Loop com scripts/release.mjs — bump sincronizado das três fontes de versão, build assinado, manifesto, upload no bucket e verificação no ar. Use SEMPRE que a tarefa for lançar ou versionar: "solta uma versão", "faz o release", "bump de versão", "publicar update", "gerar instalador", "sobe pra galera", ou quando alguém perguntar por que package.json, tauri.conf.json e Cargo.toml estão com versões diferentes. Use também antes de editar qualquer um desses três arquivos à mão, porque a sincronia é responsabilidade do script e editar manualmente quebra o updater.
---

# Release do Loop

Publicar afeta gente fora desta máquina: o instalador vai para um bucket e o
updater dos jogadores aponta para o manifesto. É uma ação de mão única —
confirme com o usuário antes de rodar, mesmo que o pedido pareça claro. "Bumpa a
versão" nem sempre quer dizer "publica agora".

## Versão tem fonte única

`package.json` é a fonte. `src-tauri/tauri.conf.json` e `src-tauri/Cargo.toml`
**espelham**. O `vite.config.js` injeta `__APP_VERSION__` (do package.json) e
`__APP_BUILD__` (contagem de commits do git).

Editar os três à mão é o caminho conhecido para publicar um instalador cuja
versão interna não bate com a do manifesto — o updater então oferece a mesma
atualização em loop, ou nenhuma. Deixe o `scripts/release.mjs` sincronizar.

## O comando

```bash
node scripts/release.mjs --bump patch --notes "Corrige o cálculo de pontos no endurance"
```

`--bump` aceita `major`, `minor` ou `patch` (padrão: `patch`). Para fixar uma
versão exata, use `--version 1.0.0`.

**Notas multi-linha:** o `npm run release --` engole quebra de linha. Para notas
com mais de uma linha, chame `node scripts/release.mjs` direto com aspas, ou use
`--notes-file`:

```bash
node scripts/release.mjs --bump minor --notes-file notas.txt
```

Sem `--notes`, as notas viram `Loop <versão>` — funcional, mas é o que o jogador
lê na tela de update. Escreva algo de verdade.

## O que o script faz (6 etapas)

1. **Bump** — calcula a versão nova e sincroniza os três arquivos.
2. **Build assinado** — leva ~6-8 min. É a etapa longa; não interrompa achando
   que travou.
3. **Confere a assinatura** — valida o `.sig` do setup.
4. **Gera o manifesto** — o JSON que o updater consome.
5. **Publica no bucket.**
6. **Verifica no ar** — confirma que o que subiu está acessível.

Se falhar no meio, o estado fica parcial: a versão já foi bumpada nos arquivos
mas o artefato pode não ter subido. Antes de rodar de novo, olhe em que etapa
parou e o que já foi commitado — rodar cego pode bumpar duas vezes.

## Antes de rodar

O release não é a hora de descobrir que um teste quebrou. Rode a verificação
completa antes (veja a skill `verificar`):

```bash
npm run test:all
```

```bash
npm run build && cargo test --manifest-path src-tauri/Cargo.toml
```

Confirme também que a árvore está limpa e no branch certo — o `__APP_BUILD__` sai
da contagem de commits do git, então o que está commitado é o que vai no build.

## O que não é do release

`.cargo/config.toml` tem `target-dir = "C:/cargo-target/iracer"`, específico
desta máquina (aponta o target para fora do OneDrive); o CI sobrescreve com
`CARGO_TARGET_DIR`. Não commite mudança nesse arquivo como parte de um release —
não é configuração geral do projeto.
