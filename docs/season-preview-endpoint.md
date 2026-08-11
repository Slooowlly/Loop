# Endpoint `/season-preview` — matéria de expectativas de pré-temporada (IA)

Antes da **primeira corrida** da temporada, a aba Notícias não tem edição de corrida
para mostrar. Em vez do "livro fechado", a revista abre com **uma matéria de
expectativas** sobre o campeonato do ano — a tese da temporada, os favoritos por
percepção pública, as relações que o grid carrega e a etapa de abertura.

O app já monta os fatos e chama o servidor (`commands/season_preview/`, comando
`enrich_season_preview_ai`). Este documento é o **contrato do lado do servidor**
(Cloud Run `iracer-news`, mesmo projeto de `/pre-race`, `/post-race`, `/race-story`,
`/world-notes`). O design completo da matéria está em
[season-preview-design.md](season-preview-design.md) — em caso de divergência, o design
manda no conteúdo e este arquivo manda no transporte.

Enquanto o endpoint não responder, nada quebra: o app cai no **montador determinístico**
(`season_preview/fallback.rs`), que devolve a mesma matéria em versão curta.

## Contrato HTTP

**Request** `POST /season-preview`

```json
{
  "facts": "TEMPORADA: ...\n\nFAVORITOS (ordem = percepção pública, NÃO ritmo real):\n- ...",
  "lang": "pt-BR",
  "install_id": "<uuid do install>",
  "target_words": { "min": 450, "max": 600 }
}
```

Header: `x-app-secret: <APP_SECRET>` (mesmo dos outros endpoints).

- `facts` — bundle de blocos nomeados, **já no idioma pedido e já qualitativo**: não vem
  um número sequer, nem nível de carro, nem salário, nem potencial. Formato canônico e
  significado de cada bloco no §6 do design.
- `target_words` — intervalo de palavras pedido para o corpo. **É a autoridade sobre o
  comprimento**; o valor atual é 450–600. Já foi 700–900, mas no playtest a matéria
  ficou longa demais para ser lida; o intervalo atual ainda cobre os dez dossiês do
  bundle com tratamento próprio por nome.

Blocos que chegam em `facts`, nesta ordem: `TEMPORADA`, `ABERTURA`, `TÍTULO`, `MATERIAL`,
`TESE SUGERIDA`, `FAVORITOS` (5 linhas), `PROMESSAS / INCÓGNITAS` (5 linhas), `RELAÇÕES`
(até 4), `INTUIÇÃO` (opcional) e `GRID`. Cada linha de piloto é
`nome | equipe | percepção | currículo | experiência | [traço] | [gancho]`.

**Response 200**

```json
{
  "headline": "Manchete de revista",
  "standfirst": "Linha-fina, uma frase",
  "body": "Parágrafo 1...\n\nParágrafo 2..."
}
```

`body` é o que não pode faltar (vazio → o cliente trata como erro e cai no fallback);
manchete e linha-fina vazias o front tolera. Parágrafos separados por linha em branco —
o front faz `split(/\n\s*\n/)` e colore os nomes de piloto e equipe.

Erros seguem o padrão dos outros endpoints: `401` segredo inválido, `429` cooldown/teto,
`5xx` erro do modelo. Qualquer um deles derruba no fallback determinístico, e o cliente
**não** cacheia o texto — a próxima abertura da revista tenta de novo.

## Brief da persona (prompt do servidor)

As regras duras estão no [§7 do design](season-preview-design.md) e não devem ser
reescritas aqui. O resumo do que mais custou caro quando faltou:

- **3ª pessoa sempre.** Nunca se dirigir ao leitor nem a um piloto em 2ª pessoa. O piloto
  do jogador é só mais uma linha do bundle, sem tratamento especial.
- **Nenhum número.** Nem níveis, nem notas, nem cifras, nem idades como estatística.
- **A ordem de `FAVORITOS` é percepção pública, não ritmo.** Sobre quem é de fato mais
  rápido, hedgear sempre. O bloco `INTUIÇÃO` é sussurro de bastidor: no máximo uma menção
  hedgeada, nunca manchete.
- **Cobrir os dez nomes**: tratamento próprio para cada favorito, ao menos uma frase para
  cada promessa, **variando a construção da frase** — a mesma fôrma repetida por piloto é
  o defeito que a versão determinística já tinha e que a de IA não pode repetir.
- Fechar pela **etapa de abertura**.

Deploy (preservando env, conforme os outros):
`gcloud run deploy iracer-news --source . --region southamerica-east1 --allow-unauthenticated`
(rodar SÓ quando o usuário pedir).
