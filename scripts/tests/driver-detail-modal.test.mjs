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
  const dossierSectionsSource = await readFile(
    path.join(projectRoot, "src/components/driver/DriverDetailModalSections.jsx"),
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
    "expected sections to support inline-left and right-side header metadata slots",
  );
  assert.match(
    drawerSource,
    /mb-5 overflow-hidden rounded-xl border border-white\/10 bg-\[#0a0f1c\]\/60[\s\S]*flex min-h-\[44px\] items-center justify-between[\s\S]*\{headerLeft\}[\s\S]*\{headerRight\}/,
    "expected sections to render as framed dossier cards with a compact header row",
  );
  assert.match(
    drawerSource,
    /<Section[\s\S]*title="Perfil"[\s\S]*headerRight=\{licenseLevelBadge \? <BadgePill badge=\{licenseLevelBadge\} \/> : null\}/,
    "expected the Perfil section to place the license badge on the right side of the card header",
  );
  assert.match(
    drawerSource,
    /title="Perfil"[\s\S]*<MotivationBar value=\{competitivo\?\.motivacao\} compact \/>[\s\S]*<ProsConsPanel competitivo=\{competitivo\} className="w-full" \/>/,
    "expected the Perfil body to place motivation above the pros-and-cons panel on the right",
  );
  assert.match(
    drawerSource,
    /function DossierTabs[\s\S]*grid grid-cols-2[\s\S]*sm:grid-cols-4[\s\S]*min-h-9 rounded-lg border px-3 text-sm font-medium/,
    "expected dossier tabs to render as four equal-width controls below the profile card",
  );
  assert.match(
    drawerSource,
    /function MotivationBar\(\{ value, compact = false, className = "" \}\)[\s\S]*if \(compact\)[\s\S]*bg-transparent[\s\S]*Motivacao[\s\S]*\{normalized\}%/,
    "expected the compact motivation component to show both the label, fill, and percentage in the profile body",
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
    /<h2[\s\S]*truncate[\s\S]*\{detail\.nome\}/,
    "expected long driver names to stay on the name line instead of wrapping below the flag",
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
    /title="Perfil"[\s\S]*<MotivationBar value=\{competitivo\?\.motivacao\} compact \/>/,
    "expected motivation to sit in the right side of the Perfil card body",
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
    /title="Perfil"[\s\S]*lg:grid-cols-\[300px_minmax\(0,1fr\)\][\s\S]*<MotivationBar value=\{competitivo\?\.motivacao\} compact \/>[\s\S]*<ProsConsPanel competitivo=\{competitivo\} className="w-full" \/>/,
    "expected the drawer header to use the dead space beside the name for the pros-and-cons panel",
  );
  assert.match(
    drawerSource,
    /function ProsConsPanel[\s\S]*grid h-\[118px\] min-h-0 grid-cols-2[\s\S]*Pontos fortes[\s\S]*Atencao[\s\S]*overflow-y-auto/,
    "expected the pros-and-cons panel near the header to keep a fixed height and split pros/cons side by side with internal scrolling",
  );
  assert.match(
    drawerSource,
    /function ProsConsPanel[\s\S]*rounded-xl border border-white\/8 bg-white\/\[0\.045\] p-3/,
    "expected the header pros-and-cons area to use two small framed boxes like the refined mockup",
  );
  assert.match(
    drawerSource,
    /title="Perfil"[\s\S]*lg:min-h-\[170px\]/,
    "expected the profile header area to keep a fixed desktop height instead of growing with the content",
  );
  assert.match(
    drawerSource,
    /title="Perfil"[\s\S]*grid min-w-0 gap-3 lg:pt-4[\s\S]*<MotivationBar[\s\S]*<ProsConsPanel competitivo=\{competitivo\}/,
    "expected the right side of the profile body to align motivation and pros-and-cons below the card header",
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
    /Vigencia[\s\S]*formatContractPeriod\(contract\)/,
    "expected contract duration to read as calendar years",
  );
  assert.doesNotMatch(
    drawerSource,
    /Temporada \$\{contract\.temporada_inicio\} ate \$\{contract\.temporada_fim\}/,
    "expected current contract duration to stop rendering as season numbers",
  );
  assert.match(
    drawerSource,
    /formatContractPeriod\(contract\)/,
    "expected current contract duration to render as calendar years",
  );
  assert.match(
    dossierSectionsSource,
    /function formatContractPeriod\(contract\)[\s\S]*contract\.ano_inicio[\s\S]*contract\.ano_fim/,
    "expected the Mercado tab to format contract duration with calendar years",
  );
  assert.doesNotMatch(
    dossierSectionsSource,
    /Temp \{detail\.contrato_mercado\.contrato\.temporada_inicio\}/,
    "expected the Mercado tab to stop rendering duration as season numbers",
  );
  assert.match(
    drawerSource,
    /function formatContractRole[\s\S]*return "N1"[\s\S]*return "N2"[\s\S]*label="Funcao"[\s\S]*formatContractRole\(contract\.papel\)/,
    "expected the contract role to be normalized to N1/N2 without redundant wording",
  );
  assert.match(
    drawerSource,
    /const\s+\[\s*activeTab,\s*setActiveTab\s*\]\s*=\s*useState\(["']resumo["']\)/,
    "expected the drawer to initialize its internal navigation on the Resumo tab",
  );
  assert.doesNotMatch(
    drawerSource,
    /useEffect\(\(\) => \{[\s\S]*setActiveTab\("resumo"\)[\s\S]*\}, \[driverId\]\)/,
    "expected adjacent-driver navigation to preserve the selected dossier tab",
  );
  assert.match(
    drawerSource,
    /["']Resumo["'][\s\S]*["']Historico["'][\s\S]*["']Rivais["'][\s\S]*["']Mercado["']/,
    "expected the drawer to declare the consolidated dossier tabs",
  );
  assert.doesNotMatch(
    drawerSource,
    /id:\s*["']qualidade["']|id:\s*["']leitura["']/,
    "expected quality and performance reading to stop being standalone tabs",
  );
  assert.match(
    drawerSource,
    /activeTab\s*===\s*["']historico["']/,
    "expected the career content to be hidden behind the Historico tab instead of rendering by default",
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
    drawerSource,
    /LIDER/,
    "expected the redundant leader badge nomenclature to be removed from the driver dossier",
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
  const dossierSectionsSource = await readFile(
    path.join(projectRoot, "src/components/driver/DriverDetailModalSections.jsx"),
    "utf8",
  );
  const rookieDossierSource = dossierSectionsSource.slice(
    dossierSectionsSource.indexOf("function RookieDossierState"),
    dossierSectionsSource.indexOf("function RookieUnavailableSection"),
  );
  const rookieUnavailableSource = dossierSectionsSource.slice(
    dossierSectionsSource.indexOf("function RookieUnavailableSection"),
    dossierSectionsSource.indexOf("function FormMetric"),
  );
  const recentFormChartSource = dossierSectionsSource.slice(
    dossierSectionsSource.indexOf("function resultColor"),
    dossierSectionsSource.indexOf("function TimelineItem"),
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
    /<SummarySection detail=\{detail\} moment=\{moment\} \/>/,
    "expected the summary tab to use the extracted section component",
  );
  assert.doesNotMatch(
    drawerSource,
    /activeTab\s*===\s*["']qualidade["']|activeTab\s*===\s*["']leitura["']/,
    "expected quality and performance reading to render inside Mercado instead of their own tabs",
  );
  assert.match(
    drawerSource,
    /<HistorySection detail=\{detail\} trajetoria=\{trajetoria\} \/>/,
    "expected the history tab to use the extracted section component",
  );
  assert.match(
    drawerSource,
    /<RivalsSection detail=\{detail\} \/>/,
    "expected the rivals tab to use the extracted section component",
  );
  assert.match(
    drawerSource,
    /<MarketSection detail=\{detail\} market=\{market\} \/>/,
    "expected the market tab to use the extracted section component",
  );
  assert.match(
    dossierSectionsSource,
    /export function MarketSection\(\{ SectionComponent, detail, market \}\)[\s\S]*<QualitySection SectionComponent=\{SectionComponent\} detail=\{detail\} \/>[\s\S]*<PerformanceReadSection SectionComponent=\{SectionComponent\} detail=\{detail\} \/>/,
    "expected Mercado to integrate the quality map and performance reading sections",
  );
  assert.doesNotMatch(
    dossierSectionsSource,
    /const QUALITY_LEVELS = \[[\s\S]*"Muito fraco"[\s\S]*"Fraco"[\s\S]*"Abaixo do esperado"[\s\S]*"Inst[aá]vel"[\s\S]*"Competente"[\s\S]*"Forte"[\s\S]*"Muito forte"[\s\S]*"Elite"[\s\S]*\]/,
    "expected the quality tab to stop deriving technical readings locally",
  );
  assert.doesNotMatch(
    dossierSectionsSource,
    /function QualityLevelRow\(\{ label, value \}\)[\s\S]*qualityLevelFromValue\(value\)/,
    "expected technical quality rows to stop deriving levels from local numeric values",
  );
  assert.match(
    dossierSectionsSource,
    /const technicalReadings = detail\.leitura_tecnica\?\.itens \?\? \[\]/,
    "expected the quality tab to consume backend technical readings",
  );
  assert.match(
    dossierSectionsSource,
    /function QualityLevelRow\(\{ item \}\)[\s\S]*\{item\.label\}[\s\S]*\{item\.nivel\}/,
    "expected technical quality rows to render backend textual levels",
  );
  assert.doesNotMatch(
    dossierSectionsSource,
    /<StatCard label="Motivacao"|<StatCard label="Motivação"/,
    "expected the quality base block to avoid the redundant motivation card",
  );
  assert.doesNotMatch(
    dossierSectionsSource,
    /title="Pontos Fortes e Atencao"|title="Pontos Fortes e Atenção"/,
    "expected the quality tab to avoid duplicating the header's strengths and attention block",
  );
  assert.match(
    dossierSectionsSource,
    /function CareerRankStat\(\{ label, value, rank, tone = "text-\[#e6edf3\]" \}\)[\s\S]*text-\[11px\][\s\S]*formatRank\(rank\)/,
    "expected career history ordinals to render smaller than the absolute values",
  );
  assert.match(
    dossierSectionsSource,
    /function RookieFormState\(\)[\s\S]*ESTREANTE[\s\S]*>0<[\s\S]*corridas/,
    "expected rookie recent form to communicate the state visually without requiring a paragraph read",
  );
  assert.match(
    dossierSectionsSource,
    /function InsufficientFormState\(\)[\s\S]*Dados insuficientes/,
    "expected non-rookie missing form data to use a distinct empty state",
  );
  assert.doesNotMatch(
    dossierSectionsSource,
    /precisa completar algumas corridas antes de o histórico revelar uma tendência confiável/,
    "expected the rookie state to avoid burying the key fact in a long sentence",
  );
  assert.match(
    dossierSectionsSource,
    /function RecentFormChart\(\{ entries, rookie, context \}\)[\s\S]*areaPolygon[\s\S]*<defs>[\s\S]*<linearGradient[\s\S]*<polygon[\s\S]*<polyline/,
    "expected recent form to render as a polished area chart instead of a raw line",
  );
  assert.match(
    recentFormChartSource,
    /className="-m-3\.5 overflow-hidden bg-\[#070b12\]"/,
    "expected the recent-form chart to expand toward the section edges instead of sitting as a compact inner card",
  );
  assert.doesNotMatch(
    recentFormChartSource,
    /rounded-xl border border-\[#58a6ff\]\/14/,
    "expected the recent-form chart to avoid a nested card shell",
  );
  assert.match(
    recentFormChartSource,
    /<rect x="0" y="0" width=\{width\} height=\{height\} rx="0" fill="#0b111c" \/>/,
    "expected the chart canvas to fill the expanded area without rounded-card corners",
  );
  assert.doesNotMatch(
    recentFormChartSource,
    /preserveAspectRatio="none"/,
    "expected the recent-form svg to keep natural proportions instead of deforming the chart",
  );
  assert.match(
    recentFormChartSource,
    /const width = 760;[\s\S]*const height = 220;[\s\S]*const chartLeft = 14;[\s\S]*const chartRight = 746;/,
    "expected the recent-form chart to use a wider native coordinate system",
  );
  assert.match(
    recentFormChartSource,
    /className="block h-auto w-full"/,
    "expected the recent-form svg to fill width while deriving height from its own proportions",
  );
  assert.match(
    dossierSectionsSource,
    /<RecentFormChart entries=\{form\.ultimas_10 \?\? form\.ultimas_5 \?\? \[\]\} rookie=\{rookie\} context=\{form\.contexto\} \/>/,
    "expected the summary recent-form chart to prefer the last 10 results while keeping old payload fallback",
  );
  assert.match(
    recentFormChartSource,
    /function resultColor\(entry\)[\s\S]*entry\?\.dnf[\s\S]*#f85149[\s\S]*finish === 1[\s\S]*#d29922[\s\S]*finish <= 3[\s\S]*#3fb950[\s\S]*finish <= 10[\s\S]*#58a6ff[\s\S]*#8b949e/,
    "expected recent-form point colors to distinguish P1, podiums, top 10s, muted finishes, and DNFs",
  );
  assert.match(
    recentFormChartSource,
    /function resultOpacity\(entry\)[\s\S]*entry\?\.dnf[\s\S]*return 1[\s\S]*finish > 10 \? 0\.36 : 1/,
    "expected recent-form results outside the top 10 to render with reduced opacity",
  );
  assert.match(
    recentFormChartSource,
    /function resultLabel\(entry\)[\s\S]*entry\?\.dnf[\s\S]*"DNF"[\s\S]*`P\$\{entry\.chegada\}`/,
    "expected recent-form points to expose a position label for each result",
  );
  assert.match(
    recentFormChartSource,
    /stroke=\{resultColor\(point\.entry\)\}/,
    "expected each recent-form point to use the semantic result color",
  );
  assert.match(
    recentFormChartSource,
    /opacity=\{resultOpacity\(point\.entry\)\}/,
    "expected each recent-form point to apply semantic opacity",
  );
  assert.match(
    recentFormChartSource,
    /y=\{Math\.max\(14, point\.y - 12\)\}[\s\S]*fill=\{resultColor\(point\.entry\)\}[\s\S]*\{resultLabel\(point\.entry\)\}/,
    "expected each recent-form point to show its result number above the dot",
  );
  assert.doesNotMatch(
    dossierSectionsSource,
    /function FormChip|<FormChip/,
    "expected the old recent-form chips to be removed",
  );
  assert.match(
    dossierSectionsSource,
    /function isCareerDebutantDetail\(detail\)[\s\S]*stats_carreira\?\.corridas[\s\S]*=== 0/,
    "expected the rookie empty state to depend only on zero career races",
  );
  assert.doesNotMatch(
    dossierSectionsSource,
    /function isCareerDebutantDetail[\s\S]*ROOKIE/,
    "expected category/license rookie badges to stop marking experienced drivers as debutants",
  );
  assert.match(
    dossierSectionsSource,
    /function RookieDossierState\(\{ SectionComponent, title = "Resumo Atual" \}\)[\s\S]*Estreante[\s\S]*Expectativa desconhecida/,
    "expected the summary tab to show a clear rookie dossier state instead of a normal verdict",
  );
  assert.match(
    dossierSectionsSource,
    /function RookieDossierState\(\{ SectionComponent, title = "Resumo Atual" \}\)[\s\S]*flex min-h-\[180px\] flex-col items-center justify-center text-center/,
    "expected the rookie summary message to be centered instead of styled as a stat card",
  );
  assert.doesNotMatch(
    rookieDossierSource,
    /rounded-xl border/,
    "expected the rookie summary message to avoid an inner card shell",
  );
  assert.match(
    dossierSectionsSource,
    /export function SummarySection\(\{ SectionComponent, detail, moment \}\)[\s\S]*if \(rookie\) return <RookieDossierState SectionComponent=\{SectionComponent\} \/>;/,
    "expected rookie summary to skip the regular current-performance cards",
  );
  assert.match(
    dossierSectionsSource,
    /function RookieUnavailableSection\(\{ SectionComponent, title \}\)[\s\S]*Indispon[ií]vel para estreante[\s\S]*Sem passado competitivo/,
    "expected history-dependent tabs to communicate that rookie analysis is unavailable",
  );
  assert.match(
    dossierSectionsSource,
    /function RookieUnavailableSection\(\{ SectionComponent, title \}\)[\s\S]*flex min-h-\[180px\] flex-col items-center justify-center text-center/,
    "expected unavailable rookie tabs to show a centered text-only state",
  );
  assert.doesNotMatch(
    rookieUnavailableSource,
    /rounded-xl border/,
    "expected unavailable rookie tabs to avoid an inner card shell",
  );
  assert.match(
    dossierSectionsSource,
    /export function PerformanceReadSection\(\{ SectionComponent, detail \}\)[\s\S]*if \(isCareerDebutantDetail\(detail\)\) return <RookieUnavailableSection SectionComponent=\{SectionComponent\} title="Leitura de Desempenho" \/>;/,
    "expected the performance-reading tab to be unavailable for rookies",
  );
  assert.match(
    dossierSectionsSource,
    /export function HistorySection\(\{ SectionComponent, detail, trajetoria \}\)[\s\S]*if \(isCareerDebutantDetail\(detail\)\) return <RookieUnavailableSection SectionComponent=\{SectionComponent\} title="Historico de Carreira" \/>;/,
    "expected the career-history tab to be unavailable for rookies",
  );
  assert.match(
    dossierSectionsSource,
    /export function RivalsSection\(\{ SectionComponent, detail \}\)[\s\S]*if \(isCareerDebutantDetail\(detail\)\) return <RookieUnavailableSection SectionComponent=\{SectionComponent\} title="Rivais" \/>;/,
    "expected the rivals tab to be unavailable for rookies",
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
