import { QueryClientProvider } from '@tanstack/react-query'
import { fireEvent, render as testingRender, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { ReactNode } from 'react'
import { describe, expect, it, vi } from 'vitest'

import { Inspector } from '@/components/editor/Inspector'
import { GlossaryEntryRow } from '@/components/editor/GlossaryEntryRow'
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
  name: 'Combination Test Manga',
  revision: 20,
  active_page: 'page_1',
  can_undo: true,
  can_redo: true,
}

const pageSummaryFixture: PageSummary = {
  id: 'page_1',
  label: 'Page 1',
  chapter: 'Chapter 3',
  index: 0,
  dimensions: [1200, 1800],
  layer_count: 2,
  thumbnail_blob: null,
  ocr_coverage: 1,
  translation_coverage: 1,
}

const renPersona: CharacterProfile = {
  id: 'person_ren',
  gender: 'male',
  firstPerson: '俺',
  secondPerson: 'お前',
  tone: 'rough and brash',
  customInstructions: 'Uses harsh language',
}

const renEntry: GlossaryEntryView = {
  revision: 20,
  id: 'person_ren',
  source: 'レン',
  translation: 'Ren',
  kind: 'person',
  enabled: true,
  confidence: 0.95,
  occurrenceCount: 5,
  examples: ['レンは叫んだ'],
  sourceOrigin: 'user',
  translationOrigin: 'user',
  presentInLastScan: true,
  character: renPersona,
}

const miaEntry: GlossaryEntryView = {
  revision: 20,
  id: 'person_mia',
  source: 'ミア',
  translation: 'Mia',
  kind: 'person',
  enabled: true,
  confidence: 0.99,
  occurrenceCount: 8,
  examples: ['ミアは微笑んだ'],
  sourceOrigin: 'user',
  translationOrigin: 'user',
  presentInLastScan: true,
  character: {
    id: 'person_mia',
    gender: 'female',
    firstPerson: '私',
    secondPerson: 'あなた',
    tone: 'polite',
    customInstructions: 'Speaks softly',
  },
}

const glossaryFixture: GlossaryView = {
  revision: 20,
  enabled: true,
  entries: [renEntry, miaEntry],
}

const layer1Shout: Extract<Layer, { type: 'text' }> = {
  type: 'text',
  id: 'layer_ren_shout',
  parent: 'page_1',
  angle_degrees: 0,
  geometry: {
    points: [
      { x: 50, y: 50 },
      { x: 200, y: 50 },
      { x: 200, y: 150 },
      { x: 50, y: 150 },
    ],
  },
  visibility: { visible: true, opacity: 1 },
  content: {
    id: 'content_ren',
    source: { text: '逃げるな！', language: 'ja' },
    translation: { text: "Don't run away!", language: 'en' },
    role: null,
    source_region: null,
  },
  typography: {
    preferred_font: 'CC Wild Words',
    font_weight: 700,
    font_style: 'normal',
    size: 28,
    auto_fit: true,
    color: [0, 0, 0, 255],
    stroke_color: [255, 255, 255, 255],
    stroke_width: 3,
    alignment: 'Center',
    writing_mode: null,
  },
  layout: 'paragraph',
  automatic_region: null,
  speaker: 'person_ren',
  balloon_shape: 'shout',
}

const layer2Whisper: Extract<Layer, { type: 'text' }> = {
  type: 'text',
  id: 'layer_mia_whisper',
  parent: 'page_1',
  angle_degrees: 0,
  geometry: {
    points: [
      { x: 300, y: 500 },
      { x: 450, y: 500 },
      { x: 450, y: 600 },
      { x: 300, y: 600 },
    ],
  },
  visibility: { visible: true, opacity: 1 },
  content: {
    id: 'content_mia',
    source: { text: '大丈夫ですよ...', language: 'ja' },
    translation: { text: "It's alright...", language: 'en' },
    role: null,
    source_region: null,
  },
  typography: {
    preferred_font: 'Noto Sans',
    font_weight: 300,
    font_style: 'italic',
    size: 18,
    auto_fit: true,
    color: [50, 50, 50, 255],
    stroke_color: [255, 255, 255, 255],
    stroke_width: 1,
    alignment: 'Center',
    writing_mode: null,
  },
  layout: 'paragraph',
  automatic_region: null,
  speaker: 'person_mia',
  balloon_shape: 'whisper',
}

const pageDetailFixture = {
  id: 'page_1',
  label: 'Page 1',
  size: { width: 1200, height: 1800 },
  layers: [layer1Shout, layer2Whisper],
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

describe('Tier 3: Cross-Feature Combination Tests', () => {
  it('combines shout balloon shape with Ren speaker selection in Inspector', async () => {
    queryClient.setQueryData(projectKey, projectFixture)
    queryClient.setQueryData(pageKey, pageDetailFixture)
    queryClient.setQueryData(fontsKey, sampleFontFamilies)
    queryClient.setQueryData(glossaryKey(projectFixture.name), glossaryFixture)
    useKoharuStore.setState({ selectedLayers: ['layer_ren_shout'] })

    const setBubbleSpeakerSpy = vi
      .spyOn(commands, 'setBubbleSpeaker')
      .mockResolvedValue(undefined as any)

    renderWithProviders(
      <Inspector
        project={projectFixture}
        page={pageSummaryFixture}
        layers={[layer1Shout, layer2Whisper]}
        selectedLayerIds={['layer_ren_shout']}
        onSelectLayers={() => {}}
      />,
    )

    expect(screen.getAllByText('レン (Ren)').length).toBeGreaterThan(0)
    expect(screen.getAllByText('Shout / Jagged').length).toBeGreaterThan(0)

    const speakerTrigger = screen.getAllByText('レン (Ren)')[0]!
    await userEvent.click(speakerTrigger)
    const miaOption = await screen.findByText('ミア (Mia)')
    await userEvent.click(miaOption)

    expect(setBubbleSpeakerSpy).toHaveBeenCalledWith(
      20,
      'layer_ren_shout',
      'person_mia',
    )
  })

  it('reflects whisper balloon shape and Mia speaker in second layer', async () => {
    queryClient.setQueryData(projectKey, projectFixture)
    queryClient.setQueryData(pageKey, pageDetailFixture)
    queryClient.setQueryData(fontsKey, sampleFontFamilies)
    queryClient.setQueryData(glossaryKey(projectFixture.name), glossaryFixture)
    useKoharuStore.setState({ selectedLayers: ['layer_mia_whisper'] })

    renderWithProviders(
      <Inspector
        project={projectFixture}
        page={pageSummaryFixture}
        layers={[layer1Shout, layer2Whisper]}
        selectedLayerIds={['layer_mia_whisper']}
        onSelectLayers={() => {}}
      />,
    )

    expect(screen.getAllByText('ミア (Mia)').length).toBeGreaterThan(0)
    expect(screen.getAllByText('Whisper').length).toBeGreaterThan(0)
  })

  it('sequences complete persona modification workflow across multiple attributes', async () => {
    const onUpdate = vi.fn()
    renderWithProviders(
      <GlossaryEntryRow
        entry={renEntry}
        disabled={false}
        onUpdate={onUpdate}
        onDelete={() => {}}
      />,
    )

    await userEvent.click(screen.getByText('Character Persona'))

    const toneInput = screen.getByDisplayValue('rough and brash')
    fireEvent.change(toneInput, { target: { value: 'calm and strategic' } })
    fireEvent.blur(toneInput)

    expect(onUpdate).toHaveBeenLastCalledWith(
      expect.objectContaining({
        character: expect.objectContaining({
          id: 'person_ren',
          tone: 'calm and strategic',
        }),
      }),
    )

    const firstPersonInput = screen.getByDisplayValue('俺')
    fireEvent.change(firstPersonInput, { target: { value: '僕' } })
    fireEvent.blur(firstPersonInput)

    expect(onUpdate).toHaveBeenLastCalledWith(
      expect.objectContaining({
        character: expect.objectContaining({
          id: 'person_ren',
          firstPerson: '僕',
        }),
      }),
    )

    const secondPersonInput = screen.getByDisplayValue('お前')
    fireEvent.change(secondPersonInput, { target: { value: '君' } })
    fireEvent.blur(secondPersonInput)

    expect(onUpdate).toHaveBeenLastCalledWith(
      expect.objectContaining({
        character: expect.objectContaining({
          id: 'person_ren',
          secondPerson: '君',
        }),
      }),
    )
  })

  it('updates typesetting preferences font presets and propagates change', async () => {
    const onChange = vi.fn()
    const currentConfig: TypesettingConfig = {
      font_families: ['Wild Words', 'Noto Sans'],
      dialogue_font: 'Wild Words',
      shout_font: 'CC Wild Words',
      thought_font: null,
      whisper_font: 'Noto Sans',
      narration_font: null,
      font_size: 22,
      font_weight: 400,
    }
    queryClient.setQueryData(fontsKey, sampleFontFamilies)

    renderWithProviders(
      <TypesettingPreferences value={currentConfig} onChange={onChange} />,
    )

    const clearShoutBtn = screen.getByRole('button', { name: 'Clear shout font' })
    await userEvent.click(clearShoutBtn)

    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({
        shout_font: null,
      }),
    )
  })
})
