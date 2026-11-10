import * as MikaSDKUI from '@mika/sdk-ui'
import * as React from 'react'
import * as ReactJSXRuntime from 'react/jsx-runtime'
import * as ReactDOM from 'react-dom'

type PluginGlobalsWindow = Window & {
  React?: typeof React
  ReactDOM?: typeof ReactDOM
  ReactJSXRuntime?: typeof ReactJSXRuntime
  MikaSDKUI?: typeof MikaSDKUI
}

const pluginWindow = window as PluginGlobalsWindow

pluginWindow.React ??= React
pluginWindow.ReactDOM ??= ReactDOM
pluginWindow.ReactJSXRuntime ??= ReactJSXRuntime
pluginWindow.MikaSDKUI ??= MikaSDKUI
