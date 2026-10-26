'use client'

import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Check, Eye, EyeOff, Globe, SlidersHorizontal } from 'lucide-react'

import { Button } from '@koharu/ui/components/button'
import { Input } from '@koharu/ui/components/input'
import { NativeSelect } from '@koharu/ui/components/native-select'
import { Popover, PopoverContent, PopoverTrigger } from '@koharu/ui/components/popover'
import type { TranslationProvider, TranslationSettings } from '@/lib/translation'
import { loadTranslationSettings, modelSelection, saveTranslationSettings } from '@/lib/translation'
import { receivePreferences, useKoharuStore } from '@/lib/store'
import { savePreferences } from '@/lib/backend'

interface TranslationSettingsPopoverProps {
  compact?: boolean
  onSettingsChange?: (settings: TranslationSettings) => void
}

export function TranslationSettingsPopover({ compact = false, onSettingsChange }: TranslationSettingsPopoverProps) {
  const { t } = useTranslation()
  const [settings, setSettings] = useState<TranslationSettings>(() => loadTranslationSettings())
  const [showKey, setShowKey] = useState(false)
  const [open, setOpen] = useState(false)
  const preferences = useKoharuStore((state) => state.preferences)
  const translationModels = useKoharuStore((state) => state.translationModels)
  const localModels = translationModels.filter((m) => m.provider === 'local')

  const updateSetting = <K extends keyof TranslationSettings>(key: K, value: TranslationSettings[K]) => {
    const updated = { ...settings, [key]: value }
    setSettings(updated)
    saveTranslationSettings(updated)
    onSettingsChange?.(updated)
  }

  const isCloud = settings.provider !== 'google' && settings.provider !== 'local'

  const providerDisplayLabel =
    settings.provider === 'google'
      ? 'Google'
      : settings.provider === 'local'
        ? 'Local'
        : settings.provider

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger
        render={
          compact ? (
            <button
              type='button'
              className='grid size-5 place-items-center rounded-sm text-muted-foreground hover:bg-foreground/[0.07] hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/25'
              title={t('translation.settingsTooltip', `Translation Settings (${providerDisplayLabel})`)}
              onClick={(e) => e.stopPropagation()}
            >
              <SlidersHorizontal className='size-3' />
            </button>
          ) : (
            <Button
              type='button'
              variant='ghost'
              size='xs'
              className='h-6 gap-1 px-1.5 text-[10px] text-muted-foreground hover:text-foreground'
              title={t('translation.settingsTooltip', 'Translation Settings & Provider')}
              onClick={(e) => e.stopPropagation()}
            >
              <SlidersHorizontal className='size-3 text-muted-foreground' />
              <span className='capitalize font-medium text-[10px]'>
                {providerDisplayLabel}
              </span>
            </Button>
          )
        }
      />
      <PopoverContent align='end' className='w-80 p-3 space-y-3 text-xs'>
        <div className='flex items-center justify-between border-b pb-2'>
          <div className='flex items-center gap-1.5 font-semibold text-foreground'>
            <Globe className='size-3.5 text-primary' />
            <span>{t('translation.settingsTitle', 'Translation Settings')}</span>
          </div>
          <span className='rounded bg-muted px-1.5 py-0.5 text-[9px] font-medium text-muted-foreground uppercase'>
            {settings.provider}
          </span>
        </div>

        <div className='space-y-1.5'>
          <label className='text-[10px] font-medium text-muted-foreground uppercase'>
            {t('translation.provider', 'Translation Provider')}
          </label>
          <NativeSelect
            value={settings.provider}
            className='h-7 text-xs'
            onChange={(e) => updateSetting('provider', e.currentTarget.value as TranslationProvider)}
          >
            <option value='google'>Google Translate (Free, Web)</option>
            <option value='local'>Koharu Local Model (Offline, GGUF)</option>
            <option value='gemini'>Google Gemini (Flash)</option>
            <option value='openai'>OpenAI (GPT-4o / GPT-4o-mini)</option>
            <option value='claude'>Claude (Anthropic Haiku / Sonnet)</option>
            <option value='deepl'>DeepL (API Free / Pro)</option>
            <option value='openrouter'>OpenRouter (Llama 3 / Mistral / etc.)</option>
          </NativeSelect>
        </div>

        {settings.provider === 'local' && (
          <div className='space-y-1.5'>
            <label className='text-[10px] font-medium text-muted-foreground uppercase'>
              {t('translation.localModel', 'Koharu Local Model')}
            </label>
            <NativeSelect
              value={settings.model || preferences?.pipeline?.translation?.model?.model || ''}
              className='h-7 text-xs'
              onChange={(e) => {
                const selectedModelName = e.currentTarget.value
                updateSetting('model', selectedModelName)
                const targetModel = localModels.find((m) => m.model === selectedModelName)
                if (targetModel && preferences) {
                  const pipeline = {
                    ...preferences.pipeline,
                    translation: {
                      ...preferences.pipeline.translation,
                      model: modelSelection(targetModel, targetModel.quantizations[0]?.id ?? null),
                    },
                  }
                  void savePreferences(pipeline, preferences.providers, preferences.typesetting).then((saved) => {
                    receivePreferences(saved)
                  })
                }
              }}
            >
              {localModels.map((m) => (
                <option key={m.model ?? m.name} value={m.model ?? ''}>
                  {m.name}
                </option>
              ))}
              {localModels.length === 0 && (
                <option value=''>Default Pipeline Model</option>
              )}
            </NativeSelect>
          </div>
        )}

        {isCloud && (
          <>
            <div className='space-y-1.5'>
              <div className='flex items-center justify-between'>
                <label className='text-[10px] font-medium text-muted-foreground uppercase'>
                  {t('translation.apiKey', 'API Key')}
                </label>
                <button
                  type='button'
                  onClick={() => setShowKey(!showKey)}
                  className='text-[10px] text-muted-foreground hover:text-foreground flex items-center gap-1'
                >
                  {showKey ? <EyeOff className='size-2.5' /> : <Eye className='size-2.5' />}
                  {showKey ? 'Hide' : 'Show'}
                </button>
              </div>
              <Input
                type={showKey ? 'text' : 'password'}
                value={settings.apiKey}
                placeholder={
                  settings.provider === 'openai'
                    ? 'sk-...'
                    : settings.provider === 'claude'
                      ? 'sk-ant-...'
                      : settings.provider === 'gemini'
                        ? 'AIza...'
                        : 'Enter API Key'
                }
                className='h-7 text-xs font-mono'
                onChange={(e) => updateSetting('apiKey', e.currentTarget.value)}
              />
            </div>

            <div className='space-y-1.5'>
              <label className='text-[10px] font-medium text-muted-foreground uppercase'>
                {t('translation.model', 'Model Name (Optional Override)')}
              </label>
              <Input
                value={settings.model}
                placeholder={
                  settings.provider === 'openai'
                    ? 'gpt-4o-mini'
                    : settings.provider === 'claude'
                      ? 'claude-3-5-haiku-latest'
                      : settings.provider === 'gemini'
                        ? 'gemini-1.5-flash'
                        : settings.provider === 'openrouter'
                          ? 'meta-llama/llama-3.3-70b-instruct'
                          : 'default'
                }
                className='h-7 text-xs'
                onChange={(e) => updateSetting('model', e.currentTarget.value)}
              />
            </div>

            {(settings.provider === 'openai' || settings.provider === 'claude') && (
              <div className='space-y-1.5'>
                <label className='text-[10px] font-medium text-muted-foreground uppercase'>
                  {t('translation.customEndpoint', 'Custom Endpoint / Proxy (Optional)')}
                </label>
                <Input
                  value={settings.customEndpoint}
                  placeholder='e.g. http://localhost:11434/v1/chat/completions'
                  className='h-7 text-xs font-mono'
                  onChange={(e) => updateSetting('customEndpoint', e.currentTarget.value)}
                />
              </div>
            )}
          </>
        )}

        <div className='space-y-1.5'>
          <label className='text-[10px] font-medium text-muted-foreground uppercase'>
            {t('translation.targetLang', 'Target Language Code')}
          </label>
          <Input
            value={settings.targetLanguage}
            placeholder='en, th, ja, es, fr, zh, etc.'
            className='h-7 text-xs'
            onChange={(e) => updateSetting('targetLanguage', e.currentTarget.value)}
          />
        </div>

        <div className='pt-1 text-[10px] text-muted-foreground italic flex items-center gap-1'>
          <Check className='size-3 text-green-500' />
          <span>{t('translation.autoSaved', 'Settings saved automatically.')}</span>
        </div>
      </PopoverContent>
    </Popover>
  )
}
