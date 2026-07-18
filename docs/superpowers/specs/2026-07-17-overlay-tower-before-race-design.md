# Torre do overlay antes da corrida

## Objetivo

Exibir a torre do overlay de monitor e VR desde treino e classificação, inclusive quando o iRacing ainda informa `class_position` como zero.

## Comportamento

- O roster YAML é a fonte da grade, portanto todos os carros resolvidos entram mesmo quando ainda estão na garagem e não aparecem em `tele.cars`; apenas pace car e carros sem identidade/piloto continuam excluídos.
- Carros com posição oficial aparecem primeiro, em ordem crescente de posição.
- Carros sem posição oficial aparecem depois. Fora do treino, eles são ordenados pelo melhor tempo válido do snapshot ao vivo `race_monitor::get_qualy_laps()`, com número do carro como desempate e fallback.
- No treino, `qualy_laps` é ignorado para evitar reutilizar dados de outro fim de semana; o número do carro define a ordem dos não classificados.
- Para posição inválida, `pos` é serializado como zero, `delta` e `gain` permanecem zero e o canvas desenha `–`.

## Limites

Não muda a resolução roster→carreira, o filtro de pace car, a janela top-3/vizinhança, o cálculo durante a corrida nem a regra que oculta a torre quando nenhum carro elegível existe.

As voltas ao vivo ficam separadas de `RaceHistory`: o histórico durável continua recebendo a cópia da quali apenas no início da corrida. O `SubSessionID` identifica o evento e invalida imediatamente dados antigos, mesmo quando o iRacing reutiliza o mesmo `SessionNum`.

Grid/delta, pneus/paradas e voltas de corrida só usam `RaceHistory` quando o `SubSessionID` do histórico coincide com o evento atual. A melhor volta visual usa a telemetria presente e, para carros fora do mundo, o registro da sessão atual; quali e corrida nunca se misturam nesse cálculo.

## Verificação

Testes unitários cobrem identidade/reset do evento, join roster↔telemetria, isolamento do histórico, melhor volta, chave de ordenação e texto de posição. A verificação final inclui testes focados, suítes gerais e builds Rust/Vite.
