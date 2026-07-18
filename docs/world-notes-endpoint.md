# Endpoint `/world-notes` — rodapé "Do mundo do Grid" (IA, futuro)

O rodapé de notícias do mundo (aba Notícias / revista) já funciona **hoje** com texto
determinístico gerado no app (`src-tauri/src/commands/world_footer.rs`, comando
`get_world_footer`). Este documento descreve o endpoint a criar no servidor Cloud Run
(`iracer-news`, mesmo projeto de `/pre-race`, `/post-race`, `/race-story`) para que as
notinhas passem a ser **reescritas por IA** com voz de revista de fofoca do paddock.

Enquanto o endpoint não existir, nada quebra — o app usa o texto determinístico.

## Como plugar no app (quando o endpoint existir)

1. Adicionar em `src-tauri/src/narrative/client.rs` uma `fetch_world_notes(facts, lang,
   install_id) -> Result<Vec<String>, StoryError>` batendo em
   `.../world-notes` (mesmo header `x-app-secret`, mesmo padrão de erro dos demais).
2. Em `commands/world_footer.rs`, após montar as notas determinísticas, montar a string
   `facts` (já é devolvida no `WorldFooterResult.facts`) e chamar o servidor. Se voltar
   OK e com o mesmo número de itens, substituir `note.text` pelas versões da IA e marcar
   `source = "ai"`; em QUALQUER erro, manter o texto determinístico (fallback).
3. Cachear por temporada+rodada (nova tabela `ai_world_notes`, molde de `ai_pre_race`)
   para não regenerar a cada abertura da aba.

## Contrato HTTP

**Request** `POST /world-notes`
```json
{
  "facts": "linha por notinha, no formato [kind] Assunto — texto determinístico\n...",
  "lang": "pt-BR",
  "install_id": "<uuid do install>"
}
```
Header: `x-app-secret: <APP_SECRET>` (mesmo dos outros endpoints).

O bloco `facts` é uma lista de fatos crus, um por linha. Cada linha:
`[<kind>] <assunto> — <texto base>`, onde `<kind>` ∈
`crise_financeira | clima_pesado | nova_diretoria | piloto_time_crise | recorde_quebrado | recorde_a_caminho`.

**Response 200**
```json
{ "notes": ["nota reescrita 1", "nota reescrita 2", "..."] }
```
Devolver **exatamente uma string por fato recebido, na mesma ordem** (o app casa por
índice). Erros seguem o padrão dos outros endpoints: `401` segredo inválido, `429`
cooldown/teto, `5xx` erro do modelo.

## Brief da persona (prompt do servidor)

- Voz: colunista de revista de automobilismo — **3ª pessoa, jornalística, PUBLICADA**.
  Curto, insinuante ("rumores dão conta de que…"), nunca afirma o que não está no fato.
  1 frase por nota, no idioma pedido.
- **NUNCA se dirigir ao leitor/jogador em 2ª pessoa** ("você", "seu ex-time"). É uma
  matéria de revista, não uma análise pessoal. Os assuntos foram SELECIONADOS por terem
  laço com o jogador ou com os líderes do campeonato, mas esse vínculo NÃO entra no texto.
- **NÃO inventar** números, patrocinadores, valores de dívida, resultados ou recordes: só
  reformular o fato recebido com cor. O `kind` diz o tom:
  - `crise_financeira` → aperto/dívida da equipe, tom de rumor de bastidor.
  - `clima_pesado` → vestiário/moral, tensão interna da equipe.
  - `nova_diretoria` → reviravolta/recomeço da equipe após fase ruim.
  - `piloto_time_crise` → um piloto cujo time atual passa por dificuldade.
  - `recorde_quebrado` → um piloto/equipe acabou de bater um recorde histórico da
    categoria; tom de "entrou para a história". Cobre ~24 tipos (vitórias, pódios, poles,
    títulos, vitórias numa temporada, dono da pista, sequência, volta mais rápida, maior
    recuperação, títulos de construtores, maior vencedora, dobradinhas, mais jovem/velho a
    vencer, campeão mais jovem, corrida mais caótica, jejum quebrado, campeonato mais
    apertado/dominante, dupla mais longeva, e "de azar"). O texto-base já vem redigido e
    correto — preservar EXATOS os números/tempos/idades e os nomes.
  - `recorde_a_caminho` → um piloto perto de igualar/quebrar um recorde histórico da
    categoria; manter o número e o nome do recordista EXATOS como vieram no fato.

Deploy (preservando env, conforme os outros):
`gcloud run deploy iracer-news --source . --region southamerica-east1 --allow-unauthenticated`
(rodar SÓ quando o usuário pedir).
