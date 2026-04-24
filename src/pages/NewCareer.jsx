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
  LOADING_MESSAGES,
  NATIONALITIES,
  STARTING_CATEGORIES,
  TEAM_PREVIEWS,
  WIZARD_STEPS,
} from "../utils/constants";

const STEP_TITLES = {
  1: "Escolha a dificuldade",
  2: "Dados do piloto",
  3: "Escolha sua categoria",
  4: "Escolha sua equipe",
  5: "Confirmar dados",
};

const STEP_DESCRIPTIONS = {
  1: "Defina o teto da IA antes de entrar no paddock.",
  2: "Monte a identidade do seu piloto para o save inicial.",
  3: "A sua jornada começa em uma das duas rookies.",
  4: "Selecione a equipe onde voc? vai estrear como segundo piloto.",
  5: "Confira tudo antes de criar o mundo completo da carreira.",
};

const INITIAL_FORM = {
  difficulty: "medio",
  playerName: "",
  nationality: "br",
  age: 20,
  category: "mazda_rookie",
  teamIndex: 0,
};

function NewCareer() {
  const navigate = useNavigate();
  const loadCareer = useCareerStore((state) => state.loadCareer);
  const [step, setStep] = useState(1);
  const [formData, setFormData] = useState(INITIAL_FORM);
  const [loading, setLoading] = useState(false);
  const [loadingMessageIndex, setLoadingMessageIndex] = useState(0);
  const [error, setError] = useState("");

  useEffect(() => {
    if (!loading) {
      setLoadingMessageIndex(0);
      return undefined;
    }

    const timer = window.setInterval(() => {
      setLoadingMessageIndex((current) => (current + 1) % LOADING_MESSAGES.length);
    }, 900);

    return () => window.clearInterval(timer);
  }, [loading]);

  const selectedCategory =
    STARTING_CATEGORIES.find((category) => category.id === formData.category) ??
    STARTING_CATEGORIES[0];
  const availableTeams = TEAM_PREVIEWS[formData.category] ?? [];
  const selectedTeam =
    availableTeams.find((team) => team.index === formData.teamIndex) ?? availableTeams[0];
  const selectedDifficulty =
    DIFFICULTIES.find((difficulty) => difficulty.id === formData.difficulty) ?? DIFFICULTIES[1];
  const selectedNationality =
    NATIONALITIES.find((nationality) => nationality.id === formData.nationality) ??
    NATIONALITIES[0];

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

    if (step === 3 && !formData.category) {
      return "Selecione uma categoria inicial.";
    }

    if (step === 4 && !availableTeams.some((team) => team.index === formData.teamIndex)) {
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
    setStep((current) => Math.min(current + 1, 5));
  }

  function handleBack() {
    setError("");
    if (step === 1) {
      navigate("/menu");
      return;
    }
    setStep((current) => Math.max(current - 1, 1));
  }

  async function handleCreateCareer() {
    setError("");
    setLoading(true);

    try {
      const result = await invoke("create_career", {
        input: {
          player_name: formData.playerName.trim(),
          player_nationality: formData.nationality,
          player_age: Number(formData.age),
          category: formData.category,
          team_index: formData.teamIndex,
          difficulty: formData.difficulty,
        },
      });

      await loadCareer(result.career_id);
      navigate("/dashboard");
    } catch (invokeError) {
      setError(
        typeof invokeError === "string"
          ? invokeError
          : "Não foi possível criar a carreira. Tente novamente.",
      );
    } finally {
      setLoading(false);
    }
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
        <div className="grid gap-5 lg:grid-cols-2">
          {STARTING_CATEGORIES.map((category) => (
            <CategoryCard
              key={category.id}
              category={category}
              selected={formData.category === category.id}
              onSelect={(categoryId) =>
                updateForm({
                  category: categoryId,
                  teamIndex: 0,
                })
              }
            />
          ))}
        </div>
      );
    }

    if (step === 4) {
      return (
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
          {availableTeams.map((team) => (
            <TeamCard
              key={`${formData.category}-${team.index}`}
              team={team}
              selected={formData.teamIndex === team.index}
              onSelect={(teamIndex) => updateForm({ teamIndex })}
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
                  {selectedTeam?.name}
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
              Esta ação criar? um novo save completo com 196 pilotos, 71 equipes e o calendário
              inicial da carreira.
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
              <p className="mt-1 text-text-primary">{selectedTeam?.country}</p>
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
                <p className="mt-1">{selectedCategory.name}</p>
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
                  onClick={() => {
                    setError("");
                    setStep(1);
                    setFormData(INITIAL_FORM);
                  }}
                >
                  Reiniciar
                </GlassButton>

                {step < 5 ? (
                  <GlassButton variant="primary" onClick={handleNext}>
                    Próximo
                  </GlassButton>
                ) : (
                  <GlassButton variant="success" onClick={handleCreateCareer}>
                    Criar carreira
                  </GlassButton>
                )}
              </div>
            </div>
          </div>
        </div>
      </div>

      <LoadingOverlay
        open={loading}
        title="Criando carreira"
        message={LOADING_MESSAGES[loadingMessageIndex]}
      />
    </div>
  );
}

export default NewCareer;
