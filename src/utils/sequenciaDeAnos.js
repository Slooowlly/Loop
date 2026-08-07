// Anos seguidos viram um intervalo: "2017 ~ 2025" no lugar de nove rótulos.
//
// Nasceu no card de títulos da ficha do piloto, onde uma dinastia de nove anos
// custava duas linhas e obrigava a somar de cabeça para descobrir que era uma
// dinastia. Vale para qualquer lista de anos de conquista — a galeria de
// campeões de um campeonato tem o mesmo problema com o mesmo piloto.
//
// A entrada é DESCENDENTE (o mais recente primeiro) e a saída mantém essa
// ordem; dentro do intervalo a contagem é crescente, porque uma dinastia se lê
// do começo.

// A partir de quantos anos seguidos o intervalo compensa. Dois não compensa:
// "2024 ~ 2025" é mais largo que "2024 2025" e ainda esconde que são dois.
const MINIMO_PARA_INTERVALO = 3;

// `serie` é o que precisa se repetir para a sequência CONTINUAR. Na galeria de
// campeões é a equipe: 2024 e 2025 seguidos, mas em casas diferentes, são duas
// conquistas distintas e cada uma carrega a própria logo — juntá-las num
// intervalo só apagaria a troca.
export function comprimeSequenciasDeAnos(itens, { ano = Number, serie = () => "" } = {}) {
  const blocos = [];
  let corrente = [];

  const fecha = () => {
    if (!corrente.length) return;
    const novo = ano(corrente[0]);
    const velho = ano(corrente[corrente.length - 1]);
    if (corrente.length >= MINIMO_PARA_INTERVALO) {
      blocos.push({ key: `${velho}-${novo}`, label: `${velho} ~ ${novo}`, itens: corrente });
      corrente = [];
      return;
    }
    for (const item of corrente) blocos.push({ key: `${ano(item)}`, label: `${ano(item)}`, itens: [item] });
    corrente = [];
  };

  for (const item of Array.isArray(itens) ? itens : []) {
    const atual = ano(item);
    if (!Number.isFinite(atual)) continue;
    const anterior = corrente.length ? corrente[corrente.length - 1] : null;
    // O ano repetido (dois títulos no mesmo ano, em categorias diferentes) não
    // continua a sequência: ele fecha a atual e abre a própria, senão dois
    // troféus virariam um rótulo só.
    if (anterior && atual === ano(anterior) - 1 && serie(item) === serie(anterior)) {
      corrente.push(item);
      continue;
    }
    fecha();
    corrente = [item];
  }
  fecha();

  return blocos;
}

// A mesma compressão para quando os anos são texto corrido, sem chip nem logo.
export function formatSequenciaDeAnos(anos, separador = ", ") {
  return comprimeSequenciasDeAnos(anos)
    .map((bloco) => bloco.label)
    .join(separador);
}
