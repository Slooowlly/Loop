function StepIndicator({ currentStep, steps }) {
  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between text-[11px] uppercase tracking-[0.24em] text-text-secondary">
        <span>Nova carreira</span>
        <span>
          Step {currentStep} de {steps.length}
        </span>
      </div>

      <div className="grid grid-cols-5 gap-3">
        {steps.map((step, index) => {
          const stepNumber = index + 1;
          const isActive = stepNumber === currentStep;
          const isDone = stepNumber < currentStep;

          return (
            <div key={step} className="space-y-2">
              <div className="h-1.5 overflow-hidden rounded-full bg-white/8">
                <div
                  className={[
                    "h-full rounded-full transition-glass",
                    isActive || isDone ? "bg-accent-primary" : "bg-white/10",
                  ].join(" ")}
                  style={{ width: isDone ? "100%" : isActive ? "72%" : "24%" }}
                />
              </div>
              <p
                className={[
                  "truncate text-[10px] uppercase tracking-[0.18em]",
                  isActive ? "text-text-primary" : "text-text-secondary",
                ].join(" ")}
                title={step}
              >
                {step}
              </p>
            </div>
          );
        })}
      </div>
    </div>
  );
}

export default StepIndicator;
