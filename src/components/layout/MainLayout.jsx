import Header from "./Header";
import PauseMenu from "./PauseMenu";
import IracingConnectedOverlay from "../iracing/IracingConnectedOverlay";

function MainLayout({ children, activeTab, onTabChange, hideHeader = false }) {
  return (
    <div className="app-shell flex h-screen flex-col overflow-hidden">
      <div className="app-backdrop" />

      {/* Único scroller da tela: o Header vive DENTRO dele (não fixo), então ao
          rolar para baixo o header/banner sobe e some, liberando toda a altura
          para as tabelas. min-h-0 garante que este bloco role em vez de esticar. */}
      <div className="relative z-10 min-h-0 flex-1 overflow-y-auto">
        {!hideHeader && <Header activeTab={activeTab} onTabChange={onTabChange} />}

        <main className="px-3 py-4 sm:px-4 lg:px-5 xl:px-6">
          <div className="mx-auto w-full max-w-[1680px] pb-8">{children}</div>
        </main>
      </div>

      {/* Menu de pausa (ESC) — voltar ao menu, salvar, configurações. */}
      <PauseMenu />

      {/* Feedback "iRacing Conectado" ao voltar o foco pro app com o sim aberto. */}
      <IracingConnectedOverlay />
    </div>
  );
}

export default MainLayout;
