# Global Driver Panorama Design

## Objetivo

Criar uma tela escondida de panorama global de pilotos, aberta por duplo clique em um piloto no dashboard de classificacao. A tela deve listar todos os pilotos da carreira, incluindo empregados, livres/desempregados e aposentados, permitindo comparar grandeza historica e metricas especificas como vitorias, titulos, podios, poles, corridas, pontos, DNFs e lesoes.

## Acesso e Navegacao

A tela nao aparece na navegacao principal para nao encher o menu. Ela funciona como uma aba interna escondida do dashboard:

- 1 clique no nome do piloto na classificacao abre a ficha atual do piloto.
- 2 cliques no nome do piloto abre o Panorama Global de Pilotos.
- A tela abre destacando o piloto usado como origem.
- A tela inclui uma acao clara de retorno para a classificacao.

Para diferenciar clique simples e duplo clique, o frontend deve atrasar a abertura da ficha por uma janela curta, em torno de 220ms. Se um duplo clique chegar nesse intervalo, cancela a ficha e abre o panorama.

## Conteudo da Tela

O topo mostra um resumo do piloto destacado:

- nome, status e contexto atual;
- posicao no Indice Historico Balanceado;
- posicoes em vitorias, titulos, podios e lesoes;
- valores principais de carreira.

A area principal mostra uma tabela global com todos os pilotos:

- posicao historica;
- piloto;
- status: ativo, livre/desempregado ou aposentado;
- categoria/equipe atual quando houver;
- Indice Historico Balanceado;
- titulos, vitorias, podios, poles, pontos, corridas, DNFs e lesoes.

Pilotos livres e aposentados aparecem com menor opacidade, mas continuam ordenaveis e legiveis.

## Ranking Historico

O ranking principal deve usar um Indice Historico Balanceado. O objetivo e valorizar categorias mais fortes sem apagar pilotos que dominaram categorias menores.

Multiplicadores iniciais por categoria:

- `mazda_rookie`, `toyota_rookie`: 0.75
- `mazda_amador`, `toyota_amador`: 0.85
- `bmw_m2`: 0.95
- `gt4`: 1.08
- `production_challenger`: 1.12
- `gt3`: 1.22
- `endurance`: 1.25

Formula base por categoria:

```text
(titulos * 140)
+(vitorias * 34)
+(podios * 13)
+(poles * 9)
+(pontos_normalizados * 0.9)
+bonus_leve_por_corridas
-(dnfs * penalidade_leve)
```

Lesoes ficam fora do indice principal. Elas aparecem como ranking e metrica separada, porque contam uma historia importante do piloto, mas nao devem diminuir diretamente a grandeza esportiva.

## Backend

Criar um comando Tauri novo, `get_global_driver_rankings`, com payload proprio. O comando deve:

- ler todos os pilotos atuais via `driver_queries::get_all_drivers`;
- incluir pilotos aposentados registrados na tabela `retired` quando existirem apenas nesse snapshot;
- juntar contrato/equipe atual quando existir;
- usar `driver_season_archive` para reconstruir resultados por categoria e aplicar multiplicadores;
- usar stats de carreira atuais como fallback quando nao houver arquivo suficiente por categoria;
- usar `injury_queries::count_injuries_by_severity_for_pilot` para totais de lesoes;
- calcular indice, rankings globais e rankings por metrica.

## Frontend

Criar um componente focado, provavelmente `src/pages/tabs/GlobalDriversTab.jsx`, renderizado pelo `Dashboard` como aba interna quando o estado ativo for esse modo escondido.

O componente deve buscar `get_global_driver_rankings`, mostrar loading/erro de forma consistente com as abas atuais, destacar o piloto de origem e permitir ordenacao por metricas.

## Testes

Cobrir em TDD:

- comando backend retorna pilotos ativos, livres e aposentados;
- indice pondera categorias de forma equilibrada;
- lesoes nao entram no indice principal, mas aparecem na metrica;
- duplo clique no piloto abre o panorama sem abrir a ficha;
- clique simples continua abrindo a ficha;
- tela destaca o piloto de origem;
- tabela permite ver e ordenar metricas historicas.
