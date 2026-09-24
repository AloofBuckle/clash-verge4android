import { useLockFn } from 'ahooks'
import { useState } from 'react'
import { updateRuleProvider } from 'tauri-plugin-mihomo-api'

import { useAppRefreshers, useRulesData } from '@/providers/app-data-context'
import { syncRuntimeProviders } from '@/services/cmds'
import { showNotice } from '@/services/notice-service'

import {
  RuleProviderButtonView,
  type RuleProviderButtonItem,
} from './provider-button-view'

export const ProviderButton = () => {
  const { ruleProviders } = useRulesData()
  const { refreshRules, refreshRuleProviders } = useAppRefreshers()
  const [updating, setUpdating] = useState<Record<string, boolean>>({})
  const providers: RuleProviderButtonItem[] = Object.entries(
    ruleProviders ?? {},
  )
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([name, provider]) => ({
      name,
      vehicleType:
        typeof provider.vehicleType === 'string'
          ? provider.vehicleType
          : provider.vehicleType.Unknown,
      behavior:
        typeof provider.behavior === 'string'
          ? provider.behavior
          : provider.behavior.Unknown,
      ruleCount: provider.ruleCount,
      updatedAt: provider.updatedAt,
    }))

  const refresh = async () => {
    await refreshRules()
    await refreshRuleProviders()
    void syncRuntimeProviders()
  }

  const updateProvider = useLockFn(async (name: string) => {
    try {
      setUpdating((prev) => ({ ...prev, [name]: true }))
      await updateRuleProvider(name)
      await refresh()
      showNotice.success(
        'rules.feedback.notifications.provider.updateSuccess',
        { name },
      )
    } catch (err) {
      showNotice.error('rules.feedback.notifications.provider.updateFailed', {
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
        showNotice.info('rules.feedback.notifications.provider.none')
        return
      }
      setUpdating(Object.fromEntries(names.map((name) => [name, true])))
      for (const name of names) {
        try {
          await updateRuleProvider(name)
          setUpdating((prev) => ({ ...prev, [name]: false }))
        } catch (err) {
          console.error(`更新 ${name} 失败`, err)
        }
      }
      await refresh()
      showNotice.success('rules.feedback.notifications.provider.allUpdated')
    } catch (err) {
      showNotice.error('rules.feedback.notifications.provider.genericError', {
        message: String(err),
      })
    } finally {
      setUpdating({})
    }
  })

  return (
    <RuleProviderButtonView
      providers={providers}
      updating={updating}
      onUpdate={updateProvider}
      onUpdateAll={updateAllProviders}
    />
  )
}
