'use client'

import { BookOpenText, Check } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { reportError, savePreferences } from '@/lib/backend'
import type { GlossarySettings } from '@/lib/glossary'
import { loadGlossarySettings, saveGlossarySettings } from '@/lib/glossary'
import { useGlossary, useProject, useSetGlossaryEnabled } from '@/lib/queries'
import { receivePreferences, useKoharuStore } from '@/lib/store'
import { Button } from '@koharu/ui/components/button'
import { Popover, PopoverContent, PopoverTrigger } from '@koharu/ui/components/popover'
import { Switch } from '@koharu/ui/components/switch'

interface GlossarySettingsPopoverProps {
  compact?: boolean
  onSettingsChange?: (settings: GlossarySettings) => void
}

export function GlossarySettingsPopover({
  compact = false,
  onSettingsChange,
}: GlossarySettingsPopoverProps) {
  const { t } = useTranslation()
  const [settings, setSettings] = useState<GlossarySettings>(() => loadGlossarySettings())
  const [open, setOpen] = useState(false)
  const project = useProject().data
  const glossaryQuery = useGlossary(project?.name)
  const glossary = glossaryQuery.data
  const toggleGlossary = useSetGlossaryEnabled(project?.name)
  const preferences = useKoharuStore((state) => state.preferences)

  useEffect(() => {
    if (!glossary) return
    setSettings((prev) => {
      if (prev.enabled === glossary.enabled) return prev
      const updated = { ...prev, enabled: glossary.enabled }
      saveGlossarySettings(updated)
      return updated
    })
  }, [glossary])

  const updateSetting = <K extends keyof GlossarySettings>(key: K, value: GlossarySettings[K]) => {
    const updated = { ...settings, [key]: value }
    setSettings(updated)
    saveGlossarySettings(updated)
    onSettingsChange?.(updated)

    if (key === 'enabled' && glossary && project?.name) {
      void toggleGlossary.mutate({ revision: glossary.revision, enabled: value as boolean })
    }

    if (
      (key === 'enablePersonas' || key === 'enableSpeakerTagging' || key === 'autoGuessSpeakers') &&
      preferences
    ) {
      const pipeline = {
        ...preferences.pipeline,
        translation: {
          ...preferences.pipeline.translation,
          enable_personas:
            key === 'enablePersonas'
              ? (value as boolean)
              : (preferences.pipeline?.translation?.enable_personas ?? true),
          enable_speaker_tagging:
            key === 'enableSpeakerTagging'
              ? (value as boolean)
              : (preferences.pipeline?.translation?.enable_speaker_tagging ?? true),
          auto_guess_speakers:
            key === 'autoGuessSpeakers'
              ? (value as boolean)
              : (preferences.pipeline?.translation?.auto_guess_speakers ?? true),
        },
      }
      void savePreferences(pipeline, preferences.providers, preferences.typesetting)
        .then((saved) => receivePreferences(saved))
        .catch(reportError)
    }
  }

  const activeCount =
    glossary?.entries.filter((e) => e.enabled && e.translation?.trim()).length ?? 0

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger
        render={
          compact ? (
            <button
              type='button'
              className='grid size-5 place-items-center rounded-sm text-muted-foreground hover:bg-foreground/[0.07] hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/25'
              title={
                settings.enabled
                  ? t('glossary.settingsTooltipActive', 'Glossary Settings (Active)')
                  : t('glossary.settingsTooltipDisabled', 'Glossary Settings (Disabled)')
              }
              onClick={(e) => e.stopPropagation()}
            >
              <BookOpenText className='size-3' />
            </button>
          ) : (
            <Button
              type='button'
              variant='ghost'
              size='xs'
              className='h-6 gap-1 px-1.5 text-[10px] text-muted-foreground hover:text-foreground'
              title={t('glossary.settingsTooltip', 'Glossary Settings & Substitution Rules')}
              onClick={(e) => e.stopPropagation()}
            >
              <BookOpenText className='size-3 text-muted-foreground' />
              <span className='text-[10px] font-medium capitalize'>
                {t('glossary.settings', 'Glossary')}
              </span>
            </Button>
          )
        }
      />
      <PopoverContent align='end' className='w-80 space-y-3 p-3 text-xs'>
        <div className='flex items-center justify-between border-b pb-2'>
          <div className='flex items-center gap-1.5 font-semibold text-foreground'>
            <BookOpenText className='size-3.5 text-primary' />
            <span>{t('glossary.settingsTitle', 'Glossary Settings')}</span>
          </div>
          <span
            className={`rounded px-1.5 py-0.5 text-[9px] font-medium uppercase ${
              settings.enabled ? 'bg-primary/10 text-primary' : 'bg-muted text-muted-foreground'
            }`}
          >
            {settings.enabled
              ? t('glossary.activeCount', { count: activeCount, defaultValue: '{{count}} Active' })
              : t('glossary.disabledStatus', 'Disabled')}
          </span>
        </div>

        <div className='space-y-2.5'>
          <div className='flex items-center justify-between gap-2'>
            <div className='min-w-0 space-y-0.5'>
              <p className='text-[11px] font-medium text-foreground'>
                {t('glossary.enforceInTranslation', 'Enforce in Translation')}
              </p>
              <p className='text-[9px] leading-tight text-muted-foreground'>
                {t(
                  'glossary.enforceDesc',
                  'Apply glossary entries during text and chapter translation',
                )}
              </p>
            </div>
            <Switch
              size='sm'
              checked={settings.enabled}
              onCheckedChange={(val) => updateSetting('enabled', val)}
            />
          </div>

          <div className='flex items-center justify-between gap-2'>
            <div className='min-w-0 space-y-0.5'>
              <p className='text-[11px] font-medium text-foreground'>
                {t('glossary.tokenShield', 'Pre-translation Token Shield')}
              </p>
              <p className='text-[9px] leading-tight text-muted-foreground'>
                {t(
                  'glossary.tokenShieldDesc',
                  'Protects terms with tokens before translation (Recommended for Google Translate & cloud models)',
                )}
              </p>
            </div>
            <Switch
              size='sm'
              checked={settings.preSubstitute}
              disabled={!settings.enabled}
              onCheckedChange={(val) => updateSetting('preSubstitute', val)}
            />
          </div>

          <div className='flex items-center justify-between gap-2'>
            <div className='min-w-0 space-y-0.5'>
              <p className='text-[11px] font-medium text-foreground'>
                {t('glossary.postReplace', 'Post-translation Replacement')}
              </p>
              <p className='text-[9px] leading-tight text-muted-foreground'>
                {t(
                  'glossary.postReplaceDesc',
                  'Scans output text and replaces leftover or untranslated source terms',
                )}
              </p>
            </div>
            <Switch
              size='sm'
              checked={settings.postReplace}
              disabled={!settings.enabled}
              onCheckedChange={(val) => updateSetting('postReplace', val)}
            />
          </div>
        </div>

        <div className='space-y-2 border-t pt-2'>
          <p className='text-[9px] font-semibold tracking-wider text-muted-foreground uppercase'>
            {t('glossary.matchingRules', 'Matching Rules')}
          </p>

          <div className='flex items-center justify-between gap-2'>
            <div className='min-w-0 space-y-0.5'>
              <p className='text-[11px] font-medium text-foreground'>
                {t('glossary.caseSensitive', 'Case Sensitive Matching')}
              </p>
              <p className='text-[9px] leading-tight text-muted-foreground'>
                {t('glossary.caseSensitiveDesc', 'Distinguish uppercase and lowercase words')}
              </p>
            </div>
            <Switch
              size='sm'
              checked={settings.caseSensitive}
              disabled={!settings.enabled}
              onCheckedChange={(val) => updateSetting('caseSensitive', val)}
            />
          </div>

          <div className='flex items-center justify-between gap-2'>
            <div className='min-w-0 space-y-0.5'>
              <p className='text-[11px] font-medium text-foreground'>
                {t('glossary.wholeWord', 'Whole Words Only')}
              </p>
              <p className='text-[9px] leading-tight text-muted-foreground'>
                {t(
                  'glossary.wholeWordDesc',
                  'Match boundary-delimited words (Latin/English terms only)',
                )}
              </p>
            </div>
            <Switch
              size='sm'
              checked={settings.wholeWord}
              disabled={!settings.enabled}
              onCheckedChange={(val) => updateSetting('wholeWord', val)}
            />
          </div>
        </div>

        <div className='space-y-2 border-t pt-2'>
          <p className='text-[9px] font-semibold tracking-wider text-muted-foreground uppercase'>
            {t('glossary.personasAndVoices', 'AI Character Personas & Voices')}
          </p>

          <div className='flex items-center justify-between gap-2'>
            <div className='min-w-0 space-y-0.5'>
              <p className='text-[11px] font-medium text-foreground'>
                {t('glossary.enablePersonas', 'Adopt Character Personas')}
              </p>
              <p className='text-[9px] leading-tight text-muted-foreground'>
                {t(
                  'glossary.personasDesc',
                  'Inject gender, tone, and speech style into AI prompts',
                )}
              </p>
            </div>
            <Switch
              size='sm'
              checked={settings.enablePersonas}
              onCheckedChange={(val) => updateSetting('enablePersonas', val)}
            />
          </div>

          <div className='flex items-center justify-between gap-2'>
            <div className='min-w-0 space-y-0.5'>
              <p className='text-[11px] font-medium text-foreground'>
                {t('glossary.enableSpeakerTagging', 'Speaker Tagging')}
              </p>
              <p className='text-[9px] leading-tight text-muted-foreground'>
                {t('glossary.taggingDesc', 'Prefix dialogue with [Speaker: Name] tag in prompt')}
              </p>
            </div>
            <Switch
              size='sm'
              checked={settings.enableSpeakerTagging}
              onCheckedChange={(val) => updateSetting('enableSpeakerTagging', val)}
            />
          </div>

          <div className='flex items-center justify-between gap-2'>
            <div className='min-w-0 space-y-0.5'>
              <p className='text-[11px] font-medium text-foreground'>
                {t('glossary.autoGuessSpeakers', 'Auto-detect Dialogue Speakers')}
              </p>
              <p className='text-[9px] leading-tight text-muted-foreground'>
                {t(
                  'glossary.autoGuessDesc',
                  'Infer speech bubble speakers from character glossary',
                )}
              </p>
            </div>
            <Switch
              size='sm'
              checked={settings.autoGuessSpeakers}
              onCheckedChange={(val) => updateSetting('autoGuessSpeakers', val)}
            />
          </div>
        </div>

        <div className='flex items-center gap-1 border-t pt-1 text-[10px] text-muted-foreground italic'>
          <Check className='size-3 text-green-500' />
          <span>{t('glossary.autoSaved', 'Settings saved automatically.')}</span>
        </div>
      </PopoverContent>
    </Popover>
  )
}
