'use client'

import { Crosshair, Trash2 } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { glossaryKinds, normalizeGlossaryTranslation } from '@/lib/glossary'
import { reportError } from '@/lib/backend'
import { useProject, useUpdateCharacterProfile } from '@/lib/queries'
import { useKoharuStore } from '@/lib/store'
import {
  commands,
  type CharacterGender,
  type CharacterProfile,
  type GlossaryEntryPatch,
  type GlossaryEntryView,
  type GlossaryKind,
} from '@koharu/bridge/protocol'
import { Badge } from '@koharu/ui/components/badge'
import { Button } from '@koharu/ui/components/button'
import { Checkbox } from '@koharu/ui/components/checkbox'
import { Input } from '@koharu/ui/components/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@koharu/ui/components/select'
import { Switch } from '@koharu/ui/components/switch'
import { toast } from '@koharu/ui/components/toast'

interface GlossaryEntryRowProps {
  entry: GlossaryEntryView
  selected?: boolean
  disabled: boolean
  onSelect?: (selected: boolean) => void
  onUpdate: (patch: GlossaryEntryPatch) => void
  onDelete: () => void
}

export function GlossaryEntryRow({
  entry,
  selected = false,
  disabled,
  onSelect,
  onUpdate,
  onDelete,
}: GlossaryEntryRowProps) {
  const { t } = useTranslation()
  const project = useProject().data
  const updateCharacterProfile = useUpdateCharacterProfile(project?.name)
  const [source, setSource] = useState(entry.source)
  const [translation, setTranslation] = useState(entry.translation ?? '')
  const [tone, setTone] = useState(entry.character?.tone ?? '')
  const [firstPerson, setFirstPerson] = useState(entry.character?.firstPerson ?? '')
  const [secondPerson, setSecondPerson] = useState(entry.character?.secondPerson ?? '')
  const [customInstructions, setCustomInstructions] = useState(entry.character?.customInstructions ?? '')
  const [honorificPolicy, setHonorificPolicy] = useState(entry.character?.honorificPolicy ?? '')
  const [forbiddenVariantsText, setForbiddenVariantsText] = useState(
    (entry.forbiddenVariants ?? []).join(', '),
  )
  const cancelSourceBlur = useRef(false)
  const cancelTranslationBlur = useRef(false)
  const occurrenceIndexRef = useRef(0)
  const selectPages = useKoharuStore((state) => state.selectPages)
  const selectLayers = useKoharuStore((state) => state.selectLayers)

  const handleLocate = async () => {
    try {
      const occurrences = await commands.locateGlossaryEntry(entry.id)
      if (!occurrences || occurrences.length === 0) {
        toast.add({ title: t('glossary.noOccurrences', 'No occurrences found on pages') })
        return
      }
      const idx = occurrenceIndexRef.current % occurrences.length
      const occurrence = occurrences[idx]
      occurrenceIndexRef.current = idx + 1
      selectPages([occurrence.pageId])
      selectLayers([occurrence.entityId])
    } catch (e) {
      reportError(e)
    }
  }

  useEffect(() => {
    setSource(entry.source)
    setTranslation(entry.translation ?? '')
    setTone(entry.character?.tone ?? '')
    setFirstPerson(entry.character?.firstPerson ?? '')
    setSecondPerson(entry.character?.secondPerson ?? '')
    setCustomInstructions(entry.character?.customInstructions ?? '')
    setHonorificPolicy(entry.character?.honorificPolicy ?? '')
    setForbiddenVariantsText((entry.forbiddenVariants ?? []).join(', '))
  }, [
    entry.revision,
    entry.source,
    entry.translation,
    entry.character?.tone,
    entry.character?.firstPerson,
    entry.character?.secondPerson,
    entry.character?.customInstructions,
    entry.character?.honorificPolicy,
    entry.forbiddenVariants,
  ])

  const patch = (overrides: Partial<GlossaryEntryPatch> = {}): GlossaryEntryPatch => ({
    source,
    translation: normalizeGlossaryTranslation(translation),
    kind: entry.kind,
    enabled: entry.enabled,
    character: entry.character,
    ...(entry.forbiddenVariants !== undefined
      ? { forbiddenVariants: entry.forbiddenVariants }
      : {}),
    ...overrides,
  })

  const saveSource = () => {
    const value = source.trim()
    if (value && value !== entry.source) onUpdate(patch({ source: value }))
  }
  const saveTranslation = () => {
    const value = normalizeGlossaryTranslation(translation)
    if (value !== entry.translation) onUpdate(patch({ translation: value }))
  }
  const saveForbiddenVariants = () => {
    const next = forbiddenVariantsText
      .split(',')
      .map((item) => item.trim())
      .filter((item) => item.length > 0)
    const current = entry.forbiddenVariants ?? []
    const unchanged =
      next.length === current.length && next.every((val, index) => val === current[index])
    if (!unchanged) {
      onUpdate(patch({ forbiddenVariants: next }))
    }
  }
  const saveCharacterProfile = (overrides: Partial<CharacterProfile>) => {
    const profile: CharacterProfile = {
      id: entry.id,
      gender: entry.character?.gender ?? 'unspecified',
      firstPerson: entry.character?.firstPerson ?? null,
      secondPerson: entry.character?.secondPerson ?? null,
      tone: entry.character?.tone ?? null,
      customInstructions: entry.character?.customInstructions ?? null,
      ...(entry.character?.honorificPolicy !== undefined
        ? { honorificPolicy: entry.character.honorificPolicy }
        : {}),
      ...overrides,
    }
    if (project?.name) {
      updateCharacterProfile.mutate({
        revision: project.revision ?? entry.revision,
        entryId: entry.id,
        profile,
      })
    }
    onUpdate(patch({ character: profile }))
  }

  return (
    <article className='rounded-lg border border-border/70 bg-background/40 p-2.5'>
      <div className='flex min-w-0 items-center gap-2'>
        <Checkbox
          aria-label={t('glossary.selectEntry', { source: entry.source })}
          checked={selected}
          disabled={disabled}
          onCheckedChange={(checked) => onSelect?.(checked === true)}
        />
        <span className='min-w-0 flex-1 truncate text-[11px] font-medium'>{entry.source}</span>
        <Badge variant='secondary' className='h-4 px-1.5 text-[9px]'>
          {t(`glossary.kinds.${entry.kind}`)}
        </Badge>
        <Switch
          size='sm'
          aria-label={t('glossary.enableEntry', { source: entry.source })}
          checked={entry.enabled}
          disabled={disabled}
          onCheckedChange={(enabled) => onUpdate(patch({ enabled }))}
        />
        <Button
          size='icon-xs'
          variant='ghost'
          title={t('glossary.locate', 'Show on image')}
          aria-label={t('glossary.locate', 'Show on image')}
          disabled={disabled}
          onClick={() => void handleLocate()}
        >
          <Crosshair className='size-3' />
        </Button>
        <Button
          size='icon-xs'
          variant='ghost'
          aria-label={t('glossary.deleteEntry', { source: entry.source })}
          disabled={disabled}
          onClick={onDelete}
        >
          <Trash2 className='size-3' />
        </Button>
      </div>
      <div className='mt-2 grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)_5.25rem] gap-1.5'>
        <Input
          aria-label={t('glossary.sourceEntry', { source: entry.source })}
          value={source}
          disabled={disabled}
          className='h-7 px-2 text-[11px]'
          onChange={(event) => setSource(event.currentTarget.value)}
          onBlur={() => {
            if (cancelSourceBlur.current) {
              cancelSourceBlur.current = false
              return
            }
            saveSource()
          }}
          onKeyDown={(event) => {
            if (event.key === 'Enter') event.currentTarget.blur()
            if (event.key === 'Escape') {
              event.preventDefault()
              cancelSourceBlur.current = true
              setSource(entry.source)
              event.currentTarget.blur()
            }
          }}
        />
        <Input
          aria-label={t('glossary.translationEntry', { source: entry.source })}
          value={translation}
          disabled={disabled}
          placeholder={t('glossary.untranslated')}
          className='h-7 px-2 text-[11px]'
          onChange={(event) => setTranslation(event.currentTarget.value)}
          onBlur={() => {
            if (cancelTranslationBlur.current) {
              cancelTranslationBlur.current = false
              return
            }
            saveTranslation()
          }}
          onKeyDown={(event) => {
            if (event.key === 'Enter') event.currentTarget.blur()
            if (event.key === 'Escape') {
              event.preventDefault()
              cancelTranslationBlur.current = true
              setTranslation(entry.translation ?? '')
              event.currentTarget.blur()
            }
          }}
        />
        <Select
          value={entry.kind}
          items={Object.fromEntries(
            glossaryKinds.map((kind) => [kind, t(`glossary.kinds.${kind}`)]),
          )}
          disabled={disabled}
          onValueChange={(kind) => kind && onUpdate(patch({ kind: kind as GlossaryKind }))}
        >
          <SelectTrigger
            size='sm'
            className='h-7 w-full'
            aria-label={t('glossary.categoryEntry', { source: entry.source })}
          >
            <SelectValue />
          </SelectTrigger>
          <SelectContent align='end'>
            {glossaryKinds.map((kind) => (
              <SelectItem key={kind} value={kind}>
                {t(`glossary.kinds.${kind}`)}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      <div className='mt-1.5'>
        <Input
          aria-label={t('glossary.forbiddenVariantsPlaceholder', 'Forbidden variants (comma-separated)')}
          value={forbiddenVariantsText}
          disabled={disabled}
          placeholder={t(
            'glossary.forbiddenVariantsPlaceholder',
            'Forbidden variants (comma-separated)',
          )}
          className='h-6 px-2 text-[10px]'
          onChange={(event) => setForbiddenVariantsText(event.currentTarget.value)}
          onBlur={saveForbiddenVariants}
          onKeyDown={(event) => {
            if (event.key === 'Enter') event.currentTarget.blur()
          }}
        />
      </div>
      <div className='mt-2 flex flex-wrap items-center gap-x-2 gap-y-1 text-[9px] text-muted-foreground'>
        {entry.confidence !== null ? (
          <span>{t('glossary.confidence', { value: Math.round(entry.confidence * 100) })}</span>
        ) : null}
        <span>{t('glossary.occurrences', { count: entry.occurrenceCount })}</span>
        <span>{t(`glossary.origins.${entry.sourceOrigin}`)}</span>
        {entry.translationOrigin ? (
          <span>{t(`glossary.origins.${entry.translationOrigin}`)}</span>
        ) : null}
        {!entry.presentInLastScan ? (
          <span className='font-medium text-amber-600 dark:text-amber-400'>
            {t('glossary.notPresent')}
          </span>
        ) : null}
      </div>
      {entry.examples.length > 0 ? (
        <details className='mt-1.5 text-[9px] text-muted-foreground'>
          <summary className='cursor-pointer select-none'>{t('glossary.examples')}</summary>
          <ul className='mt-1 space-y-0.5 border-l pl-2'>
            {entry.examples.map((example, index) => (
              <li key={`${entry.id}-${index}`}>{example}</li>
            ))}
          </ul>
        </details>
      ) : null}
      {entry.kind === 'person' ? (
        <details className='mt-2 rounded border border-border/60 bg-muted/20 p-2 text-[10px]'>
          <summary className='cursor-pointer select-none font-medium text-foreground/80'>
            {t('glossary.persona', 'Character Persona')}
          </summary>
          <div className='mt-2 grid grid-cols-2 gap-2'>
            <div>
              <label className='mb-0.5 block text-[9px] text-muted-foreground'>
                {t('glossary.gender', 'Gender')}
              </label>
              <Select
                value={entry.character?.gender ?? 'unspecified'}
                disabled={disabled}
                onValueChange={(gender) => {
                  if (!gender) return
                  saveCharacterProfile({ gender: gender as CharacterGender })
                }}
              >
                <SelectTrigger size='sm' className='h-6 w-full text-[10px]'>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value='unspecified'>
                    {t('glossary.genderUnspecified', 'Unspecified')}
                  </SelectItem>
                  <SelectItem value='male'>{t('glossary.genderMale', 'Male')}</SelectItem>
                  <SelectItem value='female'>{t('glossary.genderFemale', 'Female')}</SelectItem>
                  <SelectItem value='non_binary'>
                    {t('glossary.genderNonBinary', 'Non-Binary')}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div>
              <label className='mb-0.5 block text-[9px] text-muted-foreground'>
                {t('glossary.tone', 'Tone')}
              </label>
              <Input
                value={tone}
                onChange={(e) => setTone(e.currentTarget.value)}
                placeholder={t('glossary.tonePlaceholder', 'e.g. polite, playful')}
                disabled={disabled}
                className='h-6 px-1.5 text-[10px]'
                onBlur={() => {
                  const val = tone.trim() || null
                  if (val !== (entry.character?.tone ?? null)) {
                    saveCharacterProfile({ tone: val })
                  }
                }}
              />
            </div>
            <div>
              <label className='mb-0.5 block text-[9px] text-muted-foreground'>
                {t('glossary.firstPerson', '1st Person (I/Me)')}
              </label>
              <Input
                value={firstPerson}
                onChange={(e) => setFirstPerson(e.currentTarget.value)}
                placeholder='e.g. 私, 俺, 僕'
                disabled={disabled}
                className='h-6 px-1.5 text-[10px]'
                onBlur={() => {
                  const val = firstPerson.trim() || null
                  if (val !== (entry.character?.firstPerson ?? null)) {
                    saveCharacterProfile({ firstPerson: val })
                  }
                }}
              />
            </div>
            <div>
              <label className='mb-0.5 block text-[9px] text-muted-foreground'>
                {t('glossary.secondPerson', '2nd Person (You)')}
              </label>
              <Input
                value={secondPerson}
                onChange={(e) => setSecondPerson(e.currentTarget.value)}
                placeholder='e.g. あなた, お前'
                disabled={disabled}
                className='h-6 px-1.5 text-[10px]'
                onBlur={() => {
                  const val = secondPerson.trim() || null
                  if (val !== (entry.character?.secondPerson ?? null)) {
                    saveCharacterProfile({ secondPerson: val })
                  }
                }}
              />
            </div>
            <div className='col-span-2'>
              <label className='mb-0.5 block text-[9px] text-muted-foreground'>
                {t('glossary.customInstructions', 'Custom Persona Instructions')}
              </label>
              <Input
                value={customInstructions}
                onChange={(e) => setCustomInstructions(e.currentTarget.value)}
                placeholder={t(
                  'glossary.instructionsPlaceholder',
                  'e.g. Speaks in tsundere manner, ends sentences with ~nya',
                )}
                disabled={disabled}
                className='h-6 px-1.5 text-[10px]'
                onBlur={() => {
                  const val = customInstructions.trim() || null
                  if (val !== (entry.character?.customInstructions ?? null)) {
                    saveCharacterProfile({ customInstructions: val })
                  }
                }}
              />
            </div>
            <div className='col-span-2'>
              <label className='mb-0.5 block text-[9px] text-muted-foreground'>
                {t('glossary.honorificPolicy', 'Honorific Policy')}
              </label>
              <Input
                aria-label={t('glossary.honorificPolicy', 'Honorific Policy')}
                value={honorificPolicy}
                onChange={(e) => setHonorificPolicy(e.currentTarget.value)}
                placeholder={t(
                  'glossary.honorificPolicyPlaceholder',
                  'keep, drop, or adapt',
                )}
                disabled={disabled}
                className='h-6 px-1.5 text-[10px]'
                onBlur={() => {
                  const val = honorificPolicy.trim() || null
                  if (val !== (entry.character?.honorificPolicy ?? null)) {
                    saveCharacterProfile({ honorificPolicy: val })
                  }
                }}
                onKeyDown={(event) => {
                  if (event.key === 'Enter') event.currentTarget.blur()
                }}
              />
            </div>
          </div>
        </details>
      ) : null}
    </article>
  )
}
