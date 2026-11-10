import { describe, expect, it } from 'vitest'
import { deviceOptions } from '../AudioDeviceControls'

describe('deviceOptions', () => {
  // Chromium lists the OS default endpoint itself as deviceId "default" (Windows: "Padrão - …");
  // next to our own "system default" entry that was two options with one value — a React
  // duplicate-key error and an ambiguous selection.
  it('keeps exactly one option per value when the browser lists its own "default" device', () => {
    const options = deviceOptions(
      [
        { deviceId: 'default', label: 'Padrão - Alto-falantes' },
        { deviceId: 'communications', label: 'Comunicações - Alto-falantes' },
        { deviceId: 'abc123', label: 'Headset' },
      ],
      'Padrão do sistema',
    )

    expect(options.map((o) => o.value)).toEqual(['default', 'communications', 'abc123'])
    expect(options[0]?.label).toBe('Padrão do sistema')
  })
})
