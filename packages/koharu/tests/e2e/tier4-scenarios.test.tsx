import { QueryClientProvider } from '@tanstack/react-query'
import { render as testingRender, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { ReactNode } from 'react'
import { describe, expect, it, vi } from 'vitest'

import { Inspector } from '@/components/editor/Inspector'
import { TypesettingPreferences } from '@/components/preferences/TypesettingPreferences'
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
  type CharacterProfile,
  type FontFamily,
  type GlossaryEntryView,
  type GlossaryView,
  type Layer,
  type PageSummary,
  type ProjectInfo,
  type TypesettingConfig,
} from '@koharu/bridge/protocol'
import { TooltipProvider } from '@koharu/ui/components/tooltip'

const projectFixture: ProjectInfo = {
  name: 'Manga Ep 10',
  revision: 42,
  active_page: 'page_ch10_01',
  can_undo: true,
  can_redo: false,
}

const pageSummaryFixture: PageSummary = {
  id: 'page_ch10_01',
  label: 'Chapter 10 - Page 1',
  chapter: 'Chapter 10',
  index: 0,
  dimensions: [1600, 2400],
  layer_count: 3,
  thumbnail_blob: null,
  ocr_coverage: 1,
  translation_coverage: 1,
}

const protagonistPersona: CharacterProfile = {
  id: 'person_taro',
  gender: 'male',
  firstPerson: '俺',
  secondPerson: 'お前',
  tone: 'heroic and determined',
  customInstructions: 'Speaks with confidence',
}

const mentorPersona: CharacterProfile = {
  id: 'person_sensei',
  gender: 'female',
  firstPerson: '私',
  secondPerson: 'あなた',
  tone: 'wise and calm',
  customInstructions: 'Ancient terminology',
}

const protagonistEntry: GlossaryEntryView = {
  revision: 42,
  id: 'person_taro',
  source: '太郎',
  translation: 'Taro',
  kind: 'person',
  enabled: true,
  confidence: 1.0,
  occurrenceCount: 15,
  examples: ['太郎は剣を構えた'],
  sourceOrigin: 'user',
  translationOrigin: 'user',
  presentInLastScan: true,
  character: protagonistPersona,
}

const mentorEntry: GlossaryEntryView = {
  revision: 42,
  id: 'person_sensei',
  source: '師匠',
  translation: 'Master',
  kind: 'person',
  enabled: true,
  confidence: 0.95,
  occurrenceCount: 10,
  examples: ['師匠は頷いた'],
  sourceOrigin: 'user',
  translationOrigin: 'user',
  presentInLastScan: true,
  character: mentorPersona,
}

const glossaryFixture: GlossaryView = {
  revision: 42,
  enabled: true,
  entries: [protagonistEntry, mentorEntry],
}

const taroLayer: Extract<Layer, { type: 'text' }> = {
  type: 'text',
  id: 'layer_taro_speech',
  parent: 'page_ch10_01',
  angle_degrees: 0,
  geometry: {
    points: [
      { x: 100, y: 100 },
      { x: 300, y: 100 },
      { x: 300, y: 250 },
      { x: 100, y: 250 },
    ],
  },
  visibility: { visible: true, opacity: 1 },
  content: {
    id: 'content_taro',
    source: { text: '俺は決して諦めない！', language: 'ja' },
    translation: { text: 'I will never give up!', language: 'en' },
    role: null,
    source_region: null,
  },
  typography: {
    preferred_font: 'CC Wild Words',
    font_weight: 700,
    font_style: 'normal',
    size: 26,
    auto_fit: true,
    color: [0, 0, 0, 255],
    stroke_color: [255, 255, 255, 255],
    stroke_width: 2,
    alignment: 'Center',
    writing_mode: null,
  },
  layout: 'paragraph',
  automatic_region: null,
  speaker: 'person_taro',
  balloon_shape: 'shout',
}

const senseiLayer: Extract<Layer, { type: 'text' }> = {
  type: 'text',
  id: 'layer_sensei_thought',
  parent: 'page_ch10_01',
  angle_degrees: 0,
  geometry: {
    points: [
      { x: 400, y: 300 },
      { x: 600, y: 300 },
      { x: 600, y: 450 },
      { x: 400, y: 450 },
    ],
  },
  visibility: { visible: true, opacity: 1 },
  content: {
    id: 'content_sensei',
    source: { text: '（よくぞここまで成長した...）', language: 'ja' },
    translation: { text: '(You have grown so much...)', language: 'en' },
    role: null,
    source_region: null,
  },
  typography: {
    preferred_font: 'Comic Talk',
    font_weight: 400,
    font_style: 'italic',
    size: 20,
    auto_fit: true,
    color: [30, 30, 30, 255],
    stroke_color: [255, 255, 255, 255],
    stroke_width: 1,
    alignment: 'Center',
    writing_mode: null,
  },
  layout: 'paragraph',
  automatic_region: null,
  speaker: 'person_sensei',
  balloon_shape: 'thought',
}

const sfxLayer: Extract<Layer, { type: 'text' }> = {
  type: 'text',
  id: 'layer_sfx_boom',
  parent: 'page_ch10_01',
  angle_degrees: 15,
  geometry: {
    points: [
      { x: 800, y: 800 },
      { x: 1100, y: 800 },
      { x: 1100, y: 1000 },
      { x: 800, y: 1000 },
    ],
  },
  visibility: { visible: true, opacity: 1 },
  content: {
    id: 'content_sfx',
    source: { text: 'ドドド', language: 'ja' },
    translation: { text: 'RUMBLE', language: 'en' },
    role: null,
    source_region: null,
  },
  typography: {
    preferred_font: 'Heavy Metal',
    font_weight: 900,
    font_style: 'normal',
    size: 48,
    auto_fit: false,
    color: [255, 0, 0, 255],
    stroke_color: [0, 0, 0, 255],
    stroke_width: 4,
    alignment: 'Center',
    writing_mode: null,
  },
  layout: 'paragraph',
  automatic_region: null,
  speaker: null,
  balloon_shape: null,
}

const pageDetailFixture = {
  id: 'page_ch10_01',
  label: 'Chapter 10 - Page 1',
  size: { width: 1600, height: 2400 },
  layers: [taroLayer, senseiLayer, sfxLayer],
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
      classifications: ['shout'],
      use_cases: ['shout'],
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
      classifications: ['thought'],
      use_cases: ['thought'],
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
      classifications: ['whisper'],
      use_cases: ['whisper'],
    },
    sources: ['system'],
    faces: [],
  },
]

function renderWithProviders(ui: ReactNode) {
  return testingRender(
    <QueryClientProvider client={queryClient}>
      <TooltipProvider>{ui}</TooltipProvider>
    </QueryClientProvider>,
  )
}

describe('Tier 4: Real-World Manga Application Scenarios', () => {
  describe('Scenario 1: Multi-Character Chapter Translation with Personas & Speaker Consistency', () => {
    it('manages dialogue layers across characters with persona and balloon consistency', async () => {
      queryClient.setQueryData(projectKey, projectFixture)
      queryClient.setQueryData(pageKey, pageDetailFixture)
      queryClient.setQueryData(fontsKey, sampleFontFamilies)
      queryClient.setQueryData(glossaryKey(projectFixture.name), glossaryFixture)
      useKoharuStore.setState({ selectedLayers: ['layer_taro_speech'] })

      const setBubbleSpeakerSpy = vi
        .spyOn(commands, 'setBubbleSpeaker')
        .mockResolvedValue(undefined as any)

      renderWithProviders(
        <Inspector
          project={projectFixture}
          page={pageSummaryFixture}
          layers={[taroLayer, senseiLayer, sfxLayer]}
          selectedLayerIds={['layer_taro_speech']}
          onSelectLayers={() => {}}
        />,
      )

      expect(screen.getAllByText('太郎 (Taro)').length).toBeGreaterThan(0)
      expect(screen.getAllByText('Shout / Jagged').length).toBeGreaterThan(0)

      const speakerTrigger = screen.getAllByText('太郎 (Taro)')[0]!
      await userEvent.click(speakerTrigger)
      const senseiOption = await screen.findByText('師匠 (Master)')
      await userEvent.click(senseiOption)

      expect(setBubbleSpeakerSpy).toHaveBeenCalledWith(
        42,
        'layer_taro_speech',
        'person_sensei',
      )
    })
  })

  describe('Scenario 2: Dense SFX & Onomatopoeia Action Layer Workflow', () => {
    it('allows assigning balloon shape to action SFX layer', async () => {
      queryClient.setQueryData(projectKey, projectFixture)
      queryClient.setQueryData(pageKey, pageDetailFixture)
      queryClient.setQueryData(fontsKey, sampleFontFamilies)
      queryClient.setQueryData(glossaryKey(projectFixture.name), glossaryFixture)
      useKoharuStore.setState({ selectedLayers: ['layer_sfx_boom'] })

      const setBalloonShapeSpy = vi
        .spyOn(commands, 'setBalloonShape')
        .mockResolvedValue(undefined as any)

      renderWithProviders(
        <Inspector
          project={projectFixture}
          page={pageSummaryFixture}
          layers={[taroLayer, senseiLayer, sfxLayer]}
          selectedLayerIds={['layer_sfx_boom']}
          onSelectLayers={() => {}}
        />,
      )

      const shapeTrigger = screen.getByText('Normal Speech')
      await userEvent.click(shapeTrigger)
      const shoutOption = await screen.findByText('Shout / Jagged')
      await userEvent.click(shoutOption)

      expect(setBalloonShapeSpy).toHaveBeenCalledWith(
        42,
        'layer_sfx_boom',
        'shout',
      )
    })
  })

  describe('Scenario 3: Rapid Concurrent User Edits & Queue Conflict Recovery', () => {
    it('handles consecutive edits without unmounting or crashing', async () => {
      queryClient.setQueryData(projectKey, projectFixture)
      queryClient.setQueryData(pageKey, pageDetailFixture)
      queryClient.setQueryData(fontsKey, sampleFontFamilies)
      queryClient.setQueryData(glossaryKey(projectFixture.name), glossaryFixture)
      useKoharuStore.setState({ selectedLayers: ['layer_sensei_thought'] })

      let callCount = 0
      const setBalloonShapeSpy = vi
        .spyOn(commands, 'setBalloonShape')
        .mockImplementation(async () => {
          callCount++
          if (callCount === 2) {
            throw new Error('stale revision')
          }
        })

      renderWithProviders(
        <Inspector
          project={projectFixture}
          page={pageSummaryFixture}
          layers={[taroLayer, senseiLayer, sfxLayer]}
          selectedLayerIds={['layer_sensei_thought']}
          onSelectLayers={() => {}}
        />,
      )

      expect(screen.getAllByText('Thought / Cloud').length).toBeGreaterThan(0)

      const firstTrigger = screen.getAllByText('Thought / Cloud')[0]!
      await userEvent.click(firstTrigger)
      const whisperOption = await screen.findByText('Whisper')
      await userEvent.click(whisperOption)

      const secondTrigger = screen.getAllByText('Thought / Cloud')[0]!
      await userEvent.click(secondTrigger)
      const narrationOption = await screen.findByText('Narration Box')
      await userEvent.click(narrationOption)

      expect(setBalloonShapeSpy).toHaveBeenCalledTimes(2)
      expect(screen.getAllByText('Thought / Cloud').length).toBeGreaterThan(0)
    })
  })

  describe('Scenario 5: Full Typesetting Preset Bindings Pass', () => {
    it('binds typography presets across dialogue, shout, thought, whisper, and narration', () => {
      const fullTypesettingConfig: TypesettingConfig = {
        font_families: ['Wild Words', 'CC Wild Words', 'Comic Talk', 'Noto Sans'],
        dialogue_font: 'Wild Words',
        shout_font: 'CC Wild Words',
        thought_font: 'Comic Talk',
        whisper_font: 'Noto Sans',
        narration_font: 'Wild Words',
        font_size: 22,
        font_weight: 400,
      }
      queryClient.setQueryData(fontsKey, sampleFontFamilies)

      renderWithProviders(
        <TypesettingPreferences
          value={fullTypesettingConfig}
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
})
