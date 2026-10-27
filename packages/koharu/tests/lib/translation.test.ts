import { describe, expect, it, vi, afterEach } from 'vitest'

import {
  DEFAULT_TRANSLATION_SETTINGS,
  executeTranslation,
  loadTranslationSettings,
  saveTranslationSettings,
  type TranslationSettings,
} from '@/lib/translation'

describe('translation library and glossary integration', () => {
  const originalFetch = globalThis.fetch

  afterEach(() => {
    globalThis.fetch = originalFetch
  })

  it('loads and saves translation settings', () => {
    const initial = loadTranslationSettings()
    expect(initial.provider).toBe('google')

    const updated: TranslationSettings = {
      ...initial,
      provider: 'openai',
      apiKey: 'test-key',
      targetLanguage: 'th',
    }
    saveTranslationSettings(updated)
    expect(loadTranslationSettings()).toEqual(updated)
  })

  it('substitutes glossary tokens before calling Google Translate and restores them', async () => {
    const glossaryEntries = [
      { source: '山田太郎', translation: 'Taro Yamada', enabled: true },
      { source: '春香', translation: 'Haruka', enabled: true },
    ]

    globalThis.fetch = vi.fn().mockImplementation(async (url: string) => {
      expect(url).toContain(encodeURIComponent('__GLOSS_0__'))
      expect(url).toContain(encodeURIComponent('__GLOSS_1__'))
      return {
        ok: true,
        json: async () => [
          [['__GLOSS_0__ and __GLOSS_1__ are here.']],
        ],
      } as any
    })

    const settings: TranslationSettings = {
      ...DEFAULT_TRANSLATION_SETTINGS,
      provider: 'google',
      targetLanguage: 'en',
    }

    const result = await executeTranslation(
      '山田太郎と春香がいる。',
      settings,
      'en',
      glossaryEntries,
      {
        enabled: true,
        preSubstitute: true,
        postReplace: true,
        caseSensitive: false,
        wholeWord: false,
        enablePersonas: true,
        enableSpeakerTagging: true,
        autoGuessSpeakers: true,
      },
    )

    expect(result).toBe('Taro Yamada and Haruka are here.')
  })

  it('falls back to raw text if translation service fails or returns empty', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue({
      ok: false,
      statusText: 'Internal Server Error',
    } as any)

    const settings: TranslationSettings = {
      ...DEFAULT_TRANSLATION_SETTINGS,
      provider: 'google',
    }

    await expect(executeTranslation('Hello', settings)).rejects.toThrow('Google Translate error')
  })
})
