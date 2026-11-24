// Android 11 images ship WebView 83, which predates FinalizationRegistry and WeakRef (Chrome 84)
// that three's WebGLRenderer uses. Fallbacks: cleanup callbacks never fire, and WeakRef holds
// a strong reference — a bounded leak on legacy WebViews only.
if (typeof globalThis.FinalizationRegistry === 'undefined') {
  class NoopFinalizationRegistry {
    register(): void {}
    unregister(): boolean {
      return false
    }
  }
  Object.defineProperty(globalThis, 'FinalizationRegistry', { value: NoopFinalizationRegistry, configurable: true })
}
if (typeof globalThis.WeakRef === 'undefined') {
  class StrongRef<T extends object> {
    constructor(private readonly target: T) {}
    deref(): T {
      return this.target
    }
  }
  Object.defineProperty(globalThis, 'WeakRef', { value: StrongRef, configurable: true })
}
