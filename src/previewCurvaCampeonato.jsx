// i18n-ignore-file
//
// Arquivo de PREVIEW temporário — só para olhar a curva de campeonato desenhada
// com dados de fixture, sem precisar subir o Tauri e carregar um save.
// Sobe em http://localhost:1420/preview-curva-campeonato.html com `npm run dev`.
// Não entra no build (o Vite só empacota o index.html) e pode ser apagado junto
// com `preview-curva-campeonato.html` na raiz.
import React from "react";
import ReactDOM from "react-dom/client";

import "./index.css";
import "./i18n/index.js";
import { CurvaDeCampeonato } from "./components/driver/v2/CurvaDeCampeonato.jsx";

// A carreira do Facundo Suarez, que é o caso duro: 16 temporadas, zero vitórias,
// zero pódios, e a "subida" de 2016 que era só o grid encolhendo.
const carreiraLonga = [
  { season_number: 1, ano: 2010, categoria: "gt4", equipe_nome: "Northstar", equipe_cor: "#3fb950", posicao: 16, esperado: 14, grid: 24, vitorias: 0, podios: 0, corridas: 12, titulo: false, atual: false },
  { season_number: 2, ano: 2011, categoria: "gt3", equipe_nome: "Sunday Speed Club", equipe_cor: "#d29922", posicao: 22, esperado: 19, grid: 24, vitorias: 0, podios: 0, corridas: 12, titulo: false, atual: false },
  { season_number: 3, ano: 2012, categoria: "gt4", equipe_nome: "Sunday Speed Club", equipe_cor: "#d29922", posicao: 18, esperado: 20, grid: 24, vitorias: 0, podios: 0, corridas: 12, titulo: false, atual: false },
  { season_number: 4, ano: 2013, categoria: "gt3", equipe_nome: "Arclight", equipe_cor: "#dc0000", posicao: 22, esperado: 18, grid: 24, vitorias: 0, podios: 0, corridas: 14, titulo: false, atual: false },
  { season_number: 5, ano: 2014, categoria: "gt3", equipe_nome: "Arclight", equipe_cor: "#dc0000", posicao: 21, esperado: 17, grid: 24, vitorias: 0, podios: 0, corridas: 14, titulo: false, atual: false },
  { season_number: 6, ano: 2015, categoria: "gt3", equipe_nome: "Arclight", equipe_cor: "#dc0000", posicao: 19, esperado: 20, grid: 24, vitorias: 0, podios: 0, corridas: 14, titulo: false, atual: false },
  { season_number: 7, ano: 2016, categoria: "gt4_endurance", equipe_nome: "Kitsune Motorsport", equipe_cor: "#8020d0", posicao: 11, esperado: 9, grid: 12, vitorias: 0, podios: 1, corridas: 10, titulo: false, atual: false },
  { season_number: 8, ano: 2017, categoria: "gt4_endurance", equipe_nome: "Kitsune Motorsport", equipe_cor: "#8020d0", posicao: 10, esperado: 9, grid: 12, vitorias: 0, podios: 0, corridas: 10, titulo: false, atual: false },
  { season_number: 9, ano: 2018, categoria: "gt4_endurance", equipe_nome: "Kitsune Motorsport", equipe_cor: "#8020d0", posicao: 10, esperado: 11, grid: 12, vitorias: 0, podios: 1, corridas: 10, titulo: false, atual: false },
  { season_number: 10, ano: 2019, categoria: "gt4_endurance", equipe_nome: "Kitsune Motorsport", equipe_cor: "#8020d0", posicao: 10, esperado: 8, grid: 12, vitorias: 0, podios: 0, corridas: 10, titulo: false, atual: false },
  { season_number: 11, ano: 2020, categoria: "gt4_endurance", equipe_nome: "Kitsune Motorsport", equipe_cor: "#8020d0", posicao: 9, esperado: 10, grid: 12, vitorias: 0, podios: 2, corridas: 10, titulo: false, atual: false },
  { season_number: 12, ano: 2021, categoria: "gt4_endurance", equipe_nome: "Kitsune Motorsport", equipe_cor: "#8020d0", posicao: 9, esperado: 7, grid: 12, vitorias: 0, podios: 0, corridas: 10, titulo: false, atual: false },
  { season_number: 13, ano: 2022, categoria: "gt4_endurance", equipe_nome: "Kitsune Motorsport", equipe_cor: "#8020d0", posicao: 11, esperado: 9, grid: 12, vitorias: 0, podios: 0, corridas: 10, titulo: false, atual: false },
  { season_number: 14, ano: 2023, categoria: "gt3", equipe_nome: "Solaris", equipe_cor: "#db6d28", posicao: 18, esperado: 15, grid: 24, vitorias: 0, podios: 0, corridas: 16, titulo: false, atual: false },
  { season_number: 15, ano: 2024, categoria: "gt3", equipe_nome: "Solaris", equipe_cor: "#db6d28", posicao: 14, esperado: 16, grid: 24, vitorias: 0, podios: 0, corridas: 16, titulo: false, atual: false },
  { season_number: 16, ano: 2025, categoria: "gt3", equipe_nome: "Solaris", equipe_cor: "#db6d28", posicao: 17, esperado: 15, grid: 24, vitorias: 0, podios: 0, corridas: 16, titulo: false, atual: false },
  { season_number: 17, ano: 2026, categoria: "gt3", equipe_nome: "Solaris", equipe_cor: "#db6d28", posicao: 3, esperado: 12, grid: 24, vitorias: 0, podios: 2, corridas: 5, titulo: false, atual: true },
];

// O caso oposto: campeão duas vezes, com um ano fora do grid no meio e um ano
// sem arquivo de construtores (a laranja se parte).
const carreiraVencedora = [
  { season_number: 1, ano: 2018, categoria: "toyota_rookie", monomarca: true, equipe_nome: "Northstar", equipe_cor: "#3fb950", posicao: 7, esperado: 12, grid: 22, vitorias: 1, podios: 4, corridas: 12, titulo: false, atual: false },
  { season_number: 2, ano: 2019, categoria: "toyota_rookie", monomarca: true, equipe_nome: "Northstar", equipe_cor: "#3fb950", posicao: 1, esperado: 5, grid: 22, vitorias: 6, podios: 10, corridas: 12, titulo: true, atual: false },
  { season_number: 3, ano: 2020, categoria: "", equipe_nome: null, equipe_cor: null, posicao: null, esperado: null, grid: null, vitorias: 0, podios: 0, corridas: 0, titulo: false, atual: false },
  { season_number: 4, ano: 2021, categoria: "gt3", equipe_nome: "Arclight", equipe_cor: "#dc0000", posicao: 9, esperado: 7, grid: 18, vitorias: 0, podios: 2, corridas: 16, titulo: false, atual: false },
  { season_number: 5, ano: 2022, categoria: "gt3", equipe_nome: "Arclight", equipe_cor: "#dc0000", posicao: 4, esperado: null, grid: 18, vitorias: 1, podios: 7, corridas: 16, titulo: false, atual: false },
  { season_number: 6, ano: 2023, categoria: "gt3", equipe_nome: "Ferrari", equipe_cor: "#dc0000", posicao: 1, esperado: 2, grid: 18, vitorias: 7, podios: 12, corridas: 16, titulo: true, atual: false },
  { season_number: 7, ano: 2024, categoria: "gt3", equipe_nome: "Ferrari", equipe_cor: "#dc0000", posicao: 2, esperado: 2, grid: 18, vitorias: 4, podios: 11, corridas: 16, titulo: false, atual: false },
  { season_number: 8, ano: 2025, categoria: "gt3", equipe_nome: "Ferrari", equipe_cor: "#dc0000", posicao: 5, esperado: 3, grid: 18, vitorias: 0, podios: 3, corridas: 6, titulo: false, atual: true },
];

// O caso do Connor Lee: estreou e no ano seguinte ficou fora do grid. A estreia
// e um ponto ILHADO, sem vizinho com quem formar linha.
const estreiaIlhada = [
  { season_number: 1, ano: 2020, categoria: "mazda_rookie", monomarca: true, equipe_nome: "Northstar", equipe_cor: "#3fb950", posicao: 11, esperado: 9, grid: 20, vitorias: 0, podios: 0, corridas: 8, titulo: false, atual: false },
  { season_number: 2, ano: 2021, categoria: "", equipe_nome: null, equipe_cor: null, posicao: null, esperado: null, grid: null, vitorias: 0, podios: 0, corridas: 0, titulo: false, atual: false },
  { season_number: 3, ano: 2022, categoria: "toyota_rookie", monomarca: true, equipe_nome: "Sunday Speed Club", equipe_cor: "#d29922", posicao: 7, esperado: 9, grid: 20, vitorias: 0, podios: 2, corridas: 8, titulo: false, atual: false },
  { season_number: 4, ano: 2023, categoria: "toyota_amador", monomarca: true, equipe_nome: "Sunday Speed Club", equipe_cor: "#d29922", posicao: 14, esperado: 11, grid: 20, vitorias: 0, podios: 0, corridas: 8, titulo: false, atual: false },
  { season_number: 5, ano: 2024, categoria: "gt4", equipe_nome: "Arclight", equipe_cor: "#dc0000", posicao: 10, esperado: 7, grid: 22, vitorias: 0, podios: 1, corridas: 12, titulo: false, atual: false },
  { season_number: 6, ano: 2025, categoria: "gt4", equipe_nome: "Arclight", equipe_cor: "#dc0000", posicao: 10, esperado: 13, grid: 22, vitorias: 0, podios: 2, corridas: 12, titulo: false, atual: false },
  { season_number: 7, ano: 2026, categoria: "gt4", equipe_nome: "Arclight", equipe_cor: "#dc0000", posicao: 3, esperado: 8, grid: 22, vitorias: 1, podios: 3, corridas: 4, titulo: false, atual: true },
];

function Painel({ titulo, pontos }) {
  return (
    <div className="mb-8">
      <p className="mb-2 text-xs uppercase tracking-[0.12em] text-text-muted">{titulo}</p>
      <CurvaDeCampeonato pontos={pontos} equipeDeEstreia="Northstar" />
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root")).render(
  <React.StrictMode>
    <div className="mx-auto max-w-[880px] bg-bg-primary p-8">
      <Painel titulo="Facundo Suarez — 17 temporadas, zero vitorias" pontos={carreiraLonga} />
      <Painel titulo="Dois titulos, um ano fora do grid, um ano sem referencia" pontos={carreiraVencedora} />
      <Painel titulo="Estreia ilhada, quatro anos de monomarca" pontos={estreiaIlhada} />
    </div>
  </React.StrictMode>,
);
