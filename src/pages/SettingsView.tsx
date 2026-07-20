import { useEffect, useState } from "react";
import { ipc } from "../lib/ipc";
import { Key, CheckCircle, AlertCircle, Trash2 } from "lucide-react";

interface ProviderConfig {
  id: string;
  label: string;
  placeholder: string;
  docsUrl: string;
}

const PROVIDERS: ProviderConfig[] = [
  {
    id: "openai",
    label: "OpenAI",
    placeholder: "sk-...",
    docsUrl: "https://platform.openai.com/api-keys",
  },
  {
    id: "anthropic",
    label: "Anthropic",
    placeholder: "sk-ant-...",
    docsUrl: "https://console.anthropic.com/settings/keys",
  },
  {
    id: "gemini",
    label: "Google Gemini",
    placeholder: "AIza...",
    docsUrl: "https://aistudio.google.com/app/apikey",
  },
  {
    id: "openai_compat",
    label: "OpenAI-compatible (LM Studio, vLLM…)",
    placeholder: "Optionnel — laisser vide si le serveur local ne demande pas de clé",
    docsUrl: "",
  },
];

export function SettingsView() {
  const [configured, setConfigured] = useState<Set<string>>(new Set());
  const [keys, setKeys] = useState<Record<string, string>>({});
  const [saving, setSaving] = useState<string | null>(null);
  const [feedback, setFeedback] = useState<Record<string, { ok: boolean; msg: string }>>({});

  const loadStatus = async () => {
    const list = await ipc.providerListConfigured();
    setConfigured(new Set(list));
  };

  useEffect(() => {
    void loadStatus();
  }, []);

  const handleSave = async (providerId: string) => {
    const key = keys[providerId] ?? "";
    setSaving(providerId);
    setFeedback((f) => ({ ...f, [providerId]: { ok: false, msg: "" } }));
    try {
      if (key.trim()) {
        await ipc.providerSetApiKey({ provider: providerId, apiKey: key.trim() });
        setFeedback((f) => ({ ...f, [providerId]: { ok: true, msg: "Clé enregistrée." } }));
      } else {
        await ipc.providerDeleteApiKey({ provider: providerId });
        setFeedback((f) => ({ ...f, [providerId]: { ok: true, msg: "Clé supprimée." } }));
      }
      await loadStatus();
    } catch (e) {
      setFeedback((f) => ({ ...f, [providerId]: { ok: false, msg: String(e) } }));
    } finally {
      setSaving(null);
    }
  };

  const handleDelete = async (providerId: string) => {
    setSaving(providerId);
    try {
      await ipc.providerDeleteApiKey({ provider: providerId });
      setKeys((k) => ({ ...k, [providerId]: "" }));
      setFeedback((f) => ({ ...f, [providerId]: { ok: true, msg: "Clé supprimée." } }));
      await loadStatus();
    } catch (e) {
      setFeedback((f) => ({ ...f, [providerId]: { ok: false, msg: String(e) } }));
    } finally {
      setSaving(null);
    }
  };

  return (
    <div className="flex flex-1 flex-col overflow-y-auto p-8">
      <div className="mx-auto w-full max-w-lg space-y-8">
        <div className="flex items-center gap-3">
          <Key size={24} className="text-accent" />
          <h2 className="text-lg font-semibold text-fg">Clés API</h2>
        </div>

        <p className="text-sm text-muted">
          Les clés API sont stockées dans le keyring de votre OS (DPAPI / Keychain / Secret Service)
          et ne quittent jamais votre machine. Elles sont nécessaires uniquement pour les providers
          distants (OpenAI, Anthropic, Gemini).
        </p>

        <div className="space-y-6">
          {PROVIDERS.map((p) => {
            const isConfigured = configured.has(p.id);
            const fb = feedback[p.id];
            return (
              <div key={p.id} className="space-y-2">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <span className="text-sm font-medium text-fg">{p.label}</span>
                    {isConfigured && (
                      <span className="rounded bg-green-900/30 px-1.5 py-0.5 text-[10px] text-green-400">
                        configuré
                      </span>
                    )}
                  </div>
                  {p.docsUrl && (
                    <a
                      href={p.docsUrl}
                      target="_blank"
                      rel="noreferrer"
                      className="text-xs text-accent hover:underline"
                    >
                      obtenir une clé →
                    </a>
                  )}
                </div>

                <div className="flex gap-2">
                  <input
                    type="password"
                    placeholder={isConfigured ? "•••••••• (enregistrée)" : p.placeholder}
                    value={keys[p.id] ?? ""}
                    onChange={(e) => setKeys((k) => ({ ...k, [p.id]: e.target.value }))}
                    className="flex-1 rounded border border-border bg-bg px-3 py-2 text-sm text-fg placeholder:text-muted/60 focus:border-accent focus:outline-none"
                  />
                  <button
                    onClick={() => void handleSave(p.id)}
                    disabled={saving === p.id}
                    className="rounded bg-accent px-3 py-2 text-sm font-medium text-bg disabled:opacity-50 hover:opacity-90"
                  >
                    {saving === p.id ? "…" : "Enregistrer"}
                  </button>
                  {isConfigured && (
                    <button
                      onClick={() => void handleDelete(p.id)}
                      disabled={saving === p.id}
                      className="rounded border border-border px-2 py-2 text-muted hover:border-red-600/50 hover:text-red-400 disabled:opacity-50"
                      title="Supprimer la clé"
                    >
                      <Trash2 size={14} />
                    </button>
                  )}
                </div>

                {fb && fb.msg && (
                  <div
                    className={`flex items-center gap-1.5 text-xs ${
                      fb.ok ? "text-green-400" : "text-red-400"
                    }`}
                  >
                    {fb.ok ? <CheckCircle size={12} /> : <AlertCircle size={12} />}
                    {fb.msg}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
