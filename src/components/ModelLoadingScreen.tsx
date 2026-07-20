import { useEffect, useState } from "react";
import { Loader2, HardDrive, Cpu } from "lucide-react";

interface ModelLoadingScreenProps {
  modelId: string;
  modelName?: string;
  progress: number; // 0-100
  onCancel: () => void;
  onSkip?: () => void; // Permettre de passer à la session sans envoyer de message
}

export function ModelLoadingScreen({ modelId, modelName, progress, onCancel, onSkip }: ModelLoadingScreenProps) {
  const [dots, setDots] = useState(".");

  // Animation des points
  useEffect(() => {
    const interval = setInterval(() => {
      setDots(prev => prev.length >= 3 ? "." : prev + ".");
    }, 500);
    return () => clearInterval(interval);
  }, []);

  const displayName = modelName || modelId;
  const isIndeterminate = progress === 0;

  return (
    <div className="flex h-full items-center justify-center p-6 bg-bg">
      <div className="w-full max-w-md rounded border border-border bg-surface p-6 shadow-lg">
        {/* Header */}
        <div className="mb-6 text-center">
          <div className="mb-2 flex justify-center">
            <HardDrive size={32} className="text-accent" />
          </div>
          <h2 className="text-lg font-semibold text-fg">Chargement du modèle</h2>
          <p className="text-sm text-muted">{displayName}</p>
        </div>

        {/* Progress */}
        <div className="mb-6">
          <div className="mb-2 flex items-center justify-between text-sm">
            <span className="text-muted">Initialisation{dots}</span>
            {!isIndeterminate && (
              <span className="font-mono text-fg">{Math.round(progress)}%</span>
            )}
          </div>
          
          <div className="h-2 w-full overflow-hidden rounded-full bg-border">
            <div 
              className={`h-full transition-all duration-300 ${
                isIndeterminate 
                  ? "w-1/3 animate-pulse bg-accent/50" 
                  : "bg-accent"
              }`}
              style={isIndeterminate ? {} : { width: `${Math.max(progress, 2)}%` }}
            />
          </div>
        </div>

        {/* Info */}
        <div className="mb-6 space-y-2 text-xs text-muted">
          <div className="flex items-center gap-2">
            <Cpu size={14} />
            <span>Chargement des poids du modèle en mémoire...</span>
          </div>
          <div className="flex items-center gap-2">
            <Loader2 size={14} className="animate-spin" />
            <span>Ceci peut prendre quelques minutes selon la taille du modèle</span>
          </div>
        </div>

        {/* Action buttons */}
        <div className="flex gap-2">
          <button
            onClick={onCancel}
            className="flex-1 rounded border border-border bg-bg px-4 py-2 text-sm text-muted hover:bg-border/40 hover:text-fg transition-colors"
          >
            Annuler
          </button>
          {onSkip && (
            <button
              onClick={onSkip}
              className="flex-1 rounded bg-accent px-4 py-2 text-sm text-white hover:bg-accent/80 transition-colors"
            >
              Continuer
            </button>
          )}
        </div>
      </div>
    </div>
  );
}