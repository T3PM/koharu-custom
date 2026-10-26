import type {
  LanguageChoice,
  Model,
  ModelSelection,
  Provider,
  ProviderPreference,
} from '@koharu/bridge/protocol'

export function providerName(entries: ProviderPreference[], provider: Provider): string {
  return entries.find((entry) => entry.config.provider === provider)?.name ?? provider
}

export function modelKey(model: Model | ModelSelection): string {
  return `${model.provider}:${model.model ?? ''}`
}

export function modelSelection(model: Model, quantization: string | null): ModelSelection {
  return {
    provider: model.provider,
    model: model.model,
    quantization,
    vision: model.vision,
    reasoning: model.reasoning,
  }
}

export function orderedLanguageChoices(
  languages: readonly LanguageChoice[],
): Array<{ tag: string; name: string }> {
  return languages
    .map((language) => ({ tag: language.tag, name: language.name }))
    .sort((left, right) =>
      left.name.localeCompare(right.name, undefined, { numeric: true, sensitivity: 'base' }),
    )
}

export type TranslationProvider = 'google' | 'local' | 'openai' | 'claude' | 'gemini' | 'deepl' | 'openrouter'

export interface TranslationSettings {
  provider: TranslationProvider
  apiKey: string
  model: string
  customEndpoint: string
  targetLanguage: string
  systemPrompt: string
}

const STORAGE_KEY = 'koharu_translation_provider_settings_v1'

export const DEFAULT_TRANSLATION_SETTINGS: TranslationSettings = {
  provider: 'google',
  apiKey: '',
  model: '',
  customEndpoint: '',
  targetLanguage: 'en',
  systemPrompt:
    'You are a professional manga and comic translator. Translate the text accurately and naturally, preserving character voices, emotional tone, and sound effects. Output ONLY the translated text without notes or formatting.',
}

export function loadTranslationSettings(): TranslationSettings {
  if (typeof window === 'undefined') return DEFAULT_TRANSLATION_SETTINGS
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY)
    if (raw) {
      return { ...DEFAULT_TRANSLATION_SETTINGS, ...JSON.parse(raw) }
    }
  } catch {}
  return DEFAULT_TRANSLATION_SETTINGS
}

export function saveTranslationSettings(settings: TranslationSettings): void {
  if (typeof window === 'undefined') return
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(settings))
  } catch {}
}

export async function executeTranslation(
  sourceText: string,
  settings: TranslationSettings,
  targetLanguage?: string,
  glossaryEntries?: readonly { source: string; translation: string | null; enabled: boolean }[] | null,
): Promise<string> {
  const text = sourceText.trim()
  if (!text) return ''

  const target = (targetLanguage || settings.targetLanguage || 'en').split('-')[0]?.toLowerCase() || 'en'
  let prompt = settings.systemPrompt || DEFAULT_TRANSLATION_SETTINGS.systemPrompt

  const activeGlossary = (glossaryEntries || [])
    .filter((e) => e.enabled && e.translation?.trim() && e.source?.trim())
    .map((e) => `- ${e.source.trim()}: ${e.translation!.trim()}`)

  if (activeGlossary.length > 0) {
    prompt += `\n\nGlossary / Character names reference (strictly adhere to these translations):\n${activeGlossary.join('\n')}`
  }

  switch (settings.provider) {
    case 'google': {
      const endpoint = `https://translate.googleapis.com/translate_a/single?client=gtx&sl=auto&tl=${encodeURIComponent(target)}&dt=t&q=${encodeURIComponent(text)}`
      const res = await fetch(endpoint)
      if (!res.ok) throw new Error(`Google Translate error: ${res.statusText}`)
      const data = await res.json()
      if (Array.isArray(data) && Array.isArray(data[0])) {
        return data[0].map((s: any) => s[0] || '').join('')
      }
      return text
    }

    case 'openai':
    case 'openrouter': {
      const isOR = settings.provider === 'openrouter'
      const endpoint =
        settings.customEndpoint ||
        (isOR
          ? 'https://openrouter.ai/api/v1/chat/completions'
          : 'https://api.openai.com/v1/chat/completions')
      const model = settings.model || (isOR ? 'meta-llama/llama-3.3-70b-instruct' : 'gpt-4o-mini')

      const res = await fetch(endpoint, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${settings.apiKey.trim()}`,
        },
        body: JSON.stringify({
          model,
          messages: [
            { role: 'system', content: prompt },
            { role: 'user', content: `Translate the following text into ${target}:\n${text}` },
          ],
          temperature: 0.3,
        }),
      })
      if (!res.ok) {
        const err = await res.text()
        throw new Error(`LLM Error (${res.status}): ${err}`)
      }
      const data = await res.json()
      return data.choices?.[0]?.message?.content?.trim() || text
    }

    case 'gemini': {
      const model = settings.model || 'gemini-1.5-flash'
      const endpoint = `https://generativelanguage.googleapis.com/v1beta/models/${model}:generateContent?key=${settings.apiKey.trim()}`
      const res = await fetch(endpoint, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          system_instruction: { parts: [{ text: prompt }] },
          contents: [{ parts: [{ text: `Translate the following text into ${target}:\n${text}` }] }],
          generationConfig: { temperature: 0.3 },
        }),
      })
      if (!res.ok) {
        const err = await res.text()
        throw new Error(`Gemini Error (${res.status}): ${err}`)
      }
      const data = await res.json()
      return data.candidates?.[0]?.content?.parts?.[0]?.text?.trim() || text
    }

    case 'claude': {
      const model = settings.model || 'claude-3-5-haiku-latest'
      const endpoint = settings.customEndpoint || 'https://api.anthropic.com/v1/messages'
      const res = await fetch(endpoint, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'x-api-key': settings.apiKey.trim(),
          'anthropic-version': '2023-06-01',
          'anthropic-dangerous-direct-browser-access': 'true',
        },
        body: JSON.stringify({
          model,
          system: prompt,
          messages: [{ role: 'user', content: `Translate the following text into ${target}:\n${text}` }],
          max_tokens: 1024,
          temperature: 0.3,
        }),
      })
      if (!res.ok) {
        const err = await res.text()
        throw new Error(`Claude Error (${res.status}): ${err}`)
      }
      const data = await res.json()
      return data.content?.[0]?.text?.trim() || text
    }

    case 'deepl': {
      const key = settings.apiKey.trim()
      const endpoint = key.endsWith(':fx')
        ? 'https://api-free.deepl.com/v2/translate'
        : 'https://api.deepl.com/v2/translate'
      const body = new URLSearchParams()
      body.append('text', text)
      body.append('target_lang', target.toUpperCase())

      const res = await fetch(endpoint, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/x-www-form-urlencoded',
          Authorization: `DeepL-Auth-Key ${key}`,
        },
        body: body.toString(),
      })
      if (!res.ok) {
        const err = await res.text()
        throw new Error(`DeepL Error (${res.status}): ${err}`)
      }
      const data = await res.json()
      return data.translations?.[0]?.text || text
    }

    case 'local': {
      throw new Error('Local translation is processed through Koharu local engine.')
    }
  }
}
