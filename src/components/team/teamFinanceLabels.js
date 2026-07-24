import i18n from "../../i18n/index.js";

// Mapeador de rótulos do estado financeiro da equipe. Vive fora do MyTeamTab porque é
// usado dos dois lados da fronteira (o tab e o TeamHistoryDrawer) — manter aqui evita
// ciclo de import entre os dois módulos.
export function financialState(state) {
  return {
    elite: i18n.t("myTeamTab.finance.states.elite"),
    healthy: i18n.t("myTeamTab.finance.states.healthy"),
    stable: i18n.t("myTeamTab.finance.states.stable"),
    pressured: i18n.t("myTeamTab.finance.states.pressured"),
    crisis: i18n.t("myTeamTab.finance.states.crisis"),
    collapse: i18n.t("myTeamTab.finance.states.collapse"),
  }[state] ?? i18n.t("myTeamTab.finance.states.stable");
}
