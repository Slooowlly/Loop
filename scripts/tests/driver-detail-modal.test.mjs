import test from "node:test";
import assert from "node:assert/strict";
import { access, readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, "..", "..");

test("driver detail drawer stays above the app layers and closes with a coordinated exit animation", async () => {
  await assert.doesNotReject(() =>
    access(path.join(projectRoot, "src/components/driver/DriverDetailModal.jsx")),
  );

  const drawerSource = await readFile(
    path.join(projectRoot, "src/components/driver/DriverDetailModal.jsx"),
    "utf8",
  );
  const standingsSource = await readFile(
    path.join(projectRoot, "src/pages/tabs/StandingsTab.jsx"),
    "utf8",
  );
  const indexCssSource = await readFile(
    path.join(projectRoot, "src/index.css"),
    "utf8",
  );

  assert.match(
    standingsSource,
    /selectedDriverId/,
    "expected StandingsTab to track the selected driver",
  );
  assert.match(
    standingsSource,
    /DriverDetailModal/,
    "expected StandingsTab to render the driver detail modal",
  );
  assert.match(
    standingsSource,
    /driverIds=\{driverStandings\.map\(\(driver\) => driver\.id\)\}/,
    "expected StandingsTab to pass the current standings order into the driver detail modal",
  );
  assert.match(
    standingsSource,
    /onSelectDriver=\{setSelectedDriverId\}/,
    "expected StandingsTab to let the driver detail modal change the selected driver directly",
  );
  assert.match(
    drawerSource,
    /fixed inset-y-0 right-0/,
    "expected the detail view to be anchored as a right drawer",
  );
  assert.match(
    drawerSource,
    /export default function DriverDetailModal\(\{[\s\S]*driverId,[\s\S]*driverIds = \[\],[\s\S]*onSelectDriver = null,[\s\S]*onClose,[\s\S]*\}\)/,
    "expected the driver detail modal to accept the ordered driver list and a selection callback",
  );
  assert.match(
    drawerSource,
    /animate-drawer-in/,
    "expected the detail view to use a drawer entrance animation",
  );
  assert.match(
    drawerSource,
    /animate-drawer-out/,
    "expected the detail view to support a drawer exit animation",
  );
  assert.match(
    drawerSource,
    /animate-fade-out/,
    "expected the backdrop to support a fade-out animation during close",
  );
  assert.match(
    drawerSource,
    /setTimeout/,
    "expected the modal to delay onClose until the exit animation finishes",
  );
  assert.match(
    drawerSource,
    /const \[showEdgeNavigator, setShowEdgeNavigator\] = useState\(false\);/,
    "expected the modal to track edge navigator visibility separately from the drawer itself",
  );
  assert.match(
    drawerSource,
    /const hasShownEdgeNavigatorRef = useRef\(false\);/,
    "expected the modal to remember whether the external navigator has already completed its first entrance",
  );
  assert.match(
    drawerSource,
    /requestClose/,
    "expected the close interactions to go through a shared animated close handler",
  );
  assert.doesNotMatch(
    drawerSource,
    /querySelector\("header"\)/,
    "expected the drawer to stop measuring the header",
  );
  assert.doesNotMatch(
    drawerSource,
    /getBoundingClientRect\(\)\.bottom/,
    "expected the drawer to stop using the header bottom edge for placement",
  );
  assert.match(
    drawerSource,
    /z-\[60\]/,
    "expected the drawer shell to sit above the app header layer",
  );
  assert.match(
    drawerSource,
    /createPortal/,
    "expected the drawer to use a portal so it escapes the main stacking context",
  );
  assert.match(
    drawerSource,
    /document\.body/,
    "expected the drawer portal target to be document.body",
  );
  assert.match(
    drawerSource,
    /detail\.(perfil|profile)\.(licenca|license)/,
    "expected the drawer header to read the driver's license badge near the name",
  );
  assert.match(
    drawerSource,
    /function Section\(\{ title, headerLeft = null, headerRight = null, children, className = "" \}\)/,
    "expected sections to support both an inline-left slot and a centered right-side slot for header metadata",
  );
  assert.match(
    drawerSource,
    /relative mb-3 min-h-\[26px\] border-b border-\[#21262d\] pb-1\.5[\s\S]*relative z-\[1\] flex items-center gap-2 pr-8[\s\S]*\{headerLeft\}[\s\S]*absolute inset-x-0 top-1\/2 flex -translate-y-1\/2 justify-center/,
    "expected section headers to keep left metadata anchored while centering secondary header content against the whole modal width",
  );
  assert.match(
    drawerSource,
    /const profileHeaderMeta = \([\s\S]*flex w-full items-center justify-center[\s\S]*<MotivationBar[\s\S]*value=\{competitivo\?\.motivacao\}[\s\S]*compact[\s\S]*\/>[\s\S]*\);/,
    "expected the Perfil header meta area to center only the compact motivation bar",
  );
  assert.match(
    drawerSource,
    /<Section[\s\S]*title="Perfil"[\s\S]*headerLeft=\{licenseLevelBadge \? <BadgePill badge=\{licenseLevelBadge\} \/> : null\}/,
    "expected the Perfil section to place the Rookie badge beside the title instead of next to motivation",
  );
  assert.match(
    drawerSource,
    /<MotivationBar[\s\S]*className="min-w-\[220px\] sm:min-w-\[320px\] lg:min-w-\[420px\]"/,
    "expected the header motivation bar to stretch wider across the top area",
  );
  assert.match(
    drawerSource,
    /<Section[\s\S]*title="Perfil"[\s\S]*headerRight=\{profileHeaderMeta\}/,
    "expected the Perfil section header to surface both the badge and motivation in the top bar",
  );
  assert.match(
    drawerSource,
    /function MotivationBar\(\{ value, compact = false, className = "" \}\)[\s\S]*if \(compact\)[\s\S]*bg-transparent[\s\S]*flex-1[\s\S]*Motivacao/,
    "expected the motivation component to support a slimmer borderless compact header variant",
  );
  assert.doesNotMatch(
    drawerSource,
    /if \(compact\) \{[\s\S]*w-9 text-right font-mono text-\[10px\][\s\S]*\{normalized\}%/,
    "expected the compact motivation bar in the header to rely on the visual fill only, without a text percentage",
  );
  assert.match(
    drawerSource,
    /FlagIcon/,
    "expected the drawer header to reuse the shared FlagIcon component for nationality rendering",
  );
  assert.match(
    drawerSource,
    /perfil\?\.idade \?\? detail\.idade\} anos/,
    "expected the driver's age to move into the main header line near the name",
  );
  assert.match(
    drawerSource,
    /const currentDriverIndex = driverIds\.indexOf\(driverId\);[\s\S]*const previousDriverId = currentDriverIndex > 0 \? driverIds\[currentDriverIndex - 1\] : null;[\s\S]*const nextDriverId =[\s\S]*driverIds\[currentDriverIndex \+ 1\][\s\S]*: null;/,
    "expected the modal to derive previous and next drivers from the standings order without looping",
  );
  assert.match(
    drawerSource,
    /function selectAdjacentDriver\(targetDriverId\) \{[\s\S]*if \(!targetDriverId \|\| !onSelectDriver \|\| isClosing\) return;[\s\S]*onSelectDriver\(targetDriverId\);[\s\S]*\}/,
    "expected adjacent-driver navigation to use a guarded shared selection handler",
  );
  assert.match(
    drawerSource,
    /edgeNavigatorTimeoutRef = useRef\(null\)/,
    "expected the modal to keep a dedicated timeout ref for edge navigator timing",
  );
  assert.match(
    drawerSource,
    /if \(!hasShownEdgeNavigatorRef\.current\) \{[\s\S]*setShowEdgeNavigator\(false\);[\s\S]*edgeNavigatorTimeoutRef\.current = window\.setTimeout\(\(\) => \{[\s\S]*hasShownEdgeNavigatorRef\.current = true;[\s\S]*setShowEdgeNavigator\(true\);[\s\S]*\}, CLOSE_ANIMATION_MS\);[\s\S]*\} else \{[\s\S]*setShowEdgeNavigator\(true\);[\s\S]*\}/,
    "expected the external navigator to wait only for the first drawer entrance animation and stay visible during pilot-to-pilot navigation",
  );
  assert.match(
    drawerSource,
    /function requestClose\(\) \{[\s\S]*setIsClosing\(true\);[\s\S]*setShowEdgeNavigator\(false\);[\s\S]*window\.clearTimeout\(edgeNavigatorTimeoutRef\.current\);/,
    "expected the external navigator to hide immediately when the drawer starts closing",
  );
  assert.match(
    drawerSource,
    /function DriverEdgeNavigator\(\{[\s\S]*drawerWidth,[\s\S]*viewportWidth,[\s\S]*previousDriverId,[\s\S]*nextDriverId,[\s\S]*onSelectDriver,[\s\S]*visible,[\s\S]*isClosing,[\s\S]*\}\)/,
    "expected the modal to extract adjacent-driver navigation into a dedicated edge navigator",
  );
  assert.match(
    drawerSource,
    /function DriverEdgeNavigator[\s\S]*if \(!onSelectDriver \|\| viewportWidth < 768 \|\| !visible\) return null;[\s\S]*pointer-events-auto fixed top-24 z-\[61\] flex flex-col gap-2 sm:top-28[\s\S]*style=\{\{ right: `\$\{railRight\}px` \}\}/,
    "expected the adjacent-driver controls to stay fixed outside the drawer on the left edge and remain hidden until the drawer animation finishes",
  );
  assert.match(
    drawerSource,
    /function DriverEdgeNavigator[\s\S]*className="animate-edge-rail-in pointer-events-auto fixed top-24 z-\[61\] flex flex-col gap-2 sm:top-28"/,
    "expected the external navigator to play its own secondary drawer animation after becoming visible",
  );
  assert.match(
    indexCssSource,
    /\.animate-edge-rail-in \{[\s\S]*animation: edge-rail-in 0\.18s cubic-bezier\(0\.22, 1, 0\.36, 1\);[\s\S]*\}/,
    "expected the shared styles to define a dedicated animation class for the edge navigator reveal",
  );
  assert.match(
    indexCssSource,
    /@keyframes edge-rail-in \{[\s\S]*opacity: 0;[\s\S]*translateX\(18px\)[\s\S]*opacity: 1;[\s\S]*translateX\(0\)/,
    "expected the edge navigator reveal to slide outward like a secondary drawer",
  );
  assert.match(
    drawerSource,
    /function DriverNavigatorButton\(\{ label, direction, disabled, onClick \}\)[\s\S]*flex h-10 w-10 items-center justify-center rounded-2xl border backdrop-blur-md transition-all duration-200 ease-out[\s\S]*bg-\[#161b22\]\/96 text-\[#c9d1d9\]/,
    "expected the adjacent-driver controls to stay visibly present at rest as icon-only buttons",
  );
  assert.doesNotMatch(
    drawerSource,
    /function DriverNavigatorButton[\s\S]*hover:w-\[118px\]|function DriverNavigatorButton[\s\S]*group-hover:opacity-100/,
    "expected the external navigator buttons to stop expanding and showing text on hover",
  );
  assert.match(
    drawerSource,
    /aria-label="Fechar ficha do piloto"[\s\S]*<DriverEdgeNavigator[\s\S]*drawerWidth=\{drawerWidth\}[\s\S]*viewportWidth=\{viewportWidth\}[\s\S]*previousDriverId=\{previousDriverId\}[\s\S]*nextDriverId=\{nextDriverId\}[\s\S]*onSelectDriver=\{selectAdjacentDriver\}[\s\S]*visible=\{showEdgeNavigator && !isClosing\}[\s\S]*isClosing=\{isClosing\}[\s\S]*<div[\s\S]*fixed inset-y-0 right-0/,
    "expected the portal root to mount the external adjacent-driver navigator alongside the drawer instead of inside the scrollable panel",
  );
  assert.doesNotMatch(
    drawerSource,
    /className="hidden"[\s\S]*aria-label="Ver piloto anterior"/,
    "expected the old hidden adjacent-driver controls near the age to be removed after moving navigation to the external rail",
  );
  assert.match(
    drawerSource,
    /label="Anterior"[\s\S]*disabled=\{!previousDriverId \|\| isClosing\}[\s\S]*onClick=\{\(\) => onSelectDriver\(previousDriverId\)\}/,
    "expected the external navigator to disable the previous button at the top of the list",
  );
  assert.match(
    drawerSource,
    /label="Proximo"[\s\S]*disabled=\{!nextDriverId \|\| isClosing\}[\s\S]*onClick=\{\(\) => onSelectDriver\(nextDriverId\)\}/,
    "expected the external navigator to disable the next button at the bottom of the list",
  );
  assert.match(
    drawerSource,
    /const visibleBadges = perfil\?\.badges\?\.filter\(\(badge\) => badge\.label !== "ROOKIE"\) \|\| \[\]/,
    "expected the header badges to filter out the redundant rookie badge",
  );
  assert.doesNotMatch(
    drawerSource,
    /detail\.perfil\.licenca\.sigla/,
    "expected the name row to stop rendering the license shorthand after moving the rookie label to the Perfil header",
  );
  assert.match(
    drawerSource,
    /mb-3 text-sm text-\[#c9d1d9\][\s\S]*detail\.papel === "Numero1"[\s\S]*perfil\?\.equipe_nome/,
    "expected the role and team line to sit near the driver's name before the remaining badges",
  );
  assert.equal(
    (drawerSource.match(/<FlagIcon/g) || []).length,
    1,
    "expected the drawer header to keep only one visible flag",
  );
  assert.doesNotMatch(
    drawerSource,
    /perfil\?\.status \|\| detail\.status/,
    "expected the driver status label to be removed from the visible header metadata",
  );
  assert.match(
    drawerSource,
    /const competitivo = detail\?\.competitivo|detail\.(competitivo|competitive)|competitivo\?\./,
    "expected the drawer to consume a combined competitive block",
  );
  assert.match(
    drawerSource,
    /const profileHeaderMeta = \([\s\S]*<MotivationBar[\s\S]*value=\{competitivo\?\.motivacao\}[\s\S]*compact/,
    "expected motivation to move into the Perfil header meta area to save vertical space",
  );
  assert.match(
    drawerSource,
    /function HeaderPersonalityList[\s\S]*competitivo\?\.personalidade_primaria[\s\S]*personality\.tipo/,
    "expected the personality summary component to render the primary personality in the marked left-side area",
  );
  assert.match(
    drawerSource,
    /function HeaderPersonalityList[\s\S]*competitivo\?\.personalidade_secundaria/,
    "expected the personality summary component to support the secondary personality alongside the primary one",
  );
  assert.match(
    drawerSource,
    /title="Perfil"[\s\S]*<HeaderPersonalityList competitivo=\{competitivo\} \/>/,
    "expected the profile header to place the personality summary inside the left-side dead space",
  );
  assert.match(
    drawerSource,
    /title="Perfil"[\s\S]*lg:grid-cols-\[300px_minmax\(0,1fr\)\][\s\S]*<ProsConsPanel competitivo=\{competitivo\} className="h-\[118px\] w-full lg:h-\[118px\]" \/>/,
    "expected the drawer header to use the dead space beside the name for the pros-and-cons panel",
  );
  assert.match(
    drawerSource,
    /function ProsConsPanel[\s\S]*h-\[138px\][\s\S]*grid-cols-2[\s\S]*Qualidades[\s\S]*Pontos de atencao[\s\S]*overflow-y-auto/,
    "expected the pros-and-cons panel near the header to keep a fixed height and split pros/cons side by side with internal scrolling",
  );
  assert.match(
    drawerSource,
    /function ProsConsPanel[\s\S]*"flex h-\[138px\] min-h-0 flex-col lg:h-full"/,
    "expected the header pros-and-cons area to use a plain text layout instead of a glass card wrapper",
  );
  assert.match(
    drawerSource,
    /title="Perfil"[\s\S]*lg:min-h-\[146px\]/,
    "expected the profile header area to keep a fixed desktop height instead of growing with the content",
  );
  assert.match(
    drawerSource,
    /title="Perfil"[\s\S]*lg:pt-4[\s\S]*<ProsConsPanel competitivo=\{competitivo\}/,
    "expected the header pros-and-cons area to start lower so it aligns with the driver's name block instead of sticking to the top-right corner",
  );
  assert.doesNotMatch(
    drawerSource,
    /<Section title="Mental">/,
    "expected the Mental section to be removed after moving motivation into the Perfil header",
  );
  assert.doesNotMatch(
    drawerSource,
    /<CompetitiveSection detail=\{detail\} competitivo=\{competitivo\} \/>/,
    "expected the Atual tab to stop rendering the old mental section component",
  );
  assert.match(
    drawerSource,
    /detail\.(forma|form)/,
    "expected the drawer to render a current-form section",
  );
  assert.match(
    drawerSource,
    /Forma recente/,
    "expected the current moment summary card to be renamed to Forma recente",
  );
  assert.match(
    drawerSource,
    /Situacao contratual/,
    "expected the contract card to be renamed to Situacao contratual",
  );
  assert.match(
    drawerSource,
    /Status de forma/,
    "expected the current form card to label the form status explicitly",
  );
  assert.match(
    drawerSource,
    /Expira em/,
    "expected the contract card to emphasize when the contract expires",
  );
  assert.match(
    drawerSource,
    /Salario anual/,
    "expected the contract card to clarify the salary period",
  );
  assert.match(
    drawerSource,
    /Vigencia[\s\S]*Temporada .* ate /,
    "expected contract duration to read as 'Temporada X ate Y'",
  );
  assert.match(
    drawerSource,
    /function formatContractRole[\s\S]*Piloto N1[\s\S]*Piloto N2[\s\S]*label="Funcao"[\s\S]*formatContractRole\(contract\.papel\)/,
    "expected the contract role to be normalized to Piloto N1/N2",
  );
  assert.match(
    drawerSource,
    /const\s+\[\s*activeTab,\s*setActiveTab\s*\]\s*=\s*useState\(["']atual["']\)/,
    "expected the drawer to initialize its internal navigation on the Atual tab",
  );
  assert.match(
    drawerSource,
    /"Atual".*"Forma".*"Carreira".*"Mercado"|["']Atual["'][\s\S]*["']Forma["'][\s\S]*["']Carreira["'][\s\S]*["']Mercado["']/,
    "expected the drawer to declare the four dossier tabs",
  );
  assert.match(
    drawerSource,
    /activeTab\s*===\s*["']carreira["']/,
    "expected the career content to be hidden behind the Carreira tab instead of rendering by default",
  );
  assert.match(
    drawerSource,
    /trajetoria\??\.(titulos|foi_campeao)|detail\.trajetoria\??\.(titulos|foi_campeao)/,
    "expected the drawer to surface championship status from the career path block",
  );
  assert.doesNotMatch(
    drawerSource,
    /label:\s*"Pontos"/,
    "expected points to stop being a primary stat card in the dossier",
  );
  assert.doesNotMatch(
    drawerSource,
    /title="Quali"|title:\s*"Quali"/,
    "expected the dossier to stop splitting race info into a qualifying block",
  );
  assert.doesNotMatch(
    drawerSource,
    /label:\s*"Poles"|label:\s*"Hat-tricks"/,
    "expected the dossier to focus the primary performance cards on race information only",
  );
  assert.doesNotMatch(
    standingsSource,
    /xl:pr-\[30rem\]/,
    "expected StandingsTab to stop pushing the whole grid for the drawer",
  );
});

test("driver detail modal stops loading safely without ids and delegates dense dossier sections", async () => {
  const drawerSource = await readFile(
    path.join(projectRoot, "src/components/driver/DriverDetailModal.jsx"),
    "utf8",
  );

  assert.match(
    drawerSource,
    /if \(!driverId \|\| !careerId\) \{[\s\S]*setLoading\(false\);[\s\S]*return;[\s\S]*\}/,
    "expected the modal fetch flow to stop loading immediately when ids are missing",
  );
  assert.match(
    drawerSource,
    /from "\.\/DriverDetailModalSections"/,
    "expected the modal to import dossier sections from a dedicated companion module",
  );
  assert.match(
    drawerSource,
    /<FormSection detail=\{detail\} moment=\{moment\} \/>/,
    "expected the form tab to use the extracted section component",
  );
  assert.match(
    drawerSource,
    /<CareerSection detail=\{detail\} trajetoria=\{trajetoria\} \/>/,
    "expected the career tab to use the extracted section component",
  );
  assert.match(
    drawerSource,
    /<MarketSection detail=\{detail\} market=\{market\} \/>/,
    "expected the market tab to use the extracted section component",
  );
});

test("formatters exports formatSalary for contract rendering", async () => {
  const formattersModule = await import(
    pathToFileURL(path.join(projectRoot, "src/utils/formatters.js")).href
  );

  assert.equal(
    typeof formattersModule.formatSalary,
    "function",
    "expected formatSalary to be exported",
  );
  assert.equal(formattersModule.formatSalary(12500), "$12,500");
  assert.equal(
    formattersModule.extractNationalityCode("JP Japones"),
    "jp",
    "expected nationality code extraction to support stored country-code strings",
  );
  assert.equal(
    formattersModule.extractFlag("JP Japones"),
    "🇯🇵",
    "expected flag extraction to resolve an emoji from stored country-code strings",
  );
});
