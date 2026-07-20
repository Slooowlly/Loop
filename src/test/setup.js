import "@testing-library/jest-dom/vitest";
// Inicializa o i18n pra TODOS os testes (espelha o main.jsx do app). Sem isto, um
// teste que mocka o store — antes o caminho que inicializava o i18n — veria o
// useTranslation devolver a chave crua em vez do texto (default pt-BR).
import "../i18n/index.js";

