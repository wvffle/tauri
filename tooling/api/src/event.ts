// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

/**
 * The event system allows you to emit events to the backend and listen to events from it.
 *
 * This package is also accessible with `window.__TAURI__.event` when [`build.withGlobalTauri`](https://tauri.app/v1/api/config/#buildconfig.withglobaltauri) in `tauri.conf.json` is set to `true`.
 * @module
 */

import { invoke, transformCallback } from './core'

type EventSource =
  | {
      kind: 'global'
    }
  | {
      kind: 'window'
      label: string
    }
  | {
      kind: 'webview'
      label: string
    }

interface Event<T> {
  /** Event name */
  event: EventName
  /** The source of the event. Can be a global event, an event from a window or an event from another webview. */
  source: EventSource
  /** Event identifier used to unlisten */
  id: number
  /** Event payload */
  payload: T
}

type EventCallback<T> = (event: Event<T>) => void

type UnlistenFn = () => void

type EventName = `${TauriEvent}` | (string & Record<never, never>)

interface Options {
  /**
   * Window or webview the function targets.
   *
   * When listening to events and using this value,
   * only events triggered by the window with the given label are received.
   *
   * When emitting events, only the window with the given label will receive it.
   */
  target?:
    | { kind: 'window'; label: string }
    | { kind: 'webview'; label: string }
}

/**
 * @since 1.1.0
 */
enum TauriEvent {
  WINDOW_RESIZED = 'tauri://resize',
  WINDOW_MOVED = 'tauri://move',
  WINDOW_CLOSE_REQUESTED = 'tauri://close-requested',
  WINDOW_DESTROYED = 'tauri://destroyed',
  WINDOW_FOCUS = 'tauri://focus',
  WINDOW_BLUR = 'tauri://blur',
  WINDOW_SCALE_FACTOR_CHANGED = 'tauri://scale-change',
  WINDOW_THEME_CHANGED = 'tauri://theme-changed',
  WEBVIEW_CREATED = 'tauri://webview-created',
  WEBVIEW_FILE_DROP = 'tauri://file-drop',
  WEBVIEW_FILE_DROP_HOVER = 'tauri://file-drop-hover',
  WEBVIEW_FILE_DROP_CANCELLED = 'tauri://file-drop-cancelled'
}

/**
 * Unregister the event listener associated with the given name and id.
 *
 * @ignore
 * @param event The event name
 * @param eventId Event identifier
 * @returns
 */
async function _unlisten(event: string, eventId: number): Promise<void> {
  await invoke('plugin:event|unlisten', {
    event,
    eventId
  })
}

/**
 * Listen to an event. The event can be either global or window-specific.
 * See {@link Event.source} to check the event source.
 *
 * @example
 * ```typescript
 * import { listen } from '@tauri-apps/api/event';
 * const unlisten = await listen<string>('error', (event) => {
 *   console.log(`Got error in window ${event.source}, payload: ${event.payload}`);
 * });
 *
 * // you need to call unlisten if your handler goes out of scope e.g. the component is unmounted
 * unlisten();
 * ```
 *
 * @param event Event name. Must include only alphanumeric characters, `-`, `/`, `:` and `_`.
 * @param handler Event handler callback.
 * @returns A promise resolving to a function to unlisten to the event.
 * Note that removing the listener is required if your listener goes out of scope e.g. the component is unmounted.
 *
 * @since 1.0.0
 */
async function listen<T>(
  event: EventName,
  handler: EventCallback<T>,
  options?: Options
): Promise<UnlistenFn> {
  return invoke<number>('plugin:event|listen', {
    event,
    target: options?.target,
    handler: transformCallback(handler)
  }).then((eventId) => {
    return async () => _unlisten(event, eventId)
  })
}

/**
 * Listen to an one-off event. See {@link listen} for more information.
 *
 * @example
 * ```typescript
 * import { once } from '@tauri-apps/api/event';
 * interface LoadedPayload {
 *   loggedIn: boolean,
 *   token: string
 * }
 * const unlisten = await once<LoadedPayload>('loaded', (event) => {
 *   console.log(`App is loaded, loggedIn: ${event.payload.loggedIn}, token: ${event.payload.token}`);
 * });
 *
 * // you need to call unlisten if your handler goes out of scope e.g. the component is unmounted
 * unlisten();
 * ```
 *
 * @param event Event name. Must include only alphanumeric characters, `-`, `/`, `:` and `_`.
 * @returns A promise resolving to a function to unlisten to the event.
 * Note that removing the listener is required if your listener goes out of scope e.g. the component is unmounted.
 *
 * @since 1.0.0
 */
async function once<T>(
  event: EventName,
  handler: EventCallback<T>,
  options?: Options
): Promise<UnlistenFn> {
  return listen<T>(
    event,
    (eventData) => {
      handler(eventData)
      _unlisten(event, eventData.id).catch(() => {})
    },
    options
  )
}

/**
 * Emits an event to the backend and all Tauri windows.
 * @example
 * ```typescript
 * import { emit } from '@tauri-apps/api/event';
 * await emit('frontend-loaded', { loggedIn: true, token: 'authToken' });
 * ```
 *
 * @param event Event name. Must include only alphanumeric characters, `-`, `/`, `:` and `_`.
 *
 * @since 1.0.0
 */
async function emit(
  event: string,
  payload?: unknown,
  options?: Options
): Promise<void> {
  await invoke('plugin:event|emit', {
    event,
    target: options?.target,
    payload
  })
}

export type {
  EventSource,
  Event,
  EventCallback,
  UnlistenFn,
  EventName,
  Options
}

export { listen, once, emit, TauriEvent }
