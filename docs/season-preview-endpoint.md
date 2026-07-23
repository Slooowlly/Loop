# Endpoint `/season-preview` — matéria de expectativas de pré-temporada (IA)

Antes da **primeira corrida** da temporada, a aba Notícias não tem edição de corrida
para mostrar. Em vez do "livro fechado", a revista abre com **uma matéria de
expectativas** sobre o campeonato do ano — favoritos ao título, o cenário do jogador, a
pista de abertura e o campeão em título.

O app já monta os fatos e chama o servidor (`src-tauri/src/commands/season_preview.rs`,
comando `enrich_season_preview_ai`), espelhando o boletim de corrida (`/race-story`).
Falta **criar o endpoint `/season-preview`** no servidor Cloud Run (`iracer-news`, mesmo
projeto de `/pre-race`, `/post-race`, `/race-story`, `/world-notes`).

Enquanto o endpoint não existir, nada quebra — o front cai no texto-placeholder
(`newsMagazine.preseason.placeholder`) e mostra os favoritos por potencial ao lado.

## Já plugado no app

- `src-tauri/src/narrative/client.rs`: `fetch_season_preview(facts, lang, install_id)
  -> Result<String, StoryError>` batendo em `.../season-preview` (mesmo header
  `x-app-secret`, mesmo padrão de erro dos demais).
- `commands/season_preview.rs`: monta `facts` determinísticos (i18n `season_preview.*`),
  chama o servidor e **cacheia** o texto por temporada+categoria na tabela `ai_race_story`
  (mesmo key-value do boletim, `news_id` sintético `season-preview:{season_id}:{category}`).
  Em QUALQUER erro devolve `story: None` (fallback no front).
- Front: `src/pages/tabs/NewsMagazineTab.jsx` renderiza o spread aberto de pré-temporada
  quando `editions.length === 0`; ao concluir a 1ª corrida o bloco some sozinho.

## Contrato HTTP

**Request** `POST /season-preview`
```json
{
  "facts": "PRÉ-TEMPORADA — ...\nFAVORITOS (por potencial):\n- ...\nSEU CENÁRIO: ...",
  "lang": "pt-BR",
  "install_id": "<uuid do install>"
}
```
Header: `x-app-secret: <APP_SECRET>` (mesmo dos outros endpoints).

O bloco `facts` é texto já rotulado, uma seção por linha (gerado no idioma pedido):

- `PRÉ-TEMPORADA — <categoria>, Temporada <n> (<ano>).`
- `Abertura em <pista>, primeira de <n> etapas no calendário.`
- `Campeão em título: <nome> (<equipe>).` — pode faltar (temporada inaugural).
- `Carros: todos no mesmo nível (<L>/10) ...` **ou** `Carros: material desigual — <equipe> tem o carro mais forte (<L>/10), <equipe> o mais fraco (<L>/10).` — nível 1..=10 por equipe. No rookie é sempre uniforme.
- `FAVORITOS (por potencial):` seguido de linhas `- <nome> (<equipe>) — potencial <0-100>; <bagagem>[ · nome de público]`, onde `<bagagem>` é `estreante, ainda sem corridas` **ou** `<N> corridas na carreira, <feito>` (título > vitória > pódio > ainda sem pódio). **O histórico pesa mais que o potencial** — dois pilotos de mesmo potencial contam histórias diferentes.
- `Maior salário do grid: <nome> (<equipe>), $ <valor>/ano.` — só quando há um topo real (salário > 0; some no rookie de salário uniforme).
- `SEU CENÁRIO: você (<nome>) larga a temporada pela <equipe>, <k>º em potencial num grid de <N>.`
- `Sua bagagem: <bagagem>.` — mesma régua dos rivais (estreante x veterano com feitos).
- `Rivalidade em aberto: <nome>, ...` — só se houver nêmesis carregada.

**Response 200**
```json
{ "story": "Parágrafo 1...\n\nParágrafo 2..." }
```
Mesmo formato do `/race-story`: **um único texto**, parágrafos separados por linha em
branco (o app faz `split(/\n\s*\n/)` e colore nomes de piloto/equipe). Erros seguem o
padrão dos outros endpoints: `401` segredo inválido, `429` cooldown/teto, `5xx` erro do
modelo.

## Brief da persona (prompt do servidor)

- Voz: **prévia de revista de automobilismo, 3ª pessoa, publicada**, com o gancho de
  "abertura da temporada". Tom de expectativa e antecipação, não de retrospecto.
- Pode **dirigir-se ao piloto do jogador em 2ª pessoa** no trecho do "SEU CENÁRIO"
  (a matéria é o veículo apresentando o ano ao leitor-piloto), mas o corpo sobre o grid
  fica em 3ª pessoa jornalística. Mantenha 2–3 parágrafos curtos.
- **NÃO inventar** números, patrocinadores, resultados, títulos ou recordes que não
  estejam nos fatos. Trabalhe só com o que veio: favoritos, potencial, **bagagem de
  carreira (corridas/vitórias/pódios/estreante)**, **condição dos carros**, **maior
  salário**, equipes, pista de abertura, campeão em título, nêmesis.
- **Priorize o histórico sobre o potencial.** "Potencial" é só expectativa; o que dá peso
  a um favorito é a bagagem (um veterano com vitórias ≠ um estreante de mesmo potencial).
  Um grid de estreantes é uma história ("temporada de novatos"); um campeão em título que
  ficou é outra. Use a **condição dos carros** para calibrar o mérito: se todos os carros
  são iguais (rookie), a briga é 100% de piloto; se há material desigual, o carro forte/
  fraco entra na conversa. O **maior salário** carrega expectativa embutida (o mercado
  apostou nele) — bom gancho de pressão.
- Fechar com o gancho da **etapa de abertura** (a pista citada) como o primeiro teste real.

Deploy (preservando env, conforme os outros):
`gcloud run deploy iracer-news --source . --region southamerica-east1 --allow-unauthenticated`
(rodar SÓ quando o usuário pedir).
