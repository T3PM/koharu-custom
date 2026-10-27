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
  name: 'Boundary Test Project',
  revision: 12,
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
  layer_count: 3,
  thumbnail_blob: null,
  ocr_coverage: 1,
  translation_coverage: 1,
}

const personEntry: GlossaryEntryView = {
  revision: 12,
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

const glossaryWithPersonFixture: GlossaryView = {
  revision: 12,
  enabled: true,
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

const sampleGroupLayer: Extract<Layer, { type: 'group' }> = {
  type: 'group',
  id: 'group_1',
  parent: 'page_1',
  role: 'text',
  visibility: { visible: true, opacity: 1 },
}

const pageDetailFixture = {
  id: 'page_1',
  label: 'Page 1',
  size: { width: 1200, height: 1800 },
  layers: [sampleGroupLayer, sampleTextLayer],
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
]

function renderWithProviders(ui: ReactNode) {
  return testingRender(
    <QueryClientProvider client={queryClient}>
      <TooltipProvider>{ui}</TooltipProvider>
    </QueryClientProvider>,
  )
}

describe('Tier 2: Boundary & Corner Cases E2E Tests', () => {
  describe('Persona Form Boundary & Whitespace Normalization', () => {
    it('normalizes whitespace-only tone input to null on blur', async () => {
      const onUpdate = vi.fn()
      renderWithProviders(
        <GlossaryEntryRow
          entry={personEntry}
          disabled={false}
          onUpdate={onUpdate}
          onDelete={() => {}}
        />,
      )

      await userEvent.click(screen.getByText('Character Persona'))
      const toneInput = screen.getByDisplayValue('gentle')
      fireEvent.change(toneInput, { target: { value: '   ' } })
      fireEvent.blur(toneInput)

      expect(onUpdate).toHaveBeenCalledWith(
        expect.objectContaining({
          character: expect.objectContaining({
            id: 'person_yuki',
            tone: null,
          }),
        }),
      )
    })

    it('normalizes whitespace-only 1st person pronoun to null on blur', async () => {
      const onUpdate = vi.fn()
      renderWithProviders(
        <GlossaryEntryRow
          entry={personEntry}
          disabled={false}
          onUpdate={onUpdate}
          onDelete={() => {}}
        />,
      )

      await userEvent.click(screen.getByText('Character Persona'))
      const firstPersonInput = screen.getByDisplayValue('私')
      fireEvent.change(firstPersonInput, { target: { value: '  \t  ' } })
      fireEvent.blur(firstPersonInput)

      expect(onUpdate).toHaveBeenCalledWith(
        expect.objectContaining({
          character: expect.objectContaining({
            id: 'person_yuki',
            firstPerson: null,
          }),
        }),
      )
    })

    it('normalizes whitespace-only custom instructions to null on blur', async () => {
      const onUpdate = vi.fn()
      renderWithProviders(
        <GlossaryEntryRow
          entry={personEntry}
          disabled={false}
          onUpdate={onUpdate}
          onDelete={() => {}}
        />,
      )

      await userEvent.click(screen.getByText('Character Persona'))
      const instructionsInput = screen.getByDisplayValue('Speaks politely')
      fireEvent.change(instructionsInput, { target: { value: '' } })
      fireEvent.blur(instructionsInput)

      expect(onUpdate).toHaveBeenCalledWith(
        expect.objectContaining({
          character: expect.objectContaining({
            id: 'person_yuki',
            customInstructions: null,
          }),
        }),
      )
    })

    it('handles person entry with initially null character profile gracefully', async () => {
      const entryWithoutChar: GlossaryEntryView = {
        ...personEntry,
        id: 'person_raw',
        character: null,
      }
      const onUpdate = vi.fn()
      renderWithProviders(
        <GlossaryEntryRow
          entry={entryWithoutChar}
          disabled={false}
          onUpdate={onUpdate}
          onDelete={() => {}}
        />,
      )

      await userEvent.click(screen.getByText('Character Persona'))
      const toneInput = screen.getByPlaceholderText('e.g. polite, playful')
      expect(toneInput).toHaveValue('')

      fireEvent.change(toneInput, { target: { value: 'energetic' } })
      fireEvent.blur(toneInput)

      expect(onUpdate).toHaveBeenCalledWith(
        expect.objectContaining({
          character: expect.objectContaining({
            id: 'person_raw',
            tone: 'energetic',
          }),
        }),
      )
    })
  })

  describe('Inspector Boundary Handling & Revision Rejections', () => {
    it('gracefully survives stale expected_revision rejection during setBalloonShape', async () => {
      queryClient.setQueryData(projectKey, projectFixture)
      queryClient.setQueryData(pageKey, pageDetailFixture)
      queryClient.setQueryData(fontsKey, sampleFontFamilies)
      queryClient.setQueryData(glossaryKey(projectFixture.name), glossaryWithPersonFixture)
      useKoharuStore.setState({ selectedLayers: ['layer_1'] })

      const setBalloonShapeSpy = vi
        .spyOn(commands, 'setBalloonShape')
        .mockRejectedValue(new Error('revision mismatch: expected 12 but got 13'))

      renderWithProviders(
        <Inspector
          project={projectFixture}
          page={pageSummaryFixture}
          layers={[sampleTextLayer]}
          selectedLayerIds={['layer_1']}
          onSelectLayers={() => {}}
        />,
      )

      const trigger = screen.getByText('Normal Speech')
      await userEvent.click(trigger)
      const shoutOption = await screen.findByText('Shout / Jagged')
      await userEvent.click(shoutOption)

      expect(setBalloonShapeSpy).toHaveBeenCalled()
      expect(screen.getAllByText('Normal Speech').length).toBeGreaterThan(0)
    })

    it('gracefully survives stale expected_revision rejection during setBubbleSpeaker', async () => {
      queryClient.setQueryData(projectKey, projectFixture)
      queryClient.setQueryData(pageKey, pageDetailFixture)
      queryClient.setQueryData(fontsKey, sampleFontFamilies)
      queryClient.setQueryData(glossaryKey(projectFixture.name), glossaryWithPersonFixture)
      useKoharuStore.setState({ selectedLayers: ['layer_1'] })

      const setBubbleSpeakerSpy = vi
        .spyOn(commands, 'setBubbleSpeaker')
        .mockRejectedValue(new Error('revision mismatch: stale revision'))

      renderWithProviders(
        <Inspector
          project={projectFixture}
          page={pageSummaryFixture}
          layers={[sampleTextLayer]}
          selectedLayerIds={['layer_1']}
          onSelectLayers={() => {}}
        />,
      )

      await userEvent.click(screen.getByText('None / Unassigned'))
      const yukiOption = await screen.findByText('ユキ (Yuki)')
      await userEvent.click(yukiOption)

      expect(setBubbleSpeakerSpy).toHaveBeenCalled()
      expect(screen.getAllByText('None / Unassigned').length).toBeGreaterThan(0)
    })

    it('renders disabled state when no layers are selected', () => {
      queryClient.setQueryData(projectKey, projectFixture)
      queryClient.setQueryData(pageKey, pageDetailFixture)
      queryClient.setQueryData(fontsKey, sampleFontFamilies)
      useKoharuStore.setState({ selectedLayers: [] })

      renderWithProviders(
        <Inspector
          project={projectFixture}
          page={pageSummaryFixture}
          layers={[sampleTextLayer]}
          selectedLayerIds={[]}
          onSelectLayers={() => {}}
        />,
      )

      expect(screen.getByTestId('type-inspector')).toHaveAttribute('aria-disabled', 'true')
    })

    it('handles multiple layers selection without throwing', () => {
      queryClient.setQueryData(projectKey, projectFixture)
      queryClient.setQueryData(pageKey, pageDetailFixture)
      queryClient.setQueryData(fontsKey, sampleFontFamilies)
      useKoharuStore.setState({ selectedLayers: ['layer_1', 'group_1'] })

      renderWithProviders(
        <Inspector
          project={projectFixture}
          page={pageSummaryFixture}
          layers={[sampleGroupLayer, sampleTextLayer]}
          selectedLayerIds={['layer_1', 'group_1']}
          onSelectLayers={() => {}}
        />,
      )

      expect(screen.getByText('Type')).toBeInTheDocument()
      expect(screen.getByText('Layers')).toBeInTheDocument()
    })

    it('handles non-existent selected layer ID gracefully', () => {
      queryClient.setQueryData(projectKey, projectFixture)
      queryClient.setQueryData(pageKey, pageDetailFixture)
      queryClient.setQueryData(fontsKey, sampleFontFamilies)
      useKoharuStore.setState({ selectedLayers: ['ghost_layer_999'] })

      renderWithProviders(
        <Inspector
          project={projectFixture}
          page={pageSummaryFixture}
          layers={[sampleTextLayer]}
          selectedLayerIds={['ghost_layer_999']}
          onSelectLayers={() => {}}
        />,
      )

      expect(screen.getByTestId('type-inspector')).toHaveAttribute('aria-disabled', 'true')
    })
  })

  describe('Typesetting Preferences Boundary Cases', () => {
    it('handles empty font families list gracefully', () => {
      const emptyConfig: TypesettingConfig = {
        font_families: [],
        dialogue_font: null,
        shout_font: null,
        thought_font: null,
        whisper_font: null,
        narration_font: null,
        font_size: 16,
        font_weight: 400,
      }
      queryClient.setQueryData(fontsKey, sampleFontFamilies)

      renderWithProviders(
        <TypesettingPreferences value={emptyConfig} onChange={() => {}} />,
      )

      expect(screen.getByText('Typesetting')).toBeInTheDocument()
      expect(
        screen.getByText('No default families. Koharu will use the system fallback.'),
      ).toBeInTheDocument()
    })

    it('renders all default fallback placeholders when specific fonts are null', () => {
      const nullFontsConfig: TypesettingConfig = {
        font_families: ['Wild Words'],
        dialogue_font: null,
        shout_font: null,
        thought_font: null,
        whisper_font: null,
        narration_font: null,
        font_size: 20,
        font_weight: 400,
      }
      queryClient.setQueryData(fontsKey, sampleFontFamilies)

      renderWithProviders(
        <TypesettingPreferences value={nullFontsConfig} onChange={() => {}} />,
      )

      const fallbackPlaceholders = screen.getAllByText('Default font fallback')
      expect(fallbackPlaceholders.length).toBeGreaterThanOrEqual(5)
    })
  })
})
