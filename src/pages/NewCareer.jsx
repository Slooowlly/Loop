import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { currentLang } from "../i18n/format.js";
import { useNavigate } from "react-router-dom";

import FlagIcon from "../components/ui/FlagIcon";
import GlassButton from "../components/ui/GlassButton";
import GlassCard from "../components/ui/GlassCard";
import GlassInput from "../components/ui/GlassInput";
import NationalitySelect from "../components/ui/NationalitySelect";
import LoadingOverlay from "../components/ui/LoadingOverlay";
import TeamLogoMark from "../components/team/TeamLogoMark";
import CategoryCard from "../components/wizard/CategoryCard";
import DifficultyCard from "../components/wizard/DifficultyCard";
import StepIndicator from "../components/wizard/StepIndicator";
import TeamCard from "../components/wizard/TeamCard";
import useCareerDraft from "../hooks/useCareerDraft";
import useCareerStore from "../stores/useCareerStore";
import { extractNationalityLabel } from "../utils/formatters";
import {
  DIFFICULTIES,
  LOADING_MESSAGE_INTERVAL_MS,
  NATIONALITIES,
  STARTING_CATEGORIES,
  WIZARD_STEPS,
} from "../utils/constants";
import i18n, { DEFAULT_LANGUAGE } from "../i18n";

const INITIAL_FORM = {
  difficulty: "medio",
  playerName: "",
  nationality: "br",
  age: 20,
  category: "mazda_rookie",
  teamId: "",
};

const DRAFT_PROGRESS_POLL_MS = 1000;

/// Quantas mensagens de carregamento existem PARA VALER, contadas na mesma fonte de onde o
/// `t()` lê: as chaves `newCareer.loadingMessages.msg<i>` do locale-base (o `localeParity`
/// garante que o en-US tem as mesmas). O ciclo da tela usa `msg0..msg<n-1>`, então a contagem
/// precisa vir daqui — uma lista paralela em `constants.js` só serve para dessincronizar.
export function totalDeMensagensDeCarregamento() {
  const pacote = i18n.getResourceBundle(DEFAULT_LANGUAGE, "common");
  return Object.keys(pacote?.newCareer?.loadingMessages ?? {}).length;
}
// Campos que moldam o mundo simulado e, por isso, invalidam um draft pronto.
// Só a dificuldade entra aqui: ela alimenta `generate_historical_world` e define
// os atributos dos 200+ pilotos de IA. Nome, nacionalidade e idade do jogador só
// são usados na finalização, quando o piloto entra no grid — trocá-los depois da
// simulação custa uma reescrita de meta.json, não 26 temporadas de novo.
const WORLD_SHAPING_FIELDS = new Set(["difficulty"]);

function NewCareer() {
  const navigate = useNavigate();
  const { t } = useTranslation();
  const loadCareer = useCareerStore((state) => state.loadCareer);
  const driverPlaceholder = t("newCareer.driverPlaceholder");
  // Nacionalidades localizadas (bandeira + gentílico) resolvidas do i18n; o
  // NationalitySelect/FlagIcon leem o emoji direto desse `label`.
  const localizedNationalities = NATIONALITIES.map((nationality) => ({
    id: nationality.id,
    label: t(`newCareer.nationality.${nationality.id}`),
  }));
  const wizardStepLabels = WIZARD_STEPS.map((slug) => t(`newCareer.wizardSteps.${slug}`));
  const [step, setStep] = useState(1);
  const [formData, setFormData] = useState(INITIAL_FORM);
  const [loading, setLoading] = useState(false);
  const [loadingMessageIndex, setLoadingMessageIndex] = useState(0);
  const [error, setError] = useState("");
  // Os cinco comandos do rascunho vivem em `hooks/useCareerDraft`; aqui fica o formulário.
  const {
    draftState,
    setDraftState,
    carregar: carregarDraft,
    lerProgresso: lerProgressoDoDraft,
    gerar: gerarDraft,
    gravarIdentidade: gravarIdentidadeDoDraft,
    finalizar: finalizarDraft,
    descartar: descartarDraft,
  } = useCareerDraft();

  useEffect(() => {
    let cancelled = false;

    carregarDraft().then((state) => {
      if (cancelled || !state) return;
      applyDraftState(state, { resume: true });
    });

    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (!loading) {
      setLoadingMessageIndex(0);
      return undefined;
    }

    const total = Math.max(1, totalDeMensagensDeCarregamento());
    const timer = window.setInterval(() => {
      setLoadingMessageIndex((current) => (current + 1) % total);
    }, LOADING_MESSAGE_INTERVAL_MS);

    return () => window.clearInterval(timer);
  }, [loading]);

  useEffect(() => {
    if (!loading) return undefined;

    void lerProgressoDoDraft();
    const timer = window.setInterval(lerProgressoDoDraft, DRAFT_PROGRESS_POLL_MS);

    return () => window.clearInterval(timer);
  }, [loading, lerProgressoDoDraft]);

  const draftCategories = STARTING_CATEGORIES.filter((category) =>
    draftState?.categories?.includes(category.id),
  );
  const categoryOptions = draftCategories.length > 0 ? draftCategories : [];
  const selectedCategory =
    STARTING_CATEGORIES.find((category) => category.id === formData.category) ??
    categoryOptions[0] ??
    STARTING_CATEGORIES[0];
  const availableTeams = (draftState?.teams ?? []).filter(
    (team) => team.categoria === formData.category,
  );
  const selectedTeam =
    availableTeams.find((team) => team.id === formData.teamId) ?? availableTeams[0];
  const selectedDifficulty =
    DIFFICULTIES.find((difficulty) => difficulty.id === formData.difficulty) ?? DIFFICULTIES[1];
  const selectedNationality =
    localizedNationalities.find((nationality) => nationality.id === formData.nationality) ??
    localizedNationalities[0];
  const hasGeneratedDraft =
    draftState?.exists &&
    draftState.lifecycle_status === "draft" &&
    (draftState.teams?.length ?? 0) > 0;

  function applyDraftState(state, options = {}) {
    setDraftState(state);
    const firstCategory = state.categories?.[0] ?? INITIAL_FORM.category;
    const firstTeam = state.teams?.find((team) => team.categoria === firstCategory);
    setFormData((current) => ({
      ...current,
      // A identidade gravada no draft manda: sem reidratar, retomar um draft
      // trazia o formulário em branco, e a primeira digitação do nome contava
      // como troca de identidade e descartava o mundo já simulado.
      ...draftIdentityPatch(state),
      category: current.category && state.categories?.includes(current.category)
        ? current.category
        : firstCategory,
      teamId:
        current.teamId && state.teams?.some((team) => team.id === current.teamId)
          ? current.teamId
          : firstTeam?.id ?? "",
    }));

    if (options.resume && state.lifecycle_status === "draft" && state.teams?.length) {
      setStep(4);
    }
  }

  function updateForm(patch) {
    const shouldDiscardDraft = shouldDiscardDraftForPatch(formData, patch, draftState);
    if (shouldDiscardDraft) {
      void descartarDraft();
    }

    setFormData((current) => ({
      ...current,
      ...patch,
      ...(shouldDiscardDraft
        ? {
            category: INITIAL_FORM.category,
            teamId: "",
          }
        : {}),
    }));
  }

  function validateCurrentStep() {
    if (step === 2) {
      const trimmedName = formData.playerName.trim();
      if (trimmedName.length < 2 || trimmedName.length > 50) {
        return t("newCareer.errors.nameLength");
      }
      if (formData.age < 16 || formData.age > 30) {
        return t("newCareer.errors.ageRange");
      }
    }

    if (step === 3 && !hasGeneratedDraft) {
      return t("newCareer.errors.generateBeforeChoose");
    }

    if (step === 4 && !formData.category) {
      return t("newCareer.errors.selectCategory");
    }

    if (step === 5 && !availableTeams.some((team) => team.id === formData.teamId)) {
      return t("newCareer.errors.selectTeam");
    }

    return "";
  }

  /// Grava no draft a identidade que está na tela. A decisão de subir ou engolir o erro
  /// (`strict`) mora no hook — ver `useCareerDraft.gravarIdentidade`.
  function persistDraftIdentity({ strict = false } = {}) {
    return gravarIdentidadeDoDraft(
      {
        playerName: formData.playerName,
        nationality: formData.nationality,
        age: formData.age,
      },
      { strict },
    );
  }

  function handleNext() {
    const validationError = validateCurrentStep();
    if (validationError) {
      setError(validationError);
      return;
    }

    setError("");
    if (step === 2) persistDraftIdentity();
    setStep((current) => Math.min(current + 1, 6));
  }

  function handleBack() {
    setError("");
    if (step === 1) {
      navigate("/menu");
      return;
    }
    if (step === 2) persistDraftIdentity();
    setStep((current) => Math.max(current - 1, 1));
  }

  async function handleGenerateDraft() {
    setError("");
    setLoading(true);

    try {
      const state = await gerarDraft({
        playerName: formData.playerName.trim(),
        nationality: formData.nationality,
        age: formData.age,
        difficulty: formData.difficulty,
      });

      applyDraftState(state);
      setStep(4);
    } catch (invokeError) {
      setError(
        typeof invokeError === "string" ? invokeError : t("newCareer.errors.generateFailed"),
      );
    } finally {
      setLoading(false);
    }
  }

  async function handleCreateCareer() {
    setError("");
    if (!draftState?.career_id) {
      setError(t("newCareer.errors.generateBeforeFinalize"));
      return;
    }

    setLoading(true);

    try {
      // A finalização lê o nome do meta.json do draft. Garantir a gravação aqui
      // impede que a carreira nasça com a identidade da geração original.
      await persistDraftIdentity({ strict: true });
      const result = await finalizarDraft({
        category: formData.category,
        teamId: formData.teamId,
      });

      await loadCareer(result.career_id);
      navigate("/dashboard");
    } catch (invokeError) {
      setError(
        typeof invokeError === "string" ? invokeError : t("newCareer.errors.finalizeFailed"),
      );
    } finally {
      setLoading(false);
    }
  }

  async function handleResetWizard() {
    setError("");
    if (draftState?.exists) {
      await descartarDraft();
    }
    setStep(1);
    setFormData(INITIAL_FORM);
    setDraftState(null);
  }

  function renderStepContent() {
    if (step === 1) {
      return (
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
          {DIFFICULTIES.map((difficulty) => (
            <DifficultyCard
              key={difficulty.id}
              difficulty={difficulty}
              selected={formData.difficulty === difficulty.id}
              onSelect={(difficultyId) => updateForm({ difficulty: difficultyId })}
            />
          ))}
        </div>
      );
    }

    if (step === 2) {
      return (
        <div className="grid gap-6 xl:grid-cols-[1.3fr_0.7fr]">
          <GlassCard hover={false} className="glass-light space-y-5">
            <div>
              <p className="mb-2 text-[11px] uppercase tracking-[0.22em] text-text-secondary">
                {t("newCareer.step2.nameLabel")}
              </p>
              <GlassInput
                value={formData.playerName}
                onChange={(event) => updateForm({ playerName: event.target.value })}
                maxLength={50}
                placeholder={t("newCareer.step2.namePlaceholder")}
              />
            </div>

            <div>
              <p className="mb-2 text-[11px] uppercase tracking-[0.22em] text-text-secondary">
                {t("newCareer.labels.nationality")}
              </p>
              <NationalitySelect
                value={formData.nationality}
                onChange={(event) => updateForm({ nationality: event.target.value })}
                options={localizedNationalities}
              />
            </div>

            <div>
              <p className="mb-2 text-[11px] uppercase tracking-[0.22em] text-text-secondary">
                {t("newCareer.step2.ageLabel")}
              </p>
              <GlassInput
                type="number"
                min={16}
                max={30}
                value={formData.age}
                onChange={(event) => {
                  const nextAge = Number(event.target.value);
                  updateForm({ age: Number.isNaN(nextAge) ? 0 : nextAge });
                }}
              />
            </div>
          </GlassCard>

          <GlassCard hover={false} className="glass-light">
            <p className="text-[11px] uppercase tracking-[0.22em] text-text-secondary">
              {t("newCareer.step2.previewLabel")}
            </p>
            <h3 className="mt-4 text-3xl font-semibold text-text-primary">
              {formData.playerName.trim() || driverPlaceholder}
            </h3>
            <p className="mt-3 flex items-center gap-2 text-sm text-text-secondary">
              <FlagIcon nacionalidade={selectedNationality.label} />
              {extractNationalityLabel(selectedNationality.label) || selectedNationality.label} -{" "}
              {t("newCareer.ageYears", { count: formData.age })}
            </p>
            <div className="mt-8 space-y-4 text-sm text-text-secondary">
              <p>{t("newCareer.step2.attributesNote")}</p>
            </div>
          </GlassCard>
        </div>
      );
    }

    if (step === 3) {
      return (
        <div className="grid gap-6 xl:grid-cols-[1fr_0.45fr]">
          <GlassCard hover={false} className="glass-light rounded-[28px]">
            <p className="text-[11px] uppercase tracking-[0.22em] text-text-secondary">
              {t("newCareer.step3.draftLabel")}
            </p>
            <h3 className="mt-4 text-3xl font-semibold text-text-primary">
              {t("newCareer.step3.title")}
            </h3>
            <p className="mt-4 max-w-3xl text-sm leading-7 text-text-secondary">
              {t("newCareer.step3.description")}
            </p>

            <div className="mt-8 grid gap-3 sm:grid-cols-3">
              <div className="glass-light rounded-2xl p-4">
                <p className="text-[10px] uppercase tracking-[0.18em] text-text-muted">
                  {t("newCareer.step3.startLabel")}
                </p>
                <p className="mt-2 text-2xl font-semibold text-text-primary">2000</p>
              </div>
              <div className="glass-light rounded-2xl p-4">
                <p className="text-[10px] uppercase tracking-[0.18em] text-text-muted">
                  {t("newCareer.step3.playableLabel")}
                </p>
                <p className="mt-2 text-2xl font-semibold text-text-primary">2026</p>
              </div>
              <div className="glass-light rounded-2xl p-4">
                <p className="text-[10px] uppercase tracking-[0.18em] text-text-muted">
                  {t("newCareer.step3.statusLabel")}
                </p>
                <p className="mt-2 text-lg font-semibold text-text-primary">
                  {hasGeneratedDraft ? t("newCareer.step3.generated") : t("newCareer.step3.pending")}
                </p>
              </div>
            </div>
          </GlassCard>

          <GlassCard hover={false} className="glass-light rounded-[28px]">
            <p className="text-[11px] uppercase tracking-[0.22em] text-text-secondary">
              {t("newCareer.step3.pendingDriverLabel")}
            </p>
            <h3 className="mt-4 text-2xl font-semibold text-text-primary">
              {formData.playerName.trim() || driverPlaceholder}
            </h3>
            <div className="mt-5 space-y-4 text-sm text-text-secondary">
              <div>
                <p className="text-text-muted">{t("newCareer.labels.nationality")}</p>
                <p className="mt-1 flex items-center gap-2 text-text-primary">
                  <FlagIcon nacionalidade={selectedNationality.label} />
                  {extractNationalityLabel(selectedNationality.label) || selectedNationality.label}
                </p>
              </div>
              <div>
                <p className="text-text-muted">{t("newCareer.labels.difficulty")}</p>
                <p className="mt-1 text-text-primary">
                  {t(`newCareer.difficulty.${selectedDifficulty.id}.name`)}
                </p>
              </div>
              <div>
                <p className="text-text-muted">{t("newCareer.step3.progressLabel")}</p>
                <p className="mt-1 text-text-primary">
                  {draftState?.progress_year
                    ? t("newCareer.step3.yearValue", { year: draftState.progress_year })
                    : t("newCareer.step3.notStarted")}
                </p>
              </div>
            </div>
          </GlassCard>
        </div>
      );
    }

    if (step === 4) {
      const sharedStats = categoryOptions[0];
      return (
        <div className="space-y-5">
          {sharedStats ? (
            <div className="glass-light mx-auto flex w-fit items-stretch divide-x divide-white/10 rounded-2xl">
              {[
                [t("newCareer.stats.teams"), sharedStats.teams],
                [t("newCareer.stats.races"), sharedStats.races],
                [t("newCareer.stats.drivers"), sharedStats.drivers],
              ].map(([label, value]) => (
                <div key={label} className="flex items-baseline gap-2 px-8 py-4">
                  <span className="text-2xl font-semibold text-text-primary">{value}</span>
                  <span className="text-xs uppercase tracking-[0.18em] text-text-muted">
                    {label}
                  </span>
                </div>
              ))}
            </div>
          ) : null}
          <div className="grid gap-5 lg:grid-cols-2">
            {categoryOptions.map((category) => (
              <CategoryCard
                key={category.id}
                category={category}
                selected={formData.category === category.id}
                onSelect={(categoryId) =>
                  updateForm({
                    category: categoryId,
                    teamId:
                      draftState?.teams?.find((team) => team.categoria === categoryId)?.id ?? "",
                  })
                }
              />
            ))}
          </div>
        </div>
      );
    }

    if (step === 5) {
      return (
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
          {availableTeams.map((team) => (
            <TeamCard
              key={team.id}
              team={team}
              selected={formData.teamId === team.id}
              onSelect={(teamId) => updateForm({ teamId })}
            />
          ))}
        </div>
      );
    }

    return (
      <div className="space-y-8">
        {/* Piloto — herói centralizado */}
        <div className="text-center">
          <p className="text-[11px] uppercase tracking-[0.22em] text-text-secondary">
            {t("newCareer.step6.driverLabel")}
          </p>
          <h3 className="mt-2 text-3xl font-semibold text-text-primary">
            {formData.playerName.trim() || driverPlaceholder}
          </h3>
          <p className="mt-3 flex items-center justify-center gap-2 text-sm text-text-secondary">
            <FlagIcon nacionalidade={selectedNationality.label} />
            {extractNationalityLabel(selectedNationality.label)} ·{" "}
            {t("newCareer.ageYears", { count: formData.age })}
          </p>
        </div>

        {/* Escolhas — tiles centralizados, largura total */}
        <div className="grid gap-4 sm:grid-cols-3">
          <div className="glass-light flex flex-col items-center justify-center rounded-2xl p-5 text-center">
            <p className="text-[10px] uppercase tracking-[0.18em] text-text-muted">
              {t("newCareer.step6.carLabel")}
            </p>
            <p className="mt-2 text-2xl font-semibold text-text-primary">
              {selectedCategory.car?.split(" ")[0] ?? selectedCategory.car}
            </p>
          </div>
          <div className="glass-light flex flex-col items-center justify-center rounded-2xl p-5 text-center">
            <p className="text-[10px] uppercase tracking-[0.18em] text-text-muted">
              {t("newCareer.labels.team")}
            </p>
            <TeamLogoMark
              teamName={selectedTeam?.nome}
              color={selectedTeam?.cor_primaria}
              size="md"
              testId="confirm-team-logo"
            />
            <p className="mt-2 text-lg font-semibold leading-tight text-text-primary">
              {selectedTeam?.nome}
            </p>
          </div>
          <div className="glass-light flex flex-col items-center justify-center rounded-2xl p-5 text-center">
            <p className="text-[10px] uppercase tracking-[0.18em] text-text-muted">
              {t("newCareer.labels.difficulty")}
            </p>
            <p className="mt-2 text-2xl font-semibold text-text-primary">
              {t(`newCareer.difficulty.${selectedDifficulty.id}.name`)}
            </p>
          </div>
        </div>

        {/* O mundo que você herda */}
        {draftState?.world_summary ? (
          <div className="border-t border-white/10 pt-8">
            <div className="text-center">
              <p className="text-[11px] uppercase tracking-[0.22em] text-text-secondary">
                {t("newCareer.step6.worldTitle")}
              </p>
              <p className="mt-1 text-sm text-text-secondary">
                {t("newCareer.step6.worldSubtitle", {
                  count: draftState.world_summary.temporadas,
                })}
              </p>
            </div>
            <div className="mt-5 grid grid-cols-2 gap-4 sm:grid-cols-4">
              {[
                [t("newCareer.stats.drivers"), draftState.world_summary.pilotos],
                [t("newCareer.stats.races"), draftState.world_summary.corridas],
                [t("newCareer.stats.champions"), draftState.world_summary.campeoes],
                [t("newCareer.stats.tripleChampions"), draftState.world_summary.tricampeoes],
              ].map(([label, value]) => (
                <div key={label} className="glass-light rounded-2xl p-5 text-center">
                  <p className="text-3xl font-semibold text-text-primary">
                    {Number(value).toLocaleString(currentLang())}
                  </p>
                  <p className="mt-1 text-[10px] uppercase tracking-[0.16em] text-text-muted">
                    {label}
                  </p>
                </div>
              ))}
            </div>
          </div>
        ) : null}
      </div>
    );
  }

  return (
    <div className="app-shell px-4 py-6 text-text-primary sm:px-6 lg:px-10">
      <div className="app-backdrop" />

      <div className="relative mx-auto flex min-h-[calc(100vh-3rem)] max-w-7xl items-center justify-center py-6">
        <div className="wizard-panel glass flex max-h-[calc(100vh-4rem)] w-full flex-col overflow-hidden rounded-[32px] p-5 shadow-[0_30px_80px_rgba(0,0,0,0.42)] sm:p-8 lg:p-10">
          <div className="relative z-10 flex min-h-0 flex-1 flex-col">
            <div className="flex-shrink-0">
              <StepIndicator currentStep={step} steps={wizardStepLabels} />
            </div>

            <div className="mt-8 flex flex-shrink-0 flex-col gap-6 xl:flex-row xl:items-end xl:justify-between">
              <div>
                <p className="text-[11px] uppercase tracking-[0.24em] text-accent-primary">
                  {t(`newCareer.stepTitles.${step}`)}
                </p>
                <h1 className="mt-3 text-4xl font-semibold tracking-[-0.04em] text-text-primary sm:text-5xl">
                  {t("newCareer.heading")}
                </h1>
                <p className="mt-4 max-w-2xl text-sm leading-7 text-text-secondary sm:text-base">
                  {t(`newCareer.stepDescriptions.${step}`)}
                </p>
              </div>

              <GlassCard
                hover={false}
                className="glass-light w-full max-w-xs rounded-3xl px-5 py-4 text-sm text-text-secondary"
              >
                <p className="text-[10px] uppercase tracking-[0.18em] text-text-muted">
                  {t("newCareer.savePreview.label")}
                </p>
                <p className="mt-2 text-base font-semibold text-text-primary">
                  {formData.playerName.trim() || t("newCareer.savePreview.newDriver")}
                </p>
                <p className="mt-1">
                  {hasGeneratedDraft
                    ? selectedCategory.name
                    : t("newCareer.savePreview.pendingHistory")}
                </p>
              </GlassCard>
            </div>

            {error ? (
              <div className="mt-6 flex-shrink-0 rounded-2xl border border-status-red/40 bg-status-red/10 px-4 py-3 text-sm text-status-red">
                {error}
              </div>
            ) : null}

            {/* -mx-6 px-6: expande a caixa de corte do scroll pros lados (mantendo o
                conteúdo alinhado) pra sombra/realce dos cards de canto não ser cortada.
                py-2 dá folga vertical; o overflow-y-auto segue rolando quando precisa. */}
            <div key={step} className="wizard-step-enter -mx-6 mt-8 min-h-0 flex-1 overflow-y-auto px-6 py-2">
              {renderStepContent()}
            </div>

            <div className="mt-8 flex flex-shrink-0 flex-col gap-3 border-t border-white/10 pt-6 sm:flex-row sm:items-center sm:justify-between">
              <GlassButton variant="secondary" onClick={handleBack}>
                {step === 1 ? t("newCareer.buttons.backToMenu") : t("newCareer.buttons.back")}
              </GlassButton>

              <div className="flex flex-col items-stretch gap-3 sm:flex-row">
                <GlassButton
                  variant="secondary"
                  onClick={handleResetWizard}
                >
                  {t("newCareer.buttons.reset")}
                </GlassButton>

                {step === 3 ? (
                  <GlassButton
                    variant="primary"
                    onClick={hasGeneratedDraft ? handleNext : handleGenerateDraft}
                  >
                    {hasGeneratedDraft
                      ? t("newCareer.buttons.chooseCategory")
                      : t("newCareer.buttons.generateHistory")}
                  </GlassButton>
                ) : step < 6 ? (
                  <GlassButton variant="primary" onClick={handleNext}>
                    {t("newCareer.buttons.next")}
                  </GlassButton>
                ) : (
                  <GlassButton variant="success" onClick={handleCreateCareer}>
                    {t("newCareer.buttons.finalize")}
                  </GlassButton>
                )}
              </div>
            </div>
          </div>
        </div>
      </div>

      <LoadingOverlay
        open={loading}
        title={
          step === 6
            ? t("newCareer.loading.finalizingTitle")
            : t("newCareer.loading.generatingTitle")
        }
        message={
          step === 6
            ? t(`newCareer.loadingMessages.msg${loadingMessageIndex}`)
            : draftState?.progress_year
              ? t("newCareer.loading.simulatingSeason", { year: draftState.progress_year })
              : t(`newCareer.loadingMessages.msg${loadingMessageIndex}`)
        }
      />
    </div>
  );
}

/// Traduz a identidade que veio do draft para os campos do formulário. Campo
/// ausente ou vazio fica de fora do patch, preservando o valor atual da tela.
function draftIdentityPatch(state) {
  const patch = {};
  const name = typeof state?.player_name === "string" ? state.player_name.trim() : "";
  if (name) patch.playerName = name;

  const nationality =
    typeof state?.player_nationality === "string" ? state.player_nationality.trim() : "";
  if (nationality && NATIONALITIES.some((item) => item.id === nationality)) {
    patch.nationality = nationality;
  }

  const age = Number(state?.player_age);
  if (Number.isFinite(age) && age > 0) patch.age = age;

  const difficulty = typeof state?.difficulty === "string" ? state.difficulty.trim() : "";
  if (difficulty && DIFFICULTIES.some((item) => item.id === difficulty)) {
    patch.difficulty = difficulty;
  }

  return patch;
}

function shouldDiscardDraftForPatch(currentForm, patch, draftState) {
  if (!draftState?.exists) return false;
  return Object.entries(patch).some(([key, value]) => {
    if (!WORLD_SHAPING_FIELDS.has(key)) return false;
    return String(currentForm[key] ?? "") !== String(value ?? "");
  });
}

export default NewCareer;
