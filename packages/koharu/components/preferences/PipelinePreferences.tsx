'use client'

import { Eraser, FileText, Search } from 'lucide-react'
import { useTranslation } from 'react-i18next'

import {
  defaultModel,
  modelNames,
  modelOptions,
  replaceStage,
  stageModel,
  type ModelName,
  type ModelStage,
  type PipelineModel,
} from '@/components/preferences/models'
import {
  NumberField,
  PreferencePage,
  PreferenceRow,
  PreferenceSection,
  TextField,
} from '@/components/preferences/PreferenceFields'
import type { PipelineConfig } from '@koharu/bridge/protocol'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@koharu/ui/components/select'

const stages = [
  ['detection', Search],
  ['ocr', FileText],
  ['inpainting', Eraser],
] as const satisfies ReadonlyArray<readonly [ModelStage, typeof Search]>

export function PipelinePreferences({
  value,
  onChange,
}: {
  value: PipelineConfig
  onChange: (value: PipelineConfig) => void
}) {
  const { t } = useTranslation()
  return (
    <PreferencePage
      title={t('settings.pipeline.title')}
      description={t('settings.pipeline.description')}
    >
      <PreferenceSection title={t('settings.pipeline.processing')}>
        {stages.map(([stage, Icon]) => {
          const model = stageModel(value, stage)
          const title = t(`settings.pipeline.stages.${stage}.title`)
          return (
            <PreferenceRow
              key={stage}
              title={title}
              description={t(`settings.pipeline.stages.${stage}.description`)}
              align='start'
            >
              <div className='grid gap-3'>
                <div className='flex items-center gap-2'>
                  <Icon className='size-3.5 shrink-0 text-muted-foreground' />
                  <Select
                    value={model.model}
                    items={Object.fromEntries(
                      modelOptions[stage].map((name) => [name, modelNames[name]]),
                    )}
                    onValueChange={(name) => {
                      if (name) {
                        onChange(replaceStage(value, stage, defaultModel(name as ModelName)))
                      }
                    }}
                  >
                    <SelectTrigger
                      aria-label={t('settings.pipeline.modelLabel', { stage: title })}
                      className='h-8 min-w-0 flex-1 text-[11px]'
                    >
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {modelOptions[stage].map((name) => (
                        <SelectItem key={name} value={name}>
                          {modelNames[name]}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
                <ModelOptions
                  model={model}
                  onChange={(next) => onChange(replaceStage(value, stage, next))}
                />
              </div>
            </PreferenceRow>
          )
        })}
      </PreferenceSection>

      <PreferenceSection title={t('settings.pipeline.ocrTitle', 'OCR Options')}>
        <PreferenceRow
          title={t('settings.pipeline.textDirection', 'Text Direction')}
          description={t(
            'settings.pipeline.textDirectionDescription',
            'Preferred text orientation for OCR reading',
          )}
        >
          <Select
            value={value.ocr_options?.direction ?? 'auto'}
            items={{
              auto: 'Auto',
              vertical: 'Vertical',
              horizontal: 'Horizontal',
            }}
            onValueChange={(dir) => {
              if (dir) {
                onChange({
                  ...value,
                  ocr_options: {
                    ...(value.ocr_options ?? {
                      target_language: null,
                      direction: 'auto',
                      crop_margin: 6,
                    }),
                    direction: dir as any,
                  },
                })
              }
            }}
          >
            <SelectTrigger className='h-8 min-w-[140px] text-[11px]'>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value='auto'>Auto</SelectItem>
              <SelectItem value='vertical'>Vertical</SelectItem>
              <SelectItem value='horizontal'>Horizontal</SelectItem>
            </SelectContent>
          </Select>
        </PreferenceRow>

        <PreferenceRow
          title={t('settings.pipeline.cropMargin', 'Crop Margin')}
          description={t(
            'settings.pipeline.cropMarginDescription',
            'Padding pixels around detected text boxes',
          )}
        >
          <NumberField
            label=''
            value={value.ocr_options?.crop_margin ?? 6}
            min={0}
            max={64}
            step={1}
            onChange={(crop_margin) => {
              onChange({
                ...value,
                ocr_options: {
                  ...(value.ocr_options ?? {
                    target_language: null,
                    direction: 'auto',
                    crop_margin: 6,
                  }),
                  crop_margin: crop_margin ?? 6,
                },
              })
            }}
          />
        </PreferenceRow>
      </PreferenceSection>

      <PreferenceSection title={t('settings.pipeline.exportTitle', 'Export Options')}>
        <PreferenceRow
          title={t('settings.pipeline.exportQuality', 'Image Quality')}
          description={t(
            'settings.pipeline.exportQualityDescription',
            'Compression quality for exported JPEG / WebP / AVIF (1-100)',
          )}
        >
          <NumberField
            label=''
            value={value.export_options?.quality ?? 90}
            min={1}
            max={100}
            step={1}
            onChange={(quality) => {
              onChange({
                ...value,
                export_options: {
                  ...(value.export_options ?? {
                    format: 'png',
                    quality: 90,
                    template: '{project}/{chapter}/{page}',
                  }),
                  quality: quality ?? 90,
                },
              })
            }}
          />
        </PreferenceRow>

        <PreferenceRow
          title={t('settings.pipeline.exportTemplate', 'Export Path Template')}
          description={t(
            'settings.pipeline.exportTemplateDescription',
            'Naming pattern for exported files',
          )}
        >
          <TextField
            label=''
            value={value.export_options?.template ?? '{project}/{chapter}/{page}'}
            onChange={(template) => {
              onChange({
                ...value,
                export_options: {
                  ...(value.export_options ?? {
                    format: 'png',
                    quality: 90,
                    template: '{project}/{chapter}/{page}',
                  }),
                  template: template || '{project}/{chapter}/{page}',
                },
              })
            }}
          />
        </PreferenceRow>
      </PreferenceSection>

      <PreferenceSection title={t('settings.pipeline.hardwareTitle', 'Hardware & Performance')}>
        <PreferenceRow
          title={t('settings.pipeline.workerThreads', 'Worker Threads')}
          description={t(
            'settings.pipeline.workerThreadsDescription',
            'Parallel background worker threads (0 for auto)',
          )}
        >
          <NumberField
            label=''
            value={value.hardware_options?.threads ?? 4}
            min={1}
            max={32}
            step={1}
            onChange={(worker_threads) => {
              onChange({
                ...value,
                hardware_options: {
                  ...(value.hardware_options ?? { vram_budget_gb: null, threads: 4 }),
                  threads: worker_threads ?? 4,
                },
              })
            }}
          />
        </PreferenceRow>

        <PreferenceRow
          title={t('settings.pipeline.vramBudget', 'VRAM Budget (MB)')}
          description={t(
            'settings.pipeline.vramBudgetDescription',
            'Maximum VRAM allocation in MB for model inference (empty for auto)',
          )}
        >
          <NumberField
            label=''
            value={value.hardware_options?.vram_budget_gb ?? null}
            min={512}
            max={65536}
            step={512}
            onChange={(vram_budget_mb) => {
              onChange({
                ...value,
                hardware_options: {
                  ...(value.hardware_options ?? { vram_budget_gb: null, threads: 4 }),
                  vram_budget_gb: vram_budget_mb,
                },
              })
            }}
          />
        </PreferenceRow>
      </PreferenceSection>
    </PreferencePage>
  )
}

function ModelOptions({
  model,
  onChange,
}: {
  model: PipelineModel
  onChange: (model: PipelineModel) => void
}) {
  const { t } = useTranslation()
  switch (model.model) {
    case 'koharu-layout-rfdetr-seg-2xl':
      return (
        <div className='grid grid-cols-3 gap-2'>
          <NumberField
            label={t('settings.pipeline.options.textThreshold')}
            value={model.text_threshold ?? null}
            min={0}
            max={1}
            step={0.05}
            onChange={(text_threshold) => onChange({ ...model, text_threshold })}
          />
          <NumberField
            label={t('settings.pipeline.options.bubbleThreshold')}
            value={model.bubble_threshold ?? null}
            min={0}
            max={1}
            step={0.05}
            onChange={(bubble_threshold) => onChange({ ...model, bubble_threshold })}
          />
          <NumberField
            label={t('settings.pipeline.options.panelThreshold')}
            value={model.panel_threshold ?? null}
            min={0}
            max={1}
            step={0.05}
            onChange={(panel_threshold) => onChange({ ...model, panel_threshold })}
          />
        </div>
      )
    case 'flux2-klein':
      return (
        <TextField
          label={t('settings.pipeline.options.prompt')}
          value={model.prompt ?? 'Remove the text and reconstruct the background.'}
          onChange={(prompt) => onChange({ ...model, prompt })}
        />
      )
    case 'rorem-mixed':
      return (
        <div className='grid grid-cols-2 gap-2'>
          <TextField
            label={t('settings.pipeline.options.prompt')}
            value={model.prompt ?? ''}
            onChange={(prompt) => onChange({ ...model, prompt })}
          />
          <TextField
            label={t('settings.pipeline.options.negativePrompt')}
            value={model.negative_prompt ?? ''}
            onChange={(negative_prompt) => onChange({ ...model, negative_prompt })}
          />
        </div>
      )
    case 'paddleocr-vl-1.6':
    case 'manga-ocr':
    case 'baberu-ocr':
    case 'hayai-ocr':
    case 'lama':
    case 'aot-inpainting':
      return null
  }
}
