import { FolderOpen, History, Send } from 'lucide-react'
import { useEffect, useState } from 'react'
import { t } from '../i18n'
import { loadRecentFolders, openFolder, pickFolder, type RecentFolder } from '../model/persistence'

export const Welcome = () => {
  const [recent, setRecent] = useState<RecentFolder[]>([])
  const supported = 'showDirectoryPicker' in window
  useEffect(() => void loadRecentFolders().then(setRecent), [])

  return (
    <main className="grid min-h-dvh place-items-center bg-canvas p-6">
      <div className="w-full max-w-md animate-pop">
        <div className="mb-6 grid size-14 place-items-center rounded-2xl bg-accent text-white shadow-lg shadow-accent/30">
          <Send className="size-7 -translate-x-0.5" />
        </div>
        <h1 className="text-3xl font-bold tracking-tight text-fg">{t('appName')}</h1>
        <p className="mt-2 text-sm leading-relaxed text-muted">{t('tagline')}</p>
        {supported ? (
          <button
            type="button"
            onClick={() => void pickFolder()}
            className="mt-8 flex h-12 w-full items-center justify-center gap-2 rounded-xl bg-accent font-semibold text-white shadow-lg shadow-accent/25 transition hover:brightness-110"
          >
            <FolderOpen className="size-5" /> {t('openFolder')}
          </button>
        ) : (
          <p className="mt-8 rounded-xl bg-danger/10 p-4 text-sm text-danger">{t('unsupported')}</p>
        )}
        {recent.length > 0 && (
          <section className="mt-8">
            <h2 className="mb-2 flex items-center gap-1.5 text-xs font-semibold tracking-wide text-faint uppercase">
              <History className="size-3.5" /> {t('recentFolders')}
            </h2>
            <ul className="space-y-1">
              {recent.map((folder) => (
                <li key={folder.name}>
                  <button
                    type="button"
                    onClick={() => void openFolder(folder.handle)}
                    className="flex h-10 w-full items-center gap-2 rounded-lg px-3 text-left text-sm text-fg transition hover:bg-raised"
                  >
                    <FolderOpen className="size-4 text-muted" /> {folder.name}
                  </button>
                </li>
              ))}
            </ul>
          </section>
        )}
      </div>
    </main>
  )
}
