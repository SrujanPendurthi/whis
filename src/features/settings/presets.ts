import type { ModelProfile } from "../../shared/types";

/**
 * Template profiles to seed the editor from. Every provider is treated as
 * OpenAI-compatible (base URL + optional bearer key), so Anthropic here points
 * at its OpenAI-compatible endpoint. `id`/`apiKey` are filled in when saved.
 */
export interface Preset {
  name: string;
  profile: Omit<ModelProfile, "id">;
}

export const PRESETS: Preset[] = [
  {
    name: "Local (Higgs / LM Studio)",
    profile: {
      label: "Local",
      baseUrl: "http://127.0.0.1:8080/v1",
      model: "local",
      apiKey: "",
      temperature: 0.7,
    },
  },
  {
    name: "Ollama (local)",
    profile: {
      label: "Ollama",
      baseUrl: "http://127.0.0.1:11434/v1",
      model: "llama3.1",
      apiKey: "",
      temperature: 0.7,
    },
  },
  {
    name: "OpenAI",
    profile: {
      label: "GPT-4o",
      baseUrl: "https://api.openai.com/v1",
      model: "gpt-4o",
      apiKey: "",
      temperature: 0.7,
    },
  },
  {
    name: "Anthropic (OpenAI-compatible)",
    profile: {
      label: "Claude",
      baseUrl: "https://api.anthropic.com/v1",
      model: "claude-sonnet-4-5",
      apiKey: "",
      temperature: 0.7,
    },
  },
  {
    name: "OpenRouter",
    profile: {
      label: "OpenRouter",
      baseUrl: "https://openrouter.ai/api/v1",
      model: "openai/gpt-4o",
      apiKey: "",
      temperature: 0.7,
    },
  },
  {
    name: "Groq",
    profile: {
      label: "Groq",
      baseUrl: "https://api.groq.com/openai/v1",
      model: "llama-3.3-70b-versatile",
      apiKey: "",
      temperature: 0.7,
    },
  },
];

/** A blank profile (Custom) for a from-scratch entry. */
export const BLANK_PROFILE: Omit<ModelProfile, "id"> = {
  label: "",
  baseUrl: "",
  model: "",
  apiKey: "",
  temperature: 0.7,
};
