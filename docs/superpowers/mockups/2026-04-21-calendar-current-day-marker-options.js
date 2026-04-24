const shells = document.querySelectorAll("[data-days]");
const raceDays = new Set([5, 12, 26]);
const doneDays = new Set([2, 3, 9]);
const convDays = new Set([17, 18]);
const otherCategoryDays = new Set([7, 12, 20, 25]);
const currentDay = 21;

function createDay(day) {
  const cell = document.createElement("div");
  const classes = ["day"];

  if (day === 0) {
    classes.push("empty");
    cell.className = classes.join(" ");
    return cell;
  }

  if (raceDays.has(day)) classes.push("race");
  if (doneDays.has(day)) classes.push("done");
  if (convDays.has(day)) classes.push("conv");
  if (day === currentDay) classes.push("is-current");

  cell.className = classes.join(" ");
  cell.innerHTML = `<span>${day}</span>`;

  if (day === currentDay) {
    const label = document.createElement("span");
    label.className = "today-label";
    label.textContent = "Hoje";
    cell.appendChild(label);
  }

  if (raceDays.has(day)) {
    const dot = document.createElement("i");
    dot.className = "race-dot";
    cell.appendChild(dot);
  }

  if (otherCategoryDays.has(day)) {
    const dots = document.createElement("span");
    dots.className = "other-dots";
    dots.innerHTML = "<i></i><i></i>";
    cell.appendChild(dots);
  }

  return cell;
}

shells.forEach((shell) => {
  for (let i = 0; i < 3; i += 1) {
    shell.appendChild(createDay(0));
  }
  for (let day = 1; day <= 30; day += 1) {
    shell.appendChild(createDay(day));
  }
  for (let i = 0; i < 2; i += 1) {
    shell.appendChild(createDay(0));
  }
});
