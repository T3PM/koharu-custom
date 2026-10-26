'use client'

import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { ArchiveRestore, Folder, FolderArchive, Layers, Trash2 } from 'lucide-react'
import { useQuery } from '@tanstack/react-query'

import type { TrashItem } from '@koharu/bridge/protocol'
import { commands } from '@koharu/bridge/protocol'
import { Button } from '@koharu/ui/components/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@koharu/ui/components/dialog'
import { call } from '@/lib/backend'
import { pageKey, pagesKey, projectKey, refresh } from '@/lib/queries'

export const trashKey = ['trash'] as const

interface ProjectTrashDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  onRestoredFolder?: (folderName: string) => void
}

export function ProjectTrashDialog({
  open,
  onOpenChange,
  onRestoredFolder,
}: ProjectTrashDialogProps) {
  const { t } = useTranslation()
  const [processingId, setProcessingId] = useState<string | null>(null)
  const [clearing, setClearing] = useState(false)

  const { data: items = [], refetch } = useQuery({
    queryKey: trashKey,
    queryFn: () => call(commands.getTrashItems),
    enabled: open,
  })

  const handleRestore = async (item: TrashItem) => {
    setProcessingId(item.id)
    try {
      if (item.kind === 'folder') {
        if (item.chapter) {
          onRestoredFolder?.(item.chapter)
        }
        await call(commands.deleteTrashItem, item.id)
      } else {
        await call(commands.restoreTrashItem, item.id)
        if (item.chapter) {
          onRestoredFolder?.(item.chapter)
        }
      }
      await refetch()
      await refresh(projectKey, pagesKey, pageKey)
    } finally {
      setProcessingId(null)
    }
  }

  const handleDeletePermanent = async (id: string) => {
    setProcessingId(id)
    try {
      await call(commands.deleteTrashItem, id)
      await refetch()
    } finally {
      setProcessingId(null)
    }
  }

  const handleClearTrash = async () => {
    setClearing(true)
    try {
      await call(commands.clearTrash)
      await refetch()
    } finally {
      setClearing(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className='max-h-[85vh] max-w-xl flex flex-col gap-3 overflow-hidden'>
        <DialogHeader>
          <div className='flex items-center justify-between pr-6'>
            <DialogTitle className='flex items-center gap-2'>
              <Trash2 className='size-4 text-muted-foreground' />
              {t('trash.title', 'Project Trash & History')}
              {items.length > 0 && (
                <span className='rounded-full bg-secondary px-2 py-0.5 text-xs text-muted-foreground'>
                  {items.length}
                </span>
              )}
            </DialogTitle>
            {items.length > 0 && (
              <Button
                variant='ghost'
                size='xs'
                disabled={clearing}
                className='text-destructive hover:bg-destructive/10'
                onClick={handleClearTrash}
              >
                {t('trash.emptyTrash', 'Empty Trash')}
              </Button>
            )}
          </div>
          <DialogDescription>
            {t(
              'trash.description',
              'Restore deleted chapter folders, pages, or entire chapters back to your project at any time.',
            )}
          </DialogDescription>
        </DialogHeader>

        <div className='flex-1 overflow-y-auto pr-1 space-y-2 max-h-[50vh] min-h-[160px]'>
          {items.length === 0 ? (
            <div className='flex h-40 flex-col items-center justify-center text-center text-muted-foreground'>
              <ArchiveRestore className='mb-2 size-8 opacity-40' />
              <p className='text-sm'>{t('trash.empty', 'Trash is empty.')}</p>
              <p className='text-xs opacity-60'>
                {t('trash.emptyHint', 'Deleted items from this project will be listed here.')}
              </p>
            </div>
          ) : (
            items.map((item) => (
              <div
                key={item.id}
                className='flex items-center justify-between rounded-lg border border-border/50 bg-background/50 p-2.5 transition-colors hover:bg-muted/30'
              >
                <div className='flex items-center gap-3 min-w-0 flex-1'>
                  <div className='flex size-9 shrink-0 items-center justify-center rounded-md bg-secondary/60 text-secondary-foreground'>
                    {item.kind === 'folder' && <Folder className='size-4 text-amber-500' />}
                    {item.kind === 'folder_with_pages' && (
                      <FolderArchive className='size-4 text-amber-600' />
                    )}
                    {item.kind === 'page' && <Layers className='size-4 text-blue-500' />}
                  </div>

                  <div className='min-w-0 flex-1'>
                    <div className='flex items-center gap-1.5'>
                      <span className='font-medium text-xs truncate max-w-[240px]'>
                        {item.name}
                      </span>
                      <span className='rounded bg-muted px-1.5 py-0.5 text-[9px] font-medium text-muted-foreground'>
                        {item.kind === 'folder' && t('trash.folderOnly', 'Folder only')}
                        {item.kind === 'folder_with_pages' &&
                          t('trash.folderWithPages', {
                            count: item.pages.length,
                            defaultValue: `Folder (${item.pages.length} pages)`,
                          })}
                        {item.kind === 'page' &&
                          t('trash.pagesCount', {
                            count: item.pages.length,
                            defaultValue: `${item.pages.length} page(s)`,
                          })}
                      </span>
                    </div>

                    <div className='flex items-center gap-2 text-[10px] text-muted-foreground'>
                      <span>{new Date(item.timestamp * 1000).toLocaleString()}</span>
                      {item.chapter && (
                        <span>
                          {t('trash.chapterLabel', 'Chapter')}: {item.chapter}
                        </span>
                      )}
                    </div>
                  </div>
                </div>

                <div className='flex items-center gap-1.5 shrink-0 pl-2'>
                  <Button
                    variant='outline'
                    size='xs'
                    disabled={processingId === item.id}
                    className='gap-1 h-7 text-xs'
                    onClick={() => handleRestore(item)}
                  >
                    <ArchiveRestore className='size-3 text-primary' />
                    {t('trash.restore', 'Restore')}
                  </Button>
                  <Button
                    variant='ghost'
                    size='xs'
                    disabled={processingId === item.id}
                    className='size-7 p-0 text-muted-foreground hover:text-destructive hover:bg-destructive/10'
                    onClick={() => handleDeletePermanent(item.id)}
                    title={t('trash.deletePermanent', 'Delete permanently')}
                  >
                    <Trash2 className='size-3.5' />
                  </Button>
                </div>
              </div>
            ))
          )}
        </div>

        <DialogFooter className='border-t pt-2'>
          <Button variant='outline' size='sm' onClick={() => onOpenChange(false)}>
            {t('common.close', 'Close')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
