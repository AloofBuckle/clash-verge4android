import { useLockFn } from 'ahooks'
import { useState } from 'react'
import { updateProxyProvider } from 'tauri-plugin-mihomo-api'

import { useAppRefreshers, useProxiesData } from '@/providers/app-data-context'
import { syncRuntimeProviders } from '@/services/cmds'
import { showNotice } from '@/services/notice-service'

import {
  ProxyProviderButtonView,
  type ProxyProviderButtonItem,
} from './provider-button-view'

export const ProviderButton = () => {
  const { proxyView } = useProxiesData()
  const { refreshProxy } = useAppRefreshers()
  const [updating, setUpdating] = useState<Record<string, boolean>>({})
  const providers: ProxyProviderButtonItem[] = (proxyView?.providers ?? []).map(
    (provider) => ({
      name: provider.name,
      vehicleType: provider.vehicleType,
      proxyCount: provider.proxyRecordIds.length,
      updatedAt: provider.updatedAt,
      subscriptionInfo: provider.subscriptionInfo,
    }),
  )
  const providerUnavailable = proxyView?.providerState === 'unavailable'

  const updateProvider = useLockFn(async (name: string) => {
    try {
      setUpdating((prev) => ({ ...prev, [name]: true }))
      await updateProxyProvider(name)
      await refreshProxy()
      void syncRuntimeProviders()
      showNotice.success(
        'proxies.feedback.notifications.provider.updateSuccess',
        { name },
      )
    } catch (err) {
      showNotice.error('proxies.feedback.notifications.provider.updateFailed', {
        name,
        message: String(err),
      })
    } finally {
      setUpdating((prev) => ({ ...prev, [name]: false }))
    }
  })

  const updateAllProviders = useLockFn(async () => {
    try {
      const names = providers.map(({ name }) => name)
      if (names.length === 0) {
        showNotice.info('proxies.feedback.notifications.provider.none')
        return
      }
      setUpdating(Object.fromEntries(names.map((name) => [name, true])))
      for (const name of names) {
        try {
          await updateProxyProvider(name)
          setUpdating((prev) => ({ ...prev, [name]: false }))
        } catch (err) {
          console.error(`更新 ${name} 失败`, err)
        }
      }
      await refreshProxy()
      void syncRuntimeProviders()
      showNotice.success('proxies.feedback.notifications.provider.allUpdated')
    } catch (err) {
      showNotice.error('proxies.feedback.notifications.provider.genericError', {
        message: String(err),
      })
    } finally {
      setUpdating({})
    }
  })

  return (
    <ProxyProviderButtonView
      providers={providers}
      providerUnavailable={providerUnavailable}
      updating={updating}
      onUpdate={updateProvider}
      onUpdateAll={updateAllProviders}
    />
  )
}
