import GlobalTeamsTab from "../../../pages/tabs/GlobalTeamsTab";

// Histórico mundial de equipes (atlas, duplo clique no card do grid).
export default function TeamHistoryOverlay({ team, onClose }) {
  return (
    <div
      className="fixed inset-0 z-[80] overflow-y-auto bg-black/80 backdrop-blur-md"
      onClick={onClose}
    >
      <div
        className="mx-auto max-w-7xl px-4 py-6 sm:px-6"
        onClick={(e) => e.stopPropagation()}
      >
        <GlobalTeamsTab
          selectedTeamId={team.id}
          selectedTeamCategory={team._categoria ?? team.categoria ?? null}
          selectedTeamClassName={team.classe ?? null}
          initialZoomYears={10}
          pinnedTeamId={team.id}
          drawerPlacement="center"
          onBack={onClose}
        />
      </div>
    </div>
  );
}
