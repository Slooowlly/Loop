/**
 * Fonte ÚNICA da detecção "estou rodando dentro do shell do Tauri?".
 *
 * Vale pras três webviews declaradas no tauri.conf.json (principal, `overlay` e
 * `engineer`) — todas servem o mesmo index.html e recebem o mesmo script de IPC.
 *
 * `__TAURI_INTERNALS__` é o global que o Tauri v2 SEMPRE injeta (é por onde o
 * próprio `invoke` passa). Não use `window.__TAURI__`: no v2 ele só existe com
 * `app.withGlobalTauri = true`, que este projeto não liga.
 *
 * É função, e não constante de módulo, de propósito: avaliada na CHAMADA, um
 * teste em jsdom consegue simular o ambiente mexendo no global sem depender da
 * ordem de import.
 */
export function estaNoTauri() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}
