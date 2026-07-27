# Endpoint `/log` — envio de log de diagnóstico pelo jogador

Quando o app quebra só na máquina de outra pessoa, o que existia era print de tela.
Print não mostra código de erro do Windows, e o app é uma GUI sem console — todo
`eprintln!` do backend morre no vazio. O caso que motivou esta rota: um beta tester
com o SDK do iRacing sem puxar nada, e três rodadas de conversa sem conseguir
distinguir "simulador fechado" de "Windows negou acesso à memória".

O cliente já grava um log rotativo em `%APPDATA%\com.loop.app\logs\loop.log`
(`src-tauri/src/diagnostico.rs`) e tem um botão em Configurações › Corrida ›
Diagnóstico do iRacing que envia o final dele. Este documento é o **contrato do lado
do servidor** (Cloud Run `iracer-news`, mesmo projeto e mesmo segredo de
`/season-preview`, `/telemetry`, `/world-notes`).

Enquanto a rota não existir, nada quebra: o botão mostra "O servidor recusou o envio
(HTTP 404 Not Found)", registra a recusa no próprio log, e os botões de **copiar log**
e **abrir pasta do log** seguem funcionando — o tester ainda consegue mandar o arquivo
pelo canal de sempre.

## Contrato HTTP

**Request** `POST /log`

```json
{
  "ticket": "A1B2C3D4",
  "install_id": "<uuid do install>",
  "app_version": "0.13.2",
  "os": "windows",
  "nota": "abri o sim e a telemetria ficou toda zerada",
  "diagnostico": {
    "veredito": "acesso_negado",
    "memoria_ok": false,
    "memoria_nome": null,
    "memoria_erro": 5,
    "janela_encontrada": true,
    "janela_simulador": true,
    "status": null,
    "num_vars": null,
    "session_info_len": null,
    "elevado": false,
    "ticks_observados": 0,
    "log_caminho": "%USERPROFILE%\\AppData\\Roaming\\com.loop.app\\logs\\loop.log"
  },
  "log": "2026-07-27 12:00:28.034 [boot] Loop 0.13.2 (windows) iniciado\n..."
}
```

Header: `x-app-secret: <APP_SECRET>` (mesmo dos outros endpoints).

- `ticket` — 8 caracteres hexadecimais maiúsculos, **gerado no cliente**. É o código que
  o jogador lê na tela e informa no relato; use-o como id do documento.
- `nota` — texto livre do jogador, até 300 caracteres, pode vir vazio. É o campo mais
  valioso do corpo: descreve o que o log não conta.
- `diagnostico` — retrato cruzado da conexão no instante do envio. O `veredito` é um
  enum fechado: `ok`, `sim_fechado`, `so_launcher`, `sessao_nao_pronta`, `acesso_negado`,
  `sim_sem_sdk`, `nao_suportado`. Os demais campos são os sinais crus que produziram
  esse veredito — guarde todos, é deles que sai qualquer análise agregada depois.
- `log` — final do arquivo, até 60 KB. O nome de usuário do Windows já vem substituído
  por `%USERPROFILE%` (feito no cliente, em `diagnostico::redigir`).

Corpo total fica abaixo de ~64 KB por construção.

**Response 200** — o cliente **ignora o corpo**; qualquer 2xx conta como sucesso e ele
mostra o ticket que já tinha gerado. Devolver `{"ticket": "..."}` é útil só para
depuração pelo navegador.

**Erros** seguem o padrão dos outros endpoints: `401` segredo inválido, `413` corpo
acima do teto, `429` cooldown/teto diário, `5xx` erro interno. Todos viram a mesma
mensagem na tela do jogador ("O servidor recusou o envio (HTTP ...)") e uma linha no
log local com o status — então uma recusa não some, nem para ele nem para você.

## Como o servidor guarda

| Coleção | Doc | Serve para |
|---|---|---|
| `logs` | `{ticket}` | um envio |
| `log_daily` | `YYYY-MM-DD` (UTC) | contador do dia (teto de abuso) |

**A chave é o `ticket`, não o `install_id`.** O jogador te diz "mandei, código A1B2C3D4"
— buscar por esse código tem de ser uma leitura direta, sem query. O `install_id` entra
como campo indexado, para cruzar com as corridas em `races`/`installs` da telemetria e
ver o que mais aquela máquina estava fazendo.

Campos sugeridos além do corpo recebido:

- `received_at` — timestamp do servidor (não confie no relógio do cliente).
- `veredito` — copiado de `diagnostico.veredito` para o topo do documento, para dar
  para filtrar "todos os `acesso_negado` da semana" sem abrir cada log.
- `expire_at` — `received_at + LOG_RETENTION_DAYS`.

Uma decisão que vale copiar da telemetria: **envio repetido do mesmo ticket é upsert.**
O jogador pode apertar Enviar duas vezes achando que não foi; isso não deve virar dois
documentos.

## Leitura

Protegidas pelo mesmo segredo (header `x-app-secret` ou `?secret=`, por comodidade no
navegador, igual às rotas de leitura da telemetria).

- `GET /log/{ticket}` — o envio inteiro, para colar no editor e ler.
- `GET /log?days=7` — lista os envios da janela (ticket, `received_at`, `veredito`,
  `app_version`, primeiros 120 caracteres da `nota`), mais recentes primeiro. É a fila de
  triagem: dá para ver de relance se três testers diferentes mandaram `acesso_negado` na
  mesma versão.

## Operação

- `LOG_DAILY_CAP` — freio caso o `APP_SECRET` vaze. O segredo está dentro do binário
  distribuído, então trate a rota como pública-com-atrito, não como autenticada. Um teto
  diário de algumas centenas já protege: envio de log é ato humano, não automático.
- `LOG_MAX_BYTES` (65536) — recusa com `413` acima disso. O cliente já corta em 60 KB;
  o teto do servidor é contra corpo forjado.
- `LOG_RETENTION_DAYS` (90) — preenche `expire_at`. **Quem apaga de fato é a política de
  TTL do Firestore**, que precisa ser ligada à mão no Console (Firestore → TTL, campo
  `expire_at`, coleção `logs`). Sem isso, nada é apagado — mesma pegadinha documentada
  em [telemetry-endpoint.md](telemetry-endpoint.md).

## Privacidade

O envio **nunca é automático** e não passa pelo consentimento de telemetria: é um ato
explícito do jogador, com um texto na tela dizendo o que sai. Deliberado — quem
desligou a telemetria anônima é justamente quem mais precisa conseguir pedir ajuda.

O que sai da máquina: o final do log, versão do app, SO, `install_id`, o diagnóstico e a
nota. O nome de usuário do Windows é removido dos caminhos antes do envio. O que
**permanece** no corpo é a estrutura dos caminhos e o histórico daquela sessão do app —
tratar como dado pessoal, mesmo redigido.

## O que falta

A rota em si. O cliente está pronto e verificado ponta a ponta contra o servidor atual:
a requisição chega ao Cloud Run e volta `404` (host, segredo e serialização corretos),
sem congelar a janela e sem derrubar os outros botões do painel.
