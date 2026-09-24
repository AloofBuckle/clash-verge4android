import {
  AccessTimeRounded,
  ArrowDownwardRounded,
  ArrowUpwardRounded,
  AddRounded,
  CheckBoxOutlineBlankRounded,
  CheckBoxRounded,
  CloudUploadOutlined,
  ComputerRounded,
  ContentCopyRounded,
  ChevronRight,
  DeleteOutlineRounded,
  DeleteRounded,
  DeveloperBoardOutlined,
  DnsOutlined,
  ForkRightOutlined,
  HomeOutlined,
  IndeterminateCheckBoxRounded,
  LanguageOutlined,
  LanOutlined,
  LanRounded,
  MenuRounded,
  MoreVertRounded,
  NetworkCheckRounded,
  PauseCircleOutlineRounded,
  PlayCircleOutlineRounded,
  RefreshRounded,
  RouterOutlined,
  SettingsOutlined,
  SortByAlphaRounded,
  SortRounded,
  SignalWifi0Bar,
  SignalWifi2Bar,
  SignalWifi4Bar,
  SpeedOutlined,
  StorageOutlined,
  SubjectOutlined,
  SwapVertRounded,
  TextSnippetOutlined,
  TroubleshootRounded,
  WifiOutlined,
  WifiOff,
} from '@mui/icons-material'
import {
  Alert,
  Box,
  Button,
  ButtonGroup,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Divider,
  Drawer,
  Grid,
  IconButton,
  List,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  Menu,
  MenuItem,
  Paper,
  Stack,
  TextField,
  Tooltip,
  Typography,
  alpha,
} from '@mui/material'
import dayjs from 'dayjs'
import relativeTime from 'dayjs/plugin/relativeTime'
import {
  type FormEvent,
  type ReactNode,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react'
import { createRoot } from 'react-dom/client'
import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog'
import { readTextFile, remove, writeTextFile } from '@tauri-apps/plugin-fs'

import { BaseSearchBox, type SearchState } from '@/components/base/base-search-box'
import { BaseStyledSelect } from '@/components/base/base-styled-select'
import { Switch } from '@/components/base/base-switch'
import { NoticeManager } from '@/components/layout/notice-manager'
import {
  ConnectionDetail,
  type ConnectionDetailRef,
} from '@/components/connection/connection-detail'
import { ConnectionRowItem } from '@/components/connection/connection-row-item'
import {
  formatConnectionChains,
  formatConnectionTraffic,
  type ConnectionRowView,
} from '@/components/connection/connection-row-view'
import { EnhancedCard } from '@/components/home/enhanced-card'
import { PersistentSelect } from '@/components/home/current-proxy-select'
import LogItem from '@/components/log/log-item'
import { ProfileBox } from '@/components/profile/profile-box'
import { ProxyChainCard } from '@/components/proxy/proxy-chain-card'
import { ProxyItemView } from '@/components/proxy/proxy-item-view'
import type { ProxyChainItem } from '@/components/proxy/proxy-chain-model'
import RuleItem from '@/components/rule/rule-item'
import { SettingItem, SettingList } from '@/components/setting/mods/setting-comp'
import { StackModeSwitch } from '@/components/setting/mods/stack-mode-switch'
import { ThemeModeSwitch } from '@/components/setting/mods/theme-mode-switch'
import { NetworkInterfaceContent } from '@/components/setting/mods/network-interface-content'
import { initializeLanguage } from '@/services/i18n'
import { showNotice } from '@/services/notice-service'
import zhConnections from '@/locales/zh/connections.json'
import zhHome from '@/locales/zh/home.json'
import zhLayout from '@/locales/zh/layout.json'
import zhLogs from '@/locales/zh/logs.json'
import zhProfiles from '@/locales/zh/profiles.json'
import zhProxies from '@/locales/zh/proxies.json'
import zhRules from '@/locales/zh/rules.json'
import zhSettings from '@/locales/zh/settings.json'
import zhShared from '@/locales/zh/shared.json'

import {
  api,
  hasBackend,
  unavailable,
  type BootModuleStatus,
  type BackupSettings,
  type Capabilities,
  type CoreConnection,
  type ConnectionsSnapshot,
  type CoreSnapshot,
  type DnsOverrideSettings,
  type ExternalControllerSettings,
  type LocalBackupInfo,
  type MobilePreferences,
  type NetworkInterfaceInfo,
  type PortSettings,
  type Profile,
  type ProfileDocument,
  type ProfileOptions,
  type RootProbe,
  type RuntimePreferences,
  type RuntimeStatus,
  type SystemProxyStatus,
  type Summary,
  type TunPreferences,
  type WebDavBackupInfo,
} from './api'
import {
  MobileEmpty,
  MobileModeButtons,
  MobileTrafficStats,
} from './verge-controls'
import {
  VergeMobileTheme,
  type MobileThemeMode,
} from './verge-theme'
import {
  MobileProxyProviderButton,
  MobileRuleProviderButton,
} from './provider-buttons'
import './mobile.css'

dayjs.extend(relativeTime)

type Page =
  | 'home'
  | 'proxies'
  | 'profiles'
  | 'rules'
  | 'connections'
  | 'logs'
  | 'settings'
type Form =
  | { kind: 'subscription' | 'yaml' | 'nodes' }
  | { kind: 'copy' | 'edit' | 'delete'; profile: Profile }
type MergeTarget = { kind: 'global' } | { kind: 'profile'; profile: Profile }
type SequenceKind = 'rules' | 'proxies' | 'groups'
type SequenceTarget = { kind: SequenceKind; profile: Profile }
type ScriptTarget = { kind: 'global' } | { kind: 'profile'; profile: Profile }
const initialDocument: ProfileDocument = { activeId: null, profiles: [] }
const defaultProfileOptions: ProfileOptions = {
  selfProxy: false,
  withProxy: false,
  allowAutoUpdate: true,
  dangerAcceptInvalidCerts: false,
}
const initialSearchState: SearchState = {
  text: '',
  matchCase: false,
  matchWholeWord: false,
  useRegularExpression: false,
}
const portFields = [
  {
    key: 'mixedPort' as const,
    label: zhSettings.modals.clashPort.fields.mixed,
    fallback: 7890,
  },
  {
    key: 'httpPort' as const,
    label: zhSettings.modals.clashPort.fields.http,
    fallback: 7899,
  },
  {
    key: 'socksPort' as const,
    label: zhSettings.modals.clashPort.fields.socks,
    fallback: 7898,
  },
  {
    key: 'redirPort' as const,
    label: zhSettings.modals.clashPort.fields.redir,
    fallback: 7895,
  },
  {
    key: 'tproxyPort' as const,
    label: zhSettings.modals.clashPort.fields.tproxy,
    fallback: 7896,
  },
]
const defaultProfileScript = `// Define main function (script entry)

function main(config, profileName) {
  return config;
}
`
const labels: Record<Page, string> = {
  home: zhHome.page.title,
  proxies: zhProxies.page.title.default,
  profiles: zhProfiles.page.title,
  rules: zhRules.page.title,
  connections: zhConnections.page.title,
  logs: zhLogs.page.title,
  settings: zhSettings.page.title,
}
const navLabels: Record<Page, string> = {
  home: zhLayout.components.navigation.tabs.home,
  proxies: zhLayout.components.navigation.tabs.proxies,
  profiles: zhLayout.components.navigation.tabs.profiles,
  connections: zhLayout.components.navigation.tabs.connections,
  rules: zhLayout.components.navigation.tabs.rules,
  logs: zhLayout.components.navigation.tabs.logs,
  settings: zhLayout.components.navigation.tabs.settings,
}
const navPages: Page[] = [
  'home',
  'proxies',
  'profiles',
  'connections',
  'rules',
  'logs',
  'settings',
]
const navIcons: Record<Page, ReactNode> = {
  home: <HomeOutlined />,
  proxies: <WifiOutlined />,
  profiles: <DnsOutlined />,
  connections: <LanguageOutlined />,
  rules: <ForkRightOutlined />,
  logs: <SubjectOutlined />,
  settings: <SettingsOutlined />,
}
function formatBytes(value: number) {
  if (!Number.isFinite(value) || value <= 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const index = Math.min(Math.floor(Math.log(value) / Math.log(1024)), units.length - 1)
  const scaled = value / 1024 ** index
  return `${scaled >= 100 || index === 0 ? scaled.toFixed(0) : scaled.toFixed(1)} ${units[index]}`
}
function formatRate(value: number) {
  return `${formatBytes(value)}/s`
}
function proxyDelayLabel(value: number | null | undefined) {
  if (value === -2) return zhHome.components.currentProxy.status.testing
  if (typeof value !== 'number' || value <= 0) return '—'
  if (value >= 10_000) return zhHome.components.currentProxy.status.timeout
  return `${value} ms`
}
function proxyDelayColor(
  value: number | null | undefined,
): 'success' | 'warning' | 'error' | 'default' {
  if (typeof value !== 'number' || value <= 0) return 'default'
  if (value >= 10_000) return 'error'
  if (value >= 500) return 'error'
  if (value >= 300) return 'warning'
  return 'success'
}
function sourceLabel(source: string | null) {
  if (!source) return zhProfiles.modals.profileForm.types.local
  try {
    return new URL(source).host
  } catch {
    return source
  }
}
function parseLogLine(line: string): ILogItem {
  const time = line.match(/\btime="([^"]+)"/)?.[1]
  const type = line.match(/\blevel=([a-zA-Z]+)/)?.[1] ?? 'info'
  const payload = line.match(/\bmsg="(.*)"$/)?.[1]?.replace(/\\n/g, '\n') ?? line
  return { time, type, payload }
}
function splitEndpoint(value: string) {
  const index = value.lastIndexOf(':')
  if (index <= 0) return { host: value, port: '' }
  return {
    host: value.slice(0, index).replace(/^\[|\]$/g, ''),
    port: value.slice(index + 1),
  }
}
function connectionDetailValue(connection: CoreConnection): IConnectionsItem {
  const source = splitEndpoint(connection.source)
  const destination = splitEndpoint(connection.destination)
  return {
    id: connection.id,
    metadata: {
      network: connection.network,
      type: connection.inbound || 'TUN',
      host: connection.host,
      sourceIP: source.host,
      sourcePort: source.port,
      destinationIP: destination.host,
      remoteDestination: destination.host,
      destinationPort: destination.port,
      process: connection.process,
      processPath: '',
    },
    upload: connection.upload,
    download: connection.download,
    start: connection.start,
    chains: connection.chains,
    rule: connection.rule,
    rulePayload: connection.rulePayload,
  }
}
function chainCandidateNames(
  summary: Summary | null,
  core: CoreSnapshot | null,
  targetGroup: string,
) {
  if (!summary || !targetGroup) return []
  const concrete = new Set(summary.nodes.map((node) => node.name))
  if (targetGroup === 'GLOBAL') return [...concrete]
  const groups = new Map((core?.groups ?? []).map((group) => [group.name, group]))
  const result = new Set<string>()
  const visiting = new Set<string>()
  const visit = (name: string) => {
    if (concrete.has(name)) {
      result.add(name)
      return
    }
    if (visiting.has(name)) return
    const group = groups.get(name)
    if (!group) return
    visiting.add(name)
    for (const member of group.members) visit(member)
    visiting.delete(name)
  }
  visit(targetGroup)
  return [...result]
}
function App() {
  const [themeMode, setThemeMode] = useState<MobileThemeMode>(() => {
    const saved = window.localStorage.getItem('cv4a-theme-mode')
    return saved === 'light' || saved === 'dark' || saved === 'system'
      ? saved
      : 'system'
  })
  const [page, setPage] = useState<Page>('home')
  const [navOpen, setNavOpen] = useState(false)
  const [caps, setCaps] = useState<Capabilities>(unavailable)
  const [doc, setDoc] = useState(initialDocument)
  const [summary, setSummary] = useState<Summary | null>(null)
  const [busy, setBusy] = useState(false)
  const [loading, setLoading] = useState(true)
  const [form, setForm] = useState<Form | null>(null)
  const [name, setName] = useState('')
  const [content, setContent] = useState('')
  const [sourceUrl, setSourceUrl] = useState('')
  const [profileOption, setProfileOption] = useState<ProfileOptions>(
    defaultProfileOptions,
  )
  const [formError, setFormError] = useState('')
  const [proxySearchState, setProxySearchState] = useState<SearchState>(initialSearchState)
  const [proxySort, setProxySort] = useState<0 | 1 | 2>(0)
  const [, setRuleSearchState] = useState<SearchState>(initialSearchState)
  const [connectionSearchState, setConnectionSearchState] =
    useState<SearchState>(initialSearchState)
  const [connectionSort, setConnectionSort] = useState<'start' | 'upload' | 'download'>('start')
  const [logSearchState, setLogSearchState] = useState<SearchState>(initialSearchState)
  const [logFilter, setLogFilter] = useState<'all' | 'debug' | 'info' | 'warn' | 'err'>('all')
  const [logEnabled, setLogEnabled] = useState(true)
  const [logOrder, setLogOrder] = useState<'asc' | 'desc'>('asc')
  const [root, setRoot] = useState<RootProbe | null>(null)
  const [runtime, setRuntime] = useState<RuntimeStatus | null>(null)
  const [systemProxy, setSystemProxy] = useState<SystemProxyStatus | null>(null)
  const [networkPanel, setNetworkPanel] = useState<'system' | 'tun'>('system')
  const [homeProxyGroupName, setHomeProxyGroupName] = useState('')
  const [homeProxySelect, setHomeProxySelect] = useState<'group' | 'proxy' | null>(null)
  const [homeProxySort, setHomeProxySort] = useState<0 | 1 | 2>(() => {
    const saved = window.localStorage.getItem('cv4a-home-proxy-sort')
    return saved === '1' ? 1 : saved === '2' ? 2 : 0
  })
  const [runtimePreferences, setRuntimePreferences] =
    useState<RuntimePreferences | null>(null)
  const [mobilePreferences, setMobilePreferences] =
    useState<MobilePreferences | null>(null)
  const [tunPreferences, setTunPreferences] = useState<TunPreferences | null>(null)
  const [tunDraft, setTunDraft] = useState<TunPreferences | null>(null)
  const [tunDialogOpen, setTunDialogOpen] = useState(false)
  const [chainDialogOpen, setChainDialogOpen] = useState(false)
  const [chainTarget, setChainTarget] = useState('')
  const [chainNodes, setChainNodes] = useState<string[]>([])
  const [chainCandidate, setChainCandidate] = useState('')
  const [runtimeConfigOpen, setRuntimeConfigOpen] = useState(false)
  const [runtimeConfigText, setRuntimeConfigText] = useState('')
  const [runtimeConfigLoading, setRuntimeConfigLoading] = useState(false)
  const [backupDialogOpen, setBackupDialogOpen] = useState(false)
  const [backups, setBackups] = useState<LocalBackupInfo[]>([])
  const [backupLoading, setBackupLoading] = useState(false)
  const [, setBackupSettings] = useState<BackupSettings | null>(null)
  const [backupSettingsDraft, setBackupSettingsDraft] =
    useState<BackupSettings | null>(null)
  const [webdavBackups, setWebdavBackups] = useState<WebDavBackupInfo[]>([])
  const [webdavLoading, setWebdavLoading] = useState(false)
  const [dnsDialogOpen, setDnsDialogOpen] = useState(false)
  const [, setDnsOverride] = useState<DnsOverrideSettings | null>(null)
  const [dnsDraft, setDnsDraft] = useState<DnsOverrideSettings | null>(null)
  const [dnsLoading, setDnsLoading] = useState(false)
  const [portSettings, setPortSettings] = useState<PortSettings | null>(null)
  const [portDraft, setPortDraft] = useState<PortSettings | null>(null)
  const [portDialogOpen, setPortDialogOpen] = useState(false)
  const [portLoading, setPortLoading] = useState(false)
  const [, setExternalController] =
    useState<ExternalControllerSettings | null>(null)
  const [externalControllerDraft, setExternalControllerDraft] =
    useState<ExternalControllerSettings | null>(null)
  const [externalControllerDialogOpen, setExternalControllerDialogOpen] =
    useState(false)
  const [externalControllerLoading, setExternalControllerLoading] =
    useState(false)
  const [networkInterfaces, setNetworkInterfaces] = useState<NetworkInterfaceInfo[]>([])
  const [networkDialogOpen, setNetworkDialogOpen] = useState(false)
  const [networkLoading, setNetworkLoading] = useState(false)
  const [networkIsV4, setNetworkIsV4] = useState(true)
  const [diagnosticDialogOpen, setDiagnosticDialogOpen] = useState(false)
  const [diagnosticLoading, setDiagnosticLoading] = useState(false)
  const [diagnosticText, setDiagnosticText] = useState('')
  const [mergeTarget, setMergeTarget] = useState<MergeTarget | null>(null)
  const [mergeDraft, setMergeDraft] = useState('')
  const [sequenceTarget, setSequenceTarget] = useState<SequenceTarget | null>(null)
  const [sequenceDraft, setSequenceDraft] = useState('')
  const [scriptTarget, setScriptTarget] = useState<ScriptTarget | null>(null)
  const [scriptDraft, setScriptDraft] = useState('')
  const [bootModule, setBootModule] = useState<BootModuleStatus | null>(null)
  const [core, setCore] = useState<CoreSnapshot | null>(null)
  const [connections, setConnections] = useState<ConnectionsSnapshot | null>(null)
  const [closedConnections, setClosedConnections] = useState<CoreConnection[]>([])
  const [connectionView, setConnectionView] = useState<'active' | 'closed'>('active')
  const [traffic, setTraffic] = useState({ upload: 0, download: 0 })
  const [coreLogs, setCoreLogs] = useState('')
  const [delays, setDelays] = useState<Record<string, number | null>>({})
  const [profileMenu, setProfileMenu] = useState<{
    anchor: HTMLElement
    profile: Profile
  } | null>(null)
  const [profileBatchMode, setProfileBatchMode] = useState(false)
  const [selectedProfiles, setSelectedProfiles] = useState<Set<string>>(
    () => new Set(),
  )
  const totals = useRef<{ upload: number; download: number; at: number } | null>(null)
  const previousConnections = useRef<Map<string, CoreConnection>>(new Map())
  const connectionDetailRef = useRef<ConnectionDetailRef>(null)
  const homeDelayButtonRef = useRef<HTMLButtonElement>(null)
  const proxyMatcher = useRef<(content: string) => boolean>(() => true)
  const ruleMatcher = useRef<(content: string) => boolean>(() => true)
  const connectionMatcher = useRef<(content: string) => boolean>(() => true)
  const logMatcher = useRef<(content: string) => boolean>(() => true)
  const active = doc.profiles.find((p) => p.id === doc.activeId)
  const canEdit = caps.profileManagement && !busy && !loading
  const homeMode = (core?.mode ?? summary?.mode ?? 'rule').toLowerCase()
  const homeIsGlobalMode = homeMode === 'global'
  const homeIsDirectMode = homeMode === 'direct'
  const homeSelectableGroups = useMemo(
    () =>
      (core?.groups ?? []).filter(
        (group) => group.selectable && group.name.toUpperCase() !== 'GLOBAL',
      ),
    [core],
  )
  const homeGlobalGroup = useMemo(
    () =>
      (core?.groups ?? []).find((group) => group.name.toUpperCase() === 'GLOBAL') ??
      null,
    [core],
  )
  const homeSelectedGroup = useMemo(() => {
    if (homeIsDirectMode) return null
    if (homeIsGlobalMode) return homeGlobalGroup
    return (
      homeSelectableGroups.find((group) => group.name === homeProxyGroupName) ??
      null
    )
  }, [
    homeGlobalGroup,
    homeIsDirectMode,
    homeIsGlobalMode,
    homeProxyGroupName,
    homeSelectableGroups,
  ])
  const homeSelectedProxyName = homeIsDirectMode
    ? 'DIRECT'
    : (homeSelectedGroup?.selected ?? '')
  const homeSelectedProxyType =
    summary?.nodes.find((node) => node.name === homeSelectedProxyName)?.protocol ??
    (homeSelectedProxyName ? 'Group' : '')
  const homeSelectedDelay = homeSelectedProxyName
    ? delays[homeSelectedProxyName]
    : undefined
  const homeProxyMembers = useMemo(() => {
    const members = homeSelectedGroup?.members ?? []
    if (homeProxySort === 2) return [...members].sort((a, b) => a.localeCompare(b))
    if (homeProxySort === 1) {
      return [...members].sort((a, b) => {
        const aDelay = delays[a]
        const bDelay = delays[b]
        const aValue = typeof aDelay === 'number' && aDelay > 0 ? aDelay : Number.MAX_SAFE_INTEGER
        const bValue = typeof bDelay === 'number' && bDelay > 0 ? bDelay : Number.MAX_SAFE_INTEGER
        return aValue - bValue || a.localeCompare(b)
      })
    }
    return members
  }, [delays, homeProxySort, homeSelectedGroup])
  const globalDelayNames = useMemo(() => {
    const names = new Set<string>()
    for (const node of summary?.nodes ?? []) names.add(node.name)
    for (const group of core?.groups ?? []) {
      for (const member of group.members) names.add(member)
    }
    return [...names]
  }, [core?.groups, summary?.nodes])

  const record = useCallback((text: string, error = false) => {
    if (error) showNotice.error(text)
    else showNotice.success(text)
  }, [])
  const updateThemeMode = (mode: MobileThemeMode) => {
    setThemeMode(mode)
    window.localStorage.setItem('cv4a-theme-mode', mode)
  }
  useEffect(() => {
    if (!hasBackend()) {
      record(zhShared.feedback.errors.unexpected, true)
      setLoading(false)
      return
    }
    Promise.all([
      api.capabilities(),
      api.profiles(),
      api.runtimeStatus(),
      api.preferences(),
      api.systemProxyStatus(),
    ])
      .then(
        ([capabilities, profiles, runtimeStatus, preferences, systemProxyStatus]) => {
        setCaps(capabilities)
        setDoc(profiles)
        setRuntime(runtimeStatus)
        setSystemProxy(systemProxyStatus)
        setNetworkPanel(
          systemProxyStatus.active
            ? 'system'
            : runtimeStatus.transparentActive
              ? 'tun'
              : 'system',
        )
        setMobilePreferences(preferences)
        setPage(preferences.startPage as Page)
        if (runtimeStatus.coreConnected)
          api.coreSnapshot().then(setCore).catch(() => setCore(null))
        },
      )
      .catch((e) => record(String(e), true))
      .finally(() => setLoading(false))
  }, [record])
  useEffect(() => {
    setHomeProxySelect(null)
    if (!core) {
      setHomeProxyGroupName('')
      return
    }
    if (homeIsDirectMode) {
      setHomeProxyGroupName('DIRECT')
      return
    }
    if (homeIsGlobalMode) {
      setHomeProxyGroupName(homeGlobalGroup?.name ?? 'GLOBAL')
      return
    }

    const storageKey = `cv4a-home-proxy-group:${active?.id ?? 'default'}`
    const saved = window.localStorage.getItem(storageKey)
    const currentStillExists = homeSelectableGroups.some(
      (group) => group.name === homeProxyGroupName,
    )
    if (currentStillExists) return
    const primaryKeywords = ['auto', 'select', 'proxy']
    const primary =
      homeSelectableGroups.find((group) =>
        primaryKeywords.some((keyword) =>
          group.name.toLowerCase().includes(keyword.toLowerCase()),
        ),
      ) ?? homeSelectableGroups[0]
    const next =
      homeSelectableGroups.find((group) => group.name === saved)?.name ??
      primary?.name ??
      ''
    setHomeProxyGroupName(next)
    if (next) window.localStorage.setItem(storageKey, next)
  }, [
    active?.id,
    core,
    homeGlobalGroup?.name,
    homeIsDirectMode,
    homeIsGlobalMode,
    homeProxyGroupName,
    homeSelectableGroups,
  ])
  useEffect(() => {
    let alive = true
    setSummary(null)
    if (active)
      api
        .inspect(active.yaml)
        .then((value) => {
          if (alive) setSummary(value)
        })
        .catch((e) => {
          if (alive) record(String(e), true)
        })
    return () => {
      alive = false
    }
  }, [active, record])
  const syncRuntime = useCallback(async () => {
    const [status, vpnStatus] = await Promise.all([
      api.runtimeStatus(),
      api.systemProxyStatus().catch(() => null),
    ])
    setRuntime(status)
    if (vpnStatus) setSystemProxy(vpnStatus)
    if (status.coreConnected) setCore(await api.coreSnapshot())
    else setCore(null)
    return status
  }, [])
  const syncConnections = useCallback(async () => {
    const snapshot = await api.connections()
    const now = Date.now()
    const previous = totals.current
    if (previous && now > previous.at) {
      const seconds = (now - previous.at) / 1000
      setTraffic({
        upload: Math.max(0, (snapshot.uploadTotal - previous.upload) / seconds),
        download: Math.max(0, (snapshot.downloadTotal - previous.download) / seconds),
      })
    }
    totals.current = {
      upload: snapshot.uploadTotal,
      download: snapshot.downloadTotal,
      at: now,
    }
    const current = new Map(
      snapshot.connections.map((connection) => [connection.id, connection] as const),
    )
    const ended = [...previousConnections.current.values()].filter(
      (connection) => !current.has(connection.id),
    )
    if (ended.length) {
      setClosedConnections((old) => {
        const seen = new Set<string>()
        return [...ended.reverse(), ...old]
          .filter((connection) => {
            if (seen.has(connection.id)) return false
            seen.add(connection.id)
            return true
          })
          .slice(0, 500)
      })
    }
    previousConnections.current = current
    setConnections(snapshot)
    return snapshot
  }, [])
  useEffect(() => {
    if (!hasBackend() || loading) return
    if (!['home', 'proxies', 'rules', 'settings'].includes(page)) return
    void syncRuntime().catch(() => {})
  }, [loading, page, syncRuntime])
  useEffect(() => {
    if (!hasBackend() || loading || page !== 'settings') return
    void Promise.allSettled([
      api.probeRoot(),
      api.bootModuleStatus(),
      api.runtimePreferences(),
      api.preferences(),
      api.tunPreferences(),
      api.dnsOverride(),
      api.portSettings(),
      api.externalControllerSettings(),
    ]).then(
      ([
        rootResult,
        bootResult,
        preferencesResult,
        mobilePreferencesResult,
        tunResult,
        dnsResult,
        portResult,
        externalControllerResult,
      ]) => {
        if (rootResult.status === 'fulfilled') setRoot(rootResult.value)
        if (bootResult.status === 'fulfilled') setBootModule(bootResult.value)
        else
          setBootModule({
            installed: false,
            detail: String(bootResult.reason),
          })
        if (preferencesResult.status === 'fulfilled')
          setRuntimePreferences(preferencesResult.value)
        if (mobilePreferencesResult.status === 'fulfilled')
          setMobilePreferences(mobilePreferencesResult.value)
        if (tunResult.status === 'fulfilled') setTunPreferences(tunResult.value)
        if (dnsResult.status === 'fulfilled') setDnsOverride(dnsResult.value)
        if (portResult.status === 'fulfilled') setPortSettings(portResult.value)
        if (externalControllerResult.status === 'fulfilled')
          setExternalController(externalControllerResult.value)
      },
    )
  }, [loading, page])
  useEffect(() => {
    if (!runtime?.coreConnected || !['home', 'connections'].includes(page)) {
      if (!runtime?.coreConnected) {
        const remaining = [...previousConnections.current.values()]
        if (remaining.length) {
          setClosedConnections((old) => {
            const seen = new Set<string>()
            return [...remaining.reverse(), ...old]
              .filter((connection) => {
                if (seen.has(connection.id)) return false
                seen.add(connection.id)
                return true
              })
              .slice(0, 500)
          })
        }
        previousConnections.current.clear()
        totals.current = null
        setConnections(null)
        setTraffic({ upload: 0, download: 0 })
      }
      return
    }
    let alive = true
    const run = () => {
      void syncConnections().catch(() => {
        if (alive) setConnections(null)
      })
    }
    run()
    const timer = window.setInterval(run, 1000)
    return () => {
      alive = false
      window.clearInterval(timer)
    }
  }, [page, runtime?.coreConnected, syncConnections])
  useEffect(() => {
    if (page !== 'logs' || !runtime?.agentConnected || !logEnabled) return
    let alive = true
    const run = () =>
      api
        .logs(200)
        .then((value) => alive && setCoreLogs(value))
        .catch((e) => alive && setCoreLogs(String(e)))
    run()
    const timer = window.setInterval(run, 2000)
    return () => {
      alive = false
      window.clearInterval(timer)
    }
  }, [logEnabled, page, runtime?.agentConnected])
  useEffect(() => {
    if (
      !mobilePreferences?.enableAutoDelayDetection ||
      !runtime?.coreConnected ||
      !core ||
      !summary
    )
      return

    let disposed = false
    const run = async () => {
      const concrete = new Set(summary.nodes.map((node) => node.name))
      const selected = [
        ...new Set(
          core.groups
            .filter((group) => group.selectable && group.selected)
            .map((group) => group.selected!)
            .filter((name) => concrete.has(name)),
        ),
      ]
      if (!selected.length) return
      const results = await Promise.all(
        selected.map(async (name) => {
          try {
            return [name, await api.proxyDelay(name)] as const
          } catch {
            return [name, null] as const
          }
        }),
      )
      if (!disposed) setDelays((old) => ({ ...old, ...Object.fromEntries(results) }))
    }

    const initial = window.setTimeout(() => void run(), 1500)
    const interval = window.setInterval(
      () => void run(),
      Math.max(1, mobilePreferences.autoDelayDetectionIntervalMinutes) * 60_000,
    )
    return () => {
      disposed = true
      window.clearTimeout(initial)
      window.clearInterval(interval)
    }
  }, [core, mobilePreferences, runtime?.coreConnected, summary])
  const openForm = (next: Form) => {
    setName(
      'profile' in next
        ? next.kind === 'copy'
          ? next.profile.name
          : next.profile.name
        : '',
    )
    setContent(
      next.kind === 'copy' || next.kind === 'edit' ? next.profile.yaml : '',
    )
    setSourceUrl('profile' in next ? next.profile.source ?? '' : '')
    setProfileOption(
      'profile' in next && next.kind === 'edit'
        ? { ...defaultProfileOptions, ...next.profile.option }
        : { ...defaultProfileOptions },
    )
    setFormError('')
    setForm(next)
  }
  const activateProfile = async (profile: Profile) => {
    if (!canEdit || profile.id === doc.activeId) return
    setBusy(true)
    try {
      setDoc(await api.activate(profile.id))
      await syncRuntime()
      showNotice.success('profiles.page.feedback.notifications.profileSwitched', 1000)
    } catch (e) {
      record(String(e), true)
    } finally {
      setBusy(false)
    }
  }
  const refreshProfile = async (profile: Profile) => {
    if (!canEdit || !profile.source) return
    setBusy(true)
    try {
      setDoc(await api.refresh(profile.id))
      await syncRuntime()
    } catch (e) {
      record(String(e), true)
    } finally {
      setBusy(false)
    }
  }
  const refreshAllProfiles = async () => {
    if (!canEdit) return
    const targets = doc.profiles.filter((profile) => profile.source)
    if (targets.length === 0) return
    setBusy(true)
    const failures: string[] = []
    try {
      for (const profile of targets) {
        try {
          setDoc(await api.refresh(profile.id))
        } catch (error) {
          failures.push(`${profile.name}: ${String(error)}`)
        }
      }
      await syncRuntime().catch(() => {})
      if (failures.length) {
        record(failures.join('\n'), true)
      }
    } finally {
      setBusy(false)
    }
  }
  const openMergeEditor = (target: MergeTarget) => {
    setMergeTarget(target)
    setMergeDraft(
      target.kind === 'global'
        ? (doc.globalMergeYaml ?? '')
        : (target.profile.mergeYaml ?? ''),
    )
  }
  const saveMerge = async () => {
    if (busy || !mergeTarget) return
    setBusy(true)
    try {
      const yaml = mergeDraft.trim() ? mergeDraft : null
      const next =
        mergeTarget.kind === 'global'
          ? await api.setGlobalMerge(yaml)
          : await api.setProfileMerge(mergeTarget.profile.id, yaml)
      setDoc(next)
      setMergeTarget(null)
      await syncRuntime().catch(() => {})
      showNotice.success('shared.feedback.notifications.saved')
    } catch (e) {
      record(String(e), true)
      await syncRuntime().catch(() => {})
    } finally {
      setBusy(false)
    }
  }
  const openSequenceEditor = (target: SequenceTarget) => {
    setSequenceTarget(target)
    const yaml =
      target.kind === 'rules'
        ? target.profile.rulesYaml
        : target.kind === 'proxies'
          ? target.profile.proxiesYaml
          : target.profile.groupsYaml
    setSequenceDraft(yaml ?? 'prepend: []\nappend: []\ndelete: []\n')
  }
  const saveSequenceEnhancement = async () => {
    if (busy || !sequenceTarget) return
    setBusy(true)
    try {
      const yaml = sequenceDraft.trim() ? sequenceDraft : null
      setDoc(
        await api.setProfileSequence(
          sequenceTarget.profile.id,
          sequenceTarget.kind,
          yaml,
        ),
      )
      setSequenceTarget(null)
      await syncRuntime().catch(() => {})
      showNotice.success('shared.feedback.notifications.saved')
    } catch (e) {
      record(String(e), true)
      await syncRuntime().catch(() => {})
    } finally {
      setBusy(false)
    }
  }
  const openScriptEditor = (target: ScriptTarget) => {
    setScriptTarget(target)
    setScriptDraft(
      target.kind === 'global'
        ? (doc.globalScriptJs ?? defaultProfileScript)
        : (target.profile.scriptJs ?? defaultProfileScript),
    )
  }
  const saveScript = async () => {
    if (busy || !scriptTarget) return
    setBusy(true)
    try {
      const script = scriptDraft.trim() ? scriptDraft : null
      const next =
        scriptTarget.kind === 'global'
          ? await api.setGlobalScript(script)
          : await api.setProfileScript(scriptTarget.profile.id, script)
      setDoc(next)
      setScriptTarget(null)
      await syncRuntime().catch(() => {})
      showNotice.success('shared.feedback.notifications.saved')
    } catch (e) {
      record(String(e), true)
      await syncRuntime().catch(() => {})
    } finally {
      setBusy(false)
    }
  }
  const toggleProfileBatchMode = () => {
    setProfileBatchMode((value) => !value)
    setSelectedProfiles(new Set())
  }
  const toggleProfileSelection = (id: string) => {
    setSelectedProfiles((old) => {
      const next = new Set(old)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }
  const selectAllProfiles = () => {
    setSelectedProfiles(new Set(doc.profiles.map((profile) => profile.id)))
  }
  const deleteSelectedProfiles = async () => {
    if (!canEdit || selectedProfiles.size === 0) return
    setBusy(true)
    try {
      setDoc(await api.removeMany([...selectedProfiles]))
      setSelectedProfiles(new Set())
      setProfileBatchMode(false)
      await syncRuntime().catch(() => {})
      showNotice.success('profiles.page.feedback.notifications.batchDeleted')
    } catch (e) {
      record(String(e), true)
      await syncRuntime().catch(() => {})
    } finally {
      setBusy(false)
    }
  }
  const submit = async (event: FormEvent) => {
    event.preventDefault()
    if (!canEdit || !form) return
    setBusy(true)
    setFormError('')
    try {
      const result =
        form.kind === 'delete'
          ? await api.remove(form.profile.id)
          : form.kind === 'edit'
            ? await api.updateProfile(
                form.profile.id,
                name.trim(),
                content,
                form.profile.yaml,
                form.profile.source ? sourceUrl.trim() : null,
                form.profile.source ? profileOption : undefined,
              )
          : form.kind === 'subscription'
            ? await api.subscribe(
                name.trim(),
                sourceUrl.trim(),
                profileOption,
              )
            : form.kind === 'nodes'
              ? await api.importNodes(name.trim(), content)
              : await api.import(name.trim(), content)
      setDoc(result)
      setForm(null)
      if (form.kind === 'edit') {
        await syncRuntime()
      }
      if (form.kind === 'subscription') {
        showNotice.success('shared.feedback.notifications.importSubscriptionSuccess')
      } else if (form.kind === 'yaml' || form.kind === 'nodes') {
        showNotice.success('shared.feedback.notifications.importSuccess')
      } else if (form.kind === 'edit' || form.kind === 'copy') {
        showNotice.success('shared.feedback.notifications.common.saveSuccess')
      }
    } catch (e) {
      setFormError(String(e))
      showNotice.error(e)
    } finally {
      setBusy(false)
    }
  }
  const bootstrapRuntime = async () => {
    if (busy || loading) return
    setBusy(true)
    try {
      const status = await api.bootstrapRuntime()
      setRuntime(status)
      setRoot({ granted: true, detail: '' })
      showNotice.success('settings.feedback.notifications.clashService.installSuccess')
    } catch (e) {
      record(String(e), true)
      await syncRuntime().catch(() => {})
    } finally {
      setBusy(false)
    }
  }
  const setBootAutostart = async (enable: boolean) => {
    if (busy) return
    setBusy(true)
    try {
      const status = await api.setBootModule(enable)
      setBootModule(status)
    } catch (e) {
      record(String(e), true)
      void api.bootModuleStatus().then(setBootModule).catch(() => {})
    } finally {
      setBusy(false)
    }
  }
  const updateRuntimePreferences = async (
    patch: Partial<RuntimePreferences>,
  ) => {
    if (busy || !runtimePreferences) return
    setBusy(true)
    try {
      const next = { ...runtimePreferences, ...patch }
      const applied = await api.setRuntimePreferences(next)
      setRuntimePreferences(applied)
    } catch (e) {
      record(String(e), true)
      void api.runtimePreferences().then(setRuntimePreferences).catch(() => {})
    } finally {
      setBusy(false)
    }
  }
  const updateMobilePreferences = async (
    patch: Partial<MobilePreferences>,
  ) => {
    if (busy || !mobilePreferences) return
    setBusy(true)
    try {
      const applied = await api.setPreferences({ ...mobilePreferences, ...patch })
      setMobilePreferences(applied)
    } catch (e) {
      record(String(e), true)
      void api.preferences().then(setMobilePreferences).catch(() => {})
    } finally {
      setBusy(false)
    }
  }
  const runMihomoMaintenance = async (
    operation: () => Promise<void>,
    successKey?: string,
  ) => {
    if (busy || !runtime?.coreConnected) return
    setBusy(true)
    try {
      await operation()
      if (successKey) showNotice.success(successKey)
    } catch (e) {
      record(String(e), true)
    } finally {
      setBusy(false)
    }
  }
  const openTunSettings = () => {
    if (!tunPreferences) return
    setTunDraft({ ...tunPreferences, dnsHijack: [...tunPreferences.dnsHijack] })
    setTunDialogOpen(true)
  }
  const openProxyChain = () => {
    const stored = active?.proxyChain
    const groups = core?.groups.filter((group) => group.selectable) ?? []
    const fallbackTarget =
      core?.mode?.toLowerCase() === 'global' &&
      groups.some((group) => group.name === 'GLOBAL')
        ? 'GLOBAL'
        : groups.find((group) => group.name !== 'GLOBAL')?.name ??
          groups[0]?.name ??
          ''
    setChainTarget(stored?.targetGroup ?? fallbackTarget)
    setChainNodes(stored?.nodes ? [...stored.nodes] : [])
    setChainCandidate('')
    setChainDialogOpen(true)
  }
  const saveProxyChain = async () => {
    if (busy || !chainTarget || chainNodes.length < 2) return
    setBusy(true)
    try {
      setCore(await api.setProxyChain(chainTarget, chainNodes))
      setDoc(await api.profiles())
      await syncRuntime().catch(() => {})
      setChainDialogOpen(false)
    } catch (e) {
      record(String(e), true)
    } finally {
      setBusy(false)
    }
  }
  const clearProxyChain = async () => {
    if (busy || !active?.proxyChain) return
    setBusy(true)
    try {
      setCore(await api.clearProxyChain())
      setDoc(await api.profiles())
      await syncRuntime().catch(() => {})
      setChainNodes([])
      setChainDialogOpen(false)
    } catch (e) {
      record(String(e), true)
    } finally {
      setBusy(false)
    }
  }
  const moveChainNode = (index: number, direction: -1 | 1) => {
    const target = index + direction
    if (target < 0 || target >= chainNodes.length) return
    setChainNodes((old) => {
      const next = [...old]
      ;[next[index], next[target]] = [next[target]!, next[index]!]
      return next
    })
  }
  const saveTunSettings = async () => {
    if (busy || !tunDraft) return
    setBusy(true)
    try {
      const applied = await api.setTunPreferences(tunDraft)
      setTunPreferences(applied)
      setTunDraft(applied)
      setTunDialogOpen(false)
      await syncRuntime().catch(() => {})
      showNotice.success('settings.modals.tun.messages.applied')
    } catch (e) {
      record(String(e), true)
      void api.tunPreferences().then(setTunPreferences).catch(() => {})
    } finally {
      setBusy(false)
    }
  }
  const openRuntimeConfig = async () => {
    if (runtimeConfigLoading) return
    setRuntimeConfigText('')
    setRuntimeConfigOpen(true)
    setRuntimeConfigLoading(true)
    try {
      setRuntimeConfigText(await api.runtimeConfig())
    } catch (e) {
      setRuntimeConfigText(`# ${String(e)}\n`)
    } finally {
      setRuntimeConfigLoading(false)
    }
  }
  const restartCore = async () => {
    if (busy || !runtime?.coreRunning) return
    setBusy(true)
    try {
      setCore(await api.restartCore())
      await syncRuntime()
      showNotice.success('settings.feedback.notifications.clash.restartSuccess')
    } catch (e) {
      record(String(e), true)
      await syncRuntime().catch(() => {})
    } finally {
      setBusy(false)
    }
  }
  const upgradeCore = async () => {
    if (busy || !runtime?.agentConnected) return
    setBusy(true)
    try {
      const report = await api.upgradeCore(false)
      await syncRuntime().catch(() => {})
      showNotice.success(
        report.upgraded
          ? 'settings.feedback.notifications.clash.versionUpdated'
          : 'settings.feedback.notifications.clash.alreadyLatestVersion',
      )
    } catch (e) {
      record(String(e), true)
      await syncRuntime().catch(() => {})
    } finally {
      setBusy(false)
    }
  }
  const loadLocalBackups = async () => {
    setBackupLoading(true)
    try {
      setBackups(await api.listLocalBackups())
    } catch (e) {
      record(String(e), true)
    } finally {
      setBackupLoading(false)
    }
  }
  const loadBackupSettings = async () => {
    const settings = await api.backupSettings()
    setBackupSettings(settings)
    setBackupSettingsDraft({ ...settings })
    return settings
  }
  const loadWebDavBackups = async () => {
    setWebdavLoading(true)
    try {
      setWebdavBackups(await api.listWebDavBackups())
    } catch (e) {
      setWebdavBackups([])
      throw e
    } finally {
      setWebdavLoading(false)
    }
  }
  const openLocalBackups = () => {
    setBackupDialogOpen(true)
    void loadLocalBackups()
    void loadBackupSettings()
      .then((settings) => {
        if (settings.webdavUrl) return loadWebDavBackups()
      })
      .catch((e) => record(String(e), true))
  }
  const createLocalBackup = async () => {
    if (busy || backupLoading) return
    setBusy(true)
    try {
      const backup = await api.createLocalBackup()
      void backup
      await loadLocalBackups()
      showNotice.success('settings.modals.backup.messages.localBackupCreated')
    } catch (e) {
      record(String(e), true)
    } finally {
      setBusy(false)
    }
  }
  const restoreLocalBackup = async (filename: string) => {
    if (busy || backupLoading) return
    setBusy(true)
    try {
      setDoc(await api.restoreLocalBackup(filename))
      await syncRuntime().catch(() => {})
      await loadLocalBackups()
    } catch (e) {
      record(String(e), true)
      await syncRuntime().catch(() => {})
    } finally {
      setBusy(false)
    }
  }
  const deleteLocalBackup = async (filename: string) => {
    if (busy || backupLoading) return
    setBusy(true)
    try {
      await api.deleteLocalBackup(filename)
      await loadLocalBackups()
    } catch (e) {
      record(String(e), true)
    } finally {
      setBusy(false)
    }
  }
  const importLocalBackup = async () => {
    if (busy || backupLoading) return
    let selected: string | null = null
    try {
      const result = await openDialog({
        multiple: false,
        directory: false,
        pickerMode: 'document',
        filters: [{ name: zhSettings.modals.backup.actions.backup, extensions: ['json'] }],
      })
      if (!result || Array.isArray(result)) return
      selected = result
      setBusy(true)
      const text = await readTextFile(result)
      await api.importBackupText(text)
      await loadLocalBackups()
    } catch (e) {
      record(String(e), true)
    } finally {
      if (selected) void remove(selected).catch(() => {})
      setBusy(false)
    }
  }
  const exportLocalBackup = async (filename: string) => {
    if (busy || backupLoading) return
    setBusy(true)
    try {
      const text = await api.exportLocalBackup(filename)
      const destination = await saveDialog({
        defaultPath: filename,
        filters: [{ name: zhSettings.modals.backup.actions.backup, extensions: ['json'] }],
      })
      if (!destination) return
      await writeTextFile(destination, text)
    } catch (e) {
      record(String(e), true)
    } finally {
      setBusy(false)
    }
  }
  const saveBackupSettings = async () => {
    if (busy || !backupSettingsDraft) return
    setBusy(true)
    try {
      const applied = await api.setBackupSettings(backupSettingsDraft)
      setBackupSettings(applied)
      setBackupSettingsDraft({ ...applied })
      if (applied.webdavUrl) {
        await loadWebDavBackups().catch((e) => record(String(e), true))
      } else {
        setWebdavBackups([])
      }
      showNotice.success('settings.modals.backup.messages.webdavConfigSaved')
    } catch (e) {
      record(String(e), true)
      void loadBackupSettings().catch(() => {})
    } finally {
      setBusy(false)
    }
  }
  const testWebDav = async () => {
    if (busy || !backupSettingsDraft) return
    setBusy(true)
    try {
      const applied = await api.setBackupSettings(backupSettingsDraft)
      setBackupSettings(applied)
      setBackupSettingsDraft({ ...applied })
      await api.testWebDav()
      await loadWebDavBackups()
    } catch (e) {
      record(String(e), true)
    } finally {
      setBusy(false)
    }
  }
  const createWebDavBackup = async () => {
    if (busy || webdavLoading) return
    setBusy(true)
    try {
      await api.createWebDavBackup()
      await loadWebDavBackups()
      showNotice.success('settings.modals.backup.messages.backupCreated')
    } catch (e) {
      record(String(e), true)
    } finally {
      setBusy(false)
    }
  }
  const restoreWebDavBackup = async (filename: string) => {
    if (busy || webdavLoading) return
    setBusy(true)
    try {
      setDoc(await api.restoreWebDavBackup(filename))
      await syncRuntime().catch(() => {})
    } catch (e) {
      record(String(e), true)
      await syncRuntime().catch(() => {})
    } finally {
      setBusy(false)
    }
  }
  const deleteWebDavBackup = async (filename: string) => {
    if (busy || webdavLoading) return
    setBusy(true)
    try {
      await api.deleteWebDavBackup(filename)
      await loadWebDavBackups()
    } catch (e) {
      record(String(e), true)
    } finally {
      setBusy(false)
    }
  }
  const openDnsSettings = async () => {
    if (dnsLoading) return
    setDnsDialogOpen(true)
    setDnsLoading(true)
    try {
      const settings = await api.dnsOverride()
      setDnsOverride(settings)
      setDnsDraft({ ...settings })
    } catch (e) {
      record(String(e), true)
      setDnsDialogOpen(false)
    } finally {
      setDnsLoading(false)
    }
  }
  const saveDnsSettings = async () => {
    if (busy || dnsLoading || !dnsDraft) return
    setBusy(true)
    try {
      const applied = await api.setDnsOverride(dnsDraft)
      setDnsOverride(applied)
      setDnsDraft({ ...applied })
      setDnsDialogOpen(false)
      await syncRuntime().catch(() => {})
      showNotice.success('settings.modals.dns.messages.saved')
    } catch (e) {
      record(String(e), true)
      void api.dnsOverride().then((settings) => {
        setDnsOverride(settings)
        setDnsDraft({ ...settings })
      })
    } finally {
      setBusy(false)
    }
  }
  const openPortSettings = async () => {
    if (portLoading) return
    setPortDialogOpen(true)
    setPortLoading(true)
    try {
      const settings = await api.portSettings()
      setPortSettings(settings)
      setPortDraft({ ...settings })
    } catch (e) {
      record(String(e), true)
      setPortDialogOpen(false)
    } finally {
      setPortLoading(false)
    }
  }
  const savePortSettings = async () => {
    if (busy || portLoading || !portDraft) return
    setBusy(true)
    try {
      const applied = await api.setPortSettings(portDraft)
      setPortSettings(applied)
      setPortDraft({ ...applied })
      setPortDialogOpen(false)
      await syncRuntime().catch(() => {})
      showNotice.success('settings.modals.clashPort.messages.saved')
    } catch (e) {
      record(String(e), true)
      void api.portSettings().then((settings) => {
        setPortSettings(settings)
        setPortDraft({ ...settings })
      })
      await syncRuntime().catch(() => {})
    } finally {
      setBusy(false)
    }
  }
  const setAllowLan = async (allowLan: boolean) => {
    if (busy || !portSettings) return
    setBusy(true)
    try {
      const bindAddress =
        allowLan && ['127.0.0.1', '::1'].includes(portSettings.bindAddress)
          ? '0.0.0.0'
          : portSettings.bindAddress
      const applied = await api.setPortSettings({
        ...portSettings,
        allowLan,
        bindAddress,
      })
      setPortSettings(applied)
      await syncRuntime().catch(() => {})
    } catch (e) {
      record(String(e), true)
      void api.portSettings().then(setPortSettings).catch(() => {})
      await syncRuntime().catch(() => {})
    } finally {
      setBusy(false)
    }
  }
  const openExternalControllerSettings = async () => {
    if (externalControllerLoading) return
    setExternalControllerDialogOpen(true)
    setExternalControllerLoading(true)
    try {
      const settings = await api.externalControllerSettings()
      setExternalController(settings)
      setExternalControllerDraft({
        ...settings,
        allowOrigins: [...settings.allowOrigins],
      })
    } catch (e) {
      record(String(e), true)
      setExternalControllerDialogOpen(false)
    } finally {
      setExternalControllerLoading(false)
    }
  }
  const saveExternalControllerSettings = async () => {
    if (busy || externalControllerLoading || !externalControllerDraft) return
    setBusy(true)
    try {
      const applied = await api.setExternalControllerSettings(
        externalControllerDraft,
      )
      setExternalController(applied)
      setExternalControllerDraft({
        ...applied,
        allowOrigins: [...applied.allowOrigins],
      })
      setExternalControllerDialogOpen(false)
      await syncRuntime().catch(() => {})
      showNotice.success('shared.feedback.notifications.common.saveSuccess')
    } catch (e) {
      record(String(e), true)
      void api
        .externalControllerSettings()
        .then((settings) => {
          setExternalController(settings)
          setExternalControllerDraft({
            ...settings,
            allowOrigins: [...settings.allowOrigins],
          })
        })
        .catch(() => {})
      await syncRuntime().catch(() => {})
    } finally {
      setBusy(false)
    }
  }
  const openNetworkInterfaces = async () => {
    if (networkLoading) return
    setNetworkDialogOpen(true)
    setNetworkLoading(true)
    try {
      setNetworkInterfaces(await api.networkInterfaces())
    } catch (e) {
      record(String(e), true)
      setNetworkDialogOpen(false)
    } finally {
      setNetworkLoading(false)
    }
  }
  const openDiagnostics = async () => {
    if (diagnosticLoading) return
    setDiagnosticDialogOpen(true)
    setDiagnosticLoading(true)
    try {
      const report = await api.diagnostics()
      setDiagnosticText(report)
    } catch (e) {
      record(String(e), true)
      setDiagnosticDialogOpen(false)
    } finally {
      setDiagnosticLoading(false)
    }
  }
  const setTransparentProxy = async (enable: boolean) => {
    if (busy || !caps.tunControl || !runtime?.agentConnected || !active) return
    setBusy(true)
    try {
      if (enable) {
        setCore(await api.enableTun())
        await syncRuntime()
      } else {
        const status = await api.disableTun()
        setRuntime(status)
        if (status.coreConnected) setCore(await api.coreSnapshot())
        else setCore(null)
      }
    } catch (e) {
      record(String(e), true)
      await syncRuntime().catch(() => {})
    } finally {
      setBusy(false)
    }
  }
  const setAndroidSystemProxy = async (enable: boolean) => {
    if (busy || !caps.systemProxy || !active) return
    setBusy(true)
    try {
      const status = await api.setSystemProxy(enable)
      setSystemProxy(status)
      await syncRuntime()
    } catch (e) {
      record(String(e), true)
      await syncRuntime().catch(() => {})
    } finally {
      setBusy(false)
    }
  }
  const closeConnection = async (id: string) => {
    if (busy || !runtime?.coreConnected) return
    setBusy(true)
    try {
      const closed = connections?.connections.find((connection) => connection.id === id)
      const snapshot = await api.closeConnection(id)
      if (closed) {
        setClosedConnections((old) => [closed, ...old.filter((item) => item.id !== id)].slice(0, 500))
      }
      previousConnections.current = new Map(
        snapshot.connections.map((connection) => [connection.id, connection] as const),
      )
      setConnections(snapshot)
    } catch (e) {
      record(String(e), true)
    } finally {
      setBusy(false)
    }
  }
  const closeAllConnections = async () => {
    if (busy || !runtime?.coreConnected) return
    setBusy(true)
    try {
      const activeConnections = connections?.connections ?? []
      const snapshot = await api.closeAllConnections()
      if (activeConnections.length) {
        setClosedConnections((old) => {
          const seen = new Set<string>()
          return [[...activeConnections].reverse(), old]
            .flat()
            .filter((connection) => {
              if (seen.has(connection.id)) return false
              seen.add(connection.id)
              return true
            })
            .slice(0, 500)
        })
      }
      previousConnections.current = new Map(
        snapshot.connections.map((connection) => [connection.id, connection] as const),
      )
      setConnections(snapshot)
    } catch (e) {
      record(String(e), true)
    } finally {
      setBusy(false)
    }
  }
  const clearClosedConnections = () => {
    setClosedConnections([])
    connectionDetailRef.current?.close()
  }
  const showConnectionDetail = useCallback(
    (id: string) => {
      const selected =
        connectionView === 'active'
          ? connections?.connections
          : closedConnections
      const connection = selected?.find((item) => item.id === id)
      if (connection) {
        connectionDetailRef.current?.open(
          connectionDetailValue(connection),
          connectionView === 'closed',
        )
      }
    },
    [closedConnections, connectionView, connections],
  )
  const clearLogs = async () => {
    if (busy || !runtime?.agentConnected) return
    setBusy(true)
    try {
      await api.clearLogs()
      setCoreLogs('')
    } catch (e) {
      record(String(e), true)
    } finally {
      setBusy(false)
    }
  }
  const testDelays = async (names: string[]) => {
    if (busy || !runtime?.coreConnected) return
    const uniqueNames = [...new Set(names)]
    if (uniqueNames.length === 0) return
    setBusy(true)
    setDelays((old) => ({
      ...old,
      ...Object.fromEntries(uniqueNames.map((name) => [name, -2])),
    }))
    try {
      const results = await Promise.all(
        uniqueNames.map(async (name) => {
          try {
            return [name, await api.proxyDelay(name)] as const
          } catch {
            return [name, null] as const
          }
        }),
      )
      setDelays((old) => ({ ...old, ...Object.fromEntries(results) }))
    } finally {
      setBusy(false)
    }
  }
  const testAllDelays = async () => {
    await testDelays(globalDelayNames)
  }
  const testDelay = async (name: string) => {
    if (!runtime?.coreConnected) return
    setDelays((old) => ({ ...old, [name]: -2 }))
    try {
      const delay = await api.proxyDelay(name)
      setDelays((old) => ({ ...old, [name]: delay }))
    } catch {
      setDelays((old) => ({ ...old, [name]: null }))
    }
  }
  const coreAction = async (
    operation: () => Promise<CoreSnapshot>,
  ) => {
    if (!runtime?.coreConnected || busy) return
    setBusy(true)
    try {
      setCore(await operation())
    } catch (e) {
      record(String(e), true)
      await syncRuntime().catch(() => {})
    } finally {
      setBusy(false)
    }
  }
  const empty = (text: string) => (
    <MobileEmpty
      text={text}
      action={
        <Button
          variant="contained"
          size="small"
          disabled={!canEdit}
          onClick={() => {
            setPage('profiles')
            openForm({ kind: 'subscription' })
          }}
        >
          {zhProfiles.page.actions.import}
        </Button>
      }
    />
  )

  const ruleRows = (
    core
      ? core.rules.map((rule) => ({
          type: rule.kind,
          payload: rule.payload,
          proxy: rule.proxy,
        }))
      : (summary?.rules ?? []).map((rule) => {
          const [type = '', ...rest] = rule.split(',')
          const proxy = rest.length ? rest.at(-1) ?? '' : ''
          return {
            type,
            payload: rest.length > 1 ? rest.slice(0, -1).join(',') : rest[0] ?? '',
            proxy,
          }
        })
  )
    .map((rule, index) => ({ ...rule, index: index + 1 }))
    .filter((rule) =>
      ruleMatcher.current(`${rule.type},${rule.payload},${rule.proxy}`),
    )

  const connectionRows = useMemo(() => {
    const selectedConnections =
      connectionView === 'active'
        ? (connections?.connections ?? [])
        : closedConnections
    const rows: ConnectionRowView[] = selectedConnections.map(
      (connection) => ({
        id: connection.id,
        host:
          connection.host ||
          connection.destination ||
          zhHome.components.ipInfo.labels.unknown,
        process: connection.process,
        network: connection.network,
        type: connection.inbound,
        chains: formatConnectionChains(connection.chains),
        rule: connection.rulePayload
          ? `${connection.rule}(${connection.rulePayload})`
          : connection.rule,
        time: connection.start,
        source: connection.source,
        destination: connection.destination,
        uploadText: formatConnectionTraffic(connection.upload),
        downloadText: formatConnectionTraffic(connection.download),
        uploadSpeedText: '0 B/s',
        downloadSpeedText: '0 B/s',
        upload: connection.upload,
        download: connection.download,
        uploadSpeed: 0,
        downloadSpeed: 0,
        startTime: Date.parse(connection.start || '') || 0,
        searchableHost: connection.host,
        searchableDestinationIP: connection.destination,
        searchableProcess: connection.process,
      }),
    )
    const filtered = rows.filter((row) =>
      connectionMatcher.current(
        [
          row.host,
          row.process,
          row.network,
          row.type,
          row.chains,
          row.rule,
          row.source,
          row.destination,
        ].join(' '),
      ),
    )
    return filtered.sort((a, b) => {
      if (connectionSort === 'upload') return b.upload - a.upload
      if (connectionSort === 'download') return b.download - a.download
      return b.startTime - a.startTime
    })
  }, [closedConnections, connectionSearchState, connectionSort, connectionView, connections])

  const logItems = useMemo(() => {
    if (!coreLogs) return []
    const filtered = coreLogs
      .split('\n')
      .filter(Boolean)
      .map(parseLogLine)
      .filter((item) => {
        const normalized = item.type.toLowerCase()
        if (
          logFilter !== 'all' &&
          normalized !== logFilter &&
          !(logFilter === 'warn' && normalized === 'warning') &&
          !(logFilter === 'err' && normalized === 'error')
        ) {
          return false
        }
        return logMatcher.current(
          `${item.time ?? ''} ${item.type} ${item.payload}`,
        )
      })
    return logOrder === 'desc' ? filtered.reverse() : filtered
  }, [coreLogs, logFilter, logOrder, logSearchState])

  return (
    <VergeMobileTheme mode={themeMode}>
      <div className="mobile-verge-shell">
        <div className="mobile-verge-page">
          <header className="mobile-verge-header">
            <Stack direction="row" spacing={1} sx={{ alignItems: 'center', minWidth: 0 }}>
              <IconButton
                size="small"
                color="inherit"
                aria-label={zhLayout.components.navigation.menu.expandNavBar}
                title={zhLayout.components.navigation.menu.expandNavBar}
                onClick={() => setNavOpen(true)}
              >
                <MenuRounded />
              </IconButton>
              <Typography sx={{ fontSize: 20, fontWeight: 700 }} noWrap>
                {labels[page]}
              </Typography>
            </Stack>
            <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
              {page === 'profiles' && (
                profileBatchMode ? (
                  <>
                    <IconButton
                      size="small"
                      color="inherit"
                      title={
                        selectedProfiles.size === doc.profiles.length
                          ? zhProfiles.page.batch.actions.deselectAll
                          : zhProfiles.page.batch.actions.selectAll
                      }
                      onClick={() => {
                        if (
                          doc.profiles.length > 0 &&
                          selectedProfiles.size === doc.profiles.length
                        ) {
                          setSelectedProfiles(new Set())
                        } else {
                          selectAllProfiles()
                        }
                      }}
                    >
                      {selectedProfiles.size === 0 ? (
                        <CheckBoxOutlineBlankRounded />
                      ) : selectedProfiles.size === doc.profiles.length ? (
                        <CheckBoxRounded />
                      ) : (
                        <IndeterminateCheckBoxRounded />
                      )}
                    </IconButton>
                    <IconButton
                      size="small"
                      color="error"
                      title={zhProfiles.page.batch.actions.delete}
                      disabled={!canEdit || selectedProfiles.size === 0}
                      onClick={() => void deleteSelectedProfiles()}
                    >
                      <DeleteRounded />
                    </IconButton>
                    <Button
                      size="small"
                      variant="outlined"
                      disabled={busy}
                      onClick={toggleProfileBatchMode}
                    >
                      {zhProfiles.page.batch.actions.done} · {selectedProfiles.size}
                    </Button>
                  </>
                ) : (
                  <>
                    <IconButton
                      size="small"
                      color="inherit"
                      title={zhProfiles.page.batch.title}
                      disabled={!canEdit || doc.profiles.length === 0}
                      onClick={toggleProfileBatchMode}
                    >
                      <CheckBoxOutlineBlankRounded />
                    </IconButton>
                    <IconButton
                      size="small"
                      color="inherit"
                      title={zhProfiles.page.actions.viewRuntimeConfig}
                      aria-label={zhProfiles.page.actions.viewRuntimeConfig}
                      disabled={!runtime?.agentConnected}
                      onClick={() => void openRuntimeConfig()}
                    >
                      <TextSnippetOutlined />
                    </IconButton>
                    <IconButton
                      size="small"
                      color="inherit"
                      title={zhProfiles.page.actions.updateAll}
                      aria-label={zhProfiles.page.actions.updateAll}
                      disabled={!canEdit || !doc.profiles.some((profile) => profile.source)}
                      onClick={() => void refreshAllProfiles()}
                    >
                      <RefreshRounded />
                    </IconButton>
                    <IconButton
                      size="small"
                      color="inherit"
                      aria-label={zhProfiles.page.actions.import}
                      disabled={!canEdit}
                      onClick={() => openForm({ kind: 'subscription' })}
                    >
                      <AddRounded />
                    </IconButton>
                  </>
                )
              )}
              {page === 'rules' && (
                <MobileRuleProviderButton
                  enabled={runtime?.coreConnected ?? false}
                  onFeedback={record}
                />
              )}
              {page === 'connections' && (
                <Button
                  size="small"
                  variant="contained"
                  disabled={
                    busy ||
                    (connectionView === 'active'
                      ? !(connections?.connections.length ?? 0)
                      : closedConnections.length === 0)
                  }
                  onClick={
                    connectionView === 'active'
                      ? closeAllConnections
                      : clearClosedConnections
                  }
                >
                  {connectionView === 'active'
                    ? zhShared.actions.closeAll
                    : zhShared.actions.clear}
                </Button>
              )}
              {page === 'logs' && (
                <>
                  <IconButton
                    title={
                      logEnabled ? zhShared.actions.pause : zhShared.actions.resume
                    }
                    aria-label={
                      logEnabled ? zhShared.actions.pause : zhShared.actions.resume
                    }
                    size="small"
                    color="inherit"
                    onClick={() => setLogEnabled((value) => !value)}
                  >
                    {logEnabled ? (
                      <PauseCircleOutlineRounded />
                    ) : (
                      <PlayCircleOutlineRounded />
                    )}
                  </IconButton>
                  <IconButton
                    title={
                      logOrder === 'desc'
                        ? zhLogs.actions.showAscending
                        : zhLogs.actions.showDescending
                    }
                    aria-label={
                      logOrder === 'desc'
                        ? zhLogs.actions.showAscending
                        : zhLogs.actions.showDescending
                    }
                    size="small"
                    color="inherit"
                    onClick={() =>
                      setLogOrder((value) => (value === 'desc' ? 'asc' : 'desc'))
                    }
                  >
                    <SwapVertRounded
                      sx={{
                        transform: logOrder === 'desc' ? 'scaleY(-1)' : 'none',
                        transition: 'transform 0.2s ease',
                      }}
                    />
                  </IconButton>
                  <Button
                    size="small"
                    variant="contained"
                    disabled={busy || !runtime?.agentConnected}
                    onClick={clearLogs}
                  >
                    {zhShared.actions.clear}
                  </Button>
                </>
              )}
            </Stack>
          </header>

          <section className="mobile-verge-scroll">
            <Box className="mobile-verge-content">
              {loading && (
                <Alert severity="info" sx={{ mb: 1.5 }}>
                  {zhShared.statuses.loading}
                </Alert>
              )}
              {page === 'home' && (
                <Grid container spacing={1.5} columns={6}>
                  <Grid size={6}>
                    <EnhancedCard
                      title={active?.name ?? zhProfiles.page.title}
                      icon={<CloudUploadOutlined />}
                      iconColor="info"
                      action={
                        <Button
                          variant="outlined"
                          size="small"
                          endIcon={<StorageOutlined fontSize="small" />}
                          sx={{ borderRadius: 1.5 }}
                          onClick={() => setPage('profiles')}
                        >
                          {zhProfiles.page.title}
                        </Button>
                      }
                    >
                      {active ? (
                        <Stack spacing={1.25}>
                          <Stack direction="row" sx={{ justifyContent: 'space-between', gap: 2 }}>
                            <Typography variant="body2" color="text.secondary">
                              {zhProfiles.modals.profileForm.fields.type}
                            </Typography>
                            <Typography variant="body2">
                              {active.source
                                ? zhProfiles.modals.profileForm.types.remote
                                : zhProfiles.modals.profileForm.types.local}
                            </Typography>
                          </Stack>
                          <Divider />
                          <Stack direction="row" sx={{ justifyContent: 'space-between', gap: 2 }}>
                            <Typography variant="body2" color="text.secondary">
                              {zhShared.labels.updateTime}
                            </Typography>
                            <Typography variant="body2" sx={{ textAlign: 'right' }}>
                              {new Date(active.updatedAt * 1000).toLocaleString()}
                            </Typography>
                          </Stack>
                          {active.source && (
                            <>
                              <Divider />
                              <Button
                                variant="outlined"
                                size="small"
                                startIcon={<RefreshRounded />}
                                disabled={!canEdit}
                                onClick={() => refreshProfile(active)}
                              >
                                {zhProfiles.components.menu.update}
                              </Button>
                            </>
                          )}
                        </Stack>
                      ) : (
                        <MobileEmpty
                          text={zhProxies.page.empty.noSubscriptions.title}
                          action={
                            <Button
                              size="small"
                              variant="contained"
                              onClick={() => {
                                setPage('profiles')
                                openForm({ kind: 'subscription' })
                              }}
                            >
                              {zhProfiles.page.actions.import}
                            </Button>
                          }
                        />
                      )}
                    </EnhancedCard>
                  </Grid>

                  <Grid size={6}>
                    <EnhancedCard
                      title={zhHome.components.currentProxy.title}
                      icon={
                        homeIsDirectMode ? (
                          <SignalWifi4Bar />
                        ) : homeSelectedDelay === -2 ? (
                          <SignalWifi0Bar />
                        ) : typeof homeSelectedDelay !== 'number' ||
                          homeSelectedDelay <= 0 ? (
                          <SignalWifi0Bar />
                        ) : homeSelectedDelay >= 500 ? (
                          <WifiOff />
                        ) : homeSelectedDelay >= 300 ? (
                          <SignalWifi2Bar />
                        ) : (
                          <SignalWifi4Bar />
                        )
                      }
                      iconColor={homeSelectedProxyName ? 'primary' : undefined}
                      action={
                        <Box sx={{ display: 'flex', alignItems: 'center', gap: 0.5 }}>
                          <Tooltip title={zhHome.components.currentProxy.actions.refreshDelay}>
                            <span>
                              <IconButton
                                ref={homeDelayButtonRef}
                                size="small"
                                color="inherit"
                                disabled={
                                  busy ||
                                  homeIsDirectMode ||
                                  !runtime?.coreConnected ||
                                  globalDelayNames.length === 0
                                }
                                onClick={() => void testAllDelays()}
                              >
                                <NetworkCheckRounded />
                              </IconButton>
                            </span>
                          </Tooltip>
                          <Tooltip
                            title={
                              homeProxySort === 0
                                ? zhProxies.page.tooltips.sortDefault
                                : homeProxySort === 1
                                  ? zhProxies.page.tooltips.sortDelay
                                  : zhProxies.page.tooltips.sortName
                            }
                          >
                            <IconButton
                              size="small"
                              color="inherit"
                              disabled={homeIsDirectMode}
                              onClick={() => {
                                const next = ((homeProxySort + 1) % 3) as 0 | 1 | 2
                                setHomeProxySort(next)
                                window.localStorage.setItem(
                                  'cv4a-home-proxy-sort',
                                  String(next),
                                )
                              }}
                            >
                              {homeProxySort === 1 ? (
                                <AccessTimeRounded fontSize="small" />
                              ) : homeProxySort === 2 ? (
                                <SortByAlphaRounded fontSize="small" />
                              ) : (
                                <SortRounded fontSize="small" />
                              )}
                            </IconButton>
                          </Tooltip>
                          <Button
                            variant="outlined"
                            size="small"
                            onClick={() => setPage('proxies')}
                            sx={{ borderRadius: 1.5 }}
                            endIcon={<ChevronRight fontSize="small" />}
                          >
                            {zhLayout.components.navigation.tabs.proxies}
                          </Button>
                        </Box>
                      }
                    >
                      {!runtime?.coreConnected || !core ? (
                        <MobileEmpty text={zhHome.components.systemInfo.badges.notRunning} />
                      ) : homeSelectedProxyName || homeSelectedGroup ? (
                        <Box>
                          <Box
                            sx={(theme) => ({
                              display: 'flex',
                              alignItems: 'center',
                              justifyContent: 'space-between',
                              gap: 1,
                              p: 1,
                              mb: 2,
                              borderRadius: 1,
                              bgcolor: alpha(theme.palette.primary.main, 0.05),
                              border: `1px solid ${alpha(theme.palette.primary.main, 0.1)}`,
                            })}
                          >
                            <Box sx={{ minWidth: 0 }}>
                              <Typography variant="body1" sx={{ fontWeight: 'medium' }} noWrap>
                                {homeSelectedProxyName ||
                                  zhHome.components.currentProxy.labels.noActiveNode}
                              </Typography>
                              <Typography variant="caption" color="text.secondary">
                                {homeSelectedProxyType}
                              </Typography>
                            </Box>
                            {!homeIsDirectMode && homeSelectedProxyName && (
                              <Chip
                                size="small"
                                label={proxyDelayLabel(homeSelectedDelay)}
                                color={proxyDelayColor(homeSelectedDelay)}
                              />
                            )}
                          </Box>

                          <Box sx={{ mb: 1.5 }}>
                            <PersistentSelect
                              label={zhHome.components.currentProxy.labels.group}
                              displayValue={
                                <Typography noWrap>
                                  {homeIsDirectMode
                                    ? 'DIRECT'
                                    : homeIsGlobalMode
                                      ? (homeGlobalGroup?.name ?? 'GLOBAL')
                                      : homeProxyGroupName}
                                </Typography>
                              }
                              open={homeProxySelect === 'group'}
                              disabled={
                                busy ||
                                homeIsGlobalMode ||
                                homeIsDirectMode ||
                                homeSelectableGroups.length === 0
                              }
                              keepOpenRef={homeDelayButtonRef}
                              onOpenChange={(open) =>
                                setHomeProxySelect(open ? 'group' : null)
                              }
                              renderOptions={() =>
                                homeSelectableGroups.map((group) => (
                                  <MenuItem
                                    key={group.name}
                                    role="option"
                                    aria-selected={group.name === homeProxyGroupName}
                                    selected={group.name === homeProxyGroupName}
                                    onClick={() => {
                                      setHomeProxyGroupName(group.name)
                                      if (active) {
                                        window.localStorage.setItem(
                                          `cv4a-home-proxy-group:${active.id}`,
                                          group.name,
                                        )
                                      }
                                      setHomeProxySelect(null)
                                    }}
                                  >
                                    <Typography noWrap>{group.name}</Typography>
                                  </MenuItem>
                                ))
                              }
                            />
                          </Box>

                          <PersistentSelect
                            label={zhHome.components.currentProxy.labels.proxy}
                            displayValue={
                              <Box
                                sx={{
                                  display: 'flex',
                                  alignItems: 'center',
                                  justifyContent: 'space-between',
                                  gap: 1,
                                }}
                              >
                                <Typography noWrap sx={{ minWidth: 0, flex: 1 }}>
                                  {homeSelectedProxyName ||
                                    zhHome.components.currentProxy.labels.noActiveNode}
                                </Typography>
                                {!homeIsDirectMode && homeSelectedProxyName && (
                                  <Chip
                                    size="small"
                                    label={proxyDelayLabel(homeSelectedDelay)}
                                    color={proxyDelayColor(homeSelectedDelay)}
                                  />
                                )}
                              </Box>
                            }
                            open={homeProxySelect === 'proxy'}
                            disabled={
                              busy ||
                              homeIsDirectMode ||
                              !homeSelectedGroup ||
                              !homeSelectedGroup.selectable
                            }
                            keepOpenRef={homeDelayButtonRef}
                            onOpenChange={(open) =>
                              setHomeProxySelect(open ? 'proxy' : null)
                            }
                            renderOptions={() =>
                              homeProxyMembers.map((member) => {
                                const selected = member === homeSelectedProxyName
                                const delay = delays[member]
                                return (
                                  <MenuItem
                                    key={member}
                                    role="option"
                                    aria-selected={selected}
                                    selected={selected}
                                    onClick={() => {
                                      if (!homeSelectedGroup) return
                                      setHomeProxySelect(null)
                                      void coreAction(
                                        () =>
                                          api.selectProxy(homeSelectedGroup.name, member),
                                      )
                                    }}
                                    sx={{
                                      display: 'flex',
                                      justifyContent: 'space-between',
                                      alignItems: 'center',
                                      width: '100%',
                                      pr: 1,
                                    }}
                                  >
                                    <Typography noWrap sx={{ flex: 1, mr: 1 }}>
                                      {member}
                                    </Typography>
                                    <Chip
                                      size="small"
                                      label={proxyDelayLabel(delay)}
                                      color={proxyDelayColor(delay)}
                                      sx={{ minWidth: 60, height: 22, flexShrink: 0 }}
                                    />
                                  </MenuItem>
                                )
                              })
                            }
                          />
                        </Box>
                      ) : (
                        <MobileEmpty
                          text={zhHome.components.currentProxy.labels.noActiveNode}
                        />
                      )}
                    </EnhancedCard>
                  </Grid>

                  <Grid size={6}>
                    <EnhancedCard
                      title={zhHome.page.cards.networkSettings}
                      icon={<DnsOutlined />}
                      iconColor="primary"
                    >
                      <Stack direction="row" spacing={1} sx={{ mb: 1 }}>
                        <Paper
                          elevation={networkPanel === 'system' ? 2 : 0}
                          role="button"
                          tabIndex={0}
                          aria-pressed={networkPanel === 'system'}
                          onClick={() => setNetworkPanel('system')}
                          onKeyDown={(event) => {
                            if (event.key === 'Enter' || event.key === ' ') {
                              event.preventDefault()
                              setNetworkPanel('system')
                            }
                          }}
                          sx={{
                            flex: 1,
                            px: 1,
                            py: 1,
                            display: 'flex',
                            alignItems: 'center',
                            justifyContent: 'center',
                            gap: 1,
                            borderRadius: 1.5,
                            cursor: 'pointer',
                            bgcolor: networkPanel === 'system'
                              ? 'primary.main'
                              : 'background.paper',
                            color: networkPanel === 'system'
                              ? 'primary.contrastText'
                              : 'text.primary',
                          }}
                        >
                          <ComputerRounded fontSize="small" />
                          <Typography
                            variant="body2"
                            sx={{ fontWeight: networkPanel === 'system' ? 600 : 400 }}
                          >
                            {zhSettings.sections.system.toggles.systemProxy}
                          </Typography>
                        </Paper>
                        <Paper
                          elevation={networkPanel === 'tun' ? 2 : 0}
                          role="button"
                          tabIndex={0}
                          aria-pressed={networkPanel === 'tun'}
                          onClick={() => setNetworkPanel('tun')}
                          onKeyDown={(event) => {
                            if (event.key === 'Enter' || event.key === ' ') {
                              event.preventDefault()
                              setNetworkPanel('tun')
                            }
                          }}
                          sx={{
                            flex: 1,
                            px: 1,
                            py: 1,
                            display: 'flex',
                            alignItems: 'center',
                            justifyContent: 'center',
                            gap: 1,
                            borderRadius: 1.5,
                            cursor: 'pointer',
                            bgcolor: networkPanel === 'tun'
                              ? 'primary.main'
                              : 'background.paper',
                            color: networkPanel === 'tun'
                              ? 'primary.contrastText'
                              : 'text.primary',
                          }}
                        >
                          <TroubleshootRounded fontSize="small" />
                          <Typography
                            variant="body2"
                            sx={{ fontWeight: networkPanel === 'tun' ? 600 : 400 }}
                          >
                            {zhSettings.sections.system.toggles.tunMode}
                          </Typography>
                        </Paper>
                      </Stack>
                      <Box
                        sx={(theme) => ({
                          p: 1,
                          bgcolor: alpha(theme.palette.primary.main, 0.04),
                          borderRadius: 2,
                        })}
                      >
                        {networkPanel === 'system' ? (
                          <SettingItem label={zhSettings.sections.system.toggles.systemProxy}>
                            <Switch
                              edge="end"
                              checked={systemProxy?.active ?? false}
                              disabled={!caps.systemProxy || busy || !active}
                              onChange={(_, checked) =>
                                void setAndroidSystemProxy(checked)
                              }
                            />
                          </SettingItem>
                        ) : (
                          <SettingItem label={zhSettings.sections.system.toggles.tunMode}>
                            <Switch
                              edge="end"
                              checked={runtime?.transparentActive ?? false}
                              disabled={
                                !caps.tunControl ||
                                busy ||
                                !runtime?.agentConnected ||
                                !active
                              }
                              onChange={(_, checked) => setTransparentProxy(checked)}
                            />
                          </SettingItem>
                        )}
                      </Box>
                    </EnhancedCard>
                  </Grid>

                  <Grid size={6}>
                    <EnhancedCard
                      title={zhHome.page.cards.proxyMode}
                      icon={<RouterOutlined />}
                      iconColor="info"
                    >
                      <MobileModeButtons
                        mode={core?.mode ?? summary?.mode}
                        disabled={busy || !runtime?.coreConnected}
                        onChange={(mode) =>
                          void coreAction(() => api.setMode(mode))
                        }
                      />
                    </EnhancedCard>
                  </Grid>

                  <Grid size={6}>
                    <EnhancedCard
                      title={zhHome.page.cards.trafficStats}
                      icon={<SpeedOutlined />}
                      iconColor="secondary"
                    >
                      <MobileTrafficStats
                        upload={core ? formatRate(traffic.upload) : '—'}
                        download={core ? formatRate(traffic.download) : '—'}
                      />
                      <Stack spacing={1} sx={{ mt: 1.5 }}>
                        <Divider />
                        <Stack direction="row" sx={{ justifyContent: 'space-between' }}>
                          <Typography variant="body2" color="text.secondary">
                            {zhHome.components.traffic.metrics.activeConnections}
                          </Typography>
                          <Typography variant="body2">
                            {connections?.connections.length ?? 0}
                          </Typography>
                        </Stack>
                        <Stack direction="row" sx={{ justifyContent: 'space-between' }}>
                          <Typography variant="body2" color="text.secondary">
                            {zhShared.labels.downloaded}
                          </Typography>
                          <Typography variant="body2">
                            {formatBytes(connections?.downloadTotal ?? 0)}
                          </Typography>
                        </Stack>
                      </Stack>
                    </EnhancedCard>
                  </Grid>

                  <Grid size={6}>
                    <EnhancedCard
                      title={zhHome.components.clashInfo.title}
                      icon={<DeveloperBoardOutlined />}
                      iconColor="warning"
                    >
                      <Stack spacing={1.25}>
                        <Stack direction="row" sx={{ justifyContent: 'space-between' }}>
                          <Typography variant="body2" color="text.secondary">
                            {zhHome.components.clashInfo.fields.coreVersion}
                          </Typography>
                          <Typography variant="body2">
                            {core?.version ?? '-'}
                          </Typography>
                        </Stack>
                        <Divider />
                        <Stack direction="row" sx={{ justifyContent: 'space-between' }}>
                          <Typography variant="body2" color="text.secondary">
                            {zhHome.components.clashInfo.fields.rulesCount}
                          </Typography>
                          <Typography variant="body2">
                            {core?.rules.length ?? summary?.rules.length ?? 0}
                          </Typography>
                        </Stack>
                        <Divider />
                      </Stack>
                    </EnhancedCard>
                  </Grid>
                </Grid>
              )}

              {page === 'profiles' && (
                <>
                  <Stack direction="row" spacing={1} sx={{ pt: 1, mb: 1, px: 0.5 }}>
                    <Button
                      variant="contained"
                      size="small"
                      disabled={!canEdit}
                      onClick={() => openForm({ kind: 'subscription' })}
                    >
                      {zhProfiles.page.actions.import}
                    </Button>
                    <Button
                      variant="contained"
                      size="small"
                      disabled={!canEdit}
                      onClick={() => openForm({ kind: 'yaml' })}
                    >
                      YAML
                    </Button>
                    <Button
                      variant="outlined"
                      size="small"
                      disabled={!canEdit}
                      onClick={() => openForm({ kind: 'nodes' })}
                    >
                      URL
                    </Button>
                    <Button
                      variant={doc.globalMergeYaml ? 'contained' : 'outlined'}
                      size="small"
                      disabled={!canEdit}
                      onClick={() => openMergeEditor({ kind: 'global' })}
                    >
                      {zhProfiles.components.more.global.merge}
                    </Button>
                    <Button
                      variant={doc.globalScriptJs ? 'contained' : 'outlined'}
                      size="small"
                      disabled={!canEdit}
                      onClick={() => openScriptEditor({ kind: 'global' })}
                    >
                      {zhProfiles.components.more.global.script}
                    </Button>
                  </Stack>

                  {doc.profiles.length === 0 ? (
                    empty(zhProxies.page.empty.noSubscriptions.title)
                  ) : (
                    <Box
                      sx={{
                        display: 'grid',
                        gridTemplateColumns: 'repeat(auto-fill, minmax(260px, 1fr))',
                        gap: 1,
                        px: 0.5,
                      }}
                    >
                      {doc.profiles.map((profile) => {
                        const selected = profile.id === doc.activeId
                        const batchSelected = selectedProfiles.has(profile.id)
                        return (
                          <ProfileBox
                            key={profile.id}
                            aria-selected={selected}
                            onClick={() => {
                              if (profileBatchMode) {
                                toggleProfileSelection(profile.id)
                              } else if (!selected) {
                                void activateProfile(profile)
                              }
                            }}
                          >
                            <Box sx={{ position: 'relative' }}>
                              <Box sx={{ display: 'flex', justifyContent: 'start' }}>
                                {profileBatchMode && (
                                  <IconButton
                                    size="small"
                                    sx={{
                                      p: '2px',
                                      mr: '4px',
                                      ml: '-8px',
                                    }}
                                    onClick={(event) => {
                                      event.stopPropagation()
                                      toggleProfileSelection(profile.id)
                                    }}
                                  >
                                    {batchSelected ? (
                                      <CheckBoxRounded color="primary" />
                                    ) : (
                                      <CheckBoxOutlineBlankRounded />
                                    )}
                                  </IconButton>
                                )}
                                <Typography
                                  variant="h6"
                                  component="h2"
                                  noWrap
                                  title={profile.name}
                                  sx={{
                                    width: profileBatchMode
                                      ? 'calc(100% - 56px)'
                                      : 'calc(100% - 36px)',
                                    fontSize: 18,
                                    fontWeight: 600,
                                    lineHeight: '26px',
                                  }}
                                >
                                  {profile.name}
                                </Typography>
                              </Box>
                              {!profileBatchMode && profile.source && (
                                <IconButton
                                  title={zhShared.actions.refresh}
                                  size="small"
                                  color="inherit"
                                  disabled={!canEdit}
                                  onClick={(event) => {
                                    event.stopPropagation()
                                    void refreshProfile(profile)
                                  }}
                                  sx={{ position: 'absolute', p: '3px', top: -1, right: 26 }}
                                >
                                  <RefreshRounded color="inherit" />
                                </IconButton>
                              )}
                              {!profileBatchMode && (
                                <IconButton
                                  size="small"
                                  color="inherit"
                                  onClick={(event) => {
                                    event.stopPropagation()
                                    setProfileMenu({ anchor: event.currentTarget, profile })
                                  }}
                                  sx={{ position: 'absolute', p: '3px', top: -1, right: -5 }}
                                >
                                  <MoreVertRounded color="inherit" />
                                </IconButton>
                              )}
                            </Box>
                            <Stack direction="row" sx={{ justifyContent: 'space-between', mt: 0.75 }}>
                              <Typography
                                variant="body2"
                                noWrap
                                title={
                                  profile.source ?? zhProfiles.modals.profileForm.types.local
                                }
                              >
                                {sourceLabel(profile.source)}
                              </Typography>
                              <Typography variant="body2" sx={{ ml: 1, flexShrink: 0 }}>
                                {new Date(profile.updatedAt * 1000).toLocaleString()}
                              </Typography>
                            </Stack>
                          </ProfileBox>
                        )
                      })}
                    </Box>
                  )}

                  <Menu
                    open={Boolean(profileMenu)}
                    anchorEl={profileMenu?.anchor ?? null}
                    onClose={() => setProfileMenu(null)}
                    slotProps={{ list: { sx: { py: 0.5 } } }}
                  >
                    <MenuItem
                      dense
                      disabled={profileMenu?.profile.id === doc.activeId}
                      onClick={() => {
                        const profile = profileMenu?.profile
                        setProfileMenu(null)
                        if (profile) void activateProfile(profile)
                      }}
                    >
                      {zhProfiles.components.menu.select}
                    </MenuItem>
                    {profileMenu?.profile.source && (
                      <MenuItem
                        dense
                        onClick={() => {
                          const profile = profileMenu.profile
                          setProfileMenu(null)
                          void refreshProfile(profile)
                        }}
                      >
                        {zhProfiles.components.menu.update}
                      </MenuItem>
                    )}
                    <MenuItem
                      dense
                      onClick={() => {
                        const profile = profileMenu?.profile
                        setProfileMenu(null)
                        if (profile) openForm({ kind: 'edit', profile })
                      }}
                    >
                      {zhProfiles.components.menu.editFile}
                    </MenuItem>
                    <MenuItem
                      dense
                      onClick={() => {
                        const profile = profileMenu?.profile
                        setProfileMenu(null)
                        if (profile) openMergeEditor({ kind: 'profile', profile })
                      }}
                    >
                      {zhProfiles.components.menu.extendConfig}
                    </MenuItem>
                    <MenuItem
                      dense
                      onClick={() => {
                        const profile = profileMenu?.profile
                        setProfileMenu(null)
                        if (profile) openScriptEditor({ kind: 'profile', profile })
                      }}
                    >
                      {zhProfiles.components.menu.extendScript}
                    </MenuItem>
                    <MenuItem
                      dense
                      onClick={() => {
                        const profile = profileMenu?.profile
                        setProfileMenu(null)
                        if (profile) openSequenceEditor({ kind: 'rules', profile })
                      }}
                    >
                      {zhProfiles.components.menu.editRules}
                    </MenuItem>
                    <MenuItem
                      dense
                      onClick={() => {
                        const profile = profileMenu?.profile
                        setProfileMenu(null)
                        if (profile) openSequenceEditor({ kind: 'proxies', profile })
                      }}
                    >
                      {zhProfiles.components.menu.editProxies}
                    </MenuItem>
                    <MenuItem
                      dense
                      onClick={() => {
                        const profile = profileMenu?.profile
                        setProfileMenu(null)
                        if (profile) openSequenceEditor({ kind: 'groups', profile })
                      }}
                    >
                      {zhProfiles.components.menu.editGroups}
                    </MenuItem>
                    <MenuItem
                      dense
                      disabled={profileMenu?.profile.id === doc.activeId}
                      sx={{ color: 'error.main' }}
                      onClick={() => {
                        const profile = profileMenu?.profile
                        setProfileMenu(null)
                        if (profile) openForm({ kind: 'delete', profile })
                      }}
                    >
                      {zhShared.actions.delete}
                    </MenuItem>
                  </Menu>
                </>
              )}

              {page === 'proxies' && (
                <>
                  <Stack direction="row" spacing={1} sx={{ pt: 1, mb: 1, px: 1 }}>
                    <MobileProxyProviderButton
                      enabled={runtime?.coreConnected ?? false}
                      onFeedback={record}
                    />
                    <Box sx={{ flex: 1 }}>
                      <MobileModeButtons
                        mode={core?.mode ?? summary?.mode}
                        disabled={busy || !runtime?.coreConnected}
                        onChange={(mode) =>
                          void coreAction(() => api.setMode(mode))
                        }
                      />
                    </Box>
                    <Button
                      size="small"
                      variant={active?.proxyChain ? 'contained' : 'outlined'}
                      disabled={busy || !runtime?.coreConnected || !active}
                      onClick={openProxyChain}
                      startIcon={active?.proxyChain ? <LanRounded /> : <LanOutlined />}
                    >
                      {zhProxies.page.actions.toggleChain}
                    </Button>
                    <IconButton
                      size="small"
                      color="inherit"
                      title={zhProxies.page.tooltips.delayCheck}
                      disabled={busy || !runtime?.coreConnected || globalDelayNames.length === 0}
                      onClick={() => void testAllDelays()}
                    >
                      <NetworkCheckRounded />
                    </IconButton>
                  </Stack>
                  <Stack
                    direction="row"
                    spacing={1}
                    sx={{ mb: 1, px: 1, alignItems: 'center' }}
                  >
                    <Box sx={{ flex: 1, minWidth: 0 }}>
                      <BaseSearchBox
                        placeholder={zhProxies.page.tooltips.filter}
                        onSearch={(match, state) => {
                          proxyMatcher.current = match
                          setProxySearchState(state)
                        }}
                      />
                    </Box>
                    <ButtonGroup size="small" variant="outlined">
                      <Button
                        title={zhProxies.page.tooltips.sortDefault}
                        variant={proxySort === 0 ? 'contained' : 'outlined'}
                        onClick={() => setProxySort(0)}
                      >
                        <SortRounded fontSize="small" />
                      </Button>
                      <Button
                        title={zhProxies.page.tooltips.sortName}
                        variant={proxySort === 1 ? 'contained' : 'outlined'}
                        onClick={() => setProxySort(1)}
                      >
                        <SortByAlphaRounded fontSize="small" />
                      </Button>
                      <Button
                        title={zhProxies.page.tooltips.sortDelay}
                        variant={proxySort === 2 ? 'contained' : 'outlined'}
                        onClick={() => setProxySort(2)}
                      >
                        <SpeedOutlined fontSize="small" />
                      </Button>
                    </ButtonGroup>
                  </Stack>
                  {!active
                    ? empty(zhProxies.page.empty.inactiveSubscription.title)
                    : (core?.groups ?? summary?.groups ?? []).map((group) => {
                        const runtimeGroup = 'selected' in group ? group : null
                        const members = group.members
                          .filter((member) => {
                            const protocol =
                              summary?.nodes.find((node) => node.name === member)
                                ?.protocol ?? 'Group'
                            return proxyMatcher.current(`${member} ${protocol}`)
                          })
                          .sort((a, b) => {
                            if (proxySort === 1) return a.localeCompare(b)
                            if (proxySort === 2) {
                              const aDelay = delays[a]
                              const bDelay = delays[b]
                              const aValue =
                                typeof aDelay === 'number' ? aDelay : Number.MAX_SAFE_INTEGER
                              const bValue =
                                typeof bDelay === 'number' ? bDelay : Number.MAX_SAFE_INTEGER
                              return aValue - bValue
                            }
                            return 0
                          })
                        if (proxySearchState.text && members.length === 0) return null
                        return (
                          <Box key={group.name} sx={{ mb: 1.5 }}>
                            <Stack
                              direction="row"
                              sx={{
                                justifyContent: 'space-between',
                                alignItems: 'baseline',
                                px: 2,
                                py: 0.75,
                              }}
                            >
                              <Typography sx={{ fontSize: 16, fontWeight: 600 }} noWrap>
                                {group.name}
                              </Typography>
                              <Stack direction="row" spacing={0.5} sx={{ alignItems: 'center' }}>
                                <Typography variant="caption" color="text.secondary">
                                  {group.kind} · {members.length}
                                </Typography>
                                <IconButton
                                  size="small"
                                  color="inherit"
                                  title={zhProxies.page.tooltips.delayCheck}
                                  aria-label={zhProxies.page.tooltips.delayCheck}
                                  disabled={
                                    busy ||
                                    !runtime?.coreConnected ||
                                    globalDelayNames.length === 0
                                  }
                                  onClick={() => void testAllDelays()}
                                >
                                  <NetworkCheckRounded fontSize="inherit" />
                                </IconButton>
                              </Stack>
                            </Stack>
                            <List disablePadding>
                              {members.map((member) => (
                                <ProxyItemView
                                  key={member}
                                  name={member}
                                  type={
                                    summary?.nodes.find((node) => node.name === member)?.protocol ??
                                    'Group'
                                  }
                                  selected={runtimeGroup?.selected === member}
                                  disabled={
                                    busy ||
                                    !runtimeGroup ||
                                    !runtimeGroup.selectable
                                  }
                                  delayValue={
                                    typeof delays[member] === 'number'
                                      ? delays[member]!
                                      : -1
                                  }
                                  delayText={
                                    typeof delays[member] === 'number' &&
                                    delays[member]! > 0
                                      ? `${delays[member]} ms`
                                      : undefined
                                  }
                                  delayColor={
                                    typeof delays[member] === 'number' &&
                                    delays[member]! > 800
                                      ? 'warning.main'
                                      : 'success.main'
                                  }
                                  isPreset={[
                                    'DIRECT',
                                    'REJECT',
                                    'REJECT-DROP',
                                    'PASS',
                                    'COMPATIBLE',
                                  ].includes(member)}
                                  sx={{ p: 0 }}
                                  onClick={() =>
                                    runtimeGroup &&
                                    void coreAction(
                                      () => api.selectProxy(group.name, member),
                                    )
                                  }
                                  onDelay={() => void testDelay(member)}
                                />
                              ))}
                            </List>
                          </Box>
                        )
                      })}
                </>
              )}

              {page === 'rules' && (
                <Box sx={{ height: '100%', display: 'flex', flexDirection: 'column' }}>
                  <Box sx={{ pt: 1, mb: 0.5, mx: '10px', minHeight: 36 }}>
                    <BaseSearchBox
                      placeholder={zhShared.placeholders.filter}
                      onSearch={(match, state) => {
                        ruleMatcher.current = match
                        setRuleSearchState(state)
                      }}
                    />
                  </Box>
                  {!active ? (
                    empty(zhProxies.page.empty.inactiveSubscription.title)
                  ) : ruleRows.length ? (
                    <Box>
                      {ruleRows.map((rule) => (
                        <RuleItem
                          key={rule.index}
                          value={{
                            type: rule.type,
                            payload: rule.payload,
                            proxy: rule.proxy,
                            lineNo: rule.index,
                          }}
                        />
                      ))}
                    </Box>
                  ) : (
                    <MobileEmpty text={zhShared.statuses.empty} />
                  )}
                </Box>
              )}

              {page === 'connections' && (
                <>
                  {!runtime?.coreConnected ? (
                    <MobileEmpty text={zhHome.components.systemInfo.badges.notRunning} />
                  ) : (
                    <>
                      <Stack
                        direction="row"
                        spacing={1}
                        sx={{
                          px: 1,
                          pt: 1,
                          alignItems: 'center',
                          flexWrap: 'wrap',
                          rowGap: 1,
                        }}
                      >
                        <ButtonGroup size="small" sx={{ flexShrink: 0 }}>
                          <Button
                            variant={
                              connectionView === 'active' ? 'contained' : 'outlined'
                            }
                            onClick={() => setConnectionView('active')}
                          >
                            {zhConnections.components.actions.active}{' '}
                            {connections?.connections.length ?? 0}
                          </Button>
                          <Button
                            variant={
                              connectionView === 'closed' ? 'contained' : 'outlined'
                            }
                            onClick={() => setConnectionView('closed')}
                          >
                            {zhConnections.components.actions.closed}{' '}
                            {closedConnections.length}
                          </Button>
                        </ButtonGroup>
                        <BaseStyledSelect
                          value={connectionSort}
                          onChange={(event) =>
                            setConnectionSort(
                              event.target.value as 'start' | 'upload' | 'download',
                            )
                          }
                        >
                          <MenuItem value="start">
                            {zhConnections.components.order.default}
                          </MenuItem>
                          <MenuItem value="upload">
                            {zhConnections.components.order.uploadSpeed}
                          </MenuItem>
                          <MenuItem value="download">
                            {zhConnections.components.order.downloadSpeed}
                          </MenuItem>
                        </BaseStyledSelect>
                        <Box sx={{ flex: 1, minWidth: 0 }}>
                          <BaseSearchBox
                            placeholder={zhShared.placeholders.filter}
                            onSearch={(match, state) => {
                              connectionMatcher.current = match
                              setConnectionSearchState(state)
                            }}
                          />
                        </Box>
                      </Stack>
                      <Stack
                        direction="row"
                        spacing={2}
                        sx={{ px: 1.5, py: 1, color: 'text.secondary' }}
                      >
                        <Typography variant="body2">
                          {zhShared.labels.downloaded}: {formatBytes(connections?.downloadTotal ?? 0)}
                        </Typography>
                        <Typography variant="body2">
                          {zhShared.labels.uploaded}: {formatBytes(connections?.uploadTotal ?? 0)}
                        </Typography>
                      </Stack>
                      <Box>
                        {connectionRows.map((row) => (
                          <ConnectionRowItem
                            key={row.id}
                            row={row}
                            closed={connectionView === 'closed'}
                            onShowDetail={showConnectionDetail}
                            onClose={
                              connectionView === 'active' ? closeConnection : undefined
                            }
                          />
                        ))}
                        {connections && connectionRows.length === 0 && (
                          <MobileEmpty text={zhShared.statuses.empty} />
                        )}
                      </Box>
                    </>
                  )}
                </>
              )}

              {page === 'logs' && (
                <Box>
                  {runtime?.agentConnected ? (
                    coreLogs ? (
                      <>
                        <Box
                          sx={{
                            pt: 1,
                            mb: 0.5,
                            mx: '10px',
                            minHeight: 39,
                            display: 'flex',
                            alignItems: 'center',
                          }}
                        >
                          <BaseStyledSelect
                            value={logFilter}
                            onChange={(event) =>
                              setLogFilter(
                                event.target.value as
                                  | 'all'
                                  | 'debug'
                                  | 'info'
                                  | 'warn'
                                  | 'err',
                              )
                            }
                          >
                            <MenuItem value="all">{zhShared.filters.logLevels.all}</MenuItem>
                            <MenuItem value="debug">Debug</MenuItem>
                            <MenuItem value="info">Info</MenuItem>
                            <MenuItem value="warn">Warn</MenuItem>
                            <MenuItem value="err">Error</MenuItem>
                          </BaseStyledSelect>
                          <BaseSearchBox
                            placeholder={zhShared.placeholders.filter}
                            onSearch={(match, state) => {
                              logMatcher.current = match
                              setLogSearchState(state)
                            }}
                          />
                        </Box>
                        {logItems.length ? (
                          logItems.map((item, index) => (
                            <LogItem
                              key={`${index}-${item.time ?? ''}-${item.payload.slice(0, 24)}`}
                              value={item}
                              searchState={logSearchState}
                            />
                          ))
                        ) : (
                          <MobileEmpty text={zhShared.statuses.empty} />
                        )}
                      </>
                    ) : (
                      <MobileEmpty text={zhShared.statuses.empty} />
                    )
                  ) : (
                    <MobileEmpty text={zhHome.components.systemInfo.badges.notRunning} />
                  )}
                </Box>
              )}

              {page === 'settings' && (
                <Grid container spacing={1.5} columns={6}>
                  <Grid size={6}>
                    <Box className="verge-setting-panel">
                      <SettingList title={zhSettings.components.verge.basic.title}>
                        <SettingItem label={zhSettings.components.verge.basic.fields.themeMode}>
                          <ThemeModeSwitch
                            value={themeMode}
                            onChange={(mode) => {
                              if (mode) updateThemeMode(mode)
                            }}
                          />
                        </SettingItem>
                        <SettingItem label={zhSettings.components.verge.basic.fields.startPage}>
                          <BaseStyledSelect
                            size="small"
                            value={mobilePreferences?.startPage ?? 'home'}
                            disabled={busy || mobilePreferences === null}
                            onChange={(event) =>
                              void updateMobilePreferences({
                                startPage: event.target.value as Page,
                              })
                            }
                            sx={{ width: 120, '> div': { py: '7.5px' } }}
                          >
                            <MenuItem value="home">{zhLayout.components.navigation.tabs.home}</MenuItem>
                            <MenuItem value="proxies">{zhLayout.components.navigation.tabs.proxies}</MenuItem>
                            <MenuItem value="profiles">{zhLayout.components.navigation.tabs.profiles}</MenuItem>
                            <MenuItem value="connections">{zhLayout.components.navigation.tabs.connections}</MenuItem>
                            <MenuItem value="rules">{zhLayout.components.navigation.tabs.rules}</MenuItem>
                            <MenuItem value="logs">{zhLayout.components.navigation.tabs.logs}</MenuItem>
                            <MenuItem value="settings">{zhLayout.components.navigation.tabs.settings}</MenuItem>
                          </BaseStyledSelect>
                        </SettingItem>
                      </SettingList>
                    </Box>
                  </Grid>

                  <Grid size={6}>
                    <Box className="verge-setting-panel">
                      <SettingList title={zhSettings.modals.misc.title}>
                        <SettingItem
                          label={zhSettings.modals.misc.fields.autoCloseConnections}
                          secondary={zhSettings.modals.misc.tooltips.autoCloseConnections}
                        >
                          <Switch
                            checked={mobilePreferences?.autoCloseConnection ?? true}
                            disabled={busy || mobilePreferences === null}
                            onChange={(_, autoCloseConnection) =>
                              void updateMobilePreferences({ autoCloseConnection })
                            }
                          />
                        </SettingItem>
                        <SettingItem
                          label={zhSettings.modals.misc.fields.autoDelayDetection}
                          secondary={zhSettings.modals.misc.tooltips.autoDelayDetection}
                        >
                          <Switch
                            checked={
                              mobilePreferences?.enableAutoDelayDetection ?? false
                            }
                            disabled={busy || mobilePreferences === null}
                            onChange={(_, enableAutoDelayDetection) =>
                              void updateMobilePreferences({
                                enableAutoDelayDetection,
                              })
                            }
                          />
                        </SettingItem>
                        <SettingItem
                          label={zhSettings.modals.misc.fields.autoDelayDetectionInterval}
                          secondary={zhShared.units.minutes}
                        >
                          <TextField
                            size="small"
                            type="number"
                            disabled={
                              busy ||
                              !mobilePreferences?.enableAutoDelayDetection
                            }
                            value={
                              mobilePreferences?.autoDelayDetectionIntervalMinutes ?? 5
                            }
                            slotProps={{ htmlInput: { min: 1, max: 1440 } }}
                            onChange={(event) =>
                              setMobilePreferences((old) =>
                                old
                                  ? {
                                      ...old,
                                      autoDelayDetectionIntervalMinutes:
                                        Number.parseInt(event.target.value, 10) || 1,
                                    }
                                  : old,
                              )
                            }
                            onBlur={() => {
                              if (mobilePreferences)
                                void updateMobilePreferences({
                                  autoDelayDetectionIntervalMinutes:
                                    mobilePreferences.autoDelayDetectionIntervalMinutes,
                                })
                            }}
                            sx={{ width: 100 }}
                          />
                        </SettingItem>
                        <SettingItem
                          label={zhSettings.modals.misc.fields.defaultLatencyTest}
                          secondary={zhSettings.modals.misc.tooltips.defaultLatencyTest}
                        >
                          <TextField
                            size="small"
                            disabled={busy || mobilePreferences === null}
                            value={mobilePreferences?.defaultLatencyTest ?? ''}
                            onChange={(event) =>
                              setMobilePreferences((old) =>
                                old
                                  ? { ...old, defaultLatencyTest: event.target.value }
                                  : old,
                              )
                            }
                            onBlur={() => {
                              if (mobilePreferences)
                                void updateMobilePreferences({
                                  defaultLatencyTest:
                                    mobilePreferences.defaultLatencyTest,
                                })
                            }}
                            sx={{ width: 260, maxWidth: '55vw' }}
                          />
                        </SettingItem>
                        <SettingItem
                          label={zhSettings.modals.misc.fields.defaultLatencyTimeout}
                          secondary={zhShared.units.milliseconds}
                        >
                          <TextField
                            size="small"
                            type="number"
                            disabled={busy || mobilePreferences === null}
                            value={mobilePreferences?.defaultLatencyTimeout ?? 10000}
                            slotProps={{ htmlInput: { min: 100, max: 120000 } }}
                            onChange={(event) =>
                              setMobilePreferences((old) =>
                                old
                                  ? {
                                      ...old,
                                      defaultLatencyTimeout:
                                        Number.parseInt(event.target.value, 10) || 100,
                                    }
                                  : old,
                              )
                            }
                            onBlur={() => {
                              if (mobilePreferences)
                                void updateMobilePreferences({
                                  defaultLatencyTimeout:
                                    mobilePreferences.defaultLatencyTimeout,
                                })
                            }}
                            sx={{ width: 110 }}
                          />
                        </SettingItem>
                      </SettingList>
                    </Box>
                  </Grid>

                  <Grid size={6}>
                    <Box className="verge-setting-panel">
                      <SettingList title={zhSettings.sections.system.title}>
                        <SettingItem label={zhSettings.sections.system.toggles.systemProxy}>
                          <Switch
                            edge="end"
                            checked={systemProxy?.active ?? false}
                            disabled={!caps.systemProxy || busy || !active}
                            onChange={(_, checked) =>
                              void setAndroidSystemProxy(checked)
                            }
                          />
                        </SettingItem>
                        <SettingItem label={zhSettings.sections.system.toggles.tunMode}>
                          <Switch
                            edge="end"
                            checked={runtime?.transparentActive ?? false}
                            disabled={
                              !caps.tunControl ||
                              busy ||
                              !runtime?.agentConnected ||
                              !active
                            }
                            onChange={(_, checked) => setTransparentProxy(checked)}
                          />
                        </SettingItem>
                        <SettingItem
                          label={zhSettings.modals.tun.title}
                          onClick={openTunSettings}
                        />
                        <SettingItem
                          label={zhSettings.sections.system.fields.autoLaunch}
                        >
                          <Switch
                            edge="end"
                            checked={bootModule?.installed ?? false}
                            disabled={busy || bootModule === null}
                            onChange={(_, checked) => void setBootAutostart(checked)}
                          />
                        </SettingItem>
                      </SettingList>
                    </Box>
                  </Grid>

                  <Grid size={6}>
                    <Box className="verge-setting-panel">
                      <SettingList title={zhSettings.sections.clash.title}>
                        <SettingItem label={zhSettings.sections.clash.form.fields.ipv6}>
                          <Switch
                            edge="end"
                            checked={runtimePreferences?.ipv6 ?? false}
                            disabled={
                              busy ||
                              runtimePreferences === null ||
                              !runtime?.agentConnected ||
                              !active
                            }
                            onChange={(_, checked) =>
                              void updateRuntimePreferences({ ipv6: checked })
                            }
                          />
                        </SettingItem>
                        <SettingItem
                          label={zhSettings.sections.clash.form.fields.unifiedDelay}
                          secondary={zhSettings.sections.clash.form.tooltips.unifiedDelay}
                        >
                          <Switch
                            edge="end"
                            checked={runtimePreferences?.unifiedDelay ?? false}
                            disabled={
                              busy ||
                              runtimePreferences === null ||
                              !runtime?.agentConnected ||
                              !active
                            }
                            onChange={(_, checked) =>
                              void updateRuntimePreferences({ unifiedDelay: checked })
                            }
                          />
                        </SettingItem>
                        <SettingItem label={zhSettings.sections.clash.form.fields.logLevel}>
                          <BaseStyledSelect
                            size="small"
                            value={runtimePreferences?.logLevel ?? 'info'}
                            disabled={
                              busy ||
                              runtimePreferences === null ||
                              !runtime?.agentConnected ||
                              !active
                            }
                            onChange={(event) =>
                              void updateRuntimePreferences({
                                logLevel: String(event.target.value),
                              })
                            }
                            sx={{ width: 110, '> div': { py: '7.5px' } }}
                          >
                            <MenuItem value="debug">Debug</MenuItem>
                            <MenuItem value="info">Info</MenuItem>
                            <MenuItem value="warning">Warning</MenuItem>
                            <MenuItem value="error">Error</MenuItem>
                            <MenuItem value="silent">Silent</MenuItem>
                          </BaseStyledSelect>
                        </SettingItem>
                        <SettingItem label={zhSettings.sections.clash.form.fields.allowLan}>
                          <Switch
                            checked={portSettings?.allowLan ?? false}
                            disabled={busy || portSettings === null}
                            onChange={(_, checked) => void setAllowLan(checked)}
                          />
                        </SettingItem>
                        <SettingItem
                          label={zhSettings.sections.clash.form.fields.portConfig}
                          onClick={openPortSettings}
                        />
                        <SettingItem
                          label={zhSettings.sections.clash.form.fields.external}
                          onClick={openExternalControllerSettings}
                        />
                        <SettingItem
                          label={zhSettings.sections.clash.form.tooltips.networkInterface}
                          onClick={() => void openNetworkInterfaces()}
                        />
                        <SettingItem
                          label={zhSettings.sections.clash.form.fields.dnsOverwrite}
                          onClick={openDnsSettings}
                        />
                        <SettingItem
                          label={zhSettings.sections.clash.form.fields.updateGeoData}
                          onClick={() =>
                            runMihomoMaintenance(
                              api.updateGeo,
                              'settings.feedback.notifications.clash.geoDataUpdated',
                            )
                          }
                        />
                      </SettingList>
                    </Box>
                  </Grid>

                  <Grid size={6}>
                    <Box className="verge-setting-panel">
                      <SettingList title={zhHome.components.systemInfo.badges.serviceMode}>
                        <SettingItem label="KernelSU">
                          {root?.granted ? (
                            <CheckBoxRounded color="success" />
                          ) : (
                            <CheckBoxOutlineBlankRounded color="disabled" />
                          )}
                        </SettingItem>
                        <SettingItem
                          label={
                            runtime?.agentConnected
                              ? zhLayout.components.serviceMigration.reinstall
                              : zhSettings.sections.proxyControl.actions.installService
                          }
                          onClick={bootstrapRuntime}
                        />
                      </SettingList>
                    </Box>
                  </Grid>

                  <Grid size={6}>
                    <Box className="verge-setting-panel">
                      <SettingList title={zhSettings.sections.clash.form.fields.clashCore}>
                        <SettingItem
                          label={zhSettings.components.verge.advanced.fields.checkUpdates}
                          onClick={upgradeCore}
                        />
                        <SettingItem
                          label={zhShared.actions.restart}
                          onClick={restartCore}
                        />
                      </SettingList>
                    </Box>
                  </Grid>

                  <Grid size={6}>
                    <Box className="verge-setting-panel">
                      <SettingList title={zhSettings.modals.backup.title}>
                        <SettingItem
                          label={zhSettings.components.verge.advanced.fields.backupSetting}
                          onClick={openLocalBackups}
                        />
                      </SettingList>
                    </Box>
                  </Grid>

                  <Grid size={6}>
                    <Box className="verge-setting-panel">
                      <SettingList title={zhSettings.components.verge.advanced.title}>
                        <SettingItem label={zhSettings.components.verge.advanced.fields.vergeVersion}>
                          <Typography variant="body2">0.1.0-alpha.1</Typography>
                        </SettingItem>
                        <SettingItem
                          label={zhSettings.components.verge.advanced.fields.exportDiagnostics}
                          onClick={() => void openDiagnostics()}
                        />
                      </SettingList>
                    </Box>
                  </Grid>
                </Grid>
              )}
            </Box>
          </section>
        </div>

        <NoticeManager centerYRatio={0.618} />

        <Drawer
          anchor="left"
          open={navOpen}
          onClose={() => setNavOpen(false)}
          slotProps={{
            paper: {
              sx: {
                width: 220,
                pt: 'env(safe-area-inset-top)',
                bgcolor: 'background.paper',
              },
            },
          }}
        >
          <List
            disablePadding
            sx={{ px: 1, py: 1 }}
            aria-label={zhLayout.components.navigation.menu.expandNavBar}
          >
            {navPages.map((item) => (
              <ListItemButton
                key={item}
                selected={page === item}
                onClick={() => {
                  setPage(item)
                  setNavOpen(false)
                }}
                sx={[
                  {
                    minHeight: 48,
                    borderRadius: 1.5,
                    px: 1.5,
                    '& .MuiListItemText-primary': {
                      color: 'text.primary',
                      fontWeight: page === item ? 700 : 500,
                    },
                  },
                  ({ palette: { mode, primary } }) => {
                    const bgcolor =
                      mode === 'light'
                        ? alpha(primary.main, 0.15)
                        : alpha(primary.main, 0.35)
                    return {
                      '&.Mui-selected': { bgcolor },
                      '&.Mui-selected:hover': { bgcolor },
                    }
                  },
                ]}
              >
                <ListItemIcon sx={{ minWidth: 40, color: 'text.primary' }}>
                  {navIcons[item]}
                </ListItemIcon>
                <ListItemText primary={navLabels[item]} />
              </ListItemButton>
            ))}
          </List>
        </Drawer>

        <ConnectionDetail
          ref={connectionDetailRef}
          onCloseConnection={closeConnection}
        />

        <Dialog
          open={runtimeConfigOpen}
          fullWidth
          maxWidth="md"
          onClose={() => setRuntimeConfigOpen(false)}
          slotProps={{ paper: { sx: { borderRadius: 2 } } }}
        >
          <DialogTitle>
            <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
              {zhProfiles.page.actions.viewRuntimeConfig}
              <Chip label={zhShared.labels.readOnly} size="small" />
            </Box>
          </DialogTitle>
          <DialogContent dividers>
            <TextField
              fullWidth
              multiline
              minRows={16}
              maxRows={26}
              value={
                runtimeConfigLoading ? `${zhShared.statuses.loading}\n` : runtimeConfigText
              }
              slotProps={{ input: { readOnly: true } }}
              sx={{
                '& textarea': {
                  fontFamily: 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace',
                  fontSize: 12,
                  lineHeight: 1.45,
                },
              }}
            />
          </DialogContent>
          <DialogActions>
            <Button onClick={() => setRuntimeConfigOpen(false)} variant="outlined">
              {zhShared.actions.close}
            </Button>
          </DialogActions>
        </Dialog>

        <Dialog
          open={chainDialogOpen}
          fullWidth
          maxWidth="sm"
          onClose={() => {
            if (!busy) setChainDialogOpen(false)
          }}
          slotProps={{ paper: { sx: { borderRadius: 2 } } }}
        >
          <DialogTitle>{zhProxies.page.actions.toggleChain}</DialogTitle>
          <DialogContent dividers>
            <Stack spacing={2} sx={{ pt: 0.5 }}>
              <Alert severity="warning">
                {zhProxies.page.chain.warning}
              </Alert>
              <SettingList title={zhProxies.page.chain.header}>
                <SettingItem label={zhProxies.page.title.default}>
                  <BaseStyledSelect
                    value={chainTarget}
                    disabled={busy || Boolean(active?.proxyChain)}
                    onChange={(event) => {
                      setChainTarget(event.target.value)
                      setChainNodes([])
                      setChainCandidate('')
                    }}
                    sx={{ minWidth: 150 }}
                  >
                    {(core?.groups ?? [])
                      .filter((group) => group.selectable)
                      .map((group) => (
                        <MenuItem key={group.name} value={group.name}>
                          {group.name}
                        </MenuItem>
                      ))}
                  </BaseStyledSelect>
                </SettingItem>
              </SettingList>

              <Box>
                <Typography sx={{ fontSize: 16, fontWeight: 700, mb: 1 }}>
                  {zhProxies.page.chain.header}
                </Typography>
                {chainNodes.length === 0 ? (
                  <MobileEmpty text={zhProxies.page.chain.instruction} />
                ) : (
                  <Stack spacing={0.5}>
                    {chainNodes.map((node, index) => {
                      const item: ProxyChainItem = {
                        id: `${node}-${index}`,
                        name: node,
                        type:
                          summary?.nodes.find((candidate) => candidate.name === node)
                            ?.protocol ?? '',
                        recordId: node,
                        delay: delays[node] ?? undefined,
                      }
                      return (
                        <Stack
                          key={item.id}
                          direction="row"
                          spacing={0.5}
                          sx={{ alignItems: 'center' }}
                        >
                          <Box sx={{ minWidth: 0, flex: 1 }}>
                            <ProxyChainCard
                              proxy={item}
                              index={index}
                              isFirst={index === 0}
                              isLast={index === chainNodes.length - 1}
                              onRemove={
                                active?.proxyChain
                                  ? undefined
                                  : () =>
                                      setChainNodes((old) =>
                                        old.filter((_, i) => i !== index),
                                      )
                              }
                            />
                          </Box>
                          <Stack>
                            <IconButton
                              size="small"
                              disabled={index === 0 || busy || Boolean(active?.proxyChain)}
                              onClick={() => moveChainNode(index, -1)}
                            >
                              <ArrowUpwardRounded fontSize="small" />
                            </IconButton>
                            <IconButton
                              size="small"
                              disabled={
                                index === chainNodes.length - 1 ||
                                busy ||
                                Boolean(active?.proxyChain)
                              }
                              onClick={() => moveChainNode(index, 1)}
                            >
                              <ArrowDownwardRounded fontSize="small" />
                            </IconButton>
                          </Stack>
                        </Stack>
                      )
                    })}
                  </Stack>
                )}
              </Box>

              {!active?.proxyChain && (
                <Stack direction="row" spacing={1}>
                  <BaseStyledSelect
                    value={chainCandidate}
                    onChange={(event) => setChainCandidate(event.target.value)}
                    displayEmpty
                    sx={{ flex: 1 }}
                  >
                    <MenuItem value="" disabled>
                      {zhHome.components.currentProxy.labels.proxy}
                    </MenuItem>
                    {chainCandidateNames(summary, core, chainTarget)
                      .filter((node) => !chainNodes.includes(node))
                      .map((node) => (
                        <MenuItem key={node} value={node}>
                          {node}
                        </MenuItem>
                      ))}
                  </BaseStyledSelect>
                  <Button
                    variant="outlined"
                    disabled={!chainCandidate || busy}
                    startIcon={<AddRounded />}
                    onClick={() => {
                      if (!chainCandidate || chainNodes.includes(chainCandidate)) return
                      setChainNodes((old) => [...old, chainCandidate])
                      setChainCandidate('')
                    }}
                  >
                    {zhSettings.sections.externalCors.actions.add}
                  </Button>
                </Stack>
              )}
            </Stack>
          </DialogContent>
          <DialogActions>
            {active?.proxyChain ? (
              <Button
                color="error"
                variant="contained"
                disabled={busy}
                startIcon={<LanOutlined />}
                onClick={() => void clearProxyChain()}
              >
                {zhProxies.page.actions.disconnect}
              </Button>
            ) : (
              <Button
                color="success"
                variant="contained"
                disabled={busy || !chainTarget || chainNodes.length < 2}
                startIcon={<LanRounded />}
                onClick={() => void saveProxyChain()}
              >
                {busy
                  ? zhProxies.page.actions.connecting
                  : zhProxies.page.actions.connect}
              </Button>
            )}
            <Button disabled={busy} onClick={() => setChainDialogOpen(false)}>
              {zhShared.actions.close}
            </Button>
          </DialogActions>
        </Dialog>

        <Dialog
          open={dnsDialogOpen}
          fullWidth
          maxWidth="sm"
          onClose={() => {
            if (!busy && !dnsLoading) setDnsDialogOpen(false)
          }}
          slotProps={{ paper: { sx: { borderRadius: 2 } } }}
        >
          <DialogTitle>{zhSettings.modals.dns.dialog.title}</DialogTitle>
          <DialogContent dividers>
            <Stack spacing={1.5}>
              <Alert severity="info">{zhSettings.modals.dns.dialog.warning}</Alert>
              {dnsLoading || !dnsDraft ? (
                <Typography color="text.secondary">{zhShared.statuses.loading}</Typography>
              ) : (
                <>
                  <SettingList title={zhSettings.modals.dns.sections.general}>
                    <SettingItem label={zhSettings.modals.dns.fields.enable}>
                      <Switch
                        checked={dnsDraft.enabled}
                        disabled={busy}
                        onChange={(_, enabled) =>
                          setDnsDraft((old) => (old ? { ...old, enabled } : old))
                        }
                      />
                    </SettingItem>
                  </SettingList>
                  <TextField
                    label="YAML"
                    multiline
                    minRows={14}
                    fullWidth
                    disabled={busy}
                    value={dnsDraft.yaml}
                    onChange={(event) =>
                      setDnsDraft((old) =>
                        old ? { ...old, yaml: event.target.value } : old,
                      )
                    }
                    placeholder={`dns:\n  enable: true\n  enhanced-mode: fake-ip\n  nameserver:\n    - https://dns.alidns.com/dns-query\nhosts: {}`}
                    slotProps={{ htmlInput: { spellCheck: false } }}
                    size="small"
                  />
                </>
              )}
            </Stack>
          </DialogContent>
          <DialogActions>
            <Button
              disabled={busy || dnsLoading}
              onClick={() => setDnsDialogOpen(false)}
            >
              {zhShared.actions.cancel}
            </Button>
            <Button
              variant="contained"
              disabled={
                busy ||
                dnsLoading ||
                !dnsDraft ||
                (dnsDraft.enabled && !dnsDraft.yaml.trim())
              }
              onClick={() => void saveDnsSettings()}
            >
              {busy ? zhShared.statuses.saving : zhShared.actions.save}
            </Button>
          </DialogActions>
        </Dialog>

        <Dialog
          open={externalControllerDialogOpen}
          fullWidth
          maxWidth="sm"
          onClose={() => {
            if (!busy && !externalControllerLoading) {
              setExternalControllerDialogOpen(false)
            }
          }}
          slotProps={{ paper: { sx: { borderRadius: 2 } } }}
        >
          <DialogTitle>{zhSettings.sections.externalController.title}</DialogTitle>
          <DialogContent dividers>
            {externalControllerLoading || !externalControllerDraft ? (
              <Typography color="text.secondary">{zhShared.statuses.loading}</Typography>
            ) : (
              <Stack spacing={1.5}>
                <SettingList title={zhSettings.sections.externalController.title}>
                  <SettingItem label={zhSettings.sections.externalController.fields.enable}>
                    <Switch
                      checked={externalControllerDraft.enabled}
                      disabled={busy}
                      onChange={(_, enabled) =>
                        setExternalControllerDraft((old) =>
                          old ? { ...old, enabled } : old,
                        )
                      }
                    />
                  </SettingItem>
                  <SettingItem label={zhSettings.sections.externalController.fields.address}>
                    <TextField
                      size="small"
                      value={externalControllerDraft.address}
                      disabled={busy || !externalControllerDraft.enabled}
                      onChange={(event) =>
                        setExternalControllerDraft((old) =>
                          old ? { ...old, address: event.target.value } : old,
                        )
                      }
                      sx={{ width: 190 }}
                    />
                  </SettingItem>
                  <SettingItem label={zhSettings.sections.externalController.fields.secret}>
                    <TextField
                      size="small"
                      type="password"
                      autoComplete="new-password"
                      value={externalControllerDraft.secret}
                      disabled={busy || !externalControllerDraft.enabled}
                      onChange={(event) =>
                        setExternalControllerDraft((old) =>
                          old ? { ...old, secret: event.target.value } : old,
                        )
                      }
                      sx={{ width: 190 }}
                    />
                  </SettingItem>
                  <SettingItem label={zhSettings.sections.externalCors.fields.allowPrivateNetwork}>
                    <Switch
                      checked={externalControllerDraft.allowPrivateNetwork}
                      disabled={busy || !externalControllerDraft.enabled}
                      onChange={(_, allowPrivateNetwork) =>
                        setExternalControllerDraft((old) =>
                          old ? { ...old, allowPrivateNetwork } : old,
                        )
                      }
                    />
                  </SettingItem>
                </SettingList>
                <TextField
                  label={zhSettings.sections.externalCors.fields.allowedOrigins}
                  multiline
                  minRows={4}
                  fullWidth
                  disabled={busy || !externalControllerDraft.enabled}
                  value={externalControllerDraft.allowOrigins.join('\n')}
                  onChange={(event) =>
                    setExternalControllerDraft((old) =>
                      old
                        ? {
                            ...old,
                            allowOrigins: event.target.value.split(/\r?\n/),
                          }
                        : old,
                    )
                  }
                  size="small"
                />
              </Stack>
            )}
          </DialogContent>
          <DialogActions>
            <Button
              disabled={busy || externalControllerLoading}
              onClick={() => setExternalControllerDialogOpen(false)}
            >
              {zhShared.actions.cancel}
            </Button>
            <Button
              variant="contained"
              disabled={
                busy ||
                externalControllerLoading ||
                !externalControllerDraft ||
                (externalControllerDraft.enabled &&
                  (!externalControllerDraft.address.trim() ||
                    !externalControllerDraft.secret.trim()))
              }
              onClick={() => void saveExternalControllerSettings()}
            >
              {busy ? zhShared.statuses.saving : zhShared.actions.save}
            </Button>
          </DialogActions>
        </Dialog>

        <Dialog
          open={networkDialogOpen}
          fullWidth
          maxWidth="sm"
          onClose={() => {
            if (!networkLoading) setNetworkDialogOpen(false)
          }}
          slotProps={{ paper: { sx: { borderRadius: 2 } } }}
        >
          <DialogTitle>{zhSettings.modals.networkInterface.title}</DialogTitle>
          <DialogContent dividers>
            <NetworkInterfaceContent
              networkInterfaces={networkInterfaces}
              loading={networkLoading}
              isV4={networkIsV4}
              onToggleIpVersion={() => setNetworkIsV4((old) => !old)}
              onCopy={async (content) => {
                await navigator.clipboard.writeText(content)
              }}
            />
          </DialogContent>
          <DialogActions>
            <Button onClick={() => setNetworkDialogOpen(false)}>
              {zhShared.actions.close}
            </Button>
          </DialogActions>
        </Dialog>

        <Dialog
          open={diagnosticDialogOpen}
          fullWidth
          maxWidth="md"
          onClose={() => {
            if (!diagnosticLoading) setDiagnosticDialogOpen(false)
          }}
          slotProps={{ paper: { sx: { borderRadius: 2 } } }}
        >
          <DialogTitle>{zhSettings.components.verge.advanced.fields.exportDiagnostics}</DialogTitle>
          <DialogContent dividers>
            {diagnosticLoading ? (
              <Typography color="text.secondary">{zhShared.statuses.loading}</Typography>
            ) : (
              <TextField
                fullWidth
                multiline
                minRows={16}
                maxRows={26}
                value={diagnosticText}
                slotProps={{ input: { readOnly: true } }}
                sx={{ '& textarea': { fontFamily: 'monospace', fontSize: 12 } }}
              />
            )}
          </DialogContent>
          <DialogActions>
            <Button
              disabled={diagnosticLoading || !diagnosticText}
              onClick={async () => {
                await navigator.clipboard.writeText(diagnosticText)
              }}
            >
              {zhSettings.sections.externalController.tooltips.copy}
            </Button>
            <Button
              disabled={diagnosticLoading}
              onClick={() => setDiagnosticDialogOpen(false)}
            >
              {zhShared.actions.close}
            </Button>
          </DialogActions>
        </Dialog>

        <Dialog
          open={portDialogOpen}
          fullWidth
          maxWidth="sm"
          onClose={() => {
            if (!busy && !portLoading) setPortDialogOpen(false)
          }}
          slotProps={{ paper: { sx: { borderRadius: 2 } } }}
        >
          <DialogTitle>{zhSettings.modals.clashPort.title}</DialogTitle>
          <DialogContent dividers>
            {portLoading || !portDraft ? (
              <Typography color="text.secondary">{zhShared.statuses.loading}</Typography>
            ) : (
              <Stack spacing={1.5}>
                <SettingList title={zhSettings.sections.clash.form.fields.portConfig}>
                  <SettingItem
                    label={zhSettings.sections.clash.form.fields.tunnels.localAddr}
                  >
                    <TextField
                      size="small"
                      value={portDraft.bindAddress}
                      disabled={busy}
                      onChange={(event) =>
                        setPortDraft((old) =>
                          old
                            ? { ...old, bindAddress: event.target.value }
                            : old,
                        )
                      }
                      sx={{ width: 180 }}
                    />
                  </SettingItem>
                  {portFields.map((field) => {
                    const value = portDraft[field.key]
                    const enabled = value > 0
                    return (
                      <SettingItem key={field.key} label={field.label}>
                        <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
                          <TextField
                            size="small"
                            type="number"
                            disabled={busy || !enabled}
                            value={enabled ? value : field.fallback}
                            onChange={(event) => {
                              const next = Math.max(
                                1,
                                Math.min(
                                  65535,
                                  Number.parseInt(event.target.value, 10) || field.fallback,
                                ),
                              )
                              setPortDraft((old) =>
                                old ? { ...old, [field.key]: next } : old,
                              )
                            }}
                            slotProps={{ htmlInput: { min: 1, max: 65535 } }}
                            sx={{ width: 100 }}
                          />
                          <Switch
                            checked={enabled}
                            disabled={busy}
                            onChange={(_, checked) =>
                              setPortDraft((old) =>
                                old
                                  ? {
                                      ...old,
                                      [field.key]: checked ? field.fallback : 0,
                                    }
                                  : old,
                              )
                            }
                          />
                        </Stack>
                      </SettingItem>
                    )
                  })}
                </SettingList>
              </Stack>
            )}
          </DialogContent>
          <DialogActions>
            <Button
              disabled={busy || portLoading}
              onClick={() => setPortDialogOpen(false)}
            >
              {zhShared.actions.cancel}
            </Button>
            <Button
              variant="contained"
              disabled={busy || portLoading || !portDraft}
              onClick={() => void savePortSettings()}
            >
              {busy ? zhShared.statuses.saving : zhShared.actions.save}
            </Button>
          </DialogActions>
        </Dialog>

        <Dialog
          open={tunDialogOpen}
          fullWidth
          maxWidth="sm"
          onClose={() => {
            if (!busy) setTunDialogOpen(false)
          }}
          slotProps={{ paper: { sx: { borderRadius: 2 } } }}
        >
          <DialogTitle>{zhSettings.modals.tun.title}</DialogTitle>
          <DialogContent dividers>
            {tunDraft && (
              <SettingList title={zhSettings.modals.tun.title}>
                <SettingItem label={zhSettings.modals.tun.fields.stack}>
                  <StackModeSwitch
                    value={tunDraft.stack}
                    onChange={(stack) =>
                      setTunDraft((old) => (old ? { ...old, stack } : old))
                    }
                  />
                </SettingItem>
                <SettingItem label={zhSettings.modals.tun.fields.strictRoute}>
                  <Switch
                    edge="end"
                    checked={tunDraft.strictRoute}
                    onChange={(_, strictRoute) =>
                      setTunDraft((old) =>
                        old ? { ...old, strictRoute } : old,
                      )
                    }
                  />
                </SettingItem>
                <SettingItem label={zhSettings.modals.tun.fields.autoDetectInterface}>
                  <Switch
                    edge="end"
                    checked={tunDraft.autoDetectInterface}
                    onChange={(_, autoDetectInterface) =>
                      setTunDraft((old) =>
                        old ? { ...old, autoDetectInterface } : old,
                      )
                    }
                  />
                </SettingItem>
                <SettingItem label={zhSettings.modals.tun.fields.dnsHijack}>
                  <TextField
                    autoComplete="new-password"
                    size="small"
                    autoCorrect="off"
                    autoCapitalize="off"
                    spellCheck="false"
                    value={tunDraft.dnsHijack.join(',')}
                    placeholder="any:53,tcp://any:53"
                    onChange={(event) =>
                      setTunDraft((old) =>
                        old
                          ? {
                              ...old,
                              dnsHijack: event.target.value.split(','),
                            }
                          : old,
                      )
                    }
                    sx={{ width: 250, maxWidth: '55vw' }}
                  />
                </SettingItem>
                <SettingItem label={zhSettings.modals.tun.fields.mtu}>
                  <TextField
                    autoComplete="new-password"
                    size="small"
                    type="number"
                    value={tunDraft.mtu}
                    onChange={(event) =>
                      setTunDraft((old) =>
                        old
                          ? {
                              ...old,
                              mtu: Number.parseInt(event.target.value, 10) || 0,
                            }
                          : old,
                      )
                    }
                    sx={{ width: 120 }}
                  />
                </SettingItem>
              </SettingList>
            )}
          </DialogContent>
          <DialogActions>
            <Button disabled={busy} onClick={() => setTunDialogOpen(false)}>
              {zhShared.actions.cancel}
            </Button>
            <Button
              variant="contained"
              disabled={busy || !tunDraft}
              onClick={() => void saveTunSettings()}
            >
              {busy ? zhShared.statuses.saving : zhShared.actions.save}
            </Button>
          </DialogActions>
        </Dialog>

        <Dialog
          open={backupDialogOpen}
          fullWidth
          maxWidth="sm"
          onClose={() => {
            if (!busy && !backupLoading && !webdavLoading) {
              setBackupDialogOpen(false)
            }
          }}
          slotProps={{ paper: { sx: { borderRadius: 2 } } }}
        >
          <DialogTitle>{zhSettings.modals.backup.title}</DialogTitle>
          <DialogContent dividers>
            <Stack spacing={1.5}>
              {backupSettingsDraft && (
                <>
                  <SettingList title={zhSettings.modals.backup.auto.title}>
                    <SettingItem
                      label={zhSettings.modals.backup.auto.scheduleLabel}
                      secondary={zhSettings.modals.backup.auto.scheduleHelper}
                    >
                      <Switch
                        checked={backupSettingsDraft.autoScheduleEnabled}
                        disabled={busy}
                        onChange={(_, autoScheduleEnabled) =>
                          setBackupSettingsDraft((old) =>
                            old ? { ...old, autoScheduleEnabled } : old,
                          )
                        }
                      />
                    </SettingItem>
                    <SettingItem label={zhSettings.modals.backup.auto.intervalLabel}>
                      <TextField
                        size="small"
                        type="number"
                        value={backupSettingsDraft.autoIntervalHours}
                        disabled={busy || !backupSettingsDraft.autoScheduleEnabled}
                        slotProps={{ htmlInput: { min: 1, max: 168 } }}
                        onChange={(event) =>
                          setBackupSettingsDraft((old) =>
                            old
                              ? {
                                  ...old,
                                  autoIntervalHours: Math.max(
                                    1,
                                    Math.min(
                                      168,
                                      Number.parseInt(event.target.value, 10) || 1,
                                    ),
                                  ),
                                }
                              : old,
                          )
                        }
                        sx={{ width: 90 }}
                      />
                    </SettingItem>
                    <SettingItem
                      label={zhSettings.modals.backup.auto.changeLabel}
                      secondary={zhSettings.modals.backup.auto.changeHelper}
                    >
                      <Switch
                        checked={backupSettingsDraft.autoOnChange}
                        disabled={busy}
                        onChange={(_, autoOnChange) =>
                          setBackupSettingsDraft((old) =>
                            old ? { ...old, autoOnChange } : old,
                          )
                        }
                      />
                    </SettingItem>
                  </SettingList>

                  <SettingList title={zhSettings.modals.backup.webdav.title}>
                    <SettingItem label={zhSettings.modals.backup.fields.webdavUrl}>
                      <TextField
                        size="small"
                        value={backupSettingsDraft.webdavUrl}
                        disabled={busy}
                        onChange={(event) =>
                          setBackupSettingsDraft((old) =>
                            old ? { ...old, webdavUrl: event.target.value } : old,
                          )
                        }
                        sx={{ width: 260, maxWidth: '52vw' }}
                      />
                    </SettingItem>
                    <SettingItem label={zhSettings.modals.backup.fields.username}>
                      <TextField
                        size="small"
                        value={backupSettingsDraft.webdavUsername}
                        disabled={busy}
                        onChange={(event) =>
                          setBackupSettingsDraft((old) =>
                            old
                              ? { ...old, webdavUsername: event.target.value }
                              : old,
                          )
                        }
                        sx={{ width: 180 }}
                      />
                    </SettingItem>
                    <SettingItem label={zhShared.labels.password}>
                      <TextField
                        size="small"
                        type="password"
                        autoComplete="new-password"
                        value={backupSettingsDraft.webdavPassword}
                        disabled={busy}
                        onChange={(event) =>
                          setBackupSettingsDraft((old) =>
                            old
                              ? { ...old, webdavPassword: event.target.value }
                              : old,
                          )
                        }
                        sx={{ width: 180 }}
                      />
                    </SettingItem>
                    <SettingItem label={zhProfiles.modals.profileForm.fields.acceptInvalidCerts}>
                      <Switch
                        checked={backupSettingsDraft.webdavAcceptInvalidCerts}
                        disabled={busy}
                        onChange={(_, webdavAcceptInvalidCerts) =>
                          setBackupSettingsDraft((old) =>
                            old ? { ...old, webdavAcceptInvalidCerts } : old,
                          )
                        }
                      />
                    </SettingItem>
                  </SettingList>
                  <Stack direction="row" spacing={1} sx={{ justifyContent: 'flex-end' }}>
                    <Button
                      size="small"
                      variant="outlined"
                      disabled={busy}
                      onClick={() => void saveBackupSettings()}
                    >
                      {zhShared.actions.save}
                    </Button>
                    <Button
                      size="small"
                      variant="outlined"
                      disabled={
                        busy ||
                        !backupSettingsDraft.webdavUrl.trim() ||
                        !backupSettingsDraft.webdavUsername.trim() ||
                        !backupSettingsDraft.webdavPassword
                      }
                      onClick={() => void testWebDav()}
                    >
                      {zhShared.actions.check}
                    </Button>
                    <Button
                      size="small"
                      variant="contained"
                      disabled={
                        busy ||
                        webdavLoading ||
                        !backupSettingsDraft.webdavUrl.trim()
                      }
                      onClick={() => void createWebDavBackup()}
                    >
                      {zhSettings.modals.backup.actions.backup}
                    </Button>
                  </Stack>
                  {backupSettingsDraft.webdavUrl && (
                    webdavLoading ? (
                      <Typography color="text.secondary">{zhShared.statuses.loading}</Typography>
                    ) : webdavBackups.length === 0 ? (
                      <MobileEmpty text={zhSettings.modals.backup.history.empty} />
                    ) : (
                      <SettingList title={zhSettings.modals.backup.history.title}>
                        {webdavBackups.map((backup) => (
                          <Box key={backup.filename}>
                            <SettingItem
                              label={new Date(backup.lastModified).toLocaleString()}
                              secondary={`${formatBytes(backup.contentLength)} · ${backup.filename}`}
                            >
                              <Stack direction="row" spacing={0.75}>
                                <Button
                                  size="small"
                                  variant="outlined"
                                  disabled={busy || webdavLoading}
                                  onClick={() =>
                                    void restoreWebDavBackup(backup.filename)
                                  }
                                >
                                  {zhSettings.modals.backup.actions.restore}
                                </Button>
                                <IconButton
                                  size="small"
                                  color="error"
                                  disabled={busy || webdavLoading}
                                  aria-label={zhSettings.modals.backup.actions.deleteBackup}
                                  onClick={() =>
                                    void deleteWebDavBackup(backup.filename)
                                  }
                                >
                                  <DeleteOutlineRounded fontSize="small" />
                                </IconButton>
                              </Stack>
                            </SettingItem>
                          </Box>
                        ))}
                      </SettingList>
                    )
                  )}
                  <Divider />
                </>
              )}
              {backupLoading ? (
                <Typography color="text.secondary">{zhShared.statuses.loading}</Typography>
              ) : backups.length === 0 ? (
                <MobileEmpty text={zhSettings.modals.backup.history.empty} />
              ) : (
                <SettingList title={zhSettings.modals.backup.history.title}>
                  {backups.map((backup) => (
                    <Box key={backup.filename}>
                      <SettingItem
                        label={new Date(backup.createdAt * 1000).toLocaleString()}
                        secondary={backup.filename}
                      >
                        <Stack direction="row" spacing={0.75}>
                          <Button
                            size="small"
                            variant="outlined"
                            disabled={busy || backupLoading}
                            onClick={() => void restoreLocalBackup(backup.filename)}
                          >
                            {zhSettings.modals.backup.actions.restore}
                          </Button>
                          <Button
                            size="small"
                            variant="outlined"
                            disabled={busy || backupLoading}
                            onClick={() => void exportLocalBackup(backup.filename)}
                          >
                            {zhSettings.modals.backup.actions.export}
                          </Button>
                          <IconButton
                            size="small"
                            color="error"
                            disabled={busy || backupLoading}
                            aria-label={zhSettings.modals.backup.actions.deleteBackup}
                            onClick={() => void deleteLocalBackup(backup.filename)}
                          >
                            <DeleteOutlineRounded fontSize="small" />
                          </IconButton>
                        </Stack>
                      </SettingItem>
                    </Box>
                  ))}
                </SettingList>
              )}
            </Stack>
          </DialogContent>
          <DialogActions>
            <Button
              disabled={busy || backupLoading || webdavLoading}
              onClick={() => void importLocalBackup()}
            >
              {zhSettings.modals.backup.actions.importBackup}
            </Button>
            <Button
              disabled={busy || backupLoading || webdavLoading}
              onClick={() => setBackupDialogOpen(false)}
            >
              {zhShared.actions.close}
            </Button>
            <Button
              variant="contained"
              disabled={busy || backupLoading}
              onClick={() => void createLocalBackup()}
            >
              {zhSettings.modals.backup.actions.backup}
            </Button>
          </DialogActions>
        </Dialog>

        <Dialog
          open={sequenceTarget !== null}
          fullWidth
          maxWidth="sm"
          onClose={() => {
            if (!busy) setSequenceTarget(null)
          }}
          slotProps={{ paper: { sx: { borderRadius: 2 } } }}
        >
          <DialogTitle>
            {sequenceTarget
              ? `${sequenceTarget.profile.name} · ${
                  sequenceTarget.kind === 'rules'
                    ? zhProfiles.components.menu.editRules
                    : sequenceTarget.kind === 'proxies'
                      ? zhProfiles.components.menu.editProxies
                      : zhProfiles.components.menu.editGroups
                }`
              : ''}
          </DialogTitle>
          <DialogContent dividers>
            <Stack spacing={1.5}>
              <TextField
                label="YAML"
                multiline
                minRows={16}
                fullWidth
                disabled={busy}
                value={sequenceDraft}
                onChange={(event) => setSequenceDraft(event.target.value)}
                placeholder={
                  sequenceTarget?.kind === 'rules'
                    ? `prepend:\n  - DOMAIN-SUFFIX,example.com,DIRECT\nappend: []\ndelete:\n  - MATCH,DIRECT`
                    : sequenceTarget?.kind === 'proxies'
                      ? `prepend:\n  - name: extra-node\n    type: direct\nappend: []\ndelete: []`
                      : `prepend:\n  - name: extra-group\n    type: select\n    proxies:\n      - DIRECT\nappend: []\ndelete: []`
                }
                slotProps={{ htmlInput: { spellCheck: false } }}
                size="small"
              />
            </Stack>
          </DialogContent>
          <DialogActions>
            <Button disabled={busy} onClick={() => setSequenceTarget(null)}>
              {zhShared.actions.cancel}
            </Button>
            <Button
              variant="contained"
              disabled={busy}
              onClick={() => void saveSequenceEnhancement()}
            >
              {zhShared.actions.save}
            </Button>
          </DialogActions>
        </Dialog>

        <Dialog
          open={scriptTarget !== null}
          fullWidth
          maxWidth="sm"
          onClose={() => {
            if (!busy) setScriptTarget(null)
          }}
          slotProps={{ paper: { sx: { borderRadius: 2 } } }}
        >
          <DialogTitle>
            {scriptTarget?.kind === 'global'
              ? zhProfiles.components.more.global.script
              : `${scriptTarget?.profile.name ?? ''} · ${zhProfiles.components.menu.extendScript}`}
          </DialogTitle>
          <DialogContent dividers>
            <Stack spacing={1.5}>
              <Alert severity="info">
                {scriptTarget?.kind === 'global'
                  ? zhProfiles.modals.editor.enhance.globalScript
                  : zhProfiles.modals.editor.enhance.profileScript}
              </Alert>
              <TextField
                label="JavaScript"
                multiline
                minRows={18}
                fullWidth
                disabled={busy}
                value={scriptDraft}
                onChange={(event) => setScriptDraft(event.target.value)}
                slotProps={{ htmlInput: { spellCheck: false } }}
                size="small"
              />
            </Stack>
          </DialogContent>
          <DialogActions>
            <Button disabled={busy} onClick={() => setScriptTarget(null)}>
              {zhShared.actions.cancel}
            </Button>
            <Button variant="contained" disabled={busy} onClick={() => void saveScript()}>
              {zhShared.actions.save}
            </Button>
          </DialogActions>
        </Dialog>

        <Dialog
          open={mergeTarget !== null}
          fullWidth
          maxWidth="sm"
          onClose={() => {
            if (!busy) setMergeTarget(null)
          }}
          slotProps={{ paper: { sx: { borderRadius: 2 } } }}
        >
          <DialogTitle>
            {mergeTarget?.kind === 'global'
              ? zhProfiles.components.more.global.merge
              : `${mergeTarget?.profile.name ?? ''} · ${zhProfiles.components.menu.extendConfig}`}
          </DialogTitle>
          <DialogContent dividers>
            <Stack spacing={1.5}>
              <Alert severity="info">
                {mergeTarget?.kind === 'global'
                  ? zhProfiles.modals.editor.enhance.globalMerge
                  : zhProfiles.modals.editor.enhance.profileMerge}
              </Alert>
              <TextField
                label="YAML"
                multiline
                minRows={16}
                fullWidth
                disabled={busy}
                value={mergeDraft}
                onChange={(event) => setMergeDraft(event.target.value)}
                placeholder={`rules:\n  - DOMAIN-SUFFIX,example.com,DIRECT\ndns:\n  nameserver:\n    - 1.1.1.1`}
                slotProps={{ htmlInput: { spellCheck: false } }}
                size="small"
              />
            </Stack>
          </DialogContent>
          <DialogActions>
            <Button disabled={busy} onClick={() => setMergeTarget(null)}>
              {zhShared.actions.cancel}
            </Button>
            <Button variant="contained" disabled={busy} onClick={() => void saveMerge()}>
              {zhShared.actions.save}
            </Button>
          </DialogActions>
        </Dialog>

        <Dialog
          open={form !== null}
          fullWidth
          maxWidth="sm"
          onClose={() => {
            if (!busy) setForm(null)
          }}
          slotProps={{ paper: { sx: { borderRadius: 2 } } }}
        >
          <Box component="form" onSubmit={submit}>
            <DialogTitle>
              {form?.kind === 'subscription'
                ? zhProfiles.modals.profileForm.title.create
                : form?.kind === 'nodes'
                  ? zhProfiles.page.actions.import
                  : form?.kind === 'delete'
                    ? zhProfiles.modals.confirmDelete.title
                    : form?.kind === 'edit'
                      ? zhProfiles.modals.profileForm.title.edit
                    : form?.kind === 'copy'
                      ? zhProfiles.modals.profileForm.title.create
                      : zhProfiles.page.actions.import}
            </DialogTitle>
            <DialogContent dividers>
              {form?.kind === 'delete' ? (
                <Typography>{zhProfiles.modals.confirmDelete.message}</Typography>
              ) : (
                <Stack spacing={2} sx={{ pt: 0.5 }}>
                  <TextField
                    name="name"
                    label={zhShared.labels.name}
                    autoComplete="off"
                    required
                    value={name}
                    disabled={busy}
                    onChange={(event) => setName(event.target.value)}
                    fullWidth
                    size="small"
                  />
                  {(form?.kind === 'subscription' ||
                    (form?.kind === 'edit' && Boolean(form.profile.source))) && (
                    <TextField
                      name="url"
                      label={zhProfiles.modals.profileForm.fields.subscriptionUrl}
                      type="url"
                      autoComplete="off"
                      required
                      disabled={busy}
                      value={sourceUrl}
                      onChange={(event) => setSourceUrl(event.target.value)}
                      fullWidth
                      size="small"
                    />
                  )}
                  {form?.kind !== 'subscription' && (
                    <>
                      {(
                        form?.kind === 'yaml' ||
                        form?.kind === 'copy' ||
                        form?.kind === 'edit'
                      ) && (
                        <Button
                          component="label"
                          variant="outlined"
                          size="small"
                          disabled={busy}
                          sx={{ alignSelf: 'flex-start' }}
                        >
                          {zhProfiles.components.fileInput.chooseFile}
                          <input
                            hidden
                            type="file"
                            accept="*/*"
                            onChange={async (event) => {
                              const file = event.target.files?.[0]
                              if (!file) return
                              if (file.size > 4 * 1024 * 1024) {
                                setFormError(zhShared.feedback.errors.operationFailed)
                                return
                              }
                              try {
                                setContent(await file.text())
                                if (!name) setName(file.name.replace(/\.ya?ml$/i, ''))
                                setFormError('')
                              } catch (error) {
                                setFormError(String(error))
                              }
                            }}
                          />
                        </Button>
                      )}
                      <TextField
                        name="content"
                        label={form?.kind === 'nodes' ? 'URL' : 'YAML'}
                        required
                        multiline
                        minRows={9}
                        disabled={busy}
                        value={content}
                        onChange={(event) => setContent(event.target.value)}
                        placeholder={
                          form?.kind === 'nodes'
                            ? 'vless://…\nhysteria2://…\nanytls://…'
                            : 'proxies:\n  …\nproxy-groups:\n  …\nrules:\n  …'
                        }
                        fullWidth
                        size="small"
                      />
                    </>
                  )}
                  {(form?.kind === 'subscription' ||
                    (form?.kind === 'edit' && Boolean(form.profile.source))) && (
                    <SettingList title={zhProfiles.modals.profileForm.title.edit}>
                      <SettingItem label={zhProfiles.modals.profileForm.fields.allowAutoUpdate}>
                        <Switch
                          checked={profileOption.allowAutoUpdate ?? true}
                          disabled={busy}
                          onChange={(_, checked) =>
                            setProfileOption((old) => ({
                              ...old,
                              allowAutoUpdate: checked,
                            }))
                          }
                        />
                      </SettingItem>
                      <SettingItem label={zhProfiles.modals.profileForm.fields.updateInterval}>
                        <TextField
                          size="small"
                          type="number"
                          disabled={busy || !(profileOption.allowAutoUpdate ?? true)}
                          value={profileOption.updateInterval ?? ''}
                          onChange={(event) => {
                            const value = event.target.value.trim()
                            setProfileOption((old) => ({
                              ...old,
                              updateInterval: value === '' ? undefined : Number(value),
                            }))
                          }}
                          slotProps={{ htmlInput: { min: 0, max: 525600 } }}
                          sx={{ width: 110 }}
                        />
                      </SettingItem>
                      <SettingItem label={zhProfiles.modals.profileForm.fields.useClashProxy}>
                        <Switch
                          checked={profileOption.selfProxy ?? false}
                          disabled={busy}
                          onChange={(_, checked) =>
                            setProfileOption((old) => ({
                              ...old,
                              selfProxy: checked,
                              withProxy: checked ? false : old.withProxy,
                            }))
                          }
                        />
                      </SettingItem>
                      <SettingItem label={zhProfiles.modals.profileForm.fields.userAgent}>
                        <TextField
                          size="small"
                          disabled={busy}
                          value={profileOption.userAgent ?? ''}
                          onChange={(event) =>
                            setProfileOption((old) => ({
                              ...old,
                              userAgent: event.target.value || undefined,
                            }))
                          }
                          sx={{ width: 220 }}
                        />
                      </SettingItem>
                      <SettingItem label={zhProfiles.modals.profileForm.fields.httpTimeout}>
                        <TextField
                          size="small"
                          type="number"
                          disabled={busy}
                          value={profileOption.timeoutSeconds ?? ''}
                          onChange={(event) => {
                            const value = event.target.value.trim()
                            setProfileOption((old) => ({
                              ...old,
                              timeoutSeconds: value === '' ? undefined : Number(value),
                            }))
                          }}
                          slotProps={{ htmlInput: { min: 1, max: 300 } }}
                          sx={{ width: 100 }}
                        />
                      </SettingItem>
                      <SettingItem label={zhProfiles.modals.profileForm.fields.acceptInvalidCerts}>
                        <Switch
                          checked={profileOption.dangerAcceptInvalidCerts ?? false}
                          disabled={busy}
                          onChange={(_, checked) =>
                            setProfileOption((old) => ({
                              ...old,
                              dangerAcceptInvalidCerts: checked,
                            }))
                          }
                        />
                      </SettingItem>
                    </SettingList>
                  )}
                  {formError && <Alert severity="error">{formError}</Alert>}
                </Stack>
              )}
            </DialogContent>
            <DialogActions>
              <Button disabled={busy} onClick={() => setForm(null)}>
                {zhShared.actions.cancel}
              </Button>
              <Button
                color={form?.kind === 'delete' ? 'error' : 'primary'}
                variant="contained"
                type="submit"
                disabled={!canEdit}
                startIcon={
                  form?.kind === 'delete' ? (
                    <DeleteOutlineRounded />
                  ) : form?.kind === 'copy' ? (
                    <ContentCopyRounded />
                  ) : form?.kind === 'edit' ? (
                    <StorageOutlined />
                  ) : form?.kind === 'subscription' ? (
                    <CloudUploadOutlined />
                  ) : (
                    <AddRounded />
                  )
                }
              >
                {busy
                  ? zhShared.statuses.saving
                  : form?.kind === 'subscription'
                    ? zhProfiles.page.actions.import
                    : form?.kind === 'delete'
                      ? zhShared.actions.delete
                      : form?.kind === 'edit'
                        ? zhShared.actions.save
                      : form?.kind === 'copy'
                        ? zhShared.actions.save
                        : zhProfiles.page.actions.import}
              </Button>
            </DialogActions>
          </Box>
        </Dialog>
      </div>
    </VergeMobileTheme>
  )
}
const element = document.getElementById('root')
if (!element) throw new Error('Missing root element')
const render = () => createRoot(element).render(<App />)
void initializeLanguage('zh')
  .catch((error) => console.warn('[mobile] i18n init failed', error))
  .finally(render)
