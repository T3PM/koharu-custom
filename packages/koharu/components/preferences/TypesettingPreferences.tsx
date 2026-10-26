'use client'

import { Plus, Trash2 } from 'lucide-react'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'

import { FontPicker } from '@/components/controls/FontPicker'
import { PreferencePage } from '@/components/preferences/PreferenceFields'
import { useFonts } from '@/lib/queries'
import type { TypesettingConfig } from '@koharu/bridge/protocol'
import { Button } from '@koharu/ui/components/button'

export function TypesettingPreferences({
  value,
  onChange,
}: {
  value: TypesettingConfig
  onChange: (value: TypesettingConfig) => void
}) {
  const { t } = useTranslation()
  const fonts = useFonts()
  const families = fonts.data ?? []
  const fontFamilies = useMemo(() => value.font_families ?? [], [value.font_families])
  const selectedNames = useMemo(() => new Set(fontFamilies.map(normalizeFontName)), [fontFamilies])
  const availableFamilies = families.filter(
    (family) => !selectedNames.has(normalizeFontName(family.name)),
  )

  const replaceFamily = (index: number, family: string) => {
    const next = [...fontFamilies]
    next[index] = family
    onChange({ ...value, font_families: next })
  }

  const removeFamily = (index: number) => {
    onChange({
      ...value,
      font_families: fontFamilies.filter((_, candidate) => candidate !== index),
    })
  }

  return (
    <PreferencePage
      title={t('settings.typesetting.title')}
      description={t('settings.typesetting.description')}
    >
      <section aria-labelledby='typesetting-font-stack'>
        <header className='mb-3'>
          <h3 id='typesetting-font-stack' className='text-[12px] font-semibold'>
            {t('settings.typesetting.fontFallback')}
          </h3>
          <p className='mt-0.5 text-[11px] leading-4 text-muted-foreground'>
            {t('settings.typesetting.fontFallbackDescription')}
          </p>
        </header>
        <div className='overflow-hidden rounded-xl border border-border/80 bg-[var(--surface-panel)]'>
          {fontFamilies.length > 0 ? (
            <ol
              className='divide-y divide-border/80'
              aria-label={t('settings.typesetting.defaultFamilies')}
            >
              {fontFamilies.map((family, index) => {
                const choices = families.filter(
                  (candidate) =>
                    normalizeFontName(candidate.name) === normalizeFontName(family) ||
                    !selectedNames.has(normalizeFontName(candidate.name)),
                )
                return (
                  <li
                    key={`${normalizeFontName(family)}-${index}`}
                    className='group flex min-w-0 items-center gap-3 px-3 py-2'
                  >
                    <span
                      aria-hidden='true'
                      className='flex size-6 shrink-0 items-center justify-center rounded-md bg-muted text-[9px] font-semibold text-muted-foreground tabular-nums'
                    >
                      {String(index + 1).padStart(2, '0')}
                    </span>
                    <div className='min-w-0 flex-1'>
                      <FontPicker
                        value={family}
                        families={choices}
                        ariaLabel={t('settings.typesetting.familyLabel', { index: index + 1 })}
                        onChange={(next) => replaceFamily(index, next)}
                      />
                    </div>
                    <Button
                      type='button'
                      variant='ghost'
                      size='icon-sm'
                      aria-label={t('settings.typesetting.removeFamily', { family })}
                      title={t('settings.typesetting.removeFamily', { family })}
                      className='shrink-0 text-muted-foreground hover:bg-destructive/10 hover:text-destructive'
                      onClick={() => removeFamily(index)}
                    >
                      <Trash2 className='size-3.5' />
                    </Button>
                  </li>
                )
              })}
            </ol>
          ) : (
            <p role='status' className='px-4 py-5 text-[11px] text-muted-foreground'>
              {t('settings.typesetting.empty')}
            </p>
          )}
          <div className='flex min-w-0 items-center gap-3 border-t border-border/80 bg-muted/20 px-3 py-2'>
            <span className='flex size-6 shrink-0 items-center justify-center text-muted-foreground'>
              <Plus className='size-3.5' />
            </span>
            <div className='min-w-0 flex-1'>
              <FontPicker
                value=''
                families={availableFamilies}
                disabled={fonts.isPending || availableFamilies.length === 0}
                ariaLabel={t('settings.typesetting.addFamily')}
                placeholder={
                  fonts.isPending
                    ? t('settings.typesetting.loadingFonts')
                    : t('settings.typesetting.addFamily')
                }
                onChange={(family) =>
                  onChange({ ...value, font_families: [...fontFamilies, family] })
                }
              />
            </div>
            <span aria-hidden='true' className='size-7 shrink-0' />
          </div>
        </div>
        {fonts.isError && (
          <p role='status' className='mt-2 text-[10px] leading-4 text-destructive'>
            {t('settings.typesetting.loadError')}
          </p>
        )}
      </section>

      <section aria-labelledby='typesetting-role-presets' className='mt-6'>
        <header className='mb-3'>
          <h3 id='typesetting-role-presets' className='text-[12px] font-semibold'>
            {t('settings.typesetting.rolePresets', 'Role Font Presets')}
          </h3>
          <p className='mt-0.5 text-[11px] leading-4 text-muted-foreground'>
            {t(
              'settings.typesetting.rolePresetsDescription',
              'Specify custom font families for speech dialogue, sound effects (SFX), and narration.',
            )}
          </p>
        </header>
        <div className='overflow-hidden rounded-xl border border-border/80 bg-[var(--surface-panel)] divide-y divide-border/80'>
          <div className='flex min-w-0 items-center gap-3 px-3 py-2'>
            <span className='w-24 shrink-0 text-[11px] font-medium text-foreground'>
              {t('settings.typesetting.dialogueFont', 'Dialogue')}
            </span>
            <div className='min-w-0 flex-1'>
              <FontPicker
                value={value.dialogue_font ?? ''}
                families={families}
                ariaLabel={t('settings.typesetting.dialogueFont', 'Dialogue')}
                placeholder={t('settings.typesetting.defaultFallback', 'Default font fallback')}
                onChange={(font) => onChange({ ...value, dialogue_font: font || null })}
              />
            </div>
            {value.dialogue_font && (
              <Button
                type='button'
                variant='ghost'
                size='icon-sm'
                aria-label={t('settings.typesetting.clearDialogueFont', 'Clear dialogue font')}
                title={t('settings.typesetting.clearDialogueFont', 'Clear dialogue font')}
                className='shrink-0 text-muted-foreground hover:bg-destructive/10 hover:text-destructive'
                onClick={() => onChange({ ...value, dialogue_font: null })}
              >
                <Trash2 className='size-3.5' />
              </Button>
            )}
          </div>

          <div className='flex min-w-0 items-center gap-3 px-3 py-2'>
            <span className='w-24 shrink-0 text-[11px] font-medium text-foreground'>
              {t('settings.typesetting.sfxFont', 'SFX')}
            </span>
            <div className='min-w-0 flex-1'>
              <FontPicker
                value={value.sfx_font ?? ''}
                families={families}
                ariaLabel={t('settings.typesetting.sfxFont', 'SFX')}
                placeholder={t('settings.typesetting.defaultFallback', 'Default font fallback')}
                onChange={(font) => onChange({ ...value, sfx_font: font || null })}
              />
            </div>
            {value.sfx_font && (
              <Button
                type='button'
                variant='ghost'
                size='icon-sm'
                aria-label={t('settings.typesetting.clearSfxFont', 'Clear SFX font')}
                title={t('settings.typesetting.clearSfxFont', 'Clear SFX font')}
                className='shrink-0 text-muted-foreground hover:bg-destructive/10 hover:text-destructive'
                onClick={() => onChange({ ...value, sfx_font: null })}
              >
                <Trash2 className='size-3.5' />
              </Button>
            )}
          </div>

          <div className='flex min-w-0 items-center gap-3 px-3 py-2'>
            <span className='w-24 shrink-0 text-[11px] font-medium text-foreground'>
              {t('settings.typesetting.narrationFont', 'Narration')}
            </span>
            <div className='min-w-0 flex-1'>
              <FontPicker
                value={value.narration_font ?? ''}
                families={families}
                ariaLabel={t('settings.typesetting.narrationFont', 'Narration')}
                placeholder={t('settings.typesetting.defaultFallback', 'Default font fallback')}
                onChange={(font) => onChange({ ...value, narration_font: font || null })}
              />
            </div>
            {value.narration_font && (
              <Button
                type='button'
                variant='ghost'
                size='icon-sm'
                aria-label={t('settings.typesetting.clearNarrationFont', 'Clear narration font')}
                title={t('settings.typesetting.clearNarrationFont', 'Clear narration font')}
                className='shrink-0 text-muted-foreground hover:bg-destructive/10 hover:text-destructive'
                onClick={() => onChange({ ...value, narration_font: null })}
              >
                <Trash2 className='size-3.5' />
              </Button>
            )}
          </div>
        </div>
      </section>

      <section aria-labelledby='typesetting-typography-defaults' className='mt-6'>
        <header className='mb-3'>
          <h3 id='typesetting-typography-defaults' className='text-[12px] font-semibold'>
            {t('settings.typesetting.typographyDefaults', 'Default Typography')}
          </h3>
          <p className='mt-0.5 text-[11px] leading-4 text-muted-foreground'>
            {t(
              'settings.typesetting.typographyDefaultsDescription',
              'Configure the default font size and weight applied to newly detected text boxes.',
            )}
          </p>
        </header>
        <div className='overflow-hidden rounded-xl border border-border/80 bg-[var(--surface-panel)] divide-y divide-border/80'>
          <div className='flex min-w-0 items-center justify-between gap-3 px-3 py-2'>
            <span className='w-28 shrink-0 text-[11px] font-medium text-foreground'>
              {t('settings.typesetting.fontSize', 'Font Size (px)')}
            </span>
            <div className='flex items-center gap-2'>
              <input
                type='number'
                min='6'
                max='200'
                placeholder={t('settings.typesetting.fontSizeAuto', 'Auto-fit')}
                value={value.font_size ?? ''}
                onChange={(e) => {
                  const val = e.target.value ? Number(e.target.value) : null
                  onChange({ ...value, font_size: val })
                }}
                className='h-7 w-24 rounded-md border border-input bg-transparent px-2 text-[11px] text-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring'
              />
              {value.font_size != null && (
                <Button
                  type='button'
                  variant='ghost'
                  size='icon-sm'
                  aria-label={t('settings.typesetting.clearFontSize', 'Clear default font size')}
                  title={t('settings.typesetting.clearFontSize', 'Clear default font size')}
                  className='shrink-0 text-muted-foreground hover:bg-destructive/10 hover:text-destructive'
                  onClick={() => onChange({ ...value, font_size: null })}
                >
                  <Trash2 className='size-3.5' />
                </Button>
              )}
            </div>
          </div>

          <div className='flex min-w-0 items-center justify-between gap-3 px-3 py-2'>
            <span className='w-28 shrink-0 text-[11px] font-medium text-foreground'>
              {t('settings.typesetting.fontWeight', 'Font Weight')}
            </span>
            <div className='flex items-center gap-2'>
              <select
                value={value.font_weight ?? ''}
                onChange={(e) => {
                  const val = e.target.value ? Number(e.target.value) : null
                  onChange({ ...value, font_weight: val })
                }}
                className='h-7 rounded-md border border-input bg-background px-2 text-[11px] text-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring'
              >
                <option value=''>{t('settings.typesetting.fontWeightDefault', 'Default (Regular)')}</option>
                <option value='100'>100 - Thin</option>
                <option value='200'>200 - Extra Light</option>
                <option value='300'>300 - Light</option>
                <option value='400'>400 - Regular</option>
                <option value='500'>500 - Medium</option>
                <option value='600'>600 - Semi Bold</option>
                <option value='700'>700 - Bold</option>
                <option value='800'>800 - Extra Bold</option>
                <option value='900'>900 - Black</option>
              </select>
              {value.font_weight != null && (
                <Button
                  type='button'
                  variant='ghost'
                  size='icon-sm'
                  aria-label={t('settings.typesetting.clearFontWeight', 'Clear default font weight')}
                  title={t('settings.typesetting.clearFontWeight', 'Clear default font weight')}
                  className='shrink-0 text-muted-foreground hover:bg-destructive/10 hover:text-destructive'
                  onClick={() => onChange({ ...value, font_weight: null })}
                >
                  <Trash2 className='size-3.5' />
                </Button>
              )}
            </div>
          </div>
        </div>
      </section>
    </PreferencePage>
  )
}

function normalizeFontName(value: string): string {
  return value.normalize('NFKC').trim().replace(/\s+/g, ' ').toLowerCase()
}
