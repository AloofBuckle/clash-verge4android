import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { ProxyProviderButtonView } from '@/components/proxy/provider-button-view'
import { RuleProviderButtonView } from '@/components/rule/provider-button-view'

import {
  api,
  type ProxyProviderSummary,
  type RuleProviderSummary,
} from './api'

type Feedback = (text: string, error?: boolean) => void

export function MobileProxyProviderButton({
  enabled,
  onFeedback,
}: {
  enabled: boolean
  onFeedback: Feedback
}) {
  const { t } = useTranslation()
  const [providers, setProviders] = useState<ProxyProviderSummary[]>([])
  const [unavailable, setUnavailable] = useState(false)
  const [updating, setUpdating] = useState<Record<string, boolean>>({})

  useEffect(() => {
    let alive = true
    if (!enabled) {
      setProviders([])
      setUnavailable(false)
      return
    }
    void api
      .proxyProviders()
      .then((value) => {
        if (!alive) return
        setProviders(value)
        setUnavailable(false)
      })
      .catch(() => {
        if (alive) setUnavailable(true)
      })
    return () => {
      alive = false
    }
  }, [enabled])

  const update = async (name: string) => {
    setUpdating((old) => ({ ...old, [name]: true }))
    try {
      setProviders(await api.updateProxyProvider(name))
      setUnavailable(false)
      onFeedback(t('proxies.feedback.notifications.provider.updateSuccess', { name }))
    } catch (error) {
      onFeedback(String(error), true)
    } finally {
      setUpdating((old) => ({ ...old, [name]: false }))
    }
  }

  const updateAll = async () => {
    const names = providers.map((provider) => provider.name)
    setUpdating(Object.fromEntries(names.map((name) => [name, true])))
    try {
      let latest = providers
      for (const name of names) {
        latest = await api.updateProxyProvider(name)
        setUpdating((old) => ({ ...old, [name]: false }))
      }
      setProviders(latest)
      setUnavailable(false)
      onFeedback(t('proxies.feedback.notifications.provider.allUpdated'))
    } catch (error) {
      onFeedback(String(error), true)
    } finally {
      setUpdating({})
    }
  }

  return (
    <ProxyProviderButtonView
      providers={providers}
      providerUnavailable={unavailable}
      updating={updating}
      onUpdate={update}
      onUpdateAll={updateAll}
    />
  )
}

export function MobileRuleProviderButton({
  enabled,
  onFeedback,
}: {
  enabled: boolean
  onFeedback: Feedback
}) {
  const { t } = useTranslation()
  const [providers, setProviders] = useState<RuleProviderSummary[]>([])
  const [updating, setUpdating] = useState<Record<string, boolean>>({})

  useEffect(() => {
    let alive = true
    if (!enabled) {
      setProviders([])
      return
    }
    void api
      .ruleProviders()
      .then((value) => alive && setProviders(value))
      .catch(() => alive && setProviders([]))
    return () => {
      alive = false
    }
  }, [enabled])

  const update = async (name: string) => {
    setUpdating((old) => ({ ...old, [name]: true }))
    try {
      setProviders(await api.updateRuleProvider(name))
      onFeedback(t('rules.feedback.notifications.provider.updateSuccess', { name }))
    } catch (error) {
      onFeedback(String(error), true)
    } finally {
      setUpdating((old) => ({ ...old, [name]: false }))
    }
  }

  const updateAll = async () => {
    const names = providers.map((provider) => provider.name)
    setUpdating(Object.fromEntries(names.map((name) => [name, true])))
    try {
      let latest = providers
      for (const name of names) {
        latest = await api.updateRuleProvider(name)
        setUpdating((old) => ({ ...old, [name]: false }))
      }
      setProviders(latest)
      onFeedback(t('rules.feedback.notifications.provider.allUpdated'))
    } catch (error) {
      onFeedback(String(error), true)
    } finally {
      setUpdating({})
    }
  }

  return (
    <RuleProviderButtonView
      providers={providers}
      updating={updating}
      onUpdate={update}
      onUpdateAll={updateAll}
    />
  )
}
