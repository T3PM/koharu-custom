import { QueryClientProvider } from '@tanstack/react-query'
import { fireEvent, render as testingRender, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { ReactNode } from 'react'
import { describe, expect, it, vi } from 'vitest'

import { Inspector } from '@/components/editor/Inspector'
import { GlossaryEntryRow } from '@/components/editor/GlossaryEntryRow'
import { GlossaryPanel } from '@/components/editor/GlossaryPanel'
import { TypesettingPreferences } from '@/components/preferences/TypesettingPreferences'
import { resources } from '@/lib/i18n'
import {
  fontsKey,
  glossaryKey,
  pageKey,
  projectKey,
  queryClient,
} from '@/lib/queries'
import { useKoharuStore } from '@/lib/store'
import {
  commands,
  type FontFamily,
  type GlossaryEntryView,
  type GlossaryLintIssue,
  type GlossaryView,
  type Layer,
  type PageSummary,
  type ProjectInfo,
  type TypesettingConfig,
} from '@koharu/bridge/protocol'
import { TooltipProvider } from '@koharu/ui/components/tooltip'

const projectFixture: ProjectInfo = {
  name: 'Manga Project',
  revision: 5,
  active_page: 'page_1',
  can_undo: true,
  can_redo: false,
}

const pageSummaryFixture: PageSummary = {
  id: 'page_1',
  label: 'Page 1',
  chapter: 'Chapter 1',
  index: 0,
  dimensions: [1200, 1800],
  layer_count: 2,
  thumbnail_blob: null,
  ocr_coverage: 1,
  translation_coverage: 1,
}

const personEntry: GlossaryEntryView = {
  revision: 5,
  id: 'person_yuki',
  source: 'ユキ',
  translation: 'Yuki',
  kind: 'person',
  enabled: true,
  confidence: 1.0,
  occurrenceCount: 2,
  examples: ['ユキは微笑んだ'],
  sourceOrigin: 'user',
  translationOrigin: 'user',
  presentInLastScan: true,
  character: {
    id: 'person_yuki',
    gender: 'female',
    firstPerson: '私',
    secondPerson: 'あなた',
    tone: 'gentle',
    customInstructions: 'Speaks politely',
  },
}

const glossaryFixture: GlossaryView = {
  revision: 5,
  enabled: true,
  stale: false,
  sourceLanguage: 'ja',
  targetLanguage: 'en',
  savedSourceFingerprint: 'fp-1',
  currentSourceFingerprint: 'fp-1',
  entries: [personEntry],
}

const sampleTextLayer: Extract<Layer, { type: 'text' }> = {
  type: 'text',
  id: 'layer_1',
  parent: 'page_1',
  angle_degrees: 0,
  geometry: {
    points: [
      { x: 10, y: 10 },
      { x: 100, y: 10 },
      { x: 100, y: 60 },
      { x: 10, y: 60 },
    ],
  },
  visibility: { visible: true, opacity: 1 },
  content: {
    id: 'content_1',
    source: { text: 'こんにちは', language: 'ja' },
    translation: { text: 'Hello', language: 'en' },
    role: null,
    source_region: null,
  },
  typography: {
    preferred_font: 'Wild Words',
    font_weight: 400,
    font_style: 'normal',
    size: 24,
    auto_fit: true,
    color: [0, 0, 0, 255],
    stroke_color: [255, 255, 255, 255],
    stroke_width: 2,
    alignment: 'Center',
    writing_mode: null,
  },
  layout: 'paragraph',
  automatic_region: null,
  speaker: null,
  balloon_shape: 'normal_speech',
}

const pageDetailFixture = {
  id: 'page_1',
  label: 'Page 1',
  size: { width: 1200, height: 1800 },
  layers: [sampleTextLayer],
  regions: [],
}

const sampleFontFamilies: FontFamily[] = [
  {
    name: 'Wild Words',
    metadata: {
      primary_script: 'latn',
      scripts: ['latn'],
      languages: ['en'],
      category: 'comic',
      classifications: ['dialogue'],
      use_cases: ['dialogue'],
    },
    sources: ['bundled'],
    faces: [],
  },
  {
    name: 'CC Wild Words',
    metadata: {
      primary_script: 'latn',
      scripts: ['latn'],
      languages: ['en'],
      category: 'comic',
      classifications: ['dialogue'],
      use_cases: ['dialogue'],
    },
    sources: ['bundled'],
    faces: [],
  },
  {
    name: 'Comic Talk',
    metadata: {
      primary_script: 'latn',
      scripts: ['latn'],
      languages: ['en'],
      category: 'comic',
      classifications: ['dialogue'],
      use_cases: ['dialogue'],
    },
    sources: ['bundled'],
    faces: [],
  },
  {
    name: 'Classic Comic',
    metadata: {
      primary_script: 'latn',
      scripts: ['latn'],
      languages: ['en'],
      category: 'comic',
      classifications: ['dialogue'],
      use_cases: ['dialogue'],
    },
    sources: ['bundled'],
    faces: [],
  },
  {
    name: 'Noto Sans',
    metadata: {
      primary_script: 'latn',
      scripts: ['latn'],
      languages: ['en'],
      category: 'sans-serif',
      classifications: ['dialogue'],
      use_cases: ['dialogue'],
    },
    sources: ['system'],
    faces: [],
  },
]

const typesettingConfigFixture: TypesettingConfig = {
  font_families: ['Wild Words', 'Noto Sans', 'Comic Talk'],
  dialogue_font: 'Wild Words',
  shout_font: 'CC Wild Words',
  thought_font: 'Comic Talk',
  whisper_font: 'Noto Sans',
  narration_font: 'Classic Comic',
  font_size: 20,
  font_weight: 400,
}

function renderWithProviders(ui: ReactNode) {
  return testingRender(
    <QueryClientProvider client={queryClient}>
      <TooltipProvider>{ui}</TooltipProvider>
    </QueryClientProvider>,
  )
}

describe('Tier 1: Feature Coverage E2E Tests', () => {
  describe('Inspector Balloon Classification & Speaker Integration', () => {
    it('renders balloon shape select and dispatches setBalloonShape with expected revision', async () => {
      queryClient.setQueryData(projectKey, projectFixture)
      queryClient.setQueryData(pageKey, pageDetailFixture)
      queryClient.setQueryData(fontsKey, sampleFontFamilies)
      queryClient.setQueryData(glossaryKey('Manga Project'), glossaryFixture)
      useKoharuStore.setState({ selectedLayers: ['layer_1'] })

      const setBalloonShapeSpy = vi
        .spyOn(commands, 'setBalloonShape')
        .mockResolvedValue(undefined as any)

      renderWithProviders(
        <Inspector
          project={projectFixture}
          page={pageSummaryFixture}
          layers={[sampleTextLayer]}
          selectedLayerIds={['layer_1']}
          onSelectLayers={() => {}}
        />,
      )

      expect(screen.getByText('Balloon Shape')).toBeInTheDocument()
      expect(screen.getByText('Normal Speech')).toBeInTheDocument()

      const trigger = screen.getByText('Normal Speech')
      await userEvent.click(trigger)

      const shoutOption = await screen.findByText('Shout / Jagged')
      await userEvent.click(shoutOption)

      expect(setBalloonShapeSpy).toHaveBeenCalledWith(
        5, // project revision
        'layer_1',
        'shout',
      )
    })

    it('renders speaker dropdown populated from glossary person entries and dispatches setBubbleSpeaker', async () => {
      queryClient.setQueryData(projectKey, projectFixture)
      queryClient.setQueryData(pageKey, pageDetailFixture)
      queryClient.setQueryData(fontsKey, sampleFontFamilies)
      queryClient.setQueryData(glossaryKey('Manga Project'), glossaryFixture)
      useKoharuStore.setState({ selectedLayers: ['layer_1'] })

      const setBubbleSpeakerSpy = vi
        .spyOn(commands, 'setBubbleSpeaker')
        .mockResolvedValue(undefined as any)

      renderWithProviders(
        <Inspector
          project={projectFixture}
          page={pageSummaryFixture}
          layers={[sampleTextLayer]}
          selectedLayerIds={['layer_1']}
          onSelectLayers={() => {}}
        />,
      )

      expect(screen.getByText('Speaker')).toBeInTheDocument()
      expect(screen.getByText('None / Unassigned')).toBeInTheDocument()

      await userEvent.click(screen.getByText('None / Unassigned'))

      const yukiOption = await screen.findByText('ユキ (Yuki)')
      await userEvent.click(yukiOption)

      expect(setBubbleSpeakerSpy).toHaveBeenCalledWith(
        5, // expected_revision
        'layer_1',
        'person_yuki',
      )
    })

    it('dispatches guessPageSpeakers on Auto-detect button click', async () => {
      queryClient.setQueryData(projectKey, projectFixture)
      queryClient.setQueryData(pageKey, pageDetailFixture)
      queryClient.setQueryData(fontsKey, sampleFontFamilies)
      queryClient.setQueryData(glossaryKey('Manga Project'), glossaryFixture)
      useKoharuStore.setState({ selectedLayers: ['layer_1'] })

      const guessSpeakersSpy = vi
        .spyOn(commands, 'guessPageSpeakers')
        .mockResolvedValue(undefined as any)

      renderWithProviders(
        <Inspector
          project={projectFixture}
          page={pageSummaryFixture}
          layers={[sampleTextLayer]}
          selectedLayerIds={['layer_1']}
          onSelectLayers={() => {}}
        />,
      )

      const autoDetectBtn = screen.getByText('Auto-detect')
      await userEvent.click(autoDetectBtn)

      expect(guessSpeakersSpy).toHaveBeenCalledWith(
        5, // expected_revision
        'page_1',
      )
    })
  })

  describe('Character Persona Form State in GlossaryEntryRow', () => {
    it('renders persona configuration fields when entry kind is person', async () => {
      const onUpdate = vi.fn()
      const onDelete = vi.fn()

      renderWithProviders(
        <GlossaryEntryRow
          entry={personEntry}
          disabled={false}
          onUpdate={onUpdate}
          onDelete={onDelete}
        />,
      )

      await userEvent.click(screen.getByText('Character Persona'))

      expect(screen.getByText('Gender')).toBeInTheDocument()
      expect(screen.getByText('1st Person (I/Me)')).toBeInTheDocument()
      expect(screen.getByText('2nd Person (You)')).toBeInTheDocument()
      expect(screen.getByText('Tone')).toBeInTheDocument()
      expect(screen.getByText('Custom Persona Instructions')).toBeInTheDocument()
    })

    it('dispatches updated persona when tone is changed', async () => {
      const onUpdate = vi.fn()
      const onDelete = vi.fn()

      renderWithProviders(
        <GlossaryEntryRow
          entry={personEntry}
          disabled={false}
          onUpdate={onUpdate}
          onDelete={onDelete}
        />,
      )

      await userEvent.click(screen.getByText('Character Persona'))

      const toneInput = screen.getByDisplayValue('gentle')
      fireEvent.change(toneInput, { target: { value: 'calm and formal' } })
      fireEvent.blur(toneInput)

      expect(onUpdate).toHaveBeenCalledWith(
        expect.objectContaining({
          character: expect.objectContaining({
            id: 'person_yuki',
            tone: 'calm and formal',
          }),
        }),
      )
    })
  })

  describe('Glossary QA Linter Panel Verification', () => {
    it('runs lintGlossary on Verify QA click, renders issues, and navigates to target page and layer on click', async () => {
      queryClient.setQueryData(projectKey, projectFixture)
      queryClient.setQueryData(glossaryKey('Manga Project'), glossaryFixture)
      useKoharuStore.setState({ selectedPages: [], selectedLayers: [] })

      const mockIssues: GlossaryLintIssue[] = [
        {
          kind: 'forbidden_variant_used',
          severity: 'error',
          pageId: 'page_2',
          pageIndex: 1,
          pageLabel: 'Page 2',
          chapter: 'Chapter 1',
          layerId: 'layer_99',
          entryId: 'person_yuki',
          speakerId: 'person_yuki',
          expected: 'Yuki',
          actual: 'Yuki-chan',
          message: "Forbidden variant 'Yuki-chan' used instead of 'Yuki'",
        },
      ]

      vi.spyOn(commands, 'getProject').mockResolvedValue(projectFixture)
      vi.spyOn(commands, 'getGlossary').mockResolvedValue(glossaryFixture)
      const lintSpy = vi.spyOn(commands, 'lintGlossary').mockResolvedValue(mockIssues)
      vi.spyOn(commands, 'selectPage').mockResolvedValue(null)

      renderWithProviders(<GlossaryPanel />)

      const verifyBtn = await screen.findByRole('button', { name: 'Verify QA' })
      await userEvent.click(verifyBtn)

      expect(lintSpy).toHaveBeenCalledWith(null)
      const issueText = await screen.findByText(
        "Forbidden variant 'Yuki-chan' used instead of 'Yuki'",
      )
      expect(issueText).toBeInTheDocument()
      expect(screen.getByText('1 QA issues')).toBeInTheDocument()

      await userEvent.click(issueText)
      expect(useKoharuStore.getState().selectedPages).toEqual(['page_2'])
      expect(useKoharuStore.getState().selectedLayers).toEqual(['layer_99'])
    })
  })

  describe('Typesetting Preferences Font Bindings', () => {
    it('renders font family options for all 5 balloon shape categories', () => {
      queryClient.setQueryData(fontsKey, sampleFontFamilies)

      renderWithProviders(
        <TypesettingPreferences
          value={typesettingConfigFixture}
          onChange={() => {}}
        />,
      )

      expect(screen.getByText('Dialogue')).toBeInTheDocument()
      expect(screen.getByText('Shout')).toBeInTheDocument()
      expect(screen.getByText('Thought')).toBeInTheDocument()
      expect(screen.getByText('Whisper')).toBeInTheDocument()
      expect(screen.getByText('Narration')).toBeInTheDocument()
    })
  })

  describe('Locale Completeness Across All 9 Supported Locales', () => {
    const supportedLocales = [
      'en-US',
      'ja-JP',
      'ko-KR',
      'zh-CN',
      'zh-TW',
      'es-ES',
      'pt-BR',
      'ru-RU',
      'tr-TR',
    ] as const

    it.each(supportedLocales)('loads valid resource catalog for %s', (locale) => {
      const translation = (resources as any)[locale]?.translation
      expect(translation, `locale ${locale} must be loaded`).toBeDefined()
      expect(typeof translation).toBe('object')
      expect(translation.common).toBeDefined()
      expect(translation.glossary).toBeDefined()
    })
  })
})
