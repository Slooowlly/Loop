import Header from "./Header";

function MainLayout({ children, activeTab, onTabChange, hideHeader = false }) {
  return (
    <div className="app-shell flex h-screen flex-col">
      <div className="app-backdrop" />

      {!hideHeader && <Header activeTab={activeTab} onTabChange={onTabChange} />}

      <main className="relative z-10 flex-1 overflow-y-auto px-3 py-4 sm:px-4 lg:px-5 xl:px-6">
        <div className="mx-auto w-full max-w-[1680px] pb-8">{children}</div>
      </main>
    </div>
  );
}

export default MainLayout;
