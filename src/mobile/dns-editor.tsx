import { RestartAltRounded } from '@mui/icons-material'
import {
  Alert,
  Box,
  Button,
  FormControl,
  List,
  ListItem,
  ListItemText,
  MenuItem,
  Select,
  TextField,
  Typography,
} from '@mui/material'
import * as yaml from 'js-yaml'
import { useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { Switch } from '@/components/base/base-switch'

import type { DnsOverrideSettings } from './api'

type Values = {
  enable: boolean
  listen: string
  enhancedMode: 'fake-ip' | 'redir-host'
  fakeIpRange: string
  fakeIpRange6: string
  fakeIpFilterMode: 'blacklist' | 'whitelist'
  preferH3: boolean
  respectRules: boolean
  useHosts: boolean
  useSystemHosts: boolean
  ipv6: boolean
  fakeIpFilter: string
  nameserver: string
  fallback: string
  defaultNameserver: string
  proxyServerNameserver: string
  directNameserver: string
  directNameserverFollowPolicy: boolean
  fallbackGeoip: boolean
  fallbackGeoipCode: string
  fallbackIpcidr: string
  fallbackDomain: string
  nameserverPolicy: string
  hosts: string
}

const DEFAULT_DNS_CONFIG = {
  enable: true,
  listen: ':53',
  'enhanced-mode': 'fake-ip' as const,
  'fake-ip-range': '198.18.0.1/16',
  'fake-ip-range6': 'fdfe:dcba:9876::1/64',
  'fake-ip-filter-mode': 'blacklist' as const,
  'prefer-h3': false,
  'respect-rules': false,
  'use-hosts': false,
  'use-system-hosts': false,
  ipv6: true,
  'fake-ip-filter': [
    '*.lan',
    '*.local',
    '*.arpa',
    'time.*.com',
    'ntp.*.com',
    '+.market.xiaomi.com',
    'localhost.ptlogin2.qq.com',
    '*.msftncsi.com',
    'www.msftconnecttest.com',
  ],
  'default-nameserver': [
    'system',
    '223.6.6.6',
    '8.8.8.8',
    '2400:3200::1',
    '2001:4860:4860::8888',
  ],
  nameserver: [
    '8.8.8.8',
    'https://doh.pub/dns-query',
    'https://dns.alidns.com/dns-query',
  ],
  fallback: [] as string[],
  'proxy-server-nameserver': [] as string[],
  'direct-nameserver': [] as string[],
  'direct-nameserver-follow-policy': false,
  'fallback-filter': {
    geoip: true,
    'geoip-code': 'CN',
    ipcidr: ['240.0.0.0/4', '0.0.0.0/32'],
    domain: ['+.google.com', '+.facebook.com', '+.youtube.com'],
  },
}

const parseList = (value: string) =>
  value
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean)

const formatPolicy = (value: unknown) =>
  value && typeof value === 'object'
    ? Object.entries(value as Record<string, unknown>)
        .map(([domain, servers]) =>
          `${domain}=${Array.isArray(servers) ? servers.join(';') : String(servers ?? '')}`,
        )
        .join(', ')
    : ''

const parsePolicy = (value: string) => {
  const result: Record<string, string[]> = {}
  const ruleRegex = /\s*([^=]+?)\s*=\s*([^,]+)(?:,|$)/g
  for (const match of value.matchAll(ruleRegex)) {
    result[match[1]!.trim()] = match[2]!
      .split(';')
      .map((item) => item.trim())
      .filter(Boolean)
  }
  return result
}

const formatHosts = (value: unknown) =>
  value && typeof value === 'object'
    ? Object.entries(value as Record<string, unknown>)
        .map(([domain, target]) =>
          `${domain}=${Array.isArray(target) ? target.join(';') : String(target ?? '')}`,
        )
        .join(', ')
    : ''

const parseHosts = (value: string) => {
  const result: Record<string, string | string[]> = {}
  for (const raw of value.split(',')) {
    const [domain, ...rest] = raw.trim().split('=')
    if (!domain || rest.length === 0) continue
    const target = rest.join('=').trim()
    result[domain.trim()] = target.includes(';')
      ? target
          .split(';')
          .map((item) => item.trim())
          .filter(Boolean)
      : target
  }
  return result
}

const defaults = (): Values => ({
  enable: DEFAULT_DNS_CONFIG.enable,
  listen: DEFAULT_DNS_CONFIG.listen,
  enhancedMode: DEFAULT_DNS_CONFIG['enhanced-mode'],
  fakeIpRange: DEFAULT_DNS_CONFIG['fake-ip-range'],
  fakeIpRange6: DEFAULT_DNS_CONFIG['fake-ip-range6'],
  fakeIpFilterMode: DEFAULT_DNS_CONFIG['fake-ip-filter-mode'],
  preferH3: DEFAULT_DNS_CONFIG['prefer-h3'],
  respectRules: DEFAULT_DNS_CONFIG['respect-rules'],
  useHosts: DEFAULT_DNS_CONFIG['use-hosts'],
  useSystemHosts: DEFAULT_DNS_CONFIG['use-system-hosts'],
  ipv6: DEFAULT_DNS_CONFIG.ipv6,
  fakeIpFilter: DEFAULT_DNS_CONFIG['fake-ip-filter'].join(', '),
  defaultNameserver: DEFAULT_DNS_CONFIG['default-nameserver'].join(', '),
  nameserver: DEFAULT_DNS_CONFIG.nameserver.join(', '),
  fallback: DEFAULT_DNS_CONFIG.fallback.join(', '),
  proxyServerNameserver: DEFAULT_DNS_CONFIG['proxy-server-nameserver'].join(', '),
  directNameserver: DEFAULT_DNS_CONFIG['direct-nameserver'].join(', '),
  directNameserverFollowPolicy:
    DEFAULT_DNS_CONFIG['direct-nameserver-follow-policy'],
  fallbackGeoip: DEFAULT_DNS_CONFIG['fallback-filter'].geoip,
  fallbackGeoipCode: DEFAULT_DNS_CONFIG['fallback-filter']['geoip-code'],
  fallbackIpcidr: DEFAULT_DNS_CONFIG['fallback-filter'].ipcidr.join(', '),
  fallbackDomain: DEFAULT_DNS_CONFIG['fallback-filter'].domain.join(', '),
  nameserverPolicy: '',
  hosts: '',
})

function valuesFromYaml(content: string, enabled: boolean): Values {
  const base = defaults()
  if (!content.trim()) return { ...base, enable: enabled }
  const config = yaml.load(content) as Record<string, any> | null
  if (!config || typeof config !== 'object') return { ...base, enable: enabled }
  const dns = config.dns ?? {}
  const enhancedMode = dns['enhanced-mode']
  const fakeIpFilterMode = dns['fake-ip-filter-mode']
  return {
    enable: enabled,
    listen: dns.listen ?? base.listen,
    enhancedMode:
      enhancedMode === 'redir-host' || enhancedMode === 'fake-ip'
        ? enhancedMode
        : base.enhancedMode,
    fakeIpRange: dns['fake-ip-range'] ?? base.fakeIpRange,
    fakeIpRange6: dns['fake-ip-range6'] ?? base.fakeIpRange6,
    fakeIpFilterMode:
      fakeIpFilterMode === 'whitelist' || fakeIpFilterMode === 'blacklist'
        ? fakeIpFilterMode
        : base.fakeIpFilterMode,
    preferH3: dns['prefer-h3'] ?? base.preferH3,
    respectRules: dns['respect-rules'] ?? base.respectRules,
    useHosts: dns['use-hosts'] ?? base.useHosts,
    useSystemHosts: dns['use-system-hosts'] ?? base.useSystemHosts,
    ipv6: dns.ipv6 ?? base.ipv6,
    fakeIpFilter: dns['fake-ip-filter']?.join(', ') ?? base.fakeIpFilter,
    defaultNameserver:
      dns['default-nameserver']?.join(', ') ?? base.defaultNameserver,
    nameserver: dns.nameserver?.join(', ') ?? base.nameserver,
    fallback: dns.fallback?.join(', ') ?? base.fallback,
    proxyServerNameserver:
      dns['proxy-server-nameserver']?.join(', ') ?? base.proxyServerNameserver,
    directNameserver:
      dns['direct-nameserver']?.join(', ') ?? base.directNameserver,
    directNameserverFollowPolicy:
      dns['direct-nameserver-follow-policy'] ?? base.directNameserverFollowPolicy,
    fallbackGeoip: dns['fallback-filter']?.geoip ?? base.fallbackGeoip,
    fallbackGeoipCode:
      dns['fallback-filter']?.['geoip-code'] ?? base.fallbackGeoipCode,
    fallbackIpcidr:
      dns['fallback-filter']?.ipcidr?.join(', ') ?? base.fallbackIpcidr,
    fallbackDomain:
      dns['fallback-filter']?.domain?.join(', ') ?? base.fallbackDomain,
    nameserverPolicy: formatPolicy(dns['nameserver-policy']),
    hosts: formatHosts(config.hosts),
  }
}

function settingsFromValues(values: Values): DnsOverrideSettings {
  const config = {
    dns: {
      enable: values.enable,
      listen: values.listen,
      'enhanced-mode': values.enhancedMode,
      'fake-ip-range': values.fakeIpRange,
      'fake-ip-range6': values.fakeIpRange6,
      'fake-ip-filter-mode': values.fakeIpFilterMode,
      'prefer-h3': values.preferH3,
      'respect-rules': values.respectRules,
      'use-hosts': values.useHosts,
      'use-system-hosts': values.useSystemHosts,
      ipv6: values.ipv6,
      'fake-ip-filter': parseList(values.fakeIpFilter),
      'default-nameserver': parseList(values.defaultNameserver),
      nameserver: parseList(values.nameserver),
      fallback: parseList(values.fallback),
      'proxy-server-nameserver': parseList(values.proxyServerNameserver),
      'direct-nameserver': parseList(values.directNameserver),
      'direct-nameserver-follow-policy': values.directNameserverFollowPolicy,
      'nameserver-policy': parsePolicy(values.nameserverPolicy),
      'fallback-filter': {
        geoip: values.fallbackGeoip,
        'geoip-code': values.fallbackGeoipCode,
        ipcidr: parseList(values.fallbackIpcidr),
        domain: parseList(values.fallbackDomain),
      },
    },
    hosts: parseHosts(values.hosts),
  }
  return {
    enabled: values.enable,
    yaml: yaml.dump(config, { forceQuotes: true }),
  }
}

export function MobileDnsEditor({
  settings,
  disabled,
  onChange,
}: {
  settings: DnsOverrideSettings
  disabled: boolean
  onChange: (settings: DnsOverrideSettings) => void
}) {
  const { t } = useTranslation()
  const [visualization, setVisualization] = useState(true)
  const [values, setValues] = useState<Values>(() =>
    valuesFromYaml(settings.yaml, settings.enabled),
  )
  const [raw, setRaw] = useState(settings.yaml)

  useEffect(() => {
    setValues(valuesFromYaml(settings.yaml, settings.enabled))
    setRaw(settings.yaml)
  }, [settings.enabled, settings.yaml])

  const emitted = useMemo(() => settingsFromValues(values), [values])
  useEffect(() => {
    if (!visualization) return
    setRaw(emitted.yaml)
    if (emitted.enabled === settings.enabled && emitted.yaml === settings.yaml) {
      return
    }
    onChange(emitted)
  }, [emitted, onChange, settings.enabled, settings.yaml, visualization])

  const set = <K extends keyof Values>(key: K, value: Values[K]) =>
    setValues((old) => ({ ...old, [key]: value }))
  const toggle = (key: keyof Values) => (_: unknown, checked: boolean) =>
    set(key, checked as never)
  const text = (key: keyof Values) => (event: React.ChangeEvent<HTMLInputElement>) =>
    set(key, event.target.value as never)
  const multiline = (
    key: keyof Values,
    primary: string,
    secondary?: string,
  ) => (
    <ListItem sx={{ px: 0, flexDirection: 'column', alignItems: 'flex-start' }}>
      <ListItemText primary={primary} secondary={secondary} />
      <TextField
        fullWidth
        multiline
        minRows={2}
        maxRows={4}
        size="small"
        disabled={disabled}
        spellCheck={false}
        value={String(values[key])}
        onChange={text(key)}
      />
    </ListItem>
  )

  return (
    <>
      <Box sx={{ display: 'flex', justifyContent: 'flex-end', gap: 1, mb: 1 }}>
        <Button
          variant="outlined"
          size="small"
          color="warning"
          startIcon={<RestartAltRounded />}
          disabled={disabled}
          onClick={() => setValues(defaults())}
        >
          {t('shared.actions.resetToDefault')}
        </Button>
        <Button
          variant="contained"
          size="small"
          disabled={disabled}
          onClick={() => {
            if (!visualization) {
              try {
                const next = valuesFromYaml(raw, settings.enabled)
                setValues(next)
                onChange({ enabled: next.enable, yaml: raw })
              } catch {
                return
              }
            }
            setVisualization((value) => !value)
          }}
        >
          {visualization
            ? t('shared.editorModes.advanced')
            : t('shared.editorModes.visualization')}
        </Button>
      </Box>
      <Alert severity="info" sx={{ mb: 2 }}>
        {t('settings.modals.dns.dialog.profileScope')}
      </Alert>
      <Typography variant="body2" color="warning.main" sx={{ mb: 2, fontStyle: 'italic' }}>
        {t('settings.modals.dns.dialog.warning')}
      </Typography>
      {!visualization ? (
        <TextField
          fullWidth
          multiline
          minRows={18}
          disabled={disabled}
          value={raw}
          onChange={(event) => {
            setRaw(event.target.value)
            onChange({ enabled: settings.enabled, yaml: event.target.value })
          }}
          sx={{ '& textarea': { fontFamily: 'monospace' } }}
        />
      ) : (
        <List disablePadding>
          <Typography variant="subtitle1" sx={{ mt: 1, mb: 1, fontWeight: 'bold' }}>
            {t('settings.modals.dns.sections.general')}
          </Typography>
          <ListItem sx={{ px: 0 }}>
            <ListItemText primary={t('settings.modals.dns.fields.enable')} />
            <Switch edge="end" checked={values.enable} disabled={disabled} onChange={toggle('enable')} />
          </ListItem>
          <ListItem sx={{ px: 0 }}>
            <ListItemText primary={t('settings.modals.dns.fields.listen')} />
            <TextField size="small" disabled={disabled} value={values.listen} onChange={text('listen')} sx={{ width: 150 }} />
          </ListItem>
          <ListItem sx={{ px: 0 }}>
            <ListItemText primary={t('settings.modals.dns.fields.enhancedMode')} />
            <FormControl size="small" sx={{ width: 150 }}>
              <Select value={values.enhancedMode} disabled={disabled} onChange={(event) => set('enhancedMode', event.target.value as Values['enhancedMode'])}>
                <MenuItem value="fake-ip">fake-ip</MenuItem>
                <MenuItem value="redir-host">redir-host</MenuItem>
              </Select>
            </FormControl>
          </ListItem>
          <ListItem sx={{ px: 0 }}><ListItemText primary={t('settings.modals.dns.fields.fakeIpRange')} /><TextField size="small" disabled={disabled} value={values.fakeIpRange} onChange={text('fakeIpRange')} sx={{ width: 170 }} /></ListItem>
          <ListItem sx={{ px: 0 }}><ListItemText primary={t('settings.modals.dns.fields.fakeIpRange6')} /><TextField size="small" disabled={disabled} value={values.fakeIpRange6} onChange={text('fakeIpRange6')} sx={{ width: 210 }} /></ListItem>
          <ListItem sx={{ px: 0 }}><ListItemText primary={t('settings.modals.dns.fields.fakeIpFilterMode')} /><FormControl size="small" sx={{ width: 150 }}><Select value={values.fakeIpFilterMode} disabled={disabled} onChange={(event) => set('fakeIpFilterMode', event.target.value as Values['fakeIpFilterMode'])}><MenuItem value="blacklist">blacklist</MenuItem><MenuItem value="whitelist">whitelist</MenuItem></Select></FormControl></ListItem>
          {([
            ['ipv6', 'settings.modals.dns.fields.ipv6.label', 'settings.modals.dns.fields.ipv6.description'],
            ['preferH3', 'settings.modals.dns.fields.preferH3.label', 'settings.modals.dns.fields.preferH3.description'],
            ['respectRules', 'settings.modals.dns.fields.respectRules.label', 'settings.modals.dns.fields.respectRules.description'],
            ['useHosts', 'settings.modals.dns.fields.useHosts.label', 'settings.modals.dns.fields.useHosts.description'],
            ['useSystemHosts', 'settings.modals.dns.fields.useSystemHosts.label', 'settings.modals.dns.fields.useSystemHosts.description'],
            ['directNameserverFollowPolicy', 'settings.modals.dns.fields.directPolicy.label', 'settings.modals.dns.fields.directPolicy.description'],
          ] as const).map(([key, label, description]) => (
            <ListItem key={key} sx={{ px: 0 }}>
              <ListItemText primary={t(label)} secondary={t(description)} />
              <Switch edge="end" checked={Boolean(values[key])} disabled={disabled} onChange={toggle(key)} />
            </ListItem>
          ))}
          {multiline('defaultNameserver', t('settings.modals.dns.fields.defaultNameserver.label'), t('settings.modals.dns.fields.defaultNameserver.description'))}
          {multiline('nameserver', t('settings.modals.dns.fields.nameserver.label'), t('settings.modals.dns.fields.nameserver.description'))}
          {multiline('fallback', t('settings.modals.dns.fields.fallback.label'), t('settings.modals.dns.fields.fallback.description'))}
          {multiline('proxyServerNameserver', t('settings.modals.dns.fields.proxy.label'), t('settings.modals.dns.fields.proxy.description'))}
          {multiline('directNameserver', t('settings.modals.dns.fields.directNameserver.label'), t('settings.modals.dns.fields.directNameserver.description'))}
          {multiline('fakeIpFilter', t('settings.modals.dns.fields.fakeIpFilter.label'), t('settings.modals.dns.fields.fakeIpFilter.description'))}
          {multiline('nameserverPolicy', t('settings.modals.dns.fields.nameserverPolicy.label'), t('settings.modals.dns.fields.nameserverPolicy.description'))}
          <Typography variant="subtitle2" sx={{ mt: 2, mb: 1, fontWeight: 'bold' }}>
            {t('settings.modals.dns.sections.fallbackFilter')}
          </Typography>
          <ListItem sx={{ px: 0 }}><ListItemText primary={t('settings.modals.dns.fields.geoipFiltering.label')} secondary={t('settings.modals.dns.fields.geoipFiltering.description')} /><Switch edge="end" checked={values.fallbackGeoip} disabled={disabled} onChange={toggle('fallbackGeoip')} /></ListItem>
          <ListItem sx={{ px: 0 }}><ListItemText primary={t('settings.modals.dns.fields.geoipCode')} /><TextField size="small" disabled={disabled} value={values.fallbackGeoipCode} onChange={text('fallbackGeoipCode')} sx={{ width: 100 }} /></ListItem>
          {multiline('fallbackIpcidr', t('settings.modals.dns.fields.fallbackIpCidr.label'), t('settings.modals.dns.fields.fallbackIpCidr.description'))}
          {multiline('fallbackDomain', t('settings.modals.dns.fields.fallbackDomain.label'), t('settings.modals.dns.fields.fallbackDomain.description'))}
          <Typography variant="subtitle1" sx={{ mt: 3, mb: 0, fontWeight: 'bold' }}>
            {t('settings.modals.dns.sections.hosts')}
          </Typography>
          {multiline('hosts', t('settings.modals.dns.fields.hosts.label'), t('settings.modals.dns.fields.hosts.description'))}
        </List>
      )}
    </>
  )
}
