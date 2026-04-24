const OPTION_COPY = {
  a: {
    title: "Opcao A - Halo simples",
    summary: "A versao mais limpa: bolinha grande, anel branco, brilho da categoria e ponto interno.",
    bullets: [
      "Mais parecida com o que ja esta no app.",
      "Boa leitura mesmo em celulas pequenas.",
      "Diferencia sem competir com as bolinhas fantasmas.",
    ],
  },
  b: {
    title: "Opcao B - Alvo premium",
    summary: "A bolinha vira um alvo com aro externo e reflexo diagonal, dando mais presenca de evento principal.",
    bullets: [
      "Bem chamativa para a corrida do jogador.",
      "Tem cara de destaque/selecionado.",
      "Pode ficar forte demais em meses com muitas corridas.",
    ],
  },
  c: {
    title: "Opcao C - Radar",
    summary: "Um ponto menor com dois aneis finos ao redor, como se o calendario estivesse marcando um alvo.",
    bullets: [
      "Leve e tecnico.",
      "Funciona bem para comunicar 'sua corrida'.",
      "Menos pesado visualmente que a opcao B.",
    ],
  },
  d: {
    title: "Opcao D - Pilula de evento",
    summary: "Troca a bolinha pura por uma capsula curta, ainda usando a cor da categoria.",
    bullets: [
      "Diferencia muito bem das bolinhas fantasmas.",
      "Parece um marcador de evento oficial.",
      "Sai um pouco da ideia de 'bolinha'.",
    ],
  },
  e: {
    title: "Opcao E - Selo tecnico",
    summary: "Bolinha com aro recortado, lembrando um lacre ou sensor de corrida.",
    bullets: [
      "Mais desenhada e menos generica.",
      "Boa identidade sem ocupar tanto espaco.",
      "Minha preferida se quiser algo mais premium.",
    ],
  },
};

const WEEKDAYS = ["D", "S", "T", "Q", "Q", "S", "S"];
const DAYS = [
  null, null, null, null, null, null, 1,
  2, 3, 4, 5, 6, 7, 8,
  9, 10, 11, 12, 13, 14, 15,
  16, 17, 18, 19, 20, 21, 22,
  23, 24, 25, 26, 27, 28, 29,
  30, 31, null, null, null, null, null,
];

function createCalendar(option) {
  const grid = document.querySelector("[data-calendar-grid]");
  const weekdayHost = document.querySelector("[data-weekdays]");

  WEEKDAYS.forEach((day) => {
    const item = document.createElement("div");
    item.textContent = day;
    weekdayHost.appendChild(item);
  });

  DAYS.forEach((day) => {
    const cell = document.createElement("div");
    cell.className = day ? "day" : "day empty";

    if (day === 12) {
      cell.className += " has-track";
      cell.innerHTML = `
        <span class="today">HOJE</span>
        <span class="date">${day}</span>
        <span class="player-dot" aria-label="Corrida do jogador"></span>
        <span class="ghost-dots" aria-label="Outras categorias no dia">
          <span class="ghost" style="background: var(--red)"></span>
          <span class="ghost" style="background: var(--purple)"></span>
        </span>
      `;
    } else if (day === 17) {
      cell.className += " has-track alt-track";
      cell.innerHTML = `
        <span class="date">${day}</span>
        <span class="ghost-dots" aria-label="Outras categorias no dia">
          <span class="ghost" style="background: var(--blue)"></span>
          <span class="ghost" style="background: var(--cyan)"></span>
          <span class="ghost" style="background: var(--green)"></span>
        </span>
      `;
    } else if (day) {
      cell.innerHTML = `<span class="date">${day}</span>`;
    }

    grid.appendChild(cell);
  });

  const copy = OPTION_COPY[option];
  document.querySelector("[data-option-title]").textContent = copy.title;
  document.querySelector("[data-option-summary]").textContent = copy.summary;

  const list = document.querySelector("[data-option-bullets]");
  copy.bullets.forEach((bullet) => {
    const item = document.createElement("li");
    item.textContent = bullet;
    list.appendChild(item);
  });
}

function boot() {
  const option = document.body.dataset.option;
  createCalendar(option);
}

boot();
