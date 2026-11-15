import { create } from 'zustand'

type ToastState = { message: string | null; nonce: number }

export const useToast = create<ToastState>(() => ({ message: null, nonce: 0 }))

let toastTimer = 0

export const toast = (message: string) => {
  window.clearTimeout(toastTimer)
  useToast.setState({ message, nonce: performance.now() })
  toastTimer = window.setTimeout(() => useToast.setState({ message: null }), 1800)
}
