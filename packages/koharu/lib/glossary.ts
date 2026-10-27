import type {
  GlossaryEntryPatch,
  GlossaryEntryView,
  GlossaryKind,
  GlossaryView,
} from '@koharu/bridge/protocol'

export const glossaryKinds: readonly GlossaryKind[] = [
  'person',
  'place',
  'organization',
  'item',
  'ability',
  'term',
  'other',
]

export type GlossaryStateFilter = 'all' | 'translated' | 'untranslated' | 'disabled' | 'not-present'

export interface GlossaryFilters {
  query: string
  kind: GlossaryKind | 'all'
  state: GlossaryStateFilter
}

export function normalizeGlossaryTranslation(value: string): string | null {
  return value.trim() || null
}

export interface GlossarySettings {
  enabled: boolean
  preSubstitute: boolean
  postReplace: boolean
  caseSensitive: boolean
  wholeWord: boolean
  enablePersonas: boolean
  enableSpeakerTagging: boolean
  autoGuessSpeakers: boolean
}

export const DEFAULT_GLOSSARY_SETTINGS: GlossarySettings = {
  enabled: true,
  preSubstitute: true,
  postReplace: true,
  caseSensitive: false,
  wholeWord: false,
  enablePersonas: true,
  enableSpeakerTagging: true,
  autoGuessSpeakers: true,
}

const STORAGE_KEY = 'koharu_glossary_settings_v1'

export function loadGlossarySettings(): GlossarySettings {
  if (typeof window === 'undefined') return DEFAULT_GLOSSARY_SETTINGS
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY)
    if (raw) {
      return { ...DEFAULT_GLOSSARY_SETTINGS, ...JSON.parse(raw) }
    }
  } catch {}
  return DEFAULT_GLOSSARY_SETTINGS
}

export function saveGlossarySettings(settings: GlossarySettings): void {
  if (typeof window === 'undefined') return
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(settings))
  } catch {}
}

export interface GlossarySubstitutionResult {
  text: string
  tokenMap: Map<number, string>
}

export function substituteGlossaryTokens(
  sourceText: string,
  entries?: readonly (GlossaryEntryView | { source: string; translation: string | null; enabled: boolean })[] | null,
  options?: Partial<GlossarySettings>,
): GlossarySubstitutionResult {
  if (!sourceText || !entries || entries.length === 0) {
    return { text: sourceText, tokenMap: new Map() }
  }

  const sorted = [...entries]
    .filter((e) => e.enabled && e.translation && e.translation.trim().length > 0 && e.source.trim().length > 0)
    .sort((a, b) => b.source.trim().length - a.source.trim().length)

  if (sorted.length === 0) {
    return { text: sourceText, tokenMap: new Map() }
  }

  const tokenMap = new Map<number, string>()
  let result = sourceText
  const caseSensitive = options?.caseSensitive ?? false
  const wholeWord = options?.wholeWord ?? false

  for (const entry of sorted) {
    const source = entry.source.trim()
    const target = entry.translation!.trim()
    if (!source || !target) continue

    const escaped = source.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
    const useBoundary = wholeWord && /^\w/.test(source) && /\w$/.test(source)
    const pattern = useBoundary ? `\\b${escaped}\\b` : escaped
    const regex = new RegExp(pattern, caseSensitive ? 'g' : 'gi')

    if (regex.test(result)) {
      const idx = tokenMap.size
      tokenMap.set(idx, target)
      result = result.replace(regex, `__GLOSS_${idx}__`)
    }
  }

  return { text: result, tokenMap }
}

export function restoreGlossaryTokens(
  text: string,
  tokenMap: Map<number, string>,
): string {
  if (!text || tokenMap.size === 0) return text

  return text.replace(/__\s*GLOSS_(\d+)\s*__/gi, (match, digits) => {
    const idx = Number.parseInt(digits, 10)
    const replacement = tokenMap.get(idx)
    return replacement !== undefined ? replacement : match
  })
}

export function applyGlossaryReplacements(
  text: string,
  entries?: readonly (GlossaryEntryView | { source: string; translation: string | null; enabled: boolean })[] | null,
  options?: Partial<GlossarySettings>,
): string {
  if (!text || !entries || entries.length === 0) return text
  const sorted = [...entries]
    .filter((e) => e.enabled && e.translation && e.translation.trim().length > 0 && e.source.trim().length > 0)
    .sort((a, b) => b.source.trim().length - a.source.trim().length)

  let result = text
  const caseSensitive = options?.caseSensitive ?? false
  const wholeWord = options?.wholeWord ?? false

  for (const entry of sorted) {
    const source = entry.source.trim()
    const target = entry.translation!.trim()
    if (!source || !target) continue

    const escaped = source.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
    const useBoundary = wholeWord && /^\w/.test(source) && /\w$/.test(source)
    const pattern = useBoundary ? `\\b${escaped}\\b` : escaped
    result = result.replace(new RegExp(pattern, caseSensitive ? 'g' : 'gi'), target)
  }
  return result
}

export function filterGlossaryEntries(
  entries: readonly GlossaryEntryView[],
  filters: GlossaryFilters,
): GlossaryEntryView[] {
  const query = filters.query.trim().toLocaleLowerCase()
  return entries.filter((entry) => {
    if (filters.kind !== 'all' && entry.kind !== filters.kind) return false
    if (
      query &&
      !`${entry.source}\n${entry.translation ?? ''}`.toLocaleLowerCase().includes(query)
    ) {
      return false
    }
    switch (filters.state) {
      case 'translated':
        return Boolean(entry.translation)
      case 'untranslated':
        return !entry.translation
      case 'disabled':
        return !entry.enabled
      case 'not-present':
        return !entry.presentInLastScan
      default:
        return true
    }
  })
}

export function eligibleGlossaryTranslationIds(
  entries: readonly GlossaryEntryView[],
  selected?: ReadonlySet<string>,
): string[] {
  return entries
    .filter(
      (entry) =>
        entry.enabled && !entry.translation && (selected === undefined || selected.has(entry.id)),
    )
    .map((entry) => entry.id)
}

export function glossaryStatus(view: GlossaryView): 'unscanned' | 'available' | 'stale' {
  if (view.savedSourceFingerprint === null) return 'unscanned'
  return view.stale ? 'stale' : 'available'
}

export function parseGlossaryDocument(text: string): string {
  let value: unknown
  try {
    value = JSON.parse(text)
  } catch {
    throw new Error('The glossary file is malformed JSON.')
  }
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new Error('The glossary file must contain a JSON document.')
  }
  return text
}

function entryPatch(entry: GlossaryEntryPatch): GlossaryEntryPatch {
  return {
    source: entry.source,
    translation: entry.translation,
    kind: entry.kind,
    enabled: entry.enabled,
    character: entry.character,
    ...(entry.forbiddenVariants !== undefined
      ? { forbiddenVariants: entry.forbiddenVariants }
      : {}),
  }
}

interface UpdateQueueOptions<Owner> {
  initialRevision: number
  owner: Owner
  isOwnerCurrent: (owner: Owner) => boolean
  execute: (revision: number, id: string, patch: GlossaryEntryPatch) => Promise<GlossaryView>
  onResponse: (view: GlossaryView, current: boolean) => void | Promise<void>
}

interface PendingUpdate<Owner> {
  id: string
  patch: GlossaryEntryPatch
  owner: Owner
  generation: number
  resolve: (view: GlossaryView | undefined) => void
  reject: (error: unknown) => void
}

export interface GlossaryEntryUpdateQueue {
  enqueue: (id: string, patch: GlossaryEntryPatch) => Promise<GlossaryView | undefined>
  setRevision: (revision: number) => void
  cancel: () => void
}

export function createGlossaryEntryUpdateQueue<Owner>({
  initialRevision,
  owner,
  isOwnerCurrent,
  execute,
  onResponse,
}: UpdateQueueOptions<Owner>): GlossaryEntryUpdateQueue {
  let revision = initialRevision
  let running = false
  let active = true
  let generation = 0
  const pending: PendingUpdate<Owner>[] = []

  const run = async () => {
    if (running) return
    running = true
    while (pending.length > 0) {
      const update = pending.shift()!
      if (!active || !isOwnerCurrent(update.owner)) {
        update.resolve(undefined)
        continue
      }
      try {
        const response = await execute(revision, update.id, update.patch)
        revision = Math.max(revision, response.revision)
        const current = active && isOwnerCurrent(update.owner) && generation === update.generation
        await onResponse(response, current)
        update.resolve(response)
      } catch (error) {
        update.reject(error)
        for (const queued of pending.splice(0)) queued.reject(error)
        break
      }
    }
    running = false
  }

  return {
    enqueue(id, patch) {
      generation += 1
      const promise = new Promise<GlossaryView | undefined>((resolve, reject) => {
        pending.push({ id, patch: entryPatch(patch), owner, generation, resolve, reject })
      })
      void run()
      return promise
    },
    setRevision(nextRevision) {
      revision = Math.max(revision, nextRevision)
    },
    cancel() {
      active = false
      for (const update of pending.splice(0)) update.resolve(undefined)
    },
  }
}
