import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useNavigate } from "react-router-dom";

import GlassButton from "../components/ui/GlassButton";
import GlassCard from "../components/ui/GlassCard";
import GlassInput from "../components/ui/GlassInput";
import GlassSelect from "../components/ui/GlassSelect";
import LoadingOverlay from "../components/ui/LoadingOverlay";
import CategoryCard from "../components/wizard/CategoryCard";
import DifficultyCard from "../components/wizard/DifficultyCard";
import StepIndicator from "../components/wizard/StepIndicator";
import TeamCard from "../components/wizard/TeamCard";
import useCareerStore from "../stores/useCareerStore";
import {
  DIFFICULTIES,
  LOADING_MESSAGE_INTERVAL_MS,
  LOADING_MESSAGES,
  NATIONALITIES,
  STARTING_CATEGORIES,
  WIZARD_STEPS,
} from "../utils/constants";

const STEP_TITLES = {
  1: "Escolha a dificuldade",
  2: "Dados do piloto",
  3: "Gerar passado histórico",
  4: "Escolha sua categoria",
  5: "Escolha sua equipe",
  6: "Confirmar dados",
};

const STEP_DESCRIPTIONS = {
  1: "Defina o teto da IA antes de entrar no paddock.",
  2: "Monte a identidade do seu piloto para o save inicial.",
  3: "Simule 2000 a 2024 antes de escolher onde entrar no grid de 2025.",
  4: "A sua jornada começa em uma das categorias rookies geradas pelo histórico.",
  5: "Selecione a equipe onde você vai estrear como segundo piloto.",
  6: "Confira tudo antes de transformar o rascunho histórico em save jogável.",
};

const INITIAL_FORM = {
  difficulty: "medio",
  playerName: "",
  nationality: "br",
  age: 20,
  category: "mazda_rookie",
  teamId: "",
};

function NewCareer() {
  const navigate = useNavigate();
  const loadCareer = useCareerStore((state) => state.loadCareer);
  const [step, setStep] = useState(1);
  const [formData, setFormData] = useState(INITIAL_FORM);
  const [loading, setLoading] = useState(false);
  const [loadingMessageIndex, setLoadingMessageIndex] = useState(0);
  const [error, setError] = useState("");
  const [draftState, setDraftState] = useState(null);

  useEffect(() => {
    let cancelled = false;

    async function loadDraft() {
      try {
        const state = await invoke("get_career_draft");
        if (cancelled || !state?.exists) return;
        applyDraftState(state, { resume: true });
      } catch {
        // Draft lookup is opportunistic; creation still works if no draft can be resumed.
      }
    }

    loadDraft();

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!loading) {
      setLoadingMessageIndex(0);
      return undefined;
    }

    const timer = window.setInterval(() => {
      setLoadingMessageIndex((current) => (current + 1) % LOADING_MESSAGES.length);
    }, LOADING_MESSAGE_INTERVAL_MS);

    return () => window.clearInterval(timer);
  }, [loading]);

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
    NATIONALITIES.find((nationality) => nationality.id === formData.nationality) ??
    NATIONALITIES[0];
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
    setFormData((current) => ({ ...current, ...patch }));
  }

  function validateCurrentStep() {
    if (step === 2) {
      const trimmedName = formData.playerName.trim();
      if (trimmedName.length < 2 || trimmedName.length > 50) {
        return "O nome do piloto precisa ter entre 2 e 50 caracteres.";
      }
      if (formData.age < 16 || formData.age > 30) {
        return "A idade inicial precisa ficar entre 16 e 30 anos.";
      }
    }

    if (step === 3 && !hasGeneratedDraft) {
      return "Gere o histórico antes de escolher categoria e equipe.";
    }

    if (step === 4 && !formData.category) {
      return "Selecione uma categoria inicial.";
    }

    if (step === 5 && !availableTeams.some((team) => team.id === formData.teamId)) {
      return "Selecione uma equipe válida.";
    }

    return "";
  }

  function handleNext() {
    const validationError = validateCurrentStep();
    if (validationError) {
      setError(validationError);
      return;
    }

    setError("");
    setStep((current) => Math.min(current + 1, 6));
  }

  function handleBack() {
    setError("");
    if (step === 1) {
      navigate("/menu");
      return;
    }
    setStep((current) => Math.max(current - 1, 1));
  }

  async function handleGenerateDraft() {
    setError("");
    setLoading(true);

    try {
      const state = await invoke("create_historical_career_draft", {
        input: {
          player_name: formData.playerName.trim(),
          player_nationality: formData.nationality,
          player_age: Number(formData.age),
          difficulty: formData.difficulty,
        },
      });

      applyDraftState(state);
      setStep(4);
    } catch (invokeError) {
      setError(
        typeof invokeError === "string"
          ? invokeError
          : "Não foi possível gerar o histórico. Tente novamente.",
      );
    } finally {
      setLoading(false);
    }
  }

  async function handleCreateCareer() {
    setError("");
    if (!draftState?.career_id) {
      setError("Gere o histórico antes de finalizar a carreira.");
      return;
    }

    setLoading(true);

    try {
      const result = await invoke("finalize_career_draft", {
        input: {
          career_id: draftState.career_id,
          category: formData.category,
          team_id: formData.teamId,
        },
      });

      await loadCareer(result.career_id);
      navigate("/dashboard");
    } catch (invokeError) {
      setError(
        typeof invokeError === "string"
          ? invokeError
          : "Não foi possível finalizar a carreira. Tente novamente.",
      );
    } finally {
      setLoading(false);
    }
  }

  async function handleResetWizard() {
    setError("");
    if (draftState?.exists) {
      try {
        await invoke("discard_career_draft");
      } catch {
        // If discard fails, keep the UI reset local and let the next get/create surface errors.
      }
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
                Nome do piloto
              </p>
              <GlassInput
                value={formData.playerName}
                onChange={(event) => updateForm({ playerName: event.target.value })}
                maxLength={50}
                placeholder="João Silva"
              />
            </div>

            <div>
              <p className="mb-2 text-[11px] uppercase tracking-[0.22em] text-text-secondary">
                Nacionalidade
              </p>
              <GlassSelect
                value={formData.nationality}
                onChange={(event) => updateForm({ nationality: event.target.value })}
              >
                {NATIONALITIES.map((nationality) => (
                  <option key={nationality.id} value={nationality.id}>
                    {nationality.label}
                  </option>
                ))}
              </GlassSelect>
            </div>

            <div>
              <p className="mb-2 text-[11px] uppercase tracking-[0.22em] text-text-secondary">
                Idade
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
              Preview do piloto
            </p>
            <h3 className="mt-4 text-3xl font-semibold text-text-primary">
              {formData.playerName.trim() || "Seu piloto"}
            </h3>
            <p className="mt-3 text-sm text-text-secondary">
              {selectedNationality.label} - {formData.age} anos
            </p>
            <div className="mt-8 space-y-4 text-sm text-text-secondary">
              <p>Todos os atributos iniciais começam equilibrados em 50.</p>
              <p>Voc? entra como N2 e cresce a partir dos resultados da carreira.</p>
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
              Rascunho histórico
            </p>
            <h3 className="mt-4 text-3xl font-semibold text-text-primary">
              25 temporadas antes da estreia
            </h3>
            <p className="mt-4 max-w-3xl text-sm leading-7 text-text-secondary">
              O jogo vai simular de 2000 até 2024 sem o jogador no grid, guardando resultados,
              carreiras, trocas de equipe e evolução do mundo. Ao terminar, você entra em 2025
              substituindo o N2 da equipe escolhida.
            </p>

            <div className="mt-8 grid gap-3 sm:grid-cols-3">
              <div className="glass-light rounded-2xl p-4">
                <p className="text-[10px] uppercase tracking-[0.18em] text-text-muted">
                  Início
                </p>
                <p className="mt-2 text-2xl font-semibold text-text-primary">2000</p>
              </div>
              <div className="glass-light rounded-2xl p-4">
                <p className="text-[10px] uppercase tracking-[0.18em] text-text-muted">
                  Jogável
                </p>
                <p className="mt-2 text-2xl font-semibold text-text-primary">2025</p>
              </div>
              <div className="glass-light rounded-2xl p-4">
                <p className="text-[10px] uppercase tracking-[0.18em] text-text-muted">
                  Status
                </p>
                <p className="mt-2 text-lg font-semibold text-text-primary">
                  {hasGeneratedDraft ? "Gerado" : "Pendente"}
                </p>
              </div>
            </div>
          </GlassCard>

          <GlassCard hover={false} className="glass-light rounded-[28px]">
            <p className="text-[11px] uppercase tracking-[0.22em] text-text-secondary">
              Piloto pendente
            </p>
            <h3 className="mt-4 text-2xl font-semibold text-text-primary">
              {formData.playerName.trim() || "Seu piloto"}
            </h3>
            <div className="mt-5 space-y-4 text-sm text-text-secondary">
              <div>
                <p className="text-text-muted">Nacionalidade</p>
                <p className="mt-1 text-text-primary">{selectedNationality.label}</p>
              </div>
              <div>
                <p className="text-text-muted">Dificuldade</p>
                <p className="mt-1 text-text-primary">{selectedDifficulty.name}</p>
              </div>
              <div>
                <p className="text-text-muted">Progresso</p>
                <p className="mt-1 text-text-primary">
                  {draftState?.progress_year ? `Ano ${draftState.progress_year}` : "Não iniciado"}
                </p>
              </div>
            </div>
          </GlassCard>
        </div>
      );
    }

    if (step === 4) {
      return (
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
      <div className="grid gap-6 xl:grid-cols-[1fr_0.42fr]">
        <GlassCard hover={false} className="glass-light rounded-[28px]">
          <div className="space-y-6">
            <div>
              <p className="text-[11px] uppercase tracking-[0.22em] text-text-secondary">
                Piloto
              </p>
              <h3 className="mt-3 text-3xl font-semibold text-text-primary">
                {formData.playerName.trim() || "Seu piloto"}
              </h3>
              <p className="mt-2 text-sm text-text-secondary">
                {selectedNationality.label} - {formData.age} anos
              </p>
            </div>

            <div className="grid gap-4 md:grid-cols-3">
              <div className="glass-light rounded-2xl p-4">
                <p className="text-[10px] uppercase tracking-[0.18em] text-text-muted">
                  Categoria
                </p>
                <p className="mt-2 text-base font-semibold text-text-primary">
                  {selectedCategory.name}
                </p>
              </div>
              <div className="glass-light rounded-2xl p-4">
                <p className="text-[10px] uppercase tracking-[0.18em] text-text-muted">
                  Equipe
                </p>
                <p className="mt-2 text-base font-semibold text-text-primary">
                  {selectedTeam?.nome}
                </p>
              </div>
              <div className="glass-light rounded-2xl p-4">
                <p className="text-[10px] uppercase tracking-[0.18em] text-text-muted">
                  Dificuldade
                </p>
                <p className="mt-2 text-base font-semibold text-text-primary">
                  {selectedDifficulty.name}
                </p>
              </div>
            </div>

            <div className="rounded-2xl border border-status-yellow/30 bg-status-yellow/10 px-4 py-4 text-sm text-text-secondary">
              Esta ação ativa o save em 2025, insere o jogador como N2 e mantém o histórico
              simulado salvo para futuras telas de legado.
            </div>
          </div>
        </GlassCard>

        <GlassCard hover={false} className="glass-light rounded-[28px]">
          <p className="text-[11px] uppercase tracking-[0.22em] text-text-secondary">
            Resumo rápido
          </p>
          <div className="mt-5 space-y-5 text-sm text-text-secondary">
            <div>
              <p className="text-text-muted">Carro</p>
              <p className="mt-1 text-text-primary">{selectedCategory.car}</p>
            </div>
            <div>
              <p className="text-text-muted">Equipe escolhida</p>
              <p className="mt-1 text-text-primary">{selectedTeam?.nome_curto}</p>
            </div>
            <div>
              <p className="text-text-muted">Perfil da IA</p>
              <p className="mt-1 text-text-primary">{selectedDifficulty.desc}</p>
            </div>
          </div>
        </GlassCard>
      </div>
    );
  }

  return (
    <div className="app-shell px-4 py-6 text-text-primary sm:px-6 lg:px-10">
      <div className="app-backdrop" />

      <div className="relative mx-auto flex min-h-[calc(100vh-3rem)] max-w-7xl items-center justify-center">
        <div className="wizard-panel glass w-full overflow-hidden rounded-[32px] p-5 shadow-[0_30px_80px_rgba(0,0,0,0.42)] sm:p-8 lg:p-10">
          <div className="relative z-10">
            <StepIndicator currentStep={step} steps={WIZARD_STEPS} />

            <div className="mt-8 flex flex-col gap-6 xl:flex-row xl:items-end xl:justify-between">
              <div>
                <p className="text-[11px] uppercase tracking-[0.24em] text-accent-primary">
                  {STEP_TITLES[step]}
                </p>
                <h1 className="mt-3 text-4xl font-semibold tracking-[-0.04em] text-text-primary sm:text-5xl">
                  Monte a sua estreia.
                </h1>
                <p className="mt-4 max-w-2xl text-sm leading-7 text-text-secondary sm:text-base">
                  {STEP_DESCRIPTIONS[step]}
                </p>
              </div>

              <GlassCard
                hover={false}
                className="glass-light w-full max-w-xs rounded-3xl px-5 py-4 text-sm text-text-secondary"
              >
                <p className="text-[10px] uppercase tracking-[0.18em] text-text-muted">
                  Save preview
                </p>
                <p className="mt-2 text-base font-semibold text-text-primary">
                  {formData.playerName.trim() || "Piloto novo"}
                </p>
                <p className="mt-1">
                  {hasGeneratedDraft ? selectedCategory.name : "Histórico pendente"}
                </p>
              </GlassCard>
            </div>

            {error ? (
              <div className="mt-6 rounded-2xl border border-status-red/40 bg-status-red/10 px-4 py-3 text-sm text-status-red">
                {error}
              </div>
            ) : null}

            <div key={step} className="wizard-step-enter mt-8">
              {renderStepContent()}
            </div>

            <div className="mt-8 flex flex-col gap-3 border-t border-white/10 pt-6 sm:flex-row sm:items-center sm:justify-between">
              <GlassButton variant="secondary" onClick={handleBack}>
                {step === 1 ? "Voltar ao menu" : "Voltar"}
              </GlassButton>

              <div className="flex flex-col items-stretch gap-3 sm:flex-row">
                <GlassButton
                  variant="secondary"
                  onClick={handleResetWizard}
                >
                  Reiniciar
                </GlassButton>

                {step === 3 ? (
                  <GlassButton
                    variant="primary"
                    onClick={hasGeneratedDraft ? handleNext : handleGenerateDraft}
                  >
                    {hasGeneratedDraft ? "Escolher categoria" : "Gerar histórico"}
                  </GlassButton>
                ) : step < 6 ? (
                  <GlassButton variant="primary" onClick={handleNext}>
                    Próximo
                  </GlassButton>
                ) : (
                  <GlassButton variant="success" onClick={handleCreateCareer}>
                    Finalizar carreira
                  </GlassButton>
                )}
              </div>
            </div>
          </div>
        </div>
      </div>

      <LoadingOverlay
        open={loading}
        title={step === 6 ? "Finalizando carreira" : "Gerando histórico"}
        message={LOADING_MESSAGES[loadingMessageIndex]}
      />
    </div>
  );
}

export default NewCareer;
