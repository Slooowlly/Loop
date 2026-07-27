---
name: nova-string
description: Adiciona ou altera texto de interface no Loop respeitando o i18n obrigatório — t() no JSX, chave em pt-BR e en-US, paridade garantida, mais o lado Rust com rust-i18n e a exigência de #[serial] em teste que troca de idioma. Use SEMPRE que for escrever, mudar ou remover qualquer texto que o jogador lê: "adiciona um botão", "muda esse label", "mensagem de erro nova", "texto do tooltip", "renomear essa aba", ou ao criar componente novo com qualquer palavra visível. Use também quando o hook de pre-commit barrar o commit por string em português fora de t(), quando o npm run i18n:audit acusar cobertura, ou quando localeParity reclamar de chave faltando.
---

# Texto de interface no Loop

O jogo tem dois idiomas com paridade garantida por teste, e um hook de
pre-commit que barra o commit se você escrever português cru num `.jsx`. Isso
não é burocracia: string solta no JSX é invisível para o tradutor e só aparece
como bug quando alguém joga em inglês e vê metade da tela em português.

## Frontend

Stack: i18next, um namespace por área, arquivos em
`src/i18n/locales/<lang>/common.json`. **pt-BR é o locale-base** (é onde o texto
nasce), en-US é o par.

**1. No componente**, nunca escreva o texto direto:

```jsx
const { t } = useTranslation();
// ...
<button>{t("dashboard.acoes.avancarSemana")}</button>
```

A chave descreve *onde* e *o quê*, hierarquicamente. Siga a árvore que já existe
no `common.json` da área em vez de criar um ramo novo — a maior parte das telas
já tem seu galho.

**2. Nos dois arquivos de locale**, na mesma posição da árvore:

```jsonc
// src/i18n/locales/pt-BR/common.json
"avancarSemana": "Avançar semana"

// src/i18n/locales/en-US/common.json
"avancarSemana": "Advance week"
```

Adicionar só no pt-BR faz `localeParity.test.js` falhar — é o teste garantindo
que as duas árvores têm exatamente as mesmas chaves. Traduza de verdade; deixar
o texto em português no en-US passa no teste e entrega um bug.

**3. Interpolação e plural** seguem o i18next padrão:

```jsx
t("mercado.propostas.contagem", { count: propostas.length })
```

```jsonc
"contagem_one": "{{count}} proposta",
"contagem_other": "{{count}} propostas"
```

## Quando o hook de pre-commit barra

O checker (`.githooks/pre-commit`, mesmo código de `src/i18n/i18nCoverage.test.js`)
procura string de UI em português fora de `t()` em `.jsx` no stage. Se ele
apontar uma linha, a resposta certa quase sempre é extrair para `t()`.

Há exceções legítimas — texto que não é UI, um placeholder técnico, uma
constante de debug:

- `{/* i18n-ignore */}` na linha ou na linha acima
- `// i18n-ignore-file` em qualquer ponto do arquivo
- `git commit --no-verify` para pular pontualmente

Use a exceção quando o texto de fato não é lido pelo jogador. Silenciar o
checker para não ter que traduzir só empurra o problema para quem joga em inglês.

## Backend

`rust-i18n` lendo `src-tauri/locales/pt-BR.yml` e `en-US.yml`. Mesma regra: a
chave entra nos dois arquivos.

O detalhe que morde: **o locale é global do processo.** Um teste que troca de
idioma altera o estado que os outros testes veem. Se o seu teste faz isso, marque
com `#[serial]` (crate `serial_test`):

```rust
#[test]
#[serial]
fn narrativa_em_ingles() {
    rust_i18n::set_locale("en-US");
    // ...
}
```

Sem isso, testes que asseveram prosa em português falham de forma intermitente —
e falham em *outro arquivo*, o que faz perder muito tempo até a ficha cair.

## Fechamento

```bash
npm run i18n:audit && npm run test:ui
```

O `i18n:audit` é o mesmo checker do hook, rodado no projeto inteiro em vez de só
no stage. Se mexeu no lado Rust:

```bash
npm run build && cargo test --manifest-path src-tauri/Cargo.toml
```

Vale conferir a tela nos dois idiomas quando o texto for longo: tradução em
inglês frequentemente estoura um botão que estava justo em português.
