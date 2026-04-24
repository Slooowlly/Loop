# Driver Detail Tabs Design

## Goal
Reorganizar a ficha de piloto em abas internas para priorizar a leitura da temporada atual, escondendo blocos mais longos de carreira, forma e mercado atrás de navegação contextual dentro da própria gaveta.

## Structure

### Aba `Atual`
- Abre por padrão ao clicar em um piloto.
- Mostra o que importa "agora":
  - header com nome, bandeira, licença, equipe, papel, idade e status
  - competitivo: personalidade, qualidades, defeitos e motivação
  - performance da temporada atual
  - resumo curto de momento atual
  - contrato atual resumido

### Aba `Forma`
- Foca só em momento recente.
- Mostra:
  - últimas 5 corridas
  - média de chegada
  - tendência
  - leitura rápida do momento

### Aba `Carreira`
- Esconde o conteúdo histórico que hoje polui a abertura da ficha.
- Mostra:
  - performance acumulada de carreira
  - trajetória
  - títulos e status de campeão
  - marcos

### Aba `Mercado`
- Centraliza o bloco administrativo e os futuros blocos narrativos.
- Mostra:
  - contrato completo
  - mercado, quando existir
  - relacionamentos, reputação e saúde, quando existirem

## Interaction
- A barra de abas fica logo abaixo do header da ficha.
- A aba padrão é `Atual`.
- A troca de aba é local ao componente e não faz nova chamada ao backend.
- As abas devem parecer parte da gaveta glass, usando pills/botões compactos.
- A navegação deve continuar disponível durante o scroll, preferencialmente com barra sticky.

## Data Strategy
- Não há necessidade de mudar o payload principal agora.
- A ficha só reorganiza blocos já existentes:
  - `perfil`, `competitivo`, `performance.temporada`, `forma`, `trajetoria`, `contrato_mercado`
- `Atual` consome dados resumidos.
- `Forma`, `Carreira` e `Mercado` aprofundam o mesmo `detail` já carregado.

## Testing
- Travar que a ficha inicializa em `Atual`.
- Travar que a UI contém as tabs `Atual`, `Forma`, `Carreira`, `Mercado`.
- Travar que o bloco de carreira deixa de estar na abertura padrão da ficha.
- Verificar que o build do frontend continua verde.
