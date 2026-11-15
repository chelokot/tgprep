import type { LucideIcon } from 'lucide-react'
import type { ReactNode } from 'react'
import { useToast } from './toast'

export const IconButton = ({
  icon: Icon,
  label,
  onClick,
  active = false,
  disabled = false,
  tone = 'plain',
}: {
  icon: LucideIcon
  label: string
  onClick: () => void
  active?: boolean
  disabled?: boolean
  tone?: 'plain' | 'overlay'
}) => (
  <button
    type="button"
    title={label}
    aria-label={label}
    aria-pressed={active}
    disabled={disabled}
    onClick={onClick}
    className={`grid size-8 shrink-0 place-items-center rounded-lg transition-colors disabled:opacity-30 ${
      tone === 'overlay'
        ? active
          ? 'bg-accent text-white'
          : 'text-white/80 hover:bg-white/15 hover:text-white'
        : active
          ? 'bg-accent/15 text-accent'
          : 'text-muted hover:bg-raised hover:text-fg'
    }`}
  >
    <Icon className="size-4" strokeWidth={2} />
  </button>
)

export const Chip = ({
  active,
  onClick,
  children,
  label,
  color = null,
}: {
  active: boolean
  onClick: () => void
  children: ReactNode
  label: string
  color?: string | null
}) => (
  <button
    type="button"
    title={label}
    aria-pressed={active}
    onClick={onClick}
    style={active && color ? { backgroundColor: color } : undefined}
    className={`flex h-7 shrink-0 items-center gap-1.5 rounded-full px-2.5 text-xs font-medium transition-colors ${
      active ? 'bg-accent text-white' : 'bg-raised text-muted hover:text-fg'
    }`}
  >
    {color && !active && <span className="size-2 rounded-full" style={{ backgroundColor: color }} />}
    {children}
  </button>
)

export const Segmented = <T extends string>({
  value,
  options,
  onChange,
}: {
  value: T
  options: { value: T; label: ReactNode; title: string }[]
  onChange: (value: T) => void
}) => (
  <div className="flex shrink-0 rounded-lg bg-raised p-0.5">
    {options.map((option) => (
      <button
        key={option.value}
        type="button"
        title={option.title}
        aria-pressed={option.value === value}
        onClick={() => onChange(option.value)}
        className={`flex h-7 items-center gap-1.5 rounded-md px-2.5 text-xs font-medium transition-colors ${
          option.value === value ? 'bg-panel text-fg shadow-sm' : 'text-muted hover:text-fg'
        }`}
      >
        {option.label}
      </button>
    ))}
  </div>
)

export const RangeInput = ({
  value,
  min,
  max,
  step,
  label,
  onChange,
  onStart = () => {},
  onReset = () => {},
  className = '',
}: {
  value: number
  min: number
  max: number
  step: number
  label: string
  onChange: (value: number) => void
  onStart?: () => void
  onReset?: () => void
  className?: string
}) => (
  <input
    type="range"
    aria-label={label}
    title={label}
    min={min}
    max={max}
    step={step}
    value={value}
    onPointerDown={onStart}
    onDoubleClick={onReset}
    onChange={(event) => onChange(Number(event.target.value))}
    className={`h-1 cursor-pointer accent-accent ${className}`}
  />
)

export const PaneHeader = ({ children }: { children: ReactNode }) => (
  <div className="flex min-h-12 shrink-0 flex-wrap items-center gap-2 border-b border-line px-3 py-2">{children}</div>
)

export const Toasts = () => {
  const { message, nonce } = useToast()
  if (!message) return null
  return (
    <div className="pointer-events-none fixed inset-x-0 bottom-6 z-[70] flex justify-center">
      <div key={nonce} role="status" className="animate-pop rounded-full bg-fg px-4 py-2 text-sm font-medium text-canvas shadow-xl">
        {message}
      </div>
    </div>
  )
}
