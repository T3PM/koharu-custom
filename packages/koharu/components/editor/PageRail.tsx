'use client'

import { useQuery } from '@tanstack/react-query'
import { observeElementRect, useVirtualizer } from '@tanstack/react-virtual'
import {
  ChevronDown,
  ChevronRight,
  Download,
  FilePlus2,
  Folder,
  FolderOpen,
  FolderPlus,
  Languages,
  Plus,
  LoaderCircle,
  MoreHorizontal,
  Play,
  Search,
  Settings,
  Sparkles,
  Trash2,
} from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { ModelPicker } from '@/components/controls/ModelPicker'
import { OutputPicker, type OutputDraft } from '@/components/controls/OutputPicker'
import {
  availableModels,
  SelectorOption,
  SelectorPanel,
  SelectorRow,
} from '@/components/editor/InferenceControl'
import { ResourceMonitor } from '@/components/editor/ResourceMonitor'
import { ProjectTrashDialog } from '@/components/editor/ProjectTrashDialog'
import { call, refreshTranslationModels, savePreferences } from '@/lib/backend'
import { applyGlossaryReplacements } from '@/lib/glossary'
import {
  glossaryKey,
  pageKey,
  pagesKey,
  preparedPageKey,
  projectKey,
  queryClient,
  refresh,
  useGlossary,
  useImportPages,
  usePage,
  usePages,
  useProject,
} from '@/lib/queries'
import { pipelineStages, receivePreferences, useKoharuStore } from '@/lib/store'
import { modelKey } from '@/lib/translation'
import { prefetchCanvasPages, showCanvasPage } from '@koharu/bridge/canvas'
import {
  commands,
  type CanvasPagePreparation,
  type Layer,
  type ModelSelection,
  type Page,
  type PageImportSource,
  type PageSummary,
  type ProjectInfo,
  type Stage,
} from '@koharu/bridge/protocol'
import { Button } from '@koharu/ui/components/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@koharu/ui/components/dialog'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from '@koharu/ui/components/dropdown-menu'
import { Input } from '@koharu/ui/components/input'
import { Popover, PopoverContent, PopoverTrigger } from '@koharu/ui/components/popover'
import { ScrollArea } from '@koharu/ui/components/scroll-area'
import { Tooltip, TooltipContent, TooltipTrigger } from '@koharu/ui/components/tooltip'
import { cn } from '@koharu/ui/lib/utils'

const emptyPages: PageSummary[] = []

interface IntentPrefetchState {
  project: string
  revision: number
  pages: Set<string>
}

export function PageRail() {
  const { t } = useTranslation()
  const pages = usePages().data ?? emptyPages
  const active = usePage().data?.id ?? null
  const selected = useKoharuStore((state) => state.selectedPages)
  const selectPages = useKoharuStore((state) => state.selectPages)
  const selectLayers = useKoharuStore((state) => state.selectLayers)
  const setSettingsOpen = useKoharuStore((state) => state.setSettingsOpen)
  const { importPages, importing } = useImportPages()
  const anchor = useRef<number | null>(null)
  const selectionRequest = useRef(0)
  const intentPrefetch = useRef<IntentPrefetchState | null>(null)
  const [dragged, setDragged] = useState<string | null>(null)
  const [query, setQuery] = useState('')
  const [renaming, setRenaming] = useState<PageSummary | null>(null)
  const [renameValue, setRenameValue] = useState('')
  const [chapterModalPage, setChapterModalPage] = useState<PageSummary | null>(null)
  const [chapterInput, setChapterInput] = useState('')
  const [collapsedChapters, setCollapsedChapters] = useState<Set<string>>(new Set())
  const project = useProject().data
  const foldersStorageKey = project?.name ? `koharu:custom-folders:${project.name}` : null

  const [customFolders, setCustomFolders] = useState<string[]>(() => {
    if (typeof window === 'undefined') return []
    const key = project?.name ? `koharu:custom-folders:${project.name}` : null
    if (!key) return []
    try {
      const stored = window.localStorage.getItem(key)
      return stored ? JSON.parse(stored) : []
    } catch {
      return []
    }
  })

  useEffect(() => {
    if (!foldersStorageKey || typeof window === 'undefined') return
    try {
      const stored = window.localStorage.getItem(foldersStorageKey)
      if (stored) {
        setCustomFolders(JSON.parse(stored))
      }
    } catch {}
  }, [foldersStorageKey])

  const persistFolders = (folders: string[]) => {
    setCustomFolders(folders)
    if (foldersStorageKey && typeof window !== 'undefined') {
      try {
        window.localStorage.setItem(foldersStorageKey, JSON.stringify(folders))
      } catch {}
    }
  }

  const [trashOpen, setTrashOpen] = useState(false)
  const [folderToDelete, setFolderToDelete] = useState<string | null>(null)
  const [deletingFolderInProgress, setDeletingFolderInProgress] = useState(false)

  const handleImportToFolder = (chapterName: string, source: PageImportSource = 'files') => {
    importPages(source, chapterName)
  }

  const handleDeleteFolderChoice = async (folder: string, deletePages: boolean) => {
    setDeletingFolderInProgress(true)
    try {
      await call(commands.deleteFolder, folder, deletePages)
      const next = customFolders.filter((f) => f !== folder)
      persistFolders(next)
      setFolderToDelete(null)
      await refresh(projectKey, pagesKey, pageKey)
    } finally {
      setDeletingFolderInProgress(false)
    }
  }

  const handleDeleteSelectedPages = async () => {
    if (selected.length === 0) return
    const ids = [...selected]
    await call(commands.deletePages, ids)
    selectPages([])
    await refresh(projectKey, pagesKey, pageKey)
  }

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Delete' || e.key === 'Backspace') {
        const activeEl = document.activeElement
        if (
          activeEl &&
          (activeEl.tagName === 'INPUT' ||
            activeEl.tagName === 'TEXTAREA' ||
            (activeEl as HTMLElement).isContentEditable)
        ) {
          return
        }
        if (selected.length > 0) {
          e.preventDefault()
          void handleDeleteSelectedPages()
        }
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [selected])
  const [newFolderOpen, setNewFolderOpen] = useState(false)
  const [newFolderName, setNewFolderName] = useState('')
  const [renamingFolder, setRenamingFolder] = useState<string | null>(null)
  const [renameFolderValue, setRenameFolderValue] = useState('')
  const [exportingChapter, setExportingChapter] = useState<string | null>(null)
  const [translatingChapter, setTranslatingChapter] = useState<{
    name: string
    current: number
    total: number
  } | null>(null)
  const glossary = useGlossary(project?.name).data
  const preferences = useKoharuStore((state) => state.preferences)
  const normalized = query.trim().toLocaleLowerCase()
  const visiblePages = useMemo(
    () =>
      pages
        .map((page, index) => ({ page, index }))
        .filter(({ page }) => !normalized || page.label.toLocaleLowerCase().includes(normalized)),
    [normalized, pages],
  )
  const pageList = useRef<HTMLDivElement>(null)
  const pageVirtualizer = useVirtualizer({
    count: visiblePages.length,
    getScrollElement: () => pageList.current,
    getItemKey: (index) => visiblePages[index]?.page.id ?? index,
    estimateSize: () => 72,
    gap: 2,
    overscan: 6,
    initialRect: { width: 240, height: 600 },
    observeElementRect: (instance, callback) =>
      observeElementRect(instance, (rect) =>
        callback({ width: rect.width || 240, height: rect.height || 600 }),
      ),
  })

  useEffect(
    () => () => {
      selectionRequest.current += 1
    },
    [],
  )

  const select = (index: number, additive: boolean, range: boolean) => {
    const page = pages[index]
    if (!page) return
    let next: string[]
    if (range && anchor.current !== null) {
      const start = Math.min(anchor.current, index)
      const end = Math.max(anchor.current, index)
      const rangeIds = pages.slice(start, end + 1).map((item) => item.id)
      next = additive ? [...new Set([...selected, ...rangeIds])] : rangeIds
    } else if (additive) {
      next = selected.includes(page.id)
        ? selected.filter((id) => id !== page.id)
        : [...selected, page.id]
      anchor.current = index
    } else {
      next = [page.id]
      anchor.current = index
    }
    const previousProject = queryClient.getQueryData<ProjectInfo | null>(projectKey)
    const previousPage = queryClient.getQueryData<Page | null>(pageKey)
    const prepared = queryClient.getQueryData<CanvasPagePreparation>(preparedPageKey(page.id))
    const activated = showCanvasPage(page.id, previousProject?.revision ?? null)
    const request = ++selectionRequest.current
    const synchronize = () => {
      if (selectionRequest.current !== request) return
      if (activated && previousProject && prepared?.revision === previousProject.revision) {
        queryClient.setQueryData(projectKey, { ...previousProject, active_page: page.id })
        queryClient.setQueryData(pageKey, prepared.page)
      }
      selectPages(next)
      selectLayers([])
      void call(commands.selectPage, page.id)
        .then((selection) => {
          if (selectionRequest.current !== request) return
          queryClient.setQueryData(projectKey, selection.project)
          queryClient.setQueryData(pageKey, selection.page)
        })
        .catch(() => {
          if (selectionRequest.current !== request) return
          if (queryClient.getQueryData<ProjectInfo | null>(projectKey)?.active_page === page.id) {
            queryClient.setQueryData(projectKey, previousProject)
            queryClient.setQueryData(pageKey, previousPage)
          }
        })
    }
    if (activated) {
      requestAnimationFrame(() => window.setTimeout(synchronize, 0))
    } else {
      synchronize()
    }
  }

  const prefetchOnIntent = (page: string) => {
    const project = queryClient.getQueryData<ProjectInfo | null>(projectKey)
    if (!project || project.active_page === page) return
    let state = intentPrefetch.current
    if (!state || state.project !== project.name || state.revision !== project.revision) {
      state = { project: project.name, revision: project.revision, pages: new Set() }
      intentPrefetch.current = state
    }
    const prepared = queryClient.getQueryData<CanvasPagePreparation>(preparedPageKey(page))
    if (prepared?.revision === project.revision || state.pages.has(page)) return
    state.pages.add(page)
    void prefetchCanvasPages([page])
      .then((pages) => {
        const current = queryClient.getQueryData<ProjectInfo | null>(projectKey)
        const preparedPage = pages.find(
          (candidate) => candidate.page.id === page && candidate.revision === project.revision,
        )
        if (
          preparedPage &&
          current?.name === project.name &&
          current.revision === preparedPage.revision
        ) {
          queryClient.setQueryData(preparedPageKey(page), preparedPage)
          return
        }
        if (intentPrefetch.current === state) state.pages.delete(page)
      })
      .catch(() => {
        if (intentPrefetch.current === state) state.pages.delete(page)
      })
  }

  const submitSetChapter = () => {
    const page = chapterModalPage
    if (!page) {
      setChapterModalPage(null)
      return
    }
    const ch = chapterInput.trim() || null
    void call(commands.setPageChapter, page.id, ch)
      .then(() => refresh(projectKey, pagesKey, pageKey))
      .catch(() => undefined)
      .finally(() => {
        setChapterModalPage(null)
        setChapterInput('')
      })
  }

  const deletePage = (page: string) =>
    void call(commands.deletePages, [page])
      .then(() => {
        selectPages(selected.filter((selectedPage) => selectedPage !== page))
        if (active === page) selectLayers([])
        return refresh(projectKey, pagesKey, pageKey)
      })
      .catch(() => undefined)

  const openRename = (page: PageSummary) => {
    setRenaming(page)
    setRenameValue(page.label)
  }

  const chapterNames = useMemo(() => {
    const names = new Set<string>()
    for (const p of pages) {
      if (p.chapter?.trim()) names.add(p.chapter.trim())
    }
    for (const f of customFolders) {
      if (f.trim()) names.add(f.trim())
    }
    return Array.from(names).sort()
  }, [pages, customFolders])

  const { chapterGroups, unassignedPages } = useMemo(() => {
    const map = new Map<string, { page: PageSummary; index: number }[]>()
    for (const folder of customFolders) {
      if (folder.trim()) map.set(folder.trim(), [])
    }
    const unassigned: { page: PageSummary; index: number }[] = []
    for (const item of visiblePages) {
      const ch = item.page.chapter?.trim()
      if (ch) {
        const list = map.get(ch) ?? []
        list.push(item)
        map.set(ch, list)
      } else {
        unassigned.push(item)
      }
    }
    const groups = Array.from(map.entries())
      .map(([name, items]) => ({ name, items }))
      .sort((a, b) => a.name.localeCompare(b.name, undefined, { numeric: true }))
    return { chapterGroups: groups, unassignedPages: unassigned }
  }, [visiblePages, customFolders])

  const toggleChapterCollapse = (name: string) => {
    setCollapsedChapters((prev) => {
      const next = new Set(prev)
      if (next.has(name)) next.delete(name)
      else next.add(name)
      return next
    })
  }

  const handleExportChapter = (chapterName: string, format: 'png' | 'cbz' | 'psd' = 'png') => {
    setExportingChapter(chapterName)
    void call(commands.exportChapter, chapterName, format)
      .catch((err) => console.error('Failed to export chapter:', err))
      .finally(() => setExportingChapter(null))
  }

  const handleTranslateChapter = async (chapterName: string) => {
    const chapterPages = pages.filter((p) => p.chapter === chapterName)
    if (chapterPages.length === 0) return

    setTranslatingChapter({ name: chapterName, current: 0, total: chapterPages.length })
    const initialActive = active
    const targetLang = preferences?.pipeline?.translation?.target_language || 'en'
    const cleanTarget = targetLang.split('-')[0]?.toLowerCase() || 'en'

    try {
      for (let i = 0; i < chapterPages.length; i++) {
        const pageSummary = chapterPages[i]!
        setTranslatingChapter({ name: chapterName, current: i + 1, total: chapterPages.length })

        try {
          const selection = await call(commands.selectPage, pageSummary.id)
          const pageDetail = selection.page
          const textLayers = (pageDetail.layers || []).filter(
            (l): l is Layer & { type: 'text' } => l.type === 'text',
          )

          let pageUpdated = false
          for (const layer of textLayers) {
            const sourceText = layer.content.source?.text || layer.content.translation?.text || ''
            if (!sourceText.trim()) continue

            try {
              const endpoint = `https://translate.googleapis.com/translate_a/single?client=gtx&sl=auto&tl=${encodeURIComponent(cleanTarget)}&dt=t&q=${encodeURIComponent(sourceText)}`
              const res = await fetch(endpoint)
              if (!res.ok) continue
              const data = await res.json()
              let translated = ''
              if (Array.isArray(data) && Array.isArray(data[0])) {
                translated = data[0].map((segment: any) => segment[0] || '').join('')
              }
              if (translated) {
                const finalTranslated =
                  glossary?.enabled && glossary.entries.length > 0
                    ? applyGlossaryReplacements(translated, glossary.entries)
                    : translated
                await call(commands.setTranslation, layer.id, finalTranslated)
                pageUpdated = true
              }
            } catch (err) {
              console.error(`Failed to translate layer ${layer.id}:`, err)
            }
          }

          if (pageUpdated) {
            await call(commands.prepareCanvasPage, pageSummary.id)
          }
        } catch (err) {
          console.error(`Failed to translate page ${pageSummary.id}:`, err)
        }
      }
    } finally {
      if (initialActive) {
        try {
          await call(commands.selectPage, initialActive)
        } catch {}
      }
      await refresh(projectKey, pagesKey, pageKey)
      setTranslatingChapter(null)
    }
  }

  const submitCreateFolder = async () => {
    const name = newFolderName.trim()
    if (!name) return
    const next = Array.from(new Set([...customFolders, name]))
    persistFolders(next)
    if (selected.length > 0) {
      await Promise.all(
        selected.map((id) => call(commands.setPageChapter, id, name).catch(() => undefined)),
      )
      await refresh(projectKey, pagesKey, pageKey)
    }
    setNewFolderOpen(false)
    setNewFolderName('')
  }

  const openRenameFolder = (folder: string) => {
    setRenamingFolder(folder)
    setRenameFolderValue(folder)
  }

  const submitRenameFolder = async () => {
    const oldName = renamingFolder
    const newName = renameFolderValue.trim()
    if (!oldName || !newName || oldName === newName) {
      setRenamingFolder(null)
      return
    }
    const affected = pages.filter((p) => p.chapter === oldName)
    await Promise.all(affected.map((p) => call(commands.setPageChapter, p.id, newName)))
    const next = customFolders.map((f) => (f === oldName ? newName : f))
    persistFolders(next)
    setRenamingFolder(null)
    await refresh(projectKey, pagesKey, pageKey)
  }

// deleteFolder replaced by handleDeleteFolderChoice

  const closeRename = () => {
    setRenaming(null)
    setRenameValue('')
  }

  const submitRename = () => {
    const page = renaming
    const label = renameValue.trim()
    if (!page || !label || label === page.label) {
      closeRename()
      return
    }
    void call(commands.renamePage, page.id, label)
      .then(() => refresh(projectKey, pagesKey, pageKey))
      .catch(() => undefined)
      .finally(closeRename)
  }

  return (
    <>
      <aside className='flex h-full min-h-0 flex-col bg-[var(--surface-sidebar)]'>
        <header className='flex h-10 shrink-0 items-center justify-between px-2.5'>
          <div className='flex min-w-0 items-center gap-2'>
            <h2 className='text-[11px] font-semibold'>{t('navigator.pages')}</h2>
            <span className='rounded-full bg-primary/[0.07] px-1.5 py-0.5 text-[9px] text-muted-foreground tabular-nums'>
              {pages.length}
            </span>
          </div>
          <div className='flex items-center gap-1.5'>
            <Tooltip>
              <TooltipTrigger
                render={
                  <Button
                    type='button'
                    variant='ghost'
                    size='icon-xs'
                    aria-label={t('navigator.newFolder', 'New Chapter Folder')}
                    title={t('navigator.newFolder', 'New Chapter Folder')}
                    className='size-7 text-muted-foreground hover:bg-foreground/[0.06] hover:text-foreground'
                    onClick={() => {
                      setNewFolderName('')
                      setNewFolderOpen(true)
                    }}
                  >
                    <FolderPlus className='size-3.5' />
                  </Button>
                }
              />
              <TooltipContent>{t('navigator.newFolder', 'New Chapter Folder')}</TooltipContent>
            </Tooltip>
            {selected.length > 1 && (
              <div className='flex items-center gap-1.5'>
                <span
                  role='status'
                  aria-live='polite'
                  className='text-[9px] text-muted-foreground tabular-nums'
                >
                  {t('navigator.selected', { count: selected.length })}
                </span>
                <Button
                  type='button'
                  variant='destructive'
                  size='xs'
                  className='h-5 gap-1 px-1.5 text-[9px]'
                  onClick={handleDeleteSelectedPages}
                  title={t('navigator.deleteSelected', {
                    count: selected.length,
                    defaultValue: `Delete ${selected.length} pages`,
                  })}
                >
                  <Trash2 className='size-2.5' />
                  {t('navigator.delete', 'Delete')}
                </Button>
              </div>
            )}
          </div>
        </header>

        {pages.length > 0 && (
          <div className='border-b px-2 py-1.5'>
            <label className='flex h-6 items-center gap-1.5 rounded-md border border-input bg-background/70 px-1.5'>
              <Search className='size-3 text-muted-foreground' />
              <Input
                value={query}
                aria-label={t('navigator.filter')}
                placeholder={t('navigator.filter')}
                className='h-5 min-w-0 border-0 bg-transparent p-0 text-[11px] shadow-none focus-visible:ring-0 md:text-[11px]'
                onChange={(event) => setQuery(event.currentTarget.value)}
              />
            </label>
          </div>
        )}

        {pages.length === 0 && chapterGroups.length === 0 ? (
          <div className='flex min-h-0 flex-1 flex-col items-center justify-center px-4 text-center'>
            <p className='text-[11px] font-medium'>{t('navigator.emptyTitle')}</p>
            <p className='mt-1 text-[10px] leading-4 text-muted-foreground'>
              {t('navigator.emptyDescription')}
            </p>
            <div className='mt-3'>
              <PageImportMenu importing={importing} onImport={importPages} />
            </div>
          </div>
        ) : visiblePages.length === 0 && chapterGroups.length === 0 ? (
          <div className='grid flex-1 place-items-center px-4 text-center text-[10px] text-muted-foreground'>
            {t('navigator.noResults')}
          </div>
        ) : (
          chapterGroups.length > 0 ? (
            <ScrollArea className='min-h-0 flex-1' viewportClassName='px-1.5 py-1.5'>
              <div className='space-y-2'>
                {chapterGroups.map((group) => {
                  const isCollapsed = collapsedChapters.has(group.name)
                  return (
                    <div
                      key={group.name}
                      className='overflow-hidden rounded-xl border border-border/50 bg-background/50 shadow-xs'
                    >
                      <div
                        className='flex h-8 cursor-pointer select-none items-center justify-between bg-muted/40 px-2 py-1 transition-colors hover:bg-muted/70'
                        onClick={() => toggleChapterCollapse(group.name)}
                        onDragOver={(e) => e.preventDefault()}
                        onDrop={(e) => {
                          e.preventDefault()
                          if (dragged) {
                            void call(commands.setPageChapter, dragged, group.name)
                              .then(() => refresh(projectKey, pagesKey, pageKey))
                          }
                        }}
                      >
                        <div className='flex min-w-0 items-center gap-1.5'>
                          {isCollapsed ? (
                            <ChevronRight className='size-3.5 shrink-0 text-muted-foreground' />
                          ) : (
                            <ChevronDown className='size-3.5 shrink-0 text-muted-foreground' />
                          )}
                          {isCollapsed ? (
                            <Folder className='size-3.5 shrink-0 text-muted-foreground' />
                          ) : (
                            <FolderOpen className='size-3.5 shrink-0 text-primary' />
                          )}
                          <span className='truncate text-[11px] font-semibold'>{group.name}</span>
                          <span className='rounded-full bg-primary/[0.08] px-1 text-[8px] font-medium text-muted-foreground tabular-nums'>
                            {group.items.length}
                          </span>
                          {translatingChapter?.name === group.name && (
                            <span
                              role='status'
                              aria-live='polite'
                              className='flex items-center gap-1 text-[9px] font-medium text-primary'
                            >
                              <LoaderCircle className='size-3 animate-spin' />
                              <span>
                                {translatingChapter.current}/{translatingChapter.total}
                              </span>
                            </span>
                          )}
                        </div>
                        <div className='flex items-center gap-0.5' onClick={(e) => e.stopPropagation()}>
                          <ChapterProcessPopover
                            chapterName={group.name}
                            pages={group.items.map((i) => i.page)}
                            disabled={translatingChapter?.name === group.name}
                          />
                          <Button
                            type='button'
                            variant='ghost'
                            size='icon-xs'
                            disabled={
                              group.items.length === 0 ||
                              exportingChapter === group.name ||
                              translatingChapter?.name === group.name
                            }
                            aria-label={t('navigator.exportChapter', 'Export PNG')}
                            title={t('navigator.exportChapter', 'Export Folder PNGs')}
                            className='size-6 text-muted-foreground hover:bg-foreground/[0.08] hover:text-foreground'
                            onClick={() => handleExportChapter(group.name, 'png')}
                          >
                            {exportingChapter === group.name ? (
                              <LoaderCircle className='size-3 animate-spin text-primary' />
                            ) : (
                              <Download className='size-3' />
                            )}
                          </Button>
                          <DropdownMenu>
                            <DropdownMenuTrigger
                              render={
                                <Button
                                  variant='ghost'
                                  size='icon-xs'
                                  aria-label={t('navigator.folderActions', 'Folder actions')}
                                  className='size-6 text-muted-foreground hover:bg-foreground/[0.08]'
                                />
                              }
                            >
                              <MoreHorizontal className='size-3' />
                            </DropdownMenuTrigger>
                            <DropdownMenuContent align='end'>
                              <DropdownMenuItem
                                disabled={group.items.length === 0 || translatingChapter !== null}
                                onClick={() => handleTranslateChapter(group.name)}
                              >
                                <Languages className='size-3.5' />{' '}
                                {t('navigator.translateChapter', 'Translate Chapter')}
                              </DropdownMenuItem>
                              <DropdownMenuItem onClick={() => handleImportToFolder(group.name, 'files')}>
                                <FilePlus2 className='size-3.5' /> {t('navigator.importFilesToFolder', 'Import files to this folder')}
                              </DropdownMenuItem>
                              <DropdownMenuItem onClick={() => handleImportToFolder(group.name, 'folder')}>
                                <FolderOpen className='size-3.5' /> {t('navigator.importFolderToFolder', 'Import folder to this chapter')}
                              </DropdownMenuItem>
                              <DropdownMenuSeparator />
                              <DropdownMenuItem onClick={() => handleExportChapter(group.name, 'png')}>
                                <Download /> {t('navigator.exportPng', 'Export as PNG')}
                              </DropdownMenuItem>
                              <DropdownMenuItem onClick={() => handleExportChapter(group.name, 'cbz')}>
                                <Download /> {t('navigator.exportCbz', 'Export as CBZ')}
                              </DropdownMenuItem>
                              <DropdownMenuItem onClick={() => handleExportChapter(group.name, 'psd')}>
                                <Download /> {t('navigator.exportPsd', 'Export as PSD')}
                              </DropdownMenuItem>
                              <DropdownMenuSeparator />
                              <DropdownMenuItem onClick={() => openRenameFolder(group.name)}>
                                {t('navigator.renameFolder', 'Rename Folder')}
                              </DropdownMenuItem>
                              <DropdownMenuItem
                                variant='destructive'
                                onClick={() => setFolderToDelete(group.name)}
                              >
                                <Trash2 /> {t('navigator.deleteFolder', 'Delete Folder')}
                              </DropdownMenuItem>
                            </DropdownMenuContent>
                          </DropdownMenu>
                        </div>
                      </div>

                      {!isCollapsed && (
                        <div
                          className='min-h-8 space-y-1 bg-background/20 p-1'
                          onDragOver={(e) => e.preventDefault()}
                          onDrop={(e) => {
                            e.preventDefault()
                            if (dragged) {
                              void call(commands.setPageChapter, dragged, group.name)
                                .then(() => refresh(projectKey, pagesKey, pageKey))
                            }
                          }}
                        >
                          {group.items.length === 0 ? (
                            <div className='flex flex-col items-center justify-center py-3 text-center'>
                              <p className='text-[9px] text-muted-foreground/60 italic'>
                                {t('navigator.emptyFolder', 'Drop pages here or import')}
                              </p>
                              <div className='mt-1 flex items-center justify-center gap-1'>
                                <Button
                                  type='button'
                                  variant='ghost'
                                  size='xs'
                                  className='h-6 text-[10px] text-primary hover:bg-primary/10'
                                  onClick={() => handleImportToFolder(group.name, 'files')}
                                >
                                  <FilePlus2 className='mr-1 size-3' />
                                  {t('navigator.importFiles', 'Import files')}
                                </Button>
                                <Button
                                  type='button'
                                  variant='ghost'
                                  size='xs'
                                  className='h-6 text-[10px] text-primary hover:bg-primary/10'
                                  onClick={() => handleImportToFolder(group.name, 'folder')}
                                >
                                  <FolderOpen className='mr-1 size-3' />
                                  {t('navigator.importFolder', 'Import folder')}
                                </Button>
                              </div>
                            </div>
                          ) : (
                            group.items.map(({ page, index }) => (
                              <PageItem
                                key={page.id}
                                page={page}
                                active={active === page.id}
                                selected={selected.includes(page.id)}
                                dragged={dragged === page.id}
                                chapterNames={chapterNames}
                                onIntent={active === page.id ? undefined : () => prefetchOnIntent(page.id)}
                                onSelect={(additive, range) => select(index, additive, range)}
                                onDragStart={() => setDragged(page.id)}
                                onDragEnd={() => setDragged(null)}
                                onRename={() => openRename(page)}
                                onSetChapter={() => {
                                  setChapterModalPage(page)
                                  setChapterInput(page.chapter ?? '')
                                }}
                                onDelete={() => deletePage(page.id)}
                                onDrop={() => {
                                  if (dragged && dragged !== page.id) {
                                    void call(commands.movePage, dragged, index)
                                      .then(() => refresh(projectKey, pagesKey))
                                      .catch(() => undefined)
                                  }
                                  setDragged(null)
                                }}
                              />
                            ))
                          )}
                        </div>
                      )}
                    </div>
                  )
                })}

                {pages.length === 0 && (
                  <div className='mt-3 flex flex-col items-center justify-center rounded-xl border border-dashed border-border/60 bg-muted/20 p-4 text-center'>
                    <p className='text-[10px] text-muted-foreground'>
                      {t('navigator.emptyFolderHint', 'Folders ready. Import pages to begin.')}
                    </p>
                    <div className='mt-2.5'>
                      <PageImportMenu importing={importing} onImport={importPages} />
                    </div>
                  </div>
                )}

                {unassignedPages.length > 0 && (
                  <div className='mt-2'>
                    <div className='px-2 py-1 text-[10px] font-semibold text-muted-foreground uppercase tracking-wider'>
                      {t('navigator.unassigned', 'Other Pages')} ({unassignedPages.length})
                    </div>
                    <div className='space-y-1 px-0.5'>
                      {unassignedPages.map(({ page, index }) => (
                        <PageItem
                          key={page.id}
                          page={page}
                          active={active === page.id}
                          selected={selected.includes(page.id)}
                          dragged={dragged === page.id}
                          chapterNames={chapterNames}
                          onIntent={active === page.id ? undefined : () => prefetchOnIntent(page.id)}
                          onSelect={(additive, range) => select(index, additive, range)}
                          onDragStart={() => setDragged(page.id)}
                          onDragEnd={() => setDragged(null)}
                          onRename={() => openRename(page)}
                          onSetChapter={() => {
                            setChapterModalPage(page)
                            setChapterInput(page.chapter ?? '')
                          }}
                          onDelete={() => deletePage(page.id)}
                          onDrop={() => {
                            if (dragged && dragged !== page.id) {
                              void call(commands.movePage, dragged, index)
                                .then(() => refresh(projectKey, pagesKey))
                                .catch(() => undefined)
                            }
                            setDragged(null)
                          }}
                        />
                      ))}
                    </div>
                  </div>
                )}
              </div>
            </ScrollArea>
          ) : (
          <ScrollArea
            className='min-h-0 flex-1'
            viewportClassName='px-1.5 py-1.5'
            viewportRef={pageList}
          >
            <div className='relative w-full' style={{ height: pageVirtualizer.getTotalSize() }}>
              {pageVirtualizer.getVirtualItems().map((virtualRow) => {
                const item = visiblePages[virtualRow.index]
                if (!item) return null
                const { page, index } = item
                return (
                  <div
                    key={virtualRow.key}
                    data-index={virtualRow.index}
                    className='absolute top-0 left-0 w-full'
                    style={{ transform: `translateY(${virtualRow.start}px)` }}
                  >
                    <PageItem
                      page={page}
                      active={active === page.id}
                      selected={selected.includes(page.id)}
                      dragged={dragged === page.id}
                      onIntent={active === page.id ? undefined : () => prefetchOnIntent(page.id)}
                      onSelect={(additive, range) => select(index, additive, range)}
                      onDragStart={() => setDragged(page.id)}
                      onDragEnd={() => setDragged(null)}
                      onRename={() => openRename(page)}
                      onSetChapter={() => {
                        setChapterModalPage(page)
                        setChapterInput((page as any).chapter ?? '')
                      }}
                      onDelete={() => deletePage(page.id)}
                      onDrop={() => {
                        if (dragged && dragged !== page.id) {
                          void call(commands.movePage, dragged, index)
                            .then(() => refresh(projectKey, pagesKey))
                            .catch(() => undefined)
                        }
                        setDragged(null)
                      }}
                    />
                  </div>
                )
              })}
            </div>
          </ScrollArea>
          )
        )}

        <div className='mt-auto flex shrink-0 items-center gap-2 px-2 py-2'>
          <Tooltip>
            <TooltipTrigger
              render={
                <Button
                  type='button'
                  variant='ghost'
                  size='icon'
                  className='size-8 text-muted-foreground hover:bg-foreground/[0.06] hover:text-foreground'
                  aria-label={t('trash.title', 'Project Trash & History')}
                  onClick={() => setTrashOpen(true)}
                >
                  <Trash2 className='size-4' />
                </Button>
              }
            />
            <TooltipContent side='right'>{t('trash.title', 'Project Trash & History')}</TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger
              render={
                <Button
                  type='button'
                  variant='ghost'
                  size='icon'
                  className='size-8 text-muted-foreground hover:bg-foreground/[0.06] hover:text-foreground'
                  aria-label={t('menu.settings')}
                  onClick={() => setSettingsOpen(true)}
                />
              }
            >
              <Settings className='size-4' />
            </TooltipTrigger>
            <TooltipContent side='right'>{t('menu.settings')}</TooltipContent>
          </Tooltip>
          <ResourceMonitor />
        </div>
      </aside>

      <Dialog
        open={renaming !== null}
        onOpenChange={(open) => {
          if (!open) closeRename()
        }}
      >
        <DialogContent className='sm:max-w-md'>
          <form
            onSubmit={(event) => {
              event.preventDefault()
              submitRename()
            }}
          >
            <DialogHeader>
              <DialogTitle>{t('navigator.renameTitle')}</DialogTitle>
              <DialogDescription>{t('navigator.renameDescription')}</DialogDescription>
            </DialogHeader>
            <Input
              autoFocus
              value={renameValue}
              aria-label={t('navigator.pageName')}
              className='mt-4'
              onChange={(event) => setRenameValue(event.currentTarget.value)}
            />
            <DialogFooter className='mt-4'>
              <Button type='button' variant='ghost' onClick={closeRename}>
                {t('common.cancel')}
              </Button>
              <Button type='submit' disabled={!renameValue.trim()}>
                {t('navigator.rename')}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>

      <Dialog
        open={folderToDelete !== null}
        onOpenChange={(open) => {
          if (!open) setFolderToDelete(null)
        }}
      >
        <DialogContent className='sm:max-w-md'>
          <DialogHeader>
            <DialogTitle className='flex items-center gap-2 text-destructive'>
              <Trash2 className='size-5' />
              {t('navigator.deleteFolderTitle', {
                name: folderToDelete,
                defaultValue: `Delete Chapter Folder "${folderToDelete}"`,
              })}
            </DialogTitle>
            <DialogDescription>
              {t(
                'navigator.deleteFolderDescription',
                'Choose how you want to delete this chapter folder. Both options will be recorded in Project Trash and can be restored.',
              )}
            </DialogDescription>
          </DialogHeader>

          {folderToDelete && (
            <div className='my-2 space-y-2 rounded-lg border border-border/40 bg-muted/20 p-3 text-xs'>
              <p className='font-medium text-foreground'>
                {t('navigator.folderContains', {
                  count: pages.filter((p) => p.chapter === folderToDelete).length,
                  defaultValue: `This chapter contains ${pages.filter((p) => p.chapter === folderToDelete).length} page(s).`,
                })}
              </p>
            </div>
          )}

          <div className='flex flex-col gap-2 pt-2'>
            <Button
              type='button'
              variant='destructive'
              disabled={deletingFolderInProgress}
              className='w-full justify-start gap-2 h-auto py-2.5 px-3 text-left'
              onClick={() => folderToDelete && void handleDeleteFolderChoice(folderToDelete, true)}
            >
              <Trash2 className='size-4 shrink-0' />
              <div>
                <div className='font-semibold text-xs'>
                  {t('navigator.deleteFolderAndPages', 'Delete Folder and All Pages')}
                </div>
                <div className='text-[10px] opacity-80 font-normal'>
                  {t(
                    'navigator.deleteFolderAndPagesDesc',
                    'Removes the folder and moves all pages inside it to Project Trash.',
                  )}
                </div>
              </div>
            </Button>

            <Button
              type='button'
              variant='outline'
              disabled={deletingFolderInProgress}
              className='w-full justify-start gap-2 h-auto py-2.5 px-3 text-left hover:bg-muted/50'
              onClick={() => folderToDelete && void handleDeleteFolderChoice(folderToDelete, false)}
            >
              <Folder className='size-4 shrink-0 text-amber-500' />
              <div>
                <div className='font-semibold text-xs'>
                  {t('navigator.deleteFolderOnly', 'Delete Folder Only')}
                </div>
                <div className='text-[10px] text-muted-foreground font-normal'>
                  {t(
                    'navigator.deleteFolderOnlyDesc',
                    'Pages will remain safely in the project and become unassigned.',
                  )}
                </div>
              </div>
            </Button>
          </div>

          <DialogFooter className='border-t pt-2'>
            <Button
              type='button'
              variant='ghost'
              size='sm'
              disabled={deletingFolderInProgress}
              onClick={() => setFolderToDelete(null)}
            >
              {t('common.cancel', 'Cancel')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <ProjectTrashDialog
        open={trashOpen}
        onOpenChange={setTrashOpen}
        onRestoredFolder={(folderName) => {
          if (!customFolders.includes(folderName)) {
            persistFolders([...customFolders, folderName])
          }
        }}
      />

      <Dialog open={newFolderOpen} onOpenChange={setNewFolderOpen}>
        <DialogContent className='sm:max-w-md'>
          <form
            onSubmit={(event) => {
              event.preventDefault()
              void submitCreateFolder()
            }}
          >
            <DialogHeader>
              <DialogTitle>{t('navigator.createFolderTitle', 'Create Chapter Folder')}</DialogTitle>
              <DialogDescription>
                {t(
                  'navigator.createFolderDesc',
                  'Enter a name for the new chapter or episode folder.',
                )}
              </DialogDescription>
            </DialogHeader>
            <Input
              autoFocus
              value={newFolderName}
              placeholder='Chapter 01'
              aria-label={t('navigator.folderName', 'Folder Name')}
              className='mt-4'
              onChange={(event) => setNewFolderName(event.currentTarget.value)}
            />
            {selected.length > 0 && (
              <p className='mt-2 text-[10px] text-muted-foreground'>
                {t('navigator.moveSelectedHint', {
                  count: selected.length,
                  defaultValue: `${selected.length} selected page(s) will be added to this folder.`,
                })}
              </p>
            )}
            <DialogFooter className='mt-4'>
              <Button type='button' variant='ghost' onClick={() => setNewFolderOpen(false)}>
                {t('common.cancel')}
              </Button>
              <Button type='submit' disabled={!newFolderName.trim()}>
                {t('common.create', 'Create')}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>

      <Dialog open={renamingFolder !== null} onOpenChange={(open) => !open && setRenamingFolder(null)}>
        <DialogContent className='sm:max-w-md'>
          <form
            onSubmit={(event) => {
              event.preventDefault()
              void submitRenameFolder()
            }}
          >
            <DialogHeader>
              <DialogTitle>{t('navigator.renameFolderTitle', 'Rename Folder')}</DialogTitle>
              <DialogDescription>
                {t(
                  'navigator.renameFolderDesc',
                  'Enter a new name for this chapter folder. All pages inside will be updated.',
                )}
              </DialogDescription>
            </DialogHeader>
            <Input
              autoFocus
              value={renameFolderValue}
              aria-label={t('navigator.folderName', 'Folder Name')}
              className='mt-4'
              onChange={(event) => setRenameFolderValue(event.currentTarget.value)}
            />
            <DialogFooter className='mt-4'>
              <Button type='button' variant='ghost' onClick={() => setRenamingFolder(null)}>
                {t('common.cancel')}
              </Button>
              <Button type='submit' disabled={!renameFolderValue.trim()}>
                {t('navigator.rename', 'Rename')}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>

      <Dialog
        open={chapterModalPage !== null}
        onOpenChange={(open) => {
          if (!open) {
            setChapterModalPage(null)
            setChapterInput('')
          }
        }}
      >
        <DialogContent className='sm:max-w-md'>
          <form
            onSubmit={(event) => {
              event.preventDefault()
              submitSetChapter()
            }}
          >
            <DialogHeader>
              <DialogTitle>{t('navigator.setChapterTitle', 'Set Page Chapter')}</DialogTitle>
              <DialogDescription>
                {t(
                  'navigator.setChapterDescription',
                  'Organize this page into a chapter folder (leave empty for none).',
                )}
              </DialogDescription>
            </DialogHeader>
            <Input
              autoFocus
              value={chapterInput}
              placeholder={t('navigator.chapterNamePlaceholder', 'Chapter name (e.g. Chapter 01)')}
              aria-label={t('navigator.chapterName', 'Chapter')}
              className='mt-4'
              onChange={(event) => setChapterInput(event.currentTarget.value)}
            />
            <DialogFooter className='mt-4'>
              <Button
                type='button'
                variant='ghost'
                onClick={() => {
                  setChapterModalPage(null)
                  setChapterInput('')
                }}
              >
                {t('common.cancel')}
              </Button>
              <Button type='submit'>{t('common.save', 'Save')}</Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </>
  )
}

function PageImportMenu({
  importing,
  onImport,
}: {
  importing: boolean
  onImport: (source: PageImportSource) => void
}) {
  const { t } = useTranslation()
  const icon = importing ? (
    <LoaderCircle className='size-3.5 animate-spin' />
  ) : (
    <FilePlus2 className='size-3.5' />
  )

  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <Button
            size='sm'
            variant='outline'
            disabled={importing}
            aria-busy={importing}
            className='rounded-lg text-[10px]'
          />
        }
      >
        {icon}
        {importing ? t('navigator.importingAction') : t('navigator.importAction')}
      </DropdownMenuTrigger>
      <DropdownMenuContent
        align='start'
        className='w-auto min-w-20 border border-border/50 p-0.5 shadow-sm ring-0'
      >
        <DropdownMenuItem
          disabled={importing}
          className='min-h-7 gap-1 px-1.5 py-0.5 text-[11px] [&_svg:not([class*="size-"])]:size-3.5'
          onClick={() => onImport('files')}
        >
          <FilePlus2 />
          {t('navigator.importFiles')}
        </DropdownMenuItem>
        <DropdownMenuItem
          disabled={importing}
          className='min-h-7 gap-1 px-1.5 py-0.5 text-[11px] [&_svg:not([class*="size-"])]:size-3.5'
          onClick={() => onImport('folder')}
        >
          <FolderOpen />
          {t('navigator.importFolder')}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

function PageItem({
  page,
  active,
  selected,
  dragged,
  chapterNames,
  onIntent,
  onSelect,
  onDragStart,
  onDragEnd,
  onRename,
  onSetChapter,
  onDelete,
  onDrop,
}: {
  page: PageSummary
  active: boolean
  selected: boolean
  dragged: boolean
  chapterNames?: string[]
  onIntent?: () => void
  onSelect: (additive: boolean, range: boolean) => void
  onDragStart: () => void
  onDragEnd: () => void
  onRename: () => void
  onSetChapter: () => void
  onDelete: () => void
  onDrop: () => void
}) {
  const { t } = useTranslation()

  return (
    <article
      draggable
      data-active={active}
      data-selected={selected}
      className={cn(
        'group grid cursor-default grid-cols-[48px_minmax(0,1fr)] gap-2.5 rounded-xl p-1.5 transition-colors select-none',
        active
          ? 'bg-primary/[0.09] hover:bg-primary/[0.09]'
          : selected
            ? 'bg-foreground/[0.06] hover:bg-foreground/[0.08]'
            : 'hover:bg-foreground/[0.045]',
        dragged && 'opacity-50',
      )}
      onPointerEnter={onIntent}
      onFocus={onIntent}
      onClick={(event) => {
        if ((event.target as HTMLElement).closest('button,[role="menuitem"]')) return
        onSelect(event.ctrlKey || event.metaKey, event.shiftKey)
      }}
      onDoubleClick={onRename}
      onDragStart={onDragStart}
      onDragEnd={onDragEnd}
      onDragOver={(event) => event.preventDefault()}
      onDrop={(event) => {
        event.preventDefault()
        onDrop()
      }}
    >
      <div className='grid h-16 w-12 place-items-center overflow-hidden rounded-lg bg-[var(--surface-well)]'>
        {page.source_asset ? (
          <PageThumbnail page={page.id} asset={page.source_asset} label={page.label} />
        ) : (
          <span className='text-[9px] text-muted-foreground'>{t('navigator.noImage')}</span>
        )}
      </div>
      <div className='min-w-0 py-0.5'>
        <div className='flex items-start gap-1'>
          <span className='min-w-0 flex-1 truncate text-[10px] font-medium'>{page.label}</span>
          <DropdownMenu>
            <DropdownMenuTrigger
              render={
                <Button
                  variant='ghost'
                  size='icon-xs'
                  aria-label={t('navigator.actionsFor', { page: page.label })}
                  className='-mt-1 shrink-0 opacity-0 shadow-none group-hover:opacity-100 aria-expanded:opacity-100'
                />
              }
            >
              <MoreHorizontal />
            </DropdownMenuTrigger>
            <DropdownMenuContent align='end'>
              <DropdownMenuItem onClick={onRename}>{t('navigator.rename')}</DropdownMenuItem>
              <DropdownMenuItem onClick={onSetChapter}>
                {t('navigator.setChapter', 'Set Chapter...')}
              </DropdownMenuItem>
              {chapterNames && chapterNames.length > 0 && (
                <DropdownMenuSub>
                  <DropdownMenuSubTrigger>
                    {t('navigator.moveToFolder', 'Move to Folder')}
                  </DropdownMenuSubTrigger>
                  <DropdownMenuSubContent>
                    {chapterNames.map((ch) => (
                      <DropdownMenuItem
                        key={ch}
                        onClick={() => {
                          void call(commands.setPageChapter, page.id, ch)
                            .then(() => refresh(projectKey, pagesKey, pageKey))
                        }}
                      >
                        <Folder className='size-3.5' /> {ch}
                      </DropdownMenuItem>
                    ))}
                    {page.chapter && (
                      <>
                        <DropdownMenuSeparator />
                        <DropdownMenuItem
                          onClick={() => {
                            void call(commands.setPageChapter, page.id, null)
                              .then(() => refresh(projectKey, pagesKey, pageKey))
                          }}
                        >
                          {t('navigator.removeFromFolder', 'Remove from Folder')}
                        </DropdownMenuItem>
                      </>
                    )}
                  </DropdownMenuSubContent>
                </DropdownMenuSub>
              )}
              <DropdownMenuSeparator />
              <DropdownMenuItem variant='destructive' onClick={onDelete}>
                <Trash2 /> {t('navigator.delete')}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
        <div className='mt-1 flex items-center gap-1.5'>
          {(page as any).chapter && (
            <span className='py-0.2 inline-flex items-center rounded bg-primary/10 px-1 text-[8px] font-medium text-primary'>
              {(page as any).chapter}
            </span>
          )}
          <p className='text-[9px] leading-3.5 text-muted-foreground'>
            {t('navigator.layerCount', { count: page.layer_count })}
          </p>
        </div>
        <p className='mt-1 text-[9px] text-muted-foreground tabular-nums'>
          {page.size.width} × {page.size.height}
        </p>
      </div>
    </article>
  )
}

function PageThumbnail({ page, asset, label }: { page: string; asset: string; label: string }) {
  const { t } = useTranslation()
  const [source, setSource] = useState<string | null>(null)
  const [settled, setSettled] = useState(false)

  useEffect(() => {
    setSettled(false)
    const timeout = window.setTimeout(() => setSettled(true), 100)
    return () => window.clearTimeout(timeout)
  }, [asset])

  const thumbnail = useQuery({
    queryKey: ['thumbnail', asset],
    queryFn: async () => new Uint8Array(await call(commands.getThumbnail, page)),
    enabled: settled,
    staleTime: Number.POSITIVE_INFINITY,
    notifyOnChangeProps: ['data'],
  }).data

  useEffect(() => {
    if (!thumbnail) return
    const url = URL.createObjectURL(new Blob([thumbnail.buffer], { type: 'image/webp' }))
    setSource(url)
    return () => {
      URL.revokeObjectURL(url)
    }
  }, [thumbnail])

  return (
    <div className='grid size-full place-items-center'>
      {source ? (
        // eslint-disable-next-line @next/next/no-img-element
        <img src={source} alt={label} draggable={false} className='size-full object-contain' />
      ) : (
        <span className='text-[9px] text-muted-foreground'>{t('common.loading')}</span>
      )}
    </div>
  )
}

function ChapterProcessPopover({
  chapterName,
  pages,
  disabled,
}: {
  chapterName: string
  pages: PageSummary[]
  disabled?: boolean
}) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const [view, setView] = useState<'root' | 'model' | 'stages' | 'output'>('root')
  const [loadingModels, setLoadingModels] = useState(false)
  const [savingModel, setSavingModel] = useState<string | null>(null)
  const [savingOutput, setSavingOutput] = useState(false)
  const [stages, setStages] = useState<Stage[]>(() => [...pipelineStages])

  const preferences = useKoharuStore((state) => state.preferences)
  const translationModels = useKoharuStore((state) => state.translationModels)
  const jobs = useKoharuStore((state) => state.jobs)
  const running = Object.values(jobs).some((job) => job.state === 'running')

  const model = preferences?.pipeline.translation.model ?? null
  const translation = preferences?.pipeline.translation ?? null
  const providers = preferences?.providers.entries ?? []
  const languages = preferences?.languages ?? []
  const choices = availableModels(model, translationModels, providers)
  const modelLabel =
    choices.find((choice) => model && modelKey(choice) === modelKey(model))?.name ??
    (model ? (model.model ?? t('inference.providerDefault')) : t('inference.noModel'))
  const outputLabel =
    languages.find((language) => language.tag === translation?.target_language)?.name ??
    translation?.target_language ??
    t('inference.notSet')

  const handleOpenChange = (next: boolean) => {
    setOpen(next)
    setView('root')
    if (!next) return
    setLoadingModels(true)
    void refreshTranslationModels(true)
      .catch(() => undefined)
      .finally(() => setLoadingModels(false))
  }

  const chooseModel = (next: ModelSelection) => {
    if (!preferences || savingModel) return
    if (model && modelKey(model) === modelKey(next) && model.quantization === next.quantization) {
      setView('root')
      return
    }

    const key = modelKey(next)
    setSavingModel(key)
    const pipeline = {
      ...preferences.pipeline,
      translation: {
        ...preferences.pipeline.translation,
        model: next,
      },
    }
    void savePreferences(pipeline, preferences.providers, preferences.typesetting)
      .then((saved) => {
        receivePreferences(saved)
      })
      .catch(() => undefined)
      .finally(() => setSavingModel(null))
  }

  const saveOutput = (draft: OutputDraft) => {
    if (!preferences || !translation || savingOutput) return
    setSavingOutput(true)
    const pipeline = {
      ...preferences.pipeline,
      translation: {
        ...translation,
        target_language: draft.targetLanguage,
        instructions: draft.instructions || null,
      },
    }
    void savePreferences(pipeline, preferences.providers, preferences.typesetting)
      .then((saved) => {
        receivePreferences(saved)
      })
      .catch(() => undefined)
      .finally(() => setSavingOutput(false))
  }

  const toggleStage = (stage: Stage) => {
    if (stages.includes(stage)) {
      if (stages.length > 1) setStages(stages.filter((candidate) => candidate !== stage))
      return
    }
    setStages(pipelineStages.filter((candidate) => candidate === stage || stages.includes(candidate)))
  }

  const runPipeline = () => {
    if (pages.length === 0) return
    const pageIds = pages.map((p) => p.id)
    const operation =
      stages.length === pipelineStages.length
        ? ({ operation: 'full' } as const)
        : stages.length === 1
          ? ({ operation: 'only', stage: stages[0]! } as const)
          : ({ operation: 'stages', stages } as const)
    void call(commands.process, { scope: 'pages', value: pageIds }, operation).catch(() => undefined)
    setOpen(false)
  }

  return (
    <Popover open={open} onOpenChange={handleOpenChange}>
      <PopoverTrigger
        render={
          <Button
            type='button'
            variant='ghost'
            size='icon-xs'
            disabled={disabled || pages.length === 0 || running}
            aria-label={t('navigator.processFolder', { defaultValue: 'Process folder' })}
            title={t('navigator.processFolder', { defaultValue: 'Process all in folder' })}
            className='size-6 text-muted-foreground hover:bg-foreground/[0.08] hover:text-foreground'
            onClick={(e) => e.stopPropagation()}
          >
            <Sparkles className='size-3 text-primary' />
          </Button>
        }
      />

      <PopoverContent
        align='start'
        sideOffset={4}
        className='w-64 min-w-0 gap-0 overflow-hidden rounded-xl border border-border/50 p-1 shadow-sm ring-0'
        onClick={(e) => e.stopPropagation()}
      >
        {view === 'root' && (
          <div className='grid gap-1' aria-label={chapterName}>
            <div className='flex items-center justify-between border-b border-border/60 px-2 py-1'>
              <div className='flex min-w-0 items-center gap-1.5'>
                <Folder className='size-3.5 text-primary' />
                <span className='truncate text-[11px] font-semibold'>{chapterName}</span>
              </div>
              <span className='text-[10px] text-muted-foreground tabular-nums'>
                {pages.length} {pages.length === 1 ? 'page' : 'pages'}
              </span>
            </div>

            <div className='grid gap-0.5'>
              <SelectorRow
                label={t('inference.model')}
                value={modelLabel}
                onClick={() => setView('model')}
              />
              <SelectorRow
                label={t('inference.stages')}
                value={
                  stages.length === 1
                    ? t(`phase.${stages[0]}`)
                    : t('inference.stageCount', { count: stages.length })
                }
                onClick={() => setView('stages')}
              />
              <SelectorRow
                label={t('inference.output')}
                value={outputLabel}
                onClick={() => setView('output')}
              />
            </div>

            <div className='mt-1 border-t border-border/70 pt-1'>
              <Button
                type='button'
                size='sm'
                disabled={pages.length === 0 || running}
                className='h-8 w-full justify-center gap-2 rounded-lg bg-primary/80 text-[11px] hover:bg-primary/90'
                onClick={runPipeline}
              >
                <Play className='size-3 fill-current' />
                <span>{t('navigator.runOnChapter', { defaultValue: 'Process Chapter' })}</span>
              </Button>
            </div>
          </div>
        )}

        {view === 'model' && (
          <ModelPicker
            value={model}
            models={choices}
            providers={providers}
            loading={loadingModels}
            disabled={running}
            busyModel={savingModel}
            onBack={() => setView('root')}
            onSelect={chooseModel}
          />
        )}

        {view === 'stages' && (
          <SelectorPanel title={t('inference.pipelineStages')} onBack={() => setView('root')}>
            <SelectorOption
              value='detection'
              label={t('phase.detection')}
              detail={t('phaseDescription.detection')}
              selected={stages.includes('detection')}
              onSelect={toggleStage}
            />
            <SelectorOption
              value='ocr'
              label={t('phase.ocr')}
              detail={t('phaseDescription.ocr')}
              selected={stages.includes('ocr')}
              onSelect={toggleStage}
            />
            <SelectorOption
              value='translation'
              label={t('phase.translation')}
              detail={t('phaseDescription.translation')}
              selected={stages.includes('translation')}
              onSelect={toggleStage}
            />
            <SelectorOption
              value='inpainting'
              label={t('phase.inpainting')}
              detail={t('phaseDescription.inpainting')}
              selected={stages.includes('inpainting')}
              onSelect={toggleStage}
            />
          </SelectorPanel>
        )}

        {view === 'output' && translation && (
          <OutputPicker
            targetLanguage={translation.target_language}
            instructions={translation.instructions}
            languages={languages}
            disabled={running}
            saving={savingOutput}
            onBack={() => setView('root')}
            onChange={saveOutput}
          />
        )}
      </PopoverContent>
    </Popover>
  )
}
