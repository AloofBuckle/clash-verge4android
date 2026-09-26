import {
  InfoOutlined,
  LanguageRounded,
  LocationOnOutlined,
  NetworkCheckRounded,
  RefreshOutlined,
  SettingsOutlined,
  VisibilityOffOutlined,
  VisibilityOutlined,
} from '@mui/icons-material'
import {
  Box,
  Button,
  Chip,
  Divider,
  IconButton,
  Stack,
  Typography,
  alpha,
  keyframes,
} from '@mui/material'
import { fetch } from '@tauri-apps/plugin-http'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { BaseLoading } from '@/components/base/base-loading'
import { EnhancedCard } from '@/components/home/enhanced-card'

const DEFAULT_TESTS = [
  { name: 'Apple', url: 'https://www.apple.com' },
  { name: 'GitHub', url: 'https://www.github.com' },
  { name: 'Google', url: 'https://www.google.com' },
  { name: 'YouTube', url: 'https://www.youtube.com' },
]

interface IpInfo {
  ip: string
  country_code: string
  country: string
  region: string
  city: string
  organization: string
  asn: number
  asn_organization: string
  timezone: string
}

async function getMobileIpInfo(): Promise<IpInfo> {
  const services = [
    {
      url: 'https://api.ip.sb/geoip',
      map: (data: any): IpInfo => ({
        ip: data.ip || '',
        country_code: data.country_code || '',
        country: data.country || '',
        region: data.region || '',
        city: data.city || '',
        organization: data.organization || data.isp || '',
        asn: data.asn || 0,
        asn_organization: data.asn_organization || '',
        timezone: data.timezone || '',
      }),
    },
    {
      url: 'https://ipapi.co/json',
      map: (data: any): IpInfo => ({
        ip: data.ip || '',
        country_code: data.country_code || '',
        country: data.country_name || '',
        region: data.region || '',
        city: data.city || '',
        organization: data.org || '',
        asn: data.asn ? Number.parseInt(String(data.asn).replace('AS', ''), 10) : 0,
        asn_organization: data.org || '',
        timezone: data.timezone || '',
      }),
    },
    {
      url: 'https://ipwho.is/',
      map: (data: any): IpInfo => ({
        ip: data.ip || '',
        country_code: data.country_code || '',
        country: data.country || '',
        region: data.region || '',
        city: data.city || '',
        organization: data.connection?.org || data.connection?.isp || '',
        asn: data.connection?.asn || 0,
        asn_organization: data.connection?.isp || '',
        timezone: data.timezone?.id || '',
      }),
    },
  ]
  let lastError: unknown
  for (const service of services) {
    try {
      const response = await fetch(service.url, {
        method: 'GET',
        connectTimeout: 5000,
      })
      if (!response.ok) throw new Error(String(response.status))
      return service.map(await response.json())
    } catch (error) {
      lastError = error
    }
  }
  throw lastError ?? new Error('IP lookup failed')
}

const round = keyframes`
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
`

export function MobileTestCard() {
  const { t } = useTranslation()
  const [delays, setDelays] = useState<Record<string, number>>({})

  const testOne = useCallback(async (name: string, url: string) => {
    setDelays((old) => ({ ...old, [name]: -2 }))
    const start = performance.now()
    try {
      const response = await fetch(url, {
        method: 'GET',
        connectTimeout: 10_000,
      })
      if (!response.ok) throw new Error(String(response.status))
      setDelays((old) => ({
        ...old,
        [name]: Math.max(1, Math.round(performance.now() - start)),
      }))
    } catch {
      setDelays((old) => ({ ...old, [name]: 0 }))
    }
  }, [])

  const testAll = useCallback(() => {
    for (const item of DEFAULT_TESTS) void testOne(item.name, item.url)
  }, [testOne])

  return (
    <EnhancedCard
      title={t('home.components.tests.title')}
      icon={<NetworkCheckRounded />}
      action={
        <IconButton
          size="small"
          title={t('tests.page.actions.testAll')}
          onClick={testAll}
        >
          <NetworkCheckRounded fontSize="small" />
        </IconButton>
      }
    >
      <Box
        sx={{
          display: 'grid',
          gridTemplateColumns: 'repeat(2, minmax(0, 1fr))',
          gap: 1,
        }}
      >
        {DEFAULT_TESTS.map((item) => {
          const delay = delays[item.name] ?? -1
          return (
            <Box
              key={item.name}
              sx={(theme) => ({
                p: 1,
                borderRadius: 1.5,
                bgcolor: alpha(theme.palette.primary.main, 0.04),
                textAlign: 'center',
              })}
            >
              <LanguageRounded sx={{ height: 34 }} />
              <Typography variant="body2" noWrap>
                {item.name}
              </Typography>
              <Divider sx={{ my: 0.75 }} />
              {delay === -2 ? (
                <Box sx={{ display: 'flex', justifyContent: 'center', minHeight: 24 }}>
                  <BaseLoading />
                </Box>
              ) : (
                <Button
                  size="small"
                  onClick={() => void testOne(item.name, item.url)}
                >
                  {delay === -1
                    ? t('tests.components.item.actions.test')
                    : delay === 0
                      ? t('tests.statuses.test.failed')
                      : `${delay} ms`}
                </Button>
              )}
            </Box>
          )
        })}
      </Box>
    </EnhancedCard>
  )
}

export function MobileIpInfoCard() {
  const { t } = useTranslation()
  const [info, setInfo] = useState<IpInfo | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState(false)
  const [showIp, setShowIp] = useState(false)

  const load = useCallback(async () => {
    setLoading(true)
    setError(false)
    try {
      setInfo(await getMobileIpInfo())
    } catch {
      setError(true)
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void load()
    const timer = window.setInterval(() => void load(), 5 * 60 * 1000)
    return () => window.clearInterval(timer)
  }, [load])

  const countryFlag = useMemo(() => {
    if (!info?.country_code) return ''
    return String.fromCodePoint(
      ...info.country_code
        .toUpperCase()
        .split('')
        .map((char) => 127397 + char.charCodeAt(0)),
    )
  }, [info?.country_code])

  const row = (label: string, value?: string | number) => (
    <Stack direction="row" sx={{ justifyContent: 'space-between', gap: 1 }}>
      <Typography variant="body2" color="text.secondary">
        {label}
      </Typography>
      <Typography variant="body2" sx={{ textAlign: 'right' }}>
        {value || t('home.components.ipInfo.labels.unknown')}
      </Typography>
    </Stack>
  )

  return (
    <EnhancedCard
      title={t('home.components.ipInfo.title')}
      icon={<LocationOnOutlined />}
      action={
        <IconButton size="small" disabled={loading} onClick={() => void load()}>
          <RefreshOutlined
            sx={loading ? { animation: `1s linear infinite ${round}` } : undefined}
          />
        </IconButton>
      }
    >
      {loading && !info ? (
        <Box sx={{ display: 'flex', justifyContent: 'center', py: 2 }}>
          <BaseLoading />
        </Box>
      ) : error && !info ? (
        <Button onClick={() => void load()}>
          {t('home.components.ipInfo.errors.load')}
        </Button>
      ) : (
        <Stack spacing={1}>
          <Stack direction="row" sx={{ alignItems: 'center', gap: 1 }}>
            <Typography sx={{ fontSize: 24 }}>{countryFlag}</Typography>
            <Typography variant="subtitle1">{info?.country}</Typography>
          </Stack>
          <Stack direction="row" sx={{ justifyContent: 'space-between', alignItems: 'center' }}>
            <Typography variant="body2" color="text.secondary">
              {t('home.components.ipInfo.labels.ip')}
            </Typography>
            <Stack direction="row" sx={{ alignItems: 'center' }}>
              <Typography variant="body2" sx={{ fontFamily: 'monospace' }}>
                {showIp ? info?.ip : '••••••••••'}
              </Typography>
              <IconButton size="small" onClick={() => setShowIp((value) => !value)}>
                {showIp ? (
                  <VisibilityOffOutlined fontSize="small" />
                ) : (
                  <VisibilityOutlined fontSize="small" />
                )}
              </IconButton>
            </Stack>
          </Stack>
          {row(
            t('home.components.ipInfo.labels.asn'),
            info?.asn ? `AS${info.asn}` : undefined,
          )}
          {row(t('home.components.ipInfo.labels.isp'), info?.organization)}
          {row(t('home.components.ipInfo.labels.org'), info?.asn_organization)}
          {row(
            t('home.components.ipInfo.labels.location'),
            [info?.city, info?.region].filter(Boolean).join(', '),
          )}
          {row(t('home.components.ipInfo.labels.timezone'), info?.timezone)}
        </Stack>
      )}
    </EnhancedCard>
  )
}

export function MobileSystemInfoCard({
  appVersion,
  autoLaunch,
  running,
  serviceMode,
  lastCheckUpdate,
  onToggleAutoLaunch,
  onCheckUpdate,
  onSettings,
}: {
  appVersion: string
  autoLaunch: boolean
  running: boolean
  serviceMode: boolean
  lastCheckUpdate: number | null
  onToggleAutoLaunch: () => void
  onCheckUpdate: () => void
  onSettings: () => void
}) {
  const { t } = useTranslation()
  const mode = !running
    ? t('home.components.systemInfo.badges.notRunning')
    : serviceMode
      ? t('home.components.systemInfo.badges.serviceMode')
      : t('home.components.systemInfo.badges.sidecarMode')
  return (
    <EnhancedCard
      title={t('home.components.systemInfo.title')}
      icon={<InfoOutlined />}
      action={
        <IconButton size="small" onClick={onSettings}>
          <SettingsOutlined fontSize="small" />
        </IconButton>
      }
    >
      <Stack spacing={1.25}>
        {rowInfo(
          t('home.components.systemInfo.fields.osInfo'),
          `Android ${navigator.userAgent.match(/Android\s([^;]+)/)?.[1] ?? ''}`.trim(),
        )}
        <Divider />
        <Stack direction="row" sx={{ justifyContent: 'space-between', alignItems: 'center' }}>
          <Typography variant="body2" color="text.secondary">
            {t('home.components.systemInfo.fields.autoLaunch')}
          </Typography>
          <Chip
            size="small"
            label={autoLaunch ? t('shared.statuses.enabled') : t('shared.statuses.disabled')}
            color={autoLaunch ? 'success' : 'default'}
            variant={autoLaunch ? 'filled' : 'outlined'}
            onClick={onToggleAutoLaunch}
          />
        </Stack>
        <Divider />
        {rowInfo(t('home.components.systemInfo.fields.runningMode'), mode)}
        <Divider />
        <Stack direction="row" sx={{ justifyContent: 'space-between', gap: 1 }}>
          <Typography variant="body2" color="text.secondary">
            {t('home.components.systemInfo.fields.lastCheckUpdate')}
          </Typography>
          <Typography
            variant="body2"
            onClick={onCheckUpdate}
            sx={{ cursor: 'pointer', textDecoration: 'underline' }}
          >
            {lastCheckUpdate ? new Date(lastCheckUpdate).toLocaleString() : '-'}
          </Typography>
        </Stack>
        <Divider />
        {rowInfo(t('home.components.systemInfo.fields.vergeVersion'), `v${appVersion}`)}
      </Stack>
    </EnhancedCard>
  )
}

function rowInfo(label: string, value: string) {
  return (
    <Stack direction="row" sx={{ justifyContent: 'space-between', gap: 1 }}>
      <Typography variant="body2" color="text.secondary">
        {label}
      </Typography>
      <Typography variant="body2" sx={{ textAlign: 'right' }}>
        {value}
      </Typography>
    </Stack>
  )
}
